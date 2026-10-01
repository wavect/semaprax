use super::super::authenticated_intent::{
    deliver_http_durable_authenticated, http_intent_body_digest, read_http_intent,
    HttpIntentLookup, HttpIntentReadRefusal,
};
use super::*;

const KEY: [u8; 32] = [37; 32];
const SIGNED_AT: i64 = 1_700_000_000;

fn identity() -> String {
    prepare_http_delivery(capability(), request())
        .unwrap()
        .pending_identity_key()
        .to_owned()
}

#[test]
fn authenticated_intent_reopens_exact_facts_and_never_redispatches() {
    let temp = TempDirectory::new();
    let identity = identity();
    {
        let directory = platform::hold_directory(temp.path()).unwrap();
        let mut store = OutboundDeliveryStore::new(&directory);
        assert_eq!(
            read_http_intent(&store, &identity, &KEY),
            Ok(HttpIntentLookup::Absent)
        );
        assert_eq!(fs::read_dir(temp.path()).unwrap().count(), 0);
        let mut adapter = RecordingAdapter::default();
        let outcome = deliver_http_durable_authenticated(
            &mut store,
            2,
            capability(),
            request(),
            SIGNED_AT,
            &KEY,
            &mut adapter,
        )
        .unwrap();
        assert!(matches!(outcome, ServiceHttpDeliveryOutcome::Dispatched(_)));
        assert_eq!(adapter.0.len(), 1);
    }
    let directory = platform::hold_directory(temp.path()).unwrap();
    let mut store = OutboundDeliveryStore::new(&directory);
    let HttpIntentLookup::Authenticated(facts) = read_http_intent(&store, &identity, &KEY).unwrap()
    else {
        panic!("restart must recover authenticated facts");
    };
    assert_eq!(facts.signed_at(), SIGNED_AT);
    assert_eq!(facts.attempt(), 1);
    assert_eq!(
        facts.body_digest(),
        http_intent_body_digest(&request().body)
    );
    let mut changed = request();
    changed.body.push(b'!');
    assert_ne!(facts.body_digest(), http_intent_body_digest(&changed.body));
    let outcome = deliver_http_durable_authenticated(
        &mut store,
        3,
        capability_with_policy_id("changed-policy"),
        changed,
        SIGNED_AT + 1,
        &KEY,
        &mut PanicOnDispatch,
    )
    .unwrap();
    assert!(matches!(outcome, ServiceHttpDeliveryOutcome::Uncertain));
    assert_eq!(
        read_http_intent(&store, &identity, &KEY),
        Ok(HttpIntentLookup::Authenticated(facts))
    );
}

#[test]
fn legacy_and_authenticated_intents_block_each_other_without_upgrading_bytes() {
    for legacy_first in [true, false] {
        let temp = TempDirectory::new();
        let directory = platform::hold_directory(temp.path()).unwrap();
        let mut store = OutboundDeliveryStore::new(&directory);
        let mut adapter = RecordingAdapter::default();
        let first = if legacy_first {
            deliver_http_durable(&mut store, 2, None, capability(), request(), &mut adapter)
        } else {
            deliver_http_durable_authenticated(
                &mut store,
                2,
                capability(),
                request(),
                SIGNED_AT,
                &KEY,
                &mut adapter,
            )
        }
        .unwrap();
        assert!(matches!(first, ServiceHttpDeliveryOutcome::Dispatched(_)));
        let name =
            pending_intent_filename(OutboundCheckpointKind::HttpSession, &identity()).unwrap();
        let before = fs::read(temp.path().join(&name)).unwrap();
        if legacy_first {
            assert_eq!(before, PENDING_INTENT_BYTES);
            assert_eq!(
                read_http_intent(&store, &identity(), &KEY),
                Ok(HttpIntentLookup::LegacyBlocked)
            );
        }
        let second = if legacy_first {
            deliver_http_durable_authenticated(
                &mut store,
                2,
                capability(),
                request(),
                SIGNED_AT,
                &KEY,
                &mut PanicOnDispatch,
            )
        } else {
            deliver_http_durable(
                &mut store,
                2,
                None,
                capability(),
                request(),
                &mut PanicOnDispatch,
            )
        }
        .unwrap();
        assert!(matches!(second, ServiceHttpDeliveryOutcome::Uncertain));
        assert_eq!(fs::read(temp.path().join(name)).unwrap(), before);
    }
}

