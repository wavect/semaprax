//! Exact C member paths for authenticated nested record cleanup leaves.

use std::collections::BTreeMap;

use crate::cleanup_plan::CleanupTransition;
use crate::diagnostic::Diagnostic;
use crate::hir::{DeclarationId, ExpressionId};

pub(super) fn c_field_path(path: &[DeclarationId]) -> Result<String, Diagnostic> {
    if path.is_empty() {
        return Err(super::error(
            "nested owned Bytes leaf has an empty field path",
        ));
    }
    Ok(path
        .iter()
        .map(crate::codegen::native_emit::c_field_symbol)
        .collect::<Vec<_>>()
        .join("."))
}

pub(super) fn authenticate_transfers_at(
    plan: &super::NativeBytesPlan,
    at: &ExpressionId,
) -> Result<String, Diagnostic> {
    // Several canonical transfers may share an expression boundary. Treat
    // them as one ordered transaction: later transfers can consume a slot
    // produced by an earlier transfer, so checking every source against the
    // pre-transaction runtime state would reject a valid chain.
    let mut initial = BTreeMap::<String, (bool, String)>::new();
    let mut simulated = BTreeMap::<String, bool>::new();
    for transition in plan.transitions.get(at).into_iter().flatten() {
        let CleanupTransition::Transfer {
            source,
            destination,
            ..
        } = transition
        else {
            continue;
        };
        for (source, destination) in plan.transfer_pairs(source, destination)? {
            if source.flag == destination.flag {
                return Err(super::error(
                    "record transfer transaction aliases source and destination",
                ));
            }
            let source_state = simulated.get(&source.flag).copied();
            if source_state == Some(false) {
                return Err(super::error(
                    "record transfer transaction consumes a simulated dead source",
                ));
            }
            if source_state.is_none() {
                initial
                    .entry(source.flag.clone())
                    .or_insert_with(|| (true, source.value.clone()));
            }
            let destination_state = simulated.get(&destination.flag).copied();
            if destination_state == Some(true) {
                return Err(super::error(
                    "record transfer transaction overwrites a simulated live destination",
                ));
            }
            if destination_state.is_none() {
                initial
                    .entry(destination.flag.clone())
                    .or_insert_with(|| (false, destination.value.clone()));
            }
            simulated.insert(source.flag.clone(), false);
            simulated.insert(destination.flag.clone(), true);
        }
    }
    let mut output = String::new();
    for (flag, (must_be_live, value)) in initial {
        let failed = if must_be_live {
            format!("!{flag}")
        } else {
            flag
        };
        let state = if must_be_live { "live" } else { "dead" };
        output.push_str(&format!(
            "if ({failed}) spx_runtime_invariant_failure(\"record transfer preflight requires {state} {value}\");\n"
        ));
    }
    Ok(output)
}
