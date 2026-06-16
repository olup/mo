use std::collections::HashMap;
use std::fs;
use std::path::Path;
use std::str::FromStr;

use cranelift_codegen::entity::EntityRef;
use cranelift_codegen::ir::condcodes::{FloatCC, IntCC};
use cranelift_codegen::ir::{
    types, Block as ClifBlock, InstBuilder, MemFlags, StackSlotData, StackSlotKind, TrapCode,
};
use cranelift_codegen::isa;
use cranelift_codegen::settings::{self, Configurable};
use cranelift_frontend::{FunctionBuilder, FunctionBuilderContext, Variable};
use cranelift_module::{DataDescription, DataId, FuncId, Linkage, Module};
use cranelift_object::{ObjectBuilder, ObjectModule};
use object::write::MachOBuildVersion;
use target_lexicon::Triple;

use crate::ir::{
    IrBoolExpr, IrCompareOp, IrEnumExpr, IrEnumMatchArm, IrFloatBinaryOp, IrFloatExpr, IrFunction,
    IrFunctionExpr, IrIntBinaryOp, IrIntExpr, IrIntTy, IrProgram, IrStringExpr, IrStructExpr,
    IrTerminator, IrValueExpr, IrValueTy,
};
use crate::resource::is_unique_resource_name;
use crate::semantics::{Diagnostic, ObjectFormat, Target};

pub fn emit_object(
    program: &IrProgram,
    output: &Path,
    target: &Target,
) -> Result<(), Vec<Diagnostic>> {
    let triple = Triple::from_str(target.triple()).map_err(|err| diagnostic(err.to_string()))?;
    let isa_builder = isa::lookup(triple).map_err(|err| diagnostic(err.to_string()))?;
    let mut flags_builder = settings::builder();
    flags_builder
        .set("is_pic", "true")
        .map_err(|err| diagnostic(err.to_string()))?;
    let isa = isa_builder
        .finish(settings::Flags::new(flags_builder))
        .map_err(|err| diagnostic(err.to_string()))?;
    let builder = ObjectBuilder::new(
        isa,
        "mo".to_string(),
        cranelift_module::default_libcall_names(),
    )
    .map_err(|err| diagnostic(err.to_string()))?;
    let mut module = ObjectModule::new(builder);
    let mut function_ids = HashMap::new();

    for function in &program.functions {
        let function_id = declare_function(&mut module, program, function)?;
        function_ids.insert(function.name.clone(), function_id);
    }
    for function in &program.extern_functions {
        let function_id = declare_extern_function(&mut module, function)?;
        function_ids.insert(function.name.clone(), function_id);
    }
    let puts_id = declare_puts(&mut module)?;
    let strlen_id = declare_strlen(&mut module)?;
    let write_id = declare_write(&mut module)?;
    let putchar_id = declare_putchar(&mut module)?;
    let malloc_id = declare_malloc(&mut module)?;
    let free_id = declare_free(&mut module)?;
    let malloc_size_id = declare_malloc_size(&mut module, target)?;
    let memcpy_id = declare_memcpy(&mut module)?;
    let ioctl_id = declare_ioctl(&mut module)?;
    let pthread_create_id = declare_pthread_create(&mut module)?;
    let pthread_join_id = declare_pthread_join(&mut module)?;
    let thread_trampoline_id = declare_thread_trampoline(&mut module)?;
    let thread_env_trampoline_id = declare_thread_env_trampoline(&mut module)?;
    let string_data = define_string_data(&mut module, program)?;
    let memory_counters = define_memory_counter_data(&mut module)?;
    let struct_layouts = struct_layouts(program);
    let enum_layouts = enum_layouts(program);
    let backend_context = BackendContext {
        function_ids,
        function_sigs: backend_function_sigs(program),
        puts_id,
        strlen_id,
        write_id,
        putchar_id,
        malloc_id,
        free_id,
        malloc_size_id,
        memcpy_id,
        ioctl_id,
        pthread_create_id,
        pthread_join_id,
        thread_trampoline_id,
        thread_env_trampoline_id,
        string_data,
        memory_counters,
        struct_layouts,
        enum_layouts,
        platform_abi: PlatformAbi::for_target(target),
        drop_impls: program.drop_impls.clone(),
    };

    for function in &program.functions {
        define_function(
            &mut module,
            function,
            backend_context.function_ids[&function.name],
            &backend_context,
        )?;
    }
    define_thread_trampoline(&mut module, thread_trampoline_id)?;
    define_thread_env_trampoline(
        &mut module,
        thread_env_trampoline_id,
        free_id,
        malloc_size_id,
        &backend_context.memory_counters,
    )?;

    let mut product = module.finish();
    if target.object_format() == ObjectFormat::MachO {
        let mut build_version = MachOBuildVersion::default();
        build_version.platform = object::macho::PLATFORM_MACOS;
        build_version.minos = encoded_macho_version(11, 0, 0);
        build_version.sdk = encoded_macho_version(14, 0, 0);
        product.object.set_macho_build_version(build_version);
    }
    let bytes = product
        .object
        .write()
        .map_err(|err| diagnostic(err.to_string()))?;
    fs::write(output, bytes).map_err(|err| diagnostic(err.to_string()))?;
    Ok(())
}

fn encoded_macho_version(major: u32, minor: u32, patch: u32) -> u32 {
    (major << 16) | (minor << 8) | patch
}

fn declare_function(
    module: &mut ObjectModule,
    program: &IrProgram,
    function: &IrFunction,
) -> Result<FuncId, Vec<Diagnostic>> {
    if function.name == "main" && !function.params.is_empty() {
        return Err(diagnostic(
            "Cranelift MVP only supports zero-argument main".to_string(),
        ));
    }

    let mut sig = module.make_signature();
    for ty in &function.param_types {
        sig.params
            .push(cranelift_codegen::ir::AbiParam::new(clif_abi_type(ty)));
    }
    sig.returns
        .push(cranelift_codegen::ir::AbiParam::new(clif_abi_type(
            &function.return_type,
        )));
    let symbol = program.function_symbol(function);
    module
        .declare_function(
            &symbol,
            if function.name == "main" {
                Linkage::Export
            } else {
                Linkage::Local
            },
            &sig,
        )
        .map_err(|err| diagnostic(err.to_string()))
}

fn declare_extern_function(
    module: &mut ObjectModule,
    function: &crate::ir::IrExternFunction,
) -> Result<FuncId, Vec<Diagnostic>> {
    let mut sig = module.make_signature();
    for ty in &function.param_types {
        sig.params
            .push(cranelift_codegen::ir::AbiParam::new(clif_abi_type(ty)));
    }
    sig.returns
        .push(cranelift_codegen::ir::AbiParam::new(clif_abi_type(
            &function.return_type,
        )));
    module
        .declare_function(&function.name, Linkage::Import, &sig)
        .map_err(|err| diagnostic(err.to_string()))
}

struct BackendContext {
    function_ids: HashMap<String, FuncId>,
    function_sigs: HashMap<String, BackendFunctionSig>,
    puts_id: FuncId,
    strlen_id: FuncId,
    write_id: FuncId,
    putchar_id: FuncId,
    malloc_id: FuncId,
    free_id: FuncId,
    malloc_size_id: FuncId,
    memcpy_id: FuncId,
    ioctl_id: FuncId,
    pthread_create_id: FuncId,
    pthread_join_id: FuncId,
    thread_trampoline_id: FuncId,
    thread_env_trampoline_id: FuncId,
    string_data: HashMap<String, DataId>,
    memory_counters: MemoryCounters,
    struct_layouts: HashMap<String, StructLayout>,
    enum_layouts: HashMap<String, EnumLayout>,
    platform_abi: PlatformAbi,
    drop_impls: HashMap<String, String>,
}

#[derive(Debug, Clone)]
struct PlatformAbi {
    fionbio_ioctl_request: i64,
}

impl PlatformAbi {
    fn for_target(target: &Target) -> Self {
        if target.has("linux") {
            return Self {
                fionbio_ioctl_request: 0x5421,
            };
        }
        Self {
            fionbio_ioctl_request: 2_147_772_030,
        }
    }
}

#[derive(Debug, Clone)]
struct MemoryCounters {
    alloc_count: DataId,
    free_count: DataId,
    live_bytes: DataId,
    high_water_bytes: DataId,
}

struct BackendFunctionSig {
    params: Vec<IrValueTy>,
    ret: IrValueTy,
}

#[derive(Debug, Clone)]
struct StructLayout {
    fields: HashMap<String, StructFieldLayout>,
    size: i64,
}

#[derive(Debug, Clone)]
struct StructFieldLayout {
    offset: i32,
    ty: IrValueTy,
}

#[derive(Debug, Clone)]
struct EnumLayout {
    variants: HashMap<String, EnumVariantLayout>,
    size: i64,
}

#[derive(Debug, Clone)]
struct EnumVariantLayout {
    tag: i64,
    payload_offsets: Vec<i32>,
    payload_tys: Vec<IrValueTy>,
}

fn declare_puts(module: &mut ObjectModule) -> Result<FuncId, Vec<Diagnostic>> {
    let mut sig = module.make_signature();
    sig.params
        .push(cranelift_codegen::ir::AbiParam::new(types::I64));
    sig.returns
        .push(cranelift_codegen::ir::AbiParam::new(types::I32));
    module
        .declare_function("puts", Linkage::Import, &sig)
        .map_err(|err| diagnostic(err.to_string()))
}

fn declare_strlen(module: &mut ObjectModule) -> Result<FuncId, Vec<Diagnostic>> {
    let mut sig = module.make_signature();
    sig.params
        .push(cranelift_codegen::ir::AbiParam::new(types::I64));
    sig.returns
        .push(cranelift_codegen::ir::AbiParam::new(types::I64));
    module
        .declare_function("strlen", Linkage::Import, &sig)
        .map_err(|err| diagnostic(err.to_string()))
}

fn declare_write(module: &mut ObjectModule) -> Result<FuncId, Vec<Diagnostic>> {
    let mut sig = module.make_signature();
    sig.params
        .push(cranelift_codegen::ir::AbiParam::new(types::I64));
    sig.params
        .push(cranelift_codegen::ir::AbiParam::new(types::I64));
    sig.params
        .push(cranelift_codegen::ir::AbiParam::new(types::I64));
    sig.returns
        .push(cranelift_codegen::ir::AbiParam::new(types::I64));
    module
        .declare_function("write", Linkage::Import, &sig)
        .map_err(|err| diagnostic(err.to_string()))
}

fn declare_putchar(module: &mut ObjectModule) -> Result<FuncId, Vec<Diagnostic>> {
    let mut sig = module.make_signature();
    sig.params
        .push(cranelift_codegen::ir::AbiParam::new(types::I32));
    sig.returns
        .push(cranelift_codegen::ir::AbiParam::new(types::I32));
    module
        .declare_function("putchar", Linkage::Import, &sig)
        .map_err(|err| diagnostic(err.to_string()))
}

fn declare_malloc(module: &mut ObjectModule) -> Result<FuncId, Vec<Diagnostic>> {
    let mut sig = module.make_signature();
    sig.params
        .push(cranelift_codegen::ir::AbiParam::new(types::I64));
    sig.returns
        .push(cranelift_codegen::ir::AbiParam::new(types::I64));
    module
        .declare_function("malloc", Linkage::Import, &sig)
        .map_err(|err| diagnostic(err.to_string()))
}

fn declare_free(module: &mut ObjectModule) -> Result<FuncId, Vec<Diagnostic>> {
    let mut sig = module.make_signature();
    sig.params
        .push(cranelift_codegen::ir::AbiParam::new(types::I64));
    module
        .declare_function("free", Linkage::Import, &sig)
        .map_err(|err| diagnostic(err.to_string()))
}

fn declare_malloc_size(
    module: &mut ObjectModule,
    target: &Target,
) -> Result<FuncId, Vec<Diagnostic>> {
    let mut sig = module.make_signature();
    sig.params
        .push(cranelift_codegen::ir::AbiParam::new(types::I64));
    sig.returns
        .push(cranelift_codegen::ir::AbiParam::new(types::I64));
    let symbol = if target.has("linux") {
        "malloc_usable_size"
    } else {
        "malloc_size"
    };
    module
        .declare_function(symbol, Linkage::Import, &sig)
        .map_err(|err| diagnostic(err.to_string()))
}

fn declare_memcpy(module: &mut ObjectModule) -> Result<FuncId, Vec<Diagnostic>> {
    let mut sig = module.make_signature();
    sig.params
        .push(cranelift_codegen::ir::AbiParam::new(types::I64));
    sig.params
        .push(cranelift_codegen::ir::AbiParam::new(types::I64));
    sig.params
        .push(cranelift_codegen::ir::AbiParam::new(types::I64));
    sig.returns
        .push(cranelift_codegen::ir::AbiParam::new(types::I64));
    module
        .declare_function("memcpy", Linkage::Import, &sig)
        .map_err(|err| diagnostic(err.to_string()))
}

fn declare_ioctl(module: &mut ObjectModule) -> Result<FuncId, Vec<Diagnostic>> {
    let mut sig = module.make_signature();
    sig.params
        .push(cranelift_codegen::ir::AbiParam::new(types::I32));
    sig.params
        .push(cranelift_codegen::ir::AbiParam::new(types::I64));
    sig.params
        .push(cranelift_codegen::ir::AbiParam::new(types::I64));
    sig.returns
        .push(cranelift_codegen::ir::AbiParam::new(types::I32));
    module
        .declare_function("ioctl", Linkage::Import, &sig)
        .map_err(|err| diagnostic(err.to_string()))
}

fn declare_pthread_create(module: &mut ObjectModule) -> Result<FuncId, Vec<Diagnostic>> {
    let mut sig = module.make_signature();
    for _ in 0..4 {
        sig.params
            .push(cranelift_codegen::ir::AbiParam::new(types::I64));
    }
    sig.returns
        .push(cranelift_codegen::ir::AbiParam::new(types::I32));
    module
        .declare_function("pthread_create", Linkage::Import, &sig)
        .map_err(|err| diagnostic(err.to_string()))
}

fn declare_pthread_join(module: &mut ObjectModule) -> Result<FuncId, Vec<Diagnostic>> {
    let mut sig = module.make_signature();
    sig.params
        .push(cranelift_codegen::ir::AbiParam::new(types::I64));
    sig.params
        .push(cranelift_codegen::ir::AbiParam::new(types::I64));
    sig.returns
        .push(cranelift_codegen::ir::AbiParam::new(types::I32));
    module
        .declare_function("pthread_join", Linkage::Import, &sig)
        .map_err(|err| diagnostic(err.to_string()))
}

fn declare_thread_trampoline(module: &mut ObjectModule) -> Result<FuncId, Vec<Diagnostic>> {
    let mut sig = module.make_signature();
    sig.params
        .push(cranelift_codegen::ir::AbiParam::new(types::I64));
    sig.returns
        .push(cranelift_codegen::ir::AbiParam::new(types::I64));
    module
        .declare_function("__mo_thread_trampoline", Linkage::Local, &sig)
        .map_err(|err| diagnostic(err.to_string()))
}

fn declare_thread_env_trampoline(module: &mut ObjectModule) -> Result<FuncId, Vec<Diagnostic>> {
    let mut sig = module.make_signature();
    sig.params
        .push(cranelift_codegen::ir::AbiParam::new(types::I64));
    sig.returns
        .push(cranelift_codegen::ir::AbiParam::new(types::I64));
    module
        .declare_function("__mo_thread_env_trampoline", Linkage::Local, &sig)
        .map_err(|err| diagnostic(err.to_string()))
}

fn backend_function_sigs(program: &IrProgram) -> HashMap<String, BackendFunctionSig> {
    let mut sigs = HashMap::new();
    for function in &program.functions {
        sigs.insert(
            function.name.clone(),
            BackendFunctionSig {
                params: function.param_types.clone(),
                ret: function.return_type.clone(),
            },
        );
    }
    for function in &program.extern_functions {
        sigs.insert(
            function.name.clone(),
            BackendFunctionSig {
                params: function.param_types.clone(),
                ret: function.return_type.clone(),
            },
        );
    }
    sigs
}

fn struct_layouts(program: &IrProgram) -> HashMap<String, StructLayout> {
    program
        .structs
        .iter()
        .map(|item| {
            let mut offset = 0i32;
            let fields = item
                .fields
                .iter()
                .map(|field| {
                    offset = align_to(offset, value_align(&field.ty));
                    let field_offset = offset;
                    offset += value_size(&field.ty);
                    (
                        field.name.clone(),
                        StructFieldLayout {
                            offset: field_offset,
                            ty: field.ty.clone(),
                        },
                    )
                })
                .collect::<HashMap<_, _>>();
            (
                item.name.clone(),
                StructLayout {
                    fields,
                    size: align_to(offset, 8) as i64,
                },
            )
        })
        .collect()
}

fn enum_layouts(program: &IrProgram) -> HashMap<String, EnumLayout> {
    program
        .enums
        .iter()
        .map(|item| {
            let payload_size = item
                .variants
                .iter()
                .map(|variant| enum_payload_offsets(&variant.payload_tys).1)
                .max()
                .unwrap_or(0);
            (
                item.name.clone(),
                EnumLayout {
                    variants: item
                        .variants
                        .iter()
                        .map(|variant| {
                            (variant.name.clone(), {
                                let (payload_offsets, _) =
                                    enum_payload_offsets(&variant.payload_tys);
                                EnumVariantLayout {
                                    tag: variant.tag,
                                    payload_offsets,
                                    payload_tys: variant.payload_tys.clone(),
                                }
                            })
                        })
                        .collect(),
                    size: align_to(8 + payload_size, 8) as i64,
                },
            )
        })
        .collect()
}

