//! Consuming tails retain exact shapes and allocations through their receivers.
use super::*;
mod old;

#[derive(Debug, PartialEq, Eq)]
enum Shape {
    Seq(Option<usize>, Option<Box<Shape>>, Vec<Observation>),
    Cat(Option<usize>, Vec<Shape>),
    Alt(Option<usize>, Vec<Shape>),
}
fn shape(node: &ObservationDag) -> Shape {
    match &node.kind {
        ObservationDagKind::Seq(prefix, items) => Shape::Seq(
            node.ordinal,
            prefix.as_ref().map(|p| Box::new(shape(p))),
            items.clone(),
        ),
        ObservationDagKind::Cat(parts) => {
            Shape::Cat(node.ordinal, parts.iter().map(|p| shape(p)).collect())
        }
        ObservationDagKind::Alt(parts) => {
            Shape::Alt(node.ordinal, parts.iter().map(|p| shape(p)).collect())
        }
    }
}
fn observation(function: &ResolvedFunction) -> Observation {
    Rc::new(SkeletonObservation::Boolean {
        expression: function.body.id.clone(),
        value: true,
    })
}
fn path(item: &Observation, len: usize) -> ExprSkeletonPath {
    let mut path = empty_expr_path();
    path.observations.items = vec![item.clone(); len];
    path
}
fn assert_paths(old: &[ExprSkeletonPath], new: &[ExprSkeletonPath]) {
    assert_eq!(old.len(), new.len());
    for (old, new) in old.iter().zip(new) {
        assert_eq!(old.owned_source, new.owned_source);
        assert_eq!(old.failed, new.failed);
        assert_eq!(old.residual, new.residual);
        assert_eq!(old.continuation, new.continuation);
        assert_eq!(old.observations.items, new.observations.items);
        assert_eq!(
            old.observations.prefix.as_ref().map(|p| shape(p)),
            new.observations.prefix.as_ref().map(|p| shape(p))
        );
    }
}

fn complete_saved_merges(selected: bool) {
    let program = fixture();
    let function = &program.functions[0];
    let item = observation(function);
    let mut old_budget = ReplayBudget::with_physical(function, 1 << 22).unwrap();
    old_budget.skeleton_remaining = MAX_REPLAY_WORK_UNITS;
    let mut new_budget = ReplayBudget::with_physical(function, 1 << 22).unwrap();
    new_budget.skeleton_remaining = MAX_REPLAY_WORK_UNITS;
    if !selected {
        old_budget.admission = allocation::AdmissionMode::Ordinary;
        new_budget.admission = allocation::AdmissionMode::Ordinary;
    }
    old_budget.merge_paths = true;
    new_budget.merge_paths = true;
    let mut old_work = SkeletonWork::new(&program, function, &mut old_budget);
    let mut new_work = SkeletonWork::new(&program, function, &mut new_budget);
    let mut singleton = path(&item, 3);
    singleton.residual = true;
    let singleton_pointer = singleton.observations.items.as_ptr();
    let mut failed = path(&item, 2);
    failed.failed = true;
    let mut abrupt = path(&item, 1);
    abrupt.continuation = lexical_skeleton::Continuation::Exit(item.clone());
    let inputs = vec![
        path(&item, 2),
        singleton,
        failed.clone(),
        path(&item, 0),
        failed,
        abrupt,
    ];
    let old = old_work.tail_merge_control(inputs.clone()).unwrap();
    let new = new_work.merged_paths(inputs).unwrap();
    assert_paths(&old, &new);
    assert_eq!(new[1].observations.items.as_ptr(), singleton_pointer);
    assert_eq!(
        old_work.budget.skeleton_spent,
        new_work.budget.skeleton_spent
    );
    let commits = vec![(0, CleanupPlace::whole(StorageId::ProvisionalResult))];
    let mut single_call = path(&item, 4);
    single_call.residual = true;
    let call_pointer = single_call.observations.items.as_ptr();
    let states = vec![
        (path(&item, 2), commits.clone()),
        (single_call, vec![]),
        (path(&item, 0), commits),
    ];
    let old = old_work.saved_merged_call_states(states.clone()).unwrap();
    let new = new_work.merged_call_states(states).unwrap();
    assert_eq!(old.len(), new.len());
    for ((a, ac), (b, bc)) in old.iter().zip(&new) {
        assert_paths(std::slice::from_ref(a), std::slice::from_ref(b));
        assert_eq!(ac, bc);
    }
    assert_eq!(new[1].0.observations.items.as_ptr(), call_pointer);
    assert_eq!(
        old_work.budget.skeleton_spent,
        new_work.budget.skeleton_spent
    );
}