#[test]
fn tampering_wrong_key_and_transplanted_identity_refuse_without_mutation() {
    let temp = TempDirectory::new();
    let directory = platform::hold_directory(temp.path()).unwrap();
    let mut store = OutboundDeliveryStore::new(&directory);
    deliver_http_durable_authenticated(
        &mut store,
        2,
        capability(),
        request(),
        SIGNED_AT,
        &KEY,
        &mut RecordingAdapter::default(),
    )
    .unwrap();
    let identity = identity();
    let name = pending_intent_filename(OutboundCheckpointKind::HttpSession, &identity).unwrap();
    let path = temp.path().join(name);
    let original = fs::read(&path).unwrap();
    assert_eq!(
        read_http_intent(&store, &identity, &[38; 32]),
        Err(HttpIntentReadRefusal::InvalidRecord)
    );
    let text = String::from_utf8(original.clone()).unwrap();
    for edited in [
        text.replace(&SIGNED_AT.to_string(), &(SIGNED_AT + 1).to_string()),
        text.replace(&SIGNED_AT.to_string(), &format!("0{SIGNED_AT}")),
        text.replace("\n1\n", "\n2\n"),
        format!("{text}\n"),
        text.replace("sha256:", "sha256:A"),
        String::from("truncated\n"),
    ] {
        fs::write(&path, edited.as_bytes()).unwrap();
        assert_eq!(
            read_http_intent(&store, &identity, &KEY),
            Err(HttpIntentReadRefusal::InvalidRecord)
        );
        assert_eq!(fs::read(&path).unwrap(), edited.as_bytes());
        assert!(matches!(
            deliver_http_durable_authenticated(
                &mut store,
                2,
                capability(),
                request(),
                SIGNED_AT,
                &KEY,
                &mut PanicOnDispatch,
            )
            .unwrap(),
            ServiceHttpDeliveryOutcome::Uncertain
        ));
    }
    fs::write(&path, &original).unwrap();
    let mut changed = request();
    changed.idempotency_key.push_str("-other");
    let other = prepare_http_delivery(capability(), changed)
        .unwrap()
        .pending_identity_key()
        .to_owned();
    let other_name = pending_intent_filename(OutboundCheckpointKind::HttpSession, &other).unwrap();
    fs::write(temp.path().join(other_name), &original).unwrap();
    assert_eq!(
        read_http_intent(&store, &other, &KEY),
        Err(HttpIntentReadRefusal::InvalidRecord)
    );
    assert_eq!(fs::read(&path).unwrap(), original);
}

#[test]
fn invalid_timestamp_and_unsafe_lookup_refuse_before_writes_or_dispatch() {
    let temp = TempDirectory::new();
    let directory = platform::hold_directory(temp.path()).unwrap();
    let mut store = OutboundDeliveryStore::new(&directory);
    assert!(matches!(
        deliver_http_durable_authenticated(
            &mut store,
            2,
            capability(),
            request(),
            -1,
            &KEY,
            &mut PanicOnDispatch,
        ),
        Err(ServiceHttpDeliveryRefusal::InvalidRequest)
    ));
    assert_eq!(
        read_http_intent(&store, "../marker", &KEY),
        Err(HttpIntentReadRefusal::InvalidIdentity)
    );
    assert_eq!(fs::read_dir(temp.path()).unwrap().count(), 0);
    let name = pending_intent_filename(OutboundCheckpointKind::HttpSession, &identity()).unwrap();
    fs::create_dir(temp.path().join(name)).unwrap();
    assert_eq!(
        read_http_intent(&store, &identity(), &KEY),
        Err(HttpIntentReadRefusal::StorageUnavailable)
    );
}

#[test]
fn authenticated_create_race_dispatches_once_after_two_absence_observations() {
    let temp = TempDirectory::new();
    let barrier = std::sync::Arc::new(std::sync::Barrier::new(2));
    let dispatched = std::thread::scope(|scope| {
        let mut workers = Vec::new();
        for _ in 0..2 {
            let path = temp.path();
            let barrier = barrier.clone();
            workers.push(scope.spawn(move || {
                // Acquire independently in each thread; no directory or adapter
                // authority is transferred between the two host invocations.
                let directory = platform::hold_directory(path).unwrap();
                let mut store = OutboundDeliveryStore::new(&directory);
                assert_eq!(
                    read_http_intent(&store, &identity(), &KEY),
                    Ok(HttpIntentLookup::Absent)
                );
                barrier.wait();
                let mut adapter = RecordingAdapter::default();
                let outcome = deliver_http_durable_authenticated(
                    &mut store,
                    2,
                    capability(),
                    request(),
                    SIGNED_AT,
                    &KEY,
                    &mut adapter,
                )
                .unwrap();
                assert_eq!(
                    matches!(outcome, ServiceHttpDeliveryOutcome::Dispatched(_)),
                    adapter.0.len() == 1
                );
                adapter.0.len()
            }));
        }
        workers
            .into_iter()
            .map(|worker| worker.join().unwrap())
            .sum::<usize>()
    });
    assert_eq!(dispatched, 1);
}

