use crate::ast::*;
use crate::span::Span;
use crate::token::{Keyword, Token, TokenKind};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParseError {
    pub message: String,
    pub span: Span,
}

pub struct Parser {
    tokens: Vec<Token>,
    index: usize,
}

impl Parser {
    pub fn new(tokens: Vec<Token>) -> Self {
        Self { tokens, index: 0 }
    }

    pub fn parse_program(&mut self) -> Result<Program, ParseError> {
        let items = self.parse_items_until(TokenKind::Eof)?;
        Ok(Program { items })
    }

    fn parse_items_until(&mut self, end: TokenKind) -> Result<Vec<Item>, ParseError> {
        let mut items = Vec::new();

        while !self.at(&end) && !self.at(&TokenKind::Eof) {
            self.skip_separators();
            if self.at(&end) || self.at(&TokenKind::Eof) {
                break;
            }
            items.push(self.parse_item()?);
        }

        Ok(items)
    }

    fn parse_item(&mut self) -> Result<Item, ParseError> {
        let directives = self.parse_inline_directives()?;
        if directives.len() == 1 && self.at(&TokenKind::LBrace) {
            return self
                .parse_block_directive_with_header(directives.into_iter().next().unwrap())
                .map(Item::Directive);
        }
        if !directives.is_empty() {
            self.skip_separators();
        }
        let public = self.consume_keyword(Keyword::Pub);
        let is_async = self.consume_keyword(Keyword::Async);
        let is_unsafe = self.consume_keyword(Keyword::Unsafe);

        match self.peek_kind() {
            TokenKind::Keyword(Keyword::Module) => self.parse_module(),
            TokenKind::Keyword(Keyword::Use) => self.parse_use(),
            TokenKind::Keyword(Keyword::Import) => self.parse_import(),
            TokenKind::Keyword(Keyword::Struct) => {
                self.parse_struct(public, directives).map(Item::Struct)
            }
            TokenKind::Keyword(Keyword::Enum) => self.parse_enum(public).map(Item::Enum),
            TokenKind::Keyword(Keyword::Interface) => {
                self.parse_interface(public).map(Item::Interface)
            }
            TokenKind::Keyword(Keyword::Fn) => self
                .parse_function(public, is_async, is_unsafe)
                .map(Item::Function),
            TokenKind::Keyword(Keyword::Extern) => self.parse_extern().map(Item::Extern),
            TokenKind::Keyword(Keyword::Type) => self.parse_type_alias(public).map(Item::TypeAlias),
            TokenKind::Keyword(Keyword::Const) => self.parse_const(public).map(Item::Const),
            TokenKind::Keyword(Keyword::Static) => self.parse_static(public).map(Item::Static),
            TokenKind::Keyword(Keyword::Test) => self.parse_test().map(Item::Test),
            other => self.err(format!("expected item, found {}", other.display())),
        }
    }

    fn parse_module(&mut self) -> Result<Item, ParseError> {
        self.expect_keyword(Keyword::Module)?;
        let path = self.parse_path()?;
        self.consume_item_end();
        Ok(Item::Module(path))
    }

    fn parse_use(&mut self) -> Result<Item, ParseError> {
        self.expect_keyword(Keyword::Use)?;
        let text = self.collect_until_item_end();
        self.consume_item_end();
        Ok(Item::Use(text))
    }

    fn parse_import(&mut self) -> Result<Item, ParseError> {
        self.expect_keyword(Keyword::Import)?;
        let text = self.collect_until_item_end();
        self.consume_item_end();
        Ok(Item::Import(text))
    }

    fn parse_struct(
        &mut self,
        public: bool,
        directives: Vec<InlineDirective>,
    ) -> Result<StructItem, ParseError> {
        self.expect_keyword(Keyword::Struct)?;
        let name = self.expect_ident()?;
        let generics = self.parse_optional_generics();
        let conforms = self.parse_struct_conformance_list();
        self.expect(TokenKind::LBrace)?;
        let mut fields = Vec::new();
        let mut methods = Vec::new();

        while !self.consume(TokenKind::RBrace) {
            self.skip_separators();
            if self.consume(TokenKind::RBrace) {
                break;
            }
            let public = self.consume_keyword(Keyword::Pub);
            let is_async = self.consume_keyword(Keyword::Async);
            let is_unsafe = self.consume_keyword(Keyword::Unsafe);
            if self.at(&TokenKind::Keyword(Keyword::Fn)) {
                methods.push(self.parse_function(public, is_async, is_unsafe)?);
                continue;
            }
            if is_async || is_unsafe {
                return self.err("expected struct method after modifier");
            }
            let field_public = self.consume_keyword(Keyword::Pub);
            let field_public = public || field_public;
            let field_name = self.expect_ident()?;
            self.expect(TokenKind::Colon)?;
            let start = self.index;
            let ty_expr =
                self.parse_type_until(&[TokenKind::Newline, TokenKind::Comma, TokenKind::RBrace])?;
            let ty = self.text_from_tokens(start, self.index);
            fields.push(Field {
                public: field_public,
                name: field_name,
                ty,
                ty_expr,
            });
            self.consume_field_end();
        }

        Ok(StructItem {
            public,
            name,
            generics,
            conforms,
            directives,
            fields,
            methods,
        })
    }

    fn parse_struct_conformance_list(&mut self) -> Vec<String> {
        if !self.consume(TokenKind::Colon) {
            return Vec::new();
        }

        let mut conforms = Vec::new();
        loop {
            let item = self.collect_until_any(&[TokenKind::Comma, TokenKind::LBrace]);
            let item = item.trim();
            if !item.is_empty() {
                conforms.push(item.to_string());
            }
            if !self.consume(TokenKind::Comma) {
                break;
            }
        }
        conforms
    }

