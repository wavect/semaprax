//! Additive rich callback projection. This module generates inert source only.
//! Snapshot closure semantics and scalar-v1 admission remain unchanged.
use super::*;
use semaprax::ast::{ExprKind, Param, ParamMode, Type};
use semaprax::ast::Span;
use semaprax::hir::{ResolvedExprKind, ResolvedType};

#[path = "callback_runtime.rs"]
mod runtime;
#[cfg(test)]
#[path = "callback_tests.rs"]
mod tests;

/// A local safe trait proxy with a single scalar state transition.
/// The Rust compiler must prove this exact impl against the selected trait.
/// This does not request arbitrary associated types or unsafe implementations.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NativeCallbackSelection {
    pub factory_id: String,
    pub transition_id: String,
    pub trait_path: String,
    pub method: String,
    pub error_type: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NativeCallbackProjection {
    pub closure_identity: String,
    pub source_revision: String,
    pub c_source: String,
    pub header: String,
    pub safe_rust: String,
    pub ffi_rust: String,
    pub adapter_rust: String,
}

fn refusal(message: impl Into<String>, span: Span) -> Diagnostic {
    Diagnostic::error("SPX-B154", message, span)
}

/// Selects an explicit factory `fn(i64)->fn(i64)->i64` whose body is one
/// checked scalar-snapshot closure, and a checked `fn(i64,i64)->i64` transition.
/// The transition returns the next state; failed calls never commit that state.
/// The generated Rust impl is safe, nominal and compiler-checked by its caller.
/// This effect-free renderer neither runs derives/tools nor publishes a package.
pub fn prepare_native_rust_callbacks(
    source: &str,
    path: &Path,
    selection: &NativeCallbackSelection,
) -> Result<NativeCallbackProjection, Vec<Diagnostic>> {
    prepare(source, path, selection)
        .map_err(|error| vec![error.at_path(path.display().to_string())])
}

