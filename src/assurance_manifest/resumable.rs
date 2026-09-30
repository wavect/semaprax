//! Source contract ownership in the bounded compiler-generated transition plan.
//! Uses the existing manifest method vocabulary; this is not target evidence.
//!
//! Covers both admitted `.spx` lanes (issue #296): the sequential plan (direct
//! top-level `yield` sites, `resumable_effects::lowering::lower_sequential`)
//! and the control-dependent plan (`yield` inside `if`/`else`/`while`,
//! `resumable_effects::lowering::control::lower_control`). The two are
//! distinguished the same way `source_signature::derive_source_effect_signature`
//! already does, by `control::is_control_dependent`, so this module cannot
//! silently drift from which lane the compiler itself selected. A
//! control-dependent plan has no yield-free backend projection to validate
//! (`SPX-H006`), so its `target` field stays absent rather than naming one
//! that does not exist; its `bounds` records the dynamic suspension count and
//! whether the plan carries an owned `Bytes` local across a suspension
//! (issue #296 section 11.6), since those are the two facts that change its
//! plan identity domain (v3 vs v4).

use std::collections::BTreeMap;
use std::path::Path;

use crate::diagnostic::Diagnostic;
use crate::hir::{ResolvedFunction, ResolvedProgram};
use crate::resumable_effects::lowering::{
    control::{is_control_dependent, lower_control},
    lower_sequential,
};

use super::{obligation_id, AssuranceClass, MethodRecord, Obligation, ObligationKind};

const TOOL: &str = "semaprax-resumable-contract-placement.v1";

/// One method record per `requires`/`ensures` clause, for both the "start"
/// (entry to first suspension) and "final_resume" (last suspension to
/// completion) transitions. Shared by the sequential and control-dependent
/// lanes below: only the identities and bounds differ between them.
#[allow(clippy::too_many_arguments)]
fn insert_transition_obligations(
    result: &mut BTreeMap<String, MethodRecord>,
    function: &ResolvedFunction,
    plan_identity: &[u8; 32],
    entry_id: &str,
    first_suspended_id: &str,
    last_suspended_id: &str,
    complete_id: &str,
    bounds: String,
    target: Option<&str>,
) {
    for (kind, prefix, count, role, from, to) in [
        (
            ObligationKind::Precondition,
            "require",
            function.requires.len(),
            "start",
            entry_id,
            first_suspended_id,
        ),
        (
            ObligationKind::Postcondition,
            "ensure",
            function.ensures.len(),
            "final_resume",
            last_suspended_id,
            complete_id,
        ),
    ] {
        for index in 0..count {
            let mut method = MethodRecord::new(
                AssuranceClass::RuntimeGuarded,
                TOOL,
                env!("CARGO_PKG_VERSION"),
            );
            method.inputs = vec![
                function.id.as_str().to_owned(),
                format!(
                    "plan:sha256:{:x}",
                    crate::digest_hex::LowerHex(plan_identity)
                ),
                role.to_owned(),
                from.to_owned(),
                to.to_owned(),
            ];
            method.bounds = Some(bounds.clone());
            method.runtime_fallback = true;
            method.target = target.map(str::to_owned);
            method.detail = Some("source clause owned by this checked transition; runtime guard only; no target execution, durable runtime, handler or resume authority".to_owned());
            result.insert(
                obligation_id(kind, function.id.as_str(), &format!("{prefix}:{index}")),
                method,
            );
        }
    }
}