    fn parse_enum(&mut self, public: bool) -> Result<EnumItem, ParseError> {
        self.expect_keyword(Keyword::Enum)?;
        let name = self.expect_ident()?;
        let generics = self.parse_optional_generics();
        self.expect(TokenKind::LBrace)?;
        let mut variants = Vec::new();

        while !self.consume(TokenKind::RBrace) {
            self.skip_separators();
            if self.consume(TokenKind::RBrace) {
                break;
            }
            let variant_name = self.expect_ident()?;
            let payload = if self.consume(TokenKind::LParen) {
                Some(self.collect_balanced(TokenKind::LParen, TokenKind::RParen)?)
            } else if self.consume(TokenKind::LBrace) {
                Some(format!(
                    "{{{}}}",
                    self.collect_balanced(TokenKind::LBrace, TokenKind::RBrace)?
                ))
            } else {
                None
            };
            variants.push(EnumVariant {
                name: variant_name,
                payload,
            });
            self.consume_field_end();
        }

        Ok(EnumItem {
            public,
            name,
            generics,
            variants,
        })
    }

    fn parse_interface(&mut self, public: bool) -> Result<InterfaceItem, ParseError> {
        self.expect_keyword(Keyword::Interface)?;
        let name = self.expect_ident()?;
        let generics = self.parse_optional_generics();
        let mut extends = Vec::new();
        if self.consume(TokenKind::Colon) {
            extends.push(self.collect_until(TokenKind::LBrace));
        }
        self.expect(TokenKind::LBrace)?;
        let mut methods = Vec::new();

        while !self.consume(TokenKind::RBrace) {
            self.skip_separators();
            if self.consume(TokenKind::RBrace) {
                break;
            }
            let is_async = self.consume_keyword(Keyword::Async);
            let is_unsafe = self.consume_keyword(Keyword::Unsafe);
            methods.push(self.parse_signature(is_async, is_unsafe)?);
            self.consume_item_end();
        }

        Ok(InterfaceItem {
            public,
            name,
            generics,
            extends,
            methods,
        })
    }

    fn parse_function(
        &mut self,
        public: bool,
        is_async: bool,
        is_unsafe: bool,
    ) -> Result<FunctionItem, ParseError> {
        let span = self.tokens[self.index].span;
        self.expect_keyword(Keyword::Fn)?;
        let name = self.expect_ident()?;
        let generics = self.parse_optional_generics();
        let params = self.parse_params()?;
        let (return_type, return_type_expr) = self.parse_optional_return_type();
        let body = if self.at(&TokenKind::LBrace) {
            Some(self.parse_block()?)
        } else {
            None
        };
        self.consume_item_end();
        Ok(FunctionItem {
            span,
            source_location: None,
            public,
            is_async,
            is_unsafe,
            name,
            generics,
            params,
            return_type,
            return_type_expr,
            body,
        })
    }

    fn parse_signature(
        &mut self,
        is_async: bool,
        is_unsafe: bool,
    ) -> Result<FunctionSignature, ParseError> {
        self.expect_keyword(Keyword::Fn)?;
        let name = self.expect_ident()?;
        let _generics = self.parse_optional_generics();
        let params = self.parse_params()?;
        let (return_type, return_type_expr) = self.parse_optional_return_type();
        Ok(FunctionSignature {
            is_async,
            is_unsafe,
            name,
            params,
            return_type,
            return_type_expr,
        })
    }

    fn parse_extern(&mut self) -> Result<ExternBlock, ParseError> {
        self.expect_keyword(Keyword::Extern)?;
        let abi = match self.peek_kind() {
            TokenKind::String(value) => {
                let value = value.clone();
                self.advance();
                Some(value)
            }
            _ => None,
        };
        self.expect(TokenKind::LBrace)?;
        let mut functions = Vec::new();
        while !self.consume(TokenKind::RBrace) {
            self.skip_separators();
            if self.consume(TokenKind::RBrace) {
                break;
            }
            let is_async = self.consume_keyword(Keyword::Async);
            let is_unsafe = self.consume_keyword(Keyword::Unsafe);
            functions.push(self.parse_signature(is_async, is_unsafe)?);
            self.consume_item_end();
        }
        Ok(ExternBlock { abi, functions })
    }

    fn parse_type_alias(&mut self, public: bool) -> Result<TypeAliasItem, ParseError> {
        self.expect_keyword(Keyword::Type)?;
        let name = self.expect_ident()?;
        let generics = self.parse_optional_generics();
        self.expect(TokenKind::Eq)?;
        let start = self.index;
        let value_expr =
            self.parse_type_until(&[TokenKind::Newline, TokenKind::Semicolon, TokenKind::Eof])?;
        let value = self.text_from_tokens(start, self.index);
        self.consume_item_end();
        Ok(TypeAliasItem {
            public,
            name,
            generics,
            value,
            value_expr,
        })
    }

    fn parse_const(&mut self, public: bool) -> Result<ConstItem, ParseError> {
        self.expect_keyword(Keyword::Const)?;
        let name = self.expect_ident()?;
        let ty = if self.consume(TokenKind::Colon) {
            let start = self.index;
            let ty_expr = self.parse_type_until(&[TokenKind::Eq]);
            let ty = self.text_from_tokens(start, self.index);
            Some((ty, ty_expr?))
        } else {
            None
        };
        self.expect(TokenKind::Eq)?;
        let value = self.collect_until_item_end();
        self.consume_item_end();
        let (ty, ty_expr) = unzip_type_pair(ty);
        Ok(ConstItem {
            public,
            name,
            ty,
            ty_expr,
            value,
        })
    }

    fn parse_static(&mut self, public: bool) -> Result<StaticItem, ParseError> {
        self.expect_keyword(Keyword::Static)?;
        let name = self.expect_ident()?;
        let ty = if self.consume(TokenKind::Colon) {
            let start = self.index;
            let ty_expr = self.parse_type_until(&[TokenKind::Eq]);
            let ty = self.text_from_tokens(start, self.index);
            Some((ty, ty_expr?))
        } else {
            None
        };
        self.expect(TokenKind::Eq)?;
        let value = self.collect_until_item_end();
        self.consume_item_end();
        let (ty, ty_expr) = unzip_type_pair(ty);
        Ok(StaticItem {
            public,
            name,
            ty,
            ty_expr,
            value,
        })
    }

