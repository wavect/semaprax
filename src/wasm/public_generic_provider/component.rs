//! Private Component Model bridge for the bounded public-generic provider.
//!
//! The bridge is generated next to the checked Core provider. Its canonical
//! owned-byte resource and the resource-to-carrier marshaling live inside the
//! Component; no host adapter is imported.

use super::{
    emit_component_core, ComponentProviderCoreV1, BINDING_OFFSET, COMPONENT_INPUT_ENCODE_EXPORT_V1,
    COMPONENT_PROVIDER_LAYOUT, COMPONENT_RESULT_COPY_EXPORT_V1, DESCRIPTOR_OFFSET, EXPORTS,
    MAX_SCRATCH_BYTES, SCRATCH_BASE,
};
use crate::{
    diagnostic::Diagnostic, hir::ResolvedProgram,
    public_generic_abi::compiler_endpoint::AdmittedPublicGenericEndpointV1,
};
use sha2::{Digest, Sha256};
use std::fmt::Write as _;
use wasm_encoder::{
    CodeSection, ComponentBuilder, ComponentExportKind, ComponentValType, ConstExpr, EntityType,
    ExportKind, ExportSection, Function, FunctionSection, GlobalSection, GlobalType, ImportSection,
    Instruction, MemArg, MemArgTarget, MemoryType, Module, ModuleArg, PrimitiveValType,
    TypeSection, ValType,
};

const COMPONENT_DOMAIN: &[u8] = b"semaprax.public-generic-wasm-component.v1.artifact\0";
const COMPONENT_EXPORT: &str = "semaprax:public-generic-component/adapter@0.1.0";
const COMPONENT_RESOURCE: &str = "owned-bytes";
const COMPONENT_CONSTRUCTOR: &str = "[constructor]owned-bytes";
const COMPONENT_READ: &str = "[method]owned-bytes.read";
const COMPONENT_INVOKE: &str = "invoke";

const RESOURCE_SLOT_COUNT: u32 = 64;
const MAX_COMPONENT_LIST_BYTES: u32 = 65_536;
const RESOURCE_SLOT_SIZE: u32 = 4 + MAX_COMPONENT_LIST_BYTES;
const RESOURCE_ARENA_BASE: u32 = COMPONENT_PROVIDER_LAYOUT.workspace_end;
const RESOURCE_TEMP_BASE: u32 = RESOURCE_ARENA_BASE + RESOURCE_SLOT_COUNT * RESOURCE_SLOT_SIZE;
const RESOURCE_OUTPUT_BYTES: u32 = 2 * MAX_COMPONENT_LIST_BYTES;
const RESOURCE_READ_RETURN_RECORD: u32 = RESOURCE_TEMP_BASE + RESOURCE_OUTPUT_BYTES;
const RESOURCE_INVOKE_RETURN_RECORD: u32 = RESOURCE_READ_RETURN_RECORD + 8;
const REALLOC_BASE: u32 = RESOURCE_TEMP_BASE + RESOURCE_OUTPUT_BYTES + 64 * 1024;
const REALLOC_LIMIT: u32 = REALLOC_BASE + MAX_COMPONENT_LIST_BYTES;
const RESOURCE_MEMORY_PAGES: u32 = REALLOC_LIMIT.div_ceil(65_536);
pub(super) const COMPONENT_MEMORY_PAGES: u32 = RESOURCE_MEMORY_PAGES;
const COMPONENT_MEMORY_LIMIT: u32 = RESOURCE_MEMORY_PAGES * 65_536;
const _: () = assert!(RESOURCE_ARENA_BASE % 65_536 == 0);
const _: () = assert!(RESOURCE_ARENA_BASE >= SCRATCH_BASE + MAX_SCRATCH_BYTES * 2);
const _: () =
    assert!(RESOURCE_TEMP_BASE == RESOURCE_ARENA_BASE + RESOURCE_SLOT_COUNT * RESOURCE_SLOT_SIZE);
const _: () = assert!(RESOURCE_TEMP_BASE + RESOURCE_OUTPUT_BYTES <= RESOURCE_READ_RETURN_RECORD);
const _: () = assert!(RESOURCE_READ_RETURN_RECORD + 8 <= RESOURCE_INVOKE_RETURN_RECORD);
const _: () = assert!(RESOURCE_INVOKE_RETURN_RECORD + 12 <= REALLOC_BASE);
const _: () = assert!(REALLOC_BASE < REALLOC_LIMIT);
const _: () = assert!(REALLOC_LIMIT <= COMPONENT_MEMORY_LIMIT);
const _: () = assert!(COMPONENT_MEMORY_LIMIT - 65_536 < REALLOC_LIMIT);

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PublicGenericWasmComponentArtifactV1 {
    bytes: Vec<u8>,
    digest: String,
    descriptor_digest: String,
    provider_digest: String,
}

impl PublicGenericWasmComponentArtifactV1 {
    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }

    pub fn digest(&self) -> &str {
        &self.digest
    }

    pub fn descriptor_digest(&self) -> &str {
        &self.descriptor_digest
    }

    pub fn provider_digest(&self) -> &str {
        &self.provider_digest
    }
}

pub(crate) fn replay(
    program: &ResolvedProgram,
    endpoint: &AdmittedPublicGenericEndpointV1,
    candidate_bytes: &[u8],
    candidate_digest: &str,
    candidate_descriptor_digest: &str,
    candidate_provider_digest: &str,
) -> Result<PublicGenericWasmComponentArtifactV1, Diagnostic> {
    validate_component(candidate_bytes)?;
    let expected = emit(program, endpoint)?;
    if candidate_bytes != expected.bytes.as_slice()
        || candidate_digest != expected.digest.as_str()
        || candidate_descriptor_digest != expected.descriptor_digest.as_str()
        || candidate_provider_digest != expected.provider_digest.as_str()
    {
        return Err(super::error(
            "public-generic Wasm Component replay does not match the retained endpoint",
        ));
    }
    Ok(expected)
}

pub(crate) fn emit(
    program: &ResolvedProgram,
    endpoint: &AdmittedPublicGenericEndpointV1,
) -> Result<PublicGenericWasmComponentArtifactV1, Diagnostic> {
    let provider = emit_component_core(program, endpoint)?;
    let provider_digest = sha256_text(&provider.wasm);
    let descriptor_digest = endpoint.descriptor().descriptor_digest().to_owned();
    let component = compose(&provider)?;
    validate_component(&component)?;
    let digest = artifact_digest(endpoint.descriptor_bytes(), &provider.wasm, &component);
    Ok(PublicGenericWasmComponentArtifactV1 {
        bytes: component,
        digest,
        descriptor_digest,
        provider_digest,
    })
}

