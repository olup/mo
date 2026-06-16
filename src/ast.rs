use crate::span::Span;

#[derive(Debug, Clone, PartialEq)]
pub struct Program {
    pub items: Vec<Item>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Item {
    Module(Path),
    Use(String),
    Import(String),
    Struct(StructItem),
    Enum(EnumItem),
    Interface(InterfaceItem),
    Impl(ImplItem),
    Function(FunctionItem),
    Directive(DirectiveItem),
    Extern(ExternBlock),
    TypeAlias(TypeAliasItem),
    Const(ConstItem),
    Static(StaticItem),
    Test(TestItem),
}

#[derive(Debug, Clone, PartialEq)]
pub struct Path {
    pub segments: Vec<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct DirectiveItem {
    pub name: String,
    pub args: String,
    pub items: Vec<Item>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct StructItem {
    pub public: bool,
    pub name: String,
    pub generics: Option<String>,
    pub conforms: Vec<String>,
    pub directives: Vec<InlineDirective>,
    pub fields: Vec<Field>,
    pub methods: Vec<FunctionItem>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Field {
    pub public: bool,
    pub name: String,
    pub ty: String,
    pub ty_expr: TypeExpr,
}

#[derive(Debug, Clone, PartialEq)]
pub struct EnumItem {
    pub public: bool,
    pub name: String,
    pub generics: Option<String>,
    pub variants: Vec<EnumVariant>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct EnumVariant {
    pub name: String,
    pub payload: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct InterfaceItem {
    pub public: bool,
    pub name: String,
    pub generics: Option<String>,
    pub extends: Vec<String>,
    pub methods: Vec<FunctionSignature>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ImplItem {
    pub interface: Option<String>,
    pub target: String,
    pub methods: Vec<FunctionItem>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct FunctionItem {
    pub span: Span,
    pub source_location: Option<String>,
    pub public: bool,
    pub is_async: bool,
    pub is_unsafe: bool,
    pub name: String,
    pub generics: Option<String>,
    pub params: Vec<Param>,
    pub return_type: Option<String>,
    pub return_type_expr: Option<TypeExpr>,
    pub body: Option<Block>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct FunctionSignature {
    pub is_async: bool,
    pub is_unsafe: bool,
    pub name: String,
    pub params: Vec<Param>,
    pub return_type: Option<String>,
    pub return_type_expr: Option<TypeExpr>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Param {
    pub mutable: bool,
    pub name: String,
    pub ty: Option<String>,
    pub ty_expr: Option<TypeExpr>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ExternBlock {
    pub abi: Option<String>,
    pub functions: Vec<FunctionSignature>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct TypeAliasItem {
    pub public: bool,
    pub name: String,
    pub generics: Option<String>,
    pub value: String,
    pub value_expr: TypeExpr,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ConstItem {
    pub public: bool,
    pub name: String,
    pub ty: Option<String>,
    pub ty_expr: Option<TypeExpr>,
    pub value: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct StaticItem {
    pub public: bool,
    pub name: String,
    pub ty: Option<String>,
    pub ty_expr: Option<TypeExpr>,
    pub value: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct TestItem {
    pub name: String,
    pub body: Block,
}

#[derive(Debug, Clone, PartialEq)]
pub struct InlineDirective {
    pub name: String,
    pub args: String,
}

#[derive(Debug, Clone, PartialEq)]
pub enum TypeExpr {
    Missing,
    Path(Vec<String>),
    Generic {
        base: Box<TypeExpr>,
        args: Vec<TypeExpr>,
    },
    Tuple(Vec<TypeExpr>),
    Fn {
        is_async: bool,
        params: Vec<TypeExpr>,
        return_type: Box<TypeExpr>,
    },
    Ref {
        mutable: bool,
        inner: Box<TypeExpr>,
    },
    RawPtr {
        mutable: bool,
        inner: Box<TypeExpr>,
    },
    Impl(Box<TypeExpr>),
    Mut(Box<TypeExpr>),
}

#[derive(Debug, Clone, PartialEq)]
pub struct Block {
    pub statements: Vec<Stmt>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Stmt {
    pub kind: StmtKind,
    pub text: String,
    pub data: StmtData,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StmtKind {
    Let,
    Return,
    Break,
    Continue,
    If,
    Match,
    While,
    For,
    Loop,
    Unsafe,
    Expr,
}

#[derive(Debug, Clone, PartialEq)]
pub enum StmtData {
    Let(LetStmt),
    Return(Option<Expr>),
    Break(Option<Expr>),
    Continue,
    If(ControlStmt),
    Match(MatchExpr),
    While(ControlStmt),
    For(ForStmt),
    Loop(Block),
    Unsafe(Block),
    Expr(Expr),
    Raw,
}

#[derive(Debug, Clone, PartialEq)]
pub struct LetStmt {
    pub mutable: bool,
    pub name: String,
    pub ty: Option<String>,
    pub ty_expr: Option<TypeExpr>,
    pub value: Option<Expr>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ControlStmt {
    pub condition: Option<Expr>,
    pub body: Block,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ForStmt {
    pub pattern: String,
    pub iterator: Expr,
    pub body: Block,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Expr {
    Missing,
    Ident(String),
    Literal(Literal),
    Path(Vec<String>),
    Unary(UnaryExpr),
    Mut(Box<Expr>),
    Binary(BinaryExpr),
    Index(IndexExpr),
    Call(CallExpr),
    Member(MemberExpr),
    Await(Box<Expr>),
    Try(Box<Expr>),
    Struct(StructExpr),
    Object(ObjectExpr),
    Closure(ClosureExpr),
    Match(MatchExpr),
    If(IfExpr),
    Block(Block),
    Raw(String),
}

#[derive(Debug, Clone, PartialEq)]
pub enum Literal {
    Int(String),
    Float(String),
    String(String),
    Char(String),
    Bool(bool),
    Unit,
}

#[derive(Debug, Clone, PartialEq)]
pub struct UnaryExpr {
    pub op: UnaryOp,
    pub expr: Box<Expr>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UnaryOp {
    Ref,
    MutRef,
    Deref,
    Neg,
    Not,
}

#[derive(Debug, Clone, PartialEq)]
pub struct BinaryExpr {
    pub op: BinaryOp,
    pub left: Box<Expr>,
    pub right: Box<Expr>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct IndexExpr {
    pub target: Box<Expr>,
    pub index: Box<Expr>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BinaryOp {
    Assign,
    AddAssign,
    SubAssign,
    MulAssign,
    DivAssign,
    RemAssign,
    BitAndAssign,
    BitOrAssign,
    ShlAssign,
    ShrAssign,
    Eq,
    NotEq,
    Lt,
    Le,
    Gt,
    Ge,
    BoolAnd,
    BoolOr,
    BitAnd,
    BitOr,
    BitXor,
    Shl,
    Shr,
    Add,
    Sub,
    Mul,
    Div,
    Rem,
}

impl BinaryOp {
    pub fn is_assignment(self) -> bool {
        matches!(
            self,
            BinaryOp::Assign
                | BinaryOp::AddAssign
                | BinaryOp::SubAssign
                | BinaryOp::MulAssign
                | BinaryOp::DivAssign
                | BinaryOp::RemAssign
                | BinaryOp::BitAndAssign
                | BinaryOp::BitOrAssign
                | BinaryOp::ShlAssign
                | BinaryOp::ShrAssign
        )
    }

    pub fn compound_value_op(self) -> Option<BinaryOp> {
        Some(match self {
            BinaryOp::AddAssign => BinaryOp::Add,
            BinaryOp::SubAssign => BinaryOp::Sub,
            BinaryOp::MulAssign => BinaryOp::Mul,
            BinaryOp::DivAssign => BinaryOp::Div,
            BinaryOp::RemAssign => BinaryOp::Rem,
            BinaryOp::BitAndAssign => BinaryOp::BitAnd,
            BinaryOp::BitOrAssign => BinaryOp::BitOr,
            BinaryOp::ShlAssign => BinaryOp::Shl,
            BinaryOp::ShrAssign => BinaryOp::Shr,
            _ => return None,
        })
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct CallExpr {
    pub callee: Box<Expr>,
    pub type_args: Option<String>,
    pub args: Vec<Expr>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct MemberExpr {
    pub target: Box<Expr>,
    pub member: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct StructExpr {
    pub name: String,
    pub fields: Vec<StructFieldExpr>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct StructFieldExpr {
    pub name: String,
    pub value: Option<Expr>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ObjectExpr {
    pub fields: Vec<ObjectFieldExpr>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ObjectFieldExpr {
    pub key: String,
    pub value: Expr,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ClosureExpr {
    pub is_async: bool,
    pub is_move: bool,
    pub params: Vec<Param>,
    pub return_type: Option<String>,
    pub return_type_expr: Option<TypeExpr>,
    pub body: Block,
}

#[derive(Debug, Clone, PartialEq)]
pub struct MatchExpr {
    pub value: Box<Expr>,
    pub arms: Vec<MatchArm>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct MatchArm {
    pub pattern: String,
    pub body: Expr,
}

#[derive(Debug, Clone, PartialEq)]
pub struct IfExpr {
    pub condition: Box<Expr>,
    pub then_branch: Block,
    pub else_branch: Option<Block>,
}
