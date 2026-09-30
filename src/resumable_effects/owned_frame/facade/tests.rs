use super::super::journal::tests::{fixture, Directory};
use super::*;
use std::cell::Cell;
use std::rc::Rc;
fn policy() -> CapabilityPolicy {
    CapabilityPolicy::new(vec!["fixture.park".into()]).unwrap()
}
#[test]
fn owned_frame_public_facade_claims_one_real_owner_and_refuses_foreign_consumption() {
    let directory = Directory::new();
    let (plan, input, scope, key) = fixture();
    let argument =
        admit_owned_frame_input(&plan, input).unwrap_or_else(|e| panic!("{:?}", e.diagnostic));
    let weak = core::snapshot::argument_weak(argument.inner.as_ref().unwrap());
    let prepared = PreparedOwnedFrame::new(&plan, &argument, scope.clone(), 100, 2000).unwrap();
    let lease =
        RegisteredOwnedFrameJournalLease::fresh(directory.file(), directory.identity(), &scope)
            .unwrap();
    let mut invocation =
        match OwnedFrameInvocation::start(prepared, argument, lease, &key, &policy()) {
            OwnedFrameStart::Invocation {
                invocation,
                acknowledgement,
            } => {
                acknowledgement.unwrap();
                invocation
            }
            OwnedFrameStart::Rejected { error, .. } => panic!("{error:?}"),
        };
    invocation.begin(&policy(), &scope).unwrap();
    invocation
        .dispatch(&policy(), &scope, &mut |_| Ok(ArgumentValue::Int(1)))
        .unwrap();
    invocation.resume(&policy(), &scope).unwrap();
    invocation.settle(&policy(), &scope, &mut |_| true).unwrap();
    let mut result = invocation.claim(&policy(), &scope).unwrap();
    assert_eq!(
        invocation.status().unwrap(),
        OwnedFrameInvocationStatus::ResultDeliveryInDoubt
    );
    assert!(weak.iter().all(|leaf| leaf.strong_count() == 1));
    let releases = Rc::new(Cell::new(0));
    let counter = releases.clone();
    core::snapshot::observe_releases(Some(Box::new(move |_| counter.set(counter.get() + 1))));
    result.creator = std::process::id().wrapping_add(1);
    let rejection = match result.into_argument(&plan) {
        Err(e) => e,
        Ok(_) => panic!("foreign owner extracted"),
    };
    assert_eq!(rejection.error, OwnedFrameError::Policy);
    assert_eq!(releases.get(), 0);
    assert!(weak.iter().all(|leaf| leaf.upgrade().is_some()));
    drop(rejection); // explicitly disarms core semantic Drop, backing only
    assert_eq!(releases.get(), 0);
    assert!(weak.iter().all(|leaf| leaf.upgrade().is_none()));
    core::snapshot::observe_releases(None);
}
#[test]
fn owned_frame_public_foreign_argument_drop_disarms_foundation_disposer() {
    let (plan, input, _, _) = fixture();
    let mut argument = admit_owned_frame_input(&plan, input.clone())
        .unwrap_or_else(|e| panic!("{:?}", e.diagnostic));
    let weak = core::snapshot::argument_weak(argument.inner.as_ref().unwrap());
    let releases = Rc::new(Cell::new(0));
    let counter = releases.clone();
    core::snapshot::observe_releases(Some(Box::new(move |_| counter.set(counter.get() + 1))));
    argument.creator = std::process::id().wrapping_add(1);
    let scope = SourceCheckpointScope::new("sha256:program", "foreign", 0).unwrap();
    assert!(PreparedOwnedFrame::new(&plan, &argument, scope, 100, 2000).is_err());
    assert_eq!(releases.get(), 0);
    drop(argument);
    assert_eq!(releases.get(), 0);
    assert!(weak.iter().all(|leaf| leaf.upgrade().is_none()));
    // The control proves the observer sees ordinary foundation destruction.
    drop(admit_owned_frame_input(&plan, input).unwrap_or_else(|e| panic!("{:?}", e.diagnostic)));
    assert_eq!(releases.get(), 3);
    core::snapshot::observe_releases(None);
}
