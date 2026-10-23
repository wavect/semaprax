//! Complete independent canonical owner and ordinary immutable-label controls.
use super::*;
use crate::cleanup_plan::replay::{SkeletonObservation, SkeletonTerminal, MAX_REPLAY_WORK_UNITS};
use crate::hir::ExpressionId;
use sha2::{Digest, Sha256};
const CAP: usize = 64 * 1024 * 1024;
fn function() -> ResolvedFunction {
    crate::hir::resolve(
        &crate::check(
            "module ordinary.labels; @id(\"f\") fn f()->i64 {1}",
            "labels.spx",
        )
        .unwrap(),
    )
    .unwrap()
    .functions[0]
        .clone()
}
fn graph(
    f: &ResolvedFunction,
    width: usize,
    cycle: bool,
    unequal: bool,
) -> (Automaton, usize, usize, usize) {
    let mut a = Automaton::default();
    let mut b = ReplayBudget::unobserved();
    let sink = a.node(f, &mut b).unwrap();
    let left = a.node(f, &mut b).unwrap();
    let right = a.node(f, &mut b).unwrap();
    for (side, root) in [left, right].into_iter().enumerate() {
        for n in 0..width {
            let node = a.node(f, &mut b).unwrap();
            // Distinct Rc and identifier backing on each side must compare
            // through complete content, never source-address identity.
            let label = Label::Observation(Rc::new(SkeletonObservation::Boolean {
                expression: ExpressionId::from_owned(format!(
                    "label.{n}.{}",
                    usize::from(unequal && side == 1)
                )),
                value: n % 2 == 0,
            }));
            a.movement(f, &mut b, root, label, node).unwrap();
            for terminal in [
                SkeletonTerminal::Success,
                SkeletonTerminal::Failure,
                SkeletonTerminal::LoopLatch,
            ] {
                // Duplicate labels/targets retain exact accepted multiplicity.
                for _ in 0..2 {
                    a.movement(f, &mut b, node, Label::End(terminal), sink)
                        .unwrap();
                }
            }
        }
        if cycle {
            a.epsilon(f, &mut b, root, root).unwrap();
        }
    }
    (a, sink, left, right)
}
type States = Vec<(Vec<(usize, u64)>, usize)>;
type Signatures = Vec<((u64, Vec<(Label, usize)>), usize)>;
#[derive(Debug, PartialEq, Eq)]
struct Run {
    result: Result<Option<bool>, String>,
    states: States,
    signatures: Signatures,
    remaining: usize,
    spent: usize,
}
fn run(
    old: bool,
    f: &ResolvedFunction,
    a: &Automaton,
    sink: usize,
    roots: (usize, usize),
    weight: u64,
    work: usize,
) -> Run {
    let mut b = ReplayBudget::with_skeleton_limit(work);
    b.remaining = work;
    let mut states = Vec::new();
    let mut signatures = Vec::new();
    macro_rules! compare {
        ($c:ident) => {{
            let first = $c.id(f, vec![(roots.0, weight)], &mut b)?;
            let second = $c.id(f, vec![(roots.1, weight)], &mut b)?;
            if !b.pays_allocations() {
                // Full-state diagnostic capture belongs only to the unobserved
                // comparison. Funded controls exercise the real owner without
                // adding an unpriced test-output clone inside its account.
                states = $c
                    .states
                    .iter()
                    .map(|(key, value)| (key.as_slice().to_vec(), *value))
                    .collect();
                signatures = $c
                    .ids
                    .iter()
                    .map(|(key, value)| (key.clone(), *value))
                    .collect();
            }
            Ok(first.zip(second).map(|(x, y)| x == y))
        }};
    }
    let result: Result<Option<bool>, Diagnostic> = (|| {
        if old {
            let mut c = super::super::Canonicalizer::new(a, sink);
            compare!(c)
        } else {
            let mut c = Canonicalizer::new(a, sink);
            compare!(c)
        }
    })();
    Run {
        result: result.map_err(|d| d.json()),
        states,
        signatures,
        remaining: b.remaining,
        spent: b.skeleton_spent,
    }
}

#[test]
fn whole_immediate_source_inverse_changes_only_the_two_ordinary_dispatch_arms() {
    let before = include_str!("ordinary_labels.before.txt");
    assert_eq!(before.len(), 55814);
    assert_eq!(
        format!(
            "{:x}",
            crate::digest_hex::LowerHex(Sha256::digest(before.as_bytes()))
        ),
        "b6471558eaf00c76a99472843a9d96a58a11fe9e5779f50fdccbfc6f0011aa51"
    );
    let current = include_str!("../inline_singleton.rs");
    let mut restored = current
        .strip_suffix("\n#[cfg(test)]\nmod ordinary_labels;\n")
        .unwrap()
        .to_owned();
    assert_eq!(
        restored
            .matches("AllocationPayer::ActiveBuilder | AllocationPayer::LegacyUnobserved => {")
            .count(),
        2
    );
    restored = restored.replace(
        "AllocationPayer::ActiveBuilder | AllocationPayer::LegacyUnobserved => {",
        "AllocationPayer::ActiveBuilder => {",
    );
    for indent in ["                ", "            "] {
        let marker =
            format!("{indent}AllocationPayer::SelectedReservation => {{}}\n{indent}_ => {{");
        assert_eq!(restored.matches(&marker).count(), 1);
        restored = restored.replacen(&marker, &format!("{indent}AllocationPayer::SelectedReservation => {{}}\n{indent}AllocationPayer::LegacyUnobserved => {{}}\n{indent}_ => {{"), 1);
    }
    assert_eq!(restored.as_bytes(), before.as_bytes());
}