    fn parse_test(&mut self) -> Result<TestItem, ParseError> {
        self.expect_keyword(Keyword::Test)?;
        let name = match self.peek_kind() {
            TokenKind::String(value) => {
                let value = value.clone();
                self.advance();
                value
            }
            _ => self.expect_ident()?,
        };
        let body = self.parse_block()?;
        Ok(TestItem { name, body })
    }

    fn parse_block(&mut self) -> Result<Block, ParseError> {
        self.expect(TokenKind::LBrace)?;
        let mut statements = Vec::new();

        while !self.consume(TokenKind::RBrace) {
            self.skip_separators();
            if self.consume(TokenKind::RBrace) {
                break;
            }
            statements.push(self.parse_statement()?);
            self.consume_item_end();
        }

        Ok(Block { statements })
    }

    fn parse_statement(&mut self) -> Result<Stmt, ParseError> {
        let kind = self.current_stmt_kind();
        let start = self.index;
        let data = match kind {
            StmtKind::Let => self.parse_let_statement()?,
            StmtKind::Return => self.parse_return_statement()?,
            StmtKind::Break => self.parse_break_statement()?,
            StmtKind::Continue => {
                self.advance();
                StmtData::Continue
            }
            StmtKind::If => StmtData::If(self.parse_if_control()?),
            StmtKind::Match => StmtData::Match(self.parse_match_expression()?),
            StmtKind::While => StmtData::While(self.parse_while_statement()?),
            StmtKind::For => StmtData::For(self.parse_for_statement()?),
            StmtKind::Loop => StmtData::Loop(self.parse_loop_statement()?),
            StmtKind::Unsafe => StmtData::Unsafe(self.parse_unsafe_statement()?),
            StmtKind::Expr => StmtData::Expr(self.parse_expression_until_statement_end()?),
        };
        let text = self.text_from_tokens(start, self.index);
        Ok(Stmt { kind, text, data })
    }

    fn current_stmt_kind(&self) -> StmtKind {
        match self.peek_kind() {
            TokenKind::Keyword(Keyword::Let) => StmtKind::Let,
            TokenKind::Keyword(Keyword::Return) => StmtKind::Return,
            TokenKind::Keyword(Keyword::Break) => StmtKind::Break,
            TokenKind::Keyword(Keyword::Continue) => StmtKind::Continue,
            TokenKind::Keyword(Keyword::If) => StmtKind::If,
            TokenKind::Keyword(Keyword::Match) => StmtKind::Match,
            TokenKind::Keyword(Keyword::While) => StmtKind::While,
            TokenKind::Keyword(Keyword::For) => StmtKind::For,
            TokenKind::Keyword(Keyword::Unsafe) => StmtKind::Unsafe,
            TokenKind::Ident(value) if value == "loop" => StmtKind::Loop,
            _ => StmtKind::Expr,
        }
    }

    fn parse_let_statement(&mut self) -> Result<StmtData, ParseError> {
        self.expect_keyword(Keyword::Let)?;
        let mutable = self.consume_keyword(Keyword::Mut);
        let name = self.expect_ident()?;
        let ty = if self.consume(TokenKind::Colon) {
            let start = self.index;
            let ty_expr =
                self.parse_type_until(&[TokenKind::Eq, TokenKind::Newline, TokenKind::Semicolon])?;
            let ty = self.text_from_tokens(start, self.index);
            Some((ty, ty_expr))
        } else {
            None
        };
        let value = if self.consume(TokenKind::Eq) {
            Some(self.parse_expression_until_statement_end()?)
        } else {
            None
        };
        let (ty, ty_expr) = unzip_type_pair(ty);
        Ok(StmtData::Let(LetStmt {
            mutable,
            name,
            ty,
            ty_expr,
            value,
        }))
    }

    fn parse_return_statement(&mut self) -> Result<StmtData, ParseError> {
        self.expect_keyword(Keyword::Return)?;
        if self.at_statement_end() {
            Ok(StmtData::Return(None))
        } else {
            Ok(StmtData::Return(Some(
                self.parse_expression_until_statement_end()?,
            )))
        }
    }

    fn parse_break_statement(&mut self) -> Result<StmtData, ParseError> {
        self.expect_keyword(Keyword::Break)?;
        if self.at_statement_end() {
            Ok(StmtData::Break(None))
        } else {
            Ok(StmtData::Break(Some(
                self.parse_expression_until_statement_end()?,
            )))
        }
    }

    fn parse_if_control(&mut self) -> Result<ControlStmt, ParseError> {
        let if_expr = self.parse_if_expression()?;
        Ok(ControlStmt {
            condition: Some(*if_expr.condition),
            body: if_expr.then_branch,
        })
    }

    fn parse_while_statement(&mut self) -> Result<ControlStmt, ParseError> {
        self.expect_keyword(Keyword::While)?;
        let condition = self.parse_expression_until(TokenKind::LBrace)?;
        let body = self.parse_block()?;
        Ok(ControlStmt {
            condition: Some(condition),
            body,
        })
    }

    fn parse_for_statement(&mut self) -> Result<ForStmt, ParseError> {
        self.expect_keyword(Keyword::For)?;
        let pattern = self.collect_until_keyword(Keyword::In);
        self.expect_keyword(Keyword::In)?;
        let iterator = self.parse_expression_until(TokenKind::LBrace)?;
        let body = self.parse_block()?;
        Ok(ForStmt {
            pattern,
            iterator,
            body,
        })
    }

    fn parse_loop_statement(&mut self) -> Result<Block, ParseError> {
        self.expect_ident()?;
        self.parse_block()
    }

    fn parse_unsafe_statement(&mut self) -> Result<Block, ParseError> {
        self.expect_keyword(Keyword::Unsafe)?;
        self.parse_block()
    }

    fn parse_expression_until_statement_end(&mut self) -> Result<Expr, ParseError> {
        self.parse_expression_bp(0, &|parser| parser.at_statement_end())
    }

    fn parse_expression_until(&mut self, end: TokenKind) -> Result<Expr, ParseError> {
        self.parse_expression_bp(0, &|parser| parser.at(&end))
    }

