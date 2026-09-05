use std::path::Path;
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};

use semaprax::project::{
    with_authenticated_project, ProjectManifest, ProjectProfile,
    PROJECT_NETWORK_COMMAND_CAPABILITIES_V1, PROJECT_PROFILE_NETWORK_COMMAND_IO_V1,
    PROJECT_SCHEMA_V12,
};

const MANIFEST: &str = include_str!("../../examples/network-http-project/semaprax.toml");
static SERIAL: AtomicU64 = AtomicU64::new(0);

struct Output(std::path::PathBuf);

impl Drop for Output {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

#[test]
fn project_v12_manifest_round_trips_the_exact_network_profile() {
    let manifest = ProjectManifest::parse(MANIFEST).unwrap();
    assert_eq!(manifest.schema(), PROJECT_SCHEMA_V12);
    assert!(manifest.is_v12());
    assert_eq!(
        manifest.project_profile(),
        ProjectProfile::NetworkCommandIoV1
    );
    assert_eq!(
        manifest.profile(),
        Some(PROJECT_PROFILE_NETWORK_COMMAND_IO_V1)
    );
    assert_eq!(manifest.command(), Some("network-http.fetch"));
    assert!(manifest
        .capabilities()
        .iter()
        .map(String::as_str)
        .eq(PROJECT_NETWORK_COMMAND_CAPABILITIES_V1));
    assert_eq!(manifest.to_canonical_toml(), MANIFEST);
}

#[test]
fn project_v12_authenticates_and_admits_the_network_command_closure() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("examples/network-http-project");
    with_authenticated_project(&root.join("semaprax.toml"), |snapshot| snapshot.check()).unwrap();
}

#[test]
fn project_v12_builds_a_replayable_fixture_only_npm_web_carrier() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("examples/network-http-project");
    let build = with_authenticated_project(&root.join("semaprax.toml"), |snapshot| {
        snapshot.build_npm_inline(semaprax::project::MAX_PROJECT_NPM_BUILD_BYTES)
    })
    .unwrap();
    build.verify().unwrap();
    assert!(build
        .envelope()
        .contains(semaprax::project::PROJECT_NPM_BUILD_SCHEMA_V11));
    assert!(build.envelope().contains("semaprax.network.json"));
    // Artifact bytes are hex-encoded by the canonical carrier.
    assert!(build.envelope().contains("666978747572652d6f6e6c792e7631"));
}

#[test]
fn generated_network_web_package_runs_the_fixture_under_node() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("examples/network-http-project");
    let output = Output(std::env::temp_dir().join(format!(
        "semaprax-network-project-npm-{}-{}",
        std::process::id(),
        SERIAL.fetch_add(1, Ordering::Relaxed)
    )));
    with_authenticated_project(&root.join("semaprax.toml"), |snapshot| {
        snapshot.build_npm(&output.0)
    })
    .unwrap();
    let script = r#"import fs from 'node:fs';import{createFixture,createInvocation,instantiate}from'./semaprax.bindings.js';const fixture=createFixture(JSON.parse(fs.readFileSync(process.argv[1],'utf8')));const invocation=createInvocation([],new Uint8Array(),fixture);const result=await instantiate(new Uint8Array(fs.readFileSync('./app.wasm')),invocation);process.stdout.write(result.stdout);process.stderr.write(result.stderr);if(!result.result)process.exitCode=1;"#;
    let run = Command::new("node")
        .current_dir(&output.0)
        .args(["--input-type=module", "--eval", script])
        .arg(root.join("http.fixture.json"))
        .output()
        .unwrap();
    assert!(
        run.status.success(),
        "{}",
        String::from_utf8_lossy(&run.stderr)
    );
    assert_eq!(
        run.stdout,
        b"HTTP/1.1 200 OK\r\nContent-Length: 5\r\n\r\nhello"
    );
    assert!(run.stderr.is_empty());
}

#[test]
fn network_run_replays_a_fixture_without_opening_a_socket() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("examples/network-http-project");
    let output = Command::new(env!("CARGO_BIN_EXE_semaprax"))
        .arg("network-run")
        .arg(&root)
        .arg("--fixture")
        .arg(root.join("http.fixture.json"))
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        output.stdout,
        b"HTTP/1.1 200 OK\r\nContent-Length: 5\r\n\r\nhello"
    );
    assert!(output.stderr.is_empty());
}

#[test]
fn project_v12_rejects_capability_and_command_export_drift() {
    for invalid in [
        MANIFEST.replace("network.read\", ", ""),
        MANIFEST.replace(
            "web_exports = [\"network-http.fetch\"]",
            "web_exports = [\"network-http.other\"]",
        ),
    ] {
        assert!(ProjectManifest::parse(&invalid).is_err());
    }
}