fn compose(provider: &ComponentProviderCoreV1) -> Result<Vec<u8>, Diagnostic> {
    let bridge = bridge_module(provider)?;
    let dtor = resource_dtor_module();
    let mut component = ComponentBuilder::default();
    let mut outer = ComponentBuilder::default();

    let provider_module = component.core_module_raw(Some("checked-provider"), &provider.wasm);
    let provider_instance =
        component.core_instantiate(Some("checked-provider"), provider_module, []);
    let provider_memory = component.core_alias_export(
        Some("provider-memory"),
        provider_instance,
        "memory",
        ExportKind::Memory,
    );
    let provider_memory_instance = component.core_instantiate_exports(
        Some("provider-memory"),
        [("memory", ExportKind::Memory, provider_memory)],
    );

    let dtor_module = component.core_module(Some("owned-bytes-dtor"), &dtor);
    let dtor_instance = component.core_instantiate(
        Some("owned-bytes-dtor"),
        dtor_module,
        [("env", ModuleArg::Instance(provider_memory_instance))],
    );
    let dtor = component.core_alias_export(
        Some("owned-bytes-dtor-fn"),
        dtor_instance,
        "owned_bytes_dtor",
        ExportKind::Func,
    );
    let resource_type = component.type_resource(Some(COMPONENT_RESOURCE), ValType::I32, Some(dtor));
    let exported_resource = component.export(
        COMPONENT_RESOURCE,
        ComponentExportKind::Type,
        resource_type,
        None,
    );

    let (failure_type_index, failure_type) = component.type_defined(Some("failure"));
    failure_type.enum_type(["contract-violation", "resource-or-provider-refusal"]);
    let exported_failure = component.export(
        "failure",
        ComponentExportKind::Type,
        failure_type_index,
        None,
    );

    let resource_new = component.resource_new(resource_type);
    let resource_drop = component.resource_drop(resource_type);
    let resource_rep = component.resource_rep(resource_type);
    let canonical_instance = component.core_instantiate_exports(
        Some("canonical-resources"),
        [
            ("owned_bytes_new", ExportKind::Func, resource_new),
            ("owned_bytes_drop", ExportKind::Func, resource_drop),
            ("owned_bytes_rep", ExportKind::Func, resource_rep),
        ],
    );

    let provider_exports =
        provider_core_exports(&mut component, provider_instance, provider_memory)?;
    let provider_instance =
        component.core_instantiate_exports(Some("provider-operations"), provider_exports);
    let bridge_module = component.core_module(Some("descriptor-bound-bridge"), &bridge);
    let bridge_instance = component.core_instantiate(
        Some("descriptor-bound-bridge"),
        bridge_module,
        [
            ("provider", ModuleArg::Instance(provider_instance)),
            ("canonical", ModuleArg::Instance(canonical_instance)),
        ],
    );

    let bridge_memory = component.core_alias_export(
        Some("bridge-memory"),
        bridge_instance,
        "memory",
        ExportKind::Memory,
    );
    let bridge_realloc = component.core_alias_export(
        Some("bridge-realloc"),
        bridge_instance,
        "cabi_realloc",
        ExportKind::Func,
    );
    let constructor_core = component.core_alias_export(
        Some("owned-bytes-constructor"),
        bridge_instance,
        "owned_bytes_new",
        ExportKind::Func,
    );
    let read_core = component.core_alias_export(
        Some("owned-bytes-read"),
        bridge_instance,
        "owned_bytes_read",
        ExportKind::Func,
    );
    let invoke_core =
        component.core_alias_export(Some("invoke"), bridge_instance, "invoke", ExportKind::Func);

    let (own_resource_type_index, own_resource_type) =
        component.type_defined(Some("own-owned-bytes"));
    own_resource_type.own(exported_resource);
    let (borrow_resource_type_index, borrow_resource_type) =
        component.type_defined(Some("borrow-owned-bytes"));
    borrow_resource_type.borrow(exported_resource);
    let (list_type_index, list_type) = component.type_defined(Some("byte-list"));
    list_type.list(PrimitiveValType::U8);
    let (tuple_type_index, tuple_type) = component.type_defined(Some("output-pair"));
    tuple_type.tuple([
        ComponentValType::Type(own_resource_type_index),
        ComponentValType::Type(own_resource_type_index),
    ]);
    let (result_type_index, result_type) = component.type_defined(Some("invoke-result"));
    result_type.result(
        Some(ComponentValType::Type(tuple_type_index)),
        Some(ComponentValType::Type(exported_failure)),
    );
    let (constructor_type, mut constructor_encoder) =
        component.type_function(Some("owned-bytes-constructor"));
    constructor_encoder
        .params([("payload", ComponentValType::Type(list_type_index))])
        .result(Some(ComponentValType::Type(own_resource_type_index)));
    let (read_type, mut read_encoder) = component.type_function(Some("owned-bytes-read"));
    read_encoder
        .params([("self", ComponentValType::Type(borrow_resource_type_index))])
        .result(Some(ComponentValType::Type(list_type_index)));
    let (invoke_type, mut invoke_encoder) = component.type_function(Some("invoke"));
    invoke_encoder
        .params([
            ("left", ComponentValType::Type(own_resource_type_index)),
            ("right", ComponentValType::Type(own_resource_type_index)),
        ])
        .result(Some(ComponentValType::Type(result_type_index)));
    let constructor = component.lift_func(
        Some(COMPONENT_CONSTRUCTOR),
        constructor_core,
        constructor_type,
        [
            wasm_encoder::CanonicalOption::Memory(bridge_memory),
            wasm_encoder::CanonicalOption::Realloc(bridge_realloc),
        ],
    );
    let read = component.lift_func(
        Some(COMPONENT_READ),
        read_core,
        read_type,
        [wasm_encoder::CanonicalOption::Memory(bridge_memory)],
    );
    let invoke = component.lift_func(
        Some(COMPONENT_INVOKE),
        invoke_core,
        invoke_type,
        [wasm_encoder::CanonicalOption::Memory(bridge_memory)],
    );
    component.export(
        COMPONENT_CONSTRUCTOR,
        ComponentExportKind::Func,
        constructor,
        None,
    );
    component.export(COMPONENT_READ, ComponentExportKind::Func, read, None);
    component.export(COMPONENT_INVOKE, ComponentExportKind::Func, invoke, None);

    let adapter_component = outer.component(Some("public-generic-adapter"), component);
    let adapter_instance = outer.instantiate(
        Some("public-generic-adapter-instance"),
        adapter_component,
        std::iter::empty::<(&str, ComponentExportKind, u32)>(),
    );
    outer.export(
        COMPONENT_EXPORT,
        ComponentExportKind::Instance,
        adapter_instance,
        None,
    );
    Ok(outer.finish())
}

fn validate_component(bytes: &[u8]) -> Result<(), Diagnostic> {
    wasmparser::Validator::new_with_features(wasmparser::WasmFeatures::all())
        .validate_all(bytes)
        .map(|_| ())
        .map_err(|error| {
            super::error(format!(
                "public-generic Component failed wasmparser validation: {error}"
            ))
        })
}