    fn parse_expression_bp(
        &mut self,
        min_bp: u8,
        is_end: &dyn Fn(&Self) -> bool,
    ) -> Result<Expr, ParseError> {
        self.skip_separators();
        if is_end(self) || self.at(&TokenKind::Eof) {
            return Ok(Expr::Missing);
        }

        let mut lhs = self.parse_prefix_expr(is_end)?;

        loop {
            self.skip_inline_newlines_for_postfix();
            if is_end(self) || self.at(&TokenKind::Eof) {
                break;
            }

            if self.consume(TokenKind::Question) {
                lhs = Expr::Try(Box::new(lhs));
                continue;
            }

            if self.consume(TokenKind::Dot) {
                if self.consume_keyword(Keyword::Await) {
                    lhs = Expr::Await(Box::new(lhs));
                    continue;
                }
                let member = self.expect_ident()?;
                lhs = Expr::Member(MemberExpr {
                    target: Box::new(lhs),
                    member,
                });
                continue;
            }

            if self.consume(TokenKind::LBracket) {
                let index = self.parse_expression_until(TokenKind::RBracket)?;
                self.expect(TokenKind::RBracket)?;
                lhs = Expr::Index(crate::ast::IndexExpr {
                    target: Box::new(lhs),
                    index: Box::new(index),
                });
                continue;
            }

            let type_args = if self.at(&TokenKind::Lt) && looks_like_generic_call(self) {
                self.parse_optional_generics()
            } else {
                None
            };

            if self.consume(TokenKind::LParen) {
                let args = self.parse_expr_list(TokenKind::RParen)?;
                lhs = Expr::Call(CallExpr {
                    callee: Box::new(lhs),
                    type_args,
                    args,
                });
                continue;
            }

            let Some((left_bp, right_bp, op)) = self.current_binary_op() else {
                break;
            };
            if left_bp < min_bp {
                break;
            }
            self.advance();
            let rhs = self.parse_expression_bp(right_bp, is_end)?;
            lhs = Expr::Binary(BinaryExpr {
                op,
                left: Box::new(lhs),
                right: Box::new(rhs),
            });
        }

        Ok(lhs)
    }

    fn parse_prefix_expr(&mut self, is_end: &dyn Fn(&Self) -> bool) -> Result<Expr, ParseError> {
        self.skip_separators();
        match self.peek_kind().clone() {
            TokenKind::Keyword(Keyword::If) => self.parse_if_expression().map(Expr::If),
            TokenKind::Keyword(Keyword::Match) => self.parse_match_expression().map(Expr::Match),
            TokenKind::Keyword(Keyword::Async) => {
                self.advance();
                if self.at(&TokenKind::Keyword(Keyword::Fn)) {
                    self.parse_closure(true, false).map(Expr::Closure)
                } else {
                    Ok(Expr::Ident("async".to_string()))
                }
            }
            TokenKind::Keyword(Keyword::Move) => {
                self.advance();
                self.parse_closure(false, true).map(Expr::Closure)
            }
            TokenKind::Keyword(Keyword::Mut) => {
                self.advance();
                let expr = self.parse_expression_bp(9, is_end)?;
                Ok(Expr::Mut(Box::new(expr)))
            }
            TokenKind::Keyword(Keyword::Fn) => self.parse_closure(false, false).map(Expr::Closure),
            TokenKind::Keyword(Keyword::True) => {
                self.advance();
                Ok(Expr::Literal(Literal::Bool(true)))
            }
            TokenKind::Keyword(Keyword::False) => {
                self.advance();
                Ok(Expr::Literal(Literal::Bool(false)))
            }
            TokenKind::Int(value) => {
                self.advance();
                Ok(Expr::Literal(Literal::Int(value)))
            }
            TokenKind::Float(value) => {
                self.advance();
                Ok(Expr::Literal(Literal::Float(value)))
            }
            TokenKind::String(value) => {
                self.advance();
                Ok(Expr::Literal(Literal::String(value)))
            }
            TokenKind::Char(value) => {
                self.advance();
                Ok(Expr::Literal(Literal::Char(value)))
            }
            TokenKind::Ident(_) | TokenKind::Keyword(Keyword::Self_) => {
                self.parse_path_or_struct(is_end)
            }
            TokenKind::LParen => {
                self.advance();
                if self.consume(TokenKind::RParen) {
                    Ok(Expr::Literal(Literal::Unit))
                } else {
                    let expr =
                        self.parse_expression_bp(0, &|parser| parser.at(&TokenKind::RParen))?;
                    self.expect(TokenKind::RParen)?;
                    Ok(expr)
                }
            }
            TokenKind::LBrace if self.brace_looks_like_object_literal() => {
                self.parse_object_literal().map(Expr::Object)
            }
            TokenKind::LBrace => self.parse_block().map(Expr::Block),
            TokenKind::Amp => {
                self.advance();
                let op = if self.consume_keyword(Keyword::Mut) {
                    UnaryOp::MutRef
                } else {
                    UnaryOp::Ref
                };
                let expr = self.parse_expression_bp(9, is_end)?;
                Ok(Expr::Unary(UnaryExpr {
                    op,
                    expr: Box::new(expr),
                }))
            }
            TokenKind::Star => {
                self.advance();
                let expr = self.parse_expression_bp(9, is_end)?;
                Ok(Expr::Unary(UnaryExpr {
                    op: UnaryOp::Deref,
                    expr: Box::new(expr),
                }))
            }
            TokenKind::Minus => {
                self.advance();
                let expr = self.parse_expression_bp(9, is_end)?;
                Ok(Expr::Unary(UnaryExpr {
                    op: UnaryOp::Neg,
                    expr: Box::new(expr),
                }))
            }
            TokenKind::Bang => {
                self.advance();
                let expr = self.parse_expression_bp(9, is_end)?;
                Ok(Expr::Unary(UnaryExpr {
                    op: UnaryOp::Not,
                    expr: Box::new(expr),
                }))
            }
            other => self.err(format!("expected expression, found {}", other.display())),
        }
    }

