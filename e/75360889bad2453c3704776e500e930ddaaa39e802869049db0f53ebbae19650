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
    let function = program
        .functions()
        .get(0)
        .expect("flat fanout fixture function");
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
            // Distinct owned-source groups remain independently active after
            // merged_paths; one gate decision must fund BOTH actual shares.
            let mut second = prefix.clone();
            second.owned_source = Some(CleanupPlace::whole(StorageId::ProvisionalResult));
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
                vec![prefix, second, stopped],
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
            assert_eq!(result.len(), 9);
            for path in &result[..8] {
                assert_eq!(path.0, vec![expected.clone()]);
            }
            assert_eq!(
                result[1], result[2],
                "duplicate failures retain multiplicity"
            );
            assert!(result[3].2 && result[3].3.is_some());
            assert!(result[8].1 && result[8].0 == vec![vec![]]);
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
    assert_eq!(current.2, old.2 + 6);
}

// Independent typed invoice for this real leaf Seq(None, items): no edge means
// zero pending slots. The moved Vec is acquired once; the unchanged selected
// constructor pays its shared scratch Rc and node Rc before publishing either.
fn invoice(copies: usize) -> usize {
    fn rc<T>() -> usize {
        std::alloc::Layout::new::<[Cell<usize>; 2]>()
            .extend(std::alloc::Layout::new::<T>())
            .unwrap()
            .0
            .pad_to_align()
            .size()
    }
    let receivers = 2 * size_of::<(&mut SkeletonWork<'static, 'static>, &mut Observations)>()
        + 2 * size_of::<&mut Option<Rc<ObservationDag>>>()
        + 2 * size_of::<&mut Vec<Observation>>()
        + 2 * size_of::<(Option<Rc<ObservationDag>>, Vec<Observation>)>()
        + 2 * size_of::<&mut SkeletonWork<'static, 'static>>()
        + 2 * size_of::<ObservationDagKind>()
        + 2 * size_of::<Result<Rc<ObservationDag>, Diagnostic>>()
        + 2 * size_of::<(&mut ReplayBudget, &ResolvedFunction, usize, &str)>()
        + 2 * size_of::<Result<(), Diagnostic>>()
        + 2 * size_of::<Result<(), Diagnostic>>();
    copies * (receivers + 4 * size_of::<Observation>() + rc::<ObservationDag>())
        + rc::<RefCell<DestructionScratch>>()
}
fn storage_probe(
    program: &ResolvedProgram,
    limit: usize,
    copies: usize,
) -> (bool, usize, crate::test_allocation_observer::Metrics) {
    let function = program
        .functions()
        .get(0)
        .expect("flat fanout fixture function");
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
            for _ in 0..copies {
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
                let sharing = work.prefix_tail_sharing(&[empty_expr_path(), empty_expr_path()])?;
                assert!(matches!(sharing, PrefixSharing::Flat));
                share_prefix!(work, sharing, observations);
                assert!(observations.items.is_empty());
                let ObservationDagKind::Seq(None, moved) =
                    &observations.prefix.as_ref().unwrap().kind
                else {
                    panic!("exact leaf sequence receiver");
                };
                assert_eq!(moved.len(), 4);
                drop(observations);
            }
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
    let function = program
        .functions()
        .get(0)
        .expect("flat fanout fixture function");
    // Compute admission independently; it is outside the new storage invoice.
    let mut budget = ReplayBudget::with_physical(function, 1 << 24).unwrap();
    budget.skeleton_remaining = MAX_REPLAY_WORK_UNITS;
    budget.merge_paths = true;
    let work = SkeletonWork::new(&program, function, &mut budget);
    let admission = work.budget.physical.as_ref().unwrap().charged();
    drop(work);
    for copies in [1, 2] {
        let abundant = storage_probe(&program, 1 << 24, copies);
        assert!(abundant.0);
        assert_eq!(abundant.1, invoice(copies));
        assert!(abundant.2.allocated <= invoice(copies) && abundant.2.highwater <= invoice(copies));
        assert!(storage_probe(&program, admission + invoice(copies), copies).0);
        assert!(!storage_probe(&program, admission + invoice(copies) - 1, copies).0);
        assert!(!storage_probe(&program, admission, copies).0);
    }
}