fn provider_core_exports(
    component: &mut ComponentBuilder,
    instance: u32,
    memory: u32,
) -> Result<Vec<(&'static str, ExportKind, u32)>, Diagnostic> {
    let mut exports = Vec::with_capacity(12);
    exports.push(("memory", ExportKind::Memory, memory));
    for name in EXPORTS.iter().skip(1) {
        let function = component.core_alias_export(Some(name), instance, name, ExportKind::Func);
        exports.push((name, ExportKind::Func, function));
    }
    // The component-only provider variant exports the trusted input-frame
    // encoder and result-frame copier after the stable v1 operation table.
    let input_encode = component.core_alias_export(
        Some(COMPONENT_INPUT_ENCODE_EXPORT_V1),
        instance,
        COMPONENT_INPUT_ENCODE_EXPORT_V1,
        ExportKind::Func,
    );
    let result_copy = component.core_alias_export(
        Some(COMPONENT_RESULT_COPY_EXPORT_V1),
        instance,
        COMPONENT_RESULT_COPY_EXPORT_V1,
        ExportKind::Func,
    );
    exports.push((
        COMPONENT_INPUT_ENCODE_EXPORT_V1,
        ExportKind::Func,
        input_encode,
    ));
    exports.push((
        COMPONENT_RESULT_COPY_EXPORT_V1,
        ExportKind::Func,
        result_copy,
    ));
    Ok(exports)
}

fn resource_dtor_module() -> Module {
    let mut module = Module::new();
    let mut types = TypeSection::new();
    types.ty().function([ValType::I32], []);
    module.section(&types);

    let mut imports = ImportSection::new();
    imports.import(
        "env",
        "memory",
        EntityType::Memory(MemoryType {
            minimum: 1,
            maximum: None,
            memory64: false,
            shared: false,
            page_size_log2: None,
        }),
    );
    module.section(&imports);
    let mut functions = FunctionSection::new();
    functions.function(0);
    module.section(&functions);

    let mut body = Function::new([(3, ValType::I32)]);
    body.instruction(&Instruction::LocalGet(0));
    body.instruction(&Instruction::I32Const(1));
    body.instruction(&Instruction::I32Sub);
    body.instruction(&Instruction::LocalTee(1));
    body.instruction(&Instruction::I32Const(RESOURCE_SLOT_COUNT as i32));
    body.instruction(&Instruction::I32GeU);
    body.instruction(&Instruction::If(wasm_encoder::BlockType::Empty));
    body.instruction(&Instruction::Unreachable);
    body.instruction(&Instruction::End);
    body.instruction(&Instruction::I32Const(RESOURCE_ARENA_BASE as i32));
    body.instruction(&Instruction::LocalGet(1));
    body.instruction(&Instruction::I32Const(RESOURCE_SLOT_SIZE as i32));
    body.instruction(&Instruction::I32Mul);
    body.instruction(&Instruction::I32Add);
    body.instruction(&Instruction::LocalTee(1));
    body.instruction(&Instruction::I32Load(MemArg {
        offset: 0,
        align: 2,
        target: MemArgTarget::Memory(0),
        ordering: None,
    }));
    body.instruction(&Instruction::I32Eqz);
    body.instruction(&Instruction::If(wasm_encoder::BlockType::Empty));
    body.instruction(&Instruction::Unreachable);
    body.instruction(&Instruction::End);
    body.instruction(&Instruction::LocalGet(1));
    body.instruction(&Instruction::I32Load(MemArg {
        offset: 0,
        align: 2,
        target: MemArgTarget::Memory(0),
        ordering: None,
    }));
    body.instruction(&Instruction::I32Const(1));
    body.instruction(&Instruction::I32Sub);
    body.instruction(&Instruction::LocalSet(2));
    body.instruction(&Instruction::I32Const(0));
    body.instruction(&Instruction::LocalSet(3));
    body.instruction(&Instruction::Block(wasm_encoder::BlockType::Empty));
    body.instruction(&Instruction::Loop(wasm_encoder::BlockType::Empty));
    body.instruction(&Instruction::LocalGet(3));
    body.instruction(&Instruction::LocalGet(2));
    body.instruction(&Instruction::I32GeU);
    body.instruction(&Instruction::BrIf(1));
    body.instruction(&Instruction::LocalGet(1));
    body.instruction(&Instruction::I32Const(4));
    body.instruction(&Instruction::I32Add);
    body.instruction(&Instruction::LocalGet(3));
    body.instruction(&Instruction::I32Add);
    body.instruction(&Instruction::I32Const(0));
    body.instruction(&Instruction::I32Store8(MemArg {
        offset: 0,
        align: 0,
        target: MemArgTarget::Memory(0),
        ordering: None,
    }));
    body.instruction(&Instruction::LocalGet(3));
    body.instruction(&Instruction::I32Const(1));
    body.instruction(&Instruction::I32Add);
    body.instruction(&Instruction::LocalSet(3));
    body.instruction(&Instruction::Br(0));
    body.instruction(&Instruction::End);
    body.instruction(&Instruction::End);
    body.instruction(&Instruction::LocalGet(1));
    body.instruction(&Instruction::I32Const(0));
    body.instruction(&Instruction::I32Store(MemArg {
        offset: 0,
        align: 2,
        target: MemArgTarget::Memory(0),
        ordering: None,
    }));
    body.instruction(&Instruction::End);
    let mut exports = ExportSection::new();
    exports.export("owned_bytes_dtor", ExportKind::Func, 0);
    module.section(&exports);

    let mut code = CodeSection::new();
    code.function(&body);
    module.section(&code);
    module
}

fn bridge_module(provider: &ComponentProviderCoreV1) -> Result<Module, Diagnostic> {
    let binding_len = provider.binding.encode().len();
    bridge_core_module(provider.descriptor.len(), binding_len)
}

