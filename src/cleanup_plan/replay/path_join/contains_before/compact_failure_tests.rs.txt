//! Compact held refusal against the complete immediate-a6 receiver and gateway.
use super::*;
use crate::bounded_output::{
    active_remaining, clear_active_floor, reserve_active_required, set_active_floor,
    with_limit_usage_refusal,
};
use crate::cleanup_plan::replay::physical_budget::ByteFailure;
use crate::test_allocation_observer::{current_metrics, measure, watch_free_order};
use sha2::{Digest, Sha256};
use std::mem::{align_of, size_of};
fn function() -> ResolvedFunction {
    crate::hir::resolve(
        &crate::check(
            "module compact; @id(\"main\") fn main()->i64 {0}",
            "compact.spx",
        )
        .unwrap(),
    )
    .unwrap()
    .functions
    .remove(0)
}
// Keep the Result wrapper's actual layout: Diagnostic can dominate either arm.
// The NEW foreign from_budget call is independently prepaid before acquisition.
pub(super) fn setup_delta<T: Ord>() -> usize {
    type Error = Diagnostic;
    let gross = 3
        * (size_of::<compact_failure_before::Insertions<T>>()
            - size_of::<frontier::Insertions<T>>())
        + (size_of::<Result<compact_failure_before::Insertions<T>, Error>>()
            - size_of::<Result<frontier::Insertions<T>, Error>>())
        + 3 * (size_of::<Option<Diagnostic>>() - size_of::<Option<ByteFailure>>());
    let new_conversion = 2 * size_of::<Diagnostic>() + 2 * size_of::<ByteFailure>();
    let net = gross
        .checked_sub(new_conversion)
        .expect("new conversion fully funded");
    assert!(net > 0);
    assert_eq!(
        compact_failure_before::Insertions::<T>::controls(),
        frontier::Insertions::<T>::controls() + net
    );
    assert_eq!(
        compact_failure_before::Insertions::<T>::entry_controls(),
        frontier::Insertions::<T>::entry_controls()
    );
    assert_eq!(
        compact_failure_before::Insertions::<T>::GROUP_ENTRY_CONTROLS,
        frontier::Insertions::<T>::GROUP_ENTRY_CONTROLS
    );
    assert_eq!(
        compact_failure_before::Insertions::<T>::observed_prices(),
        frontier::Insertions::<T>::observed_prices()
    );
    net
}
#[test]
fn complete_owner_result_option_layouts_remove_only_net_new_conversion_funded_bytes() {
    assert_eq!(
        align_of::<compact_failure_before::Insertions<PathState>>(),
        align_of::<frontier::Insertions<PathState>>()
    );
    let net = setup_delta::<PathState>();
    let f = function();
    let invoice = |old| {
        let (((), allocation), refused, paid, first) = with_limit_usage_refusal(usize::MAX, || {
            measure(|| {
                let mut b = ReplayBudget::new();
                if old {
                    drop(
                        compact_failure_before::Insertions::<PathState>::acquire(&f, &mut b)
                            .unwrap(),
                    );
                } else {
                    drop(frontier::Insertions::<PathState>::acquire(&f, &mut b).unwrap());
                }
                drop(b);
            })
        });
        assert!(!refused && first.is_none() && !allocation.invalid && allocation.live == 0);
        assert_eq!(allocation.allocated, allocation.freed);
        assert!(allocation.allocated <= paid);
        (paid, allocation)
    };
    let old = invoice(true);
    let new = invoice(false);
    assert_eq!(new.0 + net, old.0);
    assert_eq!(new.1, old.1);
    for prior in [false, true] {
        let exact = if prior { old.0 } else { new.0 };
        for limit in [0, exact - 1, exact] {
            let ((success, allocation), refused, paid, first) =
                with_limit_usage_refusal(limit, || {
                    measure(|| {
                        let mut b = ReplayBudget::new();
                        let success = if prior {
                            match compact_failure_before::Insertions::<PathState>::acquire(
                                &f, &mut b,
                            ) {
                                Ok(receiver) => {
                                    drop(receiver);
                                    true
                                }
                                Err(error) => {
                                    assert_eq!(error.code, "SPX-H006");
                                    drop(error);
                                    false
                                }
                            }
                        } else {
                            match frontier::Insertions::<PathState>::acquire(&f, &mut b) {
                                Ok(receiver) => {
                                    drop(receiver);
                                    true
                                }
                                Err(error) => {
                                    assert_eq!(error.code, "SPX-H006");
                                    drop(error);
                                    false
                                }
                            }
                        };
                        drop(b);
                        success
                    })
                });
            assert_eq!(success, limit == exact);
            assert_eq!(refused, !success);
            assert_eq!(first.is_some(), !success);
            assert!(!allocation.invalid && allocation.live == 0);
            assert_eq!(allocation.allocated, allocation.freed);
            assert!(allocation.allocated <= paid && paid <= limit);
        }
    }
}
#[test]
#[allow(
    clippy::collapsible_match,
    reason = "The complete saved control retains its original expressions for independent parity checks."
)]
fn compact_zero_all_payers_keep_complete_diagnostic_first_receipt_floor_and_drop() {
    let function = function();
    for payer in 0..3 {
        for frontier in 0..6 {
            let observe = |unit: bool| {
                with_limit_usage_refusal(1 << 20, || {
                    {
                        let mut budget = ReplayBudget::new();
                        if payer == 0 {
                            budget.payer = AllocationPayer::LegacyUnobserved;
                        } else if payer == 2 {
                            budget = ReplayBudget::with_physical(&function, 1 << 18).unwrap();
                        }
                        // Positive setup remains original and prepaid. The measured
                        // zero call must never bootstrap another refusal String.
                        if payer != 0 {
                            budget.reserve_bytes(&function, 1, "setup").unwrap();
                        }
                        let remaining = active_remaining().unwrap();
                        match frontier {
                            1 => {
                                assert!(reserve_active_required(remaining));
                            }
                            2 => {
                                assert!(set_active_floor(remaining));
                            }
                            3 => {
                                assert!(set_active_floor(remaining));
                                let (_, _, used, _) = with_limit_usage_refusal(1, || {
                                    assert!(reserve_active_required(1))
                                });
                                assert_eq!(used, 1);
                            }
                            4 => {
                                assert!(!reserve_active_required(remaining + 1));
                            }
                            5 => {
                                if payer != 0 {
                                    drop(budget.budget_refusal(
                                        &function,
                                        "selected earlier refusal",
                                        "",
                                    ));
                                    assert!(budget.refusal_spent && budget.refusal_text.is_none());
                                }
                            }
                            _ => {}
                        }
                        let at = current_metrics().allocated;
                        let mut held = None;
                        if unit {
                            let mut compact = None;
                            budget.authenticate_zero_compact_into(
                                &function,
                                "parity phase",
                                &mut compact,
                            );
                            held = compact.map(ByteFailure::into_diagnostic);
                        } else {
                            budget.authenticate_zero_into(&function, "parity phase", &mut held);
                        }
                        assert_eq!(current_metrics().allocated, at);
                        let result = (
                            held,
                            budget.remaining,
                            budget.skeleton_remaining,
                            budget.refusal_spent,
                            budget.refusal_text.is_none(),
                            budget.physical.as_ref().map(|physical| physical.charged()),
                            active_remaining(),
                        );
                        clear_active_floor();
                        drop(budget);
                        result
                    }
                })
            };
            let ((), allocation) = measure(|| {
                let old = observe(false);
                let new = observe(true);
                assert_eq!(old.0, new.0, "payer={payer} frontier={frontier}");
                assert_eq!((old.1, old.2, old.3), (new.1, new.2, new.3));
                drop(old);
                drop(new);
            });
            assert!(
                !allocation.invalid
                    && allocation.live == 0
                    && allocation.allocated == allocation.freed
            );
        }
    }
}

