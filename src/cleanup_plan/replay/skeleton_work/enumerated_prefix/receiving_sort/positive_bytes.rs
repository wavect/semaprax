//! Whole post-948/current payment producers, not a tariff-only simulation.
use super::*;
use sha2::{Digest, Sha256};

fn epoch(old: bool) -> PositiveEpoch {
    PositiveEpoch(BEFORE_POSITIVE_BYTES.with(|value| value.replace(old)))
}
fn reset_events() {
    ADVANCE_PAIR_EVENTS.with(|v| v.set(0));
    BRANCH_FRAME_EVENTS.with(|v| v.set([0; 4]));
    POSITIVE_CHILD_EVENTS.with(|v| v.set(0));
}
fn events() -> [usize; 3] {
    [
        ADVANCE_PAIR_EVENTS.with(std::cell::Cell::get),
        BRANCH_FRAME_EVENTS.with(|v| v.get()[3]),
        POSITIVE_CHILD_EVENTS.with(std::cell::Cell::get),
    ]
}
fn removed(counts: [usize; 3]) -> usize {
    let [pairs, nodes, children] = counts;
    let arguments = 2 * size_of::<(&mut ReplayBudget, &ResolvedFunction, usize, &str)>();
    (2 * pairs + nodes + children) * arguments
}
fn observe(
    old: bool,
    input: &[SkeletonPath],
    function: &ResolvedFunction,
    work: usize,
) -> (Run, [usize; 3]) {
    let _epoch = epoch(old);
    reset_events();
    let result = run(true, input, function, work, usize::MAX, 0, false);
    (result, events())
}
fn equation(old: &(Run, [usize; 3]), new: &(Run, [usize; 3])) {
    same_work(&old.0, &new.0);
    assert_eq!(old.1, new.1);
    assert_eq!(old.0.heap, new.0.heap, "removed ABI is not heap credit");
    assert_eq!(old.0.paid, new.0.paid + removed(old.1));
}

#[test]
fn complete_sorters_preserve_work_order_and_count_every_eliminated_payment_tuple() {
    let old_banks = before_positive_bytes::banks();
    assert_eq!(old_banks[0], PAYMENT);
    let removed_arguments = 2 * size_of::<(&mut ReplayBudget, &ResolvedFunction, usize, &str)>();
    assert_eq!(
        old_banks[1],
        super::super::ADVANCE_BLOCK + removed_arguments
    );
    assert_eq!(
        old_banks[2],
        super::super::LEXICAL_NODE_STEP + removed_arguments
    );
    assert_eq!(
        old_banks[3],
        super::super::LEXICAL_CAT_CHILD_STEP + removed_arguments
    );
    assert_eq!(
        old_banks[4], FRAME_COMMON,
        "ambiguous repeated-loop roles are retained"
    );
    assert_eq!(
        POSITIVE_BYTE_PAYMENT,
        2 * size_of::<Result<(), ByteFailure>>()
    );
    let program = program();
    let function = &program.functions[0];
    for count in [0, 1, 2, 3, 12] {
        for width in [0, 1, 17] {
            let paths = fixture(function, count, width);
            let old = observe(true, &paths, function, usize::MAX);
            let new = observe(false, &paths, function, usize::MAX);
            equation(&old, &new);
            if count < 2 {
                assert_eq!(new.1, [0; 3]);
            }
            for work in [0, 1, 3, 17, 64] {
                equation(
                    &observe(true, &paths, function, work),
                    &observe(false, &paths, function, work),
                );
            }
        }
    }
    let mut hostile = fixture(function, 2, 1);
    hostile[0].observations.prefix = Some(dag(ObservationDagKind::Alt(Vec::new())));
    let old = observe(true, &hostile, function, usize::MAX);
    let new = observe(false, &hostile, function, usize::MAX);
    equation(&old, &new);
    assert!(new.0.result.is_err());
}

