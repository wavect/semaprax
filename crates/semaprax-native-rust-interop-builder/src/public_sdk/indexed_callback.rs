//! Selected-index admission for the bounded local safe callback trait impl.
//! Replay is metadata evidence; the selected stable rustc must still compile
//! the generated safe impl against the actual crate before any invocation.
use super::*;
use crate::indexed_binding::SelectedPackage;
use semaprax_rust_api_index::{
    ApiItem, ItemKind, Receiver, RejectionReason, RustApiIndex, Support, TypeRecordKind, Visibility,
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct IndexedResultCallbackSelection {
    pub callback_id: String,
    pub method_path: String,
    pub error_type_path: String,
    /// Exact canonical index digest selected by the caller's package plan.
    pub index_digest: String,
}

/// Inert evidence for the exact source/index pair. This is not a trust grant,
/// compiler certificate, or permission to load a foreign library.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct IndexedResultCallbackProjection {
    pub binding_identity: String,
    pub index_digest: String,
    pub method_path: String,
    pub error_type_path: String,
    pub callback: NativeResultCallbackProjection,
}

/// Derives the Rust impl tokens from one replayed public monomorphic trait:
/// `type Error; fn method(&mut self, value:i64)->Result<i64,Self::Error>`.
/// Other receivers, associated obligations, and signature shapes fail closed.
/// Source capture modes and the scalar-v1 ABI are unchanged.
pub fn prepare_indexed_native_rust_result_callback(
    source: &str,
    path: &Path,
    index_bytes: &[u8],
    package: SelectedPackage<'_>,
    selection: &IndexedResultCallbackSelection,
) -> Result<IndexedResultCallbackProjection, Vec<Diagnostic>> {
    if source.len() > MAX_SOURCE_BYTES {
        return Err(vec![Diagnostic::error(
            "SPX-B154",
            "callback source exceeds its bound",
            semaprax::ast::Span::default(),
        )
        .at_path(path.display().to_string())]);
    }
    let program = semaprax::check(source, path)?;
    let span = program
        .functions
        .iter()
        .find(|f| f.stable_id == selection.callback_id)
        .map(|f| f.span)
        .unwrap_or_default();
    let fail = |message: &str| {
        vec![Diagnostic::error("SPX-B154", message, span).at_path(path.display().to_string())]
    };
    let index = RustApiIndex::replay(index_bytes)
        .map_err(|_| fail("callback trait index is not a canonical replayable index"))?;
    index
        .require_package_identity(
            package.name,
            package.version,
            package.source_sha256,
            package.target,
            package.feature_digest,
        )
        .and_then(|_| index.require_cargo_alias_identity(package.cargo_alias))
        .and_then(|_| index.require_stable_compiler_identity(package.stable_rustc_version))
        .map_err(|_| {
            fail("callback trait package, alias, target, features, or compiler identity differs")
        })?;
    if index.digest() != selection.index_digest {
        return Err(fail("callback trait selected index digest is stale"));
    }
    let (trait_path, method) = selection
        .method_path
        .rsplit_once("::")
        .ok_or_else(|| fail("callback trait method path is incomplete"))?;
    let (error_owner, error) = selection
        .error_type_path
        .rsplit_once("::")
        .ok_or_else(|| fail("callback associated error path is incomplete"))?;
    if error_owner != trait_path
        || trait_path.split("::").next() != Some(package.cargo_alias)
        || !plain_identifier(method)
        || !plain_identifier(error)
        || semaprax::native_rust_binding::rust_api_path_tokens(trait_path).is_none()
    {
        return Err(fail(
            "callback method and associated error must belong to the same selected trait",
        ));
    }
    let find = |wanted: &str| {
        index
            .items()
            .iter()
            .find(|item| item.path == wanted)
            .ok_or_else(|| fail("callback trait selected member is absent"))
    };
    let selected_method = find(&selection.method_path)?;
    let selected_error = find(&selection.error_type_path)?;
    let owner = index
        .types()
        .iter()
        .find(|ty| ty.path == trait_path)
        .ok_or_else(|| fail("callback trait has no retained nominal type record"))?;
    if owner.kind != TypeRecordKind::Trait
        || owner.visibility != Visibility::Public
        || !owner.generics.parameters.is_empty()
        || !owner.generics.where_predicates.is_empty()
        || owner.references.as_slice() != [selection.error_type_path.as_str()]
    {
        return Err(fail("callback trait must be public, monomorphic, and have only the selected error obligation"));
    }
    if !admitted_member(selected_method, trait_path)
        || !admitted_member(selected_error, trait_path)
        || !selected_error.closure_complete
        || !closed_method_graph(
            &index,
            selected_method,
            trait_path,
            &selection.error_type_path,
        )
        || selected_method.kind != ItemKind::TraitMethod
        || selected_method.receiver != Receiver::Mutable
        || selected_error.kind != ItemKind::AssociatedType
        || selected_error.receiver != Receiver::None
        || selected_error.signature != format!("type {error}")
        || selected_error
            .associated_type
            .as_ref()
            .is_none_or(|a| !a.bounds.is_empty() || a.default.is_some())
    {
        return Err(fail("callback trait members require a public unsealed mutable method and unbounded associated error"));
    }
    if !method_signature(&selected_method.signature, method, error) {
        return Err(fail(
            "callback trait method requires fn(&mut self,i64)->Result<i64,Self::Error>",
        ));
    }
    // An index may be deliberately selected/partial. Reject known extra members;
    // rustc remains the final check for requirements omitted from the index.
    let prefix = format!("{trait_path}::");
    if index.items().iter().any(|item| {
        item.path.starts_with(&prefix)
            && item.path != selection.method_path
            && item.path != selection.error_type_path
    }) {
        return Err(fail(
            "callback trait has an additional unsupported indexed member",
        ));
    }
    let callback = prepare_native_rust_result_callback(
        source,
        path,
        &NativeResultCallbackSelection {
            callback_id: selection.callback_id.clone(),
            trait_path: trait_path.into(),
            method: method.into(),
            error_type: error.into(),
        },
    )?;
    let mut transcript = String::new();
    for value in [
        &callback.source_revision,
        &selection.callback_id,
        index.digest(),
        &selection.method_path,
        &selection.error_type_path,
    ] {
        write!(transcript, "{}:{value}", value.len()).unwrap();
    }
    Ok(IndexedResultCallbackProjection {
        binding_identity: domain_digest(
            b"semaprax.indexed-result-callback.v1\0",
            transcript.as_bytes(),
        ),
        index_digest: index.digest().into(),
        method_path: selection.method_path.clone(),
        error_type_path: selection.error_type_path.clone(),
        callback,
    })
}

