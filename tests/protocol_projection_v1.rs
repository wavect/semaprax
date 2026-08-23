//! Executable evidence for Protocol Projection v1 (`semaprax.protocol.v1`).
//!
//! Pins the canonical golden envelope digest, proves determinism and
//! canonical source round-trips, verifies digest tamper rejection including
//! forged-but-re-signed envelopes caught by closed replay, exercises the
//! fail-closed parser gates (`SPX-P120`-`SPX-P123`) and signature-resolution
//! diagnostics (`SPX-Q104`, `SPX-Q105`), and asserts the program-graph schema stays protocol-free (v15 deferred),
//! and guards that protocol-free programs keep their exact pre-existing
//! graph schema. No conformance admission, dispatch lowering, backend
//! change, or target execution is involved.

use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicUsize, Ordering};

use semaprax::protocol_check::{self, ProtocolCheckOptions, VerifiedMethod};
use semaprax::{diagnostic, graph};

static COUNTER: AtomicUsize = AtomicUsize::new(0);

const PAYLOAD_DIGEST_DOMAIN: &[u8] = b"semaprax.protocol.payload.v1\0";

const FIXTURE_PATH: &str = "tests/fixtures/protocol_projection_v1.spx";

/// The canonical protocol fixture, emitted through its relative repository
/// path so every digest is machine-independent.
const FIXTURE_SOURCE: &str = r#"module demo.proto;

@id("demo.geometry.point")
record Point { @id("demo.geometry.point.x") x: i64, }

@id("demo.shape")
protocol Shape {
  @id("demo.shape.area") fn area(self: Shape) -> i64;
  fn label(self: Self) -> bool;
}

@id("demo.main")
fn main() -> i64 { Point { x: 2 }.x }
"#;

fn write_temp(source: &str) -> PathBuf {
    let path = std::env::temp_dir().join(format!(
        "semaprax-protocol-projection-{}-{}.spx",
        std::process::id(),
        COUNTER.fetch_add(1, Ordering::SeqCst)
    ));
    std::fs::write(&path, source).unwrap();
    path
}

fn cleanup(path: &Path) {
    let _ = std::fs::remove_file(path);
}

fn cli(args: &[&str]) -> (i32, String, String) {
    let output = Command::new(env!("CARGO_BIN_EXE_semaprax"))
        .args(args)
        .output()
        .expect("semaprax binary");
    (
        output.status.code().unwrap_or(-1),
        String::from_utf8_lossy(&output.stdout).into_owned(),
        String::from_utf8_lossy(&output.stderr).into_owned(),
    )
}

fn sha256_hex(bytes: &[u8]) -> String {
    use sha2::Digest as _;
    let mut hasher = sha2::Sha256::new();
    hasher.update(bytes);
    format!(
        "sha256:{:x}",
        semaprax::digest_hex::LowerHex(hasher.finalize())
    )
}

fn payload_digest(payload: &str) -> String {
    use sha2::Digest as _;
    let mut hasher = sha2::Sha256::new();
    hasher.update(PAYLOAD_DIGEST_DOMAIN);
    hasher.update((payload.len() as u64).to_le_bytes());
    hasher.update(payload.as_bytes());
    format!(
        "sha256:{:x}",
        semaprax::digest_hex::LowerHex(hasher.finalize())
    )
}

/// Re-mints the outer digest around a tampered envelope's exact payload
/// bytes so replay must rely on its closed derivation rules rather than the
/// digest alone.
fn remint_digest(tampered_envelope: &str) -> String {
    let payload_key = "\"payload\":";
    let payload_offset = tampered_envelope
        .find(payload_key)
        .expect("tampered envelope keeps its payload member")
        + payload_key.len();
    let payload = &tampered_envelope[payload_offset..tampered_envelope.len() - 1];
    let (prefix, _) = tampered_envelope
        .split_once("\"digest\":")
        .expect("digest member");
    format!(
        "{prefix}\"digest\":{},\"bytes\":{},\"payload\":{}}}",
        diagnostic::quote_json(&payload_digest(payload)),
        payload.len(),
        payload
    )
}