#[test]
fn exact_short_zero_and_floor_accounts_keep_sticky_failure_and_full_owner_teardown() {
    let program = program();
    let function = &program.functions[0];
    let paths = fixture(function, 3, 3);
    for old in [true, false] {
        let complete = observe(old, &paths, function, usize::MAX);
        assert!(complete.0.result.is_ok());
        for floor in [0, 17] {
            for short in [false, true] {
                let _epoch = epoch(old);
                let bytes = complete.0.paid + floor - usize::from(short);
                let result = run(true, &paths, function, usize::MAX, bytes, floor, false);
                assert_eq!(result.result.is_ok(), !short);
                assert_eq!(result.refused, short);
                assert_eq!(result.sticky.is_some(), short);
            }
        }
        for bytes in [0, 1, complete.0.paid / 2, complete.0.paid] {
            let _epoch = epoch(old);
            let (((), heap), _, _, _) = with_limit_usage_refusal(bytes, || {
                measure(|| {
                    let mut paths = fixture(function, 3, 3);
                    let mut budget = ReplayBudget::with_skeleton_limit(usize::MAX);
                    let result = super::super::super::sort(&mut paths, function, &mut budget);
                    drop(result);
                    drop(paths);
                    drop(budget);
                })
            });
            assert!(!heap.invalid);
            assert_eq!(heap.live, 0);
            assert_eq!(heap.allocated, heap.freed);
        }
        let _epoch = epoch(old);
        let lent = run(true, &paths, function, usize::MAX, usize::MAX, 0, true);
        assert!(lent.result.is_err());
    }
}

#[test]
fn caught_unwind_drops_the_complete_real_sort_and_restores_the_epoch() {
    struct Reset;
    impl Drop for Reset {
        fn drop(&mut self) {
            FRAME_UNWIND.with(|v| v.set(None));
        }
    }
    let program = program();
    let function = &program.functions[0];
    let paths = fixture(function, 3, 3);
    let references: Vec<_> = paths
        .iter()
        .map(|p| Rc::strong_count(p.observations.prefix.as_ref().unwrap()))
        .collect();
    for old in [true, false] {
        for cut in [0, 1, 3, 7] {
            let _epoch = epoch(old);
            let (((), heap), refused, _, _) = with_limit_usage_refusal(usize::MAX, || {
                measure(|| {
                    let mut paths = paths.clone();
                    let mut budget = ReplayBudget::with_skeleton_limit(usize::MAX);
                    let _reset = Reset;
                    FRAME_UNWIND.with(|v| v.set(Some(cut)));
                    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                        super::super::super::sort(&mut paths, function, &mut budget)
                    }));
                    assert!(result.is_err());
                    drop(result);
                    drop(paths);
                    drop(budget);
                })
            });
            assert!(!refused && !heap.invalid);
            assert_eq!(heap.live, 0);
            assert_eq!(heap.allocated, heap.freed);
            for (path, expected) in paths.iter().zip(&references) {
                assert_eq!(
                    Rc::strong_count(path.observations.prefix.as_ref().unwrap()),
                    *expected
                );
            }
        }
    }
}