fn bridge_core_module(descriptor_len: usize, binding_len: usize) -> Result<Module, Diagnostic> {
    if descriptor_len > 64 * 1024 || binding_len > 256 * 1024 {
        return Err(Diagnostic::io(
            "SPX-W122",
            "component bridge descriptor or binding exceeds the authenticated provider bounds",
        ));
    }
    let mut module = Module::new();
    let mut types = TypeSection::new();
    types
        .ty()
        .function([ValType::I32, ValType::I32], [ValType::I32]);
    types.ty().function([ValType::I32], [ValType::I32]);
    types
        .ty()
        .function([ValType::I32, ValType::I32], [ValType::I32]);
    types.ty().function(
        [ValType::I32, ValType::I32, ValType::I32, ValType::I32],
        [ValType::I32],
    );
    types.ty().function([ValType::I32], [ValType::I32]);
    types.ty().function([ValType::I32], []);
    types.ty().function([], [ValType::I32]);
    types.ty().function([ValType::I32], [ValType::I64]);
    types.ty().function(
        [ValType::I32, ValType::I32, ValType::I32, ValType::I32],
        [ValType::I64],
    );
    types
        .ty()
        .function([ValType::I32, ValType::I32, ValType::I32], [ValType::I64]);
    types
        .ty()
        .function([ValType::I32, ValType::I32], [ValType::I64]);
    types.ty().function(
        [
            ValType::I32,
            ValType::I32,
            ValType::I32,
            ValType::I32,
            ValType::I32,
        ],
        [ValType::I64],
    );
    types.ty().function([ValType::I32], [ValType::I32]);
    types.ty().function(
        [
            ValType::I32,
            ValType::I32,
            ValType::I32,
            ValType::I32,
            ValType::I32,
            ValType::I32,
        ],
        [ValType::I32, ValType::I32, ValType::I32],
    );
    module.section(&types);

    let mut imports = ImportSection::new();
    imports.import(
        "provider",
        "memory",
        EntityType::Memory(MemoryType {
            minimum: 1,
            maximum: None,
            memory64: false,
            shared: false,
            page_size_log2: None,
        }),
    );
    for (name, ty) in [
        (EXPORTS[1], 6),
        (EXPORTS[2], 7),
        (EXPORTS[3], 6),
        (EXPORTS[4], 8),
        (EXPORTS[5], 9),
        (EXPORTS[6], 10),
        (EXPORTS[7], 9),
        (EXPORTS[8], 12),
        (EXPORTS[9], 12),
        (EXPORTS[10], 12),
        (COMPONENT_INPUT_ENCODE_EXPORT_V1, 8),
        (COMPONENT_RESULT_COPY_EXPORT_V1, 11),
    ] {
        imports.import("provider", name, EntityType::Function(ty));
    }
    imports.import("canonical", "owned_bytes_new", EntityType::Function(4));
    imports.import("canonical", "owned_bytes_drop", EntityType::Function(5));
    imports.import("canonical", "owned_bytes_rep", EntityType::Function(4));
    module.section(&imports);

    let mut functions = FunctionSection::new();
    for ty in [0, 1, 2, 3, 13] {
        functions.function(ty);
    }
    module.section(&functions);
    let mut globals = GlobalSection::new();
    globals.global(
        GlobalType {
            val_type: ValType::I32,
            mutable: true,
            shared: false,
        },
        &ConstExpr::i32_const(REALLOC_BASE as i32),
    );
    module.section(&globals);

    let mut exports = ExportSection::new();
    exports.export("memory", ExportKind::Memory, 0);
    exports.export("cabi_realloc", ExportKind::Func, 18);
    exports.export("owned_bytes_new", ExportKind::Func, 15);
    exports.export("owned_bytes_read", ExportKind::Func, 16);
    exports.export("invoke", ExportKind::Func, 17);
    module.section(&exports);

    let mut code = CodeSection::new();
    code.function(&owned_bytes_new_body());
    code.function(&owned_bytes_read_body());
    code.function(&invoke_body(descriptor_len, binding_len));
    code.function(&cabi_realloc_body());
    code.function(&cleanup_body());
    module.section(&code);
    Ok(module)
}

fn owned_bytes_new_body() -> Function {
    let mut body = Function::new([(6, ValType::I32)]);
    let (slot, base, cursor, pages, delta, byte) = (2, 3, 4, 5, 6, 7);
    body.instruction(&Instruction::MemorySize(0));
    body.instruction(&Instruction::LocalSet(pages));
    body.instruction(&Instruction::I32Const(RESOURCE_MEMORY_PAGES as i32));
    body.instruction(&Instruction::LocalGet(pages));
    body.instruction(&Instruction::I32Sub);
    body.instruction(&Instruction::LocalTee(delta));
    body.instruction(&Instruction::I32Const(0));
    body.instruction(&Instruction::I32LeS);
    body.instruction(&Instruction::If(wasm_encoder::BlockType::Empty));
    body.instruction(&Instruction::Else);
    body.instruction(&Instruction::LocalGet(delta));
    body.instruction(&Instruction::MemoryGrow(0));
    body.instruction(&Instruction::I32Const(-1));
    body.instruction(&Instruction::I32Eq);
    body.instruction(&Instruction::If(wasm_encoder::BlockType::Empty));
    body.instruction(&Instruction::Unreachable);
    body.instruction(&Instruction::End);
    body.instruction(&Instruction::End);
    body.instruction(&Instruction::LocalGet(1));
    body.instruction(&Instruction::I32Const(MAX_COMPONENT_LIST_BYTES as i32));
    body.instruction(&Instruction::I32GtU);
    body.instruction(&Instruction::If(wasm_encoder::BlockType::Empty));
    body.instruction(&Instruction::Unreachable);
    body.instruction(&Instruction::End);
    body.instruction(&Instruction::I32Const(0));
    body.instruction(&Instruction::LocalSet(slot));
    body.instruction(&Instruction::Block(wasm_encoder::BlockType::Empty));
    body.instruction(&Instruction::Loop(wasm_encoder::BlockType::Empty));
    body.instruction(&Instruction::LocalGet(slot));
    body.instruction(&Instruction::I32Const(RESOURCE_SLOT_COUNT as i32));
    body.instruction(&Instruction::I32GeU);
    body.instruction(&Instruction::If(wasm_encoder::BlockType::Empty));
    body.instruction(&Instruction::Unreachable);
    body.instruction(&Instruction::End);
    body.instruction(&Instruction::I32Const(RESOURCE_ARENA_BASE as i32));
    body.instruction(&Instruction::LocalGet(slot));
    body.instruction(&Instruction::I32Const(RESOURCE_SLOT_SIZE as i32));
    body.instruction(&Instruction::I32Mul);
    body.instruction(&Instruction::I32Add);
    body.instruction(&Instruction::LocalTee(base));
    body.instruction(&Instruction::I32Load(MemArg {
        offset: 0,
        align: 2,
        target: MemArgTarget::Memory(0),
        ordering: None,
    }));
    body.instruction(&Instruction::I32Eqz);
    body.instruction(&Instruction::If(wasm_encoder::BlockType::Empty));
    body.instruction(&Instruction::LocalGet(base));
    body.instruction(&Instruction::LocalGet(1));
    body.instruction(&Instruction::I32Const(1));
    body.instruction(&Instruction::I32Add);
    body.instruction(&Instruction::I32Store(MemArg {
        offset: 0,
        align: 2,
        target: MemArgTarget::Memory(0),
        ordering: None,
    }));
    body.instruction(&Instruction::I32Const(0));
    body.instruction(&Instruction::LocalSet(cursor));
    body.instruction(&Instruction::Block(wasm_encoder::BlockType::Empty));
    body.instruction(&Instruction::Loop(wasm_encoder::BlockType::Empty));
    body.instruction(&Instruction::LocalGet(cursor));
    body.instruction(&Instruction::LocalGet(1));
    body.instruction(&Instruction::I32GeU);
    body.instruction(&Instruction::BrIf(1));
    body.instruction(&Instruction::LocalGet(0));
    body.instruction(&Instruction::LocalGet(cursor));
    body.instruction(&Instruction::I32Add);
    body.instruction(&Instruction::I32Load8U(MemArg {
        offset: 0,
        align: 0,
        target: MemArgTarget::Memory(0),
        ordering: None,
    }));
    body.instruction(&Instruction::LocalSet(byte));
    body.instruction(&Instruction::LocalGet(base));
    body.instruction(&Instruction::I32Const(4));
    body.instruction(&Instruction::I32Add);
    body.instruction(&Instruction::LocalGet(cursor));
    body.instruction(&Instruction::I32Add);
    body.instruction(&Instruction::LocalGet(byte));
    body.instruction(&Instruction::I32Store8(MemArg {
        offset: 0,
        align: 0,
        target: MemArgTarget::Memory(0),
        ordering: None,
    }));
    body.instruction(&Instruction::LocalGet(cursor));
    body.instruction(&Instruction::I32Const(1));
    body.instruction(&Instruction::I32Add);
    body.instruction(&Instruction::LocalSet(cursor));
    body.instruction(&Instruction::Br(0));
    body.instruction(&Instruction::End);
    body.instruction(&Instruction::End);
    // The payload has been copied into the disjoint resource arena. Reclaim
    // the single bounded canonical-list staging cursor for the next call.
    body.instruction(&Instruction::I32Const(REALLOC_BASE as i32));
    body.instruction(&Instruction::GlobalSet(0));
    body.instruction(&Instruction::LocalGet(slot));
    body.instruction(&Instruction::I32Const(1));
    body.instruction(&Instruction::I32Add);
    body.instruction(&Instruction::Call(12));
    body.instruction(&Instruction::Return);
    body.instruction(&Instruction::End);
    body.instruction(&Instruction::LocalGet(slot));
    body.instruction(&Instruction::I32Const(1));
    body.instruction(&Instruction::I32Add);
    body.instruction(&Instruction::LocalSet(slot));
    body.instruction(&Instruction::Br(0));
    body.instruction(&Instruction::End);
    body.instruction(&Instruction::End);
    body.instruction(&Instruction::Unreachable);
    body.instruction(&Instruction::End);
    body
}

