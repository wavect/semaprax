use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use semaprax::project::{
    verify_execution_envelope, with_authenticated_project, ProjectExecutionOptions,
    ProjectManifest, ProjectProfile, PROJECT_COMMAND_ADAPTER_CAPABILITIES_V2,
    PROJECT_LANGUAGE_COMMAND_INPUT_V1, PROJECT_PROFILE_LINE_COMMAND_IO_V1, PROJECT_SCHEMA_V7,
};

static SERIAL: AtomicU64 = AtomicU64::new(0);

const MANIFEST: &str = "schema = \"semaprax.project.v7\"\nname = \"spxgrep-lines\"\nversion = \"1.0.0\"\nprofile = \"line-command-io.v1\"\nentry = \"grep.app\"\nsources = [\"a/app.spx\", \"z/tests.spx\"]\nweb_exports = [\"grep.lines.run\"]\ncommand = \"grep.lines.run\"\ninput = \"argv-utf8+stdin-bytes.v1\"\ncapabilities = [\"process.args.read\", \"process.stderr.write\", \"process.stdin.read\", \"process.stdout.write\"]\ntests = [\"grep.tests\"]\n";

struct Fixture(PathBuf);

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn real_project_fixture() -> Fixture {
    let root = std::env::temp_dir().join(format!(
        "semaprax-project-v7-report-{}-{}",
        std::process::id(),
        SERIAL.fetch_add(1, Ordering::Relaxed)
    ));
    std::fs::create_dir_all(root.join("src")).unwrap();
    let source = Path::new(env!("CARGO_MANIFEST_DIR")).join("examples/spxgrep-lines-project");
    for file in [
        "semaprax.toml",
        "src/app.spx",
        "src/filter.spx",
        "src/tests.spx",
    ] {
        std::fs::copy(source.join(file), root.join(file)).unwrap();
    }
    Fixture(root.canonicalize().unwrap())
}

#[test]
fn project_v7_manifest_round_trips_one_exact_line_command_envelope() {
    let manifest = ProjectManifest::parse(MANIFEST).unwrap();
    assert_eq!(manifest.schema(), PROJECT_SCHEMA_V7);
    assert!(manifest.is_v7());
    assert_eq!(manifest.project_profile(), ProjectProfile::LineCommandIoV1);
    assert_eq!(manifest.profile(), Some(PROJECT_PROFILE_LINE_COMMAND_IO_V1));
    assert_eq!(manifest.command(), Some("grep.lines.run"));
    assert_eq!(
        manifest.command_input(),
        Some(PROJECT_LANGUAGE_COMMAND_INPUT_V1)
    );
    assert!(manifest
        .capabilities()
        .iter()
        .map(String::as_str)
        .eq(PROJECT_COMMAND_ADAPTER_CAPABILITIES_V2));
    assert_eq!(manifest.to_canonical_toml(), MANIFEST);
}

#[test]
fn project_v7_rejects_profile_input_capability_and_command_export_drift() {
    for invalid in [
        MANIFEST.replace("line-command-io.v1", "language-command-io.v1"),
        MANIFEST.replace("argv-utf8+stdin-bytes.v1", "stdin-bytes+one-utf8-arg.v1"),
        MANIFEST.replace(
            "[\"process.args.read\", \"process.stderr.write\", \"process.stdin.read\", \"process.stdout.write\"]",
            "[\"process.args.read\", \"process.stdin.read\", \"process.stderr.write\", \"process.stdout.write\"]",
        ),
        MANIFEST.replace("web_exports = [\"grep.lines.run\"]", "web_exports = [\"grep.lines.other\"]"),
    ] {
        assert!(ProjectManifest::parse(&invalid).is_err());
    }
}

#[test]
fn project_v6_canonical_bytes_remain_frozen() {
    let v6 = MANIFEST
        .replace("semaprax.project.v7", "semaprax.project.v6")
        .replace("line-command-io.v1", "language-command-io.v1");
    assert_eq!(ProjectManifest::parse(&v6).unwrap().to_canonical_toml(), v6);
}

#[test]
fn project_v7_execute_entry_report_round_trips_and_v8_remains_unsupported() {
    let project = real_project_fixture();
    let execution = with_authenticated_project(&project.0.join("semaprax.toml"), |snapshot| {
        snapshot.execute_entry(&ProjectExecutionOptions::default())
    })
    .unwrap();
    verify_execution_envelope(execution.envelope()).unwrap();

    let unsupported = execution.envelope().replacen(
        "\"project_schema\":\"semaprax.project.v7\"",
        "\"project_schema\":\"semaprax.project.v8\"",
        1,
    );
    assert_ne!(unsupported, execution.envelope());
    let error = verify_execution_envelope(&unsupported).unwrap_err();
    assert_eq!(error.code, "SPX-F106");
    assert!(error.message.contains("v1, v2, v3, v4, v5, v6, or v7"));
}
