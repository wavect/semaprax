use std::collections::{BTreeMap, BTreeSet};

use crate::diagnostic::Diagnostic;

use super::{push, Finding};

pub(super) fn compare_targets(
    left: &BTreeMap<String, String>,
    right: &BTreeMap<String, String>,
    findings: &mut Vec<Finding>,
    breaking: &mut bool,
    indeterminate: &mut bool,
) -> Result<(), Diagnostic> {
    let keys = left
        .keys()
        .chain(right.keys())
        .cloned()
        .collect::<BTreeSet<_>>();
    for key in keys {
        let l = left.get(&key).map(String::as_str).unwrap_or("unknown");
        let r = right.get(&key).map(String::as_str).unwrap_or("unknown");
        if l == r {
            if l == "unproven" {
                *indeterminate = true;
                push(
                    findings,
                    "informational",
                    "targets",
                    &key,
                    l,
                    r,
                    "target_unproven",
                )?;
            }
            continue;
        }
        if matches!(l, "unproven" | "unknown") || matches!(r, "unproven" | "unknown") {
            *indeterminate = true;
            push(
                findings,
                "informational",
                "targets",
                &key,
                l,
                r,
                "target_transition_unproven",
            )?;
        } else if l == "available" && r == "unavailable" {
            *breaking = true;
            push(
                findings,
                "breaking",
                "targets",
                &key,
                l,
                r,
                "target_became_unavailable",
            )?;
        } else if l == "unavailable" && r == "available" {
            push(
                findings,
                "nonbreaking",
                "targets",
                &key,
                l,
                r,
                "target_became_available",
            )?;
        } else {
            *indeterminate = true;
            push(
                findings,
                "informational",
                "targets",
                &key,
                l,
                r,
                "unknown_target_transition",
            )?;
        }
    }
    Ok(())
}