fn owned_bytes_read_body() -> Function {
    let mut body = Function::new([(3, ValType::I32)]);
    let rep = 1;
    let base = 2;
    let len = 3;
    body.instruction(&Instruction::LocalGet(0));
    body.instruction(&Instruction::LocalTee(rep));
    body.instruction(&Instruction::I32Eqz);
    body.instruction(&Instruction::If(wasm_encoder::BlockType::Empty));
    body.instruction(&Instruction::Unreachable);
    body.instruction(&Instruction::End);
    body.instruction(&Instruction::LocalGet(rep));
    body.instruction(&Instruction::I32Const(1));
    body.instruction(&Instruction::I32Sub);
    body.instruction(&Instruction::LocalTee(rep));
    body.instruction(&Instruction::I32Const(RESOURCE_SLOT_COUNT as i32));
    body.instruction(&Instruction::I32GeU);
    body.instruction(&Instruction::If(wasm_encoder::BlockType::Empty));
    body.instruction(&Instruction::Unreachable);
    body.instruction(&Instruction::End);
    body.instruction(&Instruction::I32Const(RESOURCE_ARENA_BASE as i32));
    body.instruction(&Instruction::LocalGet(rep));
    body.instruction(&Instruction::I32Const(RESOURCE_SLOT_SIZE as i32));
    body.instruction(&Instruction::I32Mul);
    body.instruction(&Instruction::I32Add);
    body.instruction(&Instruction::LocalTee(base));
    body.instruction(&Instruction::I32Load(MemArg {
        offset: 0,
        align: 2,
        target: MemArgTarget::Memory(0),
        ordering: None,
    }));
    body.instruction(&Instruction::LocalTee(len));
    body.instruction(&Instruction::I32Eqz);
    body.instruction(&Instruction::If(wasm_encoder::BlockType::Empty));
    body.instruction(&Instruction::Unreachable);
    body.instruction(&Instruction::End);
    body.instruction(&Instruction::I32Const(RESOURCE_READ_RETURN_RECORD as i32));
    body.instruction(&Instruction::LocalGet(base));
    body.instruction(&Instruction::I32Const(4));
    body.instruction(&Instruction::I32Add);
    body.instruction(&Instruction::I32Store(MemArg {
        offset: 0,
        align: 2,
        target: MemArgTarget::Memory(0),
        ordering: None,
    }));
    body.instruction(&Instruction::I32Const(RESOURCE_READ_RETURN_RECORD as i32));
    body.instruction(&Instruction::LocalGet(len));
    body.instruction(&Instruction::I32Const(1));
    body.instruction(&Instruction::I32Sub);
    body.instruction(&Instruction::I32Store(MemArg {
        offset: 4,
        align: 2,
        target: MemArgTarget::Memory(0),
        ordering: None,
    }));
    body.instruction(&Instruction::I32Const(RESOURCE_READ_RETURN_RECORD as i32));
    body.instruction(&Instruction::End);
    body
}

