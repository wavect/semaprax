//! Experimental RI-05 source-to-native opaque-owner bridge. Pure generation
//! from validated HIR; no tool execution, package publication, or stable ABI.
use super::*;
use semaprax::hir::{
    OwnershipMode, ResolvedImport, ResolvedImportResultKind, ResolvedProgram,
    ResolvedResourceDropKind, ResolvedType, ResolvedTypeDeclarationKind,
};
#[path = "owner_sdk_c.rs"]
mod c;
#[path = "owned_string_sdk.rs"]
mod string;
pub use string::prepare_owned_string_native;

pub struct OpaqueOwnerNative {
    pub header: String,
    pub c_source: String,
    pub rust_adapter: String,
}

pub fn prepare_opaque_owner_native(
    program: &semaprax::ast::Program,
    function_id: &str,
) -> Result<OpaqueOwnerNative, Diagnostic> {
    let resolved = semaprax::hir::resolve(program)
        .map_err(|_| sdk_error("opaque owner source failed HIR resolution"))?;
    semaprax::hir::validate(&resolved)?;
    render(&resolved, function_id)
}

pub(super) fn render(
    program: &ResolvedProgram,
    function_id: &str,
) -> Result<OpaqueOwnerNative, Diagnostic> {
    semaprax::hir::validate(program)?;
    let imports = program
        .interfaces
        .iter()
        .flat_map(|interface| &interface.imports)
        .collect::<Vec<_>>();
    let constructor = imports
        .iter()
        .find(|import| {
            matches!(
                import.result.kind,
                ResolvedImportResultKind::OwnedResource { .. }
            )
        })
        .ok_or_else(|| sdk_error("opaque owner requires one Rust constructor"))?;
    let ResolvedImportResultKind::OwnedResource { resource } = &constructor.result.kind else {
        unreachable!()
    };
    let resource_type = ResolvedType::Nominal {
        declaration: resource.clone(),
        arguments: Vec::new(),
    };
    let method = imports
        .iter()
        .find(|import| {
            import.native_rust
                && import
                    .parameters
                    .first()
                    .is_some_and(|p| p.ownership == OwnershipMode::Own && p.ty == resource_type)
        })
        .ok_or_else(|| sdk_error("opaque owner requires one consuming Rust method"))?;
    let declaration = program
        .types
        .iter()
        .find(|declaration| &declaration.id == resource)
        .ok_or_else(|| sdk_error("opaque owner resource is missing"))?;
    let ResolvedTypeDeclarationKind::Resource { drop } = &declaration.kind else {
        return Err(sdk_error("opaque owner declaration is not a resource"));
    };
    let ResolvedResourceDropKind::Imported {
        import: finalizer, ..
    } = &drop.kind
    else {
        return Err(sdk_error(
            "opaque Rust owner requires an imported destructor",
        ));
    };
    let finalizer = imports
        .iter()
        .find(|import| &import.id == finalizer)
        .ok_or_else(|| sdk_error("opaque owner destructor is missing"))?;
    if imports.len() != 3
        || !constructor.native_rust
        || constructor.parameters.len() != 1
        || constructor.parameters[0].ty != ResolvedType::I64
        || constructor.parameters[0].ownership != OwnershipMode::Value
        || constructor.result.ownership != OwnershipMode::Own
        || method.parameters.len() != 2
        || method.parameters[1].ty != ResolvedType::I64
        || method.parameters[1].ownership != OwnershipMode::Value
        || method.result.kind != ResolvedImportResultKind::Bool
        || finalizer.native_rust
        || !matches!(
            constructor.failure,
            semaprax::hir::ResolvedImportFailure::Infallible
        )
        || !matches!(
            method.failure,
            semaprax::hir::ResolvedImportFailure::Infallible
        )
        || !constructor.effects.is_empty()
        || !method.effects.is_empty()
        || !finalizer.effects.is_empty()
    {
        return Err(sdk_error(
            "opaque owner signature is outside the experimental profile",
        ));
    }
    let constructor_path = rust_path(constructor)?;
    let type_path = constructor_path
        .rsplit_once("::")
        .map(|(ty, _)| ty)
        .ok_or_else(|| sdk_error("opaque owner constructor type is missing"))?;
    let method_path = rust_path(method)?;
    if method_path.rsplit_once("::").map(|(ty, _)| ty) != Some(type_path) {
        return Err(sdk_error("opaque owner method has a different Rust type"));
    }
    let function = program
        .functions
        .iter()
        .find(|function| function.id.as_str() == function_id)
        .ok_or_else(|| sdk_error("opaque owner function is missing"))?;
    let header = "#ifndef SPX_OWNER_V1_H\n#define SPX_OWNER_V1_H\n#include <stdint.h>\ntypedef struct { uint64_t context, generation, slot; } spx_owner;\nint32_t spx_owner_new(uint64_t, int64_t, spx_owner*);\nint32_t spx_owner_consume(uint64_t, spx_owner, int64_t, uint8_t*);\nint32_t spx_owner_validate(uint64_t, spx_owner);\nint32_t spx_owner_drop(uint64_t, spx_owner);\n#endif\n".to_owned();
    let c_source = c::render_program(
        program,
        function,
        &constructor.id,
        &method.id,
        &drop.id,
        &resource_type,
    )?;
    let mut rust_adapter = include_str!("owner_runtime.rs.txt")
        .replace("@TYPE@", type_path)
        .replace("@CONSTRUCTOR@", constructor_path)
        .replace("@METHOD@", method_path);
    let params = (0..function.params.len())
        .map(|index| format!(", arg_{index}:i64"))
        .collect::<String>();
    let args = (0..function.params.len())
        .map(|index| format!(", arg_{index}"))
        .collect::<String>();
    let public_params = params.strip_prefix(", ").unwrap_or("");
    let (result_type, result_value) = if function.return_type == ResolvedType::Bool {
        ("bool", "output != 0")
    } else {
        ("i64", "output")
    };
    rust_adapter.push_str(&format!("\nunsafe extern \"C\" {{ fn spx_owner_entry(context:u64{params}, out:*mut i64)->i32; }}\npub fn spx_owner_call({public_params})->Result<{result_type},i32>{{let context=spx_owner_context_new();if context==0{{return Err(4)}}let mut output=0;let status=unsafe{{spx_owner_entry(context{args},&mut output)}};let closed=spx_owner_context_close(context);if status!=0{{Err(status)}}else if closed!=0{{Err(closed)}}else{{Ok({result_value})}}}}\n"));
    Ok(OpaqueOwnerNative {
        header,
        c_source,
        rust_adapter,
    })
}

fn rust_path(import: &ResolvedImport) -> Result<&str, Diagnostic> {
    let path = import
        .rust_path
        .as_deref()
        .ok_or_else(|| sdk_error("opaque owner import has no Rust path"))?;
    semaprax::native_rust_binding::rust_api_path_tokens(path)
        .filter(|tokens| tokens == path)
        .map(|_| path)
        .ok_or_else(|| sdk_error("opaque owner Rust path is invalid"))
}

#[path = "owned_container_sdk.rs"]
mod container;
pub use container::prepare_owned_container_native;