    fn parse_path_or_struct(&mut self, is_end: &dyn Fn(&Self) -> bool) -> Result<Expr, ParseError> {
        let first = self.expect_ident()?;
        let path = vec![first.clone()];
        let name = first;
        if !is_end(self) && self.at(&TokenKind::LBrace) && self.brace_looks_like_struct_literal() {
            self.advance();
            let fields = self.parse_struct_expr_fields()?;
            Ok(Expr::Struct(StructExpr { name, fields }))
        } else if path.len() == 1 {
            Ok(Expr::Ident(name))
        } else {
            Ok(Expr::Path(path))
        }
    }

    fn parse_expr_path(&mut self) -> Result<Vec<String>, ParseError> {
        let mut segments = vec![self.expect_ident()?];
        while self.consume(TokenKind::Dot) {
            if self.at(&TokenKind::Keyword(Keyword::Await)) {
                self.index -= 1;
                break;
            }
            segments.push(self.expect_ident()?);
        }
        Ok(segments)
    }

    fn parse_struct_expr_fields(&mut self) -> Result<Vec<StructFieldExpr>, ParseError> {
        let mut fields = Vec::new();
        while !self.consume(TokenKind::RBrace) {
            self.skip_separators();
            if self.consume(TokenKind::RBrace) {
                break;
            }
            let name = self.expect_ident()?;
            let value = if self.consume(TokenKind::Colon) {
                Some(self.parse_expression_bp(0, &|parser| {
                    parser.at(&TokenKind::Comma) || parser.at(&TokenKind::RBrace)
                })?)
            } else {
                None
            };
            fields.push(StructFieldExpr { name, value });
            self.consume(TokenKind::Comma);
        }
        Ok(fields)
    }

    fn parse_object_literal(&mut self) -> Result<ObjectExpr, ParseError> {
        self.expect(TokenKind::LBrace)?;
        let mut fields = Vec::new();
        while !self.consume(TokenKind::RBrace) {
            self.skip_separators();
            if self.consume(TokenKind::RBrace) {
                break;
            }
            let key = match self.peek_kind().clone() {
                TokenKind::String(value) | TokenKind::Ident(value) => {
                    self.advance();
                    value
                }
                other => {
                    return self.err(format!("expected object key, found {}", other.display()))
                }
            };
            self.expect(TokenKind::Colon)?;
            let value = self.parse_expression_bp(0, &|parser| {
                parser.at(&TokenKind::Comma) || parser.at(&TokenKind::RBrace)
            })?;
            fields.push(ObjectFieldExpr { key, value });
            self.consume(TokenKind::Comma);
        }
        Ok(ObjectExpr { fields })
    }

    fn parse_closure(&mut self, is_async: bool, is_move: bool) -> Result<ClosureExpr, ParseError> {
        self.expect_keyword(Keyword::Fn)?;
        let params = self.parse_params()?;
        let (return_type, return_type_expr) = self.parse_optional_return_type();
        let body = self.parse_block()?;
        Ok(ClosureExpr {
            is_async,
            is_move,
            params,
            return_type,
            return_type_expr,
            body,
        })
    }

    fn parse_if_expression(&mut self) -> Result<IfExpr, ParseError> {
        self.expect_keyword(Keyword::If)?;
        let condition = self.parse_expression_until(TokenKind::LBrace)?;
        let then_branch = self.parse_block()?;
        let else_branch = if self.consume_keyword(Keyword::Else) {
            if self.at(&TokenKind::Keyword(Keyword::If)) {
                let nested = self.parse_if_expression()?;
                Some(Block {
                    statements: vec![Stmt {
                        kind: StmtKind::If,
                        text: String::new(),
                        data: StmtData::If(ControlStmt {
                            condition: Some(*nested.condition),
                            body: nested.then_branch,
                        }),
                    }],
                })
            } else {
                Some(self.parse_block()?)
            }
        } else {
            None
        };
        Ok(IfExpr {
            condition: Box::new(condition),
            then_branch,
            else_branch,
        })
    }

    fn parse_match_expression(&mut self) -> Result<MatchExpr, ParseError> {
        self.expect_keyword(Keyword::Match)?;
        let value = self.parse_expression_until(TokenKind::LBrace)?;
        self.expect(TokenKind::LBrace)?;
        let mut arms = Vec::new();
        while !self.consume(TokenKind::RBrace) {
            self.skip_separators();
            if self.consume(TokenKind::RBrace) {
                break;
            }
            let pattern = self.collect_match_pattern();
            self.expect(TokenKind::FatArrow)?;
            let body = self.parse_expression_bp(0, &|parser| {
                parser.at(&TokenKind::Newline) || parser.at(&TokenKind::RBrace)
            })?;
            arms.push(MatchArm { pattern, body });
            self.consume_item_end();
        }
        Ok(MatchExpr {
            value: Box::new(value),
            arms,
        })
    }

    fn parse_expr_list(&mut self, close: TokenKind) -> Result<Vec<Expr>, ParseError> {
        let mut args = Vec::new();
        while !self.consume(close.clone()) {
            self.skip_separators();
            if self.consume(close.clone()) {
                break;
            }
            let expr = self.parse_expression_bp(0, &|parser| {
                parser.at(&TokenKind::Comma) || parser.at(&close)
            })?;
            args.push(expr);
            self.consume(TokenKind::Comma);
        }
        Ok(args)
    }