fn define_thread_trampoline(
    module: &mut ObjectModule,
    function_id: FuncId,
) -> Result<(), Vec<Diagnostic>> {
    let mut sig = module.make_signature();
    sig.params
        .push(cranelift_codegen::ir::AbiParam::new(types::I64));
    sig.returns
        .push(cranelift_codegen::ir::AbiParam::new(types::I64));
    let mut ctx = module.make_context();
    ctx.func.signature = sig;
    let mut builder_context = FunctionBuilderContext::new();
    let mut builder = FunctionBuilder::new(&mut ctx.func, &mut builder_context);
    let entry = builder.create_block();
    builder.append_block_params_for_function_params(entry);
    builder.switch_to_block(entry);
    let task = builder.block_params(entry)[0];

    let mut task_sig = module.make_signature();
    task_sig
        .returns
        .push(cranelift_codegen::ir::AbiParam::new(types::I64));
    let sig_ref = builder.func.import_signature(task_sig);
    builder.ins().call_indirect(sig_ref, task, &[]);
    let zero = builder.ins().iconst(types::I64, 0);
    builder.ins().return_(&[zero]);
    builder.seal_all_blocks();
    builder.finalize();
    module
        .define_function(function_id, &mut ctx)
        .map_err(|err| diagnostic(format!("{err:?}")))?;
    module.clear_context(&mut ctx);
    Ok(())
}

fn define_thread_env_trampoline(
    module: &mut ObjectModule,
    function_id: FuncId,
    free_id: FuncId,
    malloc_size_id: FuncId,
    counters: &MemoryCounters,
) -> Result<(), Vec<Diagnostic>> {
    let mut sig = module.make_signature();
    sig.params
        .push(cranelift_codegen::ir::AbiParam::new(types::I64));
    sig.returns
        .push(cranelift_codegen::ir::AbiParam::new(types::I64));
    let mut ctx = module.make_context();
    ctx.func.signature = sig;
    let mut builder_context = FunctionBuilderContext::new();
    let mut builder = FunctionBuilder::new(&mut ctx.func, &mut builder_context);
    let entry = builder.create_block();
    builder.append_block_params_for_function_params(entry);
    builder.switch_to_block(entry);
    let env = builder.block_params(entry)[0];
    let task = builder.ins().load(types::I64, MemFlags::new(), env, 0);

    let mut task_sig = module.make_signature();
    task_sig
        .params
        .push(cranelift_codegen::ir::AbiParam::new(types::I64));
    task_sig
        .returns
        .push(cranelift_codegen::ir::AbiParam::new(types::I64));
    let sig_ref = builder.func.import_signature(task_sig);
    builder.ins().call_indirect(sig_ref, task, &[env]);
    let malloc_size = module.declare_func_in_func(malloc_size_id, builder.func);
    let size_call = builder.ins().call(malloc_size, &[env]);
    let usable_size = builder.inst_results(size_call)[0];
    increment_counter(&mut builder, module, &counters.free_count, 1);
    sub_live_bytes(&mut builder, module, counters, usable_size);
    let free = module.declare_func_in_func(free_id, builder.func);
    builder.ins().call(free, &[env]);
    let zero = builder.ins().iconst(types::I64, 0);
    builder.ins().return_(&[zero]);
    builder.seal_all_blocks();
    builder.finalize();
    module
        .define_function(function_id, &mut ctx)
        .map_err(|err| diagnostic(format!("{err:?}")))?;
    module.clear_context(&mut ctx);
    Ok(())
}

fn enum_payload_offsets(payload_tys: &[IrValueTy]) -> (Vec<i32>, i32) {
    let mut offsets = Vec::new();
    let mut offset = 8i32;
    for ty in payload_tys {
        offset = align_to(offset, value_align(ty));
        offsets.push(offset);
        offset += value_size(ty);
    }
    (offsets, offset - 8)
}

fn align_to(value: i32, align: i32) -> i32 {
    if align <= 1 {
        return value;
    }
    ((value + align - 1) / align) * align
}

fn value_size(ty: &IrValueTy) -> i32 {
    match ty {
        IrValueTy::Bool => 1,
        IrValueTy::Int(kind) => int_size(*kind),
        IrValueTy::Float64 => 8,
        IrValueTy::Unknown
        | IrValueTy::Unit
        | IrValueTy::String
        | IrValueTy::BorrowedString
        | IrValueTy::OwnedString
        | IrValueTy::Boxed(_)
        | IrValueTy::Vec(_)
        | IrValueTy::Struct(_)
        | IrValueTy::Enum(_)
        | IrValueTy::Function { .. } => 8,
    }
}

fn value_align(ty: &IrValueTy) -> i32 {
    value_size(ty).min(8).max(1)
}

fn int_size(kind: IrIntTy) -> i32 {
    match kind {
        IrIntTy::Int8 | IrIntTy::UInt8 => 1,
        IrIntTy::Int16 | IrIntTy::UInt16 => 2,
        IrIntTy::Int32 | IrIntTy::UInt32 => 4,
        IrIntTy::Int | IrIntTy::Int64 | IrIntTy::UInt | IrIntTy::UInt64 => 8,
    }
}

fn clif_abi_type(ty: &IrValueTy) -> cranelift_codegen::ir::Type {
    match ty {
        IrValueTy::Bool => types::I8,
        IrValueTy::Int(kind) => clif_int_type(*kind),
        IrValueTy::Float64 => types::F64,
        _ => types::I64,
    }
}

fn clif_local_type(ty: &IrValueTy) -> cranelift_codegen::ir::Type {
    match ty {
        IrValueTy::Float64 => types::F64,
        _ => types::I64,
    }
}

fn clif_int_type(kind: IrIntTy) -> cranelift_codegen::ir::Type {
    match kind {
        IrIntTy::Int8 | IrIntTy::UInt8 => types::I8,
        IrIntTy::Int16 | IrIntTy::UInt16 => types::I16,
        IrIntTy::Int32 | IrIntTy::UInt32 => types::I32,
        IrIntTy::Int | IrIntTy::Int64 | IrIntTy::UInt | IrIntTy::UInt64 => types::I64,
    }
}

fn is_signed_int(kind: IrIntTy) -> bool {
    matches!(
        kind,
        IrIntTy::Int | IrIntTy::Int8 | IrIntTy::Int16 | IrIntTy::Int32 | IrIntTy::Int64
    )
}

fn reduce_for_abi(
    builder: &mut FunctionBuilder,
    value: cranelift_codegen::ir::Value,
    ty: &IrValueTy,
) -> cranelift_codegen::ir::Value {
    let target = clif_abi_type(ty);
    if target == types::I64 || target == types::F64 {
        value
    } else {
        builder.ins().ireduce(target, value)
    }
}

fn extend_to_i64(
    builder: &mut FunctionBuilder,
    value: cranelift_codegen::ir::Value,
    ty: &IrValueTy,
) -> cranelift_codegen::ir::Value {
    let source = clif_abi_type(ty);
    if source == types::I64 || source == types::F64 {
        return value;
    }
    match ty {
        IrValueTy::Int(kind) if is_signed_int(*kind) => builder.ins().sextend(types::I64, value),
        _ => builder.ins().uextend(types::I64, value),
    }
}

fn load_value(
    builder: &mut FunctionBuilder,
    base: cranelift_codegen::ir::Value,
    offset: i32,
    ty: &IrValueTy,
) -> cranelift_codegen::ir::Value {
    match ty {
        IrValueTy::Bool => builder
            .ins()
            .uload8(types::I64, MemFlags::new(), base, offset),
        IrValueTy::Int(kind) => match (int_size(*kind), is_signed_int(*kind)) {
            (1, true) => builder
                .ins()
                .sload8(types::I64, MemFlags::new(), base, offset),
            (1, false) => builder
                .ins()
                .uload8(types::I64, MemFlags::new(), base, offset),
            (2, true) => builder
                .ins()
                .sload16(types::I64, MemFlags::new(), base, offset),
            (2, false) => builder
                .ins()
                .uload16(types::I64, MemFlags::new(), base, offset),
            (4, true) => {
                let value = builder
                    .ins()
                    .load(types::I32, MemFlags::new(), base, offset);
                builder.ins().sextend(types::I64, value)
            }
            (4, false) => {
                let value = builder
                    .ins()
                    .load(types::I32, MemFlags::new(), base, offset);
                builder.ins().uextend(types::I64, value)
            }
            _ => builder
                .ins()
                .load(types::I64, MemFlags::new(), base, offset),
        },
        IrValueTy::Float64 => builder
            .ins()
            .load(types::F64, MemFlags::new(), base, offset),
        _ => builder
            .ins()
            .load(types::I64, MemFlags::new(), base, offset),
    }
}

fn store_value(
    builder: &mut FunctionBuilder,
    value: cranelift_codegen::ir::Value,
    base: cranelift_codegen::ir::Value,
    offset: i32,
    ty: &IrValueTy,
) {
    let value = match ty {
        IrValueTy::Bool => builder.ins().ireduce(types::I8, value),
        IrValueTy::Int(kind) => {
            let target = clif_int_type(*kind);
            if target == types::I64 {
                value
            } else {
                builder.ins().ireduce(target, value)
            }
        }
        IrValueTy::Float64 => value,
        _ => value,
    };
    builder.ins().store(MemFlags::new(), value, base, offset);
}

fn define_memory_counter_data(
    module: &mut ObjectModule,
) -> Result<MemoryCounters, Vec<Diagnostic>> {
    Ok(MemoryCounters {
        alloc_count: define_zero_i64_data(module, "__mo_mem_alloc_count")?,
        free_count: define_zero_i64_data(module, "__mo_mem_free_count")?,
        live_bytes: define_zero_i64_data(module, "__mo_mem_live_bytes")?,
        high_water_bytes: define_zero_i64_data(module, "__mo_mem_high_water_bytes")?,
    })
}

fn define_zero_i64_data(module: &mut ObjectModule, name: &str) -> Result<DataId, Vec<Diagnostic>> {
    let data_id = module
        .declare_data(name, Linkage::Local, true, false)
        .map_err(|err| diagnostic(err.to_string()))?;
    let mut data = DataDescription::new();
    data.define(vec![0; 8].into_boxed_slice());
    module
        .define_data(data_id, &data)
        .map_err(|err| diagnostic(err.to_string()))?;
    Ok(data_id)
}

fn define_string_data(
    module: &mut ObjectModule,
    program: &IrProgram,
) -> Result<HashMap<String, DataId>, Vec<Diagnostic>> {
    let mut strings = HashMap::new();
    let mut next_id = 0usize;
    let mut values = Vec::new();
    for instruction in program
        .functions
        .iter()
        .flat_map(|function| function.blocks.iter())
        .flat_map(|block| block.instructions.iter())
    {
        collect_instruction_strings(instruction, &mut values);
    }
    for terminator in program
        .functions
        .iter()
        .flat_map(|function| function.blocks.iter())
        .map(|block| &block.terminator)
    {
        collect_terminator_strings(terminator, &mut values);
    }
    for value in values {
        if strings.contains_key(&value) {
            continue;
        }
        let name = format!("__mo_str_{next_id}");
        next_id += 1;
        let data_id = module
            .declare_data(&name, Linkage::Local, false, false)
            .map_err(|err| diagnostic(err.to_string()))?;
        let mut bytes = value.as_bytes().to_vec();
        bytes.push(0);
        let mut data = DataDescription::new();
        data.define(bytes.into_boxed_slice());
        module
            .define_data(data_id, &data)
            .map_err(|err| diagnostic(err.to_string()))?;
        strings.insert(value, data_id);
    }
    Ok(strings)
}

fn collect_terminator_strings(terminator: &IrTerminator, values: &mut Vec<String>) {
    match terminator {
        IrTerminator::Branch {
            condition: Some(condition),
            ..
        } => collect_bool_expr(condition, values),
        IrTerminator::Switch { value, .. } => collect_enum_expr(value, values),
        IrTerminator::Return | IrTerminator::Jump { .. } | IrTerminator::Unreachable => {}
        IrTerminator::Branch {
            condition: None, ..
        } => {}
    }
}

fn collect_instruction_strings(instruction: &crate::ir::IrInstruction, values: &mut Vec<String>) {
    match instruction {
        crate::ir::IrInstruction::ReturnUnit => {}
        crate::ir::IrInstruction::AssignString { value, .. }
        | crate::ir::IrInstruction::ReturnString { value }
        | crate::ir::IrInstruction::PrintStringExpr { value } => collect_string_expr(value, values),
        crate::ir::IrInstruction::AssignInt { value, .. }
        | crate::ir::IrInstruction::ReturnInt { value }
        | crate::ir::IrInstruction::PrintInt { value } => collect_int_expr(value, values),
        crate::ir::IrInstruction::AssignFloat { value, .. }
        | crate::ir::IrInstruction::ReturnFloat { value } => collect_float_expr(value, values),
        crate::ir::IrInstruction::AssignBool { value, .. }
        | crate::ir::IrInstruction::ReturnBool { value } => collect_bool_expr(value, values),
        crate::ir::IrInstruction::ReturnFunction { value } => collect_function_expr(value, values),
        crate::ir::IrInstruction::Assert { condition, message } => {
            collect_bool_expr(condition, values);
            values.push(message.clone());
        }
        crate::ir::IrInstruction::RawWrite { fd, text } => {
            collect_int_expr(fd, values);
            collect_string_expr(text, values);
        }
        crate::ir::IrInstruction::RawStore8 { ptr, offset, value } => {
            collect_int_expr(ptr, values);
            collect_int_expr(offset, values);
            collect_int_expr(value, values);
        }
        crate::ir::IrInstruction::RawStringStore8 {
            value,
            offset,
            byte,
        } => {
            collect_string_expr(value, values);
            collect_int_expr(offset, values);
            collect_int_expr(byte, values);
        }
        crate::ir::IrInstruction::RawFree { ptr } => collect_int_expr(ptr, values),
        crate::ir::IrInstruction::Call { args, .. } => {
            for arg in args {
                collect_value_expr(arg, values);
            }
        }
        crate::ir::IrInstruction::IndirectCall { callee, args } => {
            collect_function_expr(callee, values);
            for arg in args {
                collect_value_expr(arg, values);
            }
        }
        crate::ir::IrInstruction::AssignStruct { value, .. }
        | crate::ir::IrInstruction::ReturnStruct { value } => collect_struct_expr(value, values),
        crate::ir::IrInstruction::AssignEnum { value, .. }
        | crate::ir::IrInstruction::ReturnEnum { value } => collect_enum_expr(value, values),
        crate::ir::IrInstruction::BindEnumPayload { value, .. } => collect_enum_expr(value, values),
        crate::ir::IrInstruction::AssignFunction { .. } => {}
        crate::ir::IrInstruction::AssignField { value, .. } => collect_value_expr(value, values),
        crate::ir::IrInstruction::AssignEnumMatch { arms, .. }
        | crate::ir::IrInstruction::ReturnEnumMatch { arms, .. } => {
            for arm in arms {
                collect_value_expr(&arm.body, values);
            }
        }
        crate::ir::IrInstruction::PrintString { value } => values.push(value.clone()),
        _ => {}
    }
}

fn collect_int_expr(expr: &IrIntExpr, values: &mut Vec<String>) {
    match expr {
        IrIntExpr::StringLen(expr) | IrIntExpr::StringPtr(expr) => {
            collect_string_expr(expr, values)
        }
        IrIntExpr::EnumTag(expr) => collect_enum_expr(expr, values),
        IrIntExpr::FunctionPtr(expr) => collect_function_expr(expr, values),
        IrIntExpr::FloatToInt(expr) => collect_float_expr(expr, values),
        IrIntExpr::RawWrite { fd, text } => {
            collect_int_expr(fd, values);
            collect_string_expr(text, values);
        }
        IrIntExpr::RawAlloc { size } => collect_int_expr(size, values),
        IrIntExpr::RawLoad8 { ptr, offset } | IrIntExpr::RawLoad64 { ptr, offset } => {
            collect_int_expr(ptr, values);
            collect_int_expr(offset, values);
        }
        IrIntExpr::RawSetNonblocking { fd } => collect_int_expr(fd, values),
        IrIntExpr::RawThreadSpawn { captures, .. } => collect_value_exprs(captures, values),
        IrIntExpr::RawThreadJoin { handle } => collect_int_expr(handle, values),
        IrIntExpr::Call { args, .. } => {
            for arg in args {
                collect_value_expr(arg, values);
            }
        }
        IrIntExpr::IndirectCall { args, .. } => {
            for arg in args {
                collect_value_expr(arg, values);
            }
        }
        IrIntExpr::Binary { left, right, .. } => {
            collect_int_expr(left, values);
            collect_int_expr(right, values);
        }
        IrIntExpr::Const(_)
        | IrIntExpr::Local(_)
        | IrIntExpr::Field { .. }
        | IrIntExpr::EnvLoad { .. }
        | IrIntExpr::RawMemAllocCount
        | IrIntExpr::RawMemFreeCount
        | IrIntExpr::RawMemLiveBytes
        | IrIntExpr::RawMemHighWaterBytes => {}
    }
}

fn collect_string_expr(expr: &IrStringExpr, values: &mut Vec<String>) {
    match expr {
        IrStringExpr::Literal(value) => values.push(value.clone()),
        IrStringExpr::RawAlloc { size } => collect_int_expr(size, values),
        IrStringExpr::Concat { left, right } => {
            collect_string_expr(left, values);
            collect_string_expr(right, values);
        }
        IrStringExpr::IntToString(expr) | IrStringExpr::FromPtr(expr) => {
            collect_int_expr(expr, values)
        }
        IrStringExpr::Call { args, .. } => collect_value_exprs(args, values),
        IrStringExpr::IndirectCall { callee, args } => {
            collect_function_expr(callee, values);
            collect_value_exprs(args, values);
        }
        IrStringExpr::Local(_) | IrStringExpr::Field { .. } | IrStringExpr::EnvLoad { .. } => {}
    }
}

fn collect_bool_expr(expr: &IrBoolExpr, values: &mut Vec<String>) {
    match expr {
        IrBoolExpr::Call { args, .. } => collect_value_exprs(args, values),
        IrBoolExpr::Not(expr) => collect_bool_expr(expr, values),
        IrBoolExpr::And(left, right) | IrBoolExpr::Or(left, right) => {
            collect_bool_expr(left, values);
            collect_bool_expr(right, values);
        }
        IrBoolExpr::Compare { left, right, .. } => {
            collect_int_expr(left, values);
            collect_int_expr(right, values);
        }
        IrBoolExpr::FloatCompare { left, right, .. } => {
            collect_float_expr(left, values);
            collect_float_expr(right, values);
        }
        IrBoolExpr::BoolCompare { left, right, .. } => {
            collect_bool_expr(left, values);
            collect_bool_expr(right, values);
        }
        IrBoolExpr::StringCompare { left, right, .. } => {
            collect_string_expr(left, values);
            collect_string_expr(right, values);
        }
        IrBoolExpr::Const(_)
        | IrBoolExpr::Local(_)
        | IrBoolExpr::Field { .. }
        | IrBoolExpr::EnvLoad { .. } => {}
    }
}

