//! Actual lexical producer parity and whole predecessor custody.
use super::*;
use crate::bounded_output::with_limit_usage_refusal;
use sha2::{Digest, Sha256};
use std::mem::size_of;

fn function() -> ResolvedFunction {
    crate::hir::resolve(
        &crate::check("module edge; @id(\"main\") fn main()->i64 {0}", "edge.spx").unwrap(),
    )
    .unwrap()
    .functions
    .remove(0)
}
fn state() -> PathState {
    PathState {
        renewals: BTreeMap::new(),
        live_order: Vec::new(),
        conditional_variants: Vec::new(),
        pending_failure: None,
        selected_failure: None,
        staged_copy_result: None,
        published: false,
        returned: None,
    }
}
// This test-only adapter receives the literal production expansion; production
// does not call a bool/Result wrapper or move its Box into a helper.
fn lexical(
    function: &ResolvedFunction,
    mut state: Box<PathState>,
    condition: &EdgeCondition,
    sources: &BTreeMap<ExpressionId, StatusSourceId>,
) -> Result<Box<PathState>, Diagnostic> {
    boxed_flow::held_edge!(function, state, condition, sources);
    Ok(state)
}

#[test]
fn lexical_edge_keeps_every_condition_same_allocation_and_complete_old_error() {
    let f = function();
    let status = StatusSourceId {
        expression: f.body.id.clone(),
        lane: StatusLane::OperationFailure,
    };
    let mut sources = BTreeMap::new();
    sources.insert(f.body.id.clone(), status.clone());
    for condition in [
        EdgeCondition::Always,
        EdgeCondition::BooleanResult(f.body.id.clone(), false),
        EdgeCondition::BooleanResult(f.body.id.clone(), true),
        EdgeCondition::StatusZero(status.clone()),
        EdgeCondition::StatusNonzero(status.clone()),
        EdgeCondition::VariantCase {
            scrutinee: f.body.id.clone(),
            case: f.id.clone(),
            matches: true,
        },
        EdgeCondition::ArmSelected {
            scrutinee: f.body.id.clone(),
            arm: 0,
            selected: false,
        },
    ] {
        for flags in 0..8 {
            let mut input = state();
            input.pending_failure = (flags & 1 != 0).then(|| status.clone());
            input.selected_failure = (flags & 2 != 0).then(|| status.clone());
            input.published = flags & 4 != 0;
            let mut budget = ReplayBudget::unobserved();
            let expected = held_edge_before::state_for_edge(
                &f,
                Box::new(input.clone()),
                &condition,
                &sources,
                &mut budget,
            );
            let owned = Box::new(input);
            let pointer = std::ptr::from_ref(owned.as_ref());
            let actual = lexical(&f, owned, &condition, &sources);
            if let Ok(value) = &actual {
                assert_eq!(std::ptr::from_ref(value.as_ref()), pointer);
            }
            assert_eq!(actual.map(|v| *v), expected.map(|v| *v));
        }
    }
}

#[test]
fn only_removed_forwarding_roles_leave_the_block_invoice_with_one_terminal_bank() {
    let f = function();
    let old_tuple = size_of::<(
        &ResolvedFunction,
        Box<PathState>,
        &EdgeCondition,
        &BTreeMap<ExpressionId, StatusSourceId>,
        &mut ReplayBudget,
        Result<Box<PathState>, Diagnostic>,
        &ExpressionId,
        &StatusSourceId,
        StatusSourceId,
        Option<&StatusSourceId>,
        bool,
    )>();
    let new_tuple = size_of::<(
        &ResolvedFunction,
        &EdgeCondition,
        &BTreeMap<ExpressionId, StatusSourceId>,
        &mut ReplayBudget,
        &ExpressionId,
        &StatusSourceId,
        StatusSourceId,
        Option<&StatusSourceId>,
        bool,
    )>();
    let removed = 2 * size_of::<Result<(), Diagnostic>>() + old_tuple - new_tuple;
    assert_eq!(
        boxed_flow::HELD_EDGE_TERMINAL,
        2 * size_of::<Diagnostic>() + 2 * size_of::<Box<PathState>>()
    );
    for blocks in [1, 2, 8, 64] {
        let invoice = |old| {
            let ((), refused, paid, first) = with_limit_usage_refusal(1 << 24, || {
                let mut budget = ReplayBudget::new();
                if old {
                    held_edge_before::reserve_directory(&f, 0, &mut budget).unwrap();
                } else {
                    boxed_flow::reserve_directory(&f, 0, &mut budget).unwrap();
                }
                for _ in 0..blocks {
                    if old {
                        held_edge_before::reserve_block(&f, &mut budget).unwrap();
                    } else {
                        budget
                            .reserve_bytes(
                                &f,
                                boxed_flow::BLOCK_CONTROLS,
                                "ordinary frontier block and singleton receiving",
                            )
                            .unwrap();
                    }
                }
                drop(budget);
            });
            assert!(!refused && first.is_none());
            paid
        };
        let before = invoice(true);
        let current = invoice(false);
        assert_eq!(
            current + blocks * removed,
            before + boxed_flow::HELD_EDGE_TERMINAL
        );
        assert!(current < before);
    }
}

fn digest(bytes: &[u8]) -> String {
    format!("{:x}", crate::digest_hex::LowerHex(Sha256::digest(bytes)))
}
#[test]
fn whole_pre_inline_driver_helpers_and_original_direct_tests_are_retained_exactly() {
    assert_eq!(
        digest(include_bytes!("held_edge_inverse.json")),
        "e79da7f5d7256d5b4d4284870d38610c25698ebe57a7f53e10677baa7ccb9b7c"
    );
    let rows: serde_json::Value =
        serde_json::from_str(include_str!("held_edge_inverse.json")).unwrap();
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    for row in rows.as_array().unwrap() {
        let mut actual = std::fs::read(root.join(row["path"].as_str().unwrap())).unwrap();
        assert_eq!(actual.len() as u64, row["length"].as_u64().unwrap());
        assert_eq!(digest(&actual), row["sha256"].as_str().unwrap());
        for edit in row["edits"].as_array().unwrap().iter().rev() {
            let at = edit[0].as_u64().unwrap() as usize;
            let current = edit[1].as_str().unwrap().as_bytes();
            assert_eq!(&actual[at..at + current.len()], current);
            actual.splice(at..at + current.len(), edit[2].as_str().unwrap().bytes());
        }
        assert_eq!(actual.len() as u64, row["before_length"].as_u64().unwrap());
        assert_eq!(digest(&actual), row["before_sha256"].as_str().unwrap());
        if let Some(path) = row["full_before"].as_str() {
            assert_eq!(actual, std::fs::read(root.join(path)).unwrap());
        }
    }
}
