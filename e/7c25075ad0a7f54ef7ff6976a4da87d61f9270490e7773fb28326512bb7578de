//! Whole immediate sorter and shipping caller controls; authored, not executed.
use super::*;

fn mode(old: bool) -> AdvanceEpoch {
    AdvanceEpoch(BEFORE_ADVANCE.with(|value| value.replace(old)))
}
fn observe(
    old: bool,
    paths: &[SkeletonPath],
    function: &ResolvedFunction,
    work: usize,
    bytes: usize,
    floor: usize,
    lent: bool,
) -> (Run, usize) {
    let _mode = mode(old);
    ADVANCE_PAIR_EVENTS.with(|events| events.set(0));
    let result = run(true, paths, function, work, bytes, floor, lent);
    (result, ADVANCE_PAIR_EVENTS.with(std::cell::Cell::get))
}
fn equation(old: &(Run, usize), new: &(Run, usize)) {
    same_work(&old.0, &new.0);
    assert_eq!(old.1, new.1);
    assert_eq!(
        old.0.heap, new.0.heap,
        "no heap credit for removed call ABI"
    );
    let removed = 2 * (ADVANCE_CALL - ADVANCE_BLOCK);
    assert_eq!(old.0.paid, new.0.paid + old.1 * removed);
}

#[test]
fn lexical_blocks_keep_every_original_structural_bank_and_complete_sort_invoice() {
    assert_eq!(
        before_advance::banks(),
        [
            PAYMENT,
            ADVANCE_CALL,
            FRAME_COMMON,
            ROOT_FRAME,
            SEQ_FRAME,
            NODE_FRAME,
            NODE_STEP,
            CAT_CHILD_STEP,
            COMPARE_CALL
        ]
    );
    assert_eq!(ADVANCE_BLOCK, PAYMENT);
    assert_eq!(
        ADVANCE_CALL - ADVANCE_BLOCK,
        4 * size_of::<&mut Cursor>()
            + 4 * size_of::<&Observations>()
            + 2 * size_of::<Result<(), ByteFailure>>()
    );
    let program = program();
    let function = &program.functions[0];
    for count in [0, 1, 2, 3, 12, 32] {
        for width in [0, 1, 17] {
            let paths = fixture(function, count, width);
            let old = observe(true, &paths, function, usize::MAX, usize::MAX, 0, false);
            let new = observe(false, &paths, function, usize::MAX, usize::MAX, 0, false);
            equation(&old, &new);
            if count < 2 {
                assert_eq!(new.1, 0);
            } else {
                assert!(new.1 > 0);
            }
            eprintln!("OPT948 count={count} width={width} pairs={} old_paid={} current_paid={} old_heap={:?} current_heap={:?}", new.1,old.0.paid,new.0.paid,old.0.heap,new.0.heap);
        }
    }
    for depth in [0, 1, 17] {
        for children in [0, 1, 33] {
            let mut prefix = dag(ObservationDagKind::Cat(
                (0..children)
                    .map(|index| {
                        dag(ObservationDagKind::Seq(
                            None,
                            (0..index % 3).map(|_| item(function, true)).collect(),
                        ))
                    })
                    .collect(),
            ));
            for _ in 0..depth {
                prefix = dag(ObservationDagKind::Seq(Some(prefix), Vec::new()));
            }
            let paths = [SkeletonTerminal::Success, SkeletonTerminal::Failure]
                .into_iter()
                .map(|terminal| SkeletonPath {
                    observations: Observations {
                        prefix: Some(Rc::clone(&prefix)),
                        items: Vec::new(),
                    },
                    terminal,
                })
                .collect::<Vec<_>>();
            let old = observe(true, &paths, function, usize::MAX, usize::MAX, 0, false);
            let new = observe(false, &paths, function, usize::MAX, usize::MAX, 0, false);
            equation(&old, &new);
            assert!(
                new.1 > 0,
                "equal shared identity never skips content traversal"
            );
        }
    }
}