fn collect_float_expr(expr: &IrFloatExpr, values: &mut Vec<String>) {
    match expr {
        IrFloatExpr::Call { args, .. } | IrFloatExpr::IndirectCall { args, .. } => {
            collect_value_exprs(args, values)
        }
        IrFloatExpr::Binary { left, right, .. } => {
            collect_float_expr(left, values);
            collect_float_expr(right, values);
        }
        IrFloatExpr::IntToFloat(expr) => collect_int_expr(expr, values),
        IrFloatExpr::Const(_)
        | IrFloatExpr::Local(_)
        | IrFloatExpr::Field { .. }
        | IrFloatExpr::EnvLoad { .. } => {}
    }
}

fn collect_struct_expr(expr: &IrStructExpr, values: &mut Vec<String>) {
    match expr {
        IrStructExpr::Construct { fields, .. } => {
            for field in fields {
                collect_value_expr(&field.value, values);
            }
        }
        IrStructExpr::IndirectCall { callee, args } => {
            collect_function_expr(callee, values);
            collect_value_exprs(args, values);
        }
        IrStructExpr::Call { args, .. } => collect_value_exprs(args, values),
        IrStructExpr::Local(_) | IrStructExpr::Field { .. } | IrStructExpr::EnvLoad { .. } => {}
    }
}

fn collect_function_expr(expr: &IrFunctionExpr, values: &mut Vec<String>) {
    match expr {
        IrFunctionExpr::FromPtr(ptr) => collect_int_expr(ptr, values),
        IrFunctionExpr::Call { args, .. } => collect_value_exprs(args, values),
        IrFunctionExpr::Local(_) | IrFunctionExpr::Named(_) | IrFunctionExpr::Field { .. } => {}
    }
}

fn collect_value_exprs(exprs: &[IrValueExpr], values: &mut Vec<String>) {
    for expr in exprs {
        collect_value_expr(expr, values);
    }
}

fn collect_value_expr(expr: &IrValueExpr, values: &mut Vec<String>) {
    match expr {
        IrValueExpr::String(expr) => collect_string_expr(expr, values),
        IrValueExpr::Struct(expr) => collect_struct_expr(expr, values),
        IrValueExpr::Enum(expr) => collect_enum_expr(expr, values),
        IrValueExpr::Bool(expr) => collect_bool_expr(expr, values),
        IrValueExpr::Int(expr) => collect_int_expr(expr, values),
        IrValueExpr::Float(expr) => collect_float_expr(expr, values),
        IrValueExpr::Function(expr) => collect_function_expr(expr, values),
    }
}

fn collect_enum_expr(expr: &IrEnumExpr, values: &mut Vec<String>) {
    match expr {
        IrEnumExpr::Call { args, .. } => {
            for arg in args {
                collect_value_expr(arg, values);
            }
        }
        IrEnumExpr::IndirectCall { callee, args } => {
            collect_function_expr(callee, values);
            for arg in args {
                collect_value_expr(arg, values);
            }
        }
        IrEnumExpr::Construct { payloads, .. } => {
            for payload in payloads {
                collect_value_expr(payload, values);
            }
        }
        IrEnumExpr::Local(_) | IrEnumExpr::EnvLoad { .. } => {}
    }
}

fn define_function(
    module: &mut ObjectModule,
    function: &IrFunction,
    function_id: FuncId,
    backend_context: &BackendContext,
) -> Result<(), Vec<Diagnostic>> {
    let mut sig = module.make_signature();
    for ty in &function.param_types {
        sig.params
            .push(cranelift_codegen::ir::AbiParam::new(clif_abi_type(ty)));
    }
    sig.returns
        .push(cranelift_codegen::ir::AbiParam::new(clif_abi_type(
            &function.return_type,
        )));
    let mut ctx = module.make_context();
    ctx.func.signature = sig;
    let mut builder_context = FunctionBuilderContext::new();
    let mut builder = FunctionBuilder::new(&mut ctx.func, &mut builder_context);
    let mut block_map = HashMap::new();
    for block in &function.blocks {
        block_map.insert(block.id, builder.create_block());
    }
    let variable_types = function_variable_types(function, backend_context);
    let entry_block = *block_map
        .get(&0)
        .ok_or_else(|| diagnostic(format!("function `{}` has no entry block", function.name)))?;
    builder.append_block_params_for_function_params(entry_block);
    builder.switch_to_block(entry_block);

    let mut variables = HashMap::new();
    let mut next_variable = 0usize;
    let params = builder.block_params(entry_block).to_vec();
    for ((param, ty), value) in function
        .params
        .iter()
        .zip(function.param_types.iter())
        .zip(params)
    {
        let variable = Variable::new(next_variable);
        next_variable += 1;
        builder.declare_var(variable, clif_local_type(ty));
        let value = extend_to_i64(&mut builder, value, ty);
        builder.def_var(variable, value);
        variables.insert(param.clone(), variable);
    }
    for local in function_locals(function) {
        if variables.contains_key(&local) {
            continue;
        }
        let variable = Variable::new(next_variable);
        next_variable += 1;
        let ty = variable_types.get(&local).unwrap_or(&IrValueTy::Unknown);
        builder.declare_var(variable, clif_local_type(ty));
        variables.insert(local, variable);
    }

    for block in &function.blocks {
        let clif_block = block_id(&block_map, block.id)?;
        if builder.current_block() != Some(clif_block) {
            builder.switch_to_block(clif_block);
        }
        let mut terminated = false;
        for instruction in &block.instructions {
            match instruction {
                crate::ir::IrInstruction::ReturnUnit => {
                    let return_value = builder.ins().iconst(types::I64, 0);
                    builder.ins().return_(&[return_value]);
                    terminated = true;
                    break;
                }
                crate::ir::IrInstruction::AssignInt { local, value } => {
                    let value = codegen_int_expr(
                        &mut builder,
                        module,
                        backend_context,
                        &variable_types,
                        &mut variables,
                        &mut next_variable,
                        value,
                    )?;
                    let variable = variable_for(
                        &mut builder,
                        &mut variables,
                        &mut next_variable,
                        local.clone(),
                    );
                    builder.def_var(variable, value);
                }
                crate::ir::IrInstruction::AssignFloat { local, value } => {
                    let value = codegen_float_expr(
                        &mut builder,
                        module,
                        backend_context,
                        &variable_types,
                        &mut variables,
                        &mut next_variable,
                        value,
                    )?;
                    let variable = variable_for(
                        &mut builder,
                        &mut variables,
                        &mut next_variable,
                        local.clone(),
                    );
                    builder.def_var(variable, value);
                }
                crate::ir::IrInstruction::AssignBool { local, value } => {
                    let value = codegen_bool_as_int(
                        &mut builder,
                        module,
                        backend_context,
                        &variable_types,
                        &mut variables,
                        &mut next_variable,
                        value,
                    )?;
                    let variable = variable_for(
                        &mut builder,
                        &mut variables,
                        &mut next_variable,
                        local.clone(),
                    );
                    builder.def_var(variable, value);
                }
                crate::ir::IrInstruction::AssignString { local, value } => {
                    let value = codegen_string_expr(
                        &mut builder,
                        module,
                        backend_context,
                        &variable_types,
                        &mut variables,
                        &mut next_variable,
                        value,
                    )?;
                    let variable = variable_for(
                        &mut builder,
                        &mut variables,
                        &mut next_variable,
                        local.clone(),
                    );
                    builder.def_var(variable, value);
                }
                crate::ir::IrInstruction::AssignStruct { local, value } => {
                    let value = codegen_struct_expr(
                        &mut builder,
                        module,
                        backend_context,
                        &variable_types,
                        &mut variables,
                        &mut next_variable,
                        value,
                    )?;
                    let variable = variable_for(
                        &mut builder,
                        &mut variables,
                        &mut next_variable,
                        local.clone(),
                    );
                    builder.def_var(variable, value);
                }
                crate::ir::IrInstruction::AssignEnum { local, value } => {
                    let value = codegen_enum_expr(
                        &mut builder,
                        module,
                        backend_context,
                        &variable_types,
                        &mut variables,
                        &mut next_variable,
                        value,
                    )?;
                    let variable = variable_for(
                        &mut builder,
                        &mut variables,
                        &mut next_variable,
                        local.clone(),
                    );
                    builder.def_var(variable, value);
                }
                crate::ir::IrInstruction::AssignFunction { local, value } => {
                    let value = codegen_function_expr(
                        &mut builder,
                        module,
                        backend_context,
                        &variable_types,
                        &mut variables,
                        &mut next_variable,
                        value,
                    )?;
                    let variable = variable_for(
                        &mut builder,
                        &mut variables,
                        &mut next_variable,
                        local.clone(),
                    );
                    builder.def_var(variable, value);
                }
                crate::ir::IrInstruction::AssignField { base, field, value } => {
                    let base_value = use_local(&mut builder, &variables, base, "struct")?;
                    let field = field_layout(backend_context, &variable_types, base, field)?;
                    let value = codegen_value_expr(
                        &mut builder,
                        module,
                        backend_context,
                        &variable_types,
                        &mut variables,
                        &mut next_variable,
                        value,
                    )?;
                    store_value(&mut builder, value, base_value, field.offset, &field.ty);
                }
                crate::ir::IrInstruction::AssignEnumMatch {
                    local,
                    ty,
                    value,
                    arms,
                } => {
                    let value = codegen_enum_match_value(
                        &mut builder,
                        module,
                        backend_context,
                        &variable_types,
                        &mut variables,
                        &mut next_variable,
                        value,
                        arms,
                        ty,
                        false,
                    )?;
                    let variable = variable_for(
                        &mut builder,
                        &mut variables,
                        &mut next_variable,
                        local.clone(),
                    );
                    builder.def_var(variable, value);
                }
                crate::ir::IrInstruction::BindEnumPayload {
                    local,
                    value,
                    payload_index,
                    payload_tys,
                    payload_ty,
                } => {
                    let enum_value = codegen_enum_expr(
                        &mut builder,
                        module,
                        backend_context,
                        &variable_types,
                        &mut variables,
                        &mut next_variable,
                        value,
                    )?;
                    let payload = codegen_enum_payload(
                        &mut builder,
                        enum_value,
                        value,
                        *payload_index,
                        payload_tys,
                        payload_ty,
                    )?;
                    let variable = variable_for(
                        &mut builder,
                        &mut variables,
                        &mut next_variable,
                        local.clone(),
                    );
                    builder.def_var(variable, payload);
                }
                crate::ir::IrInstruction::ReturnInt { value } => {
                    let value = codegen_int_expr(
                        &mut builder,
                        module,
                        backend_context,
                        &variable_types,
                        &mut variables,
                        &mut next_variable,
                        value,
                    )?;
                    let value = reduce_for_abi(&mut builder, value, &function.return_type);
                    builder.ins().return_(&[value]);
                    terminated = true;
                    break;
                }
                crate::ir::IrInstruction::ReturnFloat { value } => {
                    let value = codegen_float_expr(
                        &mut builder,
                        module,
                        backend_context,
                        &variable_types,
                        &mut variables,
                        &mut next_variable,
                        value,
                    )?;
                    builder.ins().return_(&[value]);
                    terminated = true;
                    break;
                }
                crate::ir::IrInstruction::ReturnBool { value } => {
                    let value = codegen_bool_as_int(
                        &mut builder,
                        module,
                        backend_context,
                        &variable_types,
                        &mut variables,
                        &mut next_variable,
                        value,
                    )?;
                    let value = reduce_for_abi(&mut builder, value, &function.return_type);
                    builder.ins().return_(&[value]);
                    terminated = true;
                    break;
                }
                crate::ir::IrInstruction::ReturnString { value } => {
                    let value = codegen_string_expr(
                        &mut builder,
                        module,
                        backend_context,
                        &variable_types,
                        &mut variables,
                        &mut next_variable,
                        value,
                    )?;
                    let value = reduce_for_abi(&mut builder, value, &function.return_type);
                    builder.ins().return_(&[value]);
                    terminated = true;
                    break;
                }
                crate::ir::IrInstruction::ReturnFunction { value } => {
                    let value = codegen_function_expr(
                        &mut builder,
                        module,
                        backend_context,
                        &variable_types,
                        &mut variables,
                        &mut next_variable,
                        value,
                    )?;
                    let value = reduce_for_abi(&mut builder, value, &function.return_type);
                    builder.ins().return_(&[value]);
                    terminated = true;
                    break;
                }
                crate::ir::IrInstruction::ReturnStruct { value } => {
                    let value = codegen_struct_expr(
                        &mut builder,
                        module,
                        backend_context,
                        &variable_types,
                        &mut variables,
                        &mut next_variable,
                        value,
                    )?;
                    let value = reduce_for_abi(&mut builder, value, &function.return_type);
                    builder.ins().return_(&[value]);
                    terminated = true;
                    break;
                }
                crate::ir::IrInstruction::ReturnEnum { value } => {
                    let value = codegen_enum_expr(
                        &mut builder,
                        module,
                        backend_context,
                        &variable_types,
                        &mut variables,
                        &mut next_variable,
                        value,
                    )?;
                    let value = reduce_for_abi(&mut builder, value, &function.return_type);
                    builder.ins().return_(&[value]);
                    terminated = true;
                    break;
                }
                crate::ir::IrInstruction::ReturnEnumMatch {
                    ty,
                    value,
                    arms,
                    free_value_storage,
                } => {
                    let value = codegen_enum_match_value(
                        &mut builder,
                        module,
                        backend_context,
                        &variable_types,
                        &mut variables,
                        &mut next_variable,
                        value,
                        arms,
                        ty,
                        *free_value_storage,
                    )?;
                    let value = reduce_for_abi(&mut builder, value, &function.return_type);
                    builder.ins().return_(&[value]);
                    terminated = true;
                    break;
                }
                crate::ir::IrInstruction::PrintString { value } => {
                    codegen_print_string(&mut builder, module, backend_context, value)?;
                }
                crate::ir::IrInstruction::PrintStringExpr { value } => {
                    codegen_print_string_expr(
                        &mut builder,
                        module,
                        backend_context,
                        &variable_types,
                        &mut variables,
                        &mut next_variable,
                        value,
                    )?;
                }
                crate::ir::IrInstruction::PrintInt { value } => {
                    codegen_print_int(
                        &mut builder,
                        module,
                        backend_context,
                        &variable_types,
                        &mut variables,
                        &mut next_variable,
                        value,
                    )?;
                }
                crate::ir::IrInstruction::Assert { condition, message } => {
                    codegen_assert(
                        &mut builder,
                        module,
                        backend_context,
                        &variable_types,
                        &mut variables,
                        &mut next_variable,
                        condition,
                        message,
                    )?;
                }
                crate::ir::IrInstruction::RawWrite { fd, text } => {
                    codegen_raw_write(
                        &mut builder,
                        module,
                        backend_context,
                        &variable_types,
                        &mut variables,
                        &mut next_variable,
                        fd,
                        text,
                    )?;
                }
                crate::ir::IrInstruction::RawStore8 { ptr, offset, value } => {
                    codegen_raw_store8(
                        &mut builder,
                        module,
                        backend_context,
                        &variable_types,
                        &mut variables,
                        &mut next_variable,
                        ptr,
                        offset,
                        value,
                    )?;
                }
                crate::ir::IrInstruction::RawStore64 { ptr, offset, value } => {
                    codegen_raw_store64(
                        &mut builder,
                        module,
                        backend_context,
                        &variable_types,
                        &mut variables,
                        &mut next_variable,
                        ptr,
                        offset,
                        value,
                    )?;
                }
                crate::ir::IrInstruction::RawStringStore8 {
                    value,
                    offset,
                    byte,
                } => {
                    codegen_raw_string_store8(
                        &mut builder,
                        module,
                        backend_context,
                        &variable_types,
                        &mut variables,
                        &mut next_variable,
                        value,
                        offset,
                        byte,
                    )?;
                }
                crate::ir::IrInstruction::RawFree { ptr } => {
                    codegen_raw_free(
                        &mut builder,
                        module,
                        backend_context,
                        &variable_types,
                        &mut variables,
                        &mut next_variable,
                        ptr,
                    )?;
                }
                crate::ir::IrInstruction::DropBoxStorage { local } => {
                    codegen_drop_box_storage(
                        &mut builder,
                        module,
                        backend_context,
                        &variables,
                        local,
                    )?;
                }
                crate::ir::IrInstruction::Call { callee, args } => {
                    let Some(function_id) = backend_context.function_ids.get(callee) else {
                        return Err(diagnostic(format!("unknown function `{callee}`")));
                    };
                    let local_callee = module.declare_func_in_func(*function_id, builder.func);
                    let args = codegen_call_args(
                        &mut builder,
                        module,
                        backend_context,
                        &variable_types,
                        &mut variables,
                        &mut next_variable,
                        callee,
                        args,
                    )?;
                    builder.ins().call(local_callee, &args);
                }
                crate::ir::IrInstruction::IndirectCall { callee, args } => {
                    let callee = codegen_function_expr(
                        &mut builder,
                        module,
                        backend_context,
                        &variable_types,
                        &mut variables,
                        &mut next_variable,
                        callee,
                    )?;
                    let mut sig = module.make_signature();
                    for _ in args {
                        sig.params
                            .push(cranelift_codegen::ir::AbiParam::new(types::I64));
                    }
                    sig.returns
                        .push(cranelift_codegen::ir::AbiParam::new(types::I64));
                    let sig_ref = builder.func.import_signature(sig);
                    let args = codegen_value_args(
                        &mut builder,
                        module,
                        backend_context,
                        &variable_types,
                        &mut variables,
                        &mut next_variable,
                        args,
                    )?;
                    builder.ins().call_indirect(sig_ref, callee, &args);
                }
                crate::ir::IrInstruction::Drop { local, ty } => {
                    codegen_drop(&mut builder, module, backend_context, &variables, local, ty)?;
                }
                _ => {}
            }
        }
        if !terminated {
            codegen_terminator(
                &mut builder,
                module,
                backend_context,
                &variable_types,
                &mut variables,
                &mut next_variable,
                &block_map,
                &block.terminator,
            )?;
        }
    }

    builder.seal_all_blocks();
    builder.finalize();

    module
        .define_function(function_id, &mut ctx)
        .map_err(|err| diagnostic(format!("{err:?}")))?;
    module.clear_context(&mut ctx);
    Ok(())
}