fn invoke_body(descriptor_len: usize, binding_len: usize) -> Function {
    let mut body = Function::new([(1, ValType::I64), (19, ValType::I32)]);
    let (provider_id, input_id, result_id, scratch, capacity, frame_len) = (7, 8, 9, 10, 11, 12);
    let (out_ptr_a, out_len_a, out_ptr_b, out_len_b, handle_a, handle_b, status) =
        (13, 14, 15, 16, 17, 18, 19);
    let packed = 2;

    // The component resource identities are converted back to their private
    // bounded byte arenas before the descriptor-derived Carrier frame is
    // encoded. The provider independently authenticates that frame again.
    body.instruction(&Instruction::LocalGet(0));
    body.instruction(&Instruction::Call(14));
    body.instruction(&Instruction::Call(16));
    body.instruction(&Instruction::LocalTee(out_len_a));
    body.instruction(&Instruction::I32Load(MemArg {
        offset: 0,
        align: 2,
        target: MemArgTarget::Memory(0),
        ordering: None,
    }));
    body.instruction(&Instruction::LocalSet(out_ptr_a));
    body.instruction(&Instruction::LocalGet(out_len_a));
    body.instruction(&Instruction::I32Load(MemArg {
        offset: 4,
        align: 2,
        target: MemArgTarget::Memory(0),
        ordering: None,
    }));
    body.instruction(&Instruction::LocalSet(out_len_a));
    body.instruction(&Instruction::LocalGet(1));
    body.instruction(&Instruction::Call(14));
    body.instruction(&Instruction::Call(16));
    body.instruction(&Instruction::LocalTee(out_len_b));
    body.instruction(&Instruction::I32Load(MemArg {
        offset: 0,
        align: 2,
        target: MemArgTarget::Memory(0),
        ordering: None,
    }));
    body.instruction(&Instruction::LocalSet(out_ptr_b));
    body.instruction(&Instruction::LocalGet(out_len_b));
    body.instruction(&Instruction::I32Load(MemArg {
        offset: 4,
        align: 2,
        target: MemArgTarget::Memory(0),
        ordering: None,
    }));
    body.instruction(&Instruction::LocalSet(out_len_b));
    for (offset, local) in [
        (0, out_ptr_a),
        (4, out_len_a),
        (8, out_ptr_b),
        (12, out_len_b),
    ] {
        body.instruction(&Instruction::I32Const(
            (COMPONENT_PROVIDER_LAYOUT.input_leaf_table + offset) as i32,
        ));
        body.instruction(&Instruction::LocalGet(local));
        body.instruction(&Instruction::I32Store(MemArg {
            offset: 0,
            align: 2,
            target: MemArgTarget::Memory(0),
            ordering: None,
        }));
    }

    body.instruction(&Instruction::I32Const(MAX_SCRATCH_BYTES as i32));
    body.instruction(&Instruction::Call(1));
    body.instruction(&Instruction::LocalSet(packed));
    emit_status_failure(&mut body, packed, 1, provider_id, input_id, result_id);
    body.instruction(&Instruction::Call(0));
    body.instruction(&Instruction::LocalSet(scratch));
    body.instruction(&Instruction::Call(2));
    body.instruction(&Instruction::LocalSet(capacity));

    // The provider authenticates both blobs only from its public scratch
    // range. Stage the retained module data there before opening; encoding
    // may overwrite it only after the binding has been checked.
    body.instruction(&Instruction::LocalGet(scratch));
    body.instruction(&Instruction::I32Const(DESCRIPTOR_OFFSET as i32));
    body.instruction(&Instruction::I32Const(descriptor_len as i32));
    body.instruction(&Instruction::MemoryCopy {
        dst_mem: 0,
        src_mem: 0,
    });
    body.instruction(&Instruction::LocalGet(scratch));
    body.instruction(&Instruction::I32Const(descriptor_len as i32));
    body.instruction(&Instruction::I32Add);
    body.instruction(&Instruction::I32Const(BINDING_OFFSET as i32));
    body.instruction(&Instruction::I32Const(binding_len as i32));
    body.instruction(&Instruction::MemoryCopy {
        dst_mem: 0,
        src_mem: 0,
    });

    body.instruction(&Instruction::LocalGet(scratch));
    body.instruction(&Instruction::I32Const(descriptor_len as i32));
    body.instruction(&Instruction::LocalGet(scratch));
    body.instruction(&Instruction::I32Const(descriptor_len as i32));
    body.instruction(&Instruction::I32Add);
    body.instruction(&Instruction::I32Const(binding_len as i32));
    body.instruction(&Instruction::Call(3));
    body.instruction(&Instruction::LocalSet(packed));
    emit_status_failure(&mut body, packed, 1, provider_id, input_id, result_id);
    emit_low_word(&mut body, packed);
    body.instruction(&Instruction::LocalSet(provider_id));

    body.instruction(&Instruction::I32Const(
        COMPONENT_PROVIDER_LAYOUT.input_leaf_table as i32,
    ));
    body.instruction(&Instruction::I32Const(2));
    body.instruction(&Instruction::LocalGet(scratch));
    body.instruction(&Instruction::LocalGet(capacity));
    body.instruction(&Instruction::Call(10));
    body.instruction(&Instruction::LocalSet(packed));
    emit_status_failure(&mut body, packed, 1, provider_id, input_id, result_id);
    emit_low_word(&mut body, packed);
    body.instruction(&Instruction::LocalSet(frame_len));

    body.instruction(&Instruction::LocalGet(provider_id));
    body.instruction(&Instruction::LocalGet(scratch));
    body.instruction(&Instruction::LocalGet(frame_len));
    body.instruction(&Instruction::Call(4));
    body.instruction(&Instruction::LocalSet(packed));
    emit_status_failure(&mut body, packed, 1, provider_id, input_id, result_id);
    emit_low_word(&mut body, packed);
    body.instruction(&Instruction::LocalSet(input_id));

    // Input ownership transfers into the authenticated provider frame as one
    // pair; after preparation the component no longer retains the two input
    // resources, including on a checked contract failure.
    body.instruction(&Instruction::LocalGet(0));
    body.instruction(&Instruction::Call(13));
    body.instruction(&Instruction::LocalGet(1));
    body.instruction(&Instruction::Call(13));
    body.instruction(&Instruction::I32Const(0));
    body.instruction(&Instruction::LocalSet(0));
    body.instruction(&Instruction::I32Const(0));
    body.instruction(&Instruction::LocalSet(1));

    body.instruction(&Instruction::LocalGet(provider_id));
    body.instruction(&Instruction::LocalGet(input_id));
    body.instruction(&Instruction::Call(5));
    body.instruction(&Instruction::LocalSet(packed));
    emit_low_word(&mut body, packed);
    body.instruction(&Instruction::LocalSet(result_id));
    emit_status_to_local(&mut body, packed, status);
    body.instruction(&Instruction::LocalGet(status));
    body.instruction(&Instruction::I32Eqz);
    body.instruction(&Instruction::If(wasm_encoder::BlockType::Empty));
    body.instruction(&Instruction::I32Const(0));
    body.instruction(&Instruction::LocalSet(input_id)); // successful call consumes it
    body.instruction(&Instruction::Else);
    body.instruction(&Instruction::LocalGet(status));
    body.instruction(&Instruction::I32Const(11));
    body.instruction(&Instruction::I32Eq);
    body.instruction(&Instruction::If(wasm_encoder::BlockType::Empty));
    emit_cleanup_return(&mut body, provider_id, input_id, result_id, 0);
    body.instruction(&Instruction::Else);
    emit_cleanup_return(&mut body, provider_id, input_id, result_id, 1);
    body.instruction(&Instruction::End);
    body.instruction(&Instruction::End);

    body.instruction(&Instruction::LocalGet(result_id));
    body.instruction(&Instruction::LocalGet(scratch));
    body.instruction(&Instruction::LocalGet(capacity));
    body.instruction(&Instruction::Call(6));
    body.instruction(&Instruction::LocalSet(packed));
    emit_status_failure(&mut body, packed, 1, provider_id, input_id, result_id);
    emit_low_word(&mut body, packed);
    body.instruction(&Instruction::LocalSet(frame_len));
    body.instruction(&Instruction::LocalGet(scratch));
    body.instruction(&Instruction::LocalGet(frame_len));
    body.instruction(&Instruction::I32Const(RESOURCE_TEMP_BASE as i32));
    body.instruction(&Instruction::I32Const(RESOURCE_OUTPUT_BYTES as i32));
    body.instruction(&Instruction::I32Const(
        COMPONENT_PROVIDER_LAYOUT.result_leaf_table as i32,
    ));
    body.instruction(&Instruction::Call(11));
    body.instruction(&Instruction::LocalSet(packed));
    emit_status_failure(&mut body, packed, 1, provider_id, input_id, result_id);
    emit_low_word(&mut body, packed);
    body.instruction(&Instruction::Drop);

    // Exact helper table rows are written only after the result frame passes
    // the independent descriptor-bound result decoder.
    body.instruction(&Instruction::I32Const(
        COMPONENT_PROVIDER_LAYOUT.result_leaf_table as i32,
    ));
    body.instruction(&Instruction::I32Load(MemArg {
        offset: 0,
        align: 2,
        target: MemArgTarget::Memory(0),
        ordering: None,
    }));
    body.instruction(&Instruction::LocalSet(out_ptr_a));
    body.instruction(&Instruction::I32Const(
        (COMPONENT_PROVIDER_LAYOUT.result_leaf_table + 4) as i32,
    ));
    body.instruction(&Instruction::I32Load(MemArg {
        offset: 0,
        align: 2,
        target: MemArgTarget::Memory(0),
        ordering: None,
    }));
    body.instruction(&Instruction::LocalSet(out_len_a));
    body.instruction(&Instruction::I32Const(
        (COMPONENT_PROVIDER_LAYOUT.result_leaf_table + 8) as i32,
    ));
    body.instruction(&Instruction::I32Load(MemArg {
        offset: 0,
        align: 2,
        target: MemArgTarget::Memory(0),
        ordering: None,
    }));
    body.instruction(&Instruction::LocalSet(out_ptr_b));
    body.instruction(&Instruction::I32Const(
        (COMPONENT_PROVIDER_LAYOUT.result_leaf_table + 12) as i32,
    ));
    body.instruction(&Instruction::I32Load(MemArg {
        offset: 0,
        align: 2,
        target: MemArgTarget::Memory(0),
        ordering: None,
    }));
    body.instruction(&Instruction::LocalSet(out_len_b));
    body.instruction(&Instruction::LocalGet(result_id));
    body.instruction(&Instruction::Call(8));
    body.instruction(&Instruction::I32Eqz);
    body.instruction(&Instruction::If(wasm_encoder::BlockType::Empty));
    body.instruction(&Instruction::Else);
    emit_cleanup_return(&mut body, provider_id, input_id, result_id, 1);
    body.instruction(&Instruction::End);
    body.instruction(&Instruction::I32Const(0));
    body.instruction(&Instruction::LocalSet(result_id));
    body.instruction(&Instruction::LocalGet(provider_id));
    body.instruction(&Instruction::Call(9));
    body.instruction(&Instruction::I32Eqz);
    body.instruction(&Instruction::If(wasm_encoder::BlockType::Empty));
    body.instruction(&Instruction::Else);
    emit_cleanup_return(&mut body, provider_id, input_id, result_id, 1);
    body.instruction(&Instruction::End);
    body.instruction(&Instruction::I32Const(0));
    body.instruction(&Instruction::LocalSet(provider_id));

    body.instruction(&Instruction::LocalGet(out_ptr_a));
    body.instruction(&Instruction::LocalGet(out_len_a));
    body.instruction(&Instruction::Call(15));
    body.instruction(&Instruction::LocalSet(handle_a));
    body.instruction(&Instruction::LocalGet(out_ptr_b));
    body.instruction(&Instruction::LocalGet(out_len_b));
    body.instruction(&Instruction::Call(15));
    body.instruction(&Instruction::LocalSet(handle_b));
    body.instruction(&Instruction::I32Const(RESOURCE_INVOKE_RETURN_RECORD as i32));
    body.instruction(&Instruction::I32Const(0));
    body.instruction(&Instruction::I32Store(MemArg {
        offset: 0,
        align: 2,
        target: MemArgTarget::Memory(0),
        ordering: None,
    }));
    body.instruction(&Instruction::I32Const(
        (RESOURCE_INVOKE_RETURN_RECORD + 4) as i32,
    ));
    body.instruction(&Instruction::LocalGet(handle_a));
    body.instruction(&Instruction::I32Store(MemArg {
        offset: 0,
        align: 2,
        target: MemArgTarget::Memory(0),
        ordering: None,
    }));
    body.instruction(&Instruction::I32Const(
        (RESOURCE_INVOKE_RETURN_RECORD + 8) as i32,
    ));
    body.instruction(&Instruction::LocalGet(handle_b));
    body.instruction(&Instruction::I32Store(MemArg {
        offset: 0,
        align: 2,
        target: MemArgTarget::Memory(0),
        ordering: None,
    }));
    body.instruction(&Instruction::I32Const(RESOURCE_INVOKE_RETURN_RECORD as i32));
    body.instruction(&Instruction::End);
    body
}