/// Golden envelope digest over the exact library bytes for the canonical
/// protocol fixture.
#[test]
fn golden_fixture_envelope_digest_is_pinned() {
    let options = ProtocolCheckOptions::default();
    let envelope = protocol_check::generate(Path::new(FIXTURE_PATH), &options).expect("envelope");
    assert!(envelope.contains("\"schema\":\"semaprax.protocol.v1\""));
    assert!(
        envelope.contains("\"module\":\"demo.proto\",\"protocols_total\":1,\"methods_total\":2")
    );
    assert!(envelope.contains(
        "\"conformance\":{\"admitted\":0,\"candidates_considered\":1,\
\"closed_reason\":\"no_impl_declarations_in_v1\"},\"conformances\":[]"
    ));
    assert_eq!(
        sha256_hex(envelope.as_bytes()),
        "sha256:23fed66596023ce4a6fba82ddd34feda63ca34c625ce23db685b9f33dd84ca9f"
    );
}

#[test]
fn generation_is_deterministic_and_replayable() {
    let options = ProtocolCheckOptions::default();
    let path = write_temp(FIXTURE_SOURCE);
    let first = protocol_check::generate(&path, &options).unwrap();
    let second = protocol_check::generate(&path, &options).unwrap();
    assert_eq!(first, second);
    cleanup(&path);

    // The relative-path fixture replays to identical structure.
    let fixture_envelope = protocol_check::generate(Path::new(FIXTURE_PATH), &options).unwrap();
    let verified = protocol_check::verify_envelope(&fixture_envelope).expect("replays");
    assert_eq!(verified.protocols.len(), 1);
    let shape = &verified.protocols[0];
    assert_eq!(shape.stable_id, "demo.shape");
    assert_eq!(shape.name, "Shape");
    assert_eq!(
        shape.methods,
        vec![
            // Canonical bytewise stable-id order: `auto:` sorts before
            // `demo.`.
            VerifiedMethod {
                stable_id: "auto:method:demo.shape.label".to_owned(),
                name: "label".to_owned(),
            },
            VerifiedMethod {
                stable_id: "demo.shape.area".to_owned(),
                name: "area".to_owned(),
            },
        ]
    );
}

#[test]
fn canonical_formatting_round_trips_protocols() {
    let path = write_temp(FIXTURE_SOURCE);
    // Canonicalize once, then require fmt --check to accept the result.
    let (status, _, stderr) = cli(&["fmt", path.to_str().unwrap()]);
    assert_eq!(status, 0, "fmt failed: {stderr}");
    let (status, _, stderr) = cli(&["fmt", path.to_str().unwrap(), "--check"]);
    assert_eq!(
        status, 0,
        "canonical protocols must pass fmt --check: {stderr}"
    );

    // The canonical form re-parses to an identical projection envelope.
    let options = ProtocolCheckOptions::default();
    let before = protocol_check::generate(&path, &options).unwrap();
    let (status, _, stderr) = cli(&["fmt", path.to_str().unwrap()]);
    assert_eq!(status, 0, "second fmt failed: {stderr}");
    let after = protocol_check::generate(&path, &options).unwrap();
    assert_eq!(before, after, "canonical round trip must be stable");
    cleanup(&path);
}

