use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};

use semaprax::project::{
    prepare_project_interpreter, verify_execution_envelope, verify_project_source_trace,
    verify_project_source_trace_against_revision, with_authenticated_project,
    PreparedProjectExecutionOptions, PreparedProjectInterpreterOptions,
    ProjectExecutionCancellation, ProjectExecutionOptions, ProjectManifest, ProjectProfile,
    PROJECT_HTTPS_COMMAND_CAPABILITIES_V1, PROJECT_PROFILE_HTTPS_COMMAND_IO_V1, PROJECT_SCHEMA_V13,
};

const MANIFEST: &str = include_str!("../../examples/https-project/semaprax.toml");
static SERIAL: AtomicU64 = AtomicU64::new(0);

struct Output(PathBuf);

impl Drop for Output {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

#[cfg(unix)]
struct NativeOutput(PathBuf);

#[cfg(unix)]
impl Drop for NativeOutput {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.0);
    }
}

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
fn exhaustive_linux_ci_provisions_the_native_https_development_interface() {
    let workflow = include_str!("../../.github/workflows/ci.yml");
    let prerequisite = "- name: Provision the native HTTPS development interface (Linux)";
    assert_eq!(workflow.matches(prerequisite).count(), 3);
    assert_eq!(
        workflow
            .matches("sudo apt-get install --yes --no-install-recommends libcurl4-openssl-dev")
            .count(),
        3
    );
    for (job, next, test_gate) in [
        ("verify", "verify-tests", "cargo test"),
        (
            "verify-tests",
            "desktop-native-product",
            "python3 scripts/ci-msrv.py",
        ),
        ("msrv", "release-gate", "python3 scripts/ci-msrv.py"),
    ] {
        let body = workflow
            .split_once(&format!("\n  {job}:\n"))
            .unwrap()
            .1
            .split_once(&format!("\n  {next}:\n"))
            .unwrap()
            .0;
        assert!(body.contains(prerequisite), "{job} lost libcurl headers");
        assert!(body.find(prerequisite).unwrap() < body.find(test_gate).unwrap());
    }
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
fn project_v13_builds_a_replayable_fixture_only_npm_web_carrier() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("examples/https-project");
    let build = with_authenticated_project(&root.join("semaprax.toml"), |snapshot| {
        snapshot.build_npm_inline(semaprax::project::MAX_PROJECT_NPM_BUILD_BYTES)
    })
    .unwrap();
    build.verify().unwrap();
    assert!(build
        .envelope()
        .contains(semaprax::project::PROJECT_NPM_BUILD_SCHEMA_V12));
    assert!(build.envelope().contains("semaprax.https.json"));
}

#[cfg(unix)]
#[test]
fn project_v13_builds_the_libcurl_native_https_executable() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("examples/https-project");
    let suffix = std::env::consts::EXE_SUFFIX;
    let output = NativeOutput(std::env::temp_dir().join(format!(
        "semaprax-https-project-native-{}-{}{}",
        std::process::id(),
        SERIAL.fetch_add(1, Ordering::Relaxed),
        suffix
    )));
    with_authenticated_project(&root.join("semaprax.toml"), |snapshot| {
        snapshot.build_native(&output.0)
    })
    .unwrap();
    let metadata = std::fs::metadata(&output.0).unwrap();
    assert!(metadata.is_file() && metadata.len() != 0);
}

#[test]
fn generated_https_web_package_runs_fixture_v3_under_node() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("examples/https-project");
    let output = Output(std::env::temp_dir().join(format!(
        "semaprax-https-project-npm-{}-{}",
        std::process::id(),
        SERIAL.fetch_add(1, Ordering::Relaxed)
    )));
    with_authenticated_project(&root.join("semaprax.toml"), |snapshot| {
        snapshot.build_npm(&output.0)
    })
    .unwrap();
    let metadata = std::fs::read_to_string(output.0.join("semaprax.https.json")).unwrap();
    assert!(metadata.contains("\"provider\":\"fixture-only.v3\""));
    assert!(metadata.contains("\"capabilities\":[\"network.http\""));
    let script = r#"import fs from'node:fs';import{createFixture,createInvocation,instantiate}from'./semaprax.bindings.js';const fixture=createFixture(JSON.parse(fs.readFileSync(process.argv[1],'utf8')));const invocation=createInvocation([],new Uint8Array(),fixture);const result=await instantiate(new Uint8Array(fs.readFileSync('./app.wasm')),invocation);process.stdout.write(result.stdout);process.stderr.write(result.stderr);if(!result.result)process.exitCode=1;"#;
    let run = Command::new("node")
        .current_dir(&output.0)
        .args(["--input-type=module", "--eval", script])
        .arg(root.join("https.fixture.json"))
        .output()
        .unwrap();
    assert!(
        run.status.success(),
        "{}",
        String::from_utf8_lossy(&run.stderr)
    );
    assert_eq!(
        run.stdout,
        b"HTTP/1.1 200 semaprax\r\ncontent-length: 2\r\n\r\nok"
    );
    assert!(run.stderr.is_empty());
}