#[test]
fn compact_zero_oracle_attempts_sticky_expired_and_zero_owner_parity() {
    let function = function();
    for scenario in 0..4 {
        let observe = |unit| {
            let scope = allocation::OracleScope::open_with_work(4096, 64).unwrap();
            let lease = scope.lease();
            let mut budget = ReplayBudget::with_skeleton_limit_oracle(0, lease);
            budget.reserve_bytes(&function, 1, "oracle setup").unwrap();
            match scenario {
                1 => {
                    assert!(lease.charge(4096).is_err());
                }
                2 => {
                    drop(budget.budget_refusal(&function, "selected earlier oracle refusal", ""));
                    assert!(budget.refusal_spent && budget.refusal_text.is_none());
                }
                3 => {
                    lease.fail_physical("original first physical refusal");
                }
                _ => {}
            }
            let mut held = None;
            if unit {
                let mut compact = None;
                budget.authenticate_zero_compact_into(&function, "oracle parity", &mut compact);
                held = compact.map(ByteFailure::into_diagnostic);
            } else {
                budget.authenticate_zero_into(&function, "oracle parity", &mut held);
            }
            let receipt = scope.receipt();
            drop(budget);
            drop(scope);
            (held, receipt)
        };
        assert_eq!(observe(false), observe(true));
    }
    // Expired tokens must not charge the replacement live account.
    let expired = {
        let scope = allocation::OracleScope::open(0).unwrap();
        scope.lease()
    };
    let observe = |unit| {
        let scope = allocation::OracleScope::open(0).unwrap();
        let mut budget = ReplayBudget::with_skeleton_limit_oracle(0, expired);
        let mut held = None;
        if unit {
            let mut compact = None;
            budget.authenticate_zero_compact_into(&function, "expired", &mut compact);
            held = compact.map(ByteFailure::into_diagnostic);
        } else {
            budget.authenticate_zero_into(&function, "expired", &mut held);
        }
        let receipt = scope.receipt();
        drop(budget);
        drop(scope);
        (held, receipt)
    };
    assert_eq!(observe(false), observe(true));
}

