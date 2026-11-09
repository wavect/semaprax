//! Whole old/current owners. Test scaffolding is outside production authority.
mod current {
    pub(super) use super::super::super::super::*;
}
use super::*;
use crate::bounded_output::with_limit_usage_refusal;
use crate::test_allocation_observer::measure;

fn fixture() -> (ResolvedProgram, ResolvedFunction) {
    current::lexical_plan_tests::fixture(current::lexical_plan_tests::WHILE)
}

#[test]
fn compact_layout_and_original_label_order_are_actual_and_source_independent() {
    assert_eq!(
        std::mem::size_of::<Node>(),
        2 * std::mem::size_of::<Vec<usize>>()
    );
    if std::mem::size_of::<usize>() == 8 {
        // Derive the occupied inline payload independently from its fields.
        // The pinned compiler uses Label's spare discriminants for Node tags.
        let inline_payload = std::alloc::Layout::new::<current::Label>()
            .extend(std::alloc::Layout::new::<usize>())
            .unwrap()
            .0
            .pad_to_align()
            .size();
        assert_eq!(inline_payload, 24);
        assert_eq!(std::mem::size_of::<current::Node>(), inline_payload);
        assert_eq!(std::mem::size_of::<Node>(), 48);
        assert!(std::mem::size_of::<current::Node>() < std::mem::size_of::<Node>());
    }
    assert_eq!(
        std::mem::size_of::<current::Label>(),
        std::mem::size_of::<Label>()
    );
    // Identical terminal values preserve their original relative label order.
    for a in [
        SkeletonTerminal::Success,
        SkeletonTerminal::Failure,
        SkeletonTerminal::LoopLatch,
    ] {
        for b in [
            SkeletonTerminal::Success,
            SkeletonTerminal::Failure,
            SkeletonTerminal::LoopLatch,
        ] {
            assert_eq!(
                Label::End(a).cmp(&Label::End(b)),
                current::Label::End(a).cmp(&current::Label::End(b))
            );
        }
    }
}

#[test]
#[allow(
    clippy::manual_range_patterns,
    reason = "The complete saved control retains its original expressions for independent parity checks."
)]
fn exhaustive_small_order_parallel_epsilon_and_weighted_languages_match_complete_old() {
    let (_, function) = fixture();
    // Three forward pairs, six cases each: absent, one move, one epsilon,
    // mixed, duplicate epsilon, duplicate movement. All 216 complete graphs.
    let mut overflow_seen = false;
    for mut word in 0..216 {
        let mut a = Automaton::default();
        let mut b = current::Automaton::default();
        let mut ba = ReplayBudget::unobserved();
        let mut bb = ReplayBudget::unobserved();
        for _ in 0..3 {
            a.node(&function, &mut ba).unwrap();
            b.node(&function, &mut bb).unwrap();
        }
        for (from, to) in [(0, 1), (0, 2), (1, 2)] {
            let case = word % 6;
            word /= 6;
            for _ in 0..usize::from(matches!(case, 1 | 3 | 5)) + usize::from(case == 5) {
                a.movement(
                    &function,
                    &mut ba,
                    from,
                    Label::End(SkeletonTerminal::Success),
                    to,
                )
                .unwrap();
                b.movement(
                    &function,
                    &mut bb,
                    from,
                    current::Label::End(SkeletonTerminal::Success),
                    to,
                )
                .unwrap();
            }
            for _ in 0..usize::from(matches!(case, 2 | 3 | 4)) + usize::from(case == 4) {
                a.epsilon(&function, &mut ba, from, to).unwrap();
                b.epsilon(&function, &mut bb, from, to).unwrap();
            }
        }
        for (a, b) in a.nodes.iter().zip(&b.nodes) {
            assert_eq!(a.epsilon.as_slice(), b.epsilon());
            assert_eq!(a.moves.len(), b.moves().len());
            for ((la, ta), (lb, tb)) in a.moves.iter().zip(b.moves()) {
                assert_eq!(format!("{la:?}"), format!("{lb:?}"));
                assert_eq!(ta, tb);
            }
        }
        assert_eq!(ba.remaining, bb.remaining);
        let mut old = canonical_shared::Canonicalizer::new(&a, 2);
        let mut new = current::canonical_shared::Canonicalizer::new(&b, 2);
        assert_eq!(
            old.id(&function, vec![(0, 1)], &mut ba),
            new.id(&function, vec![(0, 1)], &mut bb)
        );
        assert_eq!(ba.remaining, bb.remaining);
        assert_eq!(ba.skeleton_spent, bb.skeleton_spent);
        let overflow = old.id(&function, vec![(0, u64::MAX)], &mut ba);
        assert_eq!(overflow, new.id(&function, vec![(0, u64::MAX)], &mut bb));
        overflow_seen |= matches!(overflow, Ok(None));
        assert_eq!(ba.remaining, bb.remaining);
    }
    assert!(
        overflow_seen,
        "duplicate routes exercise checked weighted overflow"
    );
}