#[test]
fn every_work_cut_empty_seq_cat_terminal_and_right_after_left_exhaustion_match() {
    let program = program();
    let function = &program.functions[0];
    for count in [0, 1, 2, 7] {
        let paths = fixture(function, count, 3);
        let old = observe(true, &paths, function, usize::MAX, usize::MAX, 0, false);
        for work in (0..=usize::MAX - old.0.remaining + 1).chain([usize::MAX]) {
            equation(
                &observe(true, &paths, function, work, usize::MAX, 0, false),
                &observe(false, &paths, function, work, usize::MAX, 0, false),
            );
        }
        equation(
            &observe(true, &paths, function, usize::MAX, usize::MAX, 0, true),
            &observe(false, &paths, function, usize::MAX, usize::MAX, 0, true),
        );
    }
    let mut paths = fixture(function, 2, 0);
    paths[0].observations = Observations {
        prefix: None,
        items: Vec::new(),
    };
    paths[1].observations = Observations {
        prefix: Some(dag(ObservationDagKind::Alt(Vec::new()))),
        items: Vec::new(),
    };
    for work in 0..24 {
        equation(
            &observe(true, &paths, function, work, usize::MAX, 0, false),
            &observe(false, &paths, function, work, usize::MAX, 0, false),
        );
    }
    let old = observe(true, &paths, function, usize::MAX, usize::MAX, 0, false);
    let new = observe(false, &paths, function, usize::MAX, usize::MAX, 0, false);
    equation(&old, &new);
    assert!(new
        .0
        .result
        .unwrap_err()
        .contains("cannot contain merged alternatives"));
}

#[test]
fn exact_short_zero_floor_sticky_and_foreign_boundaries_keep_original_status() {
    let program = program();
    let function = &program.functions[0];
    let paths = fixture(function, 7, 8);
    let healthy = observe(false, &paths, function, usize::MAX, usize::MAX, 0, false).0;
    for floor in [0, 1, 64] {
        let exact = observe(
            false,
            &paths,
            function,
            usize::MAX,
            healthy.paid + floor,
            floor,
            false,
        )
        .0;
        same_work(&healthy, &exact);
        assert_eq!(healthy.paid, exact.paid);
        let short = observe(
            false,
            &paths,
            function,
            usize::MAX,
            healthy.paid + floor - 1,
            floor,
            false,
        )
        .0;
        assert!(short.result.is_err() && (short.refused || short.refusal_spent));
        assert!(short.sticky.is_some());
    }
    for bytes in [
        0,
        1,
        ADVANCE_BLOCK - 1,
        ADVANCE_BLOCK,
        healthy.paid / 2,
        healthy.paid,
    ] {
        for work in [0, 1, 17, usize::MAX] {
            let a = observe(false, &paths, function, work, bytes, 0, false).0;
            let b = observe(false, &paths, function, work, bytes, 0, false).0;
            same_work(&a, &b);
            assert_eq!(a.paid, b.paid);
            assert!(!a.heap.invalid);
        }
    }
    let foreign = |old| {
        use crate::cleanup_plan::replay::allocation::oracle::OracleScope;
        let _mode = mode(old);
        let owner = OracleScope::open_with_work(usize::MAX, usize::MAX).unwrap();
        let mut budget = ReplayBudget::with_skeleton_limit_oracle(usize::MAX, owner.lease());
        let prior = owner.finish();
        assert!(prior.first_refusal.is_none());
        let foreign = OracleScope::open_with_work(usize::MAX, usize::MAX).unwrap();
        let before = foreign.receipt();
        let mut paths = paths.clone();
        let result = super::super::super::sort(&mut paths, function, &mut budget).unwrap_err();
        let sticky = budget
            .reserve_compact_bytes(function, 0, "foreign followup")
            .unwrap_err()
            .into_diagnostic();
        assert_eq!(
            foreign.receipt(),
            before,
            "an expired lease cannot spend the new account"
        );
        foreign.finish();
        (result, sticky)
    };
    let old = foreign(true);
    let new = foreign(false);
    assert_eq!(old, new);
}