#[test]
fn saved_unions_preserve_shared_prefixes_ordinals_and_move_nonempty_backing() {
    let program = fixture();
    let function = &program.functions[0];
    let item = observation(function);
    for selected in [false, true] {
        let mut a = ReplayBudget::with_physical(function, 1 << 22).unwrap();
        a.skeleton_remaining = MAX_REPLAY_WORK_UNITS;
        let mut b = ReplayBudget::with_physical(function, 1 << 22).unwrap();
        b.skeleton_remaining = MAX_REPLAY_WORK_UNITS;
        let mut old_work = SkeletonWork::new(&program, function, &mut a);
        let mut new_work = SkeletonWork::new(&program, function, &mut b);
        let old_prefix = old_work
            .charged_dag(ObservationDagKind::Seq(None, vec![item.clone()]))
            .unwrap();
        let new_prefix = new_work
            .charged_dag(ObservationDagKind::Seq(None, vec![item.clone()]))
            .unwrap();
        let members = |prefix: &Rc<ObservationDag>| {
            vec![
                Observations {
                    prefix: Some(prefix.clone()),
                    items: vec![item.clone(); 3],
                },
                Observations {
                    prefix: Some(prefix.clone()),
                    items: vec![],
                },
                Observations::default(),
                Observations {
                    prefix: Some(prefix.clone()),
                    items: vec![item.clone(); 2],
                },
            ]
        };
        let inputs = members(&new_prefix);
        let pointers = [inputs[0].items.as_ptr(), inputs[3].items.as_ptr()];
        let old = if selected {
            old_work.saved_union_observations(members(&old_prefix))
        } else {
            old_work.saved_ordinary_union(members(&old_prefix))
        }
        .unwrap();
        let new = if selected {
            new_work.union_observations(inputs)
        } else {
            new_work.ordinary_union(inputs)
        }
        .unwrap();
        assert_eq!(
            shape(old.prefix.as_ref().unwrap()),
            shape(new.prefix.as_ref().unwrap())
        );
        assert_eq!(
            old_work.budget.skeleton_spent,
            new_work.budget.skeleton_spent
        );
        let ObservationDagKind::Alt(parts) = &new.prefix.as_ref().unwrap().kind else {
            panic!("union")
        };
        for (index, pointer) in [(0, pointers[0]), (3, pointers[1])] {
            let ObservationDagKind::Seq(Some(prefix), items) = &parts[index].kind else {
                panic!("tail")
            };
            assert_eq!(items.as_ptr(), pointer);
            assert!(Rc::ptr_eq(prefix, &new_prefix));
        }
        assert!(Rc::ptr_eq(&parts[1], &new_prefix));
        drop(new);
        drop(old);
    }
}

fn union_probe(
    program: &ResolvedProgram,
    selected: bool,
    saved: bool,
    limit: usize,
) -> (Result<usize, ()>, crate::test_allocation_observer::Metrics) {
    let function = &program.functions[0];
    let item = observation(function);
    let mut budget = ReplayBudget::with_physical(function, limit).unwrap();
    budget.skeleton_remaining = MAX_REPLAY_WORK_UNITS;
    let mut work = SkeletonWork::new(program, function, &mut budget);
    let before = work.budget.physical.as_ref().unwrap().charged();
    crate::test_allocation_observer::measure(|| {
        let result = (|| {
            // Input carrier/tails are real allocations paid before construction.
            work.budget
                .reserve_array::<Observations>(function, 3, "test union input")?;
            let mut inputs = Vec::with_capacity(3);
            let mut pointers = [std::ptr::null(); 3];
            for (index, len) in [0, 2, 5].into_iter().enumerate() {
                work.budget
                    .reserve_array::<Observation>(function, len, "test input tail")?;
                let items = vec![item.clone(); len];
                pointers[index] = items.as_ptr();
                inputs.push(Observations::from(items));
            }
            let value = match (selected, saved) {
                (true, true) => work.saved_union_observations(inputs),
                (true, false) => work.union_observations(inputs),
                (false, true) => work.saved_ordinary_union(inputs),
                (false, false) => work.ordinary_union(inputs),
            }?;
            if !saved {
                let ObservationDagKind::Alt(parts) = &value.prefix.as_ref().unwrap().kind else {
                    panic!("union")
                };
                for index in [1, 2] {
                    let ObservationDagKind::Seq(_, items) = &parts[index].kind else {
                        panic!("tail")
                    };
                    assert_eq!(items.as_ptr(), pointers[index]);
                }
            }
            drop(value);
            Ok::<(), Diagnostic>(())
        })();
        drop(work.dag_scratch.take());
        drop(work.ordinary_scratch.take());
        match result {
            Ok(()) => Ok(work.budget.physical.as_ref().unwrap().charged() - before),
            Err(error) => {
                assert_eq!(error.code, "SPX-H006");
                drop(error);
                let charged = work.budget.physical.as_ref().unwrap().charged();
                let error = work
                    .budget
                    .reserve_bytes(function, 0, "sticky consumed union")
                    .unwrap_err();
                assert_eq!(error.code, "SPX-H006");
                drop(error);
                assert_eq!(work.budget.physical.as_ref().unwrap().charged(), charged);
                Err(())
            }
        }
    })
}