#[test]
fn flat_fanout_keeps_earlier_work_and_semantic_refusals_first() {
    let program = fixture();
    let function = program
        .functions()
        .get(0)
        .expect("flat fanout fixture function");
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
                .prefix_tail_sharing(&[empty_expr_path(), empty_expr_path()])
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
    // Full token layout substitutes the same original Boolean/Result slots;
    // no independent constructor receiver is credited from this decision.
    assert_eq!(size_of::<PrefixSharing>(), size_of::<bool>());
    assert_eq!(
        size_of::<Result<PrefixSharing, Diagnostic>>(),
        size_of::<Result<bool, Diagnostic>>()
    );
    let method = r#####"    /// Decide current sharing without changing historical Boolean callers.
    pub(in crate::cleanup_plan::replay) fn prefix_tail_sharing(
        &mut self,
        suffixes: &[ExprSkeletonPath],
    ) -> Result<PrefixSharing, Diagnostic> {
        #[cfg(test)]
        if flat_fanout::before_requested() {
            return flat_fanout::before(self, suffixes).map(|share| {
                if share {
                    PrefixSharing::Existing
                } else {
                    PrefixSharing::Disabled
                }
            });
        }
        if !self.budget.merge_paths || suffixes.len() < 2 {
            return Ok(PrefixSharing::Disabled);
        }
        self.charge(suffixes.len(), "factored suffix fanout inspection")?;
        if suffixes
            .iter()
            .any(|suffix| suffix.observations.prefix.is_some())
        {
            return Ok(PrefixSharing::Existing);
        }
        // A decision is not an allocation bank: each actual Flat share below
        // must acquire FLAT_FANOUT_CONTROLS independently before its call.
        Ok(PrefixSharing::Flat)
    }

"#####;
    let header = r#####"// This replaces the old Boolean decision in the same caller-local slot. Its
// Flat arm alone acquires the additional real sharing receivers PER PREFIX.
#[derive(Clone, Copy, Debug)]
#[repr(u8)]
pub(in crate::cleanup_plan::replay) enum PrefixSharing {
    Disabled,
    Existing,
    Flat,
}

// Reusing the old decision slots is permitted only for an identical target
// layout, independently of whether the owning runtime tests are selected.
const _: () = assert!(
    std::mem::size_of::<PrefixSharing>() == std::mem::size_of::<bool>()
        && std::mem::size_of::<Result<PrefixSharing, Diagnostic>>()
            == std::mem::size_of::<Result<bool, Diagnostic>>()
);

// New flat-only gate pays the actual/formal prefix receiver, moved kind and
// result carriers, including its own explicit reservation call, before either
// control flow or construction can acquire those receivers on this new arm.
pub(in crate::cleanup_plan::replay) const FLAT_FANOUT_CONTROLS: usize = 2 * std::mem::size_of::<(&mut SkeletonWork<'static, 'static>, &mut Observations)>()
        + 2 * std::mem::size_of::<&mut Option<Rc<ObservationDag>>>() // Option::take actual + formal
        + 2 * std::mem::size_of::<&mut Vec<Observation>>() // mem::take actual + formal
        + 2 * std::mem::size_of::<(Option<Rc<ObservationDag>>, Vec<Observation>)>()
        + 2 * std::mem::size_of::<&mut SkeletonWork<'static, 'static>>() // charged_dag actual + formal
        + 2 * std::mem::size_of::<ObservationDagKind>()
        + 2 * std::mem::size_of::<Result<Rc<ObservationDag>, Diagnostic>>()
        + 2 * std::mem::size_of::<(&mut ReplayBudget, &ResolvedFunction, usize, &str)>()
        + 2 * std::mem::size_of::<Result<(), Diagnostic>>() // reserve actual + returned receiver
        + 2 * std::mem::size_of::<Result<(), Diagnostic>>(); // share return + caller receiver

// A macro keeps the exact original sharing call in the caller's lexical frame;
// there is no new callee whose actual receiver could precede this payment.
macro_rules! share_prefix {
    ($work:ident, $sharing:ident, $observations:expr) => {{
        match $sharing {
            $crate::cleanup_plan::replay::skeleton_work::dag_storage::PrefixSharing::Disabled => {}
            $crate::cleanup_plan::replay::skeleton_work::dag_storage::PrefixSharing::Existing => {
                $work.share_prefix_tail(&mut $observations)?;
            }
            $crate::cleanup_plan::replay::skeleton_work::dag_storage::PrefixSharing::Flat => {
                $work.budget.reserve_bytes(
                    $work.function,
                    $crate::cleanup_plan::replay::skeleton_work::dag_storage::FLAT_FANOUT_CONTROLS,
                    "flat fanout shared-prefix receivers",
                )?;
                $work.share_prefix_tail(&mut $observations)?;
            }
        }
    }};
}
pub(in crate::cleanup_plan::replay) use share_prefix;

"#####;
    assert_eq!(current.matches(method).count(), 1);
    assert_eq!(current.matches(header).count(), 1);
    let restored = current.replacen(method, "", 1).replacen(header, "", 1);
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

#[test]
fn complete_current_callers_restore_whole_predecessors_without_touching_old_routes() {
    use sha2::{Digest, Sha256};
    let hash = |raw: &str| {
        format!(
            "{:x}",
            crate::digest_hex::LowerHex(Sha256::digest(raw.as_bytes()))
        )
    };
    let mut root = include_str!("../../../replay.rs").to_owned();
    // Reverse exactly the new public shared-call receiver before composing
    // the independent DAG and HIR feature inverses (full f1d66/3666 root).
    let current_call = r#####"            let observations = ordinary_plan_history::completed_fork::emission::extend_call!(
                observations, suffix, work
            )?;"#####;
    let old_call = r#####"            let observations = work.extend_owned_observations(
                observations,
                &suffix.observations,
                "call suffix clone",
            )?;"#####;
    assert_eq!(root.matches(current_call).count(), 1);
    root = root.replacen(current_call, old_call, 1);
    assert_eq!(root.len(), 326_654);
    assert_eq!(
        hash(&root),
        "853eccb0d72747e0d824711d9973c2ca6404ef20f5a3e3ed370dff8239f57d6c"
    );
    // Finite additive HIR stage from combined 3666e9a27 to the exact DAG-only
    // 48165bb59 root. The complete original DAG inverse below is unchanged.
    let added_dispatch = concat!(
        "    let prefixes = ordinary_plan_history::completed_fork::emission::dispatch!(\n",
        "        prefixes, suffixes, work\n",
        "    );\n",
    );
    assert_eq!(root.matches(added_dispatch).count(), 1);
    root = root.replacen(added_dispatch, "", 1);
    let added_finalizer = "    ordinary_plan_history::completed_fork::shipping::finish!(function, budget, expected);\n";
    assert_eq!(root.matches(added_finalizer).count(), 1);
    root = root.replacen(added_finalizer, "", 1);
    assert_eq!(root.len(), 326_445);
    assert_eq!(
        hash(&root),
        "be66867f23d4e5972e3faa68a12cfbd3b1f3f10c06a0a161b52d45fa91c29722"
    );
    let current = r#####"let share_prefix = work.prefix_tail_sharing(&suffixes)?;"#####;
    let old = r#####"let share_prefix = work.shares_prefix_tail(&suffixes)?;"#####;
    assert_eq!(root.matches(current).count(), 2);
    root = root.replace(current, old);
    let current = r#####"sequence_vector::reuse!(prefixes, suffixes, work, token share_prefix, minimum, precharged);"#####;
    let old = r#####"sequence_vector::reuse!(prefixes, suffixes, work, share_prefix, minimum, precharged);"#####;
    assert_eq!(root.matches(current).count(), 1);
    root = root.replace(current, old);
    let current = r#####"        if !precharged {
            skeleton_work::dag_storage::share_prefix!(work, share_prefix, prefix.observations);
        }"#####;
    let old = r#####"        if !precharged && share_prefix {
            work.share_prefix_tail(&mut prefix.observations)?;
        }"#####;
    assert_eq!(root.matches(current).count(), 1);
    root = root.replace(current, old);
    let current = r#####"        skeleton_work::dag_storage::share_prefix!(work, share_prefix, prefix.observations);