fn plain_identifier(value: &str) -> bool {
    value.len() <= 128
        && value
            .as_bytes()
            .first()
            .is_some_and(u8::is_ascii_alphabetic)
        && value
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'_')
}
fn admitted_member(item: &ApiItem, owner: &str) -> bool {
    item.visibility == Visibility::Public
        && item.generics.parameters.is_empty()
        && item.generics.where_predicates.is_empty()
        && item.type_roots.iter().any(|p| p == owner)
        && matches!(
            item.support,
            Support::Supported
                | Support::Rejected {
                    reason: RejectionReason::UnsupportedSignature
                }
        )
}
// The general index correctly calls external Result incomplete. Admit only
// this already closed Result<i64, Self::Error> shape, with the exact nominal
// trait/error graph and the standard Result leaf. Do not repair index facts.
fn closed_method_graph(index: &RustApiIndex, item: &ApiItem, owner: &str, error: &str) -> bool {
    if item.closure_complete {
        return true;
    }
    ["core::result::Result", "std::result::Result"]
        .iter()
        .any(|result| {
            let mut roots = vec![owner, *result];
            roots.sort_unstable();
            let mut reachable = vec![owner, error, *result];
            reachable.sort_unstable();
            item.type_roots == roots
                && item.reachable_types == reachable
                && index.types().iter().any(|ty| {
                    ty.path == *result
                        && ty.kind == TypeRecordKind::External
                        && ty.visibility == Visibility::External
                        && ty.references.is_empty()
                        && ty.generics.parameters.is_empty()
                        && ty.generics.where_predicates.is_empty()
                })
        })
}
fn method_signature(signature: &str, method: &str, error: &str) -> bool {
    let compact: String = signature
        .chars()
        .filter(|c| !c.is_ascii_whitespace())
        .collect();
    let Some(rest) = compact.strip_prefix(&format!("fn{method}(&mutself,")) else {
        return false;
    };
    let Some((argument, result)) = rest.split_once(":i64)->") else {
        return false;
    };
    if !plain_identifier(argument) {
        return false;
    }
    // `qualified_path` from the pinned rustdoc extractor spells an unqualified
    // Self projection `<Self as >::Error`. Accept only this exact closed form
    // or the equivalent prepared spelling, never arbitrary Rust syntax.
    ["Result", "core::result::Result", "std::result::Result"]
        .iter()
        .any(|base| {
            [format!("Self::{error}"), format!("<Selfas>::{error}")]
                .iter()
                .any(|associated| result == format!("{base}<i64,{associated}>"))
        })
}

#[cfg(test)]
#[path = "indexed_callback_tests.rs"]
mod tests;