#[test]
fn owned_union_literal_clone_request_delta_and_exact_one_short_drop_receivers() {
    let program = fixture();
    let function = &program.functions[0];
    let admission = ReplayBudget::with_physical(function, 1 << 22)
        .unwrap()
        .physical
        .as_ref()
        .unwrap()
        .charged();
    let removed = 7 * std::mem::size_of::<Observation>();
    for selected in [false, true] {
        let (old, old_metrics) = union_probe(&program, selected, true, 1 << 22);
        let (new, new_metrics) = union_probe(&program, selected, false, 1 << 22);
        let charged = new.unwrap();
        assert_eq!(old.unwrap(), charged + removed);
        assert_eq!(old_metrics.allocated, new_metrics.allocated + removed);
        assert_eq!(old_metrics.allocations, new_metrics.allocations + 2);
        for metrics in [old_metrics, new_metrics] {
            assert!(!metrics.invalid && metrics.live == 0);
            assert_eq!(metrics.allocated, metrics.freed);
        }
        let (exact, observed) = union_probe(&program, selected, false, admission + charged);
        assert_eq!(exact.unwrap(), charged);
        assert!(!observed.invalid && observed.live == 0 && observed.allocated <= charged);
        let (short, observed) = union_probe(&program, selected, false, admission + charged - 1);
        assert!(short.is_err());
        assert!(!observed.invalid && observed.live == 0);
    }
}

#[test]
fn saved_lexical_merges_keep_failure_residual_return_and_conditional_isolation() {
    let source = "module tail.lexical; @id(\"probe\") fn probe(n:i64)->i64 {let owner=\"prefix\"; let mut i=0; while i<1 {i=i+1; if n==0 {return 1;}; 0} return 0;} @id(\"main\") fn main()->i64 {0}";
    let parsed = crate::check(source, "tail-lexical.spx").unwrap();
    let program = crate::hir::resolve(&parsed).unwrap();
    let function = program
        .functions
        .iter()
        .find(|f| f.id.as_str() == "probe")
        .unwrap();
    assert!(function.cleanup_plan.lexical_control.is_some());
    let item = observation(function);
    let mut failed = path(&item, 2);
    failed.failed = true;
    let mut residual = path(&item, 3);
    residual.residual = true;
    let mut returned = path(&item, 4);
    returned.continuation = lexical_skeleton::Continuation::Exit(item.clone());
    let mut conditional = path(&item, 1);
    conditional
        .observations
        .items
        .push(Rc::new(SkeletonObservation::Boolean {
            expression: function.body.id.clone(),
            value: false,
        }));
    for active_region in [false, true] {
        let mut a = ReplayBudget::with_physical(function, 1 << 22).unwrap();
        a.skeleton_remaining = MAX_REPLAY_WORK_UNITS;
        let mut b = ReplayBudget::with_physical(function, 1 << 22).unwrap();
        b.skeleton_remaining = MAX_REPLAY_WORK_UNITS;
        a.merge_paths = true;
        b.merge_paths = true;
        let mut old_work = SkeletonWork::new(&program, function, &mut a);
        let mut new_work = SkeletonWork::new(&program, function, &mut b);
        if active_region {
            old_work.active_copy_region = Some(function.body.id.clone());
            new_work.active_copy_region = Some(function.body.id.clone());
        }
        let inputs = vec![
            failed.clone(),
            failed.clone(),
            residual.clone(),
            returned.clone(),
            returned.clone(),
            conditional.clone(),
            path(&item, 0),
        ];
        let pointers: Vec<_> = inputs
            .iter()
            .map(|p| p.observations.items.as_ptr())
            .collect();
        let old = old_work.tail_merge_control(inputs.clone()).unwrap();
        let new = new_work.merged_paths(inputs).unwrap();
        assert_paths(&old, &new);
        assert_eq!(new.len(), if active_region { 6 } else { 7 });
        for index in 0..5 {
            assert_eq!(new[index].observations.items.as_ptr(), pointers[index]);
        }
        let commits = vec![(0, CleanupPlace::whole(StorageId::ProvisionalResult))];
        let states = vec![
            (conditional.clone(), commits.clone()),
            (conditional.clone(), commits),
        ];
        let old = old_work.saved_merged_call_states(states.clone()).unwrap();
        let new = new_work.merged_call_states(states).unwrap();
        assert_eq!(new.len(), 2);
        for ((a, ac), (b, bc)) in old.iter().zip(&new) {
            assert_paths(std::slice::from_ref(a), std::slice::from_ref(b));
            assert_eq!(ac, bc);
        }
    }
}

