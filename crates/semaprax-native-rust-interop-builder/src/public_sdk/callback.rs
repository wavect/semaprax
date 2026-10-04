//! Additive rich callback projection. This module generates inert source only.
//! Snapshot closure semantics and scalar-v1 admission remain unchanged.
use super::*;
use semaprax::ast::Span;
use semaprax::ast::{ExprKind, Param, ParamMode, Type};
use semaprax::hir::{ResolvedExprKind, ResolvedType};

#[path = "callback_runtime.rs"]
pub(super) mod runtime;
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

/// One source revision projects both a bounded nominal Serde record and a
/// local stateful callback. This is inert source, not an installed package.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NativeSerdeCallbackProjection {
    pub source_revision: String,
    pub record: SerdeRecordProjection,
    pub callback: NativeCallbackProjection,
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
    prepare(source, path, selection, None, true)
        .map_err(|error| vec![error.at_path(path.display().to_string())])
}

/// Project one supported record and one scalar-snapshot callback from the
/// identical checked source. Ordinary callback admission stays scalar-only;
/// this opt-in route requires an exact selected record identity from the
/// fully checked source.
pub fn prepare_native_rust_serde_callbacks(
    source: &str,
    path: &Path,
    record_id: &str,
    selection: &NativeCallbackSelection,
) -> Result<NativeSerdeCallbackProjection, Vec<Diagnostic>> {
    prepare_serde_callbacks(source, path, record_id, selection, true)
}

/// Project the same record/callback source for a real standard-library
/// `Iterator` consumer, without requesting a nominal Rust trait impl.
pub fn prepare_native_rust_serde_iterator_callbacks(
    source: &str,
    path: &Path,
    record_id: &str,
    factory_id: &str,
    transition_id: &str,
) -> Result<NativeSerdeCallbackProjection, Vec<Diagnostic>> {
    let selection = NativeCallbackSelection {
        factory_id: factory_id.into(),
        transition_id: transition_id.into(),
        trait_path: String::new(),
        method: String::new(),
        error_type: String::new(),
    };
    prepare_serde_callbacks(source, path, record_id, &selection, false)
}

/// Project-only iterator projection for a source fact borrowed from a live
/// authenticated Project snapshot.
///
/// Indexed Rust imports are admitted by the Project frontend before this
/// operation. They do not enter the callback ABI: the local projection first
/// removes every interface, then resolves and validates the selected record
/// and callback declarations. A callback that depends on an imported item is
/// therefore refused. This returns inert generated source and carries no
/// publication or foreign-call authority; callers must keep the surrounding
/// Project transaction live so its final held-input recheck binds these bytes.
pub fn prepare_native_rust_serde_iterator_callbacks_from_authenticated_project_source(
    source: &semaprax::project::ProjectSource,
    path: &Path,
    record_id: &str,
    factory_id: &str,
    transition_id: &str,
) -> Result<NativeSerdeCallbackProjection, Vec<Diagnostic>> {
    let selection = NativeCallbackSelection {
        factory_id: factory_id.into(),
        transition_id: transition_id.into(),
        trait_path: String::new(),
        method: String::new(),
        error_type: String::new(),
    };
    prepare_serde_callbacks_from_authenticated_project_source(
        source.source(),
        path,
        record_id,
        &selection,
    )
}

fn prepare_serde_callbacks(
    source: &str,
    path: &Path,
    record_id: &str,
    selection: &NativeCallbackSelection,
    trait_impl: bool,
) -> Result<NativeSerdeCallbackProjection, Vec<Diagnostic>> {
    let located =
        |message| vec![refusal(message, Span::default()).at_path(path.display().to_string())];
    let parsed = semaprax::check(source, path)?;
    if !parsed.types.iter().any(|ty| ty.stable_id == record_id) {
        return Err(located(
            "Serde callback source requires its selected record",
        ));
    }
    let resolved = semaprax::hir::resolve(&parsed)?;
    let record = prepare_serde_record_projection(&resolved, record_id)
        .map_err(|error| vec![error.at_path(path.display().to_string())])?;
    let canonical = semaprax::format::canonical(&parsed);
    let source_revision =
        domain_digest(b"semaprax.rich-callback-source.v1\0", canonical.as_bytes());
    let callback = prepare_checked(
        parsed,
        resolved,
        source_revision,
        path,
        selection,
        Some(record_id),
        trait_impl,
    )
    .map_err(|error| vec![error.at_path(path.display().to_string())])?;
    Ok(NativeSerdeCallbackProjection {
        source_revision: callback.source_revision.clone(),
        record,
        callback,
    })
}