#[test]
fn compact_zero_fresh_zero_oracle_never_bootstraps_text_or_active_payer() {
    let function = function();
    let scope = allocation::OracleScope::open(0).unwrap();
    let lease = scope.lease();
    let mut budget = ReplayBudget::with_skeleton_limit_oracle(0, lease);
    let mut held = None;
    let (((), allocation), refused, paid, first) = with_limit_usage_refusal(0, || {
        measure(|| {
            budget.authenticate_zero_compact_into(&function, "fresh zero oracle", &mut held);
            assert!(held.is_none() && budget.refusal_text.is_none() && !budget.refusal_spent);
        })
    });
    assert!(!refused && paid == 0 && first.is_none() && allocation.allocated == 0);
    assert_eq!(scope.receipt().calls, 1);
    assert_eq!(scope.receipt().charged, 0);
}

#[test]
fn compact_existing_owned_or_borrowed_report_never_queries_or_overwrites() {
    let f = function();
    for owned in [false, true] {
        let mut b = ReplayBudget::new();
        b.payer = AllocationPayer::ActiveBuilder;
        let mut error = Diagnostic::io("SPX-H006", String::new());
        error.message = if owned {
            std::borrow::Cow::Owned("held report".to_owned())
        } else {
            std::borrow::Cow::Borrowed("held report")
        };
        let expected = error.clone();
        let mut held = Some(ByteFailure::from_budget(error));
        let (((), a), refused, paid, first) = with_limit_usage_refusal(0, || {
            measure(|| {
                b.authenticate_zero_compact_into(&f, "must not run", &mut held);
            })
        });
        let actual = held.unwrap().into_diagnostic();
        assert_eq!(actual, expected);
        assert_eq!(matches!(actual.message, std::borrow::Cow::Owned(_)), owned);
        assert!(!refused && paid == 0 && first.is_none() && a.allocated == 0);
    }
}
#[derive(Eq, PartialEq, Ord, PartialOrd)]
struct State([u64; 50]);
thread_local! { static DROPS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) }; }
impl Drop for State {
    fn drop(&mut self) {
        DROPS.with(|n| n.set(n.get() + 1));
    }
}
#[test]
fn actual_ordinary_and_group_entries_keep_authority_positive_zero_sticky_and_drop_order() {
    let f = function();
    let other = function();
    for grouped in [false, true] {
        for scenario in 0..6 {
            let observe = |old| {
                DROPS.with(|n| n.set(0));
                let (answer, refused, paid, first) = with_limit_usage_refusal(1 << 20, || {
                    let mut b = ReplayBudget::new();
                    let mut foreign = ReplayBudget::new();
                    macro_rules! run {
                        ($module:ident) => {{
                            let mut receiver = $module::Insertions::acquire(&f, &mut b).unwrap();
                            let mut incoming = $module::Incoming::default();
                            let mut group = BTreeSet::new();
                            let mut held = if grouped {
                                receiver.insert_owned(&f, &mut incoming, State([7; 50]), &mut b);
                                assert!(!receiver.failed);
                                Some(incoming.into_one().next().unwrap())
                            } else {
                                None
                            };
                            let mut incoming = $module::Incoming::default();
                            match scenario {
                                1 => {
                                    assert!(set_active_floor(active_remaining().unwrap()));
                                }
                                2 => {
                                    let remaining = active_remaining().unwrap();
                                    assert!(set_active_floor(remaining - 1));
                                }
                                5 => {
                                    let ((), child_refused, child_paid, child_first) =
                                        with_limit_usage_refusal(1, || {
                                            if grouped {
                                                receiver.insert_group_held(
                                                    &f,
                                                    &mut group,
                                                    held.take().unwrap(),
                                                    &mut b,
                                                );
                                            } else {
                                                receiver.insert_owned(
                                                    &f,
                                                    &mut incoming,
                                                    State([1; 50]),
                                                    &mut b,
                                                );
                                            }
                                        });
                                    assert!(
                                        !child_refused && child_paid == 0 && child_first.is_none()
                                    );
                                }
                                _ => {}
                            }
                            if scenario != 5 {
                                let function = if scenario == 3 { &other } else { &f };
                                let budget = if scenario == 4 { &mut foreign } else { &mut b };
                                if grouped {
                                    receiver.insert_group_held(
                                        function,
                                        &mut group,
                                        held.take().unwrap(),
                                        budget,
                                    );
                                } else {
                                    receiver.insert_owned(
                                        function,
                                        &mut incoming,
                                        State([1; 50]),
                                        budget,
                                    );
                                }
                            }
                            let error = if receiver.failed {
                                Some(receiver.failure())
                            } else {
                                None
                            };
                            if scenario == 0 {
                                assert!(error.is_none());
                                if grouped {
                                    receiver.insert_owned(
                                        &f,
                                        &mut incoming,
                                        State([7; 50]),
                                        &mut b,
                                    );
                                    receiver.insert_group_held(
                                        &f,
                                        &mut group,
                                        incoming.into_one().next().unwrap(),
                                        &mut b,
                                    );
                                    assert_eq!(group.len(), 1);
                                } else {
                                    for v in [2, 2, 3] {
                                        receiver.insert_owned(
                                            &f,
                                            &mut incoming,
                                            State([v; 50]),
                                            &mut b,
                                        );
                                    }
                                    assert_eq!(incoming.len(), 3);
                                    assert_eq!(incoming.iter().map(|v| v.0[0]).sum::<u64>(), 6);
                                    drop(incoming);
                                }
                                assert!(!receiver.failed);
                            } else {
                                assert!(error.is_some());
                                let at = active_remaining();
                                // An already-failed owner must neither query nor publish on retry.
                                let mut retry = $module::Incoming::default();
                                receiver.insert_owned(
                                    &other,
                                    &mut retry,
                                    State([99; 50]),
                                    &mut foreign,
                                );
                                assert!(retry.is_empty() && receiver.failed);
                                assert_eq!(active_remaining(), at);
                                drop(incoming);
                            }
                            assert_eq!(foreign.remaining, MAX_REPLAY_WORK_UNITS);
                            clear_active_floor();
                            drop(group);
                            drop(receiver);
                            (
                                error,
                                b.remaining,
                                b.skeleton_remaining,
                                DROPS.with(std::cell::Cell::get),
                            )
                        }};
                    }
                    if old {
                        run!(compact_failure_before)
                    } else {
                        run!(frontier)
                    }
                });
                assert!(paid <= 1 << 20);
                (answer, refused, first)
            };
            // The physical first receipt is intentionally compared only where
            // requests are identical; owner-acquisition byte prices differ.
            let old = observe(true);
            let new = observe(false);
            assert_eq!(old.0, new.0, "group={grouped} scenario={scenario}");
            assert_eq!(old.1, new.1);
            assert_eq!(old.2.is_some(), new.2.is_some());
        }
    }
}