fn merge_probe(
    program: &ResolvedProgram,
    calls: bool,
    saved: bool,
    limit: usize,
) -> (Result<usize, ()>, crate::test_allocation_observer::Metrics) {
    let function = &program.functions[0];
    let item = observation(function);
    let mut budget = ReplayBudget::with_physical(function, limit).unwrap();
    budget.skeleton_remaining = MAX_REPLAY_WORK_UNITS;
    budget.merge_paths = true;
    let mut work = SkeletonWork::new(program, function, &mut budget);
    let before = work.budget.physical.as_ref().unwrap().charged();
    crate::test_allocation_observer::measure(|| {
        let result = (|| {
            work.budget
                .reserve_array::<ExprSkeletonPath>(function, 3, "test merge input")?;
            let mut inputs = Vec::with_capacity(3);
            for len in [2, 3, 5] {
                work.budget
                    .reserve_array::<Observation>(function, len, "test merge tail")?;
                inputs.push(path(&item, len));
            }
            inputs[1].residual = true;
            let first = inputs[0].observations.items.as_ptr();
            let singleton = inputs[1].observations.items.as_ptr();
            let last = inputs[2].observations.items.as_ptr();
            let result: Result<(), Diagnostic> = if calls {
                work.budget
                    .reserve_array::<CallSkeletonState>(function, 3, "test call input")?;
                let states = inputs.into_iter().map(|p| (p, vec![])).collect();
                let merged = if saved {
                    work.saved_merged_call_states(states)
                } else {
                    work.merged_call_states(states)
                }?;
                // This receiver owns all state carriers until explicitly dropped.
                if !saved {
                    assert_eq!(merged[1].0.observations.items.as_ptr(), singleton);
                    let ObservationDagKind::Alt(parts) =
                        &merged[0].0.observations.prefix.as_ref().unwrap().kind
                    else {
                        panic!("merge")
                    };
                    for (part, pointer) in parts.iter().zip([first, last]) {
                        let ObservationDagKind::Seq(_, items) = &part.kind else {
                            panic!("tail")
                        };
                        assert_eq!(items.as_ptr(), pointer);
                    }
                }
                drop(merged);
                Ok(())
            } else {
                let merged = if saved {
                    work.tail_merge_control(inputs)
                } else {
                    work.merged_paths(inputs)
                }?;
                if !saved {
                    assert_eq!(merged[1].observations.items.as_ptr(), singleton);
                    let ObservationDagKind::Alt(parts) =
                        &merged[0].observations.prefix.as_ref().unwrap().kind
                    else {
                        panic!("merge")
                    };
                    for (part, pointer) in parts.iter().zip([first, last]) {
                        let ObservationDagKind::Seq(_, items) = &part.kind else {
                            panic!("tail")
                        };
                        assert_eq!(items.as_ptr(), pointer);
                    }
                }
                drop(merged);
                Ok(())
            };
            result
        })();
        drop(work.dag_scratch.take());
        drop(work.ordinary_scratch.take());
        match result {
            Ok(()) => Ok(work.budget.physical.as_ref().unwrap().charged() - before),
            Err(error) => {
                assert_eq!(error.code, "SPX-H006");
                drop(error);
                let used = work.budget.physical.as_ref().unwrap().charged();
                drop(
                    work.budget
                        .reserve_bytes(function, 0, "sticky merged tail")
                        .unwrap_err(),
                );
                assert_eq!(work.budget.physical.as_ref().unwrap().charged(), used);
                Err(())
            }
        }
    })
}

