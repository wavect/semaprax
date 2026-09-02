use super::*;

const MAIN: &str = "main";

fn two_zone_model() -> ArcZonesModel {
    ArcZonesModel::try_new(
        vec![
            ZoneSpec::root("r", MAIN),
            ZoneSpec::child("inner", "r", "worker"),
        ],
        vec![
            ObjectSpec::new("a", "r", ShareableMark::NotShareable),
            ObjectSpec::new("s", "inner", ShareableMark::NotShareable),
            ObjectSpec::new("x", "r", ShareableMark::Shareable),
        ],
    )
    .expect("model is valid")
}

fn drain(run: &mut ArcZonesRun<'_>) -> Vec<ArcZoneEvent> {
    let mut produced = Vec::new();
    loop {
        let batch = run.step().expect("script steps are valid");
        if batch.is_empty() {
            break;
        }
        produced.extend(batch);
    }
    produced
}

#[test]
fn constructor_rejects_structural_ambiguity() {
    for name in ["", "r\0x"] {
        assert_eq!(
            ArcZonesModel::try_new(vec![ZoneSpec::root(name, MAIN)], Vec::new()),
            Err(ArcZonesError::InvalidIdentity)
        );
    }
    assert_eq!(
        ArcZonesModel::try_new(Vec::new(), Vec::new()),
        Err(ArcZonesError::MissingRoot)
    );
    assert_eq!(
        ArcZonesModel::try_new(
            vec![ZoneSpec::root("r", MAIN), ZoneSpec::root("q", MAIN)],
            Vec::new()
        ),
        Err(ArcZonesError::MultipleRoots)
    );
    assert_eq!(
        ArcZonesModel::try_new(
            vec![ZoneSpec::root("r", MAIN), ZoneSpec::root("r", MAIN)],
            Vec::new()
        ),
        Err(ArcZonesError::DuplicateZone)
    );
    assert_eq!(
        ArcZonesModel::try_new(
            vec![ZoneSpec::root("r", MAIN)],
            vec![
                ObjectSpec::new("a", "r", ShareableMark::NotShareable),
                ObjectSpec::new("a", "r", ShareableMark::NotShareable)
            ]
        ),
        Err(ArcZonesError::DuplicateObject)
    );
    assert_eq!(
        ArcZonesModel::try_new(
            vec![ZoneSpec::root("r", MAIN)],
            vec![ObjectSpec::new("a", "ghost", ShareableMark::NotShareable)]
        ),
        Err(ArcZonesError::UnknownZone)
    );
    assert_eq!(
        ArcZonesModel::try_new(
            vec![
                ZoneSpec::root("r", MAIN),
                ZoneSpec::child("p", "q", MAIN),
                ZoneSpec::child("q", "p", MAIN)
            ],
            Vec::new()
        ),
        Err(ArcZonesError::ZoneCycle)
    );
}

