//! Offline stable-ID semantic package compatibility evidence v1.

use std::collections::BTreeSet;

use crate::diagnostic::Diagnostic;
use serde_json::Value;

use super::model::{Authenticated, Finding};
use super::wire::{charge, digest, limit_error};

mod api;
mod exports;
mod targets;
mod types;

pub use api::{
    generate, verify, CompatibilityInput, CompatibilityOptions, VerifiedEvidence, MAX_FINDINGS,
    MAX_INPUT_BYTES, MAX_JSON_DEPTH, MAX_OUTPUT_BYTES, MAX_WORK_UNITS, SCHEMA,
};
pub(super) use api::{DIGEST_DOMAIN, INPUT_DOMAIN};
use exports::compare_export;
use targets::compare_targets;
use types::{reachable_shared_types, scrub_type_display};

fn compare(
    base: &Authenticated,
    candidate: &Authenticated,
    work: &mut usize,
) -> Result<(&'static str, Vec<Finding>), Diagnostic> {
    let mut findings = Vec::new();
    let mut breaking = false;
    let mut indeterminate = false;
    if base.coordinate.package != candidate.coordinate.package {
        indeterminate = true;
        push(
            &mut findings,
            "informational",
            "identity",
            "package",
            &base.coordinate.package,
            &candidate.coordinate.package,
            "package_identity_mismatch",
        )?;
    }
    if base.context != candidate.context {
        indeterminate = true;
        push(
            &mut findings,
            "informational",
            "lock_context",
            "dependency_graph",
            "present",
            "changed",
            "authenticated_lock_context_drift",
        )?;
    }
    if base
        .lock_targets
        .values()
        .any(|status| status == "unproven")
        || candidate
            .lock_targets
            .values()
            .any(|status| status == "unproven")
    {
        indeterminate = true;
        push(
            &mut findings,
            "informational",
            "lock_targets",
            "aggregate",
            "unproven",
            "unproven",
            "dependency_target_coverage_unproven",
        )?;
    }
    if base.report.unproven || candidate.report.unproven {
        indeterminate = true;
        push(
            &mut findings,
            "informational",
            "coverage",
            "report",
            "proven",
            "unproven",
            "unproven_fact_inventory",
        )?;
    }
    if base.report.call_contract || candidate.report.call_contract {
        indeterminate = true;
        push(
            &mut findings,
            "informational",
            "contracts",
            "transitive_calls",
            "uncovered",
            "uncovered",
            "callee_semantics_not_closed",
        )?;
    }
    if base.report.imported_resource || candidate.report.imported_resource {
        indeterminate = true;
        push(
            &mut findings,
            "informational",
            "types",
            "imported_resource",
            "uncovered",
            "uncovered",
            "import_abi_closure_unproven",
        )?;
    }
    let export_ids = base
        .report
        .exports
        .keys()
        .chain(candidate.report.exports.keys())
        .cloned()
        .collect::<BTreeSet<_>>();
    let shared = base
        .report
        .exports
        .keys()
        .filter(|id| candidate.report.exports.contains_key(*id))
        .cloned()
        .collect::<BTreeSet<_>>();
    for id in export_ids {
        charge(work, 1)?;
        match (
            base.report.exports.get(&id),
            candidate.report.exports.get(&id),
        ) {
            (Some(_), None) => {
                breaking = true;
                push(
                    &mut findings,
                    "breaking",
                    "exports",
                    &id,
                    "present",
                    "absent",
                    "stable_export_removed",
                )?
            }
            (None, Some(_)) => push(
                &mut findings,
                "nonbreaking",
                "exports",
                &id,
                "absent",
                "present",
                "stable_export_added",
            )?,
            (Some(left), Some(right)) => {
                compare_export(&id, left, right, &mut findings, &mut breaking)?
            }
            _ => {}
        }
    }
    let reachable = reachable_shared_types(&shared, &base.report);
    for id in reachable {
        charge(work, 1)?;
        match (base.report.types.get(&id), candidate.report.types.get(&id)) {
            (Some(left), Some(right)) => {
                let left_semantic = scrub_type_display(left.clone());
                let right_semantic = scrub_type_display(right.clone());
                if left_semantic != right_semantic {
                    breaking = true;
                    push(
                        &mut findings,
                        "breaking",
                        "types",
                        &id,
                        &fact(left),
                        &fact(right),
                        "reachable_nominal_definition_changed",
                    )?
                } else if left != right {
                    push(
                        &mut findings,
                        "informational",
                        "display",
                        &id,
                        &fact(left),
                        &fact(right),
                        "identity_backed_display_name_changed",
                    )?;
                }
            }
            (Some(left), None) => {
                breaking = true;
                push(
                    &mut findings,
                    "breaking",
                    "types",
                    &id,
                    &fact(left),
                    "none",
                    "reachable_nominal_definition_removed",
                )?
            }
            _ => {}
        }
    }
    compare_targets(
        &base.report.targets,
        &candidate.report.targets,
        &mut findings,
        &mut breaking,
        &mut indeterminate,
    )?;
    let base_lock_targets = base
        .lock_targets
        .iter()
        .map(|(key, value)| (format!("lock:{key}"), value.clone()))
        .collect();
    let candidate_lock_targets = candidate
        .lock_targets
        .iter()
        .map(|(key, value)| (format!("lock:{key}"), value.clone()))
        .collect();
    compare_targets(
        &base_lock_targets,
        &candidate_lock_targets,
        &mut findings,
        &mut breaking,
        &mut indeterminate,
    )?;
    if findings.len() > MAX_FINDINGS {
        return Err(limit_error("findings exceed limit"));
    }
    Ok((
        if indeterminate {
            "indeterminate"
        } else if breaking {
            "breaking"
        } else {
            "compatible"
        },
        findings,
    ))
}

fn push(
    rows: &mut Vec<Finding>,
    classification: &'static str,
    axis: &'static str,
    subject: &str,
    before: &str,
    after: &str,
    reason: &'static str,
) -> Result<(), Diagnostic> {
    if rows.len() >= MAX_FINDINGS {
        return Err(limit_error("findings exceed limit"));
    }
    rows.push(Finding {
        classification,
        axis,
        subject: subject.to_owned(),
        before: before.to_owned(),
        after: after.to_owned(),
        reason,
    });
    Ok(())
}
fn strings(value: &Value) -> BTreeSet<String> {
    value
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
        .map(str::to_owned)
        .collect()
}
fn fact(value: &Value) -> String {
    digest(INPUT_DOMAIN, value.to_string().as_bytes())
}
#[cfg(test)]
#[path = "../tests.rs"]
mod tests;