#[test]
fn saved_merge_literal_clone_delta_and_exact_one_short_receivers() {
    let program = fixture();
    let function = &program.functions[0];
    let admission = ReplayBudget::with_physical(function, 1 << 22)
        .unwrap()
        .physical
        .as_ref()
        .unwrap()
        .charged();
    // First group tails 2+3; union tails 2+5. These are four real Vec requests.
    let removed = (2 + 3 + 2 + 5) * std::mem::size_of::<Observation>();
    for calls in [false, true] {
        let (old, old_metrics) = merge_probe(&program, calls, true, 1 << 22);
        let (new, new_metrics) = merge_probe(&program, calls, false, 1 << 22);
        let charged = new.unwrap();
        assert_eq!(old.unwrap(), charged + removed);
        assert_eq!(old_metrics.allocated, new_metrics.allocated + removed);
        assert_eq!(old_metrics.allocations, new_metrics.allocations + 4);
        for metrics in [old_metrics, new_metrics] {
            assert!(!metrics.invalid && metrics.live == 0);
            assert_eq!(metrics.allocated, metrics.freed);
        }
        let (exact, observed) = merge_probe(&program, calls, false, admission + charged);
        assert_eq!(exact.unwrap(), charged);
        assert!(!observed.invalid && observed.live == 0 && observed.allocated <= charged);
        let (short, observed) = merge_probe(&program, calls, false, admission + charged - 1);
        assert!(short.is_err());
        assert!(!observed.invalid && observed.live == 0);
    }
}

#[test]
fn complete_saved_merges_preserve_singletons_multigroups_and_call_commits() {
    complete_saved_merges(false);
    complete_saved_merges(true);
}

#[test]
fn consuming_merges_preserve_first_work_refusal_at_every_node_boundary() {
    let program = fixture();
    let function = &program.functions[0];
    let item = observation(function);
    for units in 0..32 {
        let mut a = ReplayBudget::with_physical(function, 1 << 22).unwrap();
        let mut b = ReplayBudget::with_physical(function, 1 << 22).unwrap();
        a.merge_paths = true;
        b.merge_paths = true;
        a.skeleton_remaining = units;
        b.skeleton_remaining = units;
        let mut old_work = SkeletonWork::new(&program, function, &mut a);
        let mut new_work = SkeletonWork::new(&program, function, &mut b);
        let old = old_work.tail_merge_control(vec![path(&item, 2), path(&item, 3)]);
        let new = new_work.merged_paths(vec![path(&item, 2), path(&item, 3)]);
        match (old, new) {
            (Ok(old), Ok(new)) => assert_paths(&old, &new),
            (Err(old), Err(new)) => {
                assert_eq!(old.code, new.code);
                assert_eq!(old.message, new.message);
            }
            _ => panic!("consumption changed the first work refusal"),
        }
        assert_eq!(
            old_work.budget.skeleton_spent,
            new_work.budget.skeleton_spent
        );
    }
}

fn extension_inputs(
    work: &mut SkeletonWork<'_, '_>,
    item: &Observation,
    target_len: usize,
    suffix_len: usize,
    target_prefix: bool,
    suffix_prefix: bool,
) -> Result<(Observations, Observations), Diagnostic> {
    let shared = if target_prefix || suffix_prefix {
        work.budget
            .reserve_array::<Observation>(work.function, 1, "extension shared prefix")?;
        Some(work.charged_dag(ObservationDagKind::Seq(None, vec![item.clone()]))?)
    } else {
        None
    };
    work.budget
        .reserve_array::<Observation>(work.function, target_len, "extension target tail")?;
    work.budget
        .reserve_array::<Observation>(work.function, suffix_len, "extension suffix tail")?;
    Ok((
        Observations {
            prefix: target_prefix.then(|| shared.as_ref().unwrap().clone()),
            items: vec![item.clone(); target_len],
        },
        Observations {
            prefix: suffix_prefix.then(|| shared.as_ref().unwrap().clone()),
            items: vec![item.clone(); suffix_len],
        },
    ))
}

