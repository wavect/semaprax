//! Bounded native String retention. This is pure checked-source generation,
//! separate from scalar SDK publication and the existing opaque resource ABI.
use super::*;
use semaprax::cleanup::FieldLivenessShape;

/// Keep at most 4096 UTF-8 bytes/capacity per returned Rust String. The Rust
/// target itself is trusted native code; this is a retained-value bound, not an
/// allocator sandbox around its execution.
pub fn prepare_owned_string_native(
    source: &semaprax::ast::Program,
    function_id: &str,
) -> Result<OpaqueOwnerNative, Diagnostic> {
    let program = semaprax::hir::resolve(source)
        .map_err(|_| sdk_error("native String source failed HIR resolution"))?;
    semaprax::hir::validate(&program)?;
    let imports = program
        .interfaces
        .iter()
        .flat_map(|i| &i.imports)
        .collect::<Vec<_>>();
    let constructor = imports
        .iter()
        .find(|i| i.result.kind == ResolvedImportResultKind::OwnedString)
        .ok_or_else(|| sdk_error("native String constructor is absent"))?;
    let method = imports
        .iter()
        .find(|i| {
            i.parameters
                .first()
                .is_some_and(|p| p.ty == ResolvedType::String && p.ownership == OwnershipMode::Own)
        })
        .ok_or_else(|| sdk_error("native String consumer is absent"))?;
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
            "native String signature is outside the bounded profile",
        ));
    }
    let function = program
        .functions
        .iter()
        .find(|f| f.id.as_str() == function_id)
        .ok_or_else(|| sdk_error("native String entry is absent"))?;
    let lifecycle = program
        .functions
        .iter()
        .flat_map(|f| &f.cleanup_plan.slots)
        .find_map(|slot| match &slot.field_liveness_shape {
            FieldLivenessShape::Leaf { lifecycle, .. }
                if slot.ty == ResolvedType::String
                    && lifecycle.as_str() == semaprax::cleanup::STRING_DROP_LIFECYCLE_ID =>
            {
                Some(lifecycle)
            }
            _ => None,
        })
        .ok_or_else(|| sdk_error("native String cleanup lifecycle is absent"))?;
    let header = "#ifndef SPX_OWNER_V1_H\n#define SPX_OWNER_V1_H\n#include <stdint.h>\ntypedef struct { uint64_t context, generation, slot; } spx_owner;\nint32_t spx_owner_new(uint64_t, int64_t, spx_owner*);\nint32_t spx_owner_consume(uint64_t, spx_owner, int64_t, uint8_t*);\nint32_t spx_owner_validate(uint64_t, spx_owner);\nint32_t spx_owner_drop(uint64_t, spx_owner);\nint32_t spx_owner_string_clone(uint64_t, spx_owner, spx_owner*);\nint32_t spx_owner_string_from_utf8(uint64_t, const uint8_t*, uint64_t, spx_owner*);\nint32_t spx_owner_string_from_utf8_signed(uint64_t, const uint8_t*, int64_t, spx_owner*);\n#endif\n".to_owned();
    let c_source = c::render_program(
        &program,
        function,
        &constructor.id,
        &method.id,
        lifecycle,
        &ResolvedType::String,
    )?;
    let mut rust_adapter = include_str!("owner_runtime.rs.txt")
        .replace("@TYPE@", "std::string::String")
        .replace("@CONSTRUCTOR@", rust_path(constructor)?)
        .replace("@METHOD@", rust_path(method)?);
    // The String route checks fallible bridge allocation before target effects.
    // Ordinary owner rendering keeps its prior template expansion unchanged.
    rust_adapter = rust_adapter.replace(
        "contexts.push(Context { id, slots: Vec::new() }); id",
        "if contexts.try_reserve(1).is_err() { return 0; } contexts.push(Context { id, slots: Vec::new() }); id",
    );
    let call = format!("let value = {}(arg);", rust_path(constructor)?);
    let bounded = format!("if free.is_none() {{ context.slots.try_reserve(1).map_err(|_|4)?; }}\n        {call}\n        if value.len()>4096 || value.capacity()>4096 {{ return Err(4); }}");
    rust_adapter = rust_adapter.replace(&call, &bounded);
    rust_adapter.push_str(include_str!("owned_string_runtime.rs.txt"));
    rust_adapter.push_str(include_str!("owned_string_input.rs.txt"));
    let params = (0..function.params.len())
        .map(|i| format!(", arg_{i}:i64"))
        .collect::<String>();
    let args = (0..function.params.len())
        .map(|i| format!(", arg_{i}"))
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
