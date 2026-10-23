//! Complete predecessor semantics/work and cold physical lifetime controls.
use super::super::skeleton_work::ObservationDagKind;
use super::*;
use crate::test_allocation_observer::measure;
use sha2::{Digest, Sha256};
use std::collections::BTreeSet;

fn fixture(n: usize) -> crate::hir::ResolvedProgram {
    let calls = std::iter::repeat_n("f()", n.max(1))
        .collect::<Vec<_>>()
        .join(" + ");
    let source = format!("module ordinary; @id(\"ordinary.f\") fn f()->i64 {{ 1 }} @id(\"ordinary.main\") fn main()->i64 {{ {calls} }}");
    crate::hir::resolve(&crate::check(&source, "prefix.spx").unwrap()).unwrap()
}
fn nodes(paths: &[SkeletonPath]) -> usize {
    let mut seen = BTreeSet::new();
    for path in paths {
        let mut next = path.observations.prefix.as_ref();
        while let Some(node) = next {
            if !seen.insert(std::rc::Rc::as_ptr(node)) {
                break;
            }
            let ObservationDagKind::Seq(prefix, _) = &node.kind else {
                panic!("not a sequence")
            };
            next = prefix.as_ref();
        }
    }
    seen.len()
}
#[test]
fn complete_old_and_shared_constructor_preserve_every_skeleton_cut_and_full_diagnostic() {
    for width in [1, 2, 8] {
        let program = fixture(width);
        for function in &program.functions {
            for work in (0..512).chain([MAX_REPLAY_WORK_UNITS]) {
                let mut old_budget = ReplayBudget::with_skeleton_limit(work);
                let mut new_budget = ReplayBudget::with_skeleton_limit(work);
                old_budget.payer = AllocationPayer::LegacyUnobserved;
                new_budget.payer = AllocationPayer::LegacyUnobserved;
                let start = skeleton_materializations();
                let old = before::before(function, &mut old_budget);
                let old_events = skeleton_materializations() - start;
                let start = skeleton_materializations();
                let new = plan_skeleton_paths(function, &mut new_budget);
                assert_eq!(skeleton_materializations() - start, old_events);
                assert_eq!(new_budget.skeleton_spent, old_budget.skeleton_spent);
                assert_eq!(new_budget.skeleton_remaining, old_budget.skeleton_remaining);
                match (old, new) {
                    (Ok(mut old), Ok(mut new)) => {
                        assert_eq!(old_budget.remaining - new_budget.remaining, 4 * nodes(&new));
                        let mut comparison = ReplayBudget::unobserved();
                        assert!(equal_paths(function, &mut comparison, &mut old, &mut new).unwrap());
                    }
                    (Err(old), Err(new)) => assert_eq!(old.json(), new.json()),
                    pair => panic!("different complete constructor outcomes: {pair:?}"),
                }
            }
        }
    }
}
#[test]
fn unconditional_branch_and_late_refusals_keep_all_six_fields() {
    let program = fixture(8);
    let original = program
        .functions
        .iter()
        .find(|f| f.id.as_str() == "ordinary.main")
        .unwrap();
    for edge in 0..original.cleanup_plan.edges.len() {
        let mut function = original.clone();
        function.cleanup_plan.edges[edge].condition = EdgeCondition::Always;
        for work in [0, 1, 8, 32, 128, MAX_REPLAY_WORK_UNITS] {
            let mut old = ReplayBudget::with_skeleton_limit(work);
            let mut new = ReplayBudget::with_skeleton_limit(work);
            old.payer = AllocationPayer::LegacyUnobserved;
            new.payer = AllocationPayer::LegacyUnobserved;
            match (
                before::before(&function, &mut old),
                build(&function, &mut new),
            ) {
                (Err(a), Err(b)) => assert_eq!(a.json(), b.json()),
                (Ok(mut a), Ok(mut b)) => {
                    let mut compare = ReplayBudget::unobserved();
                    assert!(equal_paths(&function, &mut compare, &mut a, &mut b).unwrap());
                }
                pair => panic!("late refusal changed: {pair:?}"),
            }
            assert_eq!(
                (new.skeleton_spent, new.skeleton_remaining),
                (old.skeleton_spent, old.skeleton_remaining)
            );
        }
    }
}
#[test]
fn actual_cold_constructor_and_final_drop_reduce_dense_plan_backing() {
    let program = fixture(64);
    let function = program
        .functions
        .iter()
        .find(|f| f.id.as_str() == "ordinary.main")
        .unwrap();
    let run = |old| {
        measure(|| {
            // Cold budget, prefix queue and final destruction all occur in capture.
            let mut budget = ReplayBudget::with_skeleton_limit(MAX_REPLAY_WORK_UNITS);
            budget.payer = AllocationPayer::LegacyUnobserved;
            let paths = if old {
                before::before(function, &mut budget)
            } else {
                plan_skeleton_paths(function, &mut budget)
            }
            .unwrap();
            let output = (paths.len(), budget.skeleton_spent);
            drop(paths);
            drop(budget);
            output
        })
    };
    let (old_output, old) = run(true);
    let (new_output, new) = run(false);
    assert_eq!(new_output, old_output);
    assert!(!old.invalid && !new.invalid);
    assert_eq!((old.live, new.live), (0, 0));
    assert_eq!(old.allocated, old.freed);
    assert_eq!(new.allocated, new.freed);
    assert!(
        new.highwater < old.highwater,
        "actual old={old:?} current={new:?}"
    );
}
#[test]
fn paid_fallback_keeps_complete_flat_outputs_work_and_final_drop() {
    let program = fixture(8);
    let (((), metrics), refused, _, first) =
        crate::bounded_output::with_limit_usage_refusal(1 << 28, || {
            measure(|| {
                for function in &program.functions {
                    let mut old = ReplayBudget::with_skeleton_limit(MAX_REPLAY_WORK_UNITS);
                    let mut new = ReplayBudget::with_skeleton_limit(MAX_REPLAY_WORK_UNITS);
                    assert!(matches!(new.payer, AllocationPayer::ActiveBuilder));
                    let expected = before::before(function, &mut old).unwrap();
                    let actual = plan_skeleton_paths(function, &mut new).unwrap();
                    assert_eq!(actual, expected);
                    assert!(actual.iter().all(|p| p.observations.prefix.is_none()));
                    assert_eq!(
                        (new.remaining, new.skeleton_remaining, new.skeleton_spent),
                        (old.remaining, old.skeleton_remaining, old.skeleton_spent)
                    );
                    drop((actual, expected, new, old));
                }
            })
        });
    assert!(!refused && first.is_none());
    assert!(!metrics.invalid && metrics.live == 0);
    assert_eq!(metrics.allocated, metrics.freed);
}
#[test]
fn complete_predecessor_constructor_is_retained_byte_for_byte() {
    let current = include_str!("before.rs");
    let prefix = "// Complete immediate-old constructor; only visibility/name are rebound.\nuse super::*;\n#[rustfmt::skip]\n";
    assert_eq!(current.matches(prefix).count(), 1);
    let rebound = "pub(super) fn before(";
    assert_eq!(current.matches(rebound).count(), 1);
    let old = current
        .strip_prefix(prefix)
        .unwrap()
        .replacen(rebound, "fn plan_skeleton_paths(", 1);
    let whole = include_str!("replay.before.txt");
    let at = whole.find("fn plan_skeleton_paths(").unwrap();
    let end = whole[at..].find("\nfn storage_regions(").unwrap() + at;
    assert_eq!(old, &whole[at..end]);
    let digest = |b: &[u8]| format!("{:x}", crate::digest_hex::LowerHex(Sha256::digest(b)));
    assert_eq!(
        digest(whole.as_bytes()),
        "c0cb32a0e049277a41ba007b61023260bccc7d48cb0ec11c4ff3a59b74582d5d"
    );
}