    fn current_binary_op(&self) -> Option<(u8, u8, BinaryOp)> {
        Some(match self.peek_kind() {
            TokenKind::Eq => (1, 1, BinaryOp::Assign),
            TokenKind::PlusEq => (1, 1, BinaryOp::AddAssign),
            TokenKind::MinusEq => (1, 1, BinaryOp::SubAssign),
            TokenKind::StarEq => (1, 1, BinaryOp::MulAssign),
            TokenKind::SlashEq => (1, 1, BinaryOp::DivAssign),
            TokenKind::PercentEq => (1, 1, BinaryOp::RemAssign),
            TokenKind::AmpEq => (1, 1, BinaryOp::BitAndAssign),
            TokenKind::PipeEq => (1, 1, BinaryOp::BitOrAssign),
            TokenKind::LtLtEq => (1, 1, BinaryOp::ShlAssign),
            TokenKind::GtGtEq => (1, 1, BinaryOp::ShrAssign),
            TokenKind::PipePipe => (2, 3, BinaryOp::BoolOr),
            TokenKind::AmpAmp => (4, 5, BinaryOp::BoolAnd),
            TokenKind::Pipe => (6, 7, BinaryOp::BitOr),
            TokenKind::Caret => (8, 9, BinaryOp::BitXor),
            TokenKind::Amp => (10, 11, BinaryOp::BitAnd),
            TokenKind::EqEq => (12, 13, BinaryOp::Eq),
            TokenKind::BangEq => (12, 13, BinaryOp::NotEq),
            TokenKind::Lt => (14, 15, BinaryOp::Lt),
            TokenKind::Le => (14, 15, BinaryOp::Le),
            TokenKind::Gt => (14, 15, BinaryOp::Gt),
            TokenKind::Ge => (14, 15, BinaryOp::Ge),
            TokenKind::LtLt => (16, 17, BinaryOp::Shl),
            TokenKind::GtGt => (16, 17, BinaryOp::Shr),
            TokenKind::Plus => (18, 19, BinaryOp::Add),
            TokenKind::Minus => (18, 19, BinaryOp::Sub),
            TokenKind::Star => (20, 21, BinaryOp::Mul),
            TokenKind::Slash => (20, 21, BinaryOp::Div),
            TokenKind::Percent => (20, 21, BinaryOp::Rem),
            _ => return None,
        })
    }

    fn at_statement_end(&self) -> bool {
        self.at(&TokenKind::Newline)
            || self.at(&TokenKind::Semicolon)
            || self.at(&TokenKind::RBrace)
            || self.at(&TokenKind::Eof)
    }

    fn skip_inline_newlines_for_postfix(&mut self) {
        while self.at(&TokenKind::Newline) {
            let next = self.tokens.get(self.index + 1).map(|token| &token.kind);
            if matches!(next, Some(TokenKind::Dot) | Some(TokenKind::Question)) {
                self.advance();
            } else {
                break;
            }
        }
    }

    fn brace_looks_like_struct_literal(&self) -> bool {
        let mut idx = self.index + 1;
        while matches!(
            self.tokens.get(idx).map(|token| &token.kind),
            Some(TokenKind::Newline)
        ) {
            idx += 1;
        }
        matches!(
            self.tokens.get(idx).map(|token| &token.kind),
            Some(TokenKind::Ident(_))
        )
    }

    fn brace_looks_like_object_literal(&self) -> bool {
        let mut idx = self.index + 1;
        while matches!(
            self.tokens.get(idx).map(|token| &token.kind),
            Some(TokenKind::Newline)
        ) {
            idx += 1;
        }
        matches!(
            (
                self.tokens.get(idx).map(|token| &token.kind),
                self.tokens.get(idx + 1).map(|token| &token.kind)
            ),
            (Some(TokenKind::String(_)), Some(TokenKind::Colon))
                | (Some(TokenKind::Ident(_)), Some(TokenKind::Colon))
        )
    }

    fn collect_match_pattern(&mut self) -> String {
        let mut parts = Vec::new();
        let mut brace_depth = 0usize;
        let mut paren_depth = 0usize;
        while !self.at(&TokenKind::Eof) {
            if self.at(&TokenKind::FatArrow) && brace_depth == 0 && paren_depth == 0 {
                break;
            }
            match self.peek_kind() {
                TokenKind::LBrace => brace_depth += 1,
                TokenKind::RBrace => brace_depth = brace_depth.saturating_sub(1),
                TokenKind::LParen => paren_depth += 1,
                TokenKind::RParen => paren_depth = paren_depth.saturating_sub(1),
                _ => {}
            }
            parts.push(self.advance().kind.display());
        }
        join_tokens(&parts)
    }

    fn parse_block_directive_with_header(
        &mut self,
        directive: InlineDirective,
    ) -> Result<DirectiveItem, ParseError> {
        self.expect(TokenKind::LBrace)?;
        let items = self.parse_items_until(TokenKind::RBrace)?;
        self.expect(TokenKind::RBrace)?;
        Ok(DirectiveItem {
            name: directive.name,
            args: directive.args,
            items,
        })
    }

    fn parse_inline_directives(&mut self) -> Result<Vec<InlineDirective>, ParseError> {
        let mut directives = Vec::new();
        while self.at(&TokenKind::At) {
            let directive = self.parse_directive_header()?;
            directives.push(directive);
            if self.at(&TokenKind::LBrace) {
                break;
            }
        }
        Ok(directives)
    }

    fn parse_directive_header(&mut self) -> Result<InlineDirective, ParseError> {
        self.expect(TokenKind::At)?;
        let name = self.expect_ident()?;
        let args = if self.consume(TokenKind::LParen) {
            self.collect_balanced(TokenKind::LParen, TokenKind::RParen)?
        } else {
            String::new()
        };
        Ok(InlineDirective { name, args })
    }

    fn parse_path(&mut self) -> Result<Path, ParseError> {
        let mut segments = vec![self.expect_ident()?];
        while self.consume(TokenKind::Dot) {
            segments.push(self.expect_ident()?);
        }
        Ok(Path { segments })
    }

    fn parse_params(&mut self) -> Result<Vec<Param>, ParseError> {
        self.expect(TokenKind::LParen)?;
        let mut params = Vec::new();
        while !self.consume(TokenKind::RParen) {
            self.skip_separators();
            if self.consume(TokenKind::RParen) {
                break;
            }
            let mutable = self.consume_keyword(Keyword::Mut);
            let name = match self.peek_kind() {
                TokenKind::Amp => {
                    self.advance();
                    let receiver_mut = self.consume_keyword(Keyword::Mut);
                    self.expect_keyword(Keyword::Self_)?;
                    if receiver_mut {
                        "&mut self".to_string()
                    } else {
                        "&self".to_string()
                    }
                }
                TokenKind::Keyword(Keyword::Self_) => {
                    self.advance();
                    "self".to_string()
                }
                _ => self.expect_ident()?,
            };
            let ty = if self.consume(TokenKind::Colon) {
                let start = self.index;
                let ty_expr = self.parse_type_until(&[TokenKind::Comma, TokenKind::RParen])?;
                let ty = self.text_from_tokens(start, self.index);
                Some((ty, ty_expr))
            } else {
                None
            };
            let (ty, ty_expr) = unzip_type_pair(ty);
            params.push(Param {
                mutable,
                name,
                ty,
                ty_expr,
            });
            self.consume(TokenKind::Comma);
        }
        Ok(params)
    }