#[test]
fn complete_cold_sort_drop_and_partial_refusal_have_identical_heap_ownership() {
    let program = program();
    let function = &program.functions[0];
    let cold = |old, bytes| {
        let _mode = mode(old);
        ADVANCE_PAIR_EVENTS.with(|events| events.set(0));
        let result = with_limit_usage_refusal(bytes, || {
            measure(|| {
                let mut paths = fixture(function, 12, 17);
                let mut budget = ReplayBudget::with_skeleton_limit(usize::MAX);
                let result = super::super::super::sort(&mut paths, function, &mut budget);
                drop(result);
                drop(paths);
                drop(budget);
            })
        });
        (result, ADVANCE_PAIR_EVENTS.with(std::cell::Cell::get))
    };
    let (old, op) = cold(true, usize::MAX);
    let (new, np) = cold(false, usize::MAX);
    assert_eq!(op, np);
    assert!(!old.1 && !new.1 && !old.0 .1.invalid && !new.0 .1.invalid);
    assert_eq!(old.0 .1, new.0 .1);
    assert_eq!(new.0 .1.live, 0);
    assert_eq!(new.0 .1.allocated, new.0 .1.freed);
    assert_eq!(old.2, new.2 + op * 2 * (ADVANCE_CALL - ADVANCE_BLOCK));
    for bytes in [0, 1, new.2 / 2, new.2.saturating_sub(1), new.2] {
        for old in [false, true] {
            let (result, _) = cold(old, bytes);
            assert!(!result.0 .1.invalid);
            assert_eq!(result.0 .1.live, 0);
            assert_eq!(result.0 .1.allocated, result.0 .1.freed);
        }
    }
}

#[test]
fn both_cursor_owners_clear_after_every_status_and_unwind_releases_all_lent_handles() {
    let program = program();
    let function = &program.functions[0];
    let paths = fixture(function, 2, 8);
    let counts: Vec<_> = paths
        .iter()
        .map(|path| Rc::strong_count(path.observations.prefix.as_ref().unwrap()))
        .collect();
    with_limit_usage_refusal(usize::MAX, || {
        let mut budget = ReplayBudget::with_skeleton_limit(usize::MAX);
        budget
            .reserve_compact_bytes(function, RECEIVERS, "current warm pair receiving bank")
            .unwrap();
        let mut pair = Pair::new();
        pair.order(&paths[0], &paths[1], function, &mut budget)
            .unwrap();
        let pointers = (pair.left.pending.as_ptr(), pair.right.pending.as_ptr());
        let capacities = (pair.left.pending.capacity(), pair.right.pending.capacity());
        for _ in 0..8 {
            pair.order(&paths[1], &paths[0], function, &mut budget)
                .unwrap();
            assert_eq!(
                (pair.left.pending.as_ptr(), pair.right.pending.as_ptr()),
                pointers
            );
            assert_eq!(
                (pair.left.pending.capacity(), pair.right.pending.capacity()),
                capacities
            );
            assert!(pair.left.pending.is_empty() && pair.right.pending.is_empty());
            assert!(pair.left.current.is_none() && pair.right.current.is_none());
            assert!(pair.left.yielded.is_none() && pair.right.yielded.is_none());
        }
    });
    for bytes in [0, 1, usize::MAX] {
        for work in [0, 1, 17, usize::MAX] {
            with_limit_usage_refusal(bytes, || {
                let mut budget = ReplayBudget::with_skeleton_limit(work);
                budget.remaining = work;
                if budget
                    .reserve_compact_bytes(function, RECEIVERS, "current pair receiving bank")
                    .is_err()
                {
                    return;
                }
                let mut pair = Pair::new();
                let result = pair.order(&paths[0], &paths[1], function, &mut budget);
                assert!(pair.left.pending.is_empty() && pair.right.pending.is_empty());
                assert!(pair.left.current.is_none() && pair.right.current.is_none());
                assert!(pair.left.yielded.is_none() && pair.right.yielded.is_none());
                drop(result);
                drop(pair);
            });
            for (path, count) in paths.iter().zip(&counts) {
                assert_eq!(
                    Rc::strong_count(path.observations.prefix.as_ref().unwrap()),
                    *count
                );
            }
        }
    }
    struct Reset;
    impl Drop for Reset {
        fn drop(&mut self) {
            FRAME_UNWIND.with(|cut| cut.set(None));
        }
    }
    let unwind = |old, cut| {
        let _mode = mode(old);
        let (((), physical), refused, paid, first) = with_limit_usage_refusal(usize::MAX, || {
            measure(|| {
                let mut paths = paths.clone();
                let mut budget = ReplayBudget::with_skeleton_limit(usize::MAX);
                let _reset = Reset;
                FRAME_UNWIND.with(|target| target.set(Some(cut)));
                let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    super::super::super::sort(&mut paths, function, &mut budget)
                }));
                assert!(result.is_err());
                drop(result);
                drop(paths);
                drop(budget);
            })
        });
        assert!(!refused && first.is_none());
        assert!(!physical.invalid && physical.live == 0);
        assert_eq!(physical.allocated, physical.freed);
        (physical, paid)
    };
    for cut in [0, 1, 3, 7] {
        let old = unwind(true, cut);
        let new = unwind(false, cut);
        assert_eq!(old.0, new.0);
        assert!(new.1 <= old.1);
        for (path, count) in paths.iter().zip(&counts) {
            assert_eq!(
                Rc::strong_count(path.observations.prefix.as_ref().unwrap()),
                *count
            );
        }
    }
}