#[test]
fn consuming_extensions_preserve_complete_shapes_work_errors_and_borrowed_suffixes() {
    let program = fixture();
    let function = &program.functions[0];
    let item = observation(function);
    for selected in [false, true] {
        for (target_prefix, suffix_prefix) in
            [(false, false), (false, true), (true, false), (true, true)]
        {
            for (target_len, suffix_len) in [(0, 0), (0, 2), (3, 0), (3, 2)] {
                for units in [
                    0,
                    1,
                    2,
                    3,
                    4,
                    5,
                    6,
                    7,
                    8,
                    9,
                    10,
                    11,
                    12,
                    31,
                    MAX_REPLAY_WORK_UNITS,
                ] {
                    let mut a = ReplayBudget::with_physical(function, 1 << 22).unwrap();
                    let mut b = ReplayBudget::with_physical(function, 1 << 22).unwrap();
                    if !selected {
                        a.admission = allocation::AdmissionMode::Ordinary;
                        b.admission = allocation::AdmissionMode::Ordinary;
                    }
                    a.merge_paths = true;
                    b.merge_paths = true;
                    a.skeleton_remaining = MAX_REPLAY_WORK_UNITS;
                    b.skeleton_remaining = MAX_REPLAY_WORK_UNITS;
                    let mut old_work = SkeletonWork::new(&program, function, &mut a);
                    let mut new_work = SkeletonWork::new(&program, function, &mut b);
                    let (old_target, old_suffix) = extension_inputs(
                        &mut old_work,
                        &item,
                        target_len,
                        suffix_len,
                        target_prefix,
                        suffix_prefix,
                    )
                    .unwrap();
                    let (new_target, new_suffix) = extension_inputs(
                        &mut new_work,
                        &item,
                        target_len,
                        suffix_len,
                        target_prefix,
                        suffix_prefix,
                    )
                    .unwrap();
                    let pointer = new_target.items.as_ptr();
                    old_work.budget.skeleton_remaining = units;
                    new_work.budget.skeleton_remaining = units;
                    let old = with_saved_extensions(|| {
                        old_work.extend_owned_observations(
                            old_target,
                            &old_suffix,
                            "extension receiver",
                        )
                    });
                    let new = new_work.extend_owned_observations(
                        new_target,
                        &new_suffix,
                        "extension receiver",
                    );
                    match (old, new) {
                        (Ok(old), Ok(new)) => {
                            assert_eq!(old.items, new.items);
                            assert_eq!(
                                old.prefix.as_ref().map(|p| shape(p)),
                                new.prefix.as_ref().map(|p| shape(p))
                            );
                            assert_eq!(
                                new_suffix.items.len(),
                                suffix_len,
                                "borrowed suffix remains held"
                            );
                            if suffix_prefix && target_len > 0 {
                                let ObservationDagKind::Cat(parts) =
                                    &new.prefix.as_ref().unwrap().kind
                                else {
                                    panic!("concatenation")
                                };
                                let ObservationDagKind::Seq(_, items) = &parts[0].kind else {
                                    panic!("owned head")
                                };
                                assert_eq!(
                                    items.as_ptr(),
                                    pointer,
                                    "receiving node holds original paid tail"
                                );
                                assert!(Rc::ptr_eq(&parts[1], new_suffix.prefix.as_ref().unwrap()));
                            }
                        }
                        (Err(old), Err(new)) => assert_eq!(old, new),
                        _ => {
                            panic!("consumption changed full extension result or first work error")
                        }
                    }
                    assert_eq!(
                        old_work.budget.skeleton_spent,
                        new_work.budget.skeleton_spent
                    );
                }
            }
        }
    }
}

fn extension_probe(
    program: &ResolvedProgram,
    selected: bool,
    saved: bool,
    limit: usize,
) -> (
    Result<(usize, usize), ()>,
    crate::test_allocation_observer::Metrics,
) {
    let function = &program.functions[0];
    let item = observation(function);
    let mut budget = ReplayBudget::with_physical(function, limit).unwrap();
    if !selected {
        budget.admission = allocation::AdmissionMode::Ordinary;
    }
    budget.merge_paths = true;
    budget.skeleton_remaining = MAX_REPLAY_WORK_UNITS;
    let before = budget.physical.as_ref().unwrap().charged();
    let mut work = SkeletonWork::new(program, function, &mut budget);
    crate::test_allocation_observer::measure(|| {
        let result = (|| {
            let (target, suffix) = extension_inputs(&mut work, &item, 3, 2, true, true)?;
            let pointer = target.items.as_ptr();
            let value = if saved {
                with_saved_extensions(|| {
                    work.extend_owned_observations(target, &suffix, "owned extension")
                })
            } else {
                work.extend_owned_observations(target, &suffix, "owned extension")
            }?;
            assert_eq!(value.items, suffix.items);
            let ObservationDagKind::Cat(parts) = &value.prefix.as_ref().unwrap().kind else {
                panic!("head")
            };
            let ObservationDagKind::Seq(_, items) = &parts[0].kind else {
                panic!("tail")
            };
            if !saved {
                assert_eq!(items.as_ptr(), pointer);
            }
            assert_eq!(items.len(), 3);
            assert!(Rc::ptr_eq(&parts[1], suffix.prefix.as_ref().unwrap()));
            drop(value);
            drop(suffix);
            Ok::<(), Diagnostic>(())
        })();
        drop(work.dag_scratch.take());
        drop(work.ordinary_scratch.take());
        match result {
            Ok(()) => Ok((
                work.budget.physical.as_ref().unwrap().charged() - before,
                work.budget.skeleton_spent,
            )),
            Err(error) => {
                assert_eq!(error.code, "SPX-H006");
                drop(error);
                let charged = work.budget.physical.as_ref().unwrap().charged();
                drop(
                    work.budget
                        .reserve_bytes(function, 0, "sticky extension receiver")
                        .unwrap_err(),
                );
                assert_eq!(work.budget.physical.as_ref().unwrap().charged(), charged);
                Err(())
            }
        }
    })
}