    fn parse_optional_generics(&mut self) -> Option<String> {
        if self.consume(TokenKind::Lt) {
            self.collect_balanced(TokenKind::Lt, TokenKind::Gt).ok()
        } else {
            None
        }
    }

    fn parse_optional_return_type(&mut self) -> (Option<String>, Option<TypeExpr>) {
        if self.consume(TokenKind::Arrow) {
            let start = self.index;
            let ty_expr = self.parse_type_until(&[
                TokenKind::LBrace,
                TokenKind::Newline,
                TokenKind::Semicolon,
            ]);
            let ty = self.text_from_tokens(start, self.index);
            match ty_expr {
                Ok(ty_expr) => (Some(ty), Some(ty_expr)),
                Err(_) => (Some(ty), Some(TypeExpr::Missing)),
            }
        } else {
            (None, None)
        }
    }

    fn parse_type_until(&mut self, ends: &[TokenKind]) -> Result<TypeExpr, ParseError> {
        self.skip_separators();
        if self.at_type_end(ends) {
            return Ok(TypeExpr::Missing);
        }
        self.parse_type_expr(ends)
    }

    fn parse_type_expr(&mut self, ends: &[TokenKind]) -> Result<TypeExpr, ParseError> {
        self.skip_separators();
        let mut ty = match self.peek_kind().clone() {
            TokenKind::Keyword(Keyword::Async) => {
                self.advance();
                self.expect_keyword(Keyword::Fn)?;
                self.parse_fn_type(true, ends)?
            }
            TokenKind::Keyword(Keyword::Fn) => {
                self.advance();
                self.parse_fn_type(false, ends)?
            }
            TokenKind::Amp => {
                self.advance();
                let mutable = self.consume_keyword(Keyword::Mut);
                let inner = self.parse_type_expr(ends)?;
                TypeExpr::Ref {
                    mutable,
                    inner: Box::new(inner),
                }
            }
            TokenKind::Star => {
                self.advance();
                let mutable = if self.consume_keyword(Keyword::Mut) {
                    true
                } else {
                    self.expect_keyword(Keyword::Const)?;
                    false
                };
                let inner = self.parse_type_expr(ends)?;
                TypeExpr::RawPtr {
                    mutable,
                    inner: Box::new(inner),
                }
            }
            TokenKind::Keyword(Keyword::Mut) => {
                self.advance();
                TypeExpr::Mut(Box::new(self.parse_type_expr(ends)?))
            }
            TokenKind::LParen => self.parse_tuple_type()?,
            TokenKind::Ident(_) | TokenKind::Keyword(Keyword::Self_) => {
                TypeExpr::Path(self.parse_expr_path()?)
            }
            other => return self.err(format!("expected type, found {}", other.display())),
        };

        if self.at(&TokenKind::Lt) {
            self.advance();
            let mut args = Vec::new();
            while !self.consume_type_gt() {
                self.skip_separators();
                if self.consume_type_gt() {
                    break;
                }
                args.push(self.parse_type_expr(&[TokenKind::Comma, TokenKind::Gt])?);
                self.consume(TokenKind::Comma);
            }
            ty = TypeExpr::Generic {
                base: Box::new(ty),
                args,
            };
        }

        while !self.at_type_end(ends)
            && !matches!(
                self.peek_kind(),
                TokenKind::Eof
                    | TokenKind::Comma
                    | TokenKind::RParen
                    | TokenKind::RBrace
                    | TokenKind::LBrace
                    | TokenKind::Newline
                    | TokenKind::Semicolon
            )
        {
            self.advance();
        }

        Ok(ty)
    }

    fn at_type_end(&self, ends: &[TokenKind]) -> bool {
        ends.iter().any(|end| self.at(end))
            || (ends.iter().any(|end| matches!(end, TokenKind::Gt))
                && matches!(self.peek_kind(), TokenKind::GtGt))
    }

    fn consume_type_gt(&mut self) -> bool {
        match self.peek_kind() {
            TokenKind::Gt => {
                self.advance();
                true
            }
            TokenKind::GtGt => {
                if let Some(token) = self.tokens.get_mut(self.index) {
                    let start = token.span.start;
                    let end = token.span.end;
                    token.kind = TokenKind::Gt;
                    token.span = Span::new(start, start + 1);
                    self.tokens
                        .insert(self.index + 1, Token::new(TokenKind::Gt, start + 1, end));
                }
                self.advance();
                true
            }
            _ => false,
        }
    }

    fn parse_tuple_type(&mut self) -> Result<TypeExpr, ParseError> {
        self.expect(TokenKind::LParen)?;
        if self.consume(TokenKind::RParen) {
            return Ok(TypeExpr::Tuple(Vec::new()));
        }
        let mut items = Vec::new();
        while !self.consume(TokenKind::RParen) {
            items.push(self.parse_type_expr(&[TokenKind::Comma, TokenKind::RParen])?);
            self.consume(TokenKind::Comma);
        }
        Ok(TypeExpr::Tuple(items))
    }

    fn parse_fn_type(
        &mut self,
        is_async: bool,
        ends: &[TokenKind],
    ) -> Result<TypeExpr, ParseError> {
        self.expect(TokenKind::LParen)?;
        let mut params = Vec::new();
        while !self.consume(TokenKind::RParen) {
            params.push(self.parse_type_expr(&[TokenKind::Comma, TokenKind::RParen])?);
            self.consume(TokenKind::Comma);
        }
        let return_type = if self.consume(TokenKind::Arrow) {
            let mut return_ends = vec![
                TokenKind::Newline,
                TokenKind::Semicolon,
                TokenKind::LBrace,
                TokenKind::Comma,
                TokenKind::RParen,
                TokenKind::Gt,
            ];
            for end in ends {
                if !return_ends.contains(end) {
                    return_ends.push(end.clone());
                }
            }
            self.parse_type_expr(&return_ends)?
        } else {
            TypeExpr::Tuple(Vec::new())
        };
        Ok(TypeExpr::Fn {
            is_async,
            params,
            return_type: Box::new(return_type),
        })
    }

