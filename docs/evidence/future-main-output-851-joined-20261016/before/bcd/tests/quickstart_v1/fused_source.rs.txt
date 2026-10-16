//! Real explicit profile dispatch; the immutable original fixture is unchanged.
use super::*;
const SOURCE: &str = include_str!("../fixtures/record-traversal-maximum.spx");
const ACCOUNT: &str = "16777216"; // Explicit caller choice, never a production default.
fn packet(output: &Output) -> serde_json::Value {
    serde_json::from_slice(&output.stdout).unwrap_or_else(|error| {
        panic!(
            "invalid profile packet: {error}; {:?}; {:?}",
            output.stdout, output.stderr
        )
    })
}
#[test]
fn explicit_fused_source_run_build_original_4096_and_account_refusal_keep_default_closed() {
    let fixture = Fixture::new("fused-source");
    std::fs::write(fixture.root.join("input.spx"), SOURCE).unwrap();
    let default = cli(&fixture.root, &["run", "input.spx", "--json"]);
    assert!(!default.status.success());
    assert!(
        String::from_utf8_lossy(&default.stdout).contains("SPX-T267")
            || String::from_utf8_lossy(&default.stderr).contains("SPX-T267")
    );
    for _ in 0..4 {
        let output = cli(
            &fixture.root,
            &[
                "run",
                "input.spx",
                "--profile",
                "fused-construction-v1",
                "--managed-bytes",
                ACCOUNT,
                "--json",
            ],
        );
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        let value = packet(&output);
        assert_eq!(value["result"], "8386560");
        assert_eq!(value["target"], "interpreter");
        assert_eq!(value["schema"], "semaprax.fused-source.v1");
        assert_eq!(value["max_steps"], 1_000_000);
        let used = value["managed_reserved"].as_u64().unwrap();
        for bytes in [used, used - 1] {
            let output = cli(
                &fixture.root,
                &[
                    "run",
                    "input.spx",
                    "--profile",
                    "fused-construction-v1",
                    "--managed-bytes",
                    &bytes.to_string(),
                    "--json",
                ],
            );
            let actual = packet(&output);
            assert_eq!(output.status.success(), bytes == used);
            if bytes == used {
                assert_eq!(actual["result"], "8386560");
            } else {
                assert_eq!(actual["outcome"], "administrative_refusal");
                assert!(actual.get("result").is_none());
            }
        }
    }
    let native_run = cli(
        &fixture.root,
        &[
            "run",
            "input.spx",
            "--profile",
            "fused-construction-v1",
            "--managed-bytes",
            ACCOUNT,
            "--native",
            "--json",
        ],
    );
    assert!(
        native_run.status.success(),
        "{}",
        String::from_utf8_lossy(&native_run.stderr)
    );
    let native_packet = packet(&native_run);
    assert_eq!(native_packet["result"], "8386560");
    assert_eq!(native_packet["target"], "native");
    assert_eq!(native_packet["schema"], "semaprax.fused-source.v1");
    let output = cli(
        &fixture.root,
        &[
            "build",
            "input.spx",
            "--profile",
            "fused-construction-v1",
            "--managed-bytes",
            ACCOUNT,
            "--target",
            "native",
            "--output",
            "native",
            "--json",
        ],
    );
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    for _ in 0..4 {
        let output = Command::new(fixture.root.join("native")).output().unwrap();
        assert!(output.status.success());
        assert_eq!(packet(&output)["result"], "8386560");
    }
    let before = std::fs::read(fixture.root.join("native")).unwrap();
    let repeat = cli(
        &fixture.root,
        &[
            "build",
            "input.spx",
            "--profile",
            "fused-construction-v1",
            "--managed-bytes",
            ACCOUNT,
            "--target",
            "native",
            "--output",
            "native",
        ],
    );
    assert!(!repeat.status.success());
    assert_eq!(std::fs::read(fixture.root.join("native")).unwrap(), before);
    std::fs::write(
        fixture.root.join("ordinary.spx"),
        "module ordinary; @id(\"ordinary.main\") fn main() -> i64 { 1 }",
    )
    .unwrap();
    let refused = cli(
        &fixture.root,
        &[
            "build",
            "ordinary.spx",
            "--profile",
            "fused-construction-v1",
            "--managed-bytes",
            ACCOUNT,
            "--target",
            "native",
            "--output",
            "refused-artifact",
            "--json",
        ],
    );
    assert!(!refused.status.success());
    assert!(!fixture.root.join("refused-artifact").exists());
    assert_eq!(
        std::fs::read_to_string(fixture.root.join("input.spx")).unwrap(),
        SOURCE
    );
}
#[test]
fn fused_source_options_deny_wrong_profile_project_fuel_and_unrelated_routes_before_io() {
    let fixture = Fixture::new("fused-options");
    for args in [
        vec![
            "build",
            "missing.spx",
            "--profile",
            "fused-construction-v1",
            "--managed-bytes",
            ACCOUNT,
        ],
        vec![
            "run",
            "missing.spx",
            "--profile",
            "wrong",
            "--managed-bytes",
            ACCOUNT,
        ],
        vec!["run", "missing.spx", "--profile", "fused-construction-v1"],
        vec!["run", "missing.spx", "--managed-bytes", ACCOUNT],
        vec![
            "run",
            "missing.spx",
            "--profile",
            "fused-construction-v1",
            "--managed-bytes",
            "0",
        ],
        vec![
            "run",
            "missing.spx",
            "--profile",
            "fused-construction-v1",
            "--managed-bytes",
            "01",
        ],
        vec![
            "run",
            "missing.spx",
            "--profile",
            "fused-construction-v1",
            "--managed-bytes",
            "1073741825",
        ],
        vec![
            "run",
            "missing.spx",
            "--profile",
            "fused-construction-v1",
            "--managed-bytes",
            ACCOUNT,
            "--native",
            "--max-steps",
            "100",
        ],
        vec![
            "run",
            "--manifest-path",
            "missing.toml",
            "--profile",
            "fused-construction-v1",
            "--managed-bytes",
            ACCOUNT,
        ],
        vec![
            "test",
            "missing.toml",
            "--profile",
            "fused-construction-v1",
            "--managed-bytes",
            ACCOUNT,
        ],
        vec![
            "build",
            "missing.spx",
            "--profile",
            "fused-construction-v1",
            "--managed-bytes",
            ACCOUNT,
            "--target",
            "web",
            "--output",
            "refused",
        ],
        vec![
            "build",
            "missing.spx",
            "--profile",
            "fused-construction-v1",
            "--managed-bytes",
            ACCOUNT,
            "--target",
            "native",
            "--export",
            "app.main",
            "--output",
            "refused",
        ],
        vec![
            "build",
            "--manifest-path",
            "missing.toml",
            "--profile",
            "fused-construction-v1",
            "--managed-bytes",
            ACCOUNT,
            "--target",
            "native",
            "--output",
            "refused",
        ],
    ] {
        let output = cli(&fixture.root, &args);
        assert_eq!(output.status.code(), Some(2), "{args:?}");
        assert!(output.stdout.is_empty());
        assert!(!fixture.root.join("refused").exists());
    }
    let build = build_cli::parse(
        &[
            "input.spx",
            "--profile",
            "fused-construction-v1",
            "--managed-bytes",
            ACCOUNT,
            "--target",
            "wasm",
        ]
        .map(str::to_owned),
    )
    .unwrap();
    assert_eq!(build.fused_account.unwrap().bytes(), 16 * 1024 * 1024);
    assert!(matches!(build.input, build_cli::BuildInput::Source(_)));
}