fn function_locals(function: &IrFunction) -> Vec<String> {
    let mut locals = Vec::new();
    for block in &function.blocks {
        for instruction in &block.instructions {
            let local = match instruction {
                crate::ir::IrInstruction::Let { local }
                | crate::ir::IrInstruction::AssignInt { local, .. }
                | crate::ir::IrInstruction::AssignFloat { local, .. }
                | crate::ir::IrInstruction::AssignBool { local, .. }
                | crate::ir::IrInstruction::AssignString { local, .. }
                | crate::ir::IrInstruction::AssignStruct { local, .. }
                | crate::ir::IrInstruction::AssignEnum { local, .. }
                | crate::ir::IrInstruction::AssignFunction { local, .. }
                | crate::ir::IrInstruction::AssignEnumMatch { local, .. }
                | crate::ir::IrInstruction::BindEnumPayload { local, .. } => local,
                crate::ir::IrInstruction::AssignField { .. } => continue,
                _ => continue,
            };
            if !locals.contains(local) {
                locals.push(local.clone());
            }
        }
    }
    locals
}

fn function_variable_types(
    function: &IrFunction,
    backend_context: &BackendContext,
) -> HashMap<String, IrValueTy> {
    let mut types = HashMap::new();
    for (param, ty) in function.params.iter().zip(function.param_types.iter()) {
        types.insert(param.clone(), ty.clone());
    }
    for block in &function.blocks {
        for instruction in &block.instructions {
            match instruction {
                crate::ir::IrInstruction::AssignInt { local, .. } => {
                    types.insert(local.clone(), IrValueTy::Int(IrIntTy::Int));
                }
                crate::ir::IrInstruction::AssignFloat { local, .. } => {
                    types.insert(local.clone(), IrValueTy::Float64);
                }
                crate::ir::IrInstruction::AssignBool { local, .. } => {
                    types.insert(local.clone(), IrValueTy::Bool);
                }
                crate::ir::IrInstruction::AssignString { local, .. } => {
                    types.insert(local.clone(), IrValueTy::OwnedString);
                }
                crate::ir::IrInstruction::AssignStruct { local, value } => {
                    if let Some(ty) = infer_struct_expr_ty(value, backend_context, &types) {
                        types.insert(local.clone(), ty);
                    }
                }
                crate::ir::IrInstruction::AssignEnum { local, value } => {
                    if let Some(ty) = infer_enum_expr_ty(value, backend_context, &types) {
                        types.insert(local.clone(), ty);
                    }
                }
                crate::ir::IrInstruction::AssignFunction { local, value } => {
                    if let Some(ty) = infer_function_expr_ty(value, backend_context, &types) {
                        types.insert(local.clone(), ty);
                    }
                }
                crate::ir::IrInstruction::AssignEnumMatch { local, ty, .. }
                | crate::ir::IrInstruction::BindEnumPayload {
                    local,
                    payload_ty: ty,
                    ..
                } => {
                    types.insert(local.clone(), ty.clone());
                }
                crate::ir::IrInstruction::Drop { local, ty } => {
                    types.entry(local.clone()).or_insert_with(|| ty.clone());
                }
                crate::ir::IrInstruction::AssignField { .. }
                | crate::ir::IrInstruction::Let { .. }
                | crate::ir::IrInstruction::ConstInt { .. }
                | crate::ir::IrInstruction::ReturnUnit
                | crate::ir::IrInstruction::ReturnInt { .. }
                | crate::ir::IrInstruction::ReturnFloat { .. }
                | crate::ir::IrInstruction::ReturnBool { .. }
                | crate::ir::IrInstruction::ReturnString { .. }
                | crate::ir::IrInstruction::ReturnStruct { .. }
                | crate::ir::IrInstruction::ReturnEnum { .. }
                | crate::ir::IrInstruction::ReturnEnumMatch { .. }
                | crate::ir::IrInstruction::ReturnFunction { .. }
                | crate::ir::IrInstruction::PrintString { .. }
                | crate::ir::IrInstruction::PrintStringExpr { .. }
                | crate::ir::IrInstruction::PrintInt { .. }
                | crate::ir::IrInstruction::Assert { .. }
                | crate::ir::IrInstruction::RawWrite { .. }
                | crate::ir::IrInstruction::RawStore8 { .. }
                | crate::ir::IrInstruction::RawStore64 { .. }
                | crate::ir::IrInstruction::RawStringStore8 { .. }
                | crate::ir::IrInstruction::Call { .. }
                | crate::ir::IrInstruction::IndirectCall { .. }
                | crate::ir::IrInstruction::DropBoxStorage { .. } => {}
                _ => {}
            }
        }
    }
    types
}

fn infer_struct_expr_ty(
    expr: &IrStructExpr,
    backend_context: &BackendContext,
    local_types: &HashMap<String, IrValueTy>,
) -> Option<IrValueTy> {
    match expr {
        IrStructExpr::Local(name) => local_types.get(name).cloned(),
        IrStructExpr::Field { base, field } => {
            let layout = field_layout(backend_context, local_types, base, field).ok()?;
            Some(layout.ty.clone())
        }
        IrStructExpr::Call { callee, .. } => backend_context
            .function_sigs
            .get(callee)
            .map(|sig| sig.ret.clone()),
        IrStructExpr::Construct { name, .. } => Some(IrValueTy::Struct(name.clone())),
        IrStructExpr::IndirectCall { .. } | IrStructExpr::EnvLoad { .. } => None,
    }
}

fn infer_enum_expr_ty(
    expr: &IrEnumExpr,
    backend_context: &BackendContext,
    local_types: &HashMap<String, IrValueTy>,
) -> Option<IrValueTy> {
    match expr {
        IrEnumExpr::Local(name) => local_types.get(name).cloned(),
        IrEnumExpr::Call { callee, .. } => backend_context
            .function_sigs
            .get(callee)
            .map(|sig| sig.ret.clone()),
        IrEnumExpr::Construct { enum_name, .. } => Some(IrValueTy::Enum(enum_name.clone())),
        IrEnumExpr::IndirectCall { .. } | IrEnumExpr::EnvLoad { .. } => None,
    }
}

fn infer_function_expr_ty(
    expr: &IrFunctionExpr,
    backend_context: &BackendContext,
    local_types: &HashMap<String, IrValueTy>,
) -> Option<IrValueTy> {
    match expr {
        IrFunctionExpr::Local(name) => local_types.get(name).cloned(),
        IrFunctionExpr::Named(name) => {
            backend_context
                .function_sigs
                .get(name)
                .map(|sig| IrValueTy::Function {
                    params: sig.params.clone(),
                    ret: Box::new(sig.ret.clone()),
                    is_async: false,
                })
        }
        IrFunctionExpr::Field { base, field } => {
            let layout = field_layout(backend_context, local_types, base, field).ok()?;
            Some(layout.ty.clone())
        }
        IrFunctionExpr::Call { callee, .. } => backend_context
            .function_sigs
            .get(callee)
            .map(|sig| sig.ret.clone()),
        IrFunctionExpr::FromPtr(_) => None,
    }
}

fn infer_value_expr_ty(
    expr: &IrValueExpr,
    backend_context: &BackendContext,
    local_types: &HashMap<String, IrValueTy>,
) -> Option<IrValueTy> {
    match expr {
        IrValueExpr::Int(_) => Some(IrValueTy::Int(IrIntTy::Int)),
        IrValueExpr::Float(_) => Some(IrValueTy::Float64),
        IrValueExpr::Bool(_) => Some(IrValueTy::Bool),
        IrValueExpr::String(_) => Some(IrValueTy::OwnedString),
        IrValueExpr::Struct(expr) => infer_struct_expr_ty(expr, backend_context, local_types),
        IrValueExpr::Enum(expr) => infer_enum_expr_ty(expr, backend_context, local_types),
        IrValueExpr::Function(expr) => infer_function_expr_ty(expr, backend_context, local_types),
    }
}

fn variable_for(
    builder: &mut FunctionBuilder,
    variables: &mut HashMap<String, Variable>,
    next_variable: &mut usize,
    name: String,
) -> Variable {
    *variables.entry(name).or_insert_with(|| {
        let variable = Variable::new(*next_variable);
        *next_variable += 1;
        builder.declare_var(variable, types::I64);
        variable
    })
}

fn codegen_terminator(
    builder: &mut FunctionBuilder,
    module: &mut ObjectModule,
    backend_context: &BackendContext,
    local_types: &HashMap<String, IrValueTy>,
    variables: &mut HashMap<String, Variable>,
    next_variable: &mut usize,
    block_map: &HashMap<usize, ClifBlock>,
    terminator: &IrTerminator,
) -> Result<(), Vec<Diagnostic>> {
    match terminator {
        IrTerminator::Return => {
            let return_value = builder.ins().iconst(types::I64, 0);
            builder.ins().return_(&[return_value]);
        }
        IrTerminator::Jump { target } => {
            builder.ins().jump(block_id(block_map, *target)?, &[]);
        }
        IrTerminator::Branch {
            condition,
            then_block,
            else_block,
        } => {
            let Some(condition) = condition else {
                return Err(diagnostic(
                    "Cranelift backend requires a concrete branch condition".to_string(),
                ));
            };
            let condition = codegen_bool_expr(
                builder,
                module,
                backend_context,
                local_types,
                variables,
                next_variable,
                condition,
            )?;
            builder.ins().brif(
                condition,
                block_id(block_map, *then_block)?,
                &[],
                block_id(block_map, *else_block)?,
                &[],
            );
        }
        IrTerminator::Switch {
            value,
            arms,
            fallback,
        } => {
            let enum_value = codegen_enum_expr(
                builder,
                module,
                backend_context,
                local_types,
                variables,
                next_variable,
                value,
            )?;
            let tag = builder
                .ins()
                .load(types::I64, MemFlags::new(), enum_value, 0);
            let mut next_test_block = None;
            for (index, arm) in arms.iter().enumerate() {
                let test_block = next_test_block.take().unwrap_or_else(|| {
                    if index == 0 {
                        builder.current_block().expect("current block")
                    } else {
                        builder.create_block()
                    }
                });
                if index > 0 {
                    builder.switch_to_block(test_block);
                }
                let else_block = if index + 1 == arms.len() {
                    block_id(block_map, *fallback)?
                } else {
                    let block = builder.create_block();
                    next_test_block = Some(block);
                    block
                };
                let expected_tag = builder.ins().iconst(types::I64, arm.tag);
                let matches = builder.ins().icmp(IntCC::Equal, tag, expected_tag);
                builder.ins().brif(
                    matches,
                    block_id(block_map, arm.target)?,
                    &[],
                    else_block,
                    &[],
                );
            }
            if arms.is_empty() {
                builder.ins().jump(block_id(block_map, *fallback)?, &[]);
            }
        }
        IrTerminator::Unreachable => {
            builder.ins().trap(TrapCode::unwrap_user(0));
        }
    }
    Ok(())
}

fn codegen_assert(
    builder: &mut FunctionBuilder,
    module: &mut ObjectModule,
    backend_context: &BackendContext,
    local_types: &HashMap<String, IrValueTy>,
    variables: &mut HashMap<String, Variable>,
    next_variable: &mut usize,
    condition: &IrBoolExpr,
    message: &str,
) -> Result<(), Vec<Diagnostic>> {
    let condition = codegen_bool_expr(
        builder,
        module,
        backend_context,
        local_types,
        variables,
        next_variable,
        condition,
    )?;
    let pass_block = builder.create_block();
    let fail_block = builder.create_block();
    builder
        .ins()
        .brif(condition, pass_block, &[], fail_block, &[]);

    builder.switch_to_block(fail_block);
    codegen_print_string(builder, module, backend_context, message)?;
    let one = builder.ins().iconst(types::I64, 1);
    builder.ins().return_(&[one]);

    builder.switch_to_block(pass_block);
    Ok(())
}

fn block_id(
    block_map: &HashMap<usize, ClifBlock>,
    id: usize,
) -> Result<ClifBlock, Vec<Diagnostic>> {
    block_map
        .get(&id)
        .copied()
        .ok_or_else(|| diagnostic(format!("unknown IR block `{id}`")))
}

fn codegen_malloc(
    builder: &mut FunctionBuilder,
    module: &mut ObjectModule,
    backend_context: &BackendContext,
    size: cranelift_codegen::ir::Value,
) -> cranelift_codegen::ir::Value {
    let malloc = module.declare_func_in_func(backend_context.malloc_id, builder.func);
    let call = builder.ins().call(malloc, &[size]);
    let ptr = builder.inst_results(call)[0];
    let malloc_size = module.declare_func_in_func(backend_context.malloc_size_id, builder.func);
    let size_call = builder.ins().call(malloc_size, &[ptr]);
    let usable_size = builder.inst_results(size_call)[0];
    increment_counter(
        builder,
        module,
        &backend_context.memory_counters.alloc_count,
        1,
    );
    add_live_bytes(
        builder,
        module,
        &backend_context.memory_counters,
        usable_size,
    );
    ptr
}

fn codegen_free(
    builder: &mut FunctionBuilder,
    module: &mut ObjectModule,
    backend_context: &BackendContext,
    value: cranelift_codegen::ir::Value,
) {
    let malloc_size = module.declare_func_in_func(backend_context.malloc_size_id, builder.func);
    let size_call = builder.ins().call(malloc_size, &[value]);
    let usable_size = builder.inst_results(size_call)[0];
    increment_counter(
        builder,
        module,
        &backend_context.memory_counters.free_count,
        1,
    );
    sub_live_bytes(
        builder,
        module,
        &backend_context.memory_counters,
        usable_size,
    );
    let free = module.declare_func_in_func(backend_context.free_id, builder.func);
    builder.ins().call(free, &[value]);
}

fn counter_value(
    builder: &mut FunctionBuilder,
    module: &mut ObjectModule,
    counter: DataId,
) -> cranelift_codegen::ir::Value {
    let global = module.declare_data_in_func(counter, builder.func);
    let address = builder.ins().symbol_value(types::I64, global);
    builder
        .ins()
        .load(types::I64, MemFlags::trusted(), address, 0)
}

fn store_counter_value(
    builder: &mut FunctionBuilder,
    module: &mut ObjectModule,
    counter: DataId,
    value: cranelift_codegen::ir::Value,
) {
    let global = module.declare_data_in_func(counter, builder.func);
    let address = builder.ins().symbol_value(types::I64, global);
    builder.ins().store(MemFlags::trusted(), value, address, 0);
}

fn increment_counter(
    builder: &mut FunctionBuilder,
    module: &mut ObjectModule,
    counter: &DataId,
    amount: i64,
) {
    let current = counter_value(builder, module, *counter);
    let next = builder.ins().iadd_imm(current, amount);
    store_counter_value(builder, module, *counter, next);
}

fn add_live_bytes(
    builder: &mut FunctionBuilder,
    module: &mut ObjectModule,
    counters: &MemoryCounters,
    amount: cranelift_codegen::ir::Value,
) {
    let current = counter_value(builder, module, counters.live_bytes);
    let next = builder.ins().iadd(current, amount);
    store_counter_value(builder, module, counters.live_bytes, next);
    let high = counter_value(builder, module, counters.high_water_bytes);
    let is_higher = builder.ins().icmp(IntCC::UnsignedGreaterThan, next, high);
    let new_high = builder.ins().select(is_higher, next, high);
    store_counter_value(builder, module, counters.high_water_bytes, new_high);
}

fn sub_live_bytes(
    builder: &mut FunctionBuilder,
    module: &mut ObjectModule,
    counters: &MemoryCounters,
    amount: cranelift_codegen::ir::Value,
) {
    let current = counter_value(builder, module, counters.live_bytes);
    let next = builder.ins().isub(current, amount);
    store_counter_value(builder, module, counters.live_bytes, next);
}

