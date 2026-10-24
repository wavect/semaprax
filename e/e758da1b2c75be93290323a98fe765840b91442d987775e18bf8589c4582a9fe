//! Actual Diagnostic-bearing production entry, separately from complete MAIN helpers.
use super::*;
use crate::bounded_output::{
    active_remaining, reserve_active_required, set_active_floor, with_limit_usage_refusal,
};
use crate::test_allocation_observer::{current_metrics, measure};
use std::cell::Cell;
thread_local! { static DROPS: Cell<usize> = const { Cell::new(0) }; }
#[derive(Eq, PartialEq, Ord, PartialOrd)]
struct State([u64; 50]);
impl Drop for State {
    fn drop(&mut self) {
        DROPS.with(|n| n.set(n.get() + 1));
    }
}
fn function() -> ResolvedFunction {
    let parsed = crate::check(
        "module entry.receiver; @id(\"main\") fn main()->i64 {0}",
        "entry.spx",
    )
    .unwrap();
    crate::hir::resolve(&parsed).unwrap().functions.remove(0)
}
fn bootstrap(function: &ResolvedFunction) -> usize {
    function.id.as_str().len() + 512
}
fn owned() -> usize {
    btree_node_upper::<Box<State>, ()>().unwrap()
        + size_of::<State>()
        + 2 * size_of::<Box<State>>()
        + Insertions::<State>::entry_controls()
}
fn held() -> usize {
    btree_node_upper::<Box<State>, ()>().unwrap() + Insertions::<State>::entry_controls()
}

#[test]
fn actual_diagnostic_owner_zero_exact_and_each_short_frontier_keep_full_drop() {
    let function = function();
    let acquisition = bootstrap(&function)
        + Insertions::<State>::controls()
        + Insertions::<State>::entry_controls();
    let exact = acquisition + owned();
    for limit in [0, acquisition - 1, exact - 1, exact] {
        let ((success, allocation), refused, paid, first) = with_limit_usage_refusal(limit, || {
            measure(|| {
                let mut budget = ReplayBudget::new();
                let mut states = BTreeSet::new();
                let success = match Insertions::<State>::acquire(&function, &mut budget) {
                    Err(error) => {
                        drop(error);
                        false
                    }
                    Ok(mut receiver) => {
                        receiver.insert_owned(&function, &mut states, State([7; 50]), &mut budget);
                        if receiver.failed {
                            assert!(states.is_empty());
                            let allocated = current_metrics().allocated;
                            receiver.insert_owned(
                                &function,
                                &mut states,
                                State([8; 50]),
                                &mut budget,
                            );
                            assert!(receiver.failed && states.is_empty());
                            assert_eq!(current_metrics().allocated, allocated);
                            drop(receiver.failure());
                            false
                        } else {
                            assert_eq!(states.first().unwrap().0, [7; 50]);
                            true
                        }
                    }
                };
                drop(states);
                drop(budget);
                success
            })
        });
        assert_eq!(success, limit == exact);
        assert_eq!(refused, !success);
        assert_eq!(first.is_some(), !success);
        assert!(paid <= limit);
        if success {
            assert_eq!(paid, exact);
        }
        assert!(!allocation.invalid && allocation.live == 0);
        assert_eq!(allocation.allocated, allocation.freed);
        assert!(allocation.allocated <= paid);
    }
}