#[test]
fn authenticated_intent_without_session_checkpoint_blocks_restart_dispatch() {
    let temp = TempDirectory::new();
    let directory = platform::hold_directory(temp.path()).unwrap();
    let mut store = OutboundDeliveryStore::new(&directory);
    // Independently encode the versioned wire to exercise restart from a
    // marker left before the typed provisional checkpoint was committed.
    use hmac::{Hmac, KeyInit, Mac};
    let prefix = format!(
        "semaprax.outbound.authenticated-http-intent.v1\n{}\n{}\n{SIGNED_AT}\n1\n",
        identity(),
        http_intent_body_digest(&request().body),
    );
    let mut mac = Hmac::<sha2::Sha256>::new_from_slice(&KEY).unwrap();
    mac.update(b"semaprax.outbound.authenticated-http-intent.mac.v1\0");
    mac.update(prefix.as_bytes());
    let tag: String = mac
        .finalize()
        .into_bytes()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect();
    let bytes = format!("{prefix}{tag}\n").into_bytes();
    assert_eq!(
        commit_pending_intent_bytes(
            &directory,
            OutboundCheckpointSyncMode::FileOnly,
            OutboundCheckpointKind::HttpSession,
            &identity(),
            &bytes,
            super::super::authenticated_intent::MAX_AUTHENTICATED_INTENT_BYTES,
        ),
        PendingIntentCommit::Fresh
    );
    assert_eq!(fs::read_dir(temp.path()).unwrap().count(), 1);
    let HttpIntentLookup::Authenticated(facts) =
        read_http_intent(&store, &identity(), &KEY).unwrap()
    else {
        panic!("independently encoded record must authenticate");
    };
    assert_eq!(facts.signed_at(), SIGNED_AT);
    assert!(matches!(
        deliver_http_durable_authenticated(
            &mut store,
            2,
            capability(),
            request(),
            SIGNED_AT,
            &KEY,
            &mut PanicOnDispatch,
        )
        .unwrap(),
        ServiceHttpDeliveryOutcome::Uncertain
    ));
    assert_eq!(fs::read_dir(temp.path()).unwrap().count(), 1);
}

#[cfg(unix)]
#[test]
fn symlink_marker_is_not_absence_or_authenticated_data() {
    let temp = TempDirectory::new();
    let directory = platform::hold_directory(temp.path()).unwrap();
    let mut store = OutboundDeliveryStore::new(&directory);
    let target = temp.path().join("held-target");
    fs::write(&target, PENDING_INTENT_BYTES).unwrap();
    let name = pending_intent_filename(OutboundCheckpointKind::HttpSession, &identity()).unwrap();
    std::os::unix::fs::symlink(&target, temp.path().join(name)).unwrap();
    assert_eq!(
        read_http_intent(&store, &identity(), &KEY),
        Err(HttpIntentReadRefusal::StorageUnavailable)
    );
    assert!(matches!(
        deliver_http_durable_authenticated(
            &mut store,
            2,
            capability(),
            request(),
            SIGNED_AT,
            &KEY,
            &mut PanicOnDispatch,
        )
        .unwrap(),
        ServiceHttpDeliveryOutcome::Uncertain
    ));
    assert_eq!(fs::read(target).unwrap(), PENDING_INTENT_BYTES);
}

#[test]
fn oversized_marker_refuses_without_treating_it_as_absence() {
    let temp = TempDirectory::new();
    let directory = platform::hold_directory(temp.path()).unwrap();
    let mut store = OutboundDeliveryStore::new(&directory);
    let name = pending_intent_filename(OutboundCheckpointKind::HttpSession, &identity()).unwrap();
    let bytes = vec![b'x'; super::super::authenticated_intent::MAX_AUTHENTICATED_INTENT_BYTES + 1];
    fs::write(temp.path().join(&name), &bytes).unwrap();
    assert_eq!(
        read_http_intent(&store, &identity(), &KEY),
        Err(HttpIntentReadRefusal::StorageUnavailable)
    );
    assert!(matches!(
        deliver_http_durable_authenticated(
            &mut store,
            2,
            capability(),
            request(),
            SIGNED_AT,
            &KEY,
            &mut PanicOnDispatch,
        )
        .unwrap(),
        ServiceHttpDeliveryOutcome::Uncertain
    ));
    assert_eq!(fs::read(temp.path().join(name)).unwrap(), bytes);
}