#[test]
fn complete_builder_and_comparison_receivers_keep_work_and_later_semantic_diagnostics() {
    for source in [
        current::lexical_plan_tests::WHILE,
        current::lexical_plan_tests::GENERATED,
    ] {
        let (program, function) = current::lexical_plan_tests::fixture(source);
        // Direct historical control is outside physical owner windows. Keep
        // the full original transition producer on the old 232-byte enum and
        // compare every field of every original corpus transition losslessly.
        for block in &function.cleanup_plan.blocks {
            for transition in &block.transitions {
                let mut historical = ReplayBudget::unobserved();
                let mut common = ReplayBudget::unobserved();
                let expected =
                    transition_observation_historical(&function, &mut historical, transition);
                let actual = transition_observation(&function, &mut common, transition);
                match (&actual, &expected) {
                    (Ok(None), Ok(None)) => {}
                    (Ok(Some(actual)), Ok(Some(expected))) => assert!(
                        crate::cleanup_plan::replay::compact_stage::observations_match(
                            actual, expected,
                        )
                    ),
                    (Err(actual), Err(expected)) => assert_eq!(actual, expected),
                    _ => panic!("complete historical transition mismatch: {actual:?} {expected:?}"),
                }
                assert_eq!(common.remaining, historical.remaining);
                assert_eq!(common.skeleton_spent, historical.skeleton_spent);
                drop((actual, expected, common, historical));
            }
        }
        let mut a = ReplayBudget::unobserved();
        let mut b = ReplayBudget::unobserved();
        let old = build_comparison(&program, &function, &mut a, None, None).unwrap();
        let new = current::build_comparison(&program, &function, &mut b, None, None).unwrap();
        assert_eq!(
            (old.sink, old.hir_root, old.plan_root),
            (new.sink, new.hir_root, new.plan_root)
        );
        assert_eq!(old.automaton.nodes.len(), new.automaton.nodes.len());
        for (a, b) in old.automaton.nodes.iter().zip(&new.automaton.nodes) {
            assert_eq!(a.epsilon.as_slice(), b.epsilon());
            assert_eq!(format!("{:?}", a.moves), format!("{:?}", b.moves()));
        }
        assert_eq!(a.remaining, b.remaining);
        assert_eq!(a.skeleton_spent, b.skeleton_spent);
        let mut malformed = function.clone();
        let edge = malformed
            .cleanup_plan
            .edges
            .iter_mut()
            .find(|edge| matches!(edge.condition, EdgeCondition::BooleanResult(_, _)))
            .expect("Boolean fixture edge");
        edge.condition = EdgeCondition::Always;
        let mut a = ReplayBudget::unobserved();
        let mut b = ReplayBudget::unobserved();
        let old_error = compare(&program, &malformed, &mut a);
        assert_eq!(old_error, current::compare(&program, &malformed, &mut b));
        assert!(old_error.is_err());
        assert_eq!(a.remaining, b.remaining);
        let mut full = ReplayBudget::unobserved();
        let start = full.remaining;
        let expected = compare(&program, &function, &mut full);
        assert_eq!(expected, Ok(Some(true)));
        let spent = start - full.remaining;
        for units in 0..=spent {
            let mut a = ReplayBudget::unobserved();
            a.remaining = units;
            let mut b = ReplayBudget::unobserved();
            b.remaining = units;
            // Result equality includes every Diagnostic field at every work frontier.
            assert_eq!(
                compare(&program, &function, &mut a),
                current::compare(&program, &function, &mut b)
            );
            assert_eq!(a.remaining, b.remaining);
            assert_eq!(a.skeleton_spent, b.skeleton_spent);
        }
    }
}