#[test]
fn actual_owned_held_and_duplicate_entries_pay_each_bank_and_preserve_box_identity() {
    let function = function();
    DROPS.with(|n| n.set(0));
    let (((), allocation), refused, paid, first) = with_limit_usage_refusal(usize::MAX, || {
        measure(|| {
            let mut budget = ReplayBudget::new();
            let mut receiver = Insertions::<State>::acquire(&function, &mut budget).unwrap();
            let mut source = BTreeSet::new();
            let mut target = BTreeSet::new();
            receiver.insert_owned(&function, &mut source, State([3; 50]), &mut budget);
            receiver.insert_owned(&function, &mut source, State([3; 50]), &mut budget);
            assert!(!receiver.failed);
            assert_eq!(source.len(), 1);
            assert_eq!(DROPS.with(Cell::get), 1);
            let value = source.pop_first().unwrap();
            let pointer = std::ptr::from_ref(value.as_ref());
            receiver.insert_held(&function, &mut target, value, &mut budget);
            assert_eq!(
                std::ptr::from_ref(target.first().unwrap().as_ref()),
                pointer
            );
            receiver.insert_owned(&function, &mut source, State([3; 50]), &mut budget);
            receiver.insert_held(
                &function,
                &mut target,
                source.pop_first().unwrap(),
                &mut budget,
            );
            assert!(!receiver.failed);
            assert_eq!(target.len(), 1);
            assert_eq!(DROPS.with(Cell::get), 2);
            drop(source);
            drop(target);
            drop(receiver);
            drop(budget);
            assert_eq!(DROPS.with(Cell::get), 3);
        })
    });
    assert!(!refused && first.is_none());
    assert_eq!(
        paid,
        bootstrap(&function)
            + Insertions::<State>::controls()
            + Insertions::<State>::entry_controls()
            + 2 * owned()
            + held()
            + 2 * Insertions::<State>::entry_controls()
    );
    assert!(!allocation.invalid && allocation.live == 0);
    assert_eq!(allocation.allocated, allocation.freed);
    assert!(allocation.allocated <= paid);
}

#[test]
fn duplicate_zero_auth_keeps_complete_main_diagnostic_and_first_receipt_at_sticky_floor() {
    let function = function();
    for frontier in 0..2 {
        let observe = |current: bool| {
            with_limit_usage_refusal(1 << 20, || {
                let mut budget = ReplayBudget::new();
                let mut receiver = Insertions::<State>::acquire(&function, &mut budget).unwrap();
                let mut states = BTreeSet::new();
                receiver.insert_owned(&function, &mut states, State([9; 50]), &mut budget);
                assert!(!receiver.failed);
                if frontier == 0 {
                    assert!(!reserve_active_required(active_remaining().unwrap() + 1));
                } else {
                    let remaining = active_remaining().unwrap();
                    assert!(set_active_floor(remaining));
                    let (_, _, spent, _) =
                        with_limit_usage_refusal(1, || assert!(reserve_active_required(1)));
                    assert_eq!(spent, 1);
                }
                let (diagnostic, allocation) = measure(|| {
                    let diagnostic = if current {
                        receiver.insert_owned(&function, &mut states, State([9; 50]), &mut budget);
                        assert!(receiver.failed);
                        receiver.failure()
                    } else {
                        before::insert_owned(&mut states, State([9; 50]), &mut |bytes, phase| {
                            budget.reserve_bytes(&function, bytes, phase)
                        })
                        .unwrap_err()
                    };
                    assert_eq!(states.len(), 1);
                    diagnostic
                });
                assert_eq!(allocation.allocated, 0);
                // The returned Diagnostic may own the preacquired refusal text;
                // its full Drop is covered by the whole-owner control above.
                diagnostic
            })
        };
        let (old, old_refused, old_paid, old_first) = observe(false);
        let (current, current_refused, current_paid, current_first) = observe(true);
        assert_eq!(current, old);
        assert_eq!(current_refused, old_refused);
        assert_eq!(current_paid, old_paid);
        assert_eq!(current_first, old_first);
    }
}

