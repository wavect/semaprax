//! Persistent-input baseline and guard owners; original map controls remain.
use super::*;
use crate::test_allocation_observer::measure;
fn rich(count: usize) -> BTreeMap<ValueId, ValidationBinding> {
    (0..count)
        .map(|index| {
            let path = vec![PlaceProjection::Field(DeclarationId::new("scope.field")); 3];
            let mut moved_places = BTreeMap::new();
            moved_places.insert(path.clone(), Availability::MaybeMoved);
            let mut definitely_partial = BTreeSet::new();
            definitely_partial.insert(path);
            let mut active_loans = BTreeSet::new();
            active_loans.insert(LoanId(index as u16));
            (
                ValueId::new(format!("scope.binding.{index:04}")),
                ValidationBinding {
                    contract_local: index % 2 == 0,
                    mutable: index % 3 == 0,
                    ty: ResolvedType::Function {
                        parameters: vec![
                            ResolvedType::Nominal {
                                declaration: DeclarationId::new("scope.nominal"),
                                arguments: vec![ResolvedType::I64, ResolvedType::Bool],
                            };
                            2
                        ],
                        result: Box::new(ResolvedType::I64),
                    },
                    ownership: OwnershipMode::Own,
                    availability: Availability::Available,
                    moved_places,
                    definitely_partial,
                    active_loans,
                },
            )
        })
        .collect()
}
#[test]
fn persistent_input_baseline_resets_only_freshness_and_retains_full_hostile_payloads() {
    let input = rich(17);
    Scope::prepare(input.len()).unwrap();
    let outer = Scope::from_prepaid(input.clone());
    let before = outer.clone();
    let mut expected = input.clone();
    for value in expected.values_mut() {
        value.contract_local = false;
    }
    let (((), actual), refused, paid) =
        crate::bounded_output::with_limit_usage(64 * 1024 * 1024, || {
            measure(|| {
                let baseline = Scope::fresh_baseline(&outer).unwrap();
                assert!(baseline.iter().eq(expected.iter()));
                for _ in 0..17 {
                    let mut clause = baseline.clone();
                    let id = clause.keys().next().unwrap().clone();
                    clause.get_mut(&id).unwrap().unwrap().availability = Availability::Moved;
                    assert_eq!(
                        baseline.get(&id).unwrap().availability,
                        Availability::Available
                    );
                    drop(clause);
                }
                assert_eq!(outer, before);
                drop(baseline);
            })
        });
    assert!(!refused && !actual.invalid);
    assert_eq!(actual.live, 0);
    assert!(actual.allocated <= paid && actual.highwater <= paid);
    assert!(outer.iter().eq(input.iter()));
}
#[test]
fn checkpoint_target_guard_keeps_full_published_state_on_unwind_without_map_materialization() {
    let mut outer = Scope::from_prepaid(rich(17));
    let before = outer.clone();
    let (((), actual), refused, _) =
        crate::bounded_output::with_limit_usage(64 * 1024 * 1024, || {
            measure(|| {
                let caught = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    let mut publication = Publication::take(&mut outer).unwrap();
                    let mut current = publication.published.clone();
                    let id = current.keys().next().unwrap().clone();
                    current.get_mut(&id).unwrap().unwrap().availability = Availability::Moved;
                    publication.publish(&current).unwrap();
                    let mut suppressed = current.clone();
                    suppressed.get_mut(&id).unwrap().unwrap().mutable = false;
                    publication.enabled = false;
                    publication.publish(&suppressed).unwrap();
                    // Real owned payload is caught and dropped below. Avoid the harness
                    // panic hook's captured output, which survives this observation.
                    std::panic::resume_unwind(Box::new(String::from(
                        "injected with persistent publication and children live",
                    )));
                }));
                assert!(caught.is_err());
                assert_eq!(
                    caught
                        .as_ref()
                        .err()
                        .unwrap()
                        .downcast_ref::<String>()
                        .map(String::as_str),
                    Some("injected with persistent publication and children live"),
                );
                drop(caught);
                let id = outer.keys().next().unwrap();
                let mut expected = before.get(id).unwrap().clone();
                expected.availability = Availability::Moved;
                assert_eq!(outer.get(id).unwrap(), &expected);
                assert!(outer.iter().skip(1).eq(before.iter().skip(1)));
                drop(std::mem::take(&mut outer));
            })
        });
    assert!(!refused && !actual.invalid);
    assert_eq!(actual.live, 0);
}

#[test]
fn prepared_insertion_drop_and_payload_refusal_preserve_the_complete_prior_root_and_debits() {
    let input = rich(17);
    let mut outer = Scope::from_prepaid(input.clone());
    let before = outer.clone();
    let (((), observed), refused, insertion_paid) =
        crate::bounded_output::with_limit_usage(64 * 1024 * 1024, || {
            measure(|| {
                let insertion = outer.prepare_insert().unwrap();
                #[allow(
                    clippy::drop_non_drop,
                    reason = "End this borrowed proof at the measured lifetime boundary."
                )]
                drop(insertion);
            })
        });
    assert!(!refused && !observed.invalid && insertion_paid > 0);
    assert_eq!(observed.allocated, 0);
    assert_eq!(observed.live, 0);
    assert_eq!(outer, before);
    assert!(outer.iter().eq(input.iter()));
    let payload = &input.values().next().unwrap().ty;
    let (((), observed), refused, paid, receipt) =
        crate::bounded_output::with_limit_usage_refusal(insertion_paid, || {
            measure(|| {
                let insertion = outer.prepare_insert().unwrap();
                let error = charge::clone_type(payload).unwrap_err();
                // The paid insertion owner drops without committing on clone refusal.
                #[allow(
                    clippy::drop_non_drop,
                    reason = "End this borrowed proof at the measured lifetime boundary."
                )]
                drop(insertion);
                assert!(!crate::bounded_output::reserve_active_required(0));
                drop(error);
            })
        });
    assert!(refused && receipt.is_some() && !observed.invalid);
    assert_eq!(paid, insertion_paid);
    assert_eq!(observed.allocated, 0);
    assert_eq!(observed.live, 0);
    assert_eq!(outer, before);
    assert!(outer.iter().eq(input.iter()));
}