fn codegen_int_expr(
    builder: &mut FunctionBuilder,
    module: &mut ObjectModule,
    backend_context: &BackendContext,
    local_types: &HashMap<String, IrValueTy>,
    variables: &mut HashMap<String, Variable>,
    next_variable: &mut usize,
    expr: &IrIntExpr,
) -> Result<cranelift_codegen::ir::Value, Vec<Diagnostic>> {
    match expr {
        IrIntExpr::Const(value) => Ok(builder.ins().iconst(types::I64, *value)),
        IrIntExpr::Local(name) => {
            let Some(variable) = variables.get(name) else {
                return Err(diagnostic(format!("unknown integer local `{name}`")));
            };
            Ok(builder.use_var(*variable))
        }
        IrIntExpr::Field { base, field } => {
            let base_value = use_local(builder, variables, base, "struct")?;
            let field = field_layout(backend_context, local_types, base, field)?;
            Ok(load_value(builder, base_value, field.offset, &field.ty))
        }
        IrIntExpr::EnumTag(expr) => {
            let value = codegen_enum_expr(
                builder,
                module,
                backend_context,
                local_types,
                variables,
                next_variable,
                expr,
            )?;
            Ok(builder.ins().load(types::I64, MemFlags::new(), value, 0))
        }
        IrIntExpr::EnvLoad { offset } => {
            let env = use_local(builder, variables, "__env", "closure environment")?;
            Ok(builder
                .ins()
                .load(types::I64, MemFlags::new(), env, *offset))
        }
        IrIntExpr::Call { callee, args } => {
            let Some(function_id) = backend_context.function_ids.get(callee) else {
                return Err(diagnostic(format!("unknown function `{callee}`")));
            };
            let local_callee = module.declare_func_in_func(*function_id, builder.func);
            let args = codegen_call_args(
                builder,
                module,
                backend_context,
                local_types,
                variables,
                next_variable,
                callee,
                args,
            )?;
            let call = builder.ins().call(local_callee, &args);
            let result = builder.inst_results(call)[0];
            let ret = backend_context
                .function_sigs
                .get(callee)
                .map(|sig| &sig.ret)
                .unwrap_or(&IrValueTy::Unknown);
            Ok(extend_to_i64(builder, result, ret))
        }
        IrIntExpr::IndirectCall { callee, args } => {
            let callee = codegen_function_expr(
                builder,
                module,
                backend_context,
                local_types,
                variables,
                next_variable,
                callee,
            )?;
            let mut sig = module.make_signature();
            for _ in args {
                sig.params
                    .push(cranelift_codegen::ir::AbiParam::new(types::I64));
            }
            sig.returns
                .push(cranelift_codegen::ir::AbiParam::new(types::I64));
            let sig_ref = builder.func.import_signature(sig);
            let args = codegen_value_args(
                builder,
                module,
                backend_context,
                local_types,
                variables,
                next_variable,
                args,
            )?;
            let call = builder.ins().call_indirect(sig_ref, callee, &args);
            let results = builder.inst_results(call);
            Ok(results[0])
        }
        IrIntExpr::StringLen(expr) => {
            let value = codegen_string_expr(
                builder,
                module,
                backend_context,
                local_types,
                variables,
                next_variable,
                expr,
            )?;
            let strlen = module.declare_func_in_func(backend_context.strlen_id, builder.func);
            let call = builder.ins().call(strlen, &[value]);
            Ok(builder.inst_results(call)[0])
        }
        IrIntExpr::StringPtr(expr) => codegen_string_expr(
            builder,
            module,
            backend_context,
            local_types,
            variables,
            next_variable,
            expr,
        ),
        IrIntExpr::FunctionPtr(expr) => codegen_function_expr(
            builder,
            module,
            backend_context,
            local_types,
            variables,
            next_variable,
            expr,
        ),
        IrIntExpr::FloatToInt(expr) => {
            let value = codegen_float_expr(
                builder,
                module,
                backend_context,
                local_types,
                variables,
                next_variable,
                expr,
            )?;
            Ok(builder.ins().fcvt_to_sint(types::I64, value))
        }
        IrIntExpr::RawWrite { fd, text } => codegen_raw_write(
            builder,
            module,
            backend_context,
            local_types,
            variables,
            next_variable,
            fd,
            text,
        ),
        IrIntExpr::RawAlloc { size } => {
            let size = codegen_int_expr(
                builder,
                module,
                backend_context,
                local_types,
                variables,
                next_variable,
                size,
            )?;
            Ok(codegen_malloc(builder, module, backend_context, size))
        }
        IrIntExpr::RawLoad8 { ptr, offset } => {
            let ptr = codegen_int_expr(
                builder,
                module,
                backend_context,
                local_types,
                variables,
                next_variable,
                ptr,
            )?;
            let offset = codegen_int_expr(
                builder,
                module,
                backend_context,
                local_types,
                variables,
                next_variable,
                offset,
            )?;
            let address = builder.ins().iadd(ptr, offset);
            Ok(builder
                .ins()
                .uload8(types::I64, MemFlags::new(), address, 0))
        }
        IrIntExpr::RawLoad64 { ptr, offset } => {
            let ptr = codegen_int_expr(
                builder,
                module,
                backend_context,
                local_types,
                variables,
                next_variable,
                ptr,
            )?;
            let offset = codegen_int_expr(
                builder,
                module,
                backend_context,
                local_types,
                variables,
                next_variable,
                offset,
            )?;
            let address = builder.ins().iadd(ptr, offset);
            Ok(builder.ins().load(types::I64, MemFlags::new(), address, 0))
        }
        IrIntExpr::RawSetNonblocking { fd } => codegen_raw_set_nonblocking(
            builder,
            module,
            backend_context,
            local_types,
            variables,
            next_variable,
            fd,
        ),
        IrIntExpr::RawThreadSpawn { task, captures } => codegen_raw_thread_spawn(
            builder,
            module,
            backend_context,
            local_types,
            variables,
            next_variable,
            task,
            captures,
        ),
        IrIntExpr::RawThreadJoin { handle } => codegen_raw_thread_join(
            builder,
            module,
            backend_context,
            local_types,
            variables,
            next_variable,
            handle,
        ),
        IrIntExpr::RawMemAllocCount => Ok(counter_value(
            builder,
            module,
            backend_context.memory_counters.alloc_count,
        )),
        IrIntExpr::RawMemFreeCount => Ok(counter_value(
            builder,
            module,
            backend_context.memory_counters.free_count,
        )),
        IrIntExpr::RawMemLiveBytes => Ok(counter_value(
            builder,
            module,
            backend_context.memory_counters.live_bytes,
        )),
        IrIntExpr::RawMemHighWaterBytes => Ok(counter_value(
            builder,
            module,
            backend_context.memory_counters.high_water_bytes,
        )),
        IrIntExpr::Binary { op, left, right } => {
            let left = codegen_int_expr(
                builder,
                module,
                backend_context,
                local_types,
                variables,
                next_variable,
                left,
            )?;
            let right = codegen_int_expr(
                builder,
                module,
                backend_context,
                local_types,
                variables,
                next_variable,
                right,
            )?;
            let value = match op {
                IrIntBinaryOp::Add => builder.ins().iadd(left, right),
                IrIntBinaryOp::Sub => builder.ins().isub(left, right),
                IrIntBinaryOp::Mul => builder.ins().imul(left, right),
                IrIntBinaryOp::Div => builder.ins().sdiv(left, right),
                IrIntBinaryOp::Rem => builder.ins().srem(left, right),
                IrIntBinaryOp::BitAnd => builder.ins().band(left, right),
                IrIntBinaryOp::BitOr => builder.ins().bor(left, right),
                IrIntBinaryOp::BitXor => builder.ins().bxor(left, right),
                IrIntBinaryOp::Shl => builder.ins().ishl(left, right),
                IrIntBinaryOp::Shr => builder.ins().sshr(left, right),
            };
            Ok(value)
        }
    }
}

fn codegen_float_expr(
    builder: &mut FunctionBuilder,
    module: &mut ObjectModule,
    backend_context: &BackendContext,
    local_types: &HashMap<String, IrValueTy>,
    variables: &mut HashMap<String, Variable>,
    next_variable: &mut usize,
    expr: &IrFloatExpr,
) -> Result<cranelift_codegen::ir::Value, Vec<Diagnostic>> {
    match expr {
        IrFloatExpr::Const(value) => {
            let value = value
                .parse::<f64>()
                .map_err(|err| diagnostic(format!("invalid Float64 literal `{value}`: {err}")))?;
            Ok(builder.ins().f64const(value))
        }
        IrFloatExpr::Local(name) => {
            let Some(variable) = variables.get(name) else {
                return Err(diagnostic(format!("unknown Float64 local `{name}`")));
            };
            Ok(builder.use_var(*variable))
        }
        IrFloatExpr::Field { base, field } => {
            let base_value = use_local(builder, variables, base, "struct")?;
            let field = field_layout(backend_context, local_types, base, field)?;
            Ok(load_value(builder, base_value, field.offset, &field.ty))
        }
        IrFloatExpr::EnvLoad { offset } => {
            let env = use_local(builder, variables, "__env", "closure environment")?;
            Ok(builder
                .ins()
                .load(types::F64, MemFlags::new(), env, *offset))
        }
        IrFloatExpr::Call { callee, args } => {
            let Some(function_id) = backend_context.function_ids.get(callee) else {
                return Err(diagnostic(format!("unknown function `{callee}`")));
            };
            let local_callee = module.declare_func_in_func(*function_id, builder.func);
            let args = codegen_call_args(
                builder,
                module,
                backend_context,
                local_types,
                variables,
                next_variable,
                callee,
                args,
            )?;
            let call = builder.ins().call(local_callee, &args);
            Ok(builder.inst_results(call)[0])
        }
        IrFloatExpr::IndirectCall { callee, args } => {
            let callee = codegen_function_expr(
                builder,
                module,
                backend_context,
                local_types,
                variables,
                next_variable,
                callee,
            )?;
            let mut sig = module.make_signature();
            for arg in args {
                let ty = infer_value_expr_ty(arg, backend_context, local_types)
                    .unwrap_or(IrValueTy::Unknown);
                sig.params
                    .push(cranelift_codegen::ir::AbiParam::new(clif_abi_type(&ty)));
            }
            sig.returns
                .push(cranelift_codegen::ir::AbiParam::new(types::F64));
            let sig_ref = builder.func.import_signature(sig);
            let args = codegen_value_args(
                builder,
                module,
                backend_context,
                local_types,
                variables,
                next_variable,
                args,
            )?;
            let call = builder.ins().call_indirect(sig_ref, callee, &args);
            Ok(builder.inst_results(call)[0])
        }
        IrFloatExpr::IntToFloat(expr) => {
            let value = codegen_int_expr(
                builder,
                module,
                backend_context,
                local_types,
                variables,
                next_variable,
                expr,
            )?;
            Ok(builder.ins().fcvt_from_sint(types::F64, value))
        }
        IrFloatExpr::Binary { op, left, right } => {
            let left = codegen_float_expr(
                builder,
                module,
                backend_context,
                local_types,
                variables,
                next_variable,
                left,
            )?;
            let right = codegen_float_expr(
                builder,
                module,
                backend_context,
                local_types,
                variables,
                next_variable,
                right,
            )?;
            let value = match op {
                IrFloatBinaryOp::Add => builder.ins().fadd(left, right),
                IrFloatBinaryOp::Sub => builder.ins().fsub(left, right),
                IrFloatBinaryOp::Mul => builder.ins().fmul(left, right),
                IrFloatBinaryOp::Div => builder.ins().fdiv(left, right),
            };
            Ok(value)
        }
    }
}

fn codegen_string_expr(
    builder: &mut FunctionBuilder,
    module: &mut ObjectModule,
    backend_context: &BackendContext,
    local_types: &HashMap<String, IrValueTy>,
    variables: &mut HashMap<String, Variable>,
    next_variable: &mut usize,
    expr: &IrStringExpr,
) -> Result<cranelift_codegen::ir::Value, Vec<Diagnostic>> {
    match expr {
        IrStringExpr::Literal(value) => string_address(builder, module, backend_context, value),
        IrStringExpr::Local(name) => use_local(builder, variables, name, "string"),
        IrStringExpr::RawAlloc { size } => {
            let size = codegen_int_expr(
                builder,
                module,
                backend_context,
                local_types,
                variables,
                next_variable,
                size,
            )?;
            Ok(codegen_malloc(builder, module, backend_context, size))
        }
        IrStringExpr::EnvLoad { offset } => {
            let env = use_local(builder, variables, "__env", "closure environment")?;
            Ok(builder
                .ins()
                .load(types::I64, MemFlags::new(), env, *offset))
        }
        IrStringExpr::Field { base, field } => {
            let base_value = use_local(builder, variables, base, "struct")?;
            let field = field_layout(backend_context, local_types, base, field)?;
            Ok(load_value(builder, base_value, field.offset, &field.ty))
        }
        IrStringExpr::Concat { left, right } => codegen_string_concat(
            builder,
            module,
            backend_context,
            local_types,
            variables,
            next_variable,
            left,
            right,
        ),
        IrStringExpr::IntToString(expr) => codegen_int_to_string(
            builder,
            module,
            backend_context,
            local_types,
            variables,
            next_variable,
            expr,
        ),
        IrStringExpr::FromPtr(expr) => codegen_int_expr(
            builder,
            module,
            backend_context,
            local_types,
            variables,
            next_variable,
            expr,
        ),
        IrStringExpr::Call { callee, args } => {
            let Some(function_id) = backend_context.function_ids.get(callee) else {
                return Err(diagnostic(format!("unknown function `{callee}`")));
            };
            let local_callee = module.declare_func_in_func(*function_id, builder.func);
            let args = codegen_call_args(
                builder,
                module,
                backend_context,
                local_types,
                variables,
                next_variable,
                callee,
                args,
            )?;
            let call = builder.ins().call(local_callee, &args);
            Ok(builder.inst_results(call)[0])
        }
        IrStringExpr::IndirectCall { callee, args } => {
            let callee = codegen_function_expr(
                builder,
                module,
                backend_context,
                local_types,
                variables,
                next_variable,
                callee,
            )?;
            let mut sig = module.make_signature();
            for _ in args {
                sig.params
                    .push(cranelift_codegen::ir::AbiParam::new(types::I64));
            }
            sig.returns
                .push(cranelift_codegen::ir::AbiParam::new(types::I64));
            let sig_ref = builder.func.import_signature(sig);
            let args = codegen_value_args(
                builder,
                module,
                backend_context,
                local_types,
                variables,
                next_variable,
                args,
            )?;
            let call = builder.ins().call_indirect(sig_ref, callee, &args);
            Ok(builder.inst_results(call)[0])
        }
    }
}

fn codegen_string_concat(
    builder: &mut FunctionBuilder,
    module: &mut ObjectModule,
    backend_context: &BackendContext,
    local_types: &HashMap<String, IrValueTy>,
    variables: &mut HashMap<String, Variable>,
    next_variable: &mut usize,
    left: &IrStringExpr,
    right: &IrStringExpr,
) -> Result<cranelift_codegen::ir::Value, Vec<Diagnostic>> {
    let left = codegen_string_expr(
        builder,
        module,
        backend_context,
        local_types,
        variables,
        next_variable,
        left,
    )?;
    let right = codegen_string_expr(
        builder,
        module,
        backend_context,
        local_types,
        variables,
        next_variable,
        right,
    )?;
    let strlen = module.declare_func_in_func(backend_context.strlen_id, builder.func);
    let left_len_call = builder.ins().call(strlen, &[left]);
    let left_len = builder.inst_results(left_len_call)[0];
    let right_len_call = builder.ins().call(strlen, &[right]);
    let right_len = builder.inst_results(right_len_call)[0];
    let content_len = builder.ins().iadd(left_len, right_len);
    let one = builder.ins().iconst(types::I64, 1);
    let alloc_len = builder.ins().iadd(content_len, one);
    let output = codegen_malloc(builder, module, backend_context, alloc_len);

    let memcpy = module.declare_func_in_func(backend_context.memcpy_id, builder.func);
    builder.ins().call(memcpy, &[output, left, left_len]);
    let right_dest = builder.ins().iadd(output, left_len);
    builder.ins().call(memcpy, &[right_dest, right, right_len]);
    let nul_dest = builder.ins().iadd(output, content_len);
    let zero = builder.ins().iconst(types::I8, 0);
    builder.ins().store(MemFlags::new(), zero, nul_dest, 0);
    Ok(output)
}

fn codegen_int_to_string(
    builder: &mut FunctionBuilder,
    module: &mut ObjectModule,
    backend_context: &BackendContext,
    local_types: &HashMap<String, IrValueTy>,
    variables: &mut HashMap<String, Variable>,
    next_variable: &mut usize,
    expr: &IrIntExpr,
) -> Result<cranelift_codegen::ir::Value, Vec<Diagnostic>> {
    let value = codegen_int_expr(
        builder,
        module,
        backend_context,
        local_types,
        variables,
        next_variable,
        expr,
    )?;
    let size = builder.ins().iconst(types::I64, 32);
    let buffer = codegen_malloc(builder, module, backend_context, size);
    let end_offset = builder.ins().iconst(types::I64, 31);
    let end = builder.ins().iadd(buffer, end_offset);
    let nul = builder.ins().iconst(types::I8, 0);
    builder.ins().store(MemFlags::new(), nul, end, 0);

    let value_slot =
        builder.create_sized_stack_slot(StackSlotData::new(StackSlotKind::ExplicitSlot, 8, 0));
    let ptr_slot =
        builder.create_sized_stack_slot(StackSlotData::new(StackSlotKind::ExplicitSlot, 8, 0));
    builder.ins().stack_store(value, value_slot, 0);
    builder.ins().stack_store(end, ptr_slot, 0);

    let zero_block = builder.create_block();
    let loop_header = builder.create_block();
    let loop_body = builder.create_block();
    let done_block = builder.create_block();

    let is_zero = builder.ins().icmp_imm(IntCC::Equal, value, 0);
    builder
        .ins()
        .brif(is_zero, zero_block, &[], loop_header, &[]);

    builder.switch_to_block(zero_block);
    let one = builder.ins().iconst(types::I64, 1);
    let ptr = builder.ins().stack_load(types::I64, ptr_slot, 0);
    let ptr = builder.ins().isub(ptr, one);
    let digit = builder.ins().iconst(types::I8, b'0' as i64);
    builder.ins().store(MemFlags::new(), digit, ptr, 0);
    builder.ins().stack_store(ptr, ptr_slot, 0);
    builder.ins().jump(done_block, &[]);

    builder.switch_to_block(loop_header);
    let current = builder.ins().stack_load(types::I64, value_slot, 0);
    let has_digits = builder.ins().icmp_imm(IntCC::SignedGreaterThan, current, 0);
    builder
        .ins()
        .brif(has_digits, loop_body, &[], done_block, &[]);

    builder.switch_to_block(loop_body);
    let ten = builder.ins().iconst(types::I64, 10);
    let current = builder.ins().stack_load(types::I64, value_slot, 0);
    let digit = builder.ins().srem(current, ten);
    let ascii_base = builder.ins().iconst(types::I64, b'0' as i64);
    let ascii = builder.ins().iadd(digit, ascii_base);
    let ascii = builder.ins().ireduce(types::I8, ascii);
    let one = builder.ins().iconst(types::I64, 1);
    let ptr = builder.ins().stack_load(types::I64, ptr_slot, 0);
    let ptr = builder.ins().isub(ptr, one);
    builder.ins().store(MemFlags::new(), ascii, ptr, 0);
    builder.ins().stack_store(ptr, ptr_slot, 0);
    let next = builder.ins().sdiv(current, ten);
    builder.ins().stack_store(next, value_slot, 0);
    builder.ins().jump(loop_header, &[]);

    builder.switch_to_block(done_block);
    let digits_start = builder.ins().stack_load(types::I64, ptr_slot, 0);
    let length = builder.ins().isub(end, digits_start);
    let index_slot =
        builder.create_sized_stack_slot(StackSlotData::new(StackSlotKind::ExplicitSlot, 8, 0));
    let zero_i64 = builder.ins().iconst(types::I64, 0);
    builder.ins().stack_store(zero_i64, index_slot, 0);

    let copy_header = builder.create_block();
    let copy_body = builder.create_block();
    let copy_done = builder.create_block();
    builder.ins().jump(copy_header, &[]);

    builder.switch_to_block(copy_header);
    let index = builder.ins().stack_load(types::I64, index_slot, 0);
    let has_more = builder.ins().icmp(IntCC::UnsignedLessThan, index, length);
    builder.ins().brif(has_more, copy_body, &[], copy_done, &[]);

    builder.switch_to_block(copy_body);
    let index = builder.ins().stack_load(types::I64, index_slot, 0);
    let src = builder.ins().iadd(digits_start, index);
    let dst = builder.ins().iadd(buffer, index);
    let byte = builder.ins().uload8(types::I64, MemFlags::new(), src, 0);
    let byte = builder.ins().ireduce(types::I8, byte);
    builder.ins().store(MemFlags::new(), byte, dst, 0);
    let one = builder.ins().iconst(types::I64, 1);
    let next = builder.ins().iadd(index, one);
    builder.ins().stack_store(next, index_slot, 0);
    builder.ins().jump(copy_header, &[]);

    builder.switch_to_block(copy_done);
    let nul_dest = builder.ins().iadd(buffer, length);
    let nul = builder.ins().iconst(types::I8, 0);
    builder.ins().store(MemFlags::new(), nul, nul_dest, 0);
    Ok(buffer)
}

