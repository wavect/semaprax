use super::super::durable::DurableOwner;
use super::*;

#[test]
fn owned_frame_durable_borrowed_replay_keeps_one_actual_owner() {
    let plan = checked_plan(SOURCE);
    let argument = admitted_argument(&plan);
    let weak = weak_backing(argument.root.as_ref().unwrap());
    let before = snapshot::argument_input(&argument).unwrap();
    let owner = DurableOwner::from_argument(argument);
    let DurableOwner::PreYield { plan, root } = &owner else {
        panic!()
    };
    let mut budget = OwnedFrameBudget::new(100).unwrap();
    let (facts, copy_environment, _) = replay::evaluate(
        plan,
        root,
        Environment::from(Vec::new()),
        0,
        None,
        true,
        &mut budget,
    );
    assert!(matches!(facts, Ok(Some((ArgumentValue::Int(5), _)))));
    drop(copy_environment);
    assert!(budget.consumed() > 0);
    assert!(weak.iter().all(|leaf| leaf.strong_count() == 1));
    let after = owner.input().unwrap();
    assert_eq!(before.fields.len(), after.fields.len());
    assert!(weak.iter().all(|leaf| leaf.upgrade().is_some()));
    drop(owner); // process backing disposal, no semantic receipt
    assert!(weak.iter().all(|leaf| leaf.upgrade().is_none()));
}

#[test]
fn owned_frame_durable_unpublished_success_retains_backing_until_claim() {
    let plan = checked_plan(SOURCE);
    let argument = admitted_argument(&plan);
    let weak = weak_backing(argument.root.as_ref().unwrap());
    let mut budget = OwnedFrameBudget::new(100).unwrap();
    let owner = DurableOwner::from_argument(argument).start(&mut budget);
    assert_eq!(owner.request(), Some(&ArgumentValue::Int(5)));
    let terminal = owner.resume(ArgumentValue::Int(7), &mut budget);
    let mut callbacks = 0;
    let release = terminal
        .settle(&mut |_| {
            callbacks += 1;
            true
        })
        .unwrap_or_else(|(_, e)| panic!("{e:?}"));
    assert_eq!(callbacks, 0);
    assert!(release.failure.is_none());
    assert!(release.observations.is_empty());
    let unpublished = release.unpublished.unwrap();
    assert!(weak.iter().all(|leaf| leaf.strong_count() == 1));
    assert!(snapshot::root_input(&unpublished.plan, &unpublished.root).is_ok());
    drop(unpublished); // no caller result was delivered; backing only
    assert!(weak.iter().all(|leaf| leaf.upgrade().is_none()));
}

#[test]
fn owned_frame_durable_callback_panic_observes_real_drop_and_continues_every_leaf() {
    let source = SOURCE.replace("yields i64 -> i64 {", "yields i64 -> i64 requires false {");
    let plan = checked_plan(&source);
    let argument = admitted_argument(&plan);
    let weak = weak_backing(argument.root.as_ref().unwrap());
    let mut budget = OwnedFrameBudget::new(100).unwrap();
    let terminal = DurableOwner::from_argument(argument).start(&mut budget);
    let primary = terminal.failure().cloned().unwrap();
    let mut observed = Vec::new();
    let release = terminal
        .settle(&mut |action| {
            observed.push(action.source.projections[0].as_str().to_owned());
            if observed.len() == 1 {
                assert!(weak[1].upgrade().is_none());
                assert!(weak[0].upgrade().is_some());
                panic!("interrupted host observation after actual first drop");
            }
            assert!(weak.iter().all(|leaf| leaf.upgrade().is_none()));
            true
        })
        .unwrap_or_else(|(_, e)| panic!("{e:?}"));
    assert_eq!(observed, ["fixture.state.a", "fixture.state.z"]);
    assert_eq!(release.failure, Some(primary));
    assert!(release.unpublished.is_none());
    assert_eq!(
        release
            .observations
            .iter()
            .map(|(_, success)| *success)
            .collect::<Vec<_>>(),
        [false, true]
    );
    assert!(weak.iter().all(|leaf| leaf.upgrade().is_none()));
}