// This is the complete existing source-bound producer, without sharing a
// rewritten reference enumeration with the changed sorting implementation.
fn shipping_input(program: &ResolvedProgram, function: &ResolvedFunction) -> Vec<SkeletonPath> {
    use crate::cleanup_plan::replay::{
        hir_skeleton_paths, ordinary_plan_history::completed_fork::emission,
        reserve_program_skeleton_work,
    };
    let (paths, refused, _, _) = with_limit_usage_refusal(usize::MAX, || {
        let mut budget = ReplayBudget::new();
        reserve_program_skeleton_work(program, std::iter::once(function), &mut budget).unwrap();
        emission::with_before(|| hir_skeleton_paths(program, function, &mut budget)).unwrap()
    });
    assert!(!refused);
    paths
        .into_iter()
        .map(|mut path| {
            path.observations.prefix = Some(dag(ObservationDagKind::Seq(
                path.observations.prefix.take(),
                std::mem::take(&mut path.observations.items),
            )));
            path
        })
        .collect()
}
#[test]
fn entire_shipping_caller_retains_its_independent_producer_and_late_hostile_cases() {
    use crate::cleanup_plan::replay::ordinary_plan_history::completed_fork::shipping;
    let checked = crate::check("module shipping953; @id(\"f\") fn f(a:bool,b:bool)->i64 {let n=if a {1}else{2};if b {n+1}else{n+2}} @id(\"main\") fn main()->i64 {f(true,false)}", "shipping953.spx").unwrap();
    let program = crate::hir::resolve(&checked).unwrap();
    let function = &program.functions[0];
    let input = shipping_input(&program, function);
    assert!(input.len() > 1);
    let invoke = |old, fault, work| {
        let _epoch = epoch(old);
        let mut paths = input.clone();
        match fault {
            0 => (),
            1 => {
                paths.pop();
            }
            2 => paths[0].terminal = SkeletonTerminal::LoopLatch,
            3 => paths[0].observations.items.push(item(function, false)),
            _ => unreachable!(),
        }
        reset_events();
        let ((result, paths, remaining, spent, heap), refused, paid, first) =
            with_limit_usage_refusal(usize::MAX, || {
                let mut budget = ReplayBudget::with_skeleton_limit(work);
                budget.remaining = work;
                let (result, heap) =
                    measure(|| shipping::compare(function, &mut budget, &mut paths));
                (
                    result.map_err(|e| e.json()),
                    contents(&paths),
                    budget.remaining,
                    budget.skeleton_spent,
                    heap,
                )
            });
        (
            (result, paths, remaining, spent, heap, refused, first),
            paid,
            events(),
        )
    };
    for fault in 0..4 {
        for work in [usize::MAX, 0, 1, 3, 17, 64] {
            let old = invoke(true, fault, work);
            let new = invoke(false, fault, work);
            assert_eq!(old.0, new.0);
            assert_eq!(old.2, new.2);
            assert_eq!(old.1, new.1 + removed(old.2));
            if work == usize::MAX {
                assert!(new.2[0] > 0);
                assert_eq!(new.0 .0.is_ok(), fault == 0);
            }
        }
    }
}

