use super::*;

fn digest(byte: char) -> String {
    assert!(byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase());
    format!("sha256:{}", byte.to_string().repeat(64))
}

fn subjects() -> Vec<RetentionObservation> {
    vec![
        RetentionObservation::new(
            RetentionSubject::image(digest('1'), digest('2'), digest('3')).unwrap(),
            11,
        )
        .unwrap(),
        RetentionObservation::new(
            RetentionSubject::candidate(digest('4'), digest('5'), digest('6')).unwrap(),
            13,
        )
        .unwrap(),
        RetentionObservation::new(
            RetentionSubject::draft(digest('7'), digest('8'), digest('9')).unwrap(),
            17,
        )
        .unwrap(),
    ]
}

fn code(error: &[Diagnostic]) -> &str {
    error.first().expect("one retention diagnostic").code
}

#[test]
fn transition_is_input_order_independent_and_evicts_in_canonical_order() {
    let observations = subjects();
    let initial_policy = RetentionPolicy::new(3, 64, 0).unwrap();
    let first = checkpoint(None, None, 1, initial_policy, &observations).unwrap();
    assert_eq!(first.checkpoint().retained_subjects().len(), 3);
    assert_eq!(first.checkpoint().retained_bytes(), 41);
    assert_eq!(first.evicted_subjects().len(), 0);

    let mut expected = observations
        .iter()
        .map(|observation| observation.subject().clone())
        .collect::<Vec<_>>();
    expected.sort_by_key(RetentionSubject::subject_digest);
    let newest = expected[2].clone();
    let next_policy = RetentionPolicy::new(2, 64, 0).unwrap();
    let next = checkpoint(
        Some(first.checkpoint()),
        Some(first.checkpoint().checkpoint_digest()),
        2,
        next_policy,
        &[RetentionObservation::new(
            newest.clone(),
            match newest.kind() {
                "image" => 11,
                "candidate" => 13,
                "draft" => 17,
                _ => unreachable!(),
            },
        )
        .unwrap()],
    )
    .unwrap();
    let retained = next
        .checkpoint()
        .retained_subjects()
        .cloned()
        .collect::<Vec<_>>();
    assert!(retained.contains(&newest));
    assert_eq!(next.evicted_subjects().count(), 1);
    assert_eq!(next.evicted_subjects().next(), Some(&expected[1]));

    let mut reversed = observations;
    reversed.reverse();
    let repeated = checkpoint(None, None, 1, initial_policy, &reversed).unwrap();
    assert_eq!(
        first.checkpoint().to_json(),
        repeated.checkpoint().to_json()
    );
    assert_eq!(first.plan_json(), repeated.plan_json());
}

#[test]
fn stale_predecessor_and_rollback_selectors_fail_closed() {
    let policy = RetentionPolicy::new(3, 64, 0).unwrap();
    let first = checkpoint(None, None, 1, policy, &subjects()).unwrap();
    let stale =
        checkpoint(Some(first.checkpoint()), Some(&digest('a')), 2, policy, &[]).unwrap_err();
    assert_eq!(code(&stale), "SPX-G423");

    let second = checkpoint(
        Some(first.checkpoint()),
        Some(first.checkpoint().checkpoint_digest()),
        2,
        policy,
        &[],
    )
    .unwrap();
    let stale_parent = restore_checkpoint(
        second.checkpoint().to_json().as_bytes(),
        second.checkpoint().checkpoint_digest(),
        Some(&digest('b')),
    )
    .unwrap_err();
    assert_eq!(code(&stale_parent), "SPX-G423");

    let rollback = restore_checkpoint(
        first.checkpoint().to_json().as_bytes(),
        second.checkpoint().checkpoint_digest(),
        None,
    )
    .unwrap_err();
    assert_eq!(code(&rollback), "SPX-G422");
}

#[test]
fn tampered_checkpoint_and_gc_plan_never_restore() {
    let transition = checkpoint(
        None,
        None,
        1,
        RetentionPolicy::new(2, 64, 0).unwrap(),
        &subjects(),
    )
    .unwrap();

    let mut checkpoint_value: Value =
        serde_json::from_str(transition.checkpoint().to_json()).unwrap();
    checkpoint_value["retained_bytes"] = json!(1);
    let tampered_checkpoint = format!("{}\n", serde_json::to_string(&checkpoint_value).unwrap());
    let error = restore_checkpoint(
        tampered_checkpoint.as_bytes(),
        transition.checkpoint().checkpoint_digest(),
        None,
    )
    .unwrap_err();
    assert_eq!(code(&error), "SPX-G422");

    let mut plan_value: Value = serde_json::from_str(transition.plan_json()).unwrap();
    plan_value["checkpoint_digest"] = json!(digest('c'));
    let tampered_plan = format!("{}\n", serde_json::to_string(&plan_value).unwrap());
    let error = restore_plan(
        tampered_plan.as_bytes(),
        transition.plan_digest(),
        transition.checkpoint(),
    )
    .unwrap_err();
    assert_eq!(code(&error), "SPX-G422");
}

#[test]
fn protected_generation_cannot_be_silently_evicted_to_fit() {
    let error = checkpoint(
        None,
        None,
        1,
        RetentionPolicy::new(1, MAX_RETENTION_TOTAL_BYTES, 1).unwrap(),
        &subjects()[..2],
    )
    .unwrap_err();
    assert_eq!(code(&error), "SPX-G421");
}

#[test]
fn restored_metadata_explicitly_carries_no_action_authority() {
    let transition = checkpoint(
        None,
        None,
        1,
        RetentionPolicy::new(2, 64, 0).unwrap(),
        &subjects(),
    )
    .unwrap();
    let restored_checkpoint = restore_checkpoint(
        transition.checkpoint().to_json().as_bytes(),
        transition.checkpoint().checkpoint_digest(),
        None,
    )
    .unwrap();
    let restored_plan = restore_plan(
        transition.plan_json().as_bytes(),
        transition.plan_digest(),
        &restored_checkpoint,
    )
    .unwrap();

    assert_eq!(restored_checkpoint.authority(), RetentionAuthority::None);
    assert_eq!(restored_plan.authority(), RetentionAuthority::None);
    for bytes in [restored_checkpoint.to_json(), restored_plan.to_json()] {
        assert!(!bytes.contains("\"store_root\":"));
        assert!(!bytes.contains("\"source_authority\":true"));
        assert!(!bytes.contains("\"approval\":true"));
        assert!(!bytes.contains("\"effect\":\"delete"));
    }
    assert_eq!(restored_plan.to_json(), transition.plan_json());
}
