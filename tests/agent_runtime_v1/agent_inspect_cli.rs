//! Executable evidence for `semaprax agent inspect` and the agent graph bundle
//! route of `semaprax verify`: the CLI prints exactly the library compiler's
//! bytes, the bundle receipt names the pinned digests, and tampering or a
//! malformed grammar fails closed.

use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::sync::atomic::{AtomicUsize, Ordering};

use semaprax::agent_definition::compile_agent_definition;

use super::agent_definition_v1::definition;
use super::{profile, task};

static SERIAL: AtomicUsize = AtomicUsize::new(0);

struct Bundle {
    root: PathBuf,
    definition: PathBuf,
    profile: PathBuf,
    graph: PathBuf,
}

impl Bundle {
    fn new() -> Self {
        let root = std::env::temp_dir().join(format!(
            "semaprax-agent-inspect-cli-{}-{}",
            std::process::id(),
            SERIAL.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir_all(&root).unwrap();
        let profile_source = profile();
        let definition_source = definition(&profile_source);
        let compiled = compile_agent_definition(&definition_source).unwrap();
        let definition = root.join("agent.json");
        let profile = root.join("profile.json");
        let graph = root.join("graph.json");
        std::fs::write(&definition, &definition_source).unwrap();
        std::fs::write(&profile, &profile_source).unwrap();
        std::fs::write(&graph, compiled.graph().canonical_json()).unwrap();
        Self {
            root,
            definition,
            profile,
            graph,
        }
    }
}

impl Drop for Bundle {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.root);
    }
}

fn cli(arguments: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_semaprax"))
        .args(arguments)
        .output()
        .unwrap()
}

fn text(path: &Path) -> &str {
    path.to_str().unwrap()
}

#[test]
fn inspect_prints_the_compiler_graph_and_profile_bytes() {
    let bundle = Bundle::new();
    let compiled =
        compile_agent_definition(&std::fs::read_to_string(&bundle.definition).unwrap()).unwrap();

    let graph = cli(&["agent", "inspect", text(&bundle.definition)]);
    assert!(graph.status.success());
    assert!(graph.stderr.is_empty());
    assert_eq!(graph.stdout, compiled.graph().canonical_json().as_bytes());
    let value: serde_json::Value = serde_json::from_slice(&graph.stdout).unwrap();
    assert_eq!(value["schema"], "semaprax.agent-graph.v1");
    assert_eq!(value["agent_id"], "fixture.agent");

    for arguments in [
        &["agent", "inspect", text(&bundle.definition), "--profile"][..],
        &["agent", "inspect", "--profile", text(&bundle.definition)][..],
    ] {
        let profile = cli(arguments);
        assert!(profile.status.success(), "{arguments:?}");
        assert!(profile.stderr.is_empty());
        assert_eq!(profile.stdout, compiled.runtime_v1_profile().as_bytes());
    }
}

#[test]
fn verify_replays_the_bundle_and_rejects_tampering() {
    let bundle = Bundle::new();
    let receipt = cli(&[
        "verify",
        text(&bundle.definition),
        text(&bundle.profile),
        text(&bundle.graph),
    ]);
    assert!(
        receipt.status.success(),
        "{}",
        String::from_utf8_lossy(&receipt.stderr)
    );
    assert!(receipt.stderr.is_empty());
    assert_eq!(
        String::from_utf8(receipt.stdout).unwrap(),
        "{\"schema\":\"semaprax.agent-graph-verification.v1\",\"agent_id\":\"fixture.agent\",\"definition_digest\":\"sha256:82ab9abbeca5e209c36224d9cab3b7b6a7cdffc3b2fce5db73123fa7425965a0\",\"graph_digest\":\"sha256:0dc7ce1d50d43077042577cf6ac3dcfb5d2a744fb3acd2ca6cea12a6e296ff61\",\"verified\":true,\"authority\":false}\n"
    );

    let graph = std::fs::read_to_string(&bundle.graph).unwrap();
    let tampered = bundle.root.join("tampered.json");
    std::fs::write(
        &tampered,
        graph.replacen(
            "\"agent_id\":\"fixture.agent\"",
            "\"agent_id\":\"fixture.other\"",
            1,
        ),
    )
    .unwrap();
    let rejected = cli(&[
        "verify",
        text(&bundle.definition),
        text(&bundle.profile),
        text(&tampered),
    ]);
    assert_eq!(rejected.status.code(), Some(1));
    assert!(rejected.stdout.is_empty());
    assert!(String::from_utf8(rejected.stderr)
        .unwrap()
        .contains("SPX-G503"));

    let swapped = cli(&[
        "verify",
        text(&bundle.definition),
        text(&bundle.graph),
        text(&bundle.profile),
    ]);
    assert_eq!(swapped.status.code(), Some(1));
    assert!(String::from_utf8(swapped.stderr)
        .unwrap()
        .contains("SPX-V201"));
}