fn emit_status_to_local(body: &mut Function, packed: u32, status: u32) {
    body.instruction(&Instruction::LocalGet(packed));
    body.instruction(&Instruction::I64Const(0xffff_ffff));
    body.instruction(&Instruction::I64And);
    body.instruction(&Instruction::I32WrapI64);
    body.instruction(&Instruction::LocalSet(status));
}

fn emit_low_word(body: &mut Function, packed: u32) {
    body.instruction(&Instruction::LocalGet(packed));
    body.instruction(&Instruction::I64Const(32));
    body.instruction(&Instruction::I64ShrU);
    body.instruction(&Instruction::I32WrapI64);
}

fn emit_cleanup_return(body: &mut Function, provider: u32, input: u32, result: u32, error: i32) {
    body.instruction(&Instruction::LocalGet(0));
    body.instruction(&Instruction::LocalGet(1));
    body.instruction(&Instruction::LocalGet(provider));
    body.instruction(&Instruction::LocalGet(input));
    body.instruction(&Instruction::LocalGet(result));
    body.instruction(&Instruction::I32Const(error));
    body.instruction(&Instruction::Call(19));
    body.instruction(&Instruction::LocalSet(19));
    body.instruction(&Instruction::LocalSet(14));
    body.instruction(&Instruction::LocalSet(13));
    for (offset, local) in [(0, 13), (4, 14), (8, 19)] {
        body.instruction(&Instruction::I32Const(
            (RESOURCE_INVOKE_RETURN_RECORD + offset) as i32,
        ));
        body.instruction(&Instruction::LocalGet(local));
        body.instruction(&Instruction::I32Store(MemArg {
            offset: 0,
            align: 2,
            target: MemArgTarget::Memory(0),
            ordering: None,
        }));
    }
    body.instruction(&Instruction::I32Const(RESOURCE_INVOKE_RETURN_RECORD as i32));
    body.instruction(&Instruction::Return);
}

fn emit_status_failure(
    body: &mut Function,
    packed: u32,
    error: i32,
    provider: u32,
    input: u32,
    result: u32,
) {
    emit_status_to_local(body, packed, 19);
    body.instruction(&Instruction::LocalGet(19));
    body.instruction(&Instruction::I32Eqz);
    body.instruction(&Instruction::If(wasm_encoder::BlockType::Empty));
    body.instruction(&Instruction::Else);
    emit_cleanup_return(body, provider, input, result, error);
    body.instruction(&Instruction::End);
}