#[test]
fn pending_or_taken_refusal_frees_identical_backing_in_the_same_box_and_cow_order() {
    let f = function();
    for take in [false, true] {
        let observe = |old| {
            let ((order, a), refused, paid, first) = with_limit_usage_refusal(1 << 20, || {
                measure(|| {
                    let mut b = ReplayBudget::new();
                    macro_rules! run {
                        ($module:ident) => {{
                            let mut r = $module::Insertions::acquire(&f, &mut b).unwrap();
                            let mut target = $module::Incoming::default();
                            r.insert_owned(&f, &mut target, State([1; 50]), &mut b);
                            assert!(!r.failed);
                            let text = b.refusal_text.as_ref().unwrap().as_ptr() as usize;
                            let value = std::ptr::from_ref(target.one().as_ref()) as usize;
                            assert!(set_active_floor(active_remaining().unwrap()));
                            r.insert_owned(&f, &mut target, State([2; 50]), &mut b);
                            assert!(r.failed && target.len() == 1 && b.refusal_text.is_none());
                            let (_, order) = watch_free_order(&[text, value], || {
                                if take {
                                    let report = r.failure();
                                    assert!(matches!(report.message, std::borrow::Cow::Owned(_)));
                                    assert_eq!(report.code, "SPX-H006");
                                    assert_eq!(report.severity, crate::diagnostic::Severity::Error);
                                    assert!(
                                        report.path.is_none()
                                            && report.span.is_none()
                                            && report.help.is_none()
                                    );
                                    drop(target);
                                    drop(report);
                                    drop(r);
                                } else {
                                    drop(r);
                                    drop(target);
                                }
                                drop(b);
                            });
                            clear_active_floor();
                            order
                        }};
                    }
                    if old {
                        run!(compact_failure_before)
                    } else {
                        run!(frontier)
                    }
                })
            });
            assert!(refused && first.is_some() && paid <= 1 << 20);
            assert!(!a.invalid && a.live == 0 && a.allocated == a.freed && a.allocated <= paid);
            assert!(!order.invalid && order.count == 2);
            assert_eq!(&order.order[..2], if take { &[1, 0] } else { &[0, 1] });
            ((order.order, order.count, order.invalid), a)
        };
        assert_eq!(observe(true), observe(false));
    }
}

