//! Deterministic ARC Zone Model v1 integration evidence.
//!
//! These tests pin the bounded hidden-model semantics of `semaprax::arc_zones`:
//! known-answer trace digests for canonical scenarios (shared fan-out release,
//! cycle rejection at zone exit, escape demotion, nested zones), hostile
//! rejections (foreign-zone handles, double release, unbalanced zone exit),
//! determinism under inventory permutation and repeated execution, and
//! domain-separated canonical JSON serialization. They prove proof data only;
//! no runtime reference counting, allocator, language syntax, compiler change,
//! or backend behavior exists or is claimed.

use semaprax::arc_zones::{
    ArcZoneEvent, ArcZoneStatus, ArcZonesError, ArcZonesModel, FinalizeCause, ObjectSpec, Op,
    ShareableMark, ZoneSpec, ARC_ZONES_MODEL_V1, ARC_ZONES_TRACE_V1,
};
use serde_json::Value;
use sha2::{Digest, Sha256};

const MODEL_DIGEST_DOMAIN: &[u8] = b"semaprax.arc-zones-model-fingerprint.v1\0";

pub const SHARED_FAN_OUT_RELEASE_TRACE_DIGEST: &str =
    "b4d9a89367c410b74b243b2e4c206e334f7a2883161431a53bcc6aee3eece956";
pub const CYCLE_REJECTION_TRACE_DIGEST: &str =
    "c25ca301dadced10c52cdbf6593e8b428524734ff5d1e3234b6255cd5ff09e51";
pub const ESCAPE_DEMOTION_TRACE_DIGEST: &str =
    "a9da55d283c201899b99fe5e5389da682edb86f181485bc5cb6c73e8f36169e7";
pub const NESTED_ZONES_TRACE_DIGEST: &str =
    "f04b2180c74cb364ea6734cd50ff723af21956018c8fa18749a3ec071286833e";

fn sha256_domain(domain: &[u8], bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(domain);
    hasher.update((bytes.len() as u64).to_le_bytes());
    hasher.update(bytes);
    format!("{:x}", semaprax::digest_hex::LowerHex(hasher.finalize()))
}

