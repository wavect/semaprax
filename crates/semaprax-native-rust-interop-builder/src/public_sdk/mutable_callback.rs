//! Inert native projection of the exact source-created transactional receiver.
//! The source and HIR gates are authoritative; this renderer cannot admit the
//! reserved mutable profile before the compiler's cross-backend gate opens.
use super::*;
use semaprax::ast::{ExprKind, ParamMode, Span, Type};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NativeMutableCallbackProjection {
    pub source_revision: String,
    pub c_source: String,
    pub safe_rust: String,
}

/// Render a same-thread Rust owner for `fn(i64)->FnMutI64(i64)->i64`.
/// The C factory constructs the actual source carrier; no alternative
/// next-state function is supplied by the caller or substituted by the SDK.
/// Rendering invokes no compiler, opens no foreign registry and grants no
/// authority to execute or publish the generated source.
pub fn prepare_native_rust_mutable_callback(
    source: &str,
    path: &Path,
    factory_id: &str,
) -> Result<NativeMutableCallbackProjection, Vec<Diagnostic>> {
    let fail = |message: &str| vec![Diagnostic::error("SPX-B154", message, Span::default())];
    if source.len() > MAX_SOURCE_BYTES {
        return Err(fail("mutable callback source exceeds its bound"));
    }
    let program = semaprax::check(source, path)?;
    let factory = program
        .functions
        .iter()
        .find(|f| f.stable_id == factory_id)
        .ok_or_else(|| fail("mutable callback factory is absent"))?;
    if !factory.explicit_id
        || !factory.type_parameters.is_empty()
        || factory.params.len() != 1
        || factory.params[0].ty != Type::I64
        || factory.params[0].mode != ParamMode::Value
        || factory.return_type != Type::MutFunctionI64
        || !factory.effects.is_empty()
        || !program.interfaces.is_empty()
    {
        return Err(fail(
            "mutable factory requires pure fn(i64)->FnMutI64(i64)->i64 without foreign interfaces",
        ));
    }
    let ExprKind::Block { statements, tail } = &factory.body.kind else {
        return Err(fail(
            "mutable callback factory must directly return its receiver",
        ));
    };
    if !statements.is_empty() || !matches!(tail.kind, ExprKind::Closure { mutable: true, .. }) {
        return Err(fail(
            "mutable callback factory must directly return its receiver",
        ));
    }
    let resolved = semaprax::hir::resolve(&program)?;
    semaprax::hir::validate(&resolved).map_err(|e| vec![e])?;
    let closure = semaprax::hir::closure::inventory(&resolved)
        .into_iter()
        .find(|e| e.span == tail.span)
        .ok_or_else(|| fail("mutable source closure body is absent"))?;
    let semaprax::hir::ResolvedExprKind::Closure { captures, .. } = &closure.kind else {
        unreachable!()
    };
    if captures.len() != 1 || captures[0].binding.name != factory.params[0].name {
        return Err(fail(
            "mutable callback must capture its exact factory parameter",
        ));
    }
    semaprax::hir::closure::closure_function(&resolved, closure).map_err(|e| vec![e])?;
    let source_revision = domain_digest(
        b"semaprax.mutable-callback-source.v1\0",
        semaprax::format::canonical(&program).as_bytes(),
    );
    let mut c_source = String::from("#define SPX_NO_ENTRY_WRAPPER 1\n");
    c_source.push_str(&semaprax::codegen::emit_c(&program).map_err(|e| vec![e])?);
    let symbol = format!(
        "spx_decl_{}",
        factory_id
            .bytes()
            .map(|b| format!("{b:02x}"))
            .collect::<String>()
    );
    c_source
        .push_str(&include_str!("mutable_callback_c.template").replace("FACTORY_SYMBOL", &symbol));
    Ok(NativeMutableCallbackProjection {
        source_revision,
        c_source,
        safe_rust: include_str!("mutable_callback_rust.template").into(),
    })
}