fn prepare_serde_callbacks_from_authenticated_project_source(
    source: &str,
    path: &Path,
    record_id: &str,
    selection: &NativeCallbackSelection,
) -> Result<NativeSerdeCallbackProjection, Vec<Diagnostic>> {
    let located = |error: Diagnostic| vec![error.at_path(path.display().to_string())];
    if source.len() > MAX_SOURCE_BYTES {
        return Err(located(refusal(
            "callback source exceeds its bound",
            Span::default(),
        )));
    }
    let parsed = semaprax::parse(source, path).map_err(located)?;
    if !parsed.types.iter().any(|ty| ty.stable_id == record_id) {
        return Err(located(refusal(
            "Serde callback source requires its selected record",
            Span::default(),
        )));
    }
    let canonical = semaprax::format::canonical(&parsed);
    let source_revision =
        domain_digest(b"semaprax.rich-callback-source.v1\0", canonical.as_bytes());
    // The selected callback must be closed over Semaprax declarations alone.
    // Clearing imports makes any selected-Rust dependency fail HIR resolution.
    let mut isolated = parsed.clone();
    isolated.interfaces.clear();
    let resolved = semaprax::hir::resolve(&isolated)
        .map_err(|mut errors| errors.remove(0).at_path(path.display().to_string()))?;
    semaprax::hir::validate(&resolved).map_err(located)?;
    let record = prepare_serde_record_projection(&resolved, record_id).map_err(located)?;
    let callback = prepare_checked(
        parsed,
        resolved,
        source_revision,
        path,
        selection,
        Some(record_id),
        false,
    )
    .map_err(|error| vec![error.at_path(path.display().to_string())])?;
    Ok(NativeSerdeCallbackProjection {
        source_revision: callback.source_revision.clone(),
        record,
        callback,
    })
}

fn prepare(
    source: &str,
    path: &Path,
    selection: &NativeCallbackSelection,
    allowed_record_id: Option<&str>,
    trait_impl: bool,
) -> Result<NativeCallbackProjection, Diagnostic> {
    let fail = |message| refusal(message, Span::default());
    if source.len() > MAX_SOURCE_BYTES {
        return Err(fail("callback source exceeds its bound"));
    }
    let program = semaprax::check(source, path).map_err(|mut errors| errors.remove(0))?;
    let canonical = semaprax::format::canonical(&program);
    let source_revision =
        domain_digest(b"semaprax.rich-callback-source.v1\0", canonical.as_bytes());
    let resolved = semaprax::hir::resolve(&program).map_err(|mut errors| errors.remove(0))?;
    semaprax::hir::validate(&resolved)?;
    prepare_checked(
        program,
        resolved,
        source_revision,
        path,
        selection,
        allowed_record_id,
        trait_impl,
    )
}

fn prepare_checked(
    mut program: semaprax::ast::Program,
    resolved: semaprax::hir::ResolvedProgram,
    source_revision: String,
    path: &Path,
    selection: &NativeCallbackSelection,
    allowed_record_id: Option<&str>,
    trait_impl: bool,
) -> Result<NativeCallbackProjection, Diagnostic> {
    let fail = |message| refusal(message, Span::default());
    let trait_tokens = if trait_impl {
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
        let error_token =
            semaprax::native_rust_binding::rust_api_path_tokens(&selection.error_type)
                .ok_or_else(|| fail("callback associated error token is unsupported"))?;
        Some((trait_path, method_token, error_token))
    } else {
        None
    };
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
        || (!program.types.is_empty()
            && !allowed_record_id
                .is_some_and(|id| program.types.iter().any(|ty| ty.stable_id == id)))
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
        retained: false,
        mutable: false,
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
    // The projection keeps only declarations selected by its checked source
    // evidence. Other Project declarations remain authenticated by the caller
    // but cannot enter this scalar callback renderer or its generated ABI.
    program.interfaces.clear();
    program.functions.retain(|function| {
        function.stable_id == selection.factory_id || function.stable_id == selection.transition_id
    });
    program
        .functions
        .retain(|f| f.stable_id != selection.factory_id);
    program.functions.push(lifted);
    // Rechecking the synthetic ordinary source prevents a renderer from
    // granting semantics/cleanup that its original checked closure lacked.
    let mut lifted_source = semaprax::format::canonical(&program);
    // The standalone checker requires an entry even though this private
    // projection exports only the selected callback and transition. Give the
    // synthetic source a trivial checked entry; it conveys no Project export
    // or authority and cannot depend on declarations removed above.
    lifted_source.push_str("\n@id(\"semaprax.callback.synthetic_main\")\nfn main() -> i64 { 0 }\n");
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
    let adapter_rust = match trait_tokens {
        Some((trait_path, method_token, error_token)) => runtime::render(
            method(&lifted_id)?,
            method(&selection.transition_id)?,
            &trait_path,
            &method_token,
            &error_token,
        ),
        None => runtime::render_iterator(method(&lifted_id)?, method(&selection.transition_id)?),
    };
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