fn digest(raw: &[u8]) -> String {
    format!("{:x}", crate::digest_hex::LowerHex(Sha256::digest(raw)))
}
pub(super) fn before_source(path: &str, mut actual: String) -> String {
    let inverse = include_bytes!("compact_failure_before/inverse.json");
    assert_eq!(inverse.len(), INVERSE_LENGTH);
    assert_eq!(digest(inverse), INVERSE_SHA);
    let rows: serde_json::Value = serde_json::from_slice(inverse).unwrap();
    let mut matched = false;
    for row in rows.as_array().unwrap() {
        if row["current"].as_str().unwrap() != path {
            continue;
        }
        assert!(!matched);
        matched = true;
        assert_eq!(
            actual.len(),
            row["current_length"].as_u64().unwrap() as usize
        );
        assert_eq!(
            digest(actual.as_bytes()),
            row["current_sha256"].as_str().unwrap()
        );
        for edit in row["edits"].as_array().unwrap() {
            let from = edit["from"].as_str().unwrap();
            assert_eq!(actual.matches(from).count(), 1);
            actual = actual.replacen(from, edit["to"].as_str().unwrap(), 1);
        }
        let before = std::fs::read(
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(row["before"].as_str().unwrap()),
        )
        .unwrap();
        assert_eq!(digest(&before), row["before_sha256"].as_str().unwrap());
        assert_eq!(actual.as_bytes(), before);
    }
    actual
}
#[test]
fn complete_immediate_frontier_gateway_and_original_controls_restore_exact_bytes() {
    let manifest = include_bytes!("compact_failure_before/source_manifest.json");
    assert_eq!(manifest.len(), MANIFEST_LENGTH);
    assert_eq!(digest(manifest), MANIFEST_SHA);
    let rows: serde_json::Value = serde_json::from_slice(manifest).unwrap();
    assert_eq!(rows.as_array().unwrap().len(), 4);
    for row in rows.as_array().unwrap() {
        let path = row["path"].as_str().unwrap();
        let read = |path: &str| {
            std::fs::read(std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(path)).unwrap()
        };
        let raw = read(path);
        let restored = before_source(path, String::from_utf8(raw).unwrap());
        let before = read(row["before"].as_str().unwrap());
        assert_eq!(before.len(), row["length"].as_u64().unwrap() as usize);
        assert_eq!(digest(&before), row["sha256"].as_str().unwrap());
        assert_eq!(restored.as_bytes(), before);
    }
    assert_eq!(
        include_bytes!("compact_failure_before.rs"),
        include_bytes!("compact_failure_before/frontier.rs.txt")
    );
    let old = include_str!("compact_failure_before/authentication.rs.txt");
    let now = include_str!("../physical_budget/authentication.rs");
    assert!(now.starts_with(old));
    let direct_old = old.split("#[cfg(test)]\nmod tests;").next().unwrap();
    let direct_compact = direct_old
        .replace("authenticate_zero_into", "authenticate_zero_compact_into")
        .replace("Option<Diagnostic>", "Option<ByteFailure>")
        .replace("self.budget_refusal(", "self.compact_refusal(");
    // Both independently compiled bodies bind the actual same payer producers;
    // the old gateway is neither deleted nor called through a compact wrapper.
    assert!(now.ends_with(direct_compact.trim_start_matches("//! Warm authentication publishes only into the caller's already paid slot.\nuse super::*;\n")));
    let physical_budget = include_bytes!("../physical_budget.rs");
    assert_eq!(physical_budget.len(), 43414);
    assert_eq!(
        digest(physical_budget),
        "dca3a126f4c0dfde7e45db233d011100c5f9411cd77766260a9b1a4669c05cbd"
    );
    let classifier_string = include_bytes!("../../../bounded_output/classifier_string.rs");
    assert_eq!(classifier_string.len(), 23258);
    assert_eq!(
        digest(classifier_string),
        "0fd752c50adec4ffe2269581cc7c7d2cc1e6d873d43a6bb47d35e6be9d826550"
    );
}
const INVERSE_LENGTH: usize = 29150;
const INVERSE_SHA: &str = "19270e2127097657de141dba75c560634497871a1159f6cac38fc06bd39b6667";
const MANIFEST_LENGTH: usize = 1076;
const MANIFEST_SHA: &str = "02b92751ee38d02f3fd1ac400ba2e669056136ccdd72cbe3470ed91b12904495";
