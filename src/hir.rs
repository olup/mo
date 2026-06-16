use crate::ast::*;
use crate::semantics::{target_predicate_matches, Diagnostic, Target};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct HirItemId(pub usize);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct HirModuleId(pub usize);

#[derive(Debug, Clone, PartialEq)]
pub struct HirProgram {
    pub modules: Vec<HirModule>,
    pub items: Vec<HirItem>,
    pub structs: Vec<HirStruct>,
    pub enums: Vec<HirEnum>,
    pub interfaces: Vec<HirInterface>,
    pub impls: Vec<HirImpl>,
    pub extern_functions: Vec<HirExternFunction>,
    pub functions: Vec<HirFunction>,
    pub tests: Vec<HirTest>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HirModule {
    pub id: HirModuleId,
    pub path: Vec<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct HirItem {
    pub id: HirItemId,
    pub module: HirModuleId,
    pub name: String,
    pub kind: HirItemKind,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HirItemKind {
    Struct,
    Enum,
    Interface,
    Impl,
    Function,
    ExternFunction,
    TypeAlias,
    Const,
    Static,
    Test,
}

#[derive(Debug, Clone, PartialEq)]
pub struct HirFunction {
    pub id: HirItemId,
    pub module: HirModuleId,
    pub name: String,
    pub generics: Option<String>,
    pub is_async: bool,
    pub is_unsafe: bool,
    pub params: Vec<Param>,
    pub return_type: Option<TypeExpr>,
    pub body: Block,
}

#[derive(Debug, Clone, PartialEq)]
pub struct HirStruct {
    pub id: HirItemId,
    pub module: HirModuleId,
    pub name: String,
    pub generics: Option<String>,
    pub conforms: Vec<String>,
    pub fields: Vec<Field>,
    pub methods: Vec<FunctionItem>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct HirEnum {
    pub id: HirItemId,
    pub module: HirModuleId,
    pub name: String,
    pub generics: Option<String>,
    pub variants: Vec<EnumVariant>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct HirInterface {
    pub id: HirItemId,
    pub module: HirModuleId,
    pub name: String,
    pub methods: Vec<FunctionSignature>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct HirImpl {
    pub id: HirItemId,
    pub module: HirModuleId,
    pub interface: Option<String>,
    pub target: String,
    pub methods: Vec<FunctionItem>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct HirExternFunction {
    pub id: HirItemId,
    pub module: HirModuleId,
    pub abi: Option<String>,
    pub name: String,
    pub params: Vec<Param>,
    pub return_type: Option<TypeExpr>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct HirTest {
    pub id: HirItemId,
    pub module: HirModuleId,
    pub name: String,
    pub body: Block,
}

pub fn lower_program(program: &Program, target: &Target) -> Result<HirProgram, Vec<Diagnostic>> {
    let mut lowerer = Lowerer {
        target,
        next_id: 0,
        modules: vec![HirModule {
            id: HirModuleId(0),
            path: Vec::new(),
        }],
        current_module: HirModuleId(0),
        items: Vec::new(),
        structs: Vec::new(),
        enums: Vec::new(),
        interfaces: Vec::new(),
        impls: Vec::new(),
        extern_functions: Vec::new(),
        functions: Vec::new(),
        tests: Vec::new(),
    };

    lowerer.lower_items(&program.items);

    Ok(HirProgram {
        modules: lowerer.modules,
        items: lowerer.items,
        structs: lowerer.structs,
        enums: lowerer.enums,
        interfaces: lowerer.interfaces,
        impls: lowerer.impls,
        extern_functions: lowerer.extern_functions,
        functions: lowerer.functions,
        tests: lowerer.tests,
    })
}

struct Lowerer<'a> {
    target: &'a Target,
    next_id: usize,
    modules: Vec<HirModule>,
    current_module: HirModuleId,
    items: Vec<HirItem>,
    structs: Vec<HirStruct>,
    enums: Vec<HirEnum>,
    interfaces: Vec<HirInterface>,
    impls: Vec<HirImpl>,
    extern_functions: Vec<HirExternFunction>,
    functions: Vec<HirFunction>,
    tests: Vec<HirTest>,
}

impl<'a> Lowerer<'a> {
    fn lower_items(&mut self, items: &[Item]) {
        for item in items {
            self.lower_item(item);
        }
    }

    fn lower_item(&mut self, item: &Item) {
        match item {
            Item::Module(path) => {
                self.current_module = self.module_id(path.segments.clone());
            }
            Item::Directive(directive) => {
                if directive.name == "target" {
                    if target_predicate_matches(&directive.args, self.target) {
                        self.lower_items(&directive.items);
                    }
                } else {
                    self.lower_items(&directive.items);
                }
            }
            Item::Struct(item) => {
                let id = self.push_item(item.name.clone(), HirItemKind::Struct);
                self.structs.push(HirStruct {
                    id,
                    module: self.current_module,
                    name: item.name.clone(),
                    generics: item.generics.clone(),
                    conforms: item.conforms.clone(),
                    fields: item.fields.clone(),
                    methods: item.methods.clone(),
                });
                if !item.methods.is_empty() {
                    self.impls.push(HirImpl {
                        id,
                        module: self.current_module,
                        interface: None,
                        target: item.name.clone(),
                        methods: item.methods.clone(),
                    });
                }
                for interface in &item.conforms {
                    self.impls.push(HirImpl {
                        id,
                        module: self.current_module,
                        interface: Some(interface.clone()),
                        target: item.name.clone(),
                        methods: item.methods.clone(),
                    });
                }
                for method in &item.methods {
                    if let Some(body) = &method.body {
                        self.functions.push(HirFunction {
                            id,
                            module: self.current_module,
                            name: method.name.clone(),
                            generics: method.generics.clone(),
                            is_async: method.is_async,
                            is_unsafe: method.is_unsafe,
                            params: method.params.clone(),
                            return_type: method.return_type_expr.clone(),
                            body: body.clone(),
                        });
                    }
                }
            }
            Item::Enum(item) => {
                let id = self.push_item(item.name.clone(), HirItemKind::Enum);
                self.enums.push(HirEnum {
                    id,
                    module: self.current_module,
                    name: item.name.clone(),
                    generics: item.generics.clone(),
                    variants: item.variants.clone(),
                });
            }
            Item::Interface(item) => {
                let id = self.push_item(item.name.clone(), HirItemKind::Interface);
                self.interfaces.push(HirInterface {
                    id,
                    module: self.current_module,
                    name: item.name.clone(),
                    methods: item.methods.clone(),
                });
            }
            Item::Impl(item) => {
                let id = self.push_item(impl_name(item), HirItemKind::Impl);
                self.impls.push(HirImpl {
                    id,
                    module: self.current_module,
                    interface: item.interface.clone(),
                    target: item.target.clone(),
                    methods: item.methods.clone(),
                });
                for method in &item.methods {
                    if let Some(body) = &method.body {
                        self.functions.push(HirFunction {
                            id,
                            module: self.current_module,
                            name: method.name.clone(),
                            generics: method.generics.clone(),
                            is_async: method.is_async,
                            is_unsafe: method.is_unsafe,
                            params: method.params.clone(),
                            return_type: method.return_type_expr.clone(),
                            body: body.clone(),
                        });
                    }
                }
            }
            Item::Function(item) => {
                let id = self.push_item(item.name.clone(), HirItemKind::Function);
                if let Some(body) = &item.body {
                    self.functions.push(HirFunction {
                        id,
                        module: self.current_module,
                        name: item.name.clone(),
                        generics: item.generics.clone(),
                        is_async: item.is_async,
                        is_unsafe: item.is_unsafe,
                        params: item.params.clone(),
                        return_type: item.return_type_expr.clone(),
                        body: body.clone(),
                    });
                }
            }
            Item::Extern(block) => {
                for function in &block.functions {
                    let id = self.push_item(function.name.clone(), HirItemKind::ExternFunction);
                    self.extern_functions.push(HirExternFunction {
                        id,
                        module: self.current_module,
                        abi: block.abi.clone(),
                        name: function.name.clone(),
                        params: function.params.clone(),
                        return_type: function.return_type_expr.clone(),
                    });
                }
            }
            Item::TypeAlias(item) => {
                self.push_item(item.name.clone(), HirItemKind::TypeAlias);
            }
            Item::Const(item) => {
                self.push_item(item.name.clone(), HirItemKind::Const);
            }
            Item::Static(item) => {
                self.push_item(item.name.clone(), HirItemKind::Static);
            }
            Item::Test(item) => {
                let id = self.push_item(item.name.clone(), HirItemKind::Test);
                self.tests.push(HirTest {
                    id,
                    module: self.current_module,
                    name: item.name.clone(),
                    body: item.body.clone(),
                });
            }
            Item::Use(_) | Item::Import(_) => {}
        }
    }

    fn push_item(&mut self, name: String, kind: HirItemKind) -> HirItemId {
        let id = HirItemId(self.next_id);
        self.next_id += 1;
        self.items.push(HirItem {
            id,
            module: self.current_module,
            name,
            kind,
        });
        id
    }

    fn module_id(&mut self, path: Vec<String>) -> HirModuleId {
        if let Some(module) = self.modules.iter().find(|module| module.path == path) {
            return module.id;
        }
        let id = HirModuleId(self.modules.len());
        self.modules.push(HirModule { id, path });
        id
    }
}

fn impl_name(item: &ImplItem) -> String {
    match &item.interface {
        Some(interface) => format!("impl {interface} for {}", item.target),
        None => format!("impl {}", item.target),
    }
}
