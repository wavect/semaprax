use std::path::Path;
use std::process::Command;

use semaprax::project::{
    prepare_project_interpreter, verify_execution_envelope, verify_project_source_trace,
    verify_project_source_trace_against_revision, with_authenticated_project,
    PreparedProjectExecutionOptions, PreparedProjectInterpreterOptions,
    ProjectExecutionCancellation, ProjectExecutionOptions, ProjectManifest, ProjectProfile,
    PROJECT_HTTPS_COMMAND_CAPABILITIES_V1, PROJECT_PROFILE_HTTPS_COMMAND_IO_V1, PROJECT_SCHEMA_V13,
};

const MANIFEST: &str = include_str!("../../examples/https-project/semaprax.toml");

#[test]
fn project_v13_manifest_round_trips_the_exact_https_profile() {
    let manifest = ProjectManifest::parse(MANIFEST).unwrap();
    assert_eq!(manifest.schema(), PROJECT_SCHEMA_V13);
    assert!(manifest.is_v13());
    assert_eq!(manifest.project_profile(), ProjectProfile::HttpsCommandIoV1);
    assert_eq!(
        manifest.profile(),
        Some(PROJECT_PROFILE_HTTPS_COMMAND_IO_V1)
    );
    assert_eq!(manifest.command(), Some("https-client.fetch"));
    assert!(manifest
        .capabilities()
        .iter()
        .map(String::as_str)
        .eq(PROJECT_HTTPS_COMMAND_CAPABILITIES_V1));
    assert_eq!(manifest.to_canonical_toml(), MANIFEST);
}

#[test]
fn project_v13_authenticates_and_admits_the_https_command_closure() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("examples/https-project");
    with_authenticated_project(&root.join("semaprax.toml"), |snapshot| snapshot.check()).unwrap();
}

#[test]
fn project_v13_execution_and_prepared_trace_envelopes_replay() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("examples/https-project");
    with_authenticated_project(&root.join("semaprax.toml"), |snapshot| {
        let execution = snapshot.execute_entry(&ProjectExecutionOptions::default())?;
        verify_execution_envelope(execution.envelope()).map_err(|error| vec![error])?;

        let revision = snapshot.retain_revision();
        let prepared = prepare_project_interpreter(
            revision.clone(),
            PreparedProjectInterpreterOptions::default(),
        )?;
        let cancellation = ProjectExecutionCancellation::new();
        let traced =
            prepared.execute_entry(&PreparedProjectExecutionOptions::default(), &cancellation)?;
        verify_project_source_trace(traced.trace().envelope()).map_err(|error| vec![error])?;
        verify_project_source_trace_against_revision(&revision, traced.trace().envelope())
            .map_err(|error| vec![error])?;
        Ok(())
    })
    .unwrap();
}

#[test]
fn https_network_run_replays_fixture_v3_without_opening_a_socket() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("examples/https-project");
    let output = Command::new(env!("CARGO_BIN_EXE_semaprax"))
        .arg("network-run")
        .arg(&root)
        .arg("--fixture")
        .arg(root.join("https.fixture.json"))
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        output.stdout,
        b"HTTP/1.1 200 semaprax\r\ncontent-length: 2\r\n\r\nok"
    );
    assert!(output.stderr.is_empty());
}

#[test]
fn project_v13_rejects_capability_and_profile_drift() {
    for invalid in [
        MANIFEST.replace("network.http", "network.connect"),
        MANIFEST.replace("https-command-io.v1", "network-command-io.v1"),
    ] {
        assert!(ProjectManifest::parse(&invalid).is_err());
    }
}