fn observe(
    function: &ResolvedFunction,
    old: bool,
    nodes: usize,
    fanout: usize,
    limit: usize,
) -> (
    (bool, crate::test_allocation_observer::Metrics),
    bool,
    usize,
    Option<crate::bounded_output::ReservationRefusal>,
) {
    with_limit_usage_refusal(limit, || {
        measure(|| {
            let mut budget = ReplayBudget::new();
            let result = (|| -> Result<(), Diagnostic> {
                if old {
                    let mut automaton = Automaton::default();
                    let mut previous = automaton.node(function, &mut budget)?;
                    for _ in 0..nodes {
                        let next = automaton.node(function, &mut budget)?;
                        for _ in 0..fanout {
                            automaton.movement(
                                function,
                                &mut budget,
                                previous,
                                Label::End(SkeletonTerminal::Success),
                                next,
                            )?;
                        }
                        previous = next;
                    }
                    drop(automaton);
                } else {
                    let mut automaton = current::Automaton::default();
                    let mut previous = automaton.node(function, &mut budget)?;
                    for _ in 0..nodes {
                        let next = automaton.node(function, &mut budget)?;
                        for _ in 0..fanout {
                            automaton.movement(
                                function,
                                &mut budget,
                                previous,
                                current::Label::End(SkeletonTerminal::Success),
                                next,
                            )?;
                        }
                        previous = next;
                    }
                    drop(automaton);
                }
                Ok(())
            })();
            let success = result.is_ok();
            drop(result);
            drop(budget);
            success
        })
    })
}

#[test]
fn whole_constructor_to_drop_reports_empty_singleton_and_unfavorable_many_costs() {
    let (_, function) = fixture();
    for (nodes, fanout) in [(0, 0), (1, 1), (127, 1), (16, 8)] {
        let ((ok_a, a), refused_a, charged_a, _) =
            observe(&function, true, nodes, fanout, usize::MAX);
        let ((ok_b, b), refused_b, charged_b, _) =
            observe(&function, false, nodes, fanout, usize::MAX);
        assert!(ok_a && ok_b && !refused_a && !refused_b);
        assert!(!a.invalid && !b.invalid);
        assert_eq!((a.live, b.live), (0, 0));
        assert_eq!(a.allocated, a.freed);
        assert_eq!(b.allocated, b.freed);
        eprintln!("844 nodes={nodes} fanout={fanout} old_reserved={charged_a} new_reserved={charged_b} old={a:?} new={b:?}");
        if nodes == 127 && fanout == 1 {
            assert!(b.allocated < a.allocated);
            assert!(b.allocations < a.allocations);
            assert!(
                charged_b < charged_a,
                "real inline gain includes every new initializer control"
            );
        }
        let ((exact, m), refused, charged, _) = observe(&function, false, nodes, fanout, charged_b);
        assert!(exact && !refused);
        assert_eq!(charged, charged_b);
        assert!(!m.invalid && m.live == 0);
        let ((short, m), refused, _, receipt) =
            observe(&function, false, nodes, fanout, charged_b - 1);
        assert!(!short && refused && receipt.is_some());
        assert!(!m.invalid && m.live == 0);
        assert_eq!(m.allocated, m.freed);
        let ((zero, m), refused, _, _) = observe(&function, false, nodes, fanout, 0);
        assert!(!zero && refused && !m.invalid && m.live == 0);
    }
}

#[test]
fn promotion_refusal_preserves_old_inline_value_and_all_drop_ownership() {
    let (_, function) = fixture();
    for epsilon_first in [false, true] {
        for cut in 0..3 {
            if !epsilon_first && cut == 2 {
                continue;
            }
            let mut budget = ReplayBudget::with_physical(&function, 1 << 20).unwrap();
            let mut automaton = current::Automaton::default();
            let from = automaton.node(&function, &mut budget).unwrap();
            let to = automaton.node(&function, &mut budget).unwrap();
            if epsilon_first {
                automaton
                    .epsilon(&function, &mut budget, from, usize::MAX)
                    .unwrap();
            } else {
                automaton
                    .movement(
                        &function,
                        &mut budget,
                        from,
                        current::Label::End(SkeletonTerminal::Failure),
                        to,
                    )
                    .unwrap();
            }
            let before = (
                format!("{:?}", automaton.nodes[from].moves()),
                automaton.nodes[from].epsilon().to_vec(),
            );
            let allowance = match cut {
                0 => 0,
                1 => {
                    current::Node::PROMOTION_CONTROLS
                        + std::mem::size_of::<current::compact_nodes::Many>()
                        + (if epsilon_first { 1 } else { 2 })
                            * std::mem::size_of::<(current::Label, usize)>()
                        - 1
                }
                2 => {
                    current::Node::PROMOTION_CONTROLS
                        + std::mem::size_of::<current::compact_nodes::Many>()
                        + std::mem::size_of::<(current::Label, usize)>()
                        + std::mem::size_of::<usize>()
                        - 1
                }
                _ => unreachable!(),
            };
            let remaining = (1 << 20) - budget.physical.as_ref().unwrap().charged();
            budget
                .reserve_bytes(&function, remaining - allowance, "844 exact frontier")
                .unwrap();
            let (failed, allocation) = measure(|| {
                automaton.movement(
                    &function,
                    &mut budget,
                    from,
                    current::Label::End(SkeletonTerminal::Success),
                    to,
                )
            });
            let phase = match cut {
                0 => "factored compact node promotion controls",
                1 => "factored automaton moves",
                2 => "factored automaton epsilon moves",
                _ => unreachable!(),
            };
            assert_eq!(failed, Err(Diagnostic::io("SPX-H006", format!("cleanup plan for function `{}` failed independent replay: cleanup replay physical reservation exhausted during {phase}", function.id))));
            assert_eq!(
                before,
                (
                    format!("{:?}", automaton.nodes[from].moves()),
                    automaton.nodes[from].epsilon().to_vec()
                )
            );
            assert!(!allocation.invalid && allocation.live == 0);
            assert_eq!(allocation.allocated, allocation.freed);
            if cut == 0 {
                assert_eq!(allocation.allocated, 0);
            } else {
                assert!(
                    allocation.allocated >= std::mem::size_of::<current::compact_nodes::Many>()
                );
            }
            assert!(budget.refusal_spent);
            assert_eq!(
                budget.reserve_bytes(&function, 0, "844 terminal retry"),
                Err(Diagnostic::io("SPX-H006", String::new()))
            );
        }
    }
}