fn payment(
    old: bool,
    budget: &mut ReplayBudget,
    function: &ResolvedFunction,
) -> Result<(), ByteFailure> {
    if old {
        return budget.reserve_compact_bytes(
            function,
            FRAME_COMMON,
            "enumerated HIR cursor common frame receiving renewal",
        );
    }
    positive_bytes!(
        budget,
        function,
        FRAME_COMMON,
        "enumerated HIR cursor common frame receiving renewal"
    );
    Ok(())
}
#[test]
fn full_positive_producers_preserve_prepare_failure_sticky_and_nested_census_callbacks() {
    use crate::cleanup_plan::replay::merge_census;
    let program = program();
    let function = &program.functions[0];
    let invoke = |old, bytes, nested| {
        with_limit_usage_refusal(bytes, || {
            let mut budget = ReplayBudget::with_skeleton_limit(usize::MAX);
            let scope = merge_census::Scope::enter(function, &budget);
            let (first, heap) = measure(|| {
                if nested {
                    with_limit_usage_refusal(usize::MAX, || payment(old, &mut budget, function)).0
                } else {
                    payment(old, &mut budget, function)
                }
            });
            let first = first.map_err(|e| e.into_diagnostic().json());
            let report = scope.report();
            let second =
                payment(old, &mut budget, function).map_err(|e| e.into_diagnostic().json());
            (
                first,
                report,
                second,
                scope.report(),
                heap,
                budget.refusal_spent,
            )
        })
    };
    for bytes in [0, 1, FRAME_COMMON - 1, FRAME_COMMON, 1_000_000] {
        for nested in [false, true] {
            let old = invoke(true, bytes, nested);
            let new = invoke(false, bytes, nested);
            assert_eq!(old, new);
            assert!(!new.0 .4.invalid);
            if nested {
                assert!(new.0 .1.foreign > 0);
            }
        }
    }

    for old in [true, false] {
        let mut budget = ReplayBudget::new();
        assert!(matches!(
            budget.payer,
            allocation::AllocationPayer::LegacyUnobserved
        ));
        let remaining = (budget.remaining, budget.skeleton_remaining);
        assert!(payment(old, &mut budget, function).is_ok());
        assert_eq!((budget.remaining, budget.skeleton_remaining), remaining);
        assert!(!budget.refusal_spent);
    }
    let selected = |old, bytes| match ReplayBudget::with_physical(function, bytes) {
        Err(error) => (Err(error.json()), None, None, false),
        Ok(mut budget) => {
            assert!(matches!(
                budget.payer,
                allocation::AllocationPayer::SelectedReservation
            ));
            let first = payment(old, &mut budget, function).map_err(|e| e.into_diagnostic().json());
            let second =
                payment(old, &mut budget, function).map_err(|e| e.into_diagnostic().json());
            (
                first,
                Some(second),
                Some(budget.physical.as_ref().unwrap().charged()),
                budget.refusal_spent,
            )
        }
    };
    for bytes in [0, 1, FRAME_COMMON - 1, FRAME_COMMON, 1_000_000] {
        let old = selected(true, bytes);
        let new = selected(false, bytes);
        assert_eq!(old, new);
        if bytes == 1_000_000 {
            assert!(new.0.is_ok() && new.1.unwrap().is_ok());
        }
    }
    let oracle = |old, bytes| {
        use crate::cleanup_plan::replay::allocation::OracleScope;
        let owner = OracleScope::open_with_work(bytes, usize::MAX).unwrap();
        let mut budget = ReplayBudget::with_skeleton_limit_oracle(usize::MAX, owner.lease());
        let first = payment(old, &mut budget, function).map_err(|e| e.into_diagnostic().json());
        let before_second = owner.receipt();
        let second = payment(old, &mut budget, function).map_err(|e| e.into_diagnostic().json());
        (
            first,
            second,
            before_second,
            owner.finish(),
            budget.refusal_spent,
        )
    };
    for bytes in [0, 1, FRAME_COMMON - 1, FRAME_COMMON, usize::MAX] {
        let old = oracle(true, bytes);
        let new = oracle(false, bytes);
        assert_eq!(old, new);
        if bytes == usize::MAX {
            assert!(new.0.is_ok() && new.1.is_ok());
            assert!(new.3.charged >= new.2.charged + FRAME_COMMON);
        }
    }
    // A nested ACTIVE observer alone does not exercise the Oracle payer.
    // Carry the actual expired lease into the new producer and a new account.
    let expired = |old| {
        use crate::cleanup_plan::replay::allocation::OracleScope;
        let owner = OracleScope::open_with_work(usize::MAX, usize::MAX).unwrap();
        let mut budget = ReplayBudget::with_skeleton_limit_oracle(usize::MAX, owner.lease());
        assert!(owner.finish().first_refusal.is_none());
        let foreign = OracleScope::open_with_work(usize::MAX, usize::MAX).unwrap();
        let before = foreign.receipt();
        let first = payment(old, &mut budget, function)
            .unwrap_err()
            .into_diagnostic();
        let sticky = budget
            .reserve_compact_bytes(function, 0, "foreign followup")
            .unwrap_err()
            .into_diagnostic();
        assert_eq!(
            foreign.receipt(),
            before,
            "expired lease cannot spend the new account"
        );
        foreign.finish();
        (first, sticky, budget.refusal_spent)
    };
    assert_eq!(expired(true), expired(false));
}

// SOURCE_BINDINGS_953

