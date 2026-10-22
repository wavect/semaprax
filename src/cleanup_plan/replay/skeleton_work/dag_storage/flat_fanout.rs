//! Flat fanout reuses the paid DAG owner; original whole source stays independent.
use super::*;
use std::cell::Cell;
use std::mem::size_of;

thread_local! {
    static BEFORE: Cell<bool> = const { Cell::new(false) };
}
pub(super) fn before_requested() -> bool {
    BEFORE.with(Cell::get)
}
fn with_before<T>(before: bool, run: impl FnOnce() -> T) -> T {
    struct Restore(bool);
    impl Drop for Restore {
        fn drop(&mut self) {
            BEFORE.with(|flag| flag.set(self.0));
        }
    }
    let restore = Restore(BEFORE.with(|flag| flag.replace(before)));
    let result = run();
    drop(restore);
    result
}

// Complete previous predicate, independently executable. The source test below
// reconstructs its original method spelling against the full immutable owner.
pub(super) fn before(
    work: &mut SkeletonWork<'_, '_>,
    suffixes: &[ExprSkeletonPath],
) -> Result<bool, Diagnostic> {
    if !work.budget.merge_paths || suffixes.len() < 2 {
        return Ok(false);
    }
    work.charge(suffixes.len(), "factored suffix fanout inspection")?;
    Ok(suffixes
        .iter()
        .any(|suffix| suffix.observations.prefix.is_some()))
}

fn fixture() -> ResolvedProgram {
    let source = "module fanout.storage; @id(\"main\") fn main()->i64 {0}";
    let ast = crate::check(source, "flat-fanout.spx").unwrap();
    crate::hir::resolve(&ast).unwrap()
}

// Small independent language expansion: no production cursor, memo, automaton,
// canonicalizer, pointer equality, or strategy census supplies the expected paths.
fn language(dag: &ObservationDag) -> Vec<Vec<Observation>> {
    match &dag.kind {
        ObservationDagKind::Seq(prefix, items) => {
            let mut words = prefix
                .as_ref()
                .map_or_else(|| vec![vec![]], |p| language(p));
            for word in &mut words {
                word.extend_from_slice(items);
            }
            words
        }
        ObservationDagKind::Alt(parts) => parts.iter().flat_map(|p| language(p)).collect(),
        ObservationDagKind::Cat(parts) => {
            let mut words = vec![vec![]];
            for part in parts {
                let tails = language(part);
                words = words
                    .into_iter()
                    .flat_map(|head| {
                        tails.iter().map(move |tail| {
                            let mut word = head.clone();
                            word.extend_from_slice(tail);
                            word
                        })
                    })
                    .collect();
            }
            words
        }
    }
}
fn words(observations: &Observations) -> Vec<Vec<Observation>> {
    let mut words = observations
        .prefix
        .as_ref()
        .map_or_else(|| vec![vec![]], |p| language(p));
    for word in &mut words {
        word.extend_from_slice(&observations.items);
    }
    words
}