#[test]
fn explicit_fused_source_wasm_package_uses_paired_loader_one_account_and_lossless_results() {
    let fixture = Fixture::new("fused-source-wasm");
    std::fs::write(fixture.root.join("input.spx"), SOURCE).unwrap();
    let build = |name: &str, bytes: u64| {
        let output = cli(
            &fixture.root,
            &[
                "build",
                "input.spx",
                "--profile",
                "fused-construction-v1",
                "--managed-bytes",
                &bytes.to_string(),
                "--target",
                "wasm",
                "--output",
                name,
                "--json",
            ],
        );
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        fixture.root.join(name)
    };
    let probe = build("probe", ACCOUNT.parse().unwrap());
    let manifest: serde_json::Value =
        serde_json::from_slice(&std::fs::read(probe.join("fused-construction-v1.json")).unwrap())
            .unwrap();
    let registry = manifest["funding"]["registry_buffer_bytes"]
        .as_u64()
        .unwrap();
    let unit = manifest["funding"]["per_invocation_arena_bytes"]
        .as_u64()
        .unwrap();
    let digest = manifest["source_sha256"].as_str().unwrap();
    let driver = fixture.root.join("public-package-driver.mjs");
    std::fs::write(
        &driver,
        include_str!("../../src/wasm/fused_source_package/driver.mjs"),
    )
    .unwrap();
    for (mode, bytes) in [
        ("exact", registry + 3 * unit),
        ("one-short", registry + unit - 1),
        ("tampered", registry + 3 * unit),
    ] {
        let package = build(mode, bytes);
        if mode == "tampered" {
            let path = package.join("fused-construction-v1.wasm");
            let mut module = std::fs::read(&path).unwrap();
            module[0] ^= 1;
            std::fs::write(path, module).unwrap();
        }
        let output = Command::new("node")
            .arg(&driver)
            .arg(&package)
            .arg(mode)
            .arg("8386560")
            .arg(digest)
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{mode}: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert_eq!(
            String::from_utf8_lossy(&output.stdout).trim(),
            "fused-source-package-v1-ok"
        );
    }
    // Actual source-level arithmetic failure through the normal public loader;
    // no host fault injection or replacement module/grant enters this path.
    let source_driver = fixture.root.join("source-outcome-driver.mjs");
    std::fs::write(
        &source_driver,
        r#"
import assert from 'node:assert/strict';
import path from 'node:path';
import {pathToFileURL} from 'node:url';
const [directory, outcome, expected] = process.argv.slice(2);
const entry = await import(pathToFileURL(path.join(directory, 'fused-construction-v1.mjs')).href);
const {MANIFEST} = entry;
const registry = MANIFEST.funding.registry_buffer_bytes;
const unit = MANIFEST.funding.per_invocation_arena_bytes;
const limit = MANIFEST.funding.account_bytes;
assert.equal(limit, registry + 3 * unit);
const runner = await entry.install(limit);
for (let index = 1; index <= 3; ++index) {
  const value = runner.invoke();
  assert.equal(value.schema, 'semaprax.fused-source.v1');
  assert.equal(value.source_sha256, MANIFEST.source_sha256);
  assert.equal(value.target, 'wasm');
  assert.equal(value.account_limit, limit);
  assert.equal(value.managed_reserved, registry + index * unit);
  assert.equal(value.outcome, outcome);
  if (outcome === 'returned') {
    assert.equal(value.result, expected);
    assert.equal(typeof value.result, 'string');
    assert.equal('status' in value, false);
  } else {
    assert.deepEqual(value.status, {domain: 'semaprax.arithmetic.v1', code: 1});
    assert.equal('result' in value, false);
  }
}
const refused = runner.invoke();
assert.equal(refused.outcome, 'administrative_refusal');
assert.equal(refused.managed_reserved, limit);
assert.equal('result' in refused, false);
assert.equal('status' in refused, false);
await assert.rejects(entry.install(limit), /install-attempted/);
console.log('public-source-outcome-ok');
"#,
    )
    .unwrap();
    for (name, tail, outcome, expected) in [
        (
            "maximum",
            "{ 9223372036854775807 } else { 0 }",
            "returned",
            "9223372036854775807",
        ),
        (
            "overflow",
            "{ total + 9223372036854775807 } else { 0 }",
            "language_failure",
            "",
        ),
    ] {
        let changed = SOURCE.replace("{ total } else { 0 }", tail);
        assert_ne!(changed, SOURCE);
        std::fs::write(fixture.root.join("input.spx"), &changed).unwrap();
        // Discover this exact source's compiler-derived unit before selecting
        // the explicit three-invocation caller amount; never auto-fund at runtime.
        let discovery = build(&format!("{name}-probe"), ACCOUNT.parse().unwrap());
        let facts: serde_json::Value = serde_json::from_slice(
            &std::fs::read(discovery.join("fused-construction-v1.json")).unwrap(),
        )
        .unwrap();
        let amount = facts["funding"]["registry_buffer_bytes"].as_u64().unwrap()
            + 3 * facts["funding"]["per_invocation_arena_bytes"]
                .as_u64()
                .unwrap();
        let package = build(name, amount);
        let output = Command::new("node")
            .arg(&source_driver)
            .arg(package)
            .arg(outcome)
            .arg(expected)
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{name}: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert_eq!(
            String::from_utf8_lossy(&output.stdout).trim(),
            "public-source-outcome-ok"
        );
        assert_eq!(
            std::fs::read_to_string(fixture.root.join("input.spx")).unwrap(),
            changed
        );
    }
    std::fs::write(fixture.root.join("input.spx"), SOURCE).unwrap();
    let published = probe.join("fused-construction-v1.wasm");
    let before = std::fs::read(&published).unwrap();
    let repeat = cli(
        &fixture.root,
        &[
            "build",
            "input.spx",
            "--profile",
            "fused-construction-v1",
            "--managed-bytes",
            ACCOUNT,
            "--target",
            "wasm",
            "--output",
            "probe",
        ],
    );
    assert!(!repeat.status.success());
    assert_eq!(std::fs::read(published).unwrap(), before);
    assert_eq!(
        std::fs::read_to_string(fixture.root.join("input.spx")).unwrap(),
        SOURCE
    );
}