fn hex_of(bytes: [u8; 32]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

fn drain(run: &mut semaprax::arc_zones::ArcZonesRun<'_>) -> Vec<ArcZoneEvent> {
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

fn event_kinds(run: &semaprax::arc_zones::ArcZonesRun<'_>) -> Vec<&'static str> {
    fn kind(event: &ArcZoneEvent) -> &'static str {
        match event {
            ArcZoneEvent::Constructed { .. } => "constructed",
            ArcZoneEvent::Retained { .. } => "retained",
            ArcZoneEvent::Released { .. } => "released",
            ArcZoneEvent::Linked { .. } => "linked",
            ArcZoneEvent::Unlinked { .. } => "unlinked",
            ArcZoneEvent::EscapedToUnique { .. } => "escaped_to_unique",
            ArcZoneEvent::Finalized { .. } => "finalized",
            ArcZoneEvent::ZoneEntered { .. } => "zone_entered",
            ArcZoneEvent::ZoneExited { .. } => "zone_exited",
            ArcZoneEvent::ZoneRejectedCycle { .. } => "zone_rejected_cycle",
        }
    }
    run.events().iter().map(kind).collect()
}

/// Shared fan-out release: three outstanding handles drain first, then the
/// base reference dies and cascades through payload links in canonical target
/// order before the zone exits clean.
fn fan_out_model() -> ArcZonesModel {
    ArcZonesModel::try_new(
        vec![ZoneSpec::root("r", "main")],
        vec![
            ObjectSpec::new("hub", "r", ShareableMark::NotShareable),
            ObjectSpec::new("left", "r", ShareableMark::NotShareable),
            ObjectSpec::new("right", "r", ShareableMark::NotShareable),
        ],
    )
    .expect("fan-out model is valid")
}

fn fan_out_script() -> Vec<Op> {
    vec![
        Op::enter_zone("r"),
        Op::construct("hub"),
        Op::construct("left"),
        Op::construct("right"),
        Op::link("hub", "left"),
        Op::link("hub", "right"),
        Op::retain("hub"),
        Op::retain("hub"),
        Op::release("hub"),
        Op::release("hub"),
        Op::release("left"),
        Op::release("right"),
        Op::release("hub"),
        Op::exit_zone("r"),
    ]
}

#[test]
fn kat_shared_fan_out_release_cascades_in_canonical_order() {
    let model = fan_out_model();
    let mut run = model.prepare_run(&fan_out_script());
    drain(&mut run);
    assert_eq!(
        event_kinds(&run),
        vec![
            "zone_entered",
            "constructed",
            "constructed",
            "constructed",
            "linked",
            "linked",
            "retained",
            "retained",
            "released",
            "released",
            "released",
            "released",
            "released",
            "finalized",
            "finalized",
            "finalized",
            "zone_exited",
        ]
    );
    let causes: Vec<(&str, FinalizeCause)> = run
        .events()
        .iter()
        .filter_map(|event| match event {
            ArcZoneEvent::Finalized { object, cause } => Some((object.as_str(), *cause)),
            _ => None,
        })
        .collect();
    assert_eq!(
        causes,
        vec![
            ("hub", FinalizeCause::Release),
            ("left", FinalizeCause::Cascade),
            ("right", FinalizeCause::Cascade),
        ]
    );
    assert!(run.is_complete());
    let summary = run.finish().unwrap();
    assert!(matches!(summary.status(), ArcZoneStatus::Complete));
    assert_eq!(
        summary.totals(),
        semaprax::arc_zones::ArcZoneTotals {
            constructed: 3,
            retained: 2,
            released: 5,
            finalized: 3,
            zones_entered: 1,
            zones_exited: 1,
            zones_rejected: 0,
        }
    );
    assert_eq!(
        hex_of(run.trace_digest()),
        SHARED_FAN_OUT_RELEASE_TRACE_DIGEST
    );
}

#[test]
fn kat_cycle_is_rejected_at_zone_exit_with_canonical_witness() {
    let model = ArcZonesModel::try_new(
        vec![ZoneSpec::root("r", "main")],
        vec![
            ObjectSpec::new("m", "r", ShareableMark::NotShareable),
            ObjectSpec::new("n", "r", ShareableMark::NotShareable),
        ],
    )
    .expect("cycle model is valid");
    let script = vec![
        Op::enter_zone("r"),
        Op::construct("m"),
        Op::construct("n"),
        Op::link("m", "n"),
        Op::link("n", "m"),
        Op::exit_zone("r"),
    ];
    let mut run = model.prepare_run(&script);
    drain(&mut run);
    assert!(run.is_rejected());
    assert_eq!(run.rejected_witness().map(|id| id.as_str()), Some("m"));
    assert_eq!(event_kinds(&run).last(), Some(&"zone_rejected_cycle"));
    let summary = run.finish().unwrap();
    assert!(matches!(summary.status(), ArcZoneStatus::Rejected));
    assert_eq!(summary.rejected_witness().map(|id| id.as_str()), Some("m"));
    assert_eq!(hex_of(run.trace_digest()), CYCLE_REJECTION_TRACE_DIGEST);
}

#[test]
fn kat_escape_demotion_rewrites_proven_zone_local_handle_to_unique() {
    let model = ArcZonesModel::try_new(
        vec![ZoneSpec::root("r", "main")],
        vec![ObjectSpec::new("solo", "r", ShareableMark::NotShareable)],
    )
    .expect("escape model is valid");
    let script = vec![
        Op::enter_zone("r"),
        Op::construct("solo"),
        Op::demote("solo"),
        Op::exit_zone("r"),
    ];
    let mut run = model.prepare_run(&script);
    drain(&mut run);
    assert_eq!(
        event_kinds(&run),
        vec![
            "zone_entered",
            "constructed",
            "escaped_to_unique",
            "finalized",
            "zone_exited",
        ]
    );
    assert!(run.is_complete());
    assert_eq!(hex_of(run.trace_digest()), ESCAPE_DEMOTION_TRACE_DIGEST);
}

#[test]
fn kat_nested_zones_finalize_children_before_parents_in_reverse_construction() {
    let model = ArcZonesModel::try_new(
        vec![
            ZoneSpec::root("r", "main"),
            ZoneSpec::child("inner", "r", "worker"),
        ],
        vec![
            ObjectSpec::new("w", "r", ShareableMark::NotShareable),
            ObjectSpec::new("x", "inner", ShareableMark::NotShareable),
            ObjectSpec::new("y", "r", ShareableMark::NotShareable),
            ObjectSpec::new("z", "inner", ShareableMark::Shareable),
        ],
    )
    .expect("nested model is valid");
    let script = vec![
        Op::enter_zone("r"),
        Op::construct("y"),
        Op::enter_zone("inner"),
        Op::construct("x"),
        Op::construct("z"),
        Op::link("z", "x"),
        Op::exit_zone("inner"),
        Op::construct("w"),
        Op::release("y"),
        Op::exit_zone("r"),
    ];
    let mut run = model.prepare_run(&script);
    drain(&mut run);
    assert_eq!(
        event_kinds(&run),
        vec![
            "zone_entered",
            "constructed",
            "zone_entered",
            "constructed",
            "constructed",
            "linked",
            "finalized",
            "finalized",
            "zone_exited",
            "constructed",
            "released",
            "finalized",
            "finalized",
            "zone_exited",
        ]
    );
    assert!(run.is_complete());
    assert_eq!(hex_of(run.trace_digest()), NESTED_ZONES_TRACE_DIGEST);
}

#[test]
fn hostile_foreign_zone_release_and_unbalanced_exit_are_rejected() {
    let model = ArcZonesModel::try_new(
        vec![
            ZoneSpec::root("r", "main"),
            ZoneSpec::child("inner", "r", "worker"),
        ],
        vec![
            ObjectSpec::new("a", "r", ShareableMark::NotShareable),
            ObjectSpec::new("b", "r", ShareableMark::NotShareable),
            ObjectSpec::new("s", "inner", ShareableMark::NotShareable),
        ],
    )
    .expect("hostile model is valid");

    // Releasing a handle from inside another zone fails closed.
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

    // Exiting the outer zone while the inner zone is still open is rejected.
    let mut run = model.prepare_run(&[
        Op::enter_zone("r"),
        Op::enter_zone("inner"),
        Op::exit_zone("r"),
    ]);
    run.step().expect("enter r");
    run.step().expect("enter inner");
    assert_eq!(run.step().unwrap_err(), ArcZonesError::UnbalancedZoneExit);

    // Double release beyond every outstanding reference is rejected while the
    // linked object stays live.
    let mut run = model.prepare_run(&[
        Op::enter_zone("r"),
        Op::construct("a"),
        Op::construct("b"),
        Op::link("b", "a"),
        Op::retain("a"),
        Op::release("a"),
        Op::release("a"),
        Op::release("a"),
    ]);
    for name in [
        "enter",
        "construct a",
        "construct b",
        "link",
        "retain",
        "release handle",
        "release base",
    ] {
        run.step().unwrap_or_else(|error| panic!("{name}: {error}"));
    }
    assert_eq!(run.strong_count("a"), Some(1));
    assert_eq!(run.step().unwrap_err(), ArcZonesError::DoubleRelease);
    assert_eq!(run.strong_count("a"), Some(1));

    // Cross-thread payload sharing without an explicit Shareable annotation is
    // rejected even though both zones are declared.
    let mut run = model.prepare_run(&[
        Op::enter_zone("r"),
        Op::construct("a"),
        Op::enter_zone("inner"),
        Op::construct("s"),
        Op::link("s", "a"),
    ]);
    for _ in 0..4 {
        run.step().expect("prefix ops");
    }
    assert_eq!(
        run.step().unwrap_err(),
        ArcZonesError::SharingWithoutShareable
    );
}

#[test]
fn determinism_survives_inventory_permutation_and_double_execution() {
    let forward = ArcZonesModel::try_new(
        vec![
            ZoneSpec::root("r", "main"),
            ZoneSpec::child("inner", "r", "worker"),
        ],
        vec![
            ObjectSpec::new("w", "r", ShareableMark::NotShareable),
            ObjectSpec::new("x", "inner", ShareableMark::NotShareable),
            ObjectSpec::new("y", "r", ShareableMark::NotShareable),
            ObjectSpec::new("z", "inner", ShareableMark::Shareable),
        ],
    )
    .unwrap();
    let backward = ArcZonesModel::try_new(
        vec![
            ZoneSpec::child("inner", "r", "worker"),
            ZoneSpec::root("r", "main"),
        ],
        vec![
            ObjectSpec::new("z", "inner", ShareableMark::Shareable),
            ObjectSpec::new("y", "r", ShareableMark::NotShareable),
            ObjectSpec::new("x", "inner", ShareableMark::NotShareable),
            ObjectSpec::new("w", "r", ShareableMark::NotShareable),
        ],
    )
    .unwrap();
    assert_eq!(forward.fingerprint(), backward.fingerprint());
    assert_eq!(forward.canonical_json(), backward.canonical_json());

    let script = vec![
        Op::enter_zone("r"),
        Op::enter_zone("inner"),
        Op::construct("x"),
        Op::construct("z"),
        Op::link("z", "x"),
        Op::exit_zone("inner"),
        Op::exit_zone("r"),
    ];
    let mut first = forward.prepare_run(&script);
    let mut second = backward.prepare_run(&script);
    let left = drain(&mut first);
    let right = drain(&mut second);
    assert_eq!(left, right);
    assert_eq!(first.trace_canonical_json(), second.trace_canonical_json());
    assert_eq!(first.trace_digest(), second.trace_digest());

    // A second independent execution of the same model/script pair is
    // byte-identical down to the digest.
    let mut replay = forward.prepare_run(&script);
    drain(&mut replay);
    assert_eq!(replay.events(), first.events());
    assert_eq!(replay.trace_canonical_json(), first.trace_canonical_json());
    assert_eq!(replay.trace_digest(), first.trace_digest());
}

#[test]
fn projections_are_valid_json_and_domain_separated() {
    let model = fan_out_model();
    let parsed_model: Value =
        serde_json::from_str(&model.canonical_json()).expect("model JSON parses");
    assert_eq!(parsed_model["schema"], ARC_ZONES_MODEL_V1);

    let mut run = model.prepare_run(&[Op::enter_zone("r"), Op::exit_zone("r")]);
    run.step().expect("enter");
    let partial: Value =
        serde_json::from_str(&run.trace_canonical_json()).expect("partial trace parses");
    assert_eq!(partial["status"], "running");
    assert_eq!(partial["rejected_witness"], Value::Null);

    run.step().expect("exit");
    let parsed_trace: Value =
        serde_json::from_str(&run.trace_canonical_json()).expect("trace JSON parses");
    assert_eq!(parsed_trace["schema"], ARC_ZONES_TRACE_V1);
    assert_eq!(parsed_trace["status"], "complete");
    assert_eq!(
        parsed_trace["model_fingerprint"],
        sha256_domain(MODEL_DIGEST_DOMAIN, model.canonical_json().as_bytes())
    );
    assert_ne!(
        run.trace_digest(),
        model.fingerprint(),
        "trace and model digests must be domain separated"
    );
}

#[test]
fn shared_fan_out_trace_projection_is_byte_pinned() {
    let model = fan_out_model();
    let mut run = model.prepare_run(&fan_out_script());
    drain(&mut run);
    assert_eq!(
        run.trace_canonical_json(),
        "{\"schema\":\"semaprax.arc-zones-trace.v1\",\"model_fingerprint\":\"80ed553b157d3ebc4a161e3f0fa76c63602c83f6607e9773036142ab483c4d52\",\"status\":\"complete\",\"rejected_witness\":null,\"events\":[{\"kind\":\"zone_entered\",\"zone\":\"r\"},{\"kind\":\"constructed\",\"object\":\"hub\"},{\"kind\":\"constructed\",\"object\":\"left\"},{\"kind\":\"constructed\",\"object\":\"right\"},{\"kind\":\"linked\",\"from\":\"hub\",\"to\":\"left\"},{\"kind\":\"linked\",\"from\":\"hub\",\"to\":\"right\"},{\"kind\":\"retained\",\"object\":\"hub\"},{\"kind\":\"retained\",\"object\":\"hub\"},{\"kind\":\"released\",\"object\":\"hub\"},{\"kind\":\"released\",\"object\":\"hub\"},{\"kind\":\"released\",\"object\":\"left\"},{\"kind\":\"released\",\"object\":\"right\"},{\"kind\":\"released\",\"object\":\"hub\"},{\"kind\":\"finalized\",\"object\":\"hub\",\"cause\":\"release\"},{\"kind\":\"finalized\",\"object\":\"left\",\"cause\":\"cascade\"},{\"kind\":\"finalized\",\"object\":\"right\",\"cause\":\"cascade\"},{\"kind\":\"zone_exited\",\"zone\":\"r\"}]}"
    );
}