#[test]
fn all_four_whole_changed_owners_restore_fixed_immediate_old_bytes() {
    let digest = |b: &[u8]| format!("{:x}", crate::digest_hex::LowerHex(Sha256::digest(b)));
    let encoded = include_str!("source_inverse.json");
    assert_eq!(
        digest(encoded.as_bytes()),
        "86f0a731a74da6427d3b8b9dfabad03a8c5b9edc647cbceae87f3bdeb32b609c"
    );
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    assert_eq!(
        std::fs::read(root.join("src/cleanup_plan/replay/plan_prefix/source_inverse.json"))
            .unwrap(),
        encoded.as_bytes()
    );
    let rows: serde_json::Value = serde_json::from_str(encoded).unwrap();
    assert_eq!(rows.as_array().unwrap().len(), 4);
    for row in rows.as_array().unwrap() {
        let path = row["path"].as_str().unwrap();
        let compiled = match path {
            "src/cleanup_plan/replay.rs" => include_bytes!("../../replay.rs").as_slice(),
            "src/cleanup_plan/replay/factored.rs" => include_bytes!("../factored.rs").as_slice(),
            "src/cleanup_plan/replay/factored/continuation_receiver.rs" => {
                include_bytes!("../factored/continuation_receiver.rs").as_slice()
            }
            "src/cleanup_plan/replay/skeleton_work.rs" => {
                include_bytes!("../skeleton_work.rs").as_slice()
            }
            _ => panic!("unbound source owner"),
        };
        let mut current = std::fs::read(root.join(path)).unwrap();
        assert_eq!(current, compiled);
        assert_eq!(
            current.len() as u64,
            row["current_length"].as_u64().unwrap()
        );
        assert_eq!(digest(&current), row["current_sha256"].as_str().unwrap());
        for edit in row["edits"].as_array().unwrap().iter().rev() {
            let at = edit[0].as_u64().unwrap() as usize;
            let after = edit[1].as_str().unwrap().as_bytes();
            assert_eq!(&current[at..at + after.len()], after);
            current.splice(at..at + after.len(), edit[2].as_str().unwrap().bytes());
        }
        let expected = match row["before_file"].as_str().unwrap() {
            "replay.before.txt" => include_bytes!("replay.before.txt").as_slice(),
            "1.before.txt" => include_bytes!("1.before.txt").as_slice(),
            "2.before.txt" => include_bytes!("2.before.txt").as_slice(),
            "3.before.txt" => include_bytes!("3.before.txt").as_slice(),
            _ => panic!("unbound predecessor"),
        };
        assert_eq!(current, expected);
        assert_eq!(current.len() as u64, row["before_length"].as_u64().unwrap());
        assert_eq!(digest(&current), row["before_sha256"].as_str().unwrap());
    }
}