#[test]
fn owned_extension_literal_clone_removal_survives_receiving_exact_short_and_drop() {
    let program = fixture();
    let function = &program.functions[0];
    let setup = ReplayBudget::with_physical(function, 1 << 22)
        .unwrap()
        .physical
        .as_ref()
        .unwrap()
        .charged();
    let removed = 3 * std::mem::size_of::<Observation>();
    for selected in [false, true] {
        let (old, old_metrics) = extension_probe(&program, selected, true, 1 << 22);
        let (new, new_metrics) = extension_probe(&program, selected, false, 1 << 22);
        let (charged, spent) = new.unwrap();
        assert_eq!(old.unwrap(), (charged + removed, spent));
        assert_eq!(old_metrics.allocated, new_metrics.allocated + removed);
        assert_eq!(old_metrics.allocations, new_metrics.allocations + 1);
        assert!(new_metrics.highwater <= old_metrics.highwater);
        for metric in [old_metrics, new_metrics] {
            assert!(!metric.invalid);
            assert_eq!(metric.live, 0);
            assert_eq!(metric.allocated, metric.freed);
        }
        let (exact, metric) = extension_probe(&program, selected, false, setup + charged);
        assert_eq!(exact.unwrap(), (charged, spent));
        assert!(!metric.invalid && metric.live == 0);
        assert_eq!(metric.allocated, metric.freed);
        let (short, metric) = extension_probe(&program, selected, false, setup + charged - 1);
        assert!(short.is_err());
        assert!(!metric.invalid && metric.live == 0);
        assert_eq!(metric.allocated, metric.freed);
    }
}

#[test]
fn whole_factored_cleanup_keeps_saved_local_extension_results_and_diagnostics() {
    const BRANCHES: &str = "module extension.full; @id(\"consume\") fn consume(a:string,b:string)->i64 {string_len(a)+string_len(b)} @id(\"main\") fn main()->i64 {let a=if true {\"a\"} else {\"b\"}; consume(a,if false {\"c\"} else {\"d\"})}";
    for source in [
        BRANCHES,
        include_str!("../../../../../../tests/owned_data/byte_helper_renewal.spx"),
    ] {
        let ast = crate::check(source, "extension-full.spx").unwrap();
        let program = crate::hir::resolve(&ast).unwrap();
        assert_eq!(
            with_saved_extensions(|| validate_program(&program)),
            validate_program(&program),
        );
        let compare = || {
            let mut budget = ReplayBudget::unobserved();
            program
                .functions
                .iter()
                .map(|function| factored::compare(&program, function, &mut budget))
                .collect::<Result<Vec<_>, _>>()
        };
        let old = with_saved_extensions(compare);
        let new = compare();
        assert_eq!(old, new);
        assert!(new
            .unwrap()
            .iter()
            .all(|comparison| *comparison == Some(true)));
        let mut hostile = program.clone();
        let commit = hostile
            .functions
            .iter_mut()
            .flat_map(|f| &mut f.cleanup_plan.blocks)
            .flat_map(|b| &mut b.transitions)
            .find_map(|transition| match transition {
                CleanupTransition::CallCommit { arguments, .. } if arguments.len() > 1 => {
                    Some(arguments)
                }
                _ => None,
            });
        if let Some(commit) = commit {
            commit.swap(0, 1);
            let old = with_saved_extensions(|| validate_program(&hostile));
            let new = validate_program(&hostile);
            assert!(old.is_err());
            assert_eq!(old, new);
        }
    }
}

const WIDE_TAIL_EVENTS: usize = 4096;

/// Every member contains a real event and a separately paid one-element Vec.
/// This is independent of the retained wide-empty-Alt canonicalizer control.
fn wide_event_union(
    work: &mut SkeletonWork<'_, '_>,
    item: &Observation,
    saved: bool,
) -> Result<Observations, Diagnostic> {
    work.budget.reserve_array::<Observations>(
        work.function,
        WIDE_TAIL_EVENTS,
        "wide nonempty input carriers",
    )?;
    let mut members = Vec::with_capacity(WIDE_TAIL_EVENTS);
    for _ in 0..WIDE_TAIL_EVENTS {
        work.budget
            .reserve_array::<Observation>(work.function, 1, "wide nonempty input event")?;
        members.push(Observations::from(vec![item.clone()]));
    }
    if saved {
        work.saved_union_observations(members)
    } else {
        work.union_observations(members)
    }
}