fn hash(bytes: &[u8]) -> String {
    format!("{:x}", crate::digest_hex::LowerHex(Sha256::digest(bytes)))
}
const SOURCE_ROWS: &str = r###"{"enumerated_prefix.rs":{"path":"src/cleanup_plan/replay/skeleton_work/enumerated_prefix.rs","current":{"length":31953,"sha256":"3a39ab0f14065a9201b4cb2526e2fba164e2c775a900919ac8e9900a7fa13626"},"before":{"length":31713,"sha256":"f955df3dd3f2ceb2fdbbd2afdfe9cc3d68835600a5df60c812732b5ff4049e47"},"inverse":[[16687,16927,"            .map_err(|failure| failure.into_diagnostic());\n    }\n    #[cfg(test)]\n    if receiving_sort::BEFORE_POSITIVE_BYTES.with(std::cell::Cell::get) {\n        return receiving_sort::before_positive_bytes::sort(paths, function, budget)\n",""]]},"receiving_sort.rs":{"path":"src/cleanup_plan/replay/skeleton_work/enumerated_prefix/receiving_sort.rs","current":{"length":29160,"sha256":"d4ac2065d1cf03108387a40bdd5180d992732f8d54c94e4750f43e08b71aaba6"},"before":{"length":25200,"sha256":"6c30e1024c4219f6c64a8e08b11858068cd575ca1ba8e61c6f455fc0eb5a5d45"},"inverse":[[4713,7349,"// A fixed-positive lexical producer has no actual/formal payment tuple. The\n// complete Result pair and all transitive prepare/payer/refusal roles survive.\nconst POSITIVE_BYTE_PAYMENT: usize = 2 * size_of::<Result<(), ByteFailure>>();\n// Keep the full ordinary method banks for the retained general Cursor::next.\n// Only the exact byte-renewal sites in advance! use these narrower ABIs.\nconst LEXICAL_NODE_STEP: usize = PAYMENT + POSITIVE_BYTE_PAYMENT;\nconst LEXICAL_CAT_CHILD_STEP: usize = CAT_CHILD_STEP - PAYMENT + POSITIVE_BYTE_PAYMENT;\nmacro_rules! positive_bytes {\n    ($budget:ident, $function:ident, $bytes:ident, $phase:literal) => {{\n        const {\n            assert!($bytes > 0);\n        }\n        #[cfg(test)]\n        let ticket =\n            crate::cleanup_plan::replay::merge_census::before($budget, $function, $bytes, $phase);\n        let result = 'reservation: {\n            match $budget.prepare_builder($function) {\n                Ok(()) => (),\n                Err(error) => break 'reservation Err(ByteFailure::from_budget(error)),\n            };\n            let accepted = match $budget.payer {\n                $crate::cleanup_plan::replay::allocation::AllocationPayer::LegacyUnobserved => true,\n                $crate::cleanup_plan::replay::allocation::AllocationPayer::ActiveBuilder => {\n                    !$budget.refusal_spent\n                        && !crate::bounded_output::active_refused()\n                        && crate::bounded_output::reserve_active_required($bytes)\n                }\n                #[cfg(test)]\n                $crate::cleanup_plan::replay::allocation::AllocationPayer::Oracle(lease) => {\n                    !$budget.refusal_spent && !lease.refused() && lease.charge($bytes).is_ok()\n                }\n                $crate::cleanup_plan::replay::allocation::AllocationPayer::SelectedReservation => {\n                    $budget\n                        .physical\n                        .as_deref_mut()\n                        .expect(\"selected reservation payer\")\n                        .reserve($bytes)\n                        .is_ok()\n                }\n            };\n            if accepted {\n                Ok(())\n            } else {\n                Err($budget.compact_refusal(\n                    $function,\n                    \"cleanup replay physical reservation exhausted during \",\n                    $phase,\n                ))\n            }\n        };\n        #[cfg(test)]\n        crate::cleanup_plan::replay::merge_census::after(\n            ticket,\n            $budget,\n            $function,\n            result.is_ok(),\n        );\n        result?;\n    }};\n}\n\n",""],[7762,7776,"OSITIVE_BYTE_P",""],[8248,8293,"positive_bytes!(\n                    $budget,","$budget.reserve_compact_bytes("],[8433,8451,"\n                )",",\n                )?"],[9107,9160,"positive_bytes!(\n                            $budget,","$budget.reserve_compact_bytes("],[9311,9337,"\n                        )",",\n                        )?"],[9932,9985,"positive_bytes!(\n                            $budget,","$budget.reserve_compact_bytes("],[10139,10165,"\n                        )",",\n                        )?"],[10929,10982,"positive_bytes!(\n                            $budget,","$budget.reserve_compact_bytes("],[11127,11232,"\n                        );\n                        positive_bytes!(\n                            $budget,",",\n                        )?;\n                        $budget.reserve_compact_bytes("],[11300,11419,"LEXICAL_NODE_STEP,\n                            \"enumerated HIR node growth receiving renewal\"\n                        )","NODE_STEP,\n                            \"enumerated HIR node growth receiving renewal\",\n                        )?"],[12958,13023,"positive_bytes!(\n                                        $budget,","$budget.reserve_compact_bytes("],[13115,13380,"LEXICAL_CAT_CHILD_STEP,\n                                        \"enumerated HIR concatenation child receiving renewal\"\n                                    );\n                                    #[cfg(test)]\n                                    positive_child_event()","CAT_CHILD_STEP,\n                                        \"enumerated HIR concatenation child receiving renewal\",\n                                    )?"],[15266,15405,"    #[allow(\n        dead_code,\n        reason = \"complete general method retained beside its independently executable predecessor\"\n    )]\n",""],[28244,29160,"\n#[cfg(test)]\n#[allow(dead_code, unused_imports, unused_macros, reason = \"complete immediate post-948 sorter and independent typed tariffs\")]\n#[rustfmt::skip]\npub(super) mod before_positive_bytes;\n#[cfg(test)]\nstd::thread_local! {\n    pub(super) static BEFORE_POSITIVE_BYTES: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };\n}\n#[cfg(test)]\nstruct PositiveEpoch(bool);\n#[cfg(test)]\nimpl Drop for PositiveEpoch {\n    fn drop(&mut self) {\n        BEFORE_POSITIVE_BYTES.with(|old| old.set(self.0));\n    }\n}\n#[cfg(test)]\nfn old_positive_byte_epoch() -> PositiveEpoch {\n    PositiveEpoch(BEFORE_POSITIVE_BYTES.with(|old| old.replace(true)))\n}\n\n#[cfg(test)]\nstd::thread_local! {\n    static POSITIVE_CHILD_EVENTS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };\n}\n#[cfg(test)]\nfn positive_child_event() {\n    POSITIVE_CHILD_EVENTS.with(|events| events.set(events.get().checked_add(1).unwrap()));\n}\n",""]]},"receiving_sort/tests.rs":{"path":"src/cleanup_plan/replay/skeleton_work/enumerated_prefix/receiving_sort/tests.rs","current":{"length":25134,"sha256":"5f07abb62df0a437625541c68792b14314ea293a2d4bce792f46a2b7588f392d"},"before":{"length":24960,"sha256":"cf815cee18a5a75c341fd999f14d102fddd7aee6d61d09ff23abcde6e43b2139"},"inverse":[[93,166,"// Historical direct controls retain the exact post-948 nominal layouts.\n",""],[276,326,"use before_positive_bytes::{Cursor, Frame, Pair};\n",""],[25083,25134,"\n#[path = \"positive_bytes.rs\"]\nmod positive_bytes;\n",""]]},"receiving_sort/lexical_advance.rs":{"path":"src/cleanup_plan/replay/skeleton_work/enumerated_prefix/receiving_sort/lexical_advance.rs","current":{"length":38490,"sha256":"13464b1ec374ef476460776034fda124f91df6ef199f79717532b00f094feb1e"},"before":{"length":37850,"sha256":"7c25075ad0a7f54ef7ff6976a4da87d61f9270490e7773fb28326512bb7578de"},"inverse":[[95,128,"use before_positive_bytes::Pair;\n",""],[129,439,"const ADVANCE_BLOCK: usize = before_positive_bytes::banks()[1];\nstruct Mode {\n    _advance: AdvanceEpoch,\n    _positive: PositiveEpoch,\n}\nfn mode(old: bool) -> Mode {\n    Mode {\n        _advance: AdvanceEpoch(BEFORE_ADVANCE.with(|value| value.replace(old))),\n        _positive: old_positive_byte_epoch(),\n    }","fn mode(old: bool) -> AdvanceEpoch {\n    AdvanceEpoch(BEFORE_ADVANCE.with(|value| value.replace(old)))"],[18585,18718,"        let normalized = positive_bytes::restore_before(\"enumerated_prefix.rs\", current);\n        let current = normalized.as_str();\n",""],[20169,20299,"        let normalized = positive_bytes::restore_before(\"receiving_sort.rs\", current);\n        let current = normalized.as_str();\n",""],[31054,31190,"        let normalized = positive_bytes::restore_before(\"receiving_sort/tests.rs\", current);\n        let current = normalized.as_str();\n",""]]}}"###;
pub(super) fn restore_before(name: &str, current: &str) -> String {
    const MAX: usize = 65_536;
    let rows: serde_json::Value = serde_json::from_str(SOURCE_ROWS).unwrap();
    let row = rows.get(name).expect("closed source domain");
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let path = root.join(row["path"].as_str().unwrap());
    let metadata = std::fs::symlink_metadata(&path).unwrap();
    assert!(metadata.is_file() && !metadata.file_type().is_symlink());
    assert!(metadata.len() <= MAX as u64);
    let physical = std::fs::read_to_string(&path).unwrap();
    assert_eq!(current, physical);
    assert_eq!(
        current.len() as u64,
        row["current"]["length"].as_u64().unwrap()
    );
    assert_eq!(
        hash(current.as_bytes()),
        row["current"]["sha256"].as_str().unwrap()
    );
    let current_archive = root
        .join("e")
        .join(row["current"]["sha256"].as_str().unwrap());
    let metadata = std::fs::symlink_metadata(&current_archive).unwrap();
    assert!(metadata.is_file() && !metadata.file_type().is_symlink());
    assert!(metadata.len() <= MAX as u64);
    assert_eq!(std::fs::read(&current_archive).unwrap(), current.as_bytes());
    let archive = root
        .join("e")
        .join(row["before"]["sha256"].as_str().unwrap());
    let metadata = std::fs::symlink_metadata(&archive).unwrap();
    assert!(metadata.is_file() && !metadata.file_type().is_symlink());
    assert!(metadata.len() <= MAX as u64);
    let before = std::fs::read_to_string(&archive).unwrap();
    assert_eq!(
        before.len() as u64,
        row["before"]["length"].as_u64().unwrap()
    );
    assert_eq!(
        hash(before.as_bytes()),
        row["before"]["sha256"].as_str().unwrap()
    );
    let mut restored = current.to_owned();
    for edit in row["inverse"].as_array().unwrap().iter().rev() {
        let start = usize::try_from(edit[0].as_u64().unwrap()).unwrap();
        let end = usize::try_from(edit[1].as_u64().unwrap()).unwrap();
        assert!(start <= end && end <= restored.len());
        assert!(restored.is_char_boundary(start) && restored.is_char_boundary(end));
        assert_eq!(&restored[start..end], edit[2].as_str().unwrap());
        let previous = edit[3].as_str().unwrap();
        assert!(
            restored
                .len()
                .checked_sub(end - start)
                .unwrap()
                .checked_add(previous.len())
                .unwrap()
                <= MAX
        );
        restored.replace_range(start..end, previous);
    }
    assert_eq!(restored, before);
    assert_eq!(std::fs::read_to_string(path).unwrap(), current);
    assert_eq!(std::fs::read(current_archive).unwrap(), current.as_bytes());
    assert_eq!(std::fs::read_to_string(archive).unwrap(), before);
    restored
}
#[test]
fn entire_post948_sources_and_independent_executable_sorter_are_retained() {
    let root_before = restore_before("receiving_sort.rs", include_str!("../receiving_sort.rs"));
    restore_before(
        "enumerated_prefix.rs",
        include_str!("../../enumerated_prefix.rs"),
    );
    restore_before("receiving_sort/tests.rs", include_str!("tests.rs"));
    restore_before(
        "receiving_sort/lexical_advance.rs",
        include_str!("lexical_advance.rs"),
    );
    let executable = include_str!("before_positive_bytes.rs")
        .split_once("\n// TEST_ONLY_953_EXACT_BANK_ACCESS\n")
        .unwrap()
        .0;
    let restored = executable
        .replace("pub(in crate::cleanup_plan::replay::skeleton_work::enumerated_prefix) fn sort(", "SAVED_VISIBILITY fn sort(")
        .replace("pub(super) ", "")
        .replace("SAVED_VISIBILITY fn sort(", "pub(super) fn sort(")
        .replace("\n                                    #[cfg(test)]\n                                    super::positive_child_event();", "");
    assert_eq!(
        restored,
        root_before
            .split_once("\n#[cfg(test)]\n#[allow(\n")
            .unwrap()
            .0
    );
}