#[test]
fn logical_length_capacity_and_clone_probes_match_the_actual_old_vec() {
    let program = fixture(1);
    let function = &program.functions[0];
    let mut budget = ReplayBudget::unobserved();
    let mut current = History::new();
    let mut old = Vec::new();
    for _ in 0..257 {
        assert_eq!(
            (current.len(), current.capacity()),
            (old.len(), old.capacity())
        );
        current.freeze(function, &mut budget).unwrap();
        let mut copied = current.clone();
        let mut old_copied = old.clone();
        assert_eq!(
            (copied.len(), copied.capacity()),
            (old_copied.len(), old_copied.capacity())
        );
        let observation = std::rc::Rc::new(SkeletonObservation::Boolean {
            expression: function.body.id.clone(),
            value: true,
        });
        copied.push(observation.clone());
        old_copied.push(observation.clone());
        assert_eq!(
            (copied.len(), copied.capacity()),
            (old_copied.len(), old_copied.capacity())
        );
        current.push(observation.clone());
        old.push(observation);
    }
}

#[test]
fn exact_short_zero_paid_fallback_and_partial_legacy_queues_keep_final_drop() {
    let program = fixture(8);
    let function = program
        .functions
        .iter()
        .find(|f| f.id.as_str() == "ordinary.main")
        .unwrap();
    let run = |limit| {
        crate::bounded_output::with_limit_usage_refusal(limit, || {
            measure(|| {
                let mut budget = ReplayBudget::with_skeleton_limit(MAX_REPLAY_WORK_UNITS);
                let result = plan_skeleton_paths(function, &mut budget);
                let succeeded = result.is_ok();
                drop(result);
                drop(budget);
                succeeded
            })
        })
    };
    let ((success, full), refused, exact, first) = run(1 << 28);
    assert!(success && !refused && first.is_none());
    assert!(exact > 0 && !full.invalid && full.live == 0);
    for limit in [0, exact - 1, exact] {
        let ((success, metrics), refused, _, first) = run(limit);
        assert_eq!(success, limit == exact);
        assert_eq!(refused, limit != exact);
        assert_eq!(first.is_some(), limit != exact);
        assert!(!metrics.invalid && metrics.live == 0);
        assert_eq!(metrics.allocated, metrics.freed);
    }
    for work in [0, 1, 8, 32, 128] {
        for old in [true, false] {
            let ((), metrics) = measure(|| {
                let mut budget = ReplayBudget::with_skeleton_limit(work);
                budget.payer = AllocationPayer::LegacyUnobserved;
                let result = if old {
                    before::before(function, &mut budget)
                } else {
                    build(function, &mut budget)
                };
                drop(result);
                drop(budget);
            });
            assert!(!metrics.invalid && metrics.live == 0);
            assert_eq!(metrics.allocated, metrics.freed);
        }
    }
}

#[test]
fn additional_node_release_work_has_its_own_zero_short_exact_full_diagnostic_frontier() {
    let program = fixture(8);
    let function = program
        .functions
        .iter()
        .find(|f| f.id.as_str() == "ordinary.main")
        .unwrap();
    let mut ample = ReplayBudget::with_skeleton_limit(MAX_REPLAY_WORK_UNITS);
    ample.payer = AllocationPayer::LegacyUnobserved;
    let paths = build(function, &mut ample).unwrap();
    let additional = 4 * nodes(&paths);
    assert!(
        additional > 0,
        "fixture must actually construct shared prefixes"
    );
    drop(paths);
    // The frozen constructor spends no ordinary-work units on these histories.
    let mut old = ReplayBudget::with_skeleton_limit(MAX_REPLAY_WORK_UNITS);
    old.payer = AllocationPayer::LegacyUnobserved;
    old.remaining = 0;
    assert!(before::before(function, &mut old).is_ok());
    for free in [0, 3, additional - 1, additional] {
        let mut budget = ReplayBudget::with_skeleton_limit(MAX_REPLAY_WORK_UNITS);
        budget.payer = AllocationPayer::LegacyUnobserved;
        budget.remaining = free;
        let result = build(function, &mut budget);
        if free == additional {
            assert!(result.is_ok());
            assert_eq!(budget.remaining, 0);
        } else {
            let actual = result.err().expect("new node/release work must refuse");
            let mut expected = ReplayBudget::unobserved();
            expected.remaining = 0;
            let expected = expected
                .charge(function, 4, "PLAN immutable prefix node and release")
                .unwrap_err();
            assert_eq!(actual.json(), expected.json());
        }
    }
}