fn prepare(
    source: &str,
    path: &Path,
    selection: &NativeCallbackSelection,
) -> Result<NativeCallbackProjection, Diagnostic> {
    let fail = |message| refusal(message, Span::default());
    if source.len() > MAX_SOURCE_BYTES {
        return Err(fail("callback source exceeds its bound"));
    }
    let trait_path = semaprax::native_rust_binding::rust_api_path_tokens(&selection.trait_path)
        .ok_or_else(|| fail("callback trait path must be a bounded Rust item path"))?;
    if selection.trait_path.len() > 256
        || !identifier(&selection.method)
        || !identifier(&selection.error_type)
    {
        return Err(fail(
            "callback trait method/associated error names are unsupported",
        ));
    }
    let method_token = semaprax::native_rust_binding::rust_api_path_tokens(&selection.method)
        .ok_or_else(|| fail("callback trait method token is unsupported"))?;
    let error_token = semaprax::native_rust_binding::rust_api_path_tokens(&selection.error_type)
        .ok_or_else(|| fail("callback associated error token is unsupported"))?;
    let mut program = semaprax::check(source, path).map_err(|mut errors| errors.remove(0))?;
    let canonical = semaprax::format::canonical(&program);
    let source_revision =
        domain_digest(b"semaprax.rich-callback-source.v1\0", canonical.as_bytes());
    let resolved = semaprax::hir::resolve(&program).map_err(|mut errors| errors.remove(0))?;
    semaprax::hir::validate(&resolved)?;
    let factory = program
        .functions
        .iter()
        .find(|f| f.stable_id == selection.factory_id)
        .ok_or_else(|| fail("callback factory is absent"))?
        .clone();
    let at = |message| refusal(message, factory.span);
    if !factory.explicit_id
        || !factory.type_parameters.is_empty()
        || factory.params.len() != 1
        || factory.params[0].ty != Type::I64
        || factory.params[0].mode != ParamMode::Value
        || !factory.effects.is_empty()
        || !factory.requires.is_empty()
        || !factory.ensures.is_empty()
        || factory.yields.is_some()
        || factory.follows.is_some()
        || !program.interfaces.is_empty()
        || !program.types.is_empty()
    {
        return Err(at(
            "callback factory requires one scalar snapshot parameter and no effects/contracts",
        ));
    }
    let ExprKind::Block { statements, tail } = &factory.body.kind else {
        return Err(at("callback factory must directly return its closure"));
    };
    if !statements.is_empty() {
        return Err(at(
            "callback capture creation must be the direct factory parameter",
        ));
    }
    let ExprKind::Closure {
        params,
        return_type,
        body,
        owning: false,
    } = &tail.kind
    else {
        return Err(at(
            "callback factory requires an existing non-owning snapshot closure",
        ));
    };
    if params.len() != 1 || params[0].ty != Type::I64 || *return_type != Type::I64 {
        return Err(at("callback closure must have signature fn(i64)->i64"));
    }
    let closure = semaprax::hir::closure::inventory(&resolved)
        .into_iter()
        .find(|e| e.span == tail.span)
        .ok_or_else(|| at("callback closure has no checked HIR identity"))?;
    let ResolvedExprKind::Closure { captures, .. } = &closure.kind else {
        unreachable!()
    };
    if captures.len() != 1
        || captures[0].binding.name != factory.params[0].name
        || captures[0].binding.ty != ResolvedType::I64
    {
        return Err(at(
            "callback must capture exactly its scalar factory parameter",
        ));
    }
    // Authenticate the ordinary independent body cleanup/loan product before
    // lifting the identical authored body into the existing checked scalar ABI.
    let product = semaprax::hir::closure::closure_function(&resolved, closure)?;
    let closure_identity = product.id.as_str().to_owned();
    let transition = program
        .functions
        .iter()
        .find(|f| f.stable_id == selection.transition_id)
        .ok_or_else(|| fail("callback state transition is absent"))?;
    if !transition.explicit_id
        || transition.params.len() != 2
        || transition
            .params
            .iter()
            .any(|p| p.ty != Type::I64 || p.mode != ParamMode::Value)
        || transition.return_type != Type::I64
        || !transition.effects.is_empty()
        || !transition.type_parameters.is_empty()
    {
        return Err(refusal(
            "callback state transition requires fn(i64,i64)->i64",
            transition.span,
        ));
    }
    let lifted_id = format!("{}.rich_callback", selection.factory_id);
    let lifted_name = format!("{}_rich_callback", factory.name);
    if program
        .functions
        .iter()
        .any(|f| f.stable_id == lifted_id || f.name == lifted_name)
    {
        return Err(at("derived callback entry identity collides with source"));
    }
    let mut lifted = factory.clone();
    lifted.stable_id = lifted_id.clone();
    lifted.name = lifted_name;
    lifted.params.push(Param {
        name: params[0].name.clone(),
        mode: ParamMode::Value,
        ty: Type::I64,
        span: params[0].span,
    });
    lifted.return_type = Type::I64;
    lifted.body = *body.clone();
    program
        .functions
        .retain(|f| f.stable_id != selection.factory_id);
    program.functions.push(lifted);
    // Rechecking the synthetic ordinary source prevents a renderer from
    // granting semantics/cleanup that its original checked closure lacked.
    let lifted_source = semaprax::format::canonical(&program);
    let program = semaprax::check(&lifted_source, path).map_err(|mut errors| errors.remove(0))?;
    let options = NativeRustSdkOptions {
        exports: canonical_values(
            vec![lifted_id.clone(), selection.transition_id.clone()],
            MAX_EXPORTS,
        )?,
        imports: vec![],
        capabilities: vec![],
    };
    let revision = domain_digest(
        SOURCE_DOMAIN,
        semaprax::format::canonical(&program).as_bytes(),
    );
    let target = target_triple().ok_or_else(|| fail("callback target is unsupported"))?;
    let spec = descriptor::canonical_spec(&program.module, &revision, target, &options)?;
    let prepared = crate::implementation::prepare_native_rust_interop(&program, spec.as_bytes())
        .map_err(|mut errors| errors.remove(0))?;
    let descriptor: Value = serde_json::from_str(prepared.descriptor())
        .map_err(|_| fail("callback descriptor is invalid"))?;
    let method = |id: &str| -> Result<&str, Diagnostic> {
        descriptor["exports"]
            .as_array()
            .and_then(|rows| rows.iter().find(|r| r["id"] == id))
            .and_then(|row| row["rust_method"].as_str())
            .ok_or_else(|| fail("callback export method is absent"))
    };
    let adapter_rust = runtime::render(
        method(&lifted_id)?,
        method(&selection.transition_id)?,
        &trait_path,
        &method_token,
        &error_token,
    );
    Ok(NativeCallbackProjection {
        closure_identity,
        source_revision,
        c_source: prepared.generated_c().into(),
        header: prepared.generated_header().into(),
        safe_rust: prepared.generated_rust().into(),
        ffi_rust: prepared.private_ffi_source().into(),
        adapter_rust,
    })
}

fn identifier(value: &str) -> bool {
    value.len() <= 128
        && !value.is_empty()
        && value
            .bytes()
            .enumerate()
            .all(|(i, b)| b == b'_' || b.is_ascii_alphabetic() || i > 0 && b.is_ascii_digit())
        && !matches!(
            value,
            "self" | "Self" | "fn" | "type" | "impl" | "unsafe" | "crate" | "super"
        )
}