fn shipping_input(program: &ResolvedProgram, function: &ResolvedFunction) -> Vec<SkeletonPath> {
    use crate::cleanup_plan::replay::{
        emission, hir_skeleton_paths, reserve_program_skeleton_work,
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
fn entire_shipping_compare_independent_plan_producer_and_all_late_hostiles_keep_their_invoices() {
    use crate::cleanup_plan::replay::ordinary_plan_history::completed_fork::shipping;
    let checked = crate::check("module shipping948; @id(\"f\") fn f(a:bool,b:bool)->i64 {let n=if a {1}else{2};if b {n+1}else{n+2}} @id(\"main\") fn main()->i64 {f(true,false)}", "shipping948.spx").unwrap();
    let program = crate::hir::resolve(&checked).unwrap();
    let function = &program.functions[0];
    let paths = shipping_input(&program, function);
    assert!(paths.len() > 1);
    let invoke = |old, fault, work| {
        let _mode = mode(old);
        let mut paths = paths.clone();
        if fault == 1 {
            paths.pop();
        }
        if fault == 2 {
            paths[0].terminal = SkeletonTerminal::LoopLatch;
        }
        if fault == 3 {
            paths[0].observations.items.push(item(function, false));
        }
        ADVANCE_PAIR_EVENTS.with(|events| events.set(0));
        let ((result, paths, remaining, spent, heap), refused, paid, first) =
            with_limit_usage_refusal(usize::MAX, || {
                let mut budget = ReplayBudget::with_skeleton_limit(work);
                budget.remaining = work;
                let (result, heap) =
                    measure(|| shipping::compare(function, &mut budget, &mut paths));
                (
                    result.map_err(|error| error.json()),
                    contents(&paths),
                    budget.remaining,
                    budget.skeleton_spent,
                    heap,
                )
            });
        (
            (result, paths, remaining, spent, heap, refused, first),
            paid,
            ADVANCE_PAIR_EVENTS.with(std::cell::Cell::get),
        )
    };
    for fault in 0..4 {
        let old = invoke(true, fault, usize::MAX);
        let new = invoke(false, fault, usize::MAX);
        assert_eq!(old.0, new.0);
        assert_eq!(old.2, new.2);
        assert!(
            new.2 > 0,
            "whole caller must actually reach the replaced advance sites"
        );
        assert_eq!(old.1, new.1 + new.2 * 2 * (ADVANCE_CALL - ADVANCE_BLOCK));
        assert_eq!(new.0 .0.is_ok(), fault == 0);
        for work in [0, 1, 3, 17, 64] {
            let old = invoke(true, fault, work);
            let new = invoke(false, fault, work);
            assert_eq!(old.0, new.0);
            assert_eq!(old.2, new.2);
            assert_eq!(old.1, new.1 + new.2 * 2 * (ADVANCE_CALL - ADVANCE_BLOCK));
        }
    }
    let cold = |old| {
        let _mode = mode(old);
        ADVANCE_PAIR_EVENTS.with(|events| events.set(0));
        let result = with_limit_usage_refusal(usize::MAX, || {
            measure(|| {
                let mut paths = shipping_input(&program, function);
                let mut budget = ReplayBudget::with_skeleton_limit(usize::MAX);
                shipping::compare(function, &mut budget, &mut paths).unwrap();
                drop(paths);
                drop(budget);
            })
        });
        (result, ADVANCE_PAIR_EVENTS.with(std::cell::Cell::get))
    };
    let (old, op) = cold(true);
    let (new, np) = cold(false);
    assert_eq!(op, np);
    assert!(!old.1 && !new.1 && !old.0 .1.invalid && !new.0 .1.invalid);
    assert_eq!(old.0 .1, new.0 .1);
    assert_eq!(new.0 .1.live, 0);
    assert_eq!(new.0 .1.allocated, new.0 .1.freed);
    assert_eq!(old.2, new.2 + op * 2 * (ADVANCE_CALL - ADVANCE_BLOCK));
}

#[test]
fn complete_immediate_sources_and_all_older_oracle_bodies_have_exact_two_way_inverses() {
    use sha2::{Digest, Sha256};
    let digest = |value: &str| {
        format!(
            "{:x}",
            crate::digest_hex::LowerHex(Sha256::digest(value.as_bytes()))
        )
    };
    {
        let current = include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/src/cleanup_plan/replay/skeleton_work/enumerated_prefix.rs"
        ));
        let before = include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/e/b77a27d55d5565de11015abbdb4d14437804674564dcd07e336c7e97b9d0080a"
        ));
        assert_eq!(
            digest(before),
            "b77a27d55d5565de11015abbdb4d14437804674564dcd07e336c7e97b9d0080a"
        );
        let edits = [(
            16461usize,
            16461usize,
            16461usize,
            16687usize,
            r#""#,
            r#"            .map_err(|failure| failure.into_diagnostic());
    }
    #[cfg(test)]
    if receiving_sort::BEFORE_ADVANCE.with(std::cell::Cell::get) {
        return receiving_sort::before_advance::sort(paths, function, budget)
"#,
        )];
        let mut inverse = current.to_owned();
        for &(_, _, start, end, before, after) in edits.iter().rev() {
            assert_eq!(&inverse[start..end], after);
            inverse.replace_range(start..end, before);
        }
        assert_eq!(inverse, before);
        let mut forward = before.to_owned();
        for &(start, end, _, _, before, after) in edits.iter().rev() {
            assert_eq!(&forward[start..end], before);
            forward.replace_range(start..end, after);
        }
        assert_eq!(forward, current);
    }
    {
        let current = include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/src/cleanup_plan/replay/skeleton_work/enumerated_prefix/receiving_sort.rs"
        ));
        let before = include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/e/e6ae7f63df349b2a4f8e260b8ec25312bf9362eed35882af10a3da3b2520c534"
        ));
        assert_eq!(
            digest(before),
            "e6ae7f63df349b2a4f8e260b8ec25312bf9362eed35882af10a3da3b2520c534"
        );
        let edits = [
            (
                1788usize,
                1788usize,
                1788usize,
                1801usize,
                r#""#,
                r#"#[cfg(test)]
"#,
            ),
            (
                4700usize,
                4700usize,
                4713usize,
                11275usize,
                r#""#,
                r#"// Direct place expressions introduce no cursor/observations alias, closure,
// returned Result or array. Every binding inside this block is the exact old
// frame/item/child binding and retains its original branch bank. A labelled
// unit block replaces only next's successful return; errors return compare's
// already-funded Result. PAYMENT still renews the same payment gateway.
const ADVANCE_BLOCK: usize = PAYMENT;
macro_rules! advance {
    ($pair:ident, $side:ident, $path:ident, $function:ident, $budget:ident) => {{
        'advance: {
            $pair.$side.yielded = None;
            if $budget.lent_skeleton {
                return Err($budget.compact_refusal(
                    $function,
                    "enumerated HIR cursor cannot spend a lent skeleton allowance",
                    "",
                ));
            }
            loop {
                $budget.reserve_compact_bytes(
                    $function,
                    FRAME_COMMON,
                    "enumerated HIR cursor common frame receiving renewal",
                )?;
                #[cfg(test)]
                frame_event(0);
                $pair.$side.current = $pair.$side.pending.pop();
                if $pair.$side.current.is_none() {
                    break;
                }
                $budget.charge_compact($function, 1, "enumerated HIR cursor traversal")?;
                match $pair
                    .$side
                    .current
                    .take()
                    .expect("the popped frame was present")
                {
                    Frame::RootItems { next } => {
                        #[cfg(test)]
                        frame_event(1);
                        $budget.reserve_compact_bytes(
                            $function,
                            ROOT_FRAME,
                            "enumerated HIR root-items frame receivers",
                        )?;
                        if let Some(item) = $path.observations.items.get(next) {
                            $pair.$side.yielded = Some(Rc::clone(item));
                            $pair
                                .$side
                                .pending
                                .push(Frame::RootItems { next: next + 1 });
                            break 'advance;
                        }
                    }
                    Frame::SeqItems { node, next } => {
                        #[cfg(test)]
                        frame_event(2);
                        $budget.reserve_compact_bytes(
                            $function,
                            SEQ_FRAME,
                            "enumerated HIR sequence-items frame receivers",
                        )?;
                        let ObservationDagKind::Seq(_, items) = &node.kind else {
                            unreachable!("sequence cursor frame retains its immutable node")
                        };
                        if let Some(item) = items.get(next) {
                            $pair.$side.yielded = Some(Rc::clone(item));
                            $pair.$side.pending.push(Frame::SeqItems {
                                node,
                                next: next + 1,
                            });
                            break 'advance;
                        }
                    }
                    Frame::Node(node) => {
                        #[cfg(test)]
                        frame_event(3);
                        $budget.reserve_compact_bytes(
                            $function,
                            NODE_FRAME,
                            "enumerated HIR node frame receivers",
                        )?;
                        $budget.reserve_compact_bytes(
                            $function,
                            NODE_STEP,
                            "enumerated HIR node growth receiving renewal",
                        )?;
                        match &node.kind {
                            ObservationDagKind::Seq(prefix, _) => {
                                $budget.reserve_compact_vec(
                                    $function,
                                    &mut $pair.$side.pending,
                                    2,
                                    "enumerated HIR sequence cursor backing",
                                )?;
                                let prefix = prefix.as_ref().map(Rc::clone);
                                // Move this handle into the resume frame; do not clone
                                // the Seq node merely to remember its items.
                                $pair.$side.pending.push(Frame::SeqItems { node, next: 0 });
                                if let Some(prefix) = prefix {
                                    $pair.$side.pending.push(Frame::Node(prefix));
                                }
                            }
                            ObservationDagKind::Cat(children) => {
                                $budget.reserve_compact_vec(
                                    $function,
                                    &mut $pair.$side.pending,
                                    children.len(),
                                    "enumerated HIR concatenation cursor backing",
                                )?;
                                let mut children = children.iter().rev();
                                loop {
                                    $budget.reserve_compact_bytes(
                                        $function,
                                        CAT_CHILD_STEP,
                                        "enumerated HIR concatenation child receiving renewal",
                                    )?;
                                    let Some(child) = children.next() else {
                                        break;
                                    };
                                    $pair.$side.pending.push(Frame::Node(Rc::clone(child)));
                                }
                            }
                            ObservationDagKind::Alt(_) => {
                                return Err($budget.compact_refusal(
                                    $function,
                                    "enumerated HIR history cannot contain merged alternatives",
                                    "",
                                ))
                            }
                        }
                    }
                }
            }
        }
    }};
}

"#,
            ),
            (
                5769usize,
                5769usize,
                12344usize,
                12361usize,
                r#""#,
                r#"    #[cfg(test)]
"#,
            ),
            (
                12790usize,
                12824usize,
                19382usize,
                19417usize,
                r#"                2 * ADVANCE_CALL,
"#,
                r#"                2 * ADVANCE_BLOCK,
"#,
            ),
            (
                12903usize,
                12967usize,
                19496usize,
                19610usize,
                r#"            self.left.next(&a.observations, function, budget)?;
"#,
                r#"            #[cfg(test)]
            advance_pair_event();
            advance!(self, left, a, function, budget);
"#,
            ),
            (
                13027usize,
                13092usize,
                19670usize,
                19726usize,
                r#"            self.right.next(&b.observations, function, budget)?;
"#,
                r#"            advance!(self, right, b, function, budget);
"#,
            ),
            (
                17351usize,
                17351usize,
                23985usize,
                24281usize,
                r#""#,
                r#"    FRAME_UNWIND.with(|cut| {
        if index == 0 {
            if let Some(remaining) = cut.get() {
                if remaining == 0 {
                    std::panic::resume_unwind(Box::new(()));
                }
                cut.set(Some(remaining - 1));
            }
        }
    });
