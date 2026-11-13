use super::*;
use crate::test_allocation_observer::measure;

#[test]
fn ordinary_numeric_memo_keeps_exact_pointer_continuation_and_opposite_rank_storage() {
    let source = crate::check(
        "module memo.rank; @id(\"main\") fn main()->i64 {0}",
        "memo-rank.spx",
    )
    .unwrap();
    let program = crate::hir::resolve(&source).unwrap();
    let function = &program.functions[0];
    // Real simultaneous Rc addresses, then exact opposite allocator ranks.
    // These live outside the narrow memo-storage observer.
    let nodes = (0..128)
        .map(|_| {
            Rc::new(ObservationDag::legacy(ObservationDagKind::Seq(
                None,
                Vec::new(),
            )))
        })
        .collect::<Vec<_>>();
    let mut pointers = nodes
        .iter()
        .map(|node| Rc::as_ptr(node) as usize)
        .collect::<Vec<_>>();
    pointers.sort_unstable();
    let run = |reverse: bool| {
        let mut budget = ReplayBudget::with_skeleton_limit(MAX_REPLAY_WORK_UNITS);
        measure(|| {
            let mut memo = Memo::Ordinary(BTreeMap::new());
            let mut old = BTreeMap::new();
            // Saved old key/value meaning, built outside no claimed paid boundary.
            for ordinal in 0..128 {
                let pointer = pointers[if reverse { 127 - ordinal } else { ordinal }];
                old.insert((pointer, 3usize), ordinal + 7);
                insert(
                    function,
                    &mut memo,
                    test_key(pointer, ordinal, 3),
                    ordinal + 7,
                    &mut budget,
                )
                .unwrap();
            }
            for ordinal in 0..128 {
                let pointer = pointers[if reverse { 127 - ordinal } else { ordinal }];
                assert_eq!(
                    get(function, &memo, &test_key(pointer, ordinal, 3), &mut budget).unwrap(),
                    old.get(&(pointer, 3)).copied()
                );
                assert_eq!(
                    get(function, &memo, &test_key(pointer, ordinal, 4), &mut budget).unwrap(),
                    None
                );
            }
            // The independent saved map has rank-dependent storage; exclude it
            // from the allocation comparison by comparing new map separately.
            drop(old);
            drop(memo);
        })
    };
    let (_, left) = run(false);
    let (_, right) = run(true);
    assert!(!left.invalid && !right.invalid);
    assert_eq!(left.live, 0);
    assert_eq!(right.live, 0);
    // Full result parity above includes the saved map. Observe only memo below.
    let storage = |reverse: bool| {
        let mut budget = ReplayBudget::with_skeleton_limit(MAX_REPLAY_WORK_UNITS);
        measure(|| {
            let mut memo = Memo::Ordinary(BTreeMap::new());
            for ordinal in 0..128 {
                let pointer = pointers[if reverse { 127 - ordinal } else { ordinal }];
                insert(
                    function,
                    &mut memo,
                    test_key(pointer, ordinal, 3),
                    ordinal + 7,
                    &mut budget,
                )
                .unwrap();
            }
            drop(memo);
        })
        .1
    };
    let left = storage(false);
    let right = storage(true);
    assert_eq!(left, right);
    assert!(!left.invalid);
    assert_eq!(left.live, 0);
    assert_eq!(left.allocated, left.freed);
    let mut budget = ReplayBudget::with_skeleton_limit(MAX_REPLAY_WORK_UNITS);
    let mut memo = Memo::Ordinary(BTreeMap::new());
    insert(
        function,
        &mut memo,
        test_key(pointers[0], 0, 3),
        7,
        &mut budget,
    )
    .unwrap();
    assert!(get(function, &memo, &test_key(pointers[1], 0, 3), &mut budget).is_err());
    assert!(insert(
        function,
        &mut memo,
        test_key(pointers[1], 0, 3),
        8,
        &mut budget
    )
    .is_err());
    assert_eq!(
        get(function, &memo, &test_key(pointers[0], 0, 3), &mut budget).unwrap(),
        Some(7)
    );
    let detached = Key {
        pointer: pointers[0],
        continuation: 3,
        ordinal: None,
    };
    assert!(get(function, &memo, &detached, &mut budget).is_err());
}

#[test]
fn one_function_work_dominates_helper_arguments_guards_and_both_contract_phases() {
    let source = crate::check(
        r#"module memo.helpers;
@id("accepts") fn accepts(value:i64)->bool {value>=0}
@id("first") fn first(value:i64)->i64
    requires accepts(value)
    ensures accepts(result)
{
    let a=if value==0 {1}else{2};
    let b=if value==1 {3}else{4};
    a+b
}
@id("second") fn second(value:i64)->i64
    requires accepts(value)
    ensures accepts(result)
{
    let a=if first(value)>0 {5}else{6};
    let b=if accepts(value) {7}else{8};
    a+b
}
@id("main") fn main()->i64
    requires accepts(0)
    ensures accepts(result)
{
    let a=if first(0)>0 {1}else{2};
    let b=if second(a)>0 {3}else{4};
    let c=if accepts(a+b) {5}else{6};
    a+b+c
}
"#,
        "memo-helpers.spx",
    )
    .unwrap();
    let program = crate::hir::resolve(&source).unwrap();
    crate::hir::validate(&program).unwrap();
    let mut functions_with_prefix = 0;
    for function in &program.functions {
        // Independent saved enumerating route: its observations stay flat and
        // are compared to the independently walked current cleanup CFG.
        let mut old_budget = ReplayBudget::with_skeleton_limit(MAX_REPLAY_WORK_UNITS);
        let mut expected = hir_skeleton_paths(&program, function, &mut old_budget).unwrap();
        let mut actual = plan_skeleton_paths(function, &mut old_budget).unwrap();
        expected.sort();
        actual.sort();
        assert_eq!(expected, actual);
        validate_structure(&program, function).unwrap();
        let mut receiving = ReplayBudget::with_skeleton_limit(MAX_REPLAY_WORK_UNITS);
        assert_eq!(
            compare(&program, function, &mut receiving).unwrap(),
            Some(true)
        );

        let mut budget = ReplayBudget::with_skeleton_limit(MAX_REPLAY_WORK_UNITS);
        budget.merge_paths = true;
        let paths = hir_skeleton_paths(&program, function, &mut budget).unwrap();
        let mut pending = paths
            .iter()
            .filter_map(|path| path.observations.prefix.as_deref())
            .collect::<Vec<_>>();
        let mut pointers = BTreeSet::new();
        let mut ordinals = BTreeMap::new();
        while let Some(node) = pending.pop() {
            let pointer = node as *const ObservationDag as usize;
            if !pointers.insert(pointer) {
                continue;
            }
            let ordinal = node
                .ordinal
                .expect("ordinary constructor supplies storage metadata");
            assert!(
                ordinals.insert(ordinal, pointer).is_none(),
                "different nodes in one returned family share an ordinal"
            );
            match &node.kind {
                ObservationDagKind::Seq(prefix, _) => {
                    pending.extend(prefix.iter().map(|node| &**node))
                }
                ObservationDagKind::Cat(parts) | ObservationDagKind::Alt(parts) => {
                    pending.extend(parts.iter().map(|node| &**node))
                }
            }
        }
        if !ordinals.is_empty() {
            functions_with_prefix += 1;
        }
        // Each compare above built its own work and memo; only this exact
        // function's returned DAG family reaches that memo. Helper bodies are
        // validated separately, never imported into a caller's skeleton.
    }
    assert_eq!(functions_with_prefix, 3);
}