#[test]
fn flat_fanout_preserves_complete_ordered_paths_duplicates_and_stopped_prefixes() {
    let program = fixture();
    let function = &program.functions()[0];
    let run = |old| {
        with_before(old, || {
            let mut budget = ReplayBudget::with_physical(function, 1 << 24).unwrap();
            budget.remaining = 0;
            budget.skeleton_remaining = MAX_REPLAY_WORK_UNITS;
            budget.lent_skeleton = true;
            budget.merge_paths = true;
            let mut work = SkeletonWork::new(&program, function, &mut budget);
            let mut prefix = empty_expr_path();
            let expression = work
                .clone_owned(&function.body.id, "fanout prefix identity")
                .unwrap();
            work.push_observation(
                &mut prefix,
                SkeletonObservation::Boolean {
                    expression,
                    value: true,
                },
                "fanout prefix observation",
            )
            .unwrap();
            let expected = prefix.observations.items.clone();
            let mut stopped = empty_expr_path();
            stopped.failed = true;
            let normal = empty_expr_path();
            let mut failed = empty_expr_path();
            failed.failed = true;
            let mut residual = empty_expr_path();
            residual.residual = true;
            residual.owned_source = Some(CleanupPlace::whole(StorageId::ProvisionalResult));
            reset_skeleton_materializations();
            let before_work = work.budget.skeleton_spent;
            let paths = sequence_skeleton_paths(
                vec![prefix, stopped],
                &[normal, failed.clone(), failed, residual],
                &mut work,
            )
            .unwrap();
            assert_eq!(paths[0].observations.prefix.is_some(), !old);
            let materialized = SKELETON_MATERIALIZATIONS.with(Cell::get);
            let charged_work = work.budget.skeleton_spent - before_work;
            let result = paths
                .iter()
                .map(|p| {
                    (
                        words(&p.observations),
                        p.failed,
                        p.residual,
                        p.owned_source.clone(),
                        p.continuation.clone(),
                    )
                })
                .collect::<Vec<_>>();
            assert_eq!(result.len(), 5);
            for path in &result[..4] {
                assert_eq!(path.0, vec![expected.clone()]);
            }
            assert_eq!(
                result[1], result[2],
                "duplicate failures retain multiplicity"
            );
            assert!(result[3].2 && result[3].3.is_some());
            assert!(result[4].1 && result[4].0 == vec![vec![]]);
            (result, materialized, charged_work)
        })
    };
    let old = run(true);
    let current = run(false);
    assert_eq!(current.0, old.0);
    assert_eq!(
        current.1, old.1,
        "all original materialization events remain"
    );
    // Leaf Seq owns zero child edges: two constructor/release units and one
    // authentic ordinal are additional work, never waived or hidden.
    assert_eq!(current.2, old.2 + 3);
}

// Independent typed invoice for this real leaf Seq(None, items): no edge means
// zero pending slots. The moved Vec is acquired once; the unchanged selected
// constructor pays its shared scratch Rc and node Rc before publishing either.
fn invoice() -> usize {
    fn rc<T>() -> usize {
        std::alloc::Layout::new::<[Cell<usize>; 2]>()
            .extend(std::alloc::Layout::new::<T>())
            .unwrap()
            .0
            .pad_to_align()
            .size()
    }
    let receivers = 2 * size_of::<(&mut SkeletonWork<'static, 'static>, &mut Observations)>()
        + 2 * size_of::<(Option<Rc<ObservationDag>>, Vec<Observation>)>()
        + 2 * size_of::<ObservationDagKind>()
        + 2 * size_of::<Result<Rc<ObservationDag>, Diagnostic>>()
        + 2 * size_of::<(&mut ReplayBudget, &ResolvedFunction, usize, &str)>()
        + 2 * size_of::<Result<(), Diagnostic>>();
    receivers
        + 4 * size_of::<Observation>()
        + rc::<RefCell<DestructionScratch>>()
        + rc::<ObservationDag>()
}
fn storage_probe(
    program: &ResolvedProgram,
    limit: usize,
) -> (bool, usize, crate::test_allocation_observer::Metrics) {
    let function = &program.functions()[0];
    let label = Rc::new(SkeletonObservation::Boolean {
        expression: function.body.id.clone(),
        value: true,
    });
    let mut budget = ReplayBudget::with_physical(function, limit).unwrap();
    budget.skeleton_remaining = MAX_REPLAY_WORK_UNITS;
    budget.merge_paths = true;
    let mut work = SkeletonWork::new(program, function, &mut budget);
    let initial = work.budget.physical.as_ref().unwrap().charged();
    let ((success, charged), metrics) = crate::test_allocation_observer::measure(|| {
        let result = (|| {
            work.budget
                .reserve_array::<Observation>(function, 4, "fanout input tail")?;
            let mut items = Vec::with_capacity(4);
            for _ in 0..4 {
                items.push(label.clone());
            }
            let mut observations = Observations {
                prefix: None,
                items,
            };
            assert!(work.shares_prefix_tail(&[empty_expr_path(), empty_expr_path()])?);
            work.share_prefix_tail(&mut observations)?;
            assert!(observations.items.is_empty());
            let ObservationDagKind::Seq(None, moved) = &observations.prefix.as_ref().unwrap().kind
            else {
                panic!("exact leaf sequence receiver");
            };
            assert_eq!(moved.len(), 4);
            drop(observations);
            Ok::<(), Diagnostic>(())
        })();
        let success = result.is_ok();
        drop(result);
        let charged = work.budget.physical.as_ref().unwrap().charged();
        if !success {
            assert!(work
                .budget
                .reserve_bytes(function, 0, "fanout sticky retry")
                .is_err());
            assert_eq!(work.budget.physical.as_ref().unwrap().charged(), charged);
        }
        drop(work.dag_scratch.take());
        drop(work.ordinary_scratch.take());
        (success, charged - initial)
    });
    assert!(!metrics.invalid && metrics.live == 0);
    assert_eq!(metrics.allocated, metrics.freed);
    (success, charged, metrics)
}