#[test]
fn digest_tampering_fails_closed() {
    let path = write_temp(FIXTURE_SOURCE);
    let envelope = protocol_check::generate(&path, &ProtocolCheckOptions::default()).unwrap();

    // Flip one digest character in place (no unsafe).
    let position = envelope.find("\"digest\":\"sha256:").unwrap() + "\"digest\":\"sha256:".len();
    let flipped = if envelope.as_bytes()[position] == b'0' {
        '1'
    } else {
        '0'
    };
    let tampered = format!(
        "{}{}{}",
        &envelope[..position],
        flipped,
        &envelope[position + 1..]
    );
    let error = protocol_check::verify_envelope(&tampered).unwrap_err();
    assert_eq!(error.code, "SPX-Q103");

    // Forged-but-re-signed mutations still fail on closed replay.
    let forged = remint_digest(&envelope.replace(
        "\"closed_reason\":\"no_impl_declarations_in_v1\"",
        "\"closed_reason\":\"records_auto_conform\"",
    ));
    let error = protocol_check::verify_envelope(&forged).unwrap_err();
    assert_eq!(error.code, "SPX-Q103");

    // Out-of-vocabulary identity origin with a consistent persistence flag
    // is likewise caught by closed replay, not by the digest.
    let forged_origin = remint_digest(&envelope.replace(
        "\"identity_origin\":\"automatic\",\"persistent\":false",
        "\"identity_origin\":\"compiler_owned\",\"persistent\":true",
    ));
    let error = protocol_check::verify_envelope(&forged_origin).unwrap_err();
    assert_eq!(error.code, "SPX-Q103");
    cleanup(&path);
}

#[test]
fn budget_exhaustion_refuses_to_truncate() {
    let path = write_temp(FIXTURE_SOURCE);
    // The smallest legal budget cannot hold the full envelope; generation
    // must fail closed instead of truncating.
    let tiny = ProtocolCheckOptions::new(graph::MIN_AGENT_CONTEXT_BYTES).unwrap();
    let errors = protocol_check::generate(&path, &tiny).expect_err("budget exhaustion");
    assert!(errors.iter().any(|item| item.code == "SPX-Q102"));
    cleanup(&path);
}

#[test]
fn parser_rejects_duplicate_protocol_structures_fail_closed() {
    let duplicate_name = r#"module t;
@id("t.main")
fn main() -> i64 { 0 }
@id("t.a")
protocol A { @id("t.a.f") fn f(self: A) -> i64; }
@id("t.a2")
protocol A { @id("t.b.f") fn f(self: A) -> i64; }
"#;
    let error = semaprax::parse(duplicate_name, Path::new("probe.spx")).unwrap_err();
    assert_eq!(error.code, "SPX-P120");

    let duplicate_method = r#"module t;
@id("t.main")
fn main() -> i64 { 0 }
@id("t.a")
protocol A {
  @id("t.a.f") fn f(self: A) -> i64;
  fn f(self: A) -> bool;
}
"#;
    let error = semaprax::parse(duplicate_method, Path::new("probe.spx")).unwrap_err();
    assert_eq!(error.code, "SPX-P121");

    let duplicate_identity = r#"module t;
@id("t.main")
fn main() -> i64 { 0 }
@id("t.same")
protocol A { @id("t.same") fn f(self: A) -> i64; }
"#;
    let error = semaprax::parse(duplicate_identity, Path::new("probe.spx")).unwrap_err();
    assert_eq!(error.code, "SPX-P122");

    let empty_protocol = r#"module t;
@id("t.main")
fn main() -> i64 { 0 }
@id("t.empty")
protocol Empty { }
"#;
    let error = semaprax::parse(empty_protocol, Path::new("probe.spx")).unwrap_err();
    assert_eq!(error.code, "SPX-P123");
}

#[test]
fn signatures_must_resolve_under_the_closed_rules() {
    let cases = [
        // Unknown named type.
        r#"module t;
@id("t.main")
fn main() -> i64 { 0 }
@id("t.p")
protocol P { @id("t.p.f") fn f(self: P, other: Missing) -> i64; }
"#,
        // Missing receiver.
        r#"module t;
@id("t.main")
fn main() -> i64 { 0 }
@id("t.p")
protocol P { @id("t.p.f") fn f() -> i64; }
"#,
        // Receiver typed as neither Self nor the protocol name.
        r#"module t;
@id("t.main")
fn main() -> i64 { 0 }
@id("t.p")
protocol P { @id("t.p.f") fn f(self: Q) -> i64; }
"#,
        // `Self` outside receiver position.
        r#"module t;
@id("t.main")
fn main() -> i64 { 0 }
@id("t.p")
protocol P { @id("t.p.f") fn f(self: P, other: Self) -> i64; }
"#,
        // Generic arguments are closed in v1.
        r#"module t;
@id("t.main")
fn main() -> i64 { 0 }
@id("t.p")
protocol P { @id("t.p.f") fn f(self: P) -> Wrapper<i64>; }
"#,
    ];
    for source in cases {
        let path = write_temp(source);
        let errors = protocol_check::generate(&path, &ProtocolCheckOptions::default())
            .expect_err("must fail closed");
        cleanup(&path);
        assert!(
            errors.iter().any(|item| item.code == "SPX-Q104"),
            "expected SPX-Q104 among {:?}",
            errors.iter().map(|item| item.code).collect::<Vec<_>>()
        );
    }
}