    fn collect_balanced(
        &mut self,
        open: TokenKind,
        close: TokenKind,
    ) -> Result<String, ParseError> {
        let mut depth = 1;
        let mut parts = Vec::new();
        while !self.at(&TokenKind::Eof) {
            let kind = self.peek_kind().clone();
            if kind == open {
                depth += 1;
                parts.push(self.advance().kind.display());
            } else if kind == close {
                depth -= 1;
                if depth == 0 {
                    self.advance();
                    return Ok(join_tokens(&parts));
                }
                parts.push(self.advance().kind.display());
            } else {
                parts.push(self.advance().kind.display());
            }
        }
        self.err("unterminated balanced construct")
    }

    fn collect_until(&mut self, end: TokenKind) -> String {
        let mut parts = Vec::new();
        while !self.at(&end) && !self.at(&TokenKind::Eof) {
            parts.push(self.advance().kind.display());
        }
        join_tokens(&parts)
    }

    fn collect_until_keyword(&mut self, keyword: Keyword) -> String {
        let mut parts = Vec::new();
        while !self.at(&TokenKind::Keyword(keyword)) && !self.at(&TokenKind::Eof) {
            parts.push(self.advance().kind.display());
        }
        join_tokens(&parts)
    }

    fn collect_until_any(&mut self, ends: &[TokenKind]) -> String {
        let mut parts = Vec::new();
        while !ends.iter().any(|end| self.at(end)) && !self.at(&TokenKind::Eof) {
            parts.push(self.advance().kind.display());
        }
        join_tokens(&parts)
    }

    fn collect_until_item_end(&mut self) -> String {
        self.collect_until_any(&[TokenKind::Newline, TokenKind::Semicolon, TokenKind::Eof])
    }

    fn consume_item_end(&mut self) {
        while self.consume(TokenKind::Newline) || self.consume(TokenKind::Semicolon) {}
    }

    fn consume_field_end(&mut self) {
        while self.consume(TokenKind::Newline)
            || self.consume(TokenKind::Comma)
            || self.consume(TokenKind::Semicolon)
        {}
    }

    fn skip_separators(&mut self) {
        while self.at(&TokenKind::Newline) || self.at(&TokenKind::Semicolon) {
            self.advance();
        }
    }

    fn expect_ident(&mut self) -> Result<String, ParseError> {
        match self.peek_kind() {
            TokenKind::Ident(value) => {
                let value = value.clone();
                self.advance();
                Ok(value)
            }
            TokenKind::Keyword(Keyword::Self_) => {
                self.advance();
                Ok("self".into())
            }
            other => self.err(format!("expected identifier, found {}", other.display())),
        }
    }

    fn expect_keyword(&mut self, keyword: Keyword) -> Result<(), ParseError> {
        if self.consume_keyword(keyword) {
            Ok(())
        } else {
            self.err(format!("expected keyword {keyword:?}"))
        }
    }

    fn expect(&mut self, kind: TokenKind) -> Result<(), ParseError> {
        if self.consume(kind.clone()) {
            Ok(())
        } else {
            self.err(format!("expected {}", kind.display()))
        }
    }

    fn consume_keyword(&mut self, keyword: Keyword) -> bool {
        if self.at(&TokenKind::Keyword(keyword)) {
            self.advance();
            true
        } else {
            false
        }
    }

    fn consume(&mut self, kind: TokenKind) -> bool {
        if self.at(&kind) {
            self.advance();
            true
        } else {
            false
        }
    }

    fn at(&self, kind: &TokenKind) -> bool {
        self.peek_kind() == kind
    }

    fn peek_kind(&self) -> &TokenKind {
        &self.tokens[self.index].kind
    }

    fn advance(&mut self) -> Token {
        let token = self.tokens[self.index].clone();
        self.index += 1;
        token
    }

    fn text_from_tokens(&self, start: usize, end: usize) -> String {
        let parts: Vec<_> = self.tokens[start..end]
            .iter()
            .map(|token| token.kind.display())
            .collect();
        join_tokens(&parts)
    }

    fn err<T>(&self, message: impl Into<String>) -> Result<T, ParseError> {
        Err(ParseError {
            message: message.into(),
            span: self.tokens[self.index].span,
        })
    }
}

fn looks_like_generic_call(parser: &Parser) -> bool {
    let mut depth = 0usize;
    let mut idx = parser.index;
    while let Some(token) = parser.tokens.get(idx) {
        match token.kind {
            TokenKind::Lt => depth += 1,
            TokenKind::Gt => {
                depth = depth.saturating_sub(1);
                if depth == 0 {
                    return matches!(
                        parser.tokens.get(idx + 1).map(|token| &token.kind),
                        Some(TokenKind::LParen)
                    );
                }
            }
            TokenKind::Newline | TokenKind::Semicolon | TokenKind::Eof => return false,
            _ => {}
        }
        idx += 1;
    }
    false
}

fn unzip_type_pair(pair: Option<(String, TypeExpr)>) -> (Option<String>, Option<TypeExpr>) {
    match pair {
        Some((text, expr)) => (Some(text), Some(expr)),
        None => (None, None),
    }
}

fn join_tokens(parts: &[String]) -> String {
    let mut output = String::new();
    for part in parts {
        if part == "\\n" {
            output.push('\n');
            continue;
        }
        if output.is_empty()
            || matches!(part.as_str(), ")" | "]" | "}" | "," | "." | "?" | ":")
            || output.ends_with(['(', '[', '{', '.', ':', '\n'])
        {
            output.push_str(part);
        } else {
            output.push(' ');
            output.push_str(part);
        }
    }
    output
}
