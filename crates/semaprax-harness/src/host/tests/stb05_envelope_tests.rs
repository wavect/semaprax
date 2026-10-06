//! STB-05 (#548): the JSON-RPC response envelope (exact `"2.0"` version and
//! a structured error object) is validated before anything is delivered.

use super::stb_fixture::*;
use super::*;
use crate::host::launch::Prepared;
use crate::host::process::{Closed, Delivery, Proc, RealSys};
use std::sync::Arc;

/// A real child that reads the host's first request (id 1) and answers
/// with exactly `resp` on stdout, then stays alive until killed.
fn answer_with(fx: &Fx, resp: &Value) -> Delivery {
    let p = Prepared {
        program: PathBuf::from("/bin/sh"),
        args: vec![
            "-c".into(),
            "read l; printf '%s\\n' \"$RESP\"; exec sleep 300".into(),
        ],
        env: [
            ("PATH".to_string(), "/bin:/usr/bin".to_string()),
            ("RESP".to_string(), resp.to_string()),
        ]
        .into(),
        cwd: fx.root.clone(),
        mode: IsolationMode::Subprocess,
    };
    let proc = Proc::spawn_with(&p, 1 << 20, 4096, Arc::new(RealSys)).unwrap();
    let rx = proc.request("harness/invoke", json!({})).unwrap();
    rx.recv_timeout(Duration::from_secs(10)).expect("delivery")
}

fn violation(d: &Delivery) -> Option<&'static str> {
    match d {
        Delivery::Closed(Closed::Violation(v)) => Some(v.code),
        _ => None,
    }
}

#[test]
fn version_must_be_exactly_the_string_2_0() {
    let fx = fixture();
    let inner = json!({"protocol": "semaprax.harness-rpc.v1", "accepted": []});
    for bad in [None, Some(json!("1.0")), Some(Value::Null), Some(json!(2))] {
        let mut resp = json!({"id": 1, "result": inner});
        if let Some(v) = &bad {
            resp["jsonrpc"] = v.clone();
        }
        let d = answer_with(&fx, &resp);
        assert_eq!(violation(&d), Some("SPX-HPC009"), "version {bad:?}");
    }
    let ok = answer_with(&fx, &json!({"jsonrpc": "2.0", "id": 1, "result": inner}));
    assert!(matches!(&ok, Delivery::Result(v) if *v == inner));
}

#[test]
fn malformed_error_objects_are_protocol_failures() {
    let fx = fixture();
    for e in [
        Value::Null,
        json!("boom"),
        json!({}),
        json!({"code": 1.5, "message": "m"}),
        json!({"code": "-32000", "message": "m"}),
        json!({"code": -32000, "message": 5}),
        json!({"code": -32000}),
    ] {
        let d = answer_with(&fx, &json!({"jsonrpc": "2.0", "id": 1, "error": e}));
        assert_eq!(violation(&d), Some("SPX-HPC009"), "error {e}");
    }
    // Valid errors (negative codes, optional data) keep their bounded message.
    let ok = answer_with(
        &fx,
        &json!({"jsonrpc": "2.0", "id": 1, "error": {"code": -32000, "message": "boom"}}),
    );
    assert!(matches!(&ok, Delivery::Error(m) if m == "boom"));
    let long = "x".repeat(500);
    let ok = answer_with(
        &fx,
        &json!({"jsonrpc": "2.0", "id": 1, "error": {"code": -32001, "message": long, "data": {"k": [1]}}}),
    );
    assert!(matches!(&ok, Delivery::Error(m) if m.len() == 200));
}

#[test]
fn existing_envelope_rejections_still_hold() {
    let fx = fixture();
    let cases = [
        (
            json!({"jsonrpc": "2.0", "id": 7, "result": {}}),
            "SPX-HPC010",
        ),
        (
            json!({"jsonrpc": "2.0", "id": 1, "result": {}, "error": {"code": 1, "message": "m"}}),
            "SPX-HPC009",
        ),
        (
            json!({"jsonrpc": "2.0", "id": "1", "result": {}}),
            "SPX-HPC009",
        ),
        (
            json!({"jsonrpc": "2.0", "id": 9, "method": "host/readFile", "params": {}}),
            "SPX-HPC011",
        ),
    ];
    for (resp, code) in cases {
        let d = answer_with(&fx, &resp);
        assert_eq!(violation(&d), Some(code), "{resp}");
    }
}

fn wrapped(fx: &Fx, mode: &str, log: &Path) -> LaunchSpec {
    stb_spec(
        fx,
        &[
            ("STB_ENVELOPE", mode),
            ("STB_INVOKE_LOG", log.to_str().unwrap()),
        ],
        |_| {},
    )
}

#[test]
fn real_adapter_malformed_initialize_wrapper_is_quarantined() {
    let fx = fixture();
    let log = fx.root.join("invoke-log");
    let m = mgr(|_| {});
    let h = m
        .prepare(PROJECT, wrapped(&fx, "init-version", &log))
        .unwrap();
    let out = h.invoke(
        &decide("i1"),
        InvocationClass::SafeRead,
        &CancelToken::new(),
    );
    assert!(
        matches!(&out, Outcome::Quarantined(d) if d.code == "SPX-HPC009"),
        "{out:?}"
    );
    assert!(matches!(h.state(), AdapterState::Quarantined(_)));
    assert_eq!((h.invoke_frames_queued(), lines(&log)), (0, 0));
}

#[test]
fn real_adapter_malformed_invocation_wrappers_are_not_success() {
    for mode in ["invoke-no-version", "invoke-bad-error"] {
        let fx = fixture();
        let log = fx.root.join("invoke-log");
        let m = mgr(|_| {});
        let h = m.prepare(PROJECT, wrapped(&fx, mode, &log)).unwrap();
        let out = h.invoke(
            &decide("i1"),
            InvocationClass::SideEffecting,
            &CancelToken::new(),
        );
        assert!(
            matches!(&out, Outcome::Quarantined(d) if d.code == "SPX-HPC009"),
            "{mode}: {out:?}"
        );
        assert!(matches!(h.state(), AdapterState::Quarantined(_)), "{mode}");
        assert_eq!((h.invoke_frames_queued(), lines(&log)), (1, 1), "{mode}");
        // The quarantine sticks: nothing is replayed.
        let again = h.invoke(
            &decide("i2"),
            InvocationClass::SideEffecting,
            &CancelToken::new(),
        );
        assert!(matches!(again, Outcome::Quarantined(_)), "{mode}");
        assert_eq!(h.invoke_frames_queued(), 1, "{mode}");
    }
    // Positive control: the same fixture with a valid envelope succeeds.
    let fx = fixture();
    let log = fx.root.join("invoke-log");
    let m = mgr(|_| {});
    let h = m.prepare(PROJECT, wrapped(&fx, "", &log)).unwrap();
    let ok = h.invoke(
        &decide("i1"),
        InvocationClass::SideEffecting,
        &CancelToken::new(),
    );
    assert!(matches!(ok, Outcome::Completed(_)), "{ok:?}");
}
