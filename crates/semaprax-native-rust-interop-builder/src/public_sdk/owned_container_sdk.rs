//! Additive closed owned-container renderer. No Project publication authority.
use super::*;
use semaprax::hir::DeclarationId;

pub fn prepare_owned_container_native(
    source: &semaprax::ast::Program,
    function_id: &str,
) -> Result<OpaqueOwnerNative, Diagnostic> {
    let program = semaprax::hir::resolve(source)
        .map_err(|_| sdk_error("native container source failed HIR resolution"))?;
    semaprax::hir::validate(&program)?;
    let imports = program
        .interfaces
        .iter()
        .flat_map(|i| &i.imports)
        .collect::<Vec<_>>();
    let constructor = imports
        .iter()
        .find(|i| {
            matches!(
                i.result.kind,
                ResolvedImportResultKind::OwnedOptionString
                    | ResolvedImportResultKind::OwnedResultStringI64
            )
        })
        .ok_or_else(|| sdk_error("native container constructor is absent"))?;
    let option = constructor.result.kind == ResolvedImportResultKind::OwnedOptionString;
    let ty = constructor.result.kind.value_type(&program.declarations)?;
    let method = imports
        .iter()
        .find(|i| {
            i.parameters
                .first()
                .is_some_and(|p| p.ty == ty && p.ownership == OwnershipMode::Own)
        })
        .ok_or_else(|| sdk_error("native container consumer is absent"))?;
    if imports.len() != 2
        || imports.iter().any(|i| {
            !i.native_rust
                || !i.effects.is_empty()
                || i.failure != semaprax::hir::ResolvedImportFailure::Infallible
        })
        || constructor.parameters.len() != 1
        || constructor.parameters[0].ty != ResolvedType::I64
        || constructor.parameters[0].ownership != OwnershipMode::Value
        || constructor.result.ownership != OwnershipMode::Own
        || method.parameters.len() != 2
        || method.parameters[1].ty != ResolvedType::I64
        || method.parameters[1].ownership != OwnershipMode::Value
        || method.result.kind != ResolvedImportResultKind::Bool
    {
        return Err(sdk_error(
            "native container signature is outside the closed profile",
        ));
    }
    let function = program
        .functions
        .iter()
        .find(|f| f.id.as_str() == function_id)
        .ok_or_else(|| sdk_error("native container entry is absent"))?;
    let layout = c::ContainerLayout::new(&program, &ty, option)?;
    let lifecycle = DeclarationId::new(semaprax::cleanup::STRING_DROP_LIFECYCLE_ID);
    let c_source = c::render_container_program(
        &program,
        function,
        &constructor.id,
        &method.id,
        &lifecycle,
        &ty,
        &layout,
    )?;
    let header="#ifndef SPX_CONTAINER_V1_H\n#define SPX_CONTAINER_V1_H\n#include <stdint.h>\ntypedef struct { uint64_t context,generation,slot; } spx_payload_owner;\ntypedef struct { uint8_t tag,reserved[7]; int64_t error; spx_payload_owner payload; } spx_container;\nint32_t spx_container_new(uint64_t,int64_t,spx_container*);\nint32_t spx_container_consume(uint64_t,spx_container,int64_t,uint8_t*);\nint32_t spx_container_validate(uint64_t,spx_container);\nint32_t spx_container_drop(uint64_t,spx_container);\n#endif\n".to_owned();
    // Reuse the existing String owner quarantine and its panic policy. This
    // profile replaces construction/consumption with typed enum matches only.
    let template = include_str!("owner_runtime.rs.txt");
    let start = template
        .find("#[no_mangle]\npub unsafe extern \"C\" fn spx_owner_new")
        .unwrap();
    let end = template
        .find("#[no_mangle]\npub extern \"C\" fn spx_owner_drop")
        .unwrap();
    let mut rust_adapter=format!("{}{}",&template[..start],&template[end..])
        .replace("@TYPE@","std::string::String")
        .replace("spx_owner_context_","spx_container_context_")
        .replace("spx_owner_drop","spx_payload_drop")
        .replace("contexts.push(Context { id, slots: Vec::new() }); id", "if contexts.try_reserve(1).is_err() { return 0; } contexts.push(Context { id, slots: Vec::new() }); id");
    let (native, decode, active, inactive) = if option {
        (
            "core::option::Option<std::string::String>",
            "None=>(0,0,None),Some(value)=>(1,0,Some(value))",
            "Some(payload)",
            "None",
        )
    } else {
        (
            "core::result::Result<std::string::String,i64>",
            "Ok(value)=>(0,0,Some(value)),Err(error)=>(1,error,None)",
            "Ok(payload)",
            "Err(wire.error)",
        )
    };
    rust_adapter.push_str(
        &include_str!("owned_container_runtime.rs.txt")
            .replace("@NATIVE_TYPE@", native)
            .replace("@ACTIVE_TAG@", &layout.active_tag.to_string())
            .replace("@OPTION@", if option { "true" } else { "false" })
            .replace("@CONSTRUCTOR@", rust_path(constructor)?)
            .replace("@DECODE@", decode)
            .replace("@ACTIVE_VALUE@", active)
            .replace("@INACTIVE_VALUE@", inactive)
            .replace("@CONSUMER@", rust_path(method)?),
    );
    let params = (0..function.params.len())
        .map(|i| format!(",arg_{i}:i64"))
        .collect::<String>();
    let args = (0..function.params.len())
        .map(|i| format!(",arg_{i}"))
        .collect::<String>();
    let public = params.strip_prefix(',').unwrap_or("");
    let (result_type, result_value) = if function.return_type == ResolvedType::Bool {
        ("bool", "output!=0")
    } else {
        ("i64", "output")
    };
    rust_adapter.push_str(&format!("unsafe extern \"C\"{{fn spx_container_entry(context:u64{params},out:*mut i64)->i32;}}\npub fn spx_container_call({public})->Result<{result_type},i32>{{let context=spx_container_context_new();if context==0{{return Err(4)}}let mut output=0;let status=unsafe{{spx_container_entry(context{args},&mut output)}};let closed=spx_container_context_close(context);if status!=0{{Err(status)}}else if closed!=0{{Err(closed)}}else{{Ok({result_value})}}}}\n"));
    Ok(OpaqueOwnerNative {
        header,
        c_source,
        rust_adapter,
    })
}