#[test]
fn agent_grammar_fails_closed() {
    let bundle = Bundle::new();
    for arguments in [
        &["agent"][..],
        &["agent", "run", text(&bundle.definition)][..],
        &["agent", "resume", text(&bundle.definition)][..],
        &["agent", "inspect"][..],
        &["agent", "inspect", text(&bundle.definition), "extra"][..],
        &["agent", "inspect", text(&bundle.definition), "--json"][..],
    ] {
        let output = cli(arguments);
        assert_eq!(output.status.code(), Some(2), "{arguments:?}");
        assert!(output.stdout.is_empty(), "{arguments:?}");
        assert!(!output.stderr.is_empty(), "{arguments:?}");
    }
    let unreadable = cli(&["agent", "inspect", text(&bundle.root.join("missing.json"))]);
    assert_eq!(unreadable.status.code(), Some(1));
    assert!(String::from_utf8(unreadable.stderr)
        .unwrap()
        .contains("SPX-I001"));
    let malformed = bundle.root.join("malformed.json");
    std::fs::write(
        &malformed,
        "{\"schema\":\"semaprax.agent-definition.v1\"}\n",
    )
    .unwrap();
    let rejected = cli(&["agent", "inspect", text(&malformed)]);
    assert_eq!(rejected.status.code(), Some(1));
    assert!(rejected.stdout.is_empty());
    assert!(String::from_utf8(rejected.stderr)
        .unwrap()
        .contains("SPX-G50"));
}

const TRANSCRIPT: &str = "{\"schema\":\"semaprax.agent-runtime-transcript.v1\",\"policy_epoch\":7,\"provider\":[{\"disposition\":\"succeeded\",\"response\":\"{\\\"schema\\\":\\\"semaprax.agent-runtime-action.v1\\\",\\\"kind\\\":\\\"tool\\\",\\\"tool_id\\\":\\\"fixture.read\\\",\\\"arguments\\\":{\\\"query\\\":\\\"alpha\\\"}}\\n\"},{\"disposition\":\"succeeded\",\"response\":\"{\\\"schema\\\":\\\"semaprax.agent-runtime-action.v1\\\",\\\"kind\\\":\\\"final\\\",\\\"message\\\":\\\"done\\\"}\\n\"}],\"tools\":[{\"result\":\"{\\\"value\\\":\\\"alpha\\\"}\"}]}\n";