#[test]
fn ordinary_borrowed_labels_keep_all_weighted_states_full_labels_and_terminals() {
    let f = function();
    for width in [0, 1, 2, 3, 8] {
        for unequal in [false, true] {
            let (a, sink, left, right) = graph(&f, width, false, unequal);
            for weight in [1, 2, 7] {
                let old = run(
                    true,
                    &f,
                    &a,
                    sink,
                    (left, right),
                    weight,
                    MAX_REPLAY_WORK_UNITS,
                );
                let current = run(
                    false,
                    &f,
                    &a,
                    sink,
                    (left, right),
                    weight,
                    MAX_REPLAY_WORK_UNITS,
                );
                assert_eq!(
                    current, old,
                    "width={width} weight={weight} unequal={unequal}"
                );
                assert_eq!(current.result, Ok(Some(!unequal || width == 0)));
            }
        }
    }
}

#[test]
fn every_small_original_work_cut_cycle_and_multiplicity_overflow_keeps_six_fields() {
    let f = function();
    let (a, sink, left, right) = graph(&f, 2, false, false);
    let complete = run(true, &f, &a, sink, (left, right), 1, MAX_REPLAY_WORK_UNITS);
    let work = MAX_REPLAY_WORK_UNITS - complete.remaining;
    for cut in 0..=work {
        assert_eq!(
            run(false, &f, &a, sink, (left, right), 1, cut),
            run(true, &f, &a, sink, (left, right), 1, cut),
            "cut={cut}"
        );
    }
    let current = run(
        false,
        &f,
        &a,
        sink,
        (left, right),
        u64::MAX,
        MAX_REPLAY_WORK_UNITS,
    );
    let old = run(
        true,
        &f,
        &a,
        sink,
        (left, right),
        u64::MAX,
        MAX_REPLAY_WORK_UNITS,
    );
    assert_eq!(current, old);
    assert_eq!(current.result, Ok(None));
    let (a, sink, left, right) = graph(&f, 2, true, false);
    let old = run(true, &f, &a, sink, (left, right), 1, MAX_REPLAY_WORK_UNITS);
    let current = run(false, &f, &a, sink, (left, right), 1, MAX_REPLAY_WORK_UNITS);
    assert!(old.result.is_err());
    assert_eq!(current, old);
}

#[test]
fn same_invocation_authentication_rejects_foreign_function_and_stays_failed() {
    let f = function();
    let foreign = f.clone();
    let (a, sink, root, _) = graph(&f, 2, false, false);
    let mut b = ReplayBudget::unobserved();
    let mut c = Canonicalizer::new(&a, sink);
    assert!(c.id(&f, vec![(root, 1)], &mut b).unwrap().is_some());
    let before = (b.remaining, b.skeleton_remaining, b.skeleton_spent);
    let first = c.id(&foreign, vec![(root, 1)], &mut b).unwrap_err();
    assert_eq!(first.code, "SPX-H006");
    assert_eq!(
        (b.remaining, b.skeleton_remaining, b.skeleton_spent),
        before
    );
    assert_eq!(
        c.id(&f, vec![(root, 1)], &mut b).unwrap_err().code,
        "SPX-H006"
    );
}

#[test]
fn existing_funded_route_keeps_exact_short_zero_and_full_owner_drop() {
    let f = function();
    let (a, sink, left, right) = graph(&f, 3, false, false);
    let (ample, refused, paid) = crate::bounded_output::with_limit_usage(CAP, || {
        run(false, &f, &a, sink, (left, right), 1, MAX_REPLAY_WORK_UNITS)
    });
    assert_eq!(ample.result, Ok(Some(true)));
    assert!(!refused && paid > 0);
    for cap in [paid, paid - 1, 0] {
        let (actual, refused, _) = crate::bounded_output::with_limit_usage(cap, || {
            run(false, &f, &a, sink, (left, right), 1, MAX_REPLAY_WORK_UNITS)
        });
        assert_eq!(actual.result.is_ok(), cap == paid);
        assert_eq!(refused, cap != paid);
        let (_, metrics) = crate::test_allocation_observer::measure(|| {
            drop(crate::bounded_output::with_limit_usage(cap, || {
                run(false, &f, &a, sink, (left, right), 1, MAX_REPLAY_WORK_UNITS)
            }));
        });
        assert!(!metrics.invalid);
        assert_eq!(metrics.live, 0);
        assert_eq!(metrics.allocated, metrics.freed);
    }
    for old in [true, false] {
        let (_, metrics) = crate::test_allocation_observer::measure(|| {
            drop(run(
                old,
                &f,
                &a,
                sink,
                (left, right),
                1,
                MAX_REPLAY_WORK_UNITS,
            ));
        });
        assert!(!metrics.invalid);
        assert_eq!(metrics.live, 0);
        assert_eq!(metrics.allocated, metrics.freed);
        eprintln!(
            "complete canonical owner with full-state diagnostic capture old={old} {metrics:?}"
        );
    }
}
