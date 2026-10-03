//! Source-authored registry calls using a separate scalar bridge instance.
//! Kept as the follow-up to callback.rs; requires its physical gate first.
use super::*;
use semaprax::ast::{ImportFailure, ImportResult, ParamMode, Type};
use semaprax::ast::Span;
use semaprax::hir::{ResolvedExprKind, ResolvedType};
const REGISTRY_DOMAIN: &str = "semaprax.rich-callback-registry.v1";
const REGISTRY_EFFECT: &str = "callback.registry";

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NativeRegistrySelection {
    pub callback: NativeCallbackSelection,
    pub registry_path: String,
    pub install_export: String,
    pub apply_export: String,
    pub close_export: String,
    pub register_import: String,
    pub dispatch_import: String,
    pub unregister_import: String,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NativeRegisteredCallbackProjection {
    pub callback: NativeCallbackProjection,
    pub source_revision: String,
    pub registry_c_source: String,
    pub registry_header: String,
    pub registry_safe_rust: String,
    pub registry_ffi_rust: String,
    pub registry_adapter: String,
}
fn failure(message: impl Into<String>) -> Diagnostic {
    Diagnostic::error("SPX-B154", message, Span::default())
}
pub fn prepare_registered_native_rust_callbacks(
    source: &str,
    path: &Path,
    selection: &NativeRegistrySelection,
) -> Result<NativeRegisteredCallbackProjection, Vec<Diagnostic>> {
    prepare(source, path, selection)
        .map_err(|error| vec![error.at_path(path.display().to_string())])
}
fn prepare(
    source: &str,
    path: &Path,
    s: &NativeRegistrySelection,
) -> Result<NativeRegisteredCallbackProjection, Diagnostic> {
    if source.len() > MAX_SOURCE_BYTES {
        return Err(failure("registered callback source exceeds bound"));
    }
    let registry = semaprax::native_rust_binding::rust_api_path_tokens(&s.registry_path)
        .filter(|_| s.registry_path.len() <= 256)
        .ok_or_else(|| failure("registry path is unsupported"))?;
    let program = semaprax::check(source, path).map_err(|mut e| e.remove(0))?;
    let resolved = semaprax::hir::resolve(&program).map_err(|mut e| e.remove(0))?;
    semaprax::hir::validate(&resolved)?;
    let exports = [&s.install_export, &s.apply_export, &s.close_export];
    let imports = [&s.register_import, &s.dispatch_import, &s.unregister_import];
    let mut distinct = BTreeSet::new();
    if !exports
        .iter()
        .chain(&imports)
        .chain([&s.callback.factory_id, &s.callback.transition_id].iter())
        .all(|id| distinct.insert(id.as_str()))
    {
        return Err(failure("callback registration identities must be distinct"));
    }
    if program.functions.len() != 6
        || program
            .interfaces
            .iter()
            .map(|i| i.imports.len())
            .sum::<usize>()
            != 3
    {
        return Err(failure("registered profile requires factory, transition, three registry exports and entry only"));
    }
    for (ordinal, (export, import)) in exports.iter().zip(&imports).enumerate() {
        let params = usize::from(ordinal < 2);
        let source_import = program
            .interfaces
            .iter()
            .flat_map(|i| &i.imports)
            .find(|i| &i.stable_id == *import)
            .ok_or_else(|| failure("registration import is absent"))?;
        if !source_import.native_rust
            || source_import.index_selected
            || source_import.rust_path.is_some()
            || source_import.params.len() != params
            || source_import
                .params
                .iter()
                .any(|p| p.ty != Type::I64 || p.mode != ParamMode::Value)
            || source_import.result != ImportResult::I64
            || source_import.effects.len() != 1
            || source_import.effects[0] != REGISTRY_EFFECT
            || source_import.failure
                != (ImportFailure::Status {
                    domain_id: REGISTRY_DOMAIN.into(),
                })
        {
            return Err(failure("registration import signature/effect/failure domain differs from its closed profile"));
        }
        let f = resolved
            .functions
            .iter()
            .find(|f| f.id.as_str() == export.as_str())
            .ok_or_else(|| failure("registration export is absent"))?;
        if f.params.len() != params
            || f.return_type != ResolvedType::I64
            || !f.requires.is_empty()
            || !f.ensures.is_empty()
        {
            return Err(failure("registration export requires one exact direct registry operation without contracts"));
        }
        let ResolvedExprKind::Block { statements, tail } = &f.body.kind else {
            return Err(failure("registration export must be a direct call"));
        };
        let ResolvedExprKind::NativeRustImportCall(call) = &tail.kind else {
            return Err(failure(
                "registration export must call its declared Rust registry import",
            ));
        };
        if !statements.is_empty()
            || call.import.as_str() != import.as_str()
            || call.args.len() != params
        {
            return Err(failure(
                "registration export calls an unexpected registry operation",
            ));
        }
        if let Some(argument) = call.args.first() {
            if !matches!(&argument.kind,ResolvedExprKind::Place(place) if place.root==f.params[0].id&&place.projections.is_empty())
            {
                return Err(failure(
                    "registration operation must receive the authored scalar parameter unchanged",
                ));
            }
        }
    }
    let source_revision = domain_digest(
        b"semaprax.rich-callback-registry-source.v1\0",
        semaprax::format::canonical(&program).as_bytes(),
    );
    let mut body = program.clone();
    body.interfaces.clear();
    body.permits.clear();
    body.functions
        .retain(|f| !exports.iter().any(|id| f.stable_id.as_str() == id.as_str()));
    let callback =
        prepare_native_rust_callbacks(&semaprax::format::canonical(&body), path, &s.callback)
            .map_err(|mut e| e.remove(0))?;
    let mut registry_program = program.clone();
    registry_program.functions.retain(|f| {
        f.stable_id != s.callback.factory_id && f.stable_id != s.callback.transition_id
    });
    let registry_program = semaprax::check(&semaprax::format::canonical(&registry_program), path)
        .map_err(|mut e| e.remove(0))?;
    let options = NativeRustSdkOptions {
        exports: canonical_values(exports.iter().map(|v| v.to_string()).collect(), MAX_EXPORTS)?,
        imports: canonical_values(imports.iter().map(|v| v.to_string()).collect(), MAX_IMPORTS)?,
        capabilities: vec![REGISTRY_EFFECT.into()],
    };
    let revision = domain_digest(
        SOURCE_DOMAIN,
        semaprax::format::canonical(&registry_program).as_bytes(),
    );
    let target =
        target_triple().ok_or_else(|| failure("registered callback target unsupported"))?;
    let spec = descriptor::canonical_spec(&program.module, &revision, target, &options)?;
    let prepared =
        crate::implementation::prepare_native_rust_interop(&registry_program, spec.as_bytes())
            .map_err(|mut e| e.remove(0))?;
    let descriptor: Value = serde_json::from_str(prepared.descriptor())
        .map_err(|_| failure("registry descriptor invalid"))?;
    let method = |group: &str, id: &str| -> Result<&str, Diagnostic> {
        descriptor[group]
            .as_array()
            .and_then(|rows| rows.iter().find(|r| r["id"] == id))
            .and_then(|r| r["rust_method"].as_str())
            .ok_or_else(|| failure("registry method absent"))
    };
    let registry_adapter = include_str!("registered_runtime.template")
        .replace("$REGISTRY", &registry)
        .replace("$REGISTER", method("imports", &s.register_import)?)
        .replace("$DISPATCH", method("imports", &s.dispatch_import)?)
        .replace("$UNREGISTER", method("imports", &s.unregister_import)?)
        .replace("$INSTALL", method("exports", &s.install_export)?)
        .replace("$APPLY", method("exports", &s.apply_export)?)
        .replace("$CLOSE", method("exports", &s.close_export)?)
        .replace("$DOMAIN", &serde_json::to_string(REGISTRY_DOMAIN).unwrap());
    Ok(NativeRegisteredCallbackProjection {
        callback,
        source_revision,
        registry_c_source: prepared.generated_c().replace(
            "\"semaprax_native_rust_interop.h\"",
            "\"registry_boundary.h\"",
        ),
        registry_header: prepared.generated_header().into(),
        registry_safe_rust: prepared
            .generated_rust()
            .replace("semaprax_native_rust_interop_ffi.rs", "registry_ffi.rs"),
        registry_ffi_rust: prepared.private_ffi_source().into(),
        registry_adapter,
    })
}

#[cfg(test)]
#[path = "registered_callback_tests.rs"]
mod tests;