#[test]
fn run_follows_the_transcript_and_replay_recomputes_its_evidence() {
    let bundle = Bundle::new();
    let task_path = bundle.root.join("task.json");
    std::fs::write(&task_path, task()).unwrap();
    let transcript = bundle.root.join("transcript.json");
    std::fs::write(&transcript, TRANSCRIPT).unwrap();

    let receipt = cli(&[
        "agent",
        "run",
        text(&bundle.definition),
        text(&task_path),
        text(&transcript),
    ]);
    assert!(
        receipt.status.success(),
        "{}",
        String::from_utf8_lossy(&receipt.stderr)
    );
    assert!(receipt.stderr.is_empty());
    let receipt: serde_json::Value = serde_json::from_slice(&receipt.stdout).unwrap();
    assert_eq!(receipt["schema"], "semaprax.agent-run-receipt.v1");
    assert_eq!(receipt["agent_id"], "fixture.agent");
    assert_eq!(receipt["status"], "completed");
    assert_eq!(receipt["final_message"], "done");
    assert_eq!(receipt["authority"], false);
    let evidence_digest = receipt["evidence_digest"].as_str().unwrap().to_owned();
    assert!(evidence_digest.starts_with("sha256:"));

    let evidence = cli(&[
        "agent",
        "run",
        text(&bundle.definition),
        text(&task_path),
        text(&transcript),
        "--evidence",
    ]);
    assert!(evidence.status.success());
    let evidence_text = String::from_utf8(evidence.stdout).unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&evidence_text).unwrap();
    assert_eq!(parsed["schema"], "semaprax.agent-runtime-evidence.v1");
    // Deterministic: the same documents produce the same evidence twice.
    let again = cli(&[
        "agent",
        "run",
        "--evidence",
        text(&bundle.definition),
        text(&task_path),
        text(&transcript),
    ]);
    assert_eq!(String::from_utf8(again.stdout).unwrap(), evidence_text);
    let trace = cli(&[
        "agent",
        "run",
        text(&bundle.definition),
        text(&task_path),
        text(&transcript),
        "--trace",
    ]);
    assert!(trace.status.success());
    let trace: serde_json::Value = serde_json::from_slice(&trace.stdout).unwrap();
    assert_eq!(trace["schema"], "semaprax.agent-runtime-trace.v1");

    let evidence_path = bundle.root.join("evidence.json");
    std::fs::write(&evidence_path, &evidence_text).unwrap();
    let replayed = cli(&[
        "agent",
        "replay",
        text(&bundle.definition),
        text(&task_path),
        text(&transcript),
        text(&evidence_path),
    ]);
    assert!(
        replayed.status.success(),
        "{}",
        String::from_utf8_lossy(&replayed.stderr)
    );
    let replayed: serde_json::Value = serde_json::from_slice(&replayed.stdout).unwrap();
    assert_eq!(replayed["schema"], "semaprax.agent-replay-receipt.v1");
    assert_eq!(replayed["verified"], true);
    assert_eq!(replayed["evidence_digest"], evidence_digest);

    let tampered = bundle.root.join("tampered.json");
    std::fs::write(
        &tampered,
        evidence_text.replacen("completed", "cancelled", 1),
    )
    .unwrap();
    let rejected = cli(&[
        "agent",
        "replay",
        text(&bundle.definition),
        text(&task_path),
        text(&transcript),
        text(&tampered),
    ]);
    assert_eq!(rejected.status.code(), Some(1));
    assert!(rejected.stdout.is_empty());
    assert!(String::from_utf8(rejected.stderr)
        .unwrap()
        .contains("SPX-V222"));

    // An exhausted provider script is a deterministic provider failure, not a usage error.
    let empty = bundle.root.join("empty.json");
    std::fs::write(
        &empty,
        "{\"schema\":\"semaprax.agent-runtime-transcript.v1\",\"provider\":[]}\n",
    )
    .unwrap();
    let failed = cli(&[
        "agent",
        "run",
        text(&bundle.definition),
        text(&task_path),
        text(&empty),
    ]);
    assert!(
        failed.status.success(),
        "{}",
        String::from_utf8_lossy(&failed.stderr)
    );
    let failed: serde_json::Value = serde_json::from_slice(&failed.stdout).unwrap();
    assert_eq!(failed["status"], "provider_failed");
    assert_eq!(failed["final_message"], serde_json::Value::Null);

    for (contents, code) in [
        ("{\"schema\":\"other\",\"provider\":[]}\n", "SPX-V221"),
        ("{\"schema\":\"semaprax.agent-runtime-transcript.v1\",\"provider\":[],\"extra\":1}\n", "SPX-V221"),
        ("{\"schema\":\"semaprax.agent-runtime-transcript.v1\",\"provider\":[{\"disposition\":\"succeeded\"}]}\n", "SPX-V221"),
        ("not json\n", "SPX-V221"),
    ] {
        let malformed = bundle.root.join("malformed.json");
        std::fs::write(&malformed, contents).unwrap();
        let output = cli(&["agent", "run", text(&bundle.definition), text(&task_path), text(&malformed)]);
        assert_eq!(output.status.code(), Some(1), "{contents}");
        assert!(output.stdout.is_empty());
        assert!(String::from_utf8(output.stderr).unwrap().contains(code), "{contents}");
    }
    for arguments in [
        &["agent", "resume", text(&bundle.definition)][..],
        &["agent", "reconcile", text(&bundle.definition)][..],
        &["agent", "run", text(&bundle.definition), text(&task_path)][..],
        &[
            "agent",
            "replay",
            text(&bundle.definition),
            text(&task_path),
            text(&transcript),
        ][..],
    ] {
        let output = cli(arguments);
        assert_eq!(output.status.code(), Some(2), "{arguments:?}");
        assert!(
            String::from_utf8(output.stderr)
                .unwrap()
                .contains("not admitted"),
            "{arguments:?}"
        );
    }
}