fn codegen_string_compare(
    builder: &mut FunctionBuilder,
    module: &mut ObjectModule,
    backend_context: &BackendContext,
    local_types: &HashMap<String, IrValueTy>,
    variables: &mut HashMap<String, Variable>,
    next_variable: &mut usize,
    op: IrCompareOp,
    left: &IrStringExpr,
    right: &IrStringExpr,
) -> Result<cranelift_codegen::ir::Value, Vec<Diagnostic>> {
    let left = codegen_string_expr(
        builder,
        module,
        backend_context,
        local_types,
        variables,
        next_variable,
        left,
    )?;
    let right = codegen_string_expr(
        builder,
        module,
        backend_context,
        local_types,
        variables,
        next_variable,
        right,
    )?;

    let strlen = module.declare_func_in_func(backend_context.strlen_id, builder.func);
    let left_len_call = builder.ins().call(strlen, &[left]);
    let left_len = builder.inst_results(left_len_call)[0];
    let right_len_call = builder.ins().call(strlen, &[right]);
    let right_len = builder.inst_results(right_len_call)[0];

    let result_slot =
        builder.create_sized_stack_slot(StackSlotData::new(StackSlotKind::ExplicitSlot, 8, 0));
    let index_slot =
        builder.create_sized_stack_slot(StackSlotData::new(StackSlotKind::ExplicitSlot, 8, 0));
    let zero = builder.ins().iconst(types::I64, 0);
    builder.ins().stack_store(zero, index_slot, 0);

    let false_block = builder.create_block();
    let true_block = builder.create_block();
    let loop_header = builder.create_block();
    let loop_body = builder.create_block();
    let done_block = builder.create_block();

    let same_len = builder.ins().icmp(IntCC::Equal, left_len, right_len);
    builder
        .ins()
        .brif(same_len, loop_header, &[], false_block, &[]);

    builder.switch_to_block(loop_header);
    let index = builder.ins().stack_load(types::I64, index_slot, 0);
    let at_end = builder.ins().icmp(IntCC::Equal, index, left_len);
    builder.ins().brif(at_end, true_block, &[], loop_body, &[]);

    builder.switch_to_block(loop_body);
    let index = builder.ins().stack_load(types::I64, index_slot, 0);
    let left_addr = builder.ins().iadd(left, index);
    let right_addr = builder.ins().iadd(right, index);
    let left_byte = builder
        .ins()
        .uload8(types::I64, MemFlags::new(), left_addr, 0);
    let right_byte = builder
        .ins()
        .uload8(types::I64, MemFlags::new(), right_addr, 0);
    let same_byte = builder.ins().icmp(IntCC::Equal, left_byte, right_byte);
    let one = builder.ins().iconst(types::I64, 1);
    let next = builder.ins().iadd(index, one);
    builder.ins().stack_store(next, index_slot, 0);
    builder
        .ins()
        .brif(same_byte, loop_header, &[], false_block, &[]);

    builder.switch_to_block(false_block);
    let zero = builder.ins().iconst(types::I64, 0);
    builder.ins().stack_store(zero, result_slot, 0);
    builder.ins().jump(done_block, &[]);

    builder.switch_to_block(true_block);
    let one = builder.ins().iconst(types::I64, 1);
    builder.ins().stack_store(one, result_slot, 0);
    builder.ins().jump(done_block, &[]);

    builder.switch_to_block(done_block);
    let result = builder.ins().stack_load(types::I64, result_slot, 0);
    let condition = match op {
        IrCompareOp::Eq => builder.ins().icmp_imm(IntCC::NotEqual, result, 0),
        IrCompareOp::NotEq => builder.ins().icmp_imm(IntCC::Equal, result, 0),
        IrCompareOp::Lt | IrCompareOp::Le | IrCompareOp::Gt | IrCompareOp::Ge => {
            return Err(diagnostic(
                "ordered string comparisons are not supported".to_string(),
            ))
        }
    };
    Ok(condition)
}

fn codegen_struct_expr(
    builder: &mut FunctionBuilder,
    module: &mut ObjectModule,
    backend_context: &BackendContext,
    local_types: &HashMap<String, IrValueTy>,
    variables: &mut HashMap<String, Variable>,
    next_variable: &mut usize,
    expr: &IrStructExpr,
) -> Result<cranelift_codegen::ir::Value, Vec<Diagnostic>> {
    match expr {
        IrStructExpr::Local(name) => use_local(builder, variables, name, "struct"),
        IrStructExpr::Field { base, field } => {
            let base_value = use_local(builder, variables, base, "struct")?;
            let field = field_layout(backend_context, local_types, base, field)?;
            Ok(load_value(builder, base_value, field.offset, &field.ty))
        }
        IrStructExpr::EnvLoad { offset } => {
            let env = use_local(builder, variables, "__env", "closure environment")?;
            Ok(builder
                .ins()
                .load(types::I64, MemFlags::new(), env, *offset))
        }
        IrStructExpr::Call { callee, args } => {
            let Some(function_id) = backend_context.function_ids.get(callee) else {
                return Err(diagnostic(format!("unknown function `{callee}`")));
            };
            let local_callee = module.declare_func_in_func(*function_id, builder.func);
            let args = codegen_call_args(
                builder,
                module,
                backend_context,
                local_types,
                variables,
                next_variable,
                callee,
                args,
            )?;
            let call = builder.ins().call(local_callee, &args);
            Ok(builder.inst_results(call)[0])
        }
        IrStructExpr::IndirectCall { callee, args } => {
            let callee = codegen_function_expr(
                builder,
                module,
                backend_context,
                local_types,
                variables,
                next_variable,
                callee,
            )?;
            let mut sig = module.make_signature();
            for _ in args {
                sig.params
                    .push(cranelift_codegen::ir::AbiParam::new(types::I64));
            }
            sig.returns
                .push(cranelift_codegen::ir::AbiParam::new(types::I64));
            let sig_ref = builder.func.import_signature(sig);
            let args = codegen_value_args(
                builder,
                module,
                backend_context,
                local_types,
                variables,
                next_variable,
                args,
            )?;
            let call = builder.ins().call_indirect(sig_ref, callee, &args);
            Ok(builder.inst_results(call)[0])
        }
        IrStructExpr::Construct { name, fields } => {
            let Some(layout) = backend_context.struct_layouts.get(name).cloned() else {
                return Err(diagnostic(format!("unknown struct `{name}`")));
            };
            let size = builder.ins().iconst(types::I64, layout.size);
            let pointer = codegen_malloc(builder, module, backend_context, size);
            for field in fields {
                let Some(layout_field) = layout.fields.get(&field.name) else {
                    return Err(diagnostic(format!(
                        "unknown field `{}` on `{name}`",
                        field.name
                    )));
                };
                let value = codegen_value_expr(
                    builder,
                    module,
                    backend_context,
                    local_types,
                    variables,
                    next_variable,
                    &field.value,
                )?;
                store_value(
                    builder,
                    value,
                    pointer,
                    layout_field.offset,
                    &layout_field.ty,
                );
            }
            Ok(pointer)
        }
    }
}

fn codegen_enum_expr(
    builder: &mut FunctionBuilder,
    module: &mut ObjectModule,
    backend_context: &BackendContext,
    local_types: &HashMap<String, IrValueTy>,
    variables: &mut HashMap<String, Variable>,
    next_variable: &mut usize,
    expr: &IrEnumExpr,
) -> Result<cranelift_codegen::ir::Value, Vec<Diagnostic>> {
    match expr {
        IrEnumExpr::Local(name) => use_local(builder, variables, name, "enum"),
        IrEnumExpr::EnvLoad { offset } => {
            let env = use_local(builder, variables, "__env", "closure environment")?;
            Ok(builder
                .ins()
                .load(types::I64, MemFlags::new(), env, *offset))
        }
        IrEnumExpr::Call { callee, args } => {
            let Some(function_id) = backend_context.function_ids.get(callee) else {
                return Err(diagnostic(format!("unknown function `{callee}`")));
            };
            let local_callee = module.declare_func_in_func(*function_id, builder.func);
            let args = codegen_call_args(
                builder,
                module,
                backend_context,
                local_types,
                variables,
                next_variable,
                callee,
                args,
            )?;
            let call = builder.ins().call(local_callee, &args);
            Ok(builder.inst_results(call)[0])
        }
        IrEnumExpr::IndirectCall { callee, args } => {
            let callee = codegen_function_expr(
                builder,
                module,
                backend_context,
                local_types,
                variables,
                next_variable,
                callee,
            )?;
            let mut sig = module.make_signature();
            for _ in args {
                sig.params
                    .push(cranelift_codegen::ir::AbiParam::new(types::I64));
            }
            sig.returns
                .push(cranelift_codegen::ir::AbiParam::new(types::I64));
            let sig_ref = builder.func.import_signature(sig);
            let args = codegen_value_args(
                builder,
                module,
                backend_context,
                local_types,
                variables,
                next_variable,
                args,
            )?;
            let call = builder.ins().call_indirect(sig_ref, callee, &args);
            Ok(builder.inst_results(call)[0])
        }
        IrEnumExpr::Construct {
            tag,
            payloads,
            enum_name,
            variant,
            ..
        } => {
            let Some(layout) = backend_context.enum_layouts.get(enum_name) else {
                return Err(diagnostic(format!("unknown enum `{enum_name}`")));
            };
            let Some(variant_layout) = layout.variants.get(variant) else {
                return Err(diagnostic(format!(
                    "unknown variant `{variant}` on enum `{enum_name}`"
                )));
            };
            if payloads.len() != variant_layout.payload_tys.len() {
                return Err(diagnostic(format!(
                    "variant `{variant}` expects {} payload(s), got {}",
                    variant_layout.payload_tys.len(),
                    payloads.len()
                )));
            }
            let size = builder.ins().iconst(types::I64, layout.size);
            let pointer = codegen_malloc(builder, module, backend_context, size);
            let tag = builder.ins().iconst(types::I64, *tag);
            builder.ins().store(MemFlags::new(), tag, pointer, 0);
            for (index, payload) in payloads.iter().enumerate() {
                let value = codegen_value_expr(
                    builder,
                    module,
                    backend_context,
                    local_types,
                    variables,
                    next_variable,
                    payload,
                )?;
                store_value(
                    builder,
                    value,
                    pointer,
                    variant_layout.payload_offsets[index],
                    &variant_layout.payload_tys[index],
                );
            }
            Ok(pointer)
        }
    }
}

fn codegen_enum_payload(
    builder: &mut FunctionBuilder,
    enum_value: cranelift_codegen::ir::Value,
    value: &IrEnumExpr,
    payload_index: usize,
    payload_tys: &[IrValueTy],
    payload_ty: &IrValueTy,
) -> Result<cranelift_codegen::ir::Value, Vec<Diagnostic>> {
    let Some(payload_offset) = enum_payload_offsets(payload_tys)
        .0
        .get(payload_index)
        .copied()
    else {
        return Err(diagnostic(format!(
            "payload index {payload_index} is out of bounds for enum match value `{value:?}`"
        )));
    };
    Ok(load_value(builder, enum_value, payload_offset, payload_ty))
}

fn codegen_enum_match_value(
    builder: &mut FunctionBuilder,
    module: &mut ObjectModule,
    backend_context: &BackendContext,
    local_types: &HashMap<String, IrValueTy>,
    variables: &mut HashMap<String, Variable>,
    next_variable: &mut usize,
    value: &IrEnumExpr,
    arms: &[IrEnumMatchArm],
    _ty: &IrValueTy,
    free_value_storage: bool,
) -> Result<cranelift_codegen::ir::Value, Vec<Diagnostic>> {
    let enum_value = codegen_enum_expr(
        builder,
        module,
        backend_context,
        local_types,
        variables,
        next_variable,
        value,
    )?;
    let tag = builder
        .ins()
        .load(types::I64, MemFlags::new(), enum_value, 0);
    let result_slot =
        builder.create_sized_stack_slot(StackSlotData::new(StackSlotKind::ExplicitSlot, 8, 0));

    let merge_block = builder.create_block();
    let fallback_block = builder.create_block();
    let mut next_test_block = None;

    for (index, arm) in arms.iter().enumerate() {
        let arm_block = builder.create_block();
        let test_block = next_test_block.take().unwrap_or_else(|| {
            if index == 0 {
                builder.current_block().expect("current block")
            } else {
                builder.create_block()
            }
        });
        if index > 0 {
            builder.switch_to_block(test_block);
        }
        let else_block = if index + 1 == arms.len() {
            fallback_block
        } else {
            let block = builder.create_block();
            next_test_block = Some(block);
            block
        };
        let expected_tag = builder.ins().iconst(types::I64, arm.tag);
        let matches = builder.ins().icmp(IntCC::Equal, tag, expected_tag);
        builder.ins().brif(matches, arm_block, &[], else_block, &[]);

        builder.switch_to_block(arm_block);
        let (payload_offsets, _) = enum_payload_offsets(&arm.payload_tys);
        let mut arm_local_types = local_types.clone();
        for (payload_index, binding) in arm.bindings.iter().enumerate() {
            let payload_ty = arm
                .payload_tys
                .get(payload_index)
                .unwrap_or(&IrValueTy::Unknown);
            arm_local_types.insert(binding.clone(), payload_ty.clone());
            let payload = load_value(
                builder,
                enum_value,
                payload_offsets[payload_index],
                payload_ty,
            );
            let variable = variable_for(builder, variables, next_variable, binding.clone());
            builder.def_var(variable, payload);
        }
        let result = codegen_value_expr(
            builder,
            module,
            backend_context,
            &arm_local_types,
            variables,
            next_variable,
            &arm.body,
        )?;
        if free_value_storage {
            codegen_free(builder, module, backend_context, enum_value);
        }
        builder.ins().stack_store(result, result_slot, 0);
        builder.ins().jump(merge_block, &[]);
    }

    builder.switch_to_block(fallback_block);
    let fallback = builder.ins().iconst(types::I64, 0);
    builder.ins().stack_store(fallback, result_slot, 0);
    builder.ins().jump(merge_block, &[]);

    builder.switch_to_block(merge_block);
    Ok(builder.ins().stack_load(types::I64, result_slot, 0))
}

fn codegen_value_args(
    builder: &mut FunctionBuilder,
    module: &mut ObjectModule,
    backend_context: &BackendContext,
    local_types: &HashMap<String, IrValueTy>,
    variables: &mut HashMap<String, Variable>,
    next_variable: &mut usize,
    args: &[IrValueExpr],
) -> Result<Vec<cranelift_codegen::ir::Value>, Vec<Diagnostic>> {
    args.iter()
        .map(|arg| {
            codegen_value_expr(
                builder,
                module,
                backend_context,
                local_types,
                variables,
                next_variable,
                arg,
            )
        })
        .collect()
}

fn codegen_call_args(
    builder: &mut FunctionBuilder,
    module: &mut ObjectModule,
    backend_context: &BackendContext,
    local_types: &HashMap<String, IrValueTy>,
    variables: &mut HashMap<String, Variable>,
    next_variable: &mut usize,
    callee: &str,
    args: &[IrValueExpr],
) -> Result<Vec<cranelift_codegen::ir::Value>, Vec<Diagnostic>> {
    let expected = backend_context
        .function_sigs
        .get(callee)
        .map(|sig| sig.params.as_slice())
        .unwrap_or(&[]);
    args.iter()
        .enumerate()
        .map(|(index, arg)| {
            let value = codegen_value_expr(
                builder,
                module,
                backend_context,
                local_types,
                variables,
                next_variable,
                arg,
            )?;
            Ok(expected
                .get(index)
                .map(|ty| reduce_for_abi(builder, value, ty))
                .unwrap_or(value))
        })
        .collect()
}

fn codegen_value_expr(
    builder: &mut FunctionBuilder,
    module: &mut ObjectModule,
    backend_context: &BackendContext,
    local_types: &HashMap<String, IrValueTy>,
    variables: &mut HashMap<String, Variable>,
    next_variable: &mut usize,
    expr: &IrValueExpr,
) -> Result<cranelift_codegen::ir::Value, Vec<Diagnostic>> {
    match expr {
        IrValueExpr::Int(expr) => codegen_int_expr(
            builder,
            module,
            backend_context,
            local_types,
            variables,
            next_variable,
            expr,
        ),
        IrValueExpr::Float(expr) => codegen_float_expr(
            builder,
            module,
            backend_context,
            local_types,
            variables,
            next_variable,
            expr,
        ),
        IrValueExpr::Bool(expr) => codegen_bool_as_int(
            builder,
            module,
            backend_context,
            local_types,
            variables,
            next_variable,
            expr,
        ),
        IrValueExpr::String(expr) => codegen_string_expr(
            builder,
            module,
            backend_context,
            local_types,
            variables,
            next_variable,
            expr,
        ),
        IrValueExpr::Struct(expr) => codegen_struct_expr(
            builder,
            module,
            backend_context,
            local_types,
            variables,
            next_variable,
            expr,
        ),
        IrValueExpr::Enum(expr) => codegen_enum_expr(
            builder,
            module,
            backend_context,
            local_types,
            variables,
            next_variable,
            expr,
        ),
        IrValueExpr::Function(expr) => codegen_function_expr(
            builder,
            module,
            backend_context,
            local_types,
            variables,
            next_variable,
            expr,
        ),
    }
}