"#,
            ),
            (
                17353usize,
                17353usize,
                24283usize,
                25200usize,
                r#""#,
                r#"
#[cfg(test)]
#[allow(dead_code, unused_imports, reason = "complete immediate post-926 sorter with independent typed tariffs")]
#[rustfmt::skip]
pub(super) mod before_advance;
#[cfg(test)]
std::thread_local! {
    pub(super) static BEFORE_ADVANCE: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
    static ADVANCE_PAIR_EVENTS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
    static FRAME_UNWIND: std::cell::Cell<Option<usize>> = const { std::cell::Cell::new(None) };
}
#[cfg(test)]
fn advance_pair_event() {
    ADVANCE_PAIR_EVENTS.with(|events| events.set(events.get().checked_add(1).unwrap()));
}
#[cfg(test)]
struct AdvanceEpoch(bool);
#[cfg(test)]
impl Drop for AdvanceEpoch {
    fn drop(&mut self) {
        BEFORE_ADVANCE.with(|old| old.set(self.0));
    }
}
#[cfg(test)]
fn old_advance_epoch() -> AdvanceEpoch {
    AdvanceEpoch(BEFORE_ADVANCE.with(|old| old.replace(true)))
}
"#,
            ),
        ];
        let mut inverse = current.to_owned();
        for &(_, _, start, end, before, after) in edits.iter().rev() {
            assert_eq!(&inverse[start..end], after);
            inverse.replace_range(start..end, before);
        }
        assert_eq!(inverse, before);
        let mut forward = before.to_owned();
        for &(start, end, _, _, before, after) in edits.iter().rev() {
            assert_eq!(&forward[start..end], before);
            forward.replace_range(start..end, after);
        }
        assert_eq!(forward, current);
    }
    {
        let current = include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/src/cleanup_plan/replay/skeleton_work/enumerated_prefix/receiving_sort/tests.rs"
        ));
        let before = include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/e/d6ce5ade786b2027c78ce173f548a0d9dacb09faba44e11435e55d467a2632f0"
        ));
        assert_eq!(
            digest(before),
            "d6ce5ade786b2027c78ce173f548a0d9dacb09faba44e11435e55d467a2632f0"
        );
        let edits = [
            (
                6161usize,
                6161usize,
                6161usize,
                6208usize,
                r#""#,
                r#"    let _before_advance = old_advance_epoch();
"#,
            ),
            (
                6970usize,
                6970usize,
                7017usize,
                7064usize,
                r#""#,
                r#"    let _before_advance = old_advance_epoch();
"#,
            ),
            (
                8421usize,
                8421usize,
                8515usize,
                8562usize,
                r#""#,
                r#"    let _before_advance = old_advance_epoch();
"#,
            ),
            (
                9937usize,
                9937usize,
                10078usize,
                10125usize,
                r#""#,
                r#"    let _before_advance = old_advance_epoch();
"#,
            ),
            (
                10912usize,
                10912usize,
                11100usize,
                11147usize,
                r#""#,
                r#"    let _before_advance = old_advance_epoch();
"#,
            ),
            (
                12282usize,
                12282usize,
                12517usize,
                12601usize,
                r#""#,
                r#"    let _before_advance = old_advance_epoch();
    use super::before_advance::Pair;
"#,
            ),
            (
                14684usize,
                14684usize,
                15003usize,
                15050usize,
                r#""#,
                r#"    let _before_advance = old_advance_epoch();
"#,
            ),
            (
                15931usize,
                15931usize,
                16297usize,
                16344usize,
                r#""#,
                r#"    let _before_advance = old_advance_epoch();
"#,
            ),
            (
                17102usize,
                17102usize,
                17515usize,
                17562usize,
                r#""#,
                r#"    let _before_advance = old_advance_epoch();
"#,
            ),
            (
                18002usize,
                18002usize,
                18462usize,
                18509usize,
                r#""#,
                r#"    let _before_advance = old_advance_epoch();
"#,
            ),
            (
                23101usize,
                23101usize,
                23608usize,
                23655usize,
                r#""#,
                r#"    let _before_advance = old_advance_epoch();
"#,
            ),
            (
                24353usize,
                24353usize,
                24907usize,
                24960usize,
                r#""#,
                r#"
#[path = "lexical_advance.rs"]
mod lexical_advance;
"#,
            ),
        ];
        let mut inverse = current.to_owned();
        for &(_, _, start, end, before, after) in edits.iter().rev() {
            assert_eq!(&inverse[start..end], after);
            inverse.replace_range(start..end, before);
        }
        assert_eq!(inverse, before);
        let mut forward = before.to_owned();
        for &(start, end, _, _, before, after) in edits.iter().rev() {
            assert_eq!(&forward[start..end], before);
            forward.replace_range(start..end, after);
        }
        assert_eq!(forward, current);
    }
    {
        let current = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/cleanup_plan/replay/skeleton_work/enumerated_prefix/receiving_sort/branch_frames.rs"));
        let before = include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/e/9da3102be302545d598dab9a301c426a8c75a2288cc477291d18cc6f068ba26f"
        ));
        assert_eq!(
            digest(before),
            "9da3102be302545d598dab9a301c426a8c75a2288cc477291d18cc6f068ba26f"
        );
        let edits = [
            (
                1907usize,
                1907usize,
                1907usize,
                1954usize,
                r#""#,
                r#"    let _before_advance = old_advance_epoch();
"#,
            ),
            (
                3115usize,
                3115usize,
                3162usize,
                3209usize,
                r#""#,
                r#"    let _before_advance = old_advance_epoch();
"#,
            ),
            (
                4864usize,
                4864usize,
                4958usize,
                5005usize,
                r#""#,
                r#"    let _before_advance = old_advance_epoch();
"#,
            ),
            (
                5648usize,
                5648usize,
                5789usize,
                5836usize,
                r#""#,
                r#"    let _before_advance = old_advance_epoch();
"#,
            ),
            (
                8003usize,
                8003usize,
                8191usize,
                8238usize,
                r#""#,
                r#"    let _before_advance = old_advance_epoch();
"#,
            ),
            (
                10993usize,
                10993usize,
                11228usize,
                11275usize,
                r#""#,
                r#"    let _before_advance = old_advance_epoch();
"#,
            ),
        ];
        let mut inverse = current.to_owned();
        for &(_, _, start, end, before, after) in edits.iter().rev() {
            assert_eq!(&inverse[start..end], after);
            inverse.replace_range(start..end, before);
        }
        assert_eq!(inverse, before);
        let mut forward = before.to_owned();
        for &(start, end, _, _, before, after) in edits.iter().rev() {
            assert_eq!(&forward[start..end], before);
            forward.replace_range(start..end, after);
        }
        assert_eq!(forward, current);
    }
    let captured = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/e/e6ae7f63df349b2a4f8e260b8ec25312bf9362eed35882af10a3da3b2520c534"
    ));
    let executable = include_str!("before_advance.rs")
        .split_once("\n// TEST_ONLY_948_EXACT_BANK_ACCESS\n")
        .unwrap()
        .0;
    let restored = executable
        .replace(
            "use super::super::*;\nuse super::{before_branch_frames, frame_event};",
            "use super::*;",
        )
        .replace(
            "pub(in crate::cleanup_plan::replay::skeleton_work::enumerated_prefix) fn sort(",
            "pub(super) fn sort(",
        )
        .replace(
            "            #[cfg(test)]\n            super::advance_pair_event();\n",
            "",
        )
        .replace("pub(super) enum Frame {", "enum Frame {")
        .replace("pub(super) struct Cursor {", "struct Cursor {")
        .replace("pub(super) struct Pair {", "struct Pair {")
        .replace("    pub(super) ", "    ");
    assert_eq!(
        restored,
        captured.split_once("\n#[cfg(test)]\n#[allow(\n").unwrap().0
    );
}