#[test]
fn foreign_function_budget_and_scope_poison_only_original_owner_and_never_restore_entry() {
    let function = function();
    let foreign_function = function.clone();
    for foreign in 0..3 {
        let (((), allocation), refused, _, first) = with_limit_usage_refusal(1 << 20, || {
            measure(|| {
                let mut budget = ReplayBudget::new();
                let mut receiver = Insertions::<State>::acquire(&function, &mut budget).unwrap();
                let mut states = BTreeSet::new();
                receiver.insert_owned(&function, &mut states, State([4; 50]), &mut budget);
                assert!(!receiver.failed);
                let allocated = current_metrics().allocated;
                match foreign {
                    0 => receiver.insert_owned(
                        &foreign_function,
                        &mut states,
                        State([4; 50]),
                        &mut budget,
                    ),
                    1 => {
                        let mut foreign_budget = ReplayBudget::new();
                        receiver.insert_owned(
                            &function,
                            &mut states,
                            State([4; 50]),
                            &mut foreign_budget,
                        );
                    }
                    _ => {
                        let (_, foreign_refused, foreign_paid, foreign_first) =
                            with_limit_usage_refusal(0, || {
                                let at = current_metrics().allocated;
                                receiver.insert_owned(
                                    &function,
                                    &mut states,
                                    State([4; 50]),
                                    &mut budget,
                                );
                                assert_eq!(current_metrics().allocated, at);
                            });
                        assert!(!foreign_refused && foreign_paid == 0 && foreign_first.is_none());
                    }
                }
                assert!(receiver.failed);
                assert_eq!(states.len(), 1);
                if foreign < 2 {
                    assert_eq!(current_metrics().allocated, allocated);
                }
                let allocated = current_metrics().allocated;
                receiver.insert_owned(&function, &mut states, State([5; 50]), &mut budget);
                assert!(receiver.failed);
                assert_eq!(states.len(), 1);
                assert_eq!(current_metrics().allocated, allocated);
                assert_eq!(
                    receiver.failure(),
                    Diagnostic::io("SPX-H006", String::new())
                );
                drop(states);
                drop(receiver);
                drop(budget);
            })
        });
        assert!(refused);
        assert_eq!(first.unwrap().requested, 0);
        assert!(!allocation.invalid && allocation.live == 0);
        assert_eq!(allocation.allocated, allocation.freed);
    }
}