fn codegen_function_expr(
    builder: &mut FunctionBuilder,
    module: &mut ObjectModule,
    backend_context: &BackendContext,
    local_types: &HashMap<String, IrValueTy>,
    variables: &mut HashMap<String, Variable>,
    next_variable: &mut usize,
    expr: &IrFunctionExpr,
) -> Result<cranelift_codegen::ir::Value, Vec<Diagnostic>> {
    match expr {
        IrFunctionExpr::Local(name) => use_local(builder, variables, name, "function"),
        IrFunctionExpr::Named(name) => {
            let Some(function_id) = backend_context.function_ids.get(name) else {
                return Err(diagnostic(format!("unknown function `{name}`")));
            };
            let local_callee = module.declare_func_in_func(*function_id, builder.func);
            Ok(builder.ins().func_addr(types::I64, local_callee))
        }
        IrFunctionExpr::Field { base, field } => {
            let base_value = use_local(builder, variables, base, "struct")?;
            let field = field_layout(backend_context, local_types, base, field)?;
            Ok(load_value(builder, base_value, field.offset, &field.ty))
        }
        IrFunctionExpr::Call { callee, args } => {
            let Some(function_id) = backend_context.function_ids.get(callee) else {
                return Err(diagnostic(format!("unknown function `{callee}`")));
            };
            let local_callee = module.declare_func_in_func(*function_id, builder.func);
            let args = codegen_call_args(
                builder,
                module,
                backend_context,
                local_types,
                variables,
                next_variable,
                callee,
                args,
            )?;
            let call = builder.ins().call(local_callee, &args);
            Ok(builder.inst_results(call)[0])
        }
        IrFunctionExpr::FromPtr(ptr) => codegen_int_expr(
            builder,
            module,
            backend_context,
            local_types,
            variables,
            next_variable,
            ptr,
        ),
    }
}

fn use_local(
    builder: &mut FunctionBuilder,
    variables: &HashMap<String, Variable>,
    name: &str,
    kind: &str,
) -> Result<cranelift_codegen::ir::Value, Vec<Diagnostic>> {
    let Some(variable) = variables.get(name) else {
        return Err(diagnostic(format!("unknown {kind} local `{name}`")));
    };
    Ok(builder.use_var(*variable))
}

fn string_address(
    builder: &mut FunctionBuilder,
    module: &mut ObjectModule,
    backend_context: &BackendContext,
    value: &str,
) -> Result<cranelift_codegen::ir::Value, Vec<Diagnostic>> {
    let Some(data_id) = backend_context.string_data.get(value) else {
        return Err(diagnostic(format!("missing string data for `{value}`")));
    };
    let global = module.declare_data_in_func(*data_id, builder.func);
    Ok(builder.ins().symbol_value(types::I64, global))
}

fn field_layout<'a>(
    backend_context: &'a BackendContext,
    local_types: &HashMap<String, IrValueTy>,
    base: &str,
    field: &str,
) -> Result<&'a StructFieldLayout, Vec<Diagnostic>> {
    if let Some(layout_name) = local_types
        .get(base)
        .and_then(|ty| field_layout_name_for_ty(ty))
    {
        let layout = backend_context
            .struct_layouts
            .get(layout_name)
            .ok_or_else(|| {
                diagnostic(format!(
                    "unknown struct layout `{layout_name}` for local `{base}`"
                ))
            })?;
        return layout.fields.get(field).ok_or_else(|| {
            diagnostic(format!(
                "unknown field `{field}` on `{layout_name}` for local `{base}`"
            ))
        });
    }

    Err(diagnostic(format!(
        "unknown type for field `{field}` on struct local `{base}`"
    )))
}

fn field_layout_name_for_ty(ty: &IrValueTy) -> Option<&str> {
    match ty {
        IrValueTy::Struct(name) | IrValueTy::Enum(name) => Some(name.as_str()),
        IrValueTy::Vec(_) => Some("vec__Vec"),
        IrValueTy::Boxed(_) => Some("box__Box"),
        _ => None,
    }
}

fn codegen_bool_expr(
    builder: &mut FunctionBuilder,
    module: &mut ObjectModule,
    backend_context: &BackendContext,
    local_types: &HashMap<String, IrValueTy>,
    variables: &mut HashMap<String, Variable>,
    next_variable: &mut usize,
    expr: &IrBoolExpr,
) -> Result<cranelift_codegen::ir::Value, Vec<Diagnostic>> {
    match expr {
        IrBoolExpr::Const(value) => {
            let value = builder.ins().iconst(types::I64, i64::from(*value));
            Ok(builder.ins().icmp_imm(IntCC::NotEqual, value, 0))
        }
        IrBoolExpr::Local(name) => {
            let Some(variable) = variables.get(name) else {
                return Err(diagnostic(format!("unknown boolean local `{name}`")));
            };
            let value = builder.use_var(*variable);
            Ok(builder.ins().icmp_imm(IntCC::NotEqual, value, 0))
        }
        IrBoolExpr::Field { base, field } => {
            let base_value = use_local(builder, variables, base, "struct")?;
            let field = field_layout(backend_context, local_types, base, field)?;
            let value = load_value(builder, base_value, field.offset, &field.ty);
            Ok(builder.ins().icmp_imm(IntCC::NotEqual, value, 0))
        }
        IrBoolExpr::EnvLoad { offset } => {
            let Some(env) = variables.get("__env") else {
                return Err(diagnostic("missing thread closure environment".to_string()));
            };
            let env = builder.use_var(*env);
            let value = builder
                .ins()
                .load(types::I64, MemFlags::trusted(), env, *offset);
            Ok(builder.ins().icmp_imm(IntCC::NotEqual, value, 0))
        }
        IrBoolExpr::Call { callee, args } => {
            let Some(function_id) = backend_context.function_ids.get(callee) else {
                return Err(diagnostic(format!("unknown function `{callee}`")));
            };
            let local_callee = module.declare_func_in_func(*function_id, builder.func);
            let args = codegen_call_args(
                builder,
                module,
                backend_context,
                local_types,
                variables,
                next_variable,
                callee,
                args,
            )?;
            let call = builder.ins().call(local_callee, &args);
            let value = builder.inst_results(call)[0];
            let ret = backend_context
                .function_sigs
                .get(callee)
                .map(|sig| &sig.ret)
                .unwrap_or(&IrValueTy::Unknown);
            let value = extend_to_i64(builder, value, ret);
            Ok(builder.ins().icmp_imm(IntCC::NotEqual, value, 0))
        }
        IrBoolExpr::Not(expr) => {
            let value = codegen_bool_expr(
                builder,
                module,
                backend_context,
                local_types,
                variables,
                next_variable,
                expr,
            )?;
            Ok(builder.ins().icmp_imm(IntCC::Equal, value, 0))
        }
        IrBoolExpr::And(left, right) => {
            let left = codegen_bool_as_int(
                builder,
                module,
                backend_context,
                local_types,
                variables,
                next_variable,
                left,
            )?;
            let right = codegen_bool_as_int(
                builder,
                module,
                backend_context,
                local_types,
                variables,
                next_variable,
                right,
            )?;
            let value = builder.ins().band(left, right);
            Ok(builder.ins().icmp_imm(IntCC::NotEqual, value, 0))
        }
        IrBoolExpr::Or(left, right) => {
            let left = codegen_bool_as_int(
                builder,
                module,
                backend_context,
                local_types,
                variables,
                next_variable,
                left,
            )?;
            let right = codegen_bool_as_int(
                builder,
                module,
                backend_context,
                local_types,
                variables,
                next_variable,
                right,
            )?;
            let value = builder.ins().bor(left, right);
            Ok(builder.ins().icmp_imm(IntCC::NotEqual, value, 0))
        }
        IrBoolExpr::Compare { op, left, right } => {
            let left = codegen_int_expr(
                builder,
                module,
                backend_context,
                local_types,
                variables,
                next_variable,
                left,
            )?;
            let right = codegen_int_expr(
                builder,
                module,
                backend_context,
                local_types,
                variables,
                next_variable,
                right,
            )?;
            Ok(builder.ins().icmp(compare_op(*op), left, right))
        }
        IrBoolExpr::FloatCompare { op, left, right } => {
            let left = codegen_float_expr(
                builder,
                module,
                backend_context,
                local_types,
                variables,
                next_variable,
                left,
            )?;
            let right = codegen_float_expr(
                builder,
                module,
                backend_context,
                local_types,
                variables,
                next_variable,
                right,
            )?;
            Ok(builder.ins().fcmp(float_compare_op(*op), left, right))
        }
        IrBoolExpr::BoolCompare { op, left, right } => codegen_bool_compare(
            builder,
            module,
            backend_context,
            local_types,
            variables,
            next_variable,
            *op,
            left,
            right,
        ),
        IrBoolExpr::StringCompare { op, left, right } => codegen_string_compare(
            builder,
            module,
            backend_context,
            local_types,
            variables,
            next_variable,
            *op,
            left,
            right,
        ),
    }
}

fn codegen_bool_compare(
    builder: &mut FunctionBuilder,
    module: &mut ObjectModule,
    backend_context: &BackendContext,
    local_types: &HashMap<String, IrValueTy>,
    variables: &mut HashMap<String, Variable>,
    next_variable: &mut usize,
    op: IrCompareOp,
    left: &IrBoolExpr,
    right: &IrBoolExpr,
) -> Result<cranelift_codegen::ir::Value, Vec<Diagnostic>> {
    let left = codegen_bool_expr(
        builder,
        module,
        backend_context,
        local_types,
        variables,
        next_variable,
        left,
    )?;
    let right = codegen_bool_expr(
        builder,
        module,
        backend_context,
        local_types,
        variables,
        next_variable,
        right,
    )?;
    let one = builder.ins().iconst(types::I64, 1);
    let zero = builder.ins().iconst(types::I64, 0);
    let left = builder.ins().select(left, one, zero);
    let one = builder.ins().iconst(types::I64, 1);
    let zero = builder.ins().iconst(types::I64, 0);
    let right = builder.ins().select(right, one, zero);
    match op {
        IrCompareOp::Eq | IrCompareOp::NotEq => Ok(builder.ins().icmp(compare_op(op), left, right)),
        IrCompareOp::Lt | IrCompareOp::Le | IrCompareOp::Gt | IrCompareOp::Ge => Err(diagnostic(
            "ordered boolean comparisons are not supported".to_string(),
        )),
    }
}

fn codegen_bool_as_int(
    builder: &mut FunctionBuilder,
    module: &mut ObjectModule,
    backend_context: &BackendContext,
    local_types: &HashMap<String, IrValueTy>,
    variables: &mut HashMap<String, Variable>,
    next_variable: &mut usize,
    expr: &IrBoolExpr,
) -> Result<cranelift_codegen::ir::Value, Vec<Diagnostic>> {
    let condition = codegen_bool_expr(
        builder,
        module,
        backend_context,
        local_types,
        variables,
        next_variable,
        expr,
    )?;
    let one = builder.ins().iconst(types::I64, 1);
    let zero = builder.ins().iconst(types::I64, 0);
    Ok(builder.ins().select(condition, one, zero))
}

fn compare_op(op: IrCompareOp) -> IntCC {
    match op {
        IrCompareOp::Eq => IntCC::Equal,
        IrCompareOp::NotEq => IntCC::NotEqual,
        IrCompareOp::Lt => IntCC::SignedLessThan,
        IrCompareOp::Le => IntCC::SignedLessThanOrEqual,
        IrCompareOp::Gt => IntCC::SignedGreaterThan,
        IrCompareOp::Ge => IntCC::SignedGreaterThanOrEqual,
    }
}

fn float_compare_op(op: IrCompareOp) -> FloatCC {
    match op {
        IrCompareOp::Eq => FloatCC::Equal,
        IrCompareOp::NotEq => FloatCC::NotEqual,
        IrCompareOp::Lt => FloatCC::LessThan,
        IrCompareOp::Le => FloatCC::LessThanOrEqual,
        IrCompareOp::Gt => FloatCC::GreaterThan,
        IrCompareOp::Ge => FloatCC::GreaterThanOrEqual,
    }
}

fn codegen_print_string(
    builder: &mut FunctionBuilder,
    module: &mut ObjectModule,
    backend_context: &BackendContext,
    value: &str,
) -> Result<(), Vec<Diagnostic>> {
    let address = string_address(builder, module, backend_context, value)?;
    let puts = module.declare_func_in_func(backend_context.puts_id, builder.func);
    builder.ins().call(puts, &[address]);
    Ok(())
}

fn codegen_print_string_expr(
    builder: &mut FunctionBuilder,
    module: &mut ObjectModule,
    backend_context: &BackendContext,
    local_types: &HashMap<String, IrValueTy>,
    variables: &mut HashMap<String, Variable>,
    next_variable: &mut usize,
    value: &IrStringExpr,
) -> Result<(), Vec<Diagnostic>> {
    let address = codegen_string_expr(
        builder,
        module,
        backend_context,
        local_types,
        variables,
        next_variable,
        value,
    )?;
    let puts = module.declare_func_in_func(backend_context.puts_id, builder.func);
    builder.ins().call(puts, &[address]);
    Ok(())
}

fn codegen_raw_write(
    builder: &mut FunctionBuilder,
    module: &mut ObjectModule,
    backend_context: &BackendContext,
    local_types: &HashMap<String, IrValueTy>,
    variables: &mut HashMap<String, Variable>,
    next_variable: &mut usize,
    fd: &IrIntExpr,
    text: &IrStringExpr,
) -> Result<cranelift_codegen::ir::Value, Vec<Diagnostic>> {
    let fd = codegen_int_expr(
        builder,
        module,
        backend_context,
        local_types,
        variables,
        next_variable,
        fd,
    )?;
    let text = codegen_string_expr(
        builder,
        module,
        backend_context,
        local_types,
        variables,
        next_variable,
        text,
    )?;
    let strlen = module.declare_func_in_func(backend_context.strlen_id, builder.func);
    let len_call = builder.ins().call(strlen, &[text]);
    let len = builder.inst_results(len_call)[0];
    let write = module.declare_func_in_func(backend_context.write_id, builder.func);
    let write_call = builder.ins().call(write, &[fd, text, len]);
    Ok(builder.inst_results(write_call)[0])
}

fn codegen_raw_store8(
    builder: &mut FunctionBuilder,
    module: &mut ObjectModule,
    backend_context: &BackendContext,
    local_types: &HashMap<String, IrValueTy>,
    variables: &mut HashMap<String, Variable>,
    next_variable: &mut usize,
    ptr: &IrIntExpr,
    offset: &IrIntExpr,
    value: &IrIntExpr,
) -> Result<(), Vec<Diagnostic>> {
    let ptr = codegen_int_expr(
        builder,
        module,
        backend_context,
        local_types,
        variables,
        next_variable,
        ptr,
    )?;
    let offset = codegen_int_expr(
        builder,
        module,
        backend_context,
        local_types,
        variables,
        next_variable,
        offset,
    )?;
    let address = builder.ins().iadd(ptr, offset);
    let value = codegen_int_expr(
        builder,
        module,
        backend_context,
        local_types,
        variables,
        next_variable,
        value,
    )?;
    let value = builder.ins().ireduce(types::I8, value);
    builder.ins().store(MemFlags::new(), value, address, 0);
    Ok(())
}

fn codegen_raw_store64(
    builder: &mut FunctionBuilder,
    module: &mut ObjectModule,
    backend_context: &BackendContext,
    local_types: &HashMap<String, IrValueTy>,
    variables: &mut HashMap<String, Variable>,
    next_variable: &mut usize,
    ptr: &IrIntExpr,
    offset: &IrIntExpr,
    value: &IrIntExpr,
) -> Result<(), Vec<Diagnostic>> {
    let ptr = codegen_int_expr(
        builder,
        module,
        backend_context,
        local_types,
        variables,
        next_variable,
        ptr,
    )?;
    let offset = codegen_int_expr(
        builder,
        module,
        backend_context,
        local_types,
        variables,
        next_variable,
        offset,
    )?;
    let address = builder.ins().iadd(ptr, offset);
    let value = codegen_int_expr(
        builder,
        module,
        backend_context,
        local_types,
        variables,
        next_variable,
        value,
    )?;
    builder.ins().store(MemFlags::new(), value, address, 0);
    Ok(())
}

fn codegen_raw_string_store8(
    builder: &mut FunctionBuilder,
    module: &mut ObjectModule,
    backend_context: &BackendContext,
    local_types: &HashMap<String, IrValueTy>,
    variables: &mut HashMap<String, Variable>,
    next_variable: &mut usize,
    value: &IrStringExpr,
    offset: &IrIntExpr,
    byte: &IrIntExpr,
) -> Result<(), Vec<Diagnostic>> {
    let value = codegen_string_expr(
        builder,
        module,
        backend_context,
        local_types,
        variables,
        next_variable,
        value,
    )?;
    let offset = codegen_int_expr(
        builder,
        module,
        backend_context,
        local_types,
        variables,
        next_variable,
        offset,
    )?;
    let address = builder.ins().iadd(value, offset);
    let byte = codegen_int_expr(
        builder,
        module,
        backend_context,
        local_types,
        variables,
        next_variable,
        byte,
    )?;
    let byte = builder.ins().ireduce(types::I8, byte);
    builder.ins().store(MemFlags::new(), byte, address, 0);
    Ok(())
}

