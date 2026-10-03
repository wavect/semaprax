//! Inert checked Result callback projection. No new source capture semantics.
use super::*;
use semaprax::ast::{Expr, ExprKind, ParamMode, Span, Type};
const ENTRY: &str = "semaprax.callback.result.entry";
const PUBLISH: &str = "semaprax.callback.result.publish";

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NativeResultCallbackSelection {
    pub callback_id: String,
    pub trait_path: String,
    pub method: String,
    pub error_type: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NativeResultCallbackProjection {
    pub callback_identity: String,
    pub source_revision: String,
    pub c_source: String,
    pub header: String,
    pub safe_rust: String,
    pub ffi_rust: String,
    pub adapter_rust: String,
}

/// Selects a pure `fn(i64,i64)->Result<i64,i64>` body. The first parameter is
/// an explicit scalar environment value. State transitions remain explicit;
/// this API does not reinterpret a snapshot closure as a mutable capture.
pub fn prepare_native_rust_result_callback(
    source: &str,
    path: &Path,
    selection: &NativeResultCallbackSelection,
) -> Result<NativeResultCallbackProjection, Vec<Diagnostic>> {
    prepare(source, path, selection)
        .map_err(|error| vec![error.at_path(path.display().to_string())])
}

fn prepare(
    source: &str,
    path: &Path,
    s: &NativeResultCallbackSelection,
) -> Result<NativeResultCallbackProjection, Diagnostic> {
    let fail = |message| Diagnostic::error("SPX-B154", message, Span::default());
    if source.len() > MAX_SOURCE_BYTES {
        return Err(fail("Result callback source exceeds bound"));
    }
    let token = |value: &str| {
        semaprax::native_rust_binding::rust_api_path_tokens(value)
            .filter(|_| value.len() <= 256)
            .ok_or_else(|| fail("Result callback Rust trait token is unsupported"))
    };
    let trait_path = token(&s.trait_path)?;
    let method = token(&s.method)?;
    let error_type = token(&s.error_type)?;
    if s.method.contains("::") || s.error_type.contains("::") {
        return Err(fail(
            "Result callback method and associated error must be identifiers",
        ));
    }
    let mut program = semaprax::check(source, path).map_err(|mut errors| errors.remove(0))?;
    let canonical = semaprax::format::canonical(&program);
    let resolved = semaprax::hir::resolve(&program).map_err(|mut errors| errors.remove(0))?;
    semaprax::hir::validate(&resolved)?;
    if !program.interfaces.is_empty()
        || !program.types.is_empty()
        || !program.permits.is_empty()
        || !program.protocols.is_empty()
        || !program.implementations.is_empty()
        || !program.module_uses.is_empty()
        || !program.agents.is_empty()
        || !program.session_protocols.is_empty()
    {
        return Err(fail(
            "Result callback requires a closed pure scalar source module",
        ));
    }
    let callback = program
        .functions
        .iter()
        .find(|f| f.stable_id == s.callback_id)
        .ok_or_else(|| fail("Result callback declaration is absent"))?;
    let result = Type::Named {
        name: "Result".into(),
        arguments: vec![Type::I64, Type::I64],
    };
    if !callback.explicit_id
        || callback.params.len() != 2
        || callback
            .params
            .iter()
            .any(|p| p.mode != ParamMode::Value || p.ty != Type::I64)
        || callback.return_type != result
        || !callback.type_parameters.is_empty()
        || callback.yields.is_some()
        || callback.follows.is_some()
        || program.functions.iter().any(|f| !f.effects.is_empty())
    {
        return Err(Diagnostic::error(
            "SPX-B154",
            "Result callback requires pure fn(i64,i64)->Result<i64,i64>",
            callback.span,
        ));
    }
    if callback.ensures.iter().any(|condition| {
        !parameter_contract(
            condition,
            &callback
                .params
                .iter()
                .map(|p| p.name.as_str())
                .collect::<Vec<_>>(),
        )
    }) {
        return Err(Diagnostic::error(
            "SPX-B154",
            "Result callback postconditions must depend only on scalar parameters",
            callback.span,
        ));
    }
    let mut lifted = callback.clone();
    lifted.body = publish_tail(&callback.body, 0)?;
    lifted.stable_id = ENTRY.into();
    lifted.return_type = Type::I64;
    lifted.effects = vec!["callback.result".into()];
    if program.functions.iter().any(|f| {
        [ENTRY, PUBLISH].contains(&f.stable_id.as_str())
            || ["spx_callback_result_entry", "spx_callback_result_publish"]
                .contains(&f.name.as_str())
    }) {
        return Err(fail(
            "Result callback derived identity collides with source",
        ));
    }
    // Translate only authenticated terminal Result constructors. Every payload,
    // branch and preceding statement executes once in its original order. A
    // private import stages the tag/value; postconditions and the ordinary C
    // boundary must succeed before generated Rust publishes it to its caller.
    let derived = format!(
        r#"module callback.result.projection;
permit {{ callback.result }}
@id("semaprax.callback.result.host") interface CallbackResultHost permits {{ callback.result }} {{
 @id("{PUBLISH}") import rust fn spx_callback_result_publish(tag:i64,value:i64)->i64 effects {{ callback.result }} failure status "semaprax.callback.result.v1";
}}
@id("semaprax.callback.result.parse_stub") fn parse_stub()->i64 {{0}}
"#
    );
    let mut derived = semaprax::parse(&derived, path)?;
    program.permits.append(&mut derived.permits);
    program.interfaces.append(&mut derived.interfaces);
    program.functions.retain(|f| f.stable_id != s.callback_id);
    program.functions.push(lifted);
    let projected = semaprax::format::canonical(&program);
    let program = semaprax::check(&projected, path).map_err(|mut errors| errors.remove(0))?;
    let revision = domain_digest(SOURCE_DOMAIN, projected.as_bytes());
    let options = NativeRustSdkOptions {
        exports: vec![ENTRY.into()],
        imports: vec![PUBLISH.into()],
        capabilities: vec!["callback.result".into()],
    };
    let target = target_triple().ok_or_else(|| fail("Result callback target is unsupported"))?;
    let spec = descriptor::canonical_spec(&program.module, &revision, target, &options)?;
    let prepared = crate::implementation::prepare_native_rust_interop(&program, spec.as_bytes())
        .map_err(|mut errors| errors.remove(0))?;
    let descriptor: Value = serde_json::from_str(prepared.descriptor())
        .map_err(|_| fail("Result callback descriptor is invalid"))?;
    let method_for = |group: &str, id: &str| {
        descriptor[group]
            .as_array()
            .and_then(|rows| rows.iter().find(|r| r["id"] == id))
            .and_then(|row| row["rust_method"].as_str())
            .ok_or_else(|| fail("Result callback descriptor method is absent"))
    };
    let adapter_rust = callback::runtime::render_result(
        method_for("exports", ENTRY)?,
        method_for("imports", PUBLISH)?,
        &trait_path,
        &method,
        &error_type,
    );
    Ok(NativeResultCallbackProjection {
        callback_identity: s.callback_id.clone(),
        source_revision: domain_digest(
            b"semaprax.rich-result-callback-source.v1\0",
            canonical.as_bytes(),
        ),
        c_source: prepared.generated_c().into(),
        header: prepared.generated_header().into(),
        safe_rust: prepared.generated_rust().into(),
        ffi_rust: prepared.private_ffi_source().into(),
        adapter_rust,
    })
}

// A Result-dependent postcondition cannot be silently reinterpreted against
// the scalar acknowledgement. This first profile admits parameter-only checks.
fn parameter_contract(expr: &Expr, parameters: &[&str]) -> bool {
    match &expr.kind {
        ExprKind::Int(_) | ExprKind::Bool(_) => true,
        ExprKind::Var(name) => parameters.contains(&name.as_str()),
        ExprKind::Unary { value, .. } => parameter_contract(value, parameters),
        ExprKind::Binary { left, right, .. } => {
            parameter_contract(left, parameters) && parameter_contract(right, parameters)
        }
        _ => false,
    }
}

fn publish_tail(expr: &Expr, depth: usize) -> Result<Expr, Diagnostic> {
    let refuse = || {
        Diagnostic::error(
            "SPX-B154",
            "Result callback requires bounded terminal Result constructors, blocks and branches",
            expr.span,
        )
    };
    if depth > 64 {
        return Err(refuse());
    }
    let mut result = expr.clone();
    match &mut result.kind {
        ExprKind::Block { tail, .. } => **tail = publish_tail(tail, depth + 1)?,
        ExprKind::If {
            then_branch,
            else_branch,
            ..
        } => {
            **then_branch = publish_tail(then_branch, depth + 1)?;
            **else_branch = publish_tail(else_branch, depth + 1)?;
        }
        ExprKind::ConstructVariant {
            type_name,
            case_name,
            fields,
            ..
        } if type_name == "Result" && fields.len() == 1 => {
            let tag = match (case_name.as_str(), fields[0].name.as_str()) {
                ("Ok", "value") => 0,
                ("Err", "error") => 1,
                _ => return Err(refuse()),
            };
            result.kind = ExprKind::Call {
                name: "spx_callback_result_publish".into(),
                type_arguments: Vec::new(),
                args: vec![
                    Expr {
                        kind: ExprKind::Int(tag),
                        span: expr.span,
                    },
                    fields[0].value.clone(),
                ],
            };
        }
        _ => return Err(refuse()),
    }
    Ok(result)
}

#[cfg(test)]
#[path = "result_callback_tests.rs"]
mod tests;