fn cabi_realloc_body() -> Function {
    let mut body = Function::new([(5, ValType::I32)]);
    let (start, end, copy_len, cursor) = (4, 5, 6, 7);
    body.instruction(&Instruction::LocalGet(3));
    body.instruction(&Instruction::I32Eqz);
    body.instruction(&Instruction::If(wasm_encoder::BlockType::Result(
        ValType::I32,
    )));
    body.instruction(&Instruction::LocalGet(0));
    body.instruction(&Instruction::I32Eqz);
    body.instruction(&Instruction::If(wasm_encoder::BlockType::Empty));
    body.instruction(&Instruction::Else);
    body.instruction(&Instruction::LocalGet(0));
    body.instruction(&Instruction::LocalGet(1));
    body.instruction(&Instruction::I32Add);
    body.instruction(&Instruction::GlobalGet(0));
    body.instruction(&Instruction::I32Eq);
    body.instruction(&Instruction::If(wasm_encoder::BlockType::Empty));
    body.instruction(&Instruction::LocalGet(0));
    body.instruction(&Instruction::GlobalSet(0));
    body.instruction(&Instruction::End);
    body.instruction(&Instruction::End);
    body.instruction(&Instruction::I32Const(0));
    body.instruction(&Instruction::Else);
    body.instruction(&Instruction::GlobalGet(0));
    body.instruction(&Instruction::LocalGet(2));
    body.instruction(&Instruction::I32Const(1));
    body.instruction(&Instruction::I32Sub);
    body.instruction(&Instruction::I32Add);
    body.instruction(&Instruction::LocalGet(2));
    body.instruction(&Instruction::I32Const(1));
    body.instruction(&Instruction::I32Sub);
    body.instruction(&Instruction::I32Const(-1));
    body.instruction(&Instruction::I32Xor);
    body.instruction(&Instruction::I32And);
    body.instruction(&Instruction::LocalTee(start));
    body.instruction(&Instruction::LocalGet(3));
    body.instruction(&Instruction::I32Add);
    body.instruction(&Instruction::LocalTee(end));
    body.instruction(&Instruction::I32Const(REALLOC_LIMIT as i32));
    body.instruction(&Instruction::I32GtU);
    body.instruction(&Instruction::If(wasm_encoder::BlockType::Result(
        ValType::I32,
    )));
    body.instruction(&Instruction::Unreachable);
    body.instruction(&Instruction::Else);
    body.instruction(&Instruction::MemorySize(0));
    body.instruction(&Instruction::I32Const(65_536));
    body.instruction(&Instruction::I32Mul);
    body.instruction(&Instruction::LocalGet(end));
    body.instruction(&Instruction::I32LtU);
    body.instruction(&Instruction::If(wasm_encoder::BlockType::Empty));
    body.instruction(&Instruction::LocalGet(end));
    body.instruction(&Instruction::I32Const(65_535));
    body.instruction(&Instruction::I32Add);
    body.instruction(&Instruction::I32Const(16));
    body.instruction(&Instruction::I32ShrU);
    body.instruction(&Instruction::MemorySize(0));
    body.instruction(&Instruction::I32Sub);
    body.instruction(&Instruction::MemoryGrow(0));
    body.instruction(&Instruction::I32Const(-1));
    body.instruction(&Instruction::I32Eq);
    body.instruction(&Instruction::If(wasm_encoder::BlockType::Empty));
    body.instruction(&Instruction::Unreachable);
    body.instruction(&Instruction::End);
    body.instruction(&Instruction::End);
    body.instruction(&Instruction::LocalGet(0));
    body.instruction(&Instruction::I32Eqz);
    body.instruction(&Instruction::If(wasm_encoder::BlockType::Empty));
    body.instruction(&Instruction::Else);
    body.instruction(&Instruction::LocalGet(1));
    body.instruction(&Instruction::LocalGet(3));
    body.instruction(&Instruction::I32LtU);
    body.instruction(&Instruction::If(wasm_encoder::BlockType::Result(
        ValType::I32,
    )));
    body.instruction(&Instruction::LocalGet(1));
    body.instruction(&Instruction::Else);
    body.instruction(&Instruction::LocalGet(3));
    body.instruction(&Instruction::End);
    body.instruction(&Instruction::LocalSet(copy_len));
    body.instruction(&Instruction::I32Const(0));
    body.instruction(&Instruction::LocalSet(cursor));
    body.instruction(&Instruction::Block(wasm_encoder::BlockType::Empty));
    body.instruction(&Instruction::Loop(wasm_encoder::BlockType::Empty));
    body.instruction(&Instruction::LocalGet(cursor));
    body.instruction(&Instruction::LocalGet(copy_len));
    body.instruction(&Instruction::I32GeU);
    body.instruction(&Instruction::BrIf(1));
    body.instruction(&Instruction::LocalGet(start));
    body.instruction(&Instruction::LocalGet(cursor));
    body.instruction(&Instruction::I32Add);
    body.instruction(&Instruction::LocalGet(0));
    body.instruction(&Instruction::LocalGet(cursor));
    body.instruction(&Instruction::I32Add);
    body.instruction(&Instruction::I32Load8U(MemArg {
        offset: 0,
        align: 0,
        target: MemArgTarget::Memory(0),
        ordering: None,
    }));
    body.instruction(&Instruction::I32Store8(MemArg {
        offset: 0,
        align: 0,
        target: MemArgTarget::Memory(0),
        ordering: None,
    }));
    body.instruction(&Instruction::LocalGet(cursor));
    body.instruction(&Instruction::I32Const(1));
    body.instruction(&Instruction::I32Add);
    body.instruction(&Instruction::LocalSet(cursor));
    body.instruction(&Instruction::Br(0));
    body.instruction(&Instruction::End);
    body.instruction(&Instruction::End);
    body.instruction(&Instruction::End);
    body.instruction(&Instruction::LocalGet(end));
    body.instruction(&Instruction::GlobalSet(0));
    body.instruction(&Instruction::LocalGet(start));
    body.instruction(&Instruction::End);
    body.instruction(&Instruction::End);
    body.instruction(&Instruction::End);
    body
}

fn cleanup_body() -> Function {
    let mut body = Function::new([]);
    for handle in [0, 1] {
        body.instruction(&Instruction::LocalGet(handle));
        body.instruction(&Instruction::I32Eqz);
        body.instruction(&Instruction::If(wasm_encoder::BlockType::Empty));
        body.instruction(&Instruction::Else);
        body.instruction(&Instruction::LocalGet(handle));
        body.instruction(&Instruction::Call(13));
        body.instruction(&Instruction::End);
    }
    for (handle, import) in [(3, 7), (4, 8), (2, 9)] {
        body.instruction(&Instruction::LocalGet(handle));
        body.instruction(&Instruction::I32Eqz);
        body.instruction(&Instruction::If(wasm_encoder::BlockType::Empty));
        body.instruction(&Instruction::Else);
        body.instruction(&Instruction::LocalGet(handle));
        body.instruction(&Instruction::Call(import));
        body.instruction(&Instruction::Drop);
        body.instruction(&Instruction::End);
    }
    body.instruction(&Instruction::I32Const(1));
    body.instruction(&Instruction::LocalGet(5));
    body.instruction(&Instruction::I32Const(0));
    body.instruction(&Instruction::End);
    body
}

fn artifact_digest(descriptor: &[u8], provider: &[u8], component: &[u8]) -> String {
    let mut preimage = Vec::with_capacity(descriptor.len() + provider.len() + component.len() + 64);
    preimage.extend_from_slice(COMPONENT_DOMAIN);
    for bytes in [descriptor, provider, component] {
        preimage.extend_from_slice(&(bytes.len() as u64).to_le_bytes());
        preimage.extend_from_slice(bytes);
    }
    sha256_text(&preimage)
}

fn sha256_text(bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);
    let mut text = String::with_capacity(71);
    text.push_str("sha256:");
    for byte in digest {
        write!(text, "{byte:02x}").expect("writing a fixed digest cannot fail");
    }
    text
}

#[cfg(test)]
mod tests {
    use super::bridge_core_module;

    #[test]
    fn bounded_descriptor_bridge_is_deterministic_and_core_valid() {
        let first = bridge_core_module(64 * 1024, 256 * 1024)
            .expect("exact authenticated provider bounds must be admitted")
            .finish();
        let second = bridge_core_module(64 * 1024, 256 * 1024)
            .expect("same bridge inputs must be admitted")
            .finish();
        assert_eq!(first, second);
        wasmparser::Validator::new_with_features(wasmparser::WasmFeatures::all())
            .validate_all(&first)
            .expect("bounded bridge Core module must validate");
        assert!(bridge_core_module(64 * 1024 + 1, 256 * 1024).is_err());
        assert!(bridge_core_module(64 * 1024, 256 * 1024 + 1).is_err());
    }
}