#[test]
fn flat_fanout_typed_storage_invoice_exact_short_zero_and_complete_drop() {
    let program = fixture();
    let function = &program.functions()[0];
    // Compute admission independently; it is outside the new storage invoice.
    let mut budget = ReplayBudget::with_physical(function, 1 << 24).unwrap();
    budget.skeleton_remaining = MAX_REPLAY_WORK_UNITS;
    budget.merge_paths = true;
    let work = SkeletonWork::new(&program, function, &mut budget);
    let admission = work.budget.physical.as_ref().unwrap().charged();
    drop(work);
    let abundant = storage_probe(&program, 1 << 24);
    assert!(abundant.0);
    assert_eq!(abundant.1, invoice());
    assert!(abundant.2.allocated <= invoice() && abundant.2.highwater <= invoice());
    assert!(storage_probe(&program, admission + invoice()).0);
    assert!(!storage_probe(&program, admission + invoice() - 1).0);
    assert!(!storage_probe(&program, admission).0);
}

#[test]
fn flat_fanout_keeps_earlier_work_and_semantic_refusals_first() {
    let program = fixture();
    let function = &program.functions()[0];
    for old in [true, false] {
        with_before(old, || {
            let mut budget = ReplayBudget::with_physical(function, 1 << 24).unwrap();
            budget.skeleton_remaining = MAX_REPLAY_WORK_UNITS;
            budget.remaining = MAX_REPLAY_WORK_UNITS;
            budget.lent_skeleton = true;
            budget.merge_paths = true;
            let mut work = SkeletonWork::new(&program, function, &mut budget);
            work.budget.skeleton_remaining = 0;
            work.budget.remaining = 0;
            let paid = work.budget.physical.as_ref().unwrap().charged();
            let error = work
                .shares_prefix_tail(&[empty_expr_path(), empty_expr_path()])
                .unwrap_err();
            assert!(error.message.contains("factored suffix fanout inspection"));
            assert_eq!(work.budget.physical.as_ref().unwrap().charged(), paid);
            assert!(work
                .budget
                .reserve_bytes(function, 0, "after earlier work refusal")
                .is_err());
        });
    }
    let detached = function.clone();
    let run = |old| {
        with_before(old, || {
            let mut budget = ReplayBudget::with_physical(&detached, 1 << 24).unwrap();
            let result = factored::compare(&program, &detached, &mut budget).unwrap_err();
            assert!(result.message.contains("not held by its current context"));
            // Semantic refusal does not invent a physical exhaustion.
            assert!(budget
                .reserve_bytes(&detached, 0, "after semantic refusal")
                .is_ok());
            (result.json(), budget.physical.as_ref().unwrap().charged())
        })
    };
    assert_eq!(run(false), run(true));
}