fn methods(program: &ResolvedProgram) -> Result<BTreeMap<String, MethodRecord>, Diagnostic> {
    let mut result = BTreeMap::new();
    for function in &program.functions {
        if function.yields.is_none()
            || (function.requires.is_empty() && function.ensures.is_empty())
        {
            continue;
        }
        if is_control_dependent(function) {
            let plan = lower_control(program, function)?;
            // Prove the closed plan can actually be built: `lower_control`
            // itself rebuilds every site's loan/cleanup attachments and
            // carried-locals proof. There is no yield-free projection to
            // separately validate for this lane (`SPX-H006`).
            let bounds = format!(
                "control_dependent_copy_scalar_yields:{}{}",
                plan.sites.len(),
                if plan.carries_owned_bytes {
                    ":carries_owned_bytes"
                } else {
                    ""
                }
            );
            insert_transition_obligations(
                &mut result,
                function,
                plan.identity.as_bytes(),
                plan.entry.id.as_str(),
                plan.sites[0].state.id.as_str(),
                plan.sites.last().unwrap().state.id.as_str(),
                plan.complete.id.as_str(),
                bounds,
                None,
            );
        } else {
            let plan = lower_sequential(program, function)?;
            // Prove the closed projections can actually be built and validated;
            // a plan's state labels alone do not justify contract placement.
            plan.start_program(program)?;
            for index in 0..plan.suspensions.len() {
                plan.resume_program_at(program, index)?;
            }
            let bounds = format!("sequential_copy_scalar_yields:{}", plan.suspensions.len());
            insert_transition_obligations(
                &mut result,
                function,
                plan.identity.as_bytes(),
                plan.entry.id.as_str(),
                plan.suspensions[0].state.id.as_str(),
                plan.suspensions.last().unwrap().state.id.as_str(),
                plan.complete.id.as_str(),
                bounds,
                Some("resumable_yield_free_projection"),
            );
        }
    }
    Ok(result)
}

pub(super) fn attach(
    program: &ResolvedProgram,
    obligations: &mut [Obligation],
) -> Result<(), Diagnostic> {
    for (id, method) in methods(program)? {
        let obligation = obligations
            .iter_mut()
            .find(|obligation| obligation.id == id)
            .ok_or_else(|| drift("resumable source contract obligation is absent"))?;
        obligation.methods.push(method);
    }
    Ok(())
}

fn drift(message: &str) -> Diagnostic {
    Diagnostic::io("SPX-Z104", message.to_owned())
}

pub(super) fn verify_source(envelope: &str, source: &str, path: &Path) -> Result<(), Diagnostic> {
    let parsed = crate::parse(source, path)?;
    let mut expected = if parsed
        .functions
        .iter()
        .any(|function| function.yields.is_some())
    {
        let program = crate::hir::resolve(&parsed)
            .map_err(|_| drift("resumable assurance source no longer resolves"))?;
        methods(&program).map_err(|_| drift("resumable assurance source no longer lowers"))?
    } else {
        BTreeMap::new()
    };
    let source_obligations = super::derive::derive_obligations(&parsed)
        .into_iter()
        .filter(|obligation| expected.contains_key(&obligation.id))
        .map(|obligation| (obligation.id.clone(), obligation))
        .collect::<BTreeMap<_, _>>();
    let value: serde_json::Value = serde_json::from_str(envelope)
        .map_err(|_| drift("resumable assurance envelope is malformed"))?;
    let obligations = value["payload"]["obligations"]
        .as_array()
        .ok_or_else(|| drift("resumable assurance obligations are absent"))?;
    for obligation in obligations {
        let id = obligation["id"].as_str().unwrap_or_default();
        let records = obligation["methods"]
            .as_array()
            .ok_or_else(|| drift("resumable assurance methods are absent"))?;
        let recorded = records
            .iter()
            .enumerate()
            .filter(|(_, method)| method["tool"].as_str() == Some(TOOL))
            .collect::<Vec<_>>();
        match expected.remove(id) {
            Some(method) => {
                let owner = source_obligations
                    .get(id)
                    .ok_or_else(|| drift("resumable source contract obligation is absent"))?;
                if obligation["declaration_id"].as_str() != Some(owner.declaration_id.as_str())
                    || obligation["kind"].as_str() != Some(owner.kind.token())
                {
                    return Err(drift(
                        "resumable contract obligation owner or kind differs from checked source",
                    ));
                }
                let canonical: serde_json::Value =
                    serde_json::from_str(&super::render::render_method(&method))
                        .map_err(|_| drift("resumable assurance method cannot render"))?;
                if recorded.len() != 1 || recorded[0].0 != 1 || recorded[0].1 != &canonical {
                    return Err(drift(
                        "resumable contract transition binding differs from checked source",
                    ));
                }
            }
            None if !recorded.is_empty() => {
                return Err(drift("unexpected resumable contract transition binding"))
            }
            None => {}
        }
    }
    if !expected.is_empty() {
        return Err(drift("resumable contract transition obligation is missing"));
    }
    Ok(())
}

#[cfg(test)]
mod tests;