#[test]
fn actual_entry_and_box_layout_inventory_and_complete_main_receiver_sources_are_exact() {
    let rows: serde_json::Value = serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/docs/evidence/boxed-state-receiving-849-20261016/source-inverses.json"
    )))
    .unwrap();
    let sources = [
        (
            "src/cleanup_plan/replay/path_join.rs",
            include_str!("../../path_join.rs"),
        ),
        (
            "src/cleanup_plan/replay/path_join/boxed_states.rs",
            include_str!("../boxed_states.rs"),
        ),
        (
            "src/bounded_output/classifier_string.rs",
            include_str!("../../../../bounded_output/classifier_string.rs"),
        ),
    ];
    use sha2::{Digest, Sha256};
    for (source, raw) in sources {
        let current = if source == "src/bounded_output/classifier_string.rs" {
            // Bind today's complete physical/compiled input before selecting
            // this guard's unchanged whole pre-StorageParent catalog stage.
            crate::source_reflows_merge::main_e177_final_inputs::value_direct(
                source,
                "88389f912f0bd6c275a8cee2bec309210bb0c575c7b449547606d8449dfdb9cc",
                raw,
            )
        } else {
            crate::source_reflows_merge::boxed849_input(source, raw)
        };
        let row = rows["rows"]
            .as_array()
            .unwrap()
            .iter()
            .find(|row| row["source"] == source)
            .unwrap();
        assert_eq!(
            format!(
                "{:x}",
                crate::digest_hex::LowerHex(Sha256::digest(current.as_bytes()))
            ),
            row["current_sha256"]
        );
        let mut previous = current;
        for edit in row["edits"].as_array().unwrap().iter().rev() {
            let new = edit["current"].as_str().unwrap();
            assert_eq!(previous.matches(new).count(), 1);
            previous = previous.replacen(new, edit["previous"].as_str().unwrap(), 1);
        }
        assert_eq!(previous, row["before"].as_str().unwrap());
    }
    let original = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/docs/evidence/boxed-state-receiving-849-20261016/boxed_states.before.rs.txt"
    ));
    let compiled = include_str!("before.rs");
    let expected = original
        .replace(
            "use super::super::physical_storage::btree_node_upper;",
            "use super::super::super::physical_storage::btree_node_upper;",
        )
        .replace("#[cfg(test)]\nmod tests;\n", "");
    // The whole independent receiver adds only this closed lint annotation;
    // rustfmt also removes one empty line at EOF. Every body byte stays exact.
    let lint = "#![allow(\n    clippy::duplicate_mod,\n    reason = \"The complete fixture intentionally compiles independently against current and predecessor owner types.\"\n)]\n";
    assert_eq!(compiled.matches(lint).count(), 1);
    assert!(expected.ends_with("\n\n"));
    let expected = format!("{lint}{}", expected.strip_suffix('\n').unwrap());
    assert_eq!(compiled, expected);
    let mut complete_main = rows["rows"]
        .as_array()
        .unwrap()
        .iter()
        .find(|row| row["source"] == "src/cleanup_plan/replay/path_join.rs")
        .unwrap()["before"]
        .as_str()
        .unwrap()
        .to_owned();
    for edit in rows["main_receiver_adaptations"].as_array().unwrap() {
        let old = edit["old"].as_str().unwrap();
        assert_eq!(complete_main.matches(old).count(), 1);
        complete_main = complete_main.replacen(old, edit["new"].as_str().unwrap(), 1);
    }
    // Restore only the one token gap removed by rustfmt in the receiving copy.
    let old_gap = "\n\n\nfn joined_ownership_state";
    let formatted_gap = "\n\nfn joined_ownership_state";
    assert_eq!(complete_main.matches(old_gap).count(), 1);
    complete_main = complete_main.replacen(old_gap, formatted_gap, 1);
    assert_eq!(include_str!("../before_receiving.rs"), complete_main);

    assert_eq!(
        owned(),
        btree_node_upper::<Box<State>, ()>().unwrap()
            + size_of::<State>()
            + 2 * size_of::<Box<State>>()
            + Insertions::<State>::entry_controls()
    );
    assert_eq!(
        held(),
        btree_node_upper::<Box<State>, ()>().unwrap() + Insertions::<State>::entry_controls()
    );
}

#[test]
fn complete_main_callback_and_current_receiving_replay_keep_full_success_error_work_and_costs() {
    let checked = crate::check(
        "module whole.entries; @id(\"f\") fn f(flag:bool)->i64 {if flag {7}else{9}} @id(\"main\") fn main()->i64 {f(false)}",
        "whole.spx",
    )
    .unwrap();
    let program = crate::hir::resolve(&checked).unwrap();
    let function = program
        .functions
        .iter()
        .find(|function| function.name == "f")
        .unwrap();
    let observe = |old: bool| {
        with_limit_usage_refusal(usize::MAX, || {
            measure(|| {
                let mut budget = ReplayBudget::new();
                let storage =
                    super::super::super::validate_slots_paid(function, &mut budget).unwrap();
                let leaves =
                    super::super::super::collect_leaves_paid(function, &mut budget).unwrap();
                let result: Result<(), Diagnostic> = if old {
                    super::super::before_receiving::validate_path_states(
                        &program,
                        function,
                        &storage,
                        &leaves,
                        &mut budget,
                        None,
                        None,
                    )
                } else {
                    super::super::validate_path_states(
                        &program,
                        function,
                        &storage,
                        &leaves,
                        &mut budget,
                        None,
                        None,
                    )
                };
                assert!(result.is_ok());
                let work = (budget.remaining, budget.skeleton_remaining);
                drop(result);
                drop(leaves);
                drop(storage);
                drop(budget);
                work
            })
        })
    };
    let ((old_work, old_allocation), old_refused, old_paid, old_first) = observe(true);
    let ((current_work, current_allocation), current_refused, current_paid, current_first) =
        observe(false);
    assert_eq!(old_work, current_work);
    assert!(!old_refused && !current_refused);
    assert!(old_first.is_none() && current_first.is_none());
    for (allocation, paid) in [
        (old_allocation, old_paid),
        (current_allocation, current_paid),
    ] {
        assert!(!allocation.invalid && allocation.live == 0);
        assert_eq!(allocation.allocated, allocation.freed);
        assert!(allocation.allocated <= paid);
    }
    // Honest small-case tuple: new receiving/last-bank charges may exceed MAIN.
    eprintln!("849 whole small callback old/current reserved={old_paid}/{current_paid}");
}