#[test]
fn nested_floor_keeps_original_first_refusal_and_terminal_retry_fields() {
    let (_, function) = fixture();
    let observe = |old| {
        with_limit_usage_refusal(8192, || {
            assert!(crate::bounded_output::set_active_floor(8192));
            with_limit_usage_refusal(4096, || {
                // One real child debit crosses the restored parent's floor;
                // an unspent child refusal alone does not refuse its parent.
                assert!(crate::bounded_output::reserve_active_required(1));
                assert!(crate::bounded_output::set_active_floor(4095));
                let mut budget = ReplayBudget::new();
                let first = if old {
                    Automaton::default().node(&function, &mut budget)
                } else {
                    current::Automaton::default().node(&function, &mut budget)
                };
                let retry = budget.reserve_bytes(&function, 0, "844 floor retry");
                crate::bounded_output::clear_active_floor();
                assert!(budget
                    .reserve_bytes(&function, 0, "844 restored floor retry")
                    .is_err());
                (first, retry, budget.remaining)
            })
        })
    };
    let a = observe(true);
    let b = observe(false);
    assert_eq!(
        a, b,
        "complete first and retry Diagnostics, both ledger receipts, and work"
    );
    assert!(a.0 .0 .0.is_err() && a.0 .0 .1.is_err() && a.0 .1 && a.1);
    assert_eq!((a.0 .2, a.2), (1, 1));
    let child = a.0 .3.unwrap();
    assert!(child.requested > 0);
    assert_eq!((child.remaining, child.floor), (4095, 4095));
    let parent = a.3.unwrap();
    assert_eq!(
        (parent.requested, parent.remaining, parent.floor),
        (1, 8192, 8192)
    );
}

#[test]
fn complete_original_receivers_and_all_new_source_owners_remain_authenticated() {
    use sha2::{Digest, Sha256};
    let digest = |s: &str| {
        format!(
            "{:x}",
            crate::digest_hex::LowerHex(Sha256::digest(s.as_bytes()))
        )
    };
    let packet: serde_json::Value = serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/docs/evidence/compact-automaton-844/source-inverses.json"
    )))
    .unwrap();
    for entry in packet.as_array().unwrap() {
        let path = entry["source"].as_str().unwrap();
        let raw =
            std::fs::read_to_string(std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(path))
                .unwrap();
        let current = crate::source_reflows_merge::compact844_input(path, &raw);
        assert_eq!(digest(&current), entry["current_sha256"]);
        let new = entry["edits"][0]["new"].as_str().unwrap();
        let old = entry["edits"][0]["old"].as_str().unwrap();
        assert_eq!(current, new);
        assert_eq!(digest(old), entry["original_sha256"]);
        let archive = std::fs::read_to_string(
            std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                .join(entry["archive"].as_str().unwrap()),
        )
        .unwrap();
        assert_eq!(old, archive);
    }
    let receivers: serde_json::Value = serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/docs/evidence/compact-automaton-844/old-receiver-adaptations.json"
    )))
    .unwrap();
    for entry in receivers.as_array().unwrap() {
        let path = entry["source"].as_str().unwrap();
        let raw =
            std::fs::read_to_string(std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(path))
                .unwrap();
        let compiled = crate::source_reflows_merge::compact844_input(path, &raw);
        assert_eq!(compiled, entry["compiled"]);
        assert_eq!(digest(&compiled), entry["compiled_sha256"]);
        let original = entry["original"].as_str().unwrap();
        assert_eq!(digest(original), entry["original_sha256"]);
        assert_eq!(
            original,
            std::fs::read_to_string(
                std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                    .join(entry["archive"].as_str().unwrap())
            )
            .unwrap()
        );
    }
    let owners: serde_json::Value = serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/docs/evidence/compact-automaton-844/new-owners.json"
    )))
    .unwrap();
    assert_eq!(owners.as_array().unwrap().len(), 7);
    for entry in owners.as_array().unwrap() {
        let path = entry["source"].as_str().unwrap();
        let raw =
            std::fs::read_to_string(std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(path))
                .unwrap();
        assert_eq!(
            digest(&crate::source_reflows_merge::compact844_input(path, &raw)),
            entry["sha256"]
        );
    }
}