#[test]
fn hostile_operations_fail_closed() {
    let model = two_zone_model();
    // Foreign-zone handle release: `a` is homed in the root while the
    // innermost open zone is `inner`.
    let mut run = model.prepare_run(&[
        Op::enter_zone("r"),
        Op::construct("a"),
        Op::enter_zone("inner"),
        Op::release("a"),
    ]);
    run.step().expect("enter r");
    run.step().expect("construct a");
    run.step().expect("enter inner");
    assert_eq!(run.step().unwrap_err(), ArcZonesError::ForeignZoneObject);

    // Double release: `a` stays alive only through x's payload link after
    // its handle and base reference were each released once; a third
    // release has nothing left to give up.
    let mut replay = model.prepare_run(&[
        Op::enter_zone("r"),
        Op::construct("a"),
        Op::construct("x"),
        Op::link("x", "a"),
        Op::retain("a"),
        Op::release("a"),
        Op::release("a"),
        Op::release("a"),
    ]);
    replay.step().expect("enter");
    replay.step().expect("construct a");
    replay.step().expect("construct x");
    replay.step().expect("link");
    replay.step().expect("retain");
    replay.step().expect("release handle");
    replay.step().expect("release base");
    assert_eq!(replay.strong_count("a"), Some(1));
    assert_eq!(replay.step().unwrap_err(), ArcZonesError::DoubleRelease);
    assert_eq!(replay.strong_count("a"), Some(1));

    // Unbalanced exit: leaving the root while a child is still open.
    let mut run = model.prepare_run(&[
        Op::enter_zone("r"),
        Op::enter_zone("inner"),
        Op::exit_zone("r"),
    ]);
    run.step().expect("enter r");
    run.step().expect("enter inner");
    assert_eq!(run.step().unwrap_err(), ArcZonesError::UnbalancedZoneExit);

    // Cross-thread sharing without an explicit Shareable annotation:
    // linking worker-homed `s` to main-thread root-homed non-shareable
    // `a` would share `a` across zones and threads.
    let mut run = model.prepare_run(&[
        Op::enter_zone("r"),
        Op::construct("a"),
        Op::enter_zone("inner"),
        Op::construct("s"),
        Op::link("s", "a"),
    ]);
    for step_name in ["enter r", "construct a", "enter inner", "construct s"] {
        run.step()
            .unwrap_or_else(|error| panic!("{step_name}: {error}"));
    }
    assert_eq!(
        run.step().unwrap_err(),
        ArcZonesError::SharingWithoutShareable
    );

    // Shared use after demotion fails closed.
    let mut run = model.prepare_run(&[
        Op::enter_zone("r"),
        Op::construct("a"),
        Op::demote("a"),
        Op::retain("a"),
    ]);
    run.step().expect("enter");
    run.step().expect("construct");
    run.step().expect("demote");
    assert_eq!(run.step().unwrap_err(), ArcZonesError::SharedUseOfUnique);
}

#[test]
fn link_and_unlink_hostility_and_cascade_order() {
    let model = ArcZonesModel::try_new(
        vec![ZoneSpec::root("r", MAIN)],
        vec![
            ObjectSpec::new("b", "r", ShareableMark::NotShareable),
            ObjectSpec::new("c", "r", ShareableMark::NotShareable),
            ObjectSpec::new("d", "r", ShareableMark::NotShareable),
        ],
    )
    .unwrap();
    let mut run = model.prepare_run(&[
        Op::enter_zone("r"),
        Op::construct("b"),
        Op::construct("c"),
        Op::construct("d"),
        Op::link("d", "c"),
        Op::link("d", "c"),
        Op::unlink("d", "b"),
    ]);
    run.step().expect("enter");
    run.step().expect("construct b");
    run.step().expect("construct c");
    run.step().expect("construct d");
    run.step().expect("link d->c");
    assert_eq!(run.step().unwrap_err(), ArcZonesError::DuplicateLiveLink);
    let mut replay = model.prepare_run(&[
        Op::enter_zone("r"),
        Op::construct("b"),
        Op::construct("c"),
        Op::construct("d"),
        Op::link("d", "c"),
        Op::unlink("d", "b"),
    ]);
    for _ in 0..5 {
        replay.step().expect("prefix ops");
    }
    assert_eq!(replay.step().unwrap_err(), ArcZonesError::UnknownLiveLink);

    // Releasing the sole owner cascades through outgoing links in exact
    // canonical target order once the children gave up their own base
    // references.
    let mut run = model.prepare_run(&[
        Op::enter_zone("r"),
        Op::construct("d"),
        Op::construct("b"),
        Op::construct("c"),
        Op::link("d", "c"),
        Op::link("d", "b"),
        Op::release("b"),
        Op::release("c"),
        Op::release("d"),
    ]);
    drain(&mut run);
    let finalized: Vec<&str> = run
        .events()
        .iter()
        .filter_map(|event| match event {
            ArcZoneEvent::Finalized { object, cause } => Some((object.as_str(), *cause)),
            _ => None,
        })
        .map(|(id, _)| id)
        .collect();
    assert_eq!(finalized, vec!["d", "b", "c"]);
}