fn codegen_raw_set_nonblocking(
    builder: &mut FunctionBuilder,
    module: &mut ObjectModule,
    backend_context: &BackendContext,
    local_types: &HashMap<String, IrValueTy>,
    variables: &mut HashMap<String, Variable>,
    next_variable: &mut usize,
    fd: &IrIntExpr,
) -> Result<cranelift_codegen::ir::Value, Vec<Diagnostic>> {
    let fd = codegen_int_expr(
        builder,
        module,
        backend_context,
        local_types,
        variables,
        next_variable,
        fd,
    )?;
    let fd = builder.ins().ireduce(types::I32, fd);
    let size = builder.ins().iconst(types::I64, 4);
    let enabled_ptr = codegen_malloc(builder, module, backend_context, size);
    let enabled = builder.ins().iconst(types::I8, 1);
    builder
        .ins()
        .store(MemFlags::new(), enabled, enabled_ptr, 0);
    let zero = builder.ins().iconst(types::I8, 0);
    builder.ins().store(MemFlags::new(), zero, enabled_ptr, 1);
    builder.ins().store(MemFlags::new(), zero, enabled_ptr, 2);
    builder.ins().store(MemFlags::new(), zero, enabled_ptr, 3);
    let request = builder.ins().iconst(
        types::I64,
        backend_context.platform_abi.fionbio_ioctl_request,
    );
    let ioctl = module.declare_func_in_func(backend_context.ioctl_id, builder.func);
    let call = builder.ins().call(ioctl, &[fd, request, enabled_ptr]);
    let result = builder.inst_results(call)[0];
    let result_slot =
        builder.create_sized_stack_slot(StackSlotData::new(StackSlotKind::ExplicitSlot, 4, 0));
    builder.ins().stack_store(result, result_slot, 0);
    codegen_free(builder, module, backend_context, enabled_ptr);
    let result = builder.ins().stack_load(types::I32, result_slot, 0);
    Ok(builder.ins().sextend(types::I64, result))
}

fn codegen_raw_thread_spawn(
    builder: &mut FunctionBuilder,
    module: &mut ObjectModule,
    backend_context: &BackendContext,
    local_types: &HashMap<String, IrValueTy>,
    variables: &mut HashMap<String, Variable>,
    next_variable: &mut usize,
    task: &IrFunctionExpr,
    captures: &[IrValueExpr],
) -> Result<cranelift_codegen::ir::Value, Vec<Diagnostic>> {
    let task = codegen_function_expr(
        builder,
        module,
        backend_context,
        local_types,
        variables,
        next_variable,
        task,
    )?;
    let size = builder.ins().iconst(types::I64, 8);
    let handle_ptr = codegen_malloc(builder, module, backend_context, size);
    let mut arg = task;
    let trampoline_id = if captures.is_empty() {
        backend_context.thread_trampoline_id
    } else {
        let env_size = builder
            .ins()
            .iconst(types::I64, ((captures.len() + 1) * 8) as i64);
        let env = codegen_malloc(builder, module, backend_context, env_size);
        builder.ins().store(MemFlags::new(), task, env, 0);
        for (index, capture) in captures.iter().enumerate() {
            let value = codegen_value_expr(
                builder,
                module,
                backend_context,
                local_types,
                variables,
                next_variable,
                capture,
            )?;
            builder
                .ins()
                .store(MemFlags::new(), value, env, ((index + 1) * 8) as i32);
        }
        arg = env;
        backend_context.thread_env_trampoline_id
    };
    let trampoline = module.declare_func_in_func(trampoline_id, builder.func);
    let trampoline_addr = builder.ins().func_addr(types::I64, trampoline);
    let pthread_create =
        module.declare_func_in_func(backend_context.pthread_create_id, builder.func);
    let zero = builder.ins().iconst(types::I64, 0);
    builder
        .ins()
        .call(pthread_create, &[handle_ptr, zero, trampoline_addr, arg]);
    Ok(handle_ptr)
}

fn codegen_raw_thread_join(
    builder: &mut FunctionBuilder,
    module: &mut ObjectModule,
    backend_context: &BackendContext,
    local_types: &HashMap<String, IrValueTy>,
    variables: &mut HashMap<String, Variable>,
    next_variable: &mut usize,
    handle: &IrIntExpr,
) -> Result<cranelift_codegen::ir::Value, Vec<Diagnostic>> {
    let handle = codegen_int_expr(
        builder,
        module,
        backend_context,
        local_types,
        variables,
        next_variable,
        handle,
    )?;
    let thread = builder.ins().load(types::I64, MemFlags::new(), handle, 0);
    let zero = builder.ins().iconst(types::I64, 0);
    let pthread_join = module.declare_func_in_func(backend_context.pthread_join_id, builder.func);
    let call = builder.ins().call(pthread_join, &[thread, zero]);
    let result = builder.inst_results(call)[0];
    let result_slot =
        builder.create_sized_stack_slot(StackSlotData::new(StackSlotKind::ExplicitSlot, 4, 0));
    builder.ins().stack_store(result, result_slot, 0);
    codegen_free(builder, module, backend_context, handle);
    let result = builder.ins().stack_load(types::I32, result_slot, 0);
    Ok(builder.ins().sextend(types::I64, result))
}

fn codegen_raw_free(
    builder: &mut FunctionBuilder,
    module: &mut ObjectModule,
    backend_context: &BackendContext,
    local_types: &HashMap<String, IrValueTy>,
    variables: &mut HashMap<String, Variable>,
    next_variable: &mut usize,
    ptr: &IrIntExpr,
) -> Result<(), Vec<Diagnostic>> {
    let ptr = codegen_int_expr(
        builder,
        module,
        backend_context,
        local_types,
        variables,
        next_variable,
        ptr,
    )?;
    codegen_free(builder, module, backend_context, ptr);
    Ok(())
}

fn codegen_print_int(
    builder: &mut FunctionBuilder,
    module: &mut ObjectModule,
    backend_context: &BackendContext,
    local_types: &HashMap<String, IrValueTy>,
    variables: &mut HashMap<String, Variable>,
    next_variable: &mut usize,
    value: &IrIntExpr,
) -> Result<(), Vec<Diagnostic>> {
    let value = codegen_int_expr(
        builder,
        module,
        backend_context,
        local_types,
        variables,
        next_variable,
        value,
    )?;
    let ten = builder.ins().iconst(types::I64, 10);
    let ascii_zero = builder.ins().iconst(types::I64, 48);
    let quotient = builder.ins().sdiv(value, ten);
    let remainder = builder.ins().srem(value, ten);
    let has_tens = builder.ins().icmp_imm(IntCC::NotEqual, quotient, 0);
    let tens_block = builder.create_block();
    let ones_block = builder.create_block();

    builder
        .ins()
        .brif(has_tens, tens_block, &[], ones_block, &[]);

    builder.switch_to_block(tens_block);
    let tens_digit = builder.ins().iadd(quotient, ascii_zero);
    call_putchar(builder, module, backend_context, tens_digit);
    builder.ins().jump(ones_block, &[]);

    builder.switch_to_block(ones_block);
    let ones_digit = builder.ins().iadd(remainder, ascii_zero);
    call_putchar(builder, module, backend_context, ones_digit);
    let newline = builder.ins().iconst(types::I64, 10);
    call_putchar(builder, module, backend_context, newline);
    Ok(())
}

fn codegen_drop(
    builder: &mut FunctionBuilder,
    module: &mut ObjectModule,
    backend_context: &BackendContext,
    variables: &HashMap<String, Variable>,
    local: &str,
    ty: &IrValueTy,
) -> Result<(), Vec<Diagnostic>> {
    if !is_heap_owned_ty(ty) {
        return Ok(());
    }
    let value = use_local(builder, variables, local, "owned")?;
    codegen_drop_value(builder, module, backend_context, value, ty)
}

fn codegen_drop_box_storage(
    builder: &mut FunctionBuilder,
    module: &mut ObjectModule,
    backend_context: &BackendContext,
    variables: &HashMap<String, Variable>,
    local: &str,
) -> Result<(), Vec<Diagnostic>> {
    let value = use_local(builder, variables, local, "box")?;
    let ptr = builder.ins().load(types::I64, MemFlags::new(), value, 0);
    codegen_free(builder, module, backend_context, ptr);
    codegen_free(builder, module, backend_context, value);
    Ok(())
}

fn codegen_drop_value(
    builder: &mut FunctionBuilder,
    module: &mut ObjectModule,
    backend_context: &BackendContext,
    value: cranelift_codegen::ir::Value,
    ty: &IrValueTy,
) -> Result<(), Vec<Diagnostic>> {
    match ty {
        IrValueTy::OwnedString => {
            codegen_free(builder, module, backend_context, value);
        }
        IrValueTy::Boxed(payload_ty) => {
            codegen_box_drop_value(builder, module, backend_context, value, payload_ty)?;
            codegen_free(builder, module, backend_context, value);
        }
        IrValueTy::Vec(payload_ty) => {
            if !codegen_vec_resource_drop(builder, module, backend_context, value, payload_ty) {
                codegen_vec_drop_value(builder, module, backend_context, value, payload_ty)?;
            }
            codegen_free(builder, module, backend_context, value);
        }
        IrValueTy::Struct(name) => {
            if codegen_user_drop_impl(builder, module, backend_context, value, name) {
                codegen_free(builder, module, backend_context, value);
                return Ok(());
            }
            if codegen_named_resource_drop(builder, module, backend_context, value, name) {
                codegen_free(builder, module, backend_context, value);
                return Ok(());
            }
            let Some(layout) = backend_context.struct_layouts.get(name).cloned() else {
                return Err(diagnostic(format!("unknown struct `{name}` for drop")));
            };
            for field in layout.fields.values() {
                if let Some(drop_ty) = nested_owned_drop_ty(&field.ty) {
                    let field_value =
                        builder
                            .ins()
                            .load(types::I64, MemFlags::new(), value, field.offset);
                    codegen_drop_value(builder, module, backend_context, field_value, &drop_ty)?;
                }
            }
            codegen_free(builder, module, backend_context, value);
        }
        IrValueTy::Enum(name) => {
            let Some(layout) = backend_context.enum_layouts.get(name).cloned() else {
                return Err(diagnostic(format!("unknown enum `{name}` for drop")));
            };
            let tag = builder.ins().load(types::I64, MemFlags::new(), value, 0);
            let done_block = builder.create_block();
            let fallback_block = builder.create_block();
            let mut next_test_block = None;
            let variants = layout.variants.values().cloned().collect::<Vec<_>>();

            for (variant_index, variant) in variants.iter().enumerate() {
                let arm_block = builder.create_block();
                let test_block = next_test_block.take().unwrap_or_else(|| {
                    if variant_index == 0 {
                        builder.current_block().expect("current block")
                    } else {
                        builder.create_block()
                    }
                });
                if variant_index > 0 {
                    builder.switch_to_block(test_block);
                }
                let else_block = if variant_index + 1 == variants.len() {
                    fallback_block
                } else {
                    let block = builder.create_block();
                    next_test_block = Some(block);
                    block
                };
                let expected_tag = builder.ins().iconst(types::I64, variant.tag);
                let matches = builder.ins().icmp(IntCC::Equal, tag, expected_tag);
                builder.ins().brif(matches, arm_block, &[], else_block, &[]);

                builder.switch_to_block(arm_block);
                let (payload_offsets, _) = enum_payload_offsets(&variant.payload_tys);
                for (payload_index, payload_ty) in variant.payload_tys.iter().enumerate() {
                    if let Some(drop_ty) = nested_owned_drop_ty(payload_ty) {
                        let payload =
                            load_value(builder, value, payload_offsets[payload_index], payload_ty);
                        codegen_drop_value(builder, module, backend_context, payload, &drop_ty)?;
                    }
                }
                builder.ins().jump(done_block, &[]);
            }

            builder.switch_to_block(fallback_block);
            builder.ins().jump(done_block, &[]);
            builder.switch_to_block(done_block);
            codegen_free(builder, module, backend_context, value);
        }
        _ => {}
    }
    Ok(())
}

fn nested_owned_drop_ty(ty: &IrValueTy) -> Option<IrValueTy> {
    if is_generic_placeholder_ty(ty) {
        return None;
    }
    if matches!(ty, IrValueTy::String) {
        return Some(IrValueTy::OwnedString);
    }
    is_heap_owned_ty(ty).then(|| ty.clone())
}

fn is_generic_placeholder_ty(ty: &IrValueTy) -> bool {
    matches!(ty, IrValueTy::Struct(name) if name.len() == 1 && name.chars().all(|ch| ch.is_ascii_uppercase()))
}

fn codegen_box_drop_value(
    builder: &mut FunctionBuilder,
    module: &mut ObjectModule,
    backend_context: &BackendContext,
    value: cranelift_codegen::ir::Value,
    payload_ty: &IrValueTy,
) -> Result<(), Vec<Diagnostic>> {
    let ptr = builder.ins().load(types::I64, MemFlags::new(), value, 0);
    if is_heap_owned_ty(payload_ty) || matches!(payload_ty, IrValueTy::String) {
        let payload = builder.ins().load(types::I64, MemFlags::new(), ptr, 0);
        let drop_ty = if matches!(payload_ty, IrValueTy::String) {
            IrValueTy::OwnedString
        } else {
            payload_ty.clone()
        };
        codegen_drop_value(builder, module, backend_context, payload, &drop_ty)?;
    }
    codegen_free(builder, module, backend_context, ptr);
    Ok(())
}

fn codegen_vec_drop_value(
    builder: &mut FunctionBuilder,
    module: &mut ObjectModule,
    backend_context: &BackendContext,
    value: cranelift_codegen::ir::Value,
    payload_ty: &IrValueTy,
) -> Result<(), Vec<Diagnostic>> {
    let data = builder.ins().load(types::I64, MemFlags::new(), value, 0);
    let length = builder.ins().load(types::I64, MemFlags::new(), value, 8);

    let index_slot =
        builder.create_sized_stack_slot(StackSlotData::new(StackSlotKind::ExplicitSlot, 8, 0));
    let zero = builder.ins().iconst(types::I64, 0);
    builder.ins().stack_store(zero, index_slot, 0);

    let loop_header = builder.create_block();
    let loop_body = builder.create_block();
    let free_data = builder.create_block();
    let done = builder.create_block();

    let has_data = builder.ins().icmp_imm(IntCC::NotEqual, data, 0);
    builder.ins().brif(has_data, loop_header, &[], done, &[]);

    builder.switch_to_block(loop_header);
    let index = builder.ins().stack_load(types::I64, index_slot, 0);
    let has_more = builder.ins().icmp(IntCC::UnsignedLessThan, index, length);
    builder.ins().brif(has_more, loop_body, &[], free_data, &[]);

    builder.switch_to_block(loop_body);
    let index = builder.ins().stack_load(types::I64, index_slot, 0);
    let offset = builder.ins().imul_imm(index, 8);
    let address = builder.ins().iadd(data, offset);
    let element = builder.ins().load(types::I64, MemFlags::new(), address, 0);
    let drop_ty = if matches!(payload_ty, IrValueTy::String) {
        IrValueTy::OwnedString
    } else {
        payload_ty.clone()
    };
    if is_heap_owned_ty(&drop_ty) {
        codegen_drop_value(builder, module, backend_context, element, &drop_ty)?;
    }
    let next = builder.ins().iadd_imm(index, 1);
    builder.ins().stack_store(next, index_slot, 0);
    builder.ins().jump(loop_header, &[]);

    builder.switch_to_block(free_data);
    codegen_free(builder, module, backend_context, data);
    builder.ins().jump(done, &[]);

    builder.switch_to_block(done);
    Ok(())
}

fn codegen_vec_resource_drop(
    builder: &mut FunctionBuilder,
    module: &mut ObjectModule,
    backend_context: &BackendContext,
    value: cranelift_codegen::ir::Value,
    payload_ty: &IrValueTy,
) -> bool {
    let function = match payload_ty {
        IrValueTy::String | IrValueTy::OwnedString => "vec__destroy_string",
        IrValueTy::Int(_) => "vec__destroy_int",
        _ => return false,
    };
    let Some(function_id) = backend_context.function_ids.get(function) else {
        return false;
    };
    let local = module.declare_func_in_func(*function_id, builder.func);
    builder.ins().call(local, &[value]);
    true
}

fn codegen_user_drop_impl(
    builder: &mut FunctionBuilder,
    module: &mut ObjectModule,
    backend_context: &BackendContext,
    value: cranelift_codegen::ir::Value,
    name: &str,
) -> bool {
    let Some(function) = backend_context.drop_impls.get(name) else {
        return false;
    };
    let Some(function_id) = backend_context.function_ids.get(function) else {
        return false;
    };
    let local = module.declare_func_in_func(*function_id, builder.func);
    builder.ins().call(local, &[value]);
    true
}

fn codegen_named_resource_drop(
    builder: &mut FunctionBuilder,
    module: &mut ObjectModule,
    backend_context: &BackendContext,
    value: cranelift_codegen::ir::Value,
    name: &str,
) -> bool {
    if !is_unique_resource_name(name) {
        return false;
    }
    let functions: &[&str] = match name {
        "box__Box" => &["box__destroy_int"],
        "buffer__ByteBuffer" => &["buffer__byte_buffer_destroy"],
        "buffer__Buffer" => &["buffer__destroy"],
        "buffer__StringBuilder" => &["buffer__string_builder_destroy"],
        "shared__Shared" => &["shared__destroy_int"],
        "task__TaskQueue4" => &["task__close", "task__join_queue", "task__destroy_queue"],
        "task__TaskQueue4Int" => &[
            "task__close_int",
            "task__join_queue_int",
            "task__destroy_queue_int",
        ],
        "net__TcpListener" => &["net__tcp_listener_close"],
        "net__TcpStream" => &["net__tcp_stream_close"],
        _ => return false,
    };
    for function in functions {
        let Some(function_id) = backend_context.function_ids.get(*function) else {
            return false;
        };
        let local = module.declare_func_in_func(*function_id, builder.func);
        builder.ins().call(local, &[value]);
    }
    true
}

fn is_heap_owned_ty(ty: &IrValueTy) -> bool {
    matches!(
        ty,
        IrValueTy::OwnedString
            | IrValueTy::Boxed(_)
            | IrValueTy::Vec(_)
            | IrValueTy::Struct(_)
            | IrValueTy::Enum(_)
    )
}

fn call_putchar(
    builder: &mut FunctionBuilder,
    module: &mut ObjectModule,
    backend_context: &BackendContext,
    value: cranelift_codegen::ir::Value,
) {
    let value = builder.ins().ireduce(types::I32, value);
    let putchar = module.declare_func_in_func(backend_context.putchar_id, builder.func);
    builder.ins().call(putchar, &[value]);
}

fn diagnostic(message: String) -> Vec<Diagnostic> {
    vec![Diagnostic {
        message,
        location: None,
        code: None,
    }]
}