#[test]
fn complete_build_comparison_and_canonical_receivers_report_physical_costs_through_drop() {
    for source in [
        current::lexical_plan_tests::WHILE,
        current::lexical_plan_tests::GENERATED,
    ] {
        let (program, function) = current::lexical_plan_tests::fixture(source);
        let observe = |old, limit| {
            with_limit_usage_refusal(limit, || {
                measure(|| {
                    let mut budget = ReplayBudget::new();
                    let result = if old {
                        compare(&program, &function, &mut budget)
                    } else {
                        current::compare(&program, &function, &mut budget)
                    };
                    let accepted = matches!(result, Ok(Some(true)));
                    drop(result);
                    drop(budget);
                    accepted
                })
            })
        };
        let ((a, ma), ra, ca, _) = observe(true, usize::MAX);
        let ((b, mb), rb, cb, _) = observe(false, usize::MAX);
        assert!(a && b && !ra && !rb);
        assert!(!ma.invalid && !mb.invalid);
        assert_eq!((ma.live, mb.live), (0, 0));
        assert_eq!(ma.allocated, ma.freed);
        assert_eq!(mb.allocated, mb.freed);
        eprintln!(
            "844 immediate-parent common-observation comparison {} old_reserved={ca} new_reserved={cb} old={ma:?} new={mb:?}",
            function.id
        );
        // No universal cost inequality: branching promotion can cost more.
        let ((exact, m), refused, charged, _) = observe(false, cb);
        assert!(exact && !refused && !m.invalid && m.live == 0);
        assert_eq!(charged, cb);
        let ((short, m), refused, _, receipt) = observe(false, cb - 1);
        assert!(!short && refused && receipt.is_some() && !m.invalid && m.live == 0);
        assert_eq!(m.allocated, m.freed);
        let ((zero, m), refused, _, _) = observe(false, 0);
        assert!(!zero && refused && !m.invalid && m.live == 0);
    }
}

#[test]
fn immediate_parent_831_and_compact_storage_have_independent_complete_source_domains() {
    use sha2::{Digest, Sha256};
    let digest = |s: &str| {
        format!(
            "{:x}",
            crate::digest_hex::LowerHex(Sha256::digest(s.as_bytes()))
        )
    };
    let rows: serde_json::Value = serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/docs/evidence/compact-automaton-844-immediate7a87/immediate-old-receivers.json"
    )))
    .unwrap();
    assert_eq!(rows.as_array().unwrap().len(), 5);
    for row in rows.as_array().unwrap() {
        let path = row["source"].as_str().unwrap();
        let raw =
            std::fs::read_to_string(std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(path))
                .unwrap();
        let compiled = crate::source_reflows_merge::compact844_current_input(path, &raw);
        assert_eq!(compiled, row["compiled"]);
        assert_eq!(digest(&compiled), row["compiled_sha256"]);
        let original = std::fs::read_to_string(
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(row["archive"].as_str().unwrap()),
        )
        .unwrap();
        assert_eq!(original, row["original"]);
        assert_eq!(digest(&original), row["original_sha256"]);
    }
    let owners: serde_json::Value = serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/docs/evidence/compact-automaton-844-immediate7a87/new-owners.json"
    )))
    .unwrap();
    assert_eq!(owners.as_array().unwrap().len(), 7);
    for row in owners.as_array().unwrap() {
        let path = row["source"].as_str().unwrap();
        let raw =
            std::fs::read_to_string(std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(path))
                .unwrap();
        assert_eq!(
            digest(&crate::source_reflows_merge::compact844_current_input(
                path, &raw
            )),
            row["sha256"]
        );
    }
}