#[test]
fn compact_positive_results_amortize_the_once_paid_diagnostic_conversion_bank() {
    let function = function();
    let duplicate_entries = 64;
    let (((), allocation), refused, paid, first) = with_limit_usage_refusal(usize::MAX, || {
        measure(|| {
            let mut budget = ReplayBudget::new();
            let mut receiver = Insertions::<State>::acquire(&function, &mut budget).unwrap();
            let mut states = BTreeSet::new();
            receiver.insert_owned(&function, &mut states, State([3; 50]), &mut budget);
            let allocated = current_metrics().allocated;
            for _ in 0..duplicate_entries {
                receiver.insert_owned(&function, &mut states, State([3; 50]), &mut budget);
                assert!(!receiver.failed);
                assert_eq!(states.len(), 1);
                assert_eq!(current_metrics().allocated, allocated);
            }
            drop(states);
            drop(receiver);
            drop(budget);
        })
    });
    assert!(!refused && first.is_none());
    assert!(!allocation.invalid && allocation.live == 0);
    assert_eq!(allocation.allocated, allocation.freed);
    assert!(allocation.allocated <= paid);
    let cold = 2 * size_of::<ByteFailure>() + 2 * size_of::<Diagnostic>();
    let transport_delta =
        size_of::<Result<(), Diagnostic>>() - size_of::<Result<(), ByteFailure>>();
    assert!(transport_delta > 0);
    assert_eq!(
        paid,
        bootstrap(&function)
            + Insertions::<State>::controls()
            + owned()
            + (duplicate_entries + 1) * Insertions::<State>::entry_controls()
    );
    // Owner fields and every old fee except this Result are unchanged. The
    // complete original source is retained by the source-history owner.
    // This is target-layout arithmetic, not an old executable-cost claim.
    let original_transport_charge = paid - cold + (duplicate_entries + 2) * transport_delta;
    assert!(paid < original_transport_charge);
}

#[test]
fn compact_positive_refusal_keeps_original_fields_receipt_work_and_sticky_owner() {
    let function = function();
    let observe = |compact: bool| {
        let ((diagnostic, work), refused, paid, first) = with_limit_usage_refusal(1 << 20, || {
            let mut budget = ReplayBudget::new();
            let mut receiver = Insertions::<State>::acquire(&function, &mut budget).unwrap();
            let mut states = BTreeSet::new();
            receiver.insert_owned(&function, &mut states, State([5; 50]), &mut budget);
            assert!(!receiver.failed);
            assert!(set_active_floor(active_remaining().unwrap()));
            let diagnostic = if compact {
                receiver.insert_owned(&function, &mut states, State([5; 50]), &mut budget);
                assert!(receiver.failed);
                receiver.insert_owned(&function, &mut states, State([7; 50]), &mut budget);
                assert!(receiver.failed);
                receiver.failure()
            } else {
                budget
                    .reserve_bytes(
                        &function,
                        Insertions::<State>::entry_controls(),
                        "ownership duplicate receiving renewal",
                    )
                    .unwrap_err()
            };
            assert_eq!(states.len(), 1);
            let work = (budget.remaining, budget.skeleton_remaining);
            drop(states);
            drop(receiver);
            drop(budget);
            (diagnostic, work)
        });
        (diagnostic.json(), work, refused, paid, first)
    };
    assert_eq!(observe(true), observe(false));
}