"#####;
    let old = r#####"        if share_prefix {
            work.share_prefix_tail(&mut prefix.observations)?;
        }
"#####;
    assert_eq!(root.matches(current).count(), 1);
    root = root.replace(current, old);
    assert_eq!(
        hash(&root),
        "5b40d53c43ccf6335bd2fbaf33c3c57d75b5bcfab2a7d6ebbd03851d77ee6e9d"
    );
    let sequence = include_str!("../../sequence_vector.rs");
    let added_arm = r#####"    ($prefixes:ident, $suffixes:ident, $work:ident, token $sharing:ident, $minimum:ident, $precharged:ident) => {{
        $crate::cleanup_plan::replay::sequence_vector::reuse!(
            $prefixes, $suffixes, $work,
            matches!($sharing, $crate::cleanup_plan::replay::skeleton_work::dag_storage::PrefixSharing::Existing),
            $minimum, $precharged);
    }};
"#####;
    assert_eq!(sequence.matches(added_arm).count(), 1);
    assert_eq!(sequence.matches("$share:expr").count(), 2);
    let restored = sequence
        .replacen(added_arm, "", 1)
        .replace("$share:expr", "$share:ident");
    assert_eq!(
        hash(&restored),
        "7a1f98ebe875f5f9e7e18882722f5e7db5bf259466c99c9d2884f7584be300bb"
    );
}