#[test]
fn flat_fanout_preserves_complete_old_owner_predicate_and_constructor_sources() {
    use sha2::{Digest, Sha256};
    let hash = |raw: &str| {
        format!(
            "{:x}",
            crate::digest_hex::LowerHex(Sha256::digest(raw.as_bytes()))
        )
    };
    let before = include_str!("flat_fanout.before.txt");
    assert_eq!(
        hash(before),
        "8bf934d2a558aee7fa7e3c47526fa695e58c64c61139b3a6f6b74e2cdd6a709b"
    );
    assert_eq!(
        hash(include_str!("../ordinary_dag.rs")),
        "08b5b67b03b2e80391e9a2ed4ccb936937481c88b38da9cc18fae79da2418d66"
    );
    let current = include_str!("../dag_storage.rs");
    let old = "        Ok(suffixes\n            .iter()\n            .any(|suffix| suffix.observations.prefix.is_some()))";
    let new = r###"        if suffixes
            .iter()
            .any(|suffix| suffix.observations.prefix.is_some())
        {
            return Ok(true);
        }
        // Flat success/failure siblings also clone the same owned prefix.
        // Its existing Vec can move into the already charged DAG receiver.
        self.budget.reserve_bytes(
            self.function,
            FLAT_FANOUT_CONTROLS,
            "flat fanout shared-prefix receivers",
        )?;
        Ok(true)"###;
    let dispatch = "        #[cfg(test)]\n        if flat_fanout::before_requested() {\n            return flat_fanout::before(self, suffixes);\n        }\n";
    assert_eq!(current.matches(new).count(), 1);
    assert_eq!(current.matches(dispatch).count(), 1);
    let controls = r###"// New flat-only gate pays the actual/formal prefix receiver, moved kind and
// result carriers, including its own explicit reservation call, before either
// control flow or construction can acquire those receivers on this new arm.
const FLAT_FANOUT_CONTROLS: usize = 2 * std::mem::size_of::<(
    &mut SkeletonWork<'static, 'static>,
    &mut Observations,
)>() + 2 * std::mem::size_of::<(Option<Rc<ObservationDag>>, Vec<Observation>)>()
    + 2 * std::mem::size_of::<ObservationDagKind>()
    + 2 * std::mem::size_of::<Result<Rc<ObservationDag>, Diagnostic>>()
    + 2 * std::mem::size_of::<(&mut ReplayBudget, &ResolvedFunction, usize, &str)>()
    + 2 * std::mem::size_of::<Result<(), Diagnostic>>();

"###;
    assert_eq!(current.matches(controls).count(), 1);
    let restored = current
        .replacen(new, old, 1)
        .replacen(dispatch, "", 1)
        .replacen(controls, "", 1);
    let restored = restored
        .strip_suffix("\n#[cfg(test)]\nmod flat_fanout;\n")
        .unwrap();
    assert_eq!(restored, before);
    let method = &before[before
        .find("        if !self.budget.merge_paths || suffixes.len() < 2 {")
        .unwrap()..];
    let body = method.split_once("\n    }\n").unwrap().0;
    let saved = include_str!("flat_fanout.rs");
    let saved = saved
        .split_once("    if !work.budget.merge_paths || suffixes.len() < 2 {")
        .unwrap()
        .1;
    let saved = format!("    if !work.budget.merge_paths || suffixes.len() < 2 {{{saved}");
    let saved = saved
        .split_once("\n}\n")
        .unwrap()
        .0
        .replace("work.", "self.");
    // Method indentation is a source-only distinction; preserve every byte of
    // the original body after removing precisely its one method-indent level.
    let body = body
        .lines()
        .map(|line| line.strip_prefix("    ").unwrap_or(line))
        .collect::<Vec<_>>()
        .join("\n");
    assert_eq!(saved, body);
}