#[test]
fn self_loop_is_rejected_at_zone_exit_with_canonical_witness() {
    let model = ArcZonesModel::try_new(
        vec![ZoneSpec::root("r", MAIN)],
        vec![ObjectSpec::new("m", "r", ShareableMark::NotShareable)],
    )
    .unwrap();
    let mut run = model.prepare_run(&[
        Op::enter_zone("r"),
        Op::construct("m"),
        Op::link("m", "m"),
        Op::exit_zone("r"),
    ]);
    drain(&mut run);
    assert!(run.is_rejected());
    assert_eq!(run.rejected_witness().map(DeclarationId::as_str), Some("m"));
    assert!(matches!(
        run.finish().unwrap(),
        ArcZoneSummary {
            status: ArcZoneStatus::Rejected,
            ..
        }
    ));
}

#[test]
fn determinism_survives_inventory_permutation() {
    let forward = ArcZonesModel::try_new(
        vec![ZoneSpec::root("r", MAIN)],
        vec![
            ObjectSpec::new("a", "r", ShareableMark::NotShareable),
            ObjectSpec::new("b", "r", ShareableMark::NotShareable),
        ],
    )
    .unwrap();
    let backward = ArcZonesModel::try_new(
        vec![ZoneSpec::root("r", MAIN)],
        vec![
            ObjectSpec::new("b", "r", ShareableMark::NotShareable),
            ObjectSpec::new("a", "r", ShareableMark::NotShareable),
        ],
    )
    .unwrap();
    assert_eq!(forward.fingerprint(), backward.fingerprint());
    assert_eq!(forward.canonical_json(), backward.canonical_json());

    let script: Vec<Op> = vec![Op::enter_zone("r"), Op::construct("a"), Op::exit_zone("r")];
    let mut left = forward.prepare_run(&script);
    let mut right = backward.prepare_run(&script);
    drain(&mut left);
    drain(&mut right);
    assert_eq!(left.events(), right.events());
    assert_eq!(left.trace_digest(), right.trace_digest());
}

#[test]
fn strong_count_tracks_fan_out_and_demotion_preconditions() {
    let model = two_zone_model();
    let mut run = model.prepare_run(&[
        Op::enter_zone("r"),
        Op::construct("a"),
        Op::retain("a"),
        Op::retain("a"),
    ]);
    run.step().expect("enter");
    run.step().expect("construct");
    run.step().expect("retain");
    run.step().expect("retain");
    assert_eq!(run.strong_count("a"), Some(3));
    assert!(!run.is_complete());

    let mut run = model.prepare_run(&[Op::enter_zone("r"), Op::construct("x")]);
    run.step().expect("enter");
    run.step().expect("construct");
    assert_eq!(run.strong_count("x"), Some(1));
    // Demotion outside any open zone cannot even address an object.
    let mut orphan = model.prepare_run(&[Op::demote("x")]);
    assert_eq!(
        orphan.step().unwrap_err(),
        ArcZonesError::DeadOrUnconstructedObject
    );
}

#[test]
fn projections_are_valid_json_and_domain_separated() {
    let model = two_zone_model();
    let parsed_model: serde_json::Value =
        serde_json::from_str(&model.canonical_json()).expect("model JSON parses");
    assert_eq!(parsed_model["schema"], ARC_ZONES_MODEL_V1);

    let mut run = model.prepare_run(&[Op::enter_zone("r"), Op::exit_zone("r")]);
    run.step().expect("enter");
    let partial_json = run.trace_canonical_json();
    let parsed_partial: serde_json::Value =
        serde_json::from_str(&partial_json).expect("partial trace parses");
    assert_eq!(parsed_partial["status"], "running");

    run.step().expect("exit");
    let parsed_trace: serde_json::Value =
        serde_json::from_str(&run.trace_canonical_json()).expect("trace JSON parses");
    assert_eq!(parsed_trace["schema"], ARC_ZONES_TRACE_V1);
    assert_eq!(parsed_trace["status"], "complete");
    assert_ne!(
        run.trace_digest(),
        model.fingerprint(),
        "trace and model digests must be domain separated"
    );
}