#[test]
fn stable_id_collisions_involving_protocols_fail_closed() {
    let collides_with_function = r#"module t;
@id("t.clash")
fn main() -> i64 { 0 }
@id("t.clash")
protocol P { @id("t.p.f") fn f(self: P) -> i64; }
"#;
    let path = write_temp(collides_with_function);
    let errors = protocol_check::generate(&path, &ProtocolCheckOptions::default())
        .expect_err("must fail closed");
    cleanup(&path);
    assert!(errors.iter().any(|item| item.code == "SPX-Q105"));

    let collides_with_record_field = r#"module t;
@id("t.rec")
record Rec { @id("t.rec.x") x: i64, }
@id("t.main")
fn main() -> i64 { Rec { x: 1 }.x }
@id("t.rec.x")
protocol P { @id("t.p.f") fn f(self: P) -> i64; }
"#;
    let path = write_temp(collides_with_record_field);
    let errors = protocol_check::generate(&path, &ProtocolCheckOptions::default())
        .expect_err("must fail closed");
    cleanup(&path);
    assert!(errors.iter().any(|item| item.code == "SPX-Q105"));
}

#[test]
fn cli_exit_codes_follow_generation_outcomes() {
    let good = write_temp(FIXTURE_SOURCE);
    let (status, stdout, _) = cli(&["protocol-check", good.to_str().unwrap()]);
    assert_eq!(status, 0);
    assert!(stdout.contains("\"schema\":\"semaprax.protocol.v1\""));
    assert!(!stdout.contains("\"kind\":\"protocol\""));
    cleanup(&good);

    let bad = write_temp(
        "module t;\n@id(\"t.main\")\nfn main() -> i64 { 0 }\n@id(\"t.p\")\nprotocol P { @id(\"t.p.f\") fn f(self: P, other: Missing) -> i64; }\n",
    );
    let (status, _, stderr) = cli(&["protocol-check", bad.to_str().unwrap()]);
    assert_eq!(status, 1);
    assert!(stderr.contains("SPX-Q104"));
    cleanup(&bad);

    let (status, _, stderr) = cli(&["protocol-check"]);
    assert_eq!(status, 2);
    assert!(stderr.contains("missing file path"));

    let (_, _, stderr) = cli(&["protocol-check", "missing.spx", "--wat"]);
    assert!(stderr.contains("unknown protocol-check option"));
}

#[test]
fn graph_keeps_protocol_free_schema_until_the_v15_tranche_lands() {
    // Protocol Projection v1 is deliberately front-end only: the program
    // graph schema lattice (v10-v14) is unchanged and protocol programs keep
    // their pre-protocol schema until the dedicated v15 tranche lands. The
    // envelope carries the full protocol inventory instead.
    let fixture = Path::new(FIXTURE_PATH);
    let (status, stdout, _) = cli(&["graph", fixture.to_str().unwrap()]);
    assert_eq!(status, 0);
    assert!(!stdout.contains("semaprax.graph.v15"));
    assert!(!stdout.contains("\"kind\":\"protocol\""));

    let without = write_temp("module t;\n@id(\"t.main\")\nfn main() -> i64 { 42 }\n");
    let (status, stdout, _) = cli(&["graph", without.to_str().unwrap()]);
    assert_eq!(status, 0);
    assert!(stdout.contains("\"schema\":\"semaprax.graph.v10\""));
    assert!(!stdout.contains("\"kind\":\"protocol\""));
    cleanup(&without);
}