#[test]
fn generated_https_web_package_rejects_untrusted_fixture_and_provider_results() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("examples/https-project");
    let output = Output(std::env::temp_dir().join(format!(
        "semaprax-https-project-hostile-{}-{}",
        std::process::id(),
        SERIAL.fetch_add(1, Ordering::Relaxed)
    )));
    with_authenticated_project(&root.join("semaprax.toml"), |snapshot| {
        snapshot.build_npm(&output.0)
    })
    .unwrap();
    let script = r#"import fs from'node:fs';import{createFixture,createInvocation,instantiate}from'./semaprax.bindings.js';const wasm=new Uint8Array(fs.readFileSync('./app.wasm'));const expectThrow=async(fn,check)=>{let error;try{await fn()}catch(value){error=value}if(!error||!check(error))throw Error('expected rejection')};await expectThrow(()=>createFixture({schema:'semaprax.network-fixture.v2',connections:[],https:[]}),error=>error instanceof TypeError);const oversized=createFixture({schema:'semaprax.network-fixture.v3',connections:[],https:[{url:'https://example.test/data',response:'x'.repeat(1025)}]});const once=createInvocation([],new Uint8Array(),oversized);await expectThrow(()=>instantiate(wasm,once),error=>error.domain==='semaprax.http.v1'&&error.code===4);await expectThrow(()=>instantiate(wasm,once),error=>error instanceof TypeError);const mismatch=createFixture({schema:'semaprax.network-fixture.v3',connections:[],https:[{url:'https://other.test/',response:'ok'}]});await expectThrow(()=>instantiate(wasm,createInvocation([],new Uint8Array(),mismatch)),error=>error.domain==='semaprax.http.v1'&&error.code===3);const tampered=new Uint8Array(wasm);tampered[tampered.length-1]^=1;const valid=createFixture({schema:'semaprax.network-fixture.v3',connections:[],https:[{url:'https://example.test/data',response:'ok'}]});await expectThrow(()=>instantiate(tampered,createInvocation([],new Uint8Array(),valid)),error=>error.message==='Wasm authentication');"#;
    let run = Command::new("node")
        .current_dir(&output.0)
        .args(["--input-type=module", "--eval", script])
        .output()
        .unwrap();
    assert!(
        run.status.success(),
        "{}",
        String::from_utf8_lossy(&run.stderr)
    );
    assert!(run.stdout.is_empty());
    assert!(run.stderr.is_empty());
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

#[test]
fn project_v13_https_browser_gate_is_locked_isolated_and_provisioned() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let fixture = root.join("platform-tests/https-browser-v1");
    let package: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(fixture.join("package.json")).unwrap())
            .unwrap();
    assert_eq!(package["devDependencies"]["@playwright/test"], "1.62.0");
    let lock: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(fixture.join("package-lock.json")).unwrap())
            .unwrap();
    assert_eq!(lock["lockfileVersion"], 3);
    assert_eq!(
        lock["packages"]["node_modules/playwright-core"]["version"],
        "1.62.0"
    );

    let config = std::fs::read_to_string(fixture.join("playwright.config.mjs")).unwrap();
    let server = std::fs::read_to_string(fixture.join("serve.mjs")).unwrap();
    let browser = std::fs::read_to_string(fixture.join("tests/https.spec.mjs")).unwrap();
    for required in [
        "SEMAPRAX_HTTPS_PACKAGE_ROOT",
        "browserName: \"chromium\"",
        "workers: 1",
        "retries: 0",
        "reuseExistingServer: false",
    ] {
        assert!(
            config.contains(required),
            "browser config lost `{required}`"
        );
    }
    assert!(server.contains("127.0.0.1"));
    assert!(server.contains("application/wasm"));
    for required in [
        "reuseRejected: true",
        "authenticationRejected: true",
        "new Set(origins)",
        "pageErrors",
        "requestfailed",
    ] {
        assert!(browser.contains(required), "browser test lost `{required}`");
    }

    let workflow = std::fs::read_to_string(root.join(".github/workflows/ci.yml")).unwrap();
    for required in [
        "Build the Project v13 HTTPS package into an isolated fixture",
        "examples/https-project/semaprax.toml --target npm",
        "SEMAPRAX_HTTPS_PACKAGE_ROOT=$https_root",
        "working-directory: platform-tests/https-browser-v1",
        "Exercise generated fixture-backed HTTPS in real Chromium",
    ] {
        assert!(
            workflow.contains(required),
            "HTTPS browser CI lost `{required}`"
        );
    }
}
