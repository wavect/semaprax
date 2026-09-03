//! Project admission for an entry closure that declares a Native Rust
//! callback.
//!
//! Every other Project v1 profile proves admission by deriving its Web target.
//! A closure that declares an `import rust fn` has no Web target: WebAssembly
//! rejects native Rust imports, and the ordinary native backend cannot lower a
//! callback call site either. Its only consumer is the generated C and safe
//! Rust bridge, which the Native Rust SDK builder renders from linked HIR
//! rather than from a compiled target.
//!
//! So this admission proves the facts the Project itself promises — that every
//! selected export is an explicitly identified monomorphic scalar function
//! whose effects are covered by the declared callbacks — and derives no target
//! bytes and no scalar WIT descriptor. The SDK builder keeps its own, narrower
//! export and import selection rules; this boundary does not restate them.

use std::collections::{BTreeMap, BTreeSet};

use crate::diagnostic::Diagnostic;
use crate::hir::{IdentityOrigin, OwnershipMode, ResolvedProgram, ResolvedType};

/// The exact parameter bound shared with the Native Rust interoperability
/// profile.
const MAX_PARAMETERS: usize = 8;

fn admission_error(message: impl Into<String>) -> Diagnostic {
    Diagnostic::io("SPX-J117", message)
}

/// Reports whether the linked entry closure declares a Native Rust callback.
///
/// This selects the admission route. It reads declarations rather than call
/// sites so a declared but uncalled callback is admitted the same way, which
/// keeps admission agreeing with Graph v25 selection.
pub(super) fn declares_callback(program: &ResolvedProgram) -> bool {
    program
        .interfaces
        .iter()
        .flat_map(|interface| &interface.imports)
        .any(|import| import.native_rust)
}

const fn scalar(ty: &ResolvedType) -> bool {
    matches!(ty, ResolvedType::I64 | ResolvedType::Bool)
}

/// Admit one Native Rust callback Project without deriving any target.
pub(super) fn prepare(program: &ResolvedProgram, selected: &[String]) -> Result<(), Diagnostic> {
    crate::hir::validate(program)?;
    if !declares_callback(program) {
        return Err(admission_error(
            "Native Rust callback admission requires a declared `import rust fn`",
        ));
    }
    if selected.is_empty() {
        return Err(admission_error(
            "Native Rust callback Project selects no exports",
        ));
    }

    // Effects are admitted exactly when a declared callback grants them. The
    // workspace linker already holds this over the whole retained closure; the
    // repeat here keeps the Project boundary self-evident rather than implied.
    let granted = program
        .interfaces
        .iter()
        .flat_map(|interface| &interface.imports)
        .filter(|import| import.native_rust)
        .flat_map(|import| &import.effects)
        .map(String::as_str)
        .collect::<BTreeSet<_>>();

    let functions = program
        .functions
        .iter()
        .map(|function| (function.id.as_str(), function))
        .collect::<BTreeMap<_, _>>();
    let mut seen = BTreeSet::new();
    let mut previous: Option<&str> = None;
    for stable_id in selected {
        if !seen.insert(stable_id.as_str()) {
            return Err(admission_error(format!(
                "Native Rust callback export `{stable_id}` is selected more than once"
            )));
        }
        if previous.is_some_and(|prior| prior.as_bytes() >= stable_id.as_bytes()) {
            return Err(admission_error(
                "Native Rust callback export identities are not in canonical manifest order",
            ));
        }
        previous = Some(stable_id);
        let function = functions.get(stable_id.as_str()).copied().ok_or_else(|| {
            admission_error(format!(
                "Native Rust callback export `{stable_id}` does not name a monomorphic function"
            ))
        })?;
        if program
            .declarations
            .declaration(&function.id)
            .map(|declaration| declaration.identity_origin)
            != Some(IdentityOrigin::Explicit)
        {
            return Err(admission_error(format!(
                "Native Rust callback export `{stable_id}` must have an explicit identity"
            )));
        }
        if function.params.len() > MAX_PARAMETERS {
            return Err(admission_error(format!(
                "Native Rust callback export `{stable_id}` exceeds the {MAX_PARAMETERS}-parameter limit"
            )));
        }
        if function
            .params
            .iter()
            .any(|parameter| parameter.ownership != OwnershipMode::Value)
        {
            return Err(admission_error(format!(
                "Native Rust callback export `{stable_id}` has a non-value parameter"
            )));
        }
        if function
            .params
            .iter()
            .any(|parameter| !scalar(&parameter.ty))
            || !scalar(&function.return_type)
        {
            return Err(admission_error(format!(
                "Native Rust callback export `{stable_id}` has a non-scalar signature"
            )));
        }
        if let Some(effect) = function
            .effects
            .iter()
            .find(|effect| !granted.contains(effect.as_str()))
        {
            return Err(admission_error(format!(
                "Native Rust callback export `{stable_id}` declares effect `{effect}` that no declared callback grants"
            )));
        }
    }
    Ok(())
}