#[test]
fn selected_4096_nonempty_event_tails_keep_literal_old_current_removed_through_receiver_drop() {
    let program = fixture();
    let function = &program.functions[0];
    let item = observation(function);
    // Complete ordinal/node/duplicate/event oracle outside the storage witness.
    let mut old_budget = ReplayBudget::with_physical(function, 1 << 25).unwrap();
    let mut current_budget = ReplayBudget::with_physical(function, 1 << 25).unwrap();
    old_budget.skeleton_remaining = MAX_REPLAY_WORK_UNITS;
    current_budget.skeleton_remaining = MAX_REPLAY_WORK_UNITS;
    let mut old_work = SkeletonWork::new(&program, function, &mut old_budget);
    let mut current_work = SkeletonWork::new(&program, function, &mut current_budget);
    let old = wide_event_union(&mut old_work, &item, true).unwrap();
    let current = wide_event_union(&mut current_work, &item, false).unwrap();
    assert_eq!(
        shape(old.prefix.as_ref().unwrap()),
        shape(current.prefix.as_ref().unwrap()),
    );
    assert_eq!(
        old_work.budget.skeleton_spent,
        current_work.budget.skeleton_spent
    );
    drop(old);
    drop(current);
    drop(old_work);
    drop(current_work);

    let observe = |saved, limit| {
        let mut budget = ReplayBudget::with_physical(function, limit).unwrap();
        budget.skeleton_remaining = MAX_REPLAY_WORK_UNITS;
        let mut work = SkeletonWork::new(&program, function, &mut budget);
        let before = work.budget.physical.as_ref().unwrap().charged();
        crate::test_allocation_observer::measure(|| {
            let result = wide_event_union(&mut work, &item, saved);
            let accepted = result.is_ok();
            if let Ok(value) = &result {
                let root = value.prefix.as_ref().unwrap();
                assert_eq!(root.ordinal, Some(WIDE_TAIL_EVENTS));
                let ObservationDagKind::Alt(parts) = &root.kind else {
                    panic!("wide nonempty alternatives")
                };
                assert_eq!(parts.len(), WIDE_TAIL_EVENTS);
                for (ordinal, part) in parts.iter().enumerate() {
                    assert_eq!(part.ordinal, Some(ordinal));
                    let ObservationDagKind::Seq(None, events) = &part.kind else {
                        panic!("wide nonempty event receiver")
                    };
                    assert_eq!(events.as_slice(), std::slice::from_ref(&item));
                }
            }
            // Receiving result and the shared iterative destruction carrier
            // are both released in this actual allocator window.
            drop(result);
            drop(work.dag_scratch.take());
            drop(work.ordinary_scratch.take());
            if !accepted {
                let charged = work.budget.physical.as_ref().unwrap().charged();
                drop(
                    work.budget
                        .reserve_bytes(function, 0, "sticky wide nonempty event union")
                        .unwrap_err(),
                );
                assert_eq!(work.budget.physical.as_ref().unwrap().charged(), charged);
                return Err(());
            }
            Ok((
                work.budget.physical.as_ref().unwrap().charged() - before,
                work.budget.skeleton_spent,
            ))
        })
    };
    let (old, old_metrics) = observe(true, 1 << 25);
    let (current, current_metrics) = observe(false, 1 << 25);
    let (charged, spent) = current.unwrap();
    let removed = WIDE_TAIL_EVENTS * std::mem::size_of::<Observation>();
    assert_eq!(old.unwrap(), (charged + removed, spent));
    assert_eq!(old_metrics.allocated, current_metrics.allocated + removed);
    assert_eq!(
        old_metrics.allocations,
        current_metrics.allocations + WIDE_TAIL_EVENTS
    );
    for metrics in [old_metrics, current_metrics] {
        assert!(!metrics.invalid && metrics.live == 0);
        assert_eq!(metrics.allocated, metrics.freed);
    }
    let admission = ReplayBudget::with_physical(function, 1 << 25)
        .unwrap()
        .physical
        .as_ref()
        .unwrap()
        .charged();
    let (exact, metrics) = observe(false, admission + charged);
    assert_eq!(exact.unwrap(), (charged, spent));
    assert!(!metrics.invalid && metrics.live == 0 && metrics.allocated <= charged);
    let (short, metrics) = observe(false, admission + charged - 1);
    assert!(short.is_err());
    assert!(!metrics.invalid && metrics.live == 0);
    assert_eq!(metrics.allocated, metrics.freed);
}
