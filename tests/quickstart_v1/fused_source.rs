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
    let required = native_packet["managed_reserved"].as_u64().unwrap();
    let native = |bytes: u64| {
        cli(
            &fixture.root,
            &[
                "run",
                "input.spx",
                "--profile",
                "fused-construction-v1",
                "--managed-bytes",
                &bytes.to_string(),
                "--native",
                "--json",
            ],
        )
    };
    let exact = native(required);
    assert!(exact.status.success());
    let exact = packet(&exact);
    assert_eq!(exact["outcome"], "returned");
    assert_eq!(exact["result"], "8386560");
    assert_eq!(exact["account_limit"], required);
    assert_eq!(exact["managed_reserved"], required);
    assert_eq!(exact["source_sha256"], native_packet["source_sha256"]);

    let short = native(required - 1);
    assert_eq!(short.status.code(), Some(2));
    let short = packet(&short);
    assert_eq!(short["outcome"], "administrative_refusal");
    assert_eq!(short["account_limit"], required - 1);
    assert_eq!(short["managed_reserved"], 0);
    assert_eq!(short["source_sha256"], exact["source_sha256"]);
    assert!(short.get("result").is_none() && short.get("status").is_none());

    let overflow = SOURCE.replace(
        "{ total } else { 0 }",
        "{ total + 9223372036854775807 } else { 0 }",
    );
    assert_ne!(overflow, SOURCE);
    std::fs::write(fixture.root.join("input.spx"), &overflow).unwrap();
    let failure = native(ACCOUNT.parse().unwrap());
    assert_eq!(failure.status.code(), Some(1));
    let failure = packet(&failure);
    assert_eq!(failure["outcome"], "language_failure");
    assert_eq!(
        failure["status"],
        serde_json::json!({"domain": "semaprax.arithmetic.v1", "code": 1})
    );
    assert!(failure.get("result").is_none());
    assert_ne!(failure["source_sha256"], exact["source_sha256"]);
    std::fs::write(fixture.root.join("input.spx"), SOURCE).unwrap();

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
    #[cfg(target_os = "macos")]
    {
        success(
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
                "native-repeat",
            ],
        );
        assert_eq!(
            std::fs::read(fixture.root.join("native-repeat")).unwrap(),
            before
        );
    }
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
    let identical = build("probe-repeat", ACCOUNT.parse().unwrap());
    let package = |directory: &Path| {
        std::fs::read_dir(directory)
            .unwrap()
            .map(|entry| {
                let entry = entry.unwrap();
                assert!(entry.file_type().unwrap().is_file());
                (entry.file_name(), std::fs::read(entry.path()).unwrap())
            })
            .collect::<std::collections::BTreeMap<_, _>>()
    };
    let first_package = package(&probe);
    assert_eq!(
        first_package
            .keys()
            .map(|name| name.to_str().unwrap())
            .collect::<Vec<_>>(),
        [
            "fused-construction-v1.json",
            "fused-construction-v1.mjs",
            "fused-construction-v1.wasm",
            "selected-host.mjs",
        ]
    );
    assert_eq!(package(&identical), first_package);
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
            .current_dir(&fixture.root)
            .arg(driver.file_name().unwrap())
            .arg(package.file_name().unwrap())
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
            .current_dir(&fixture.root)
            .arg(source_driver.file_name().unwrap())
            .arg(package.file_name().unwrap())
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

#[test]
fn fused_source_admits_distinct_row_counts_widths_and_names_across_backends() {
    let fixture = Fixture::new("fused-source-alternate");
    let alternate = r#"module alternate.inventory;

@id("alternate.entry")
record Entry {
    @id("alternate.entry.rank") rank: i64,
    @id("alternate.entry.caption") caption: string,
    @id("alternate.entry.storage") storage: Bytes,
}

@id("alternate.entrypoint")
fn main() -> i64 {
    let mut pending = vec_with_capacity<Entry>(57usize);
    let mut sequence = 0;
    while sequence < 57 {
        let entry = Entry { rank: sequence, caption: "é\u{0}", storage: bytes_zeroed(5usize) };
        pending = vec_push<Entry>(pending, entry);
        sequence = sequence + 1;
        0
    }
    let entries = pending;
    let mut sum = 0;
    let mut intact = true;
    for borrow current at position in entries {
        let text = current.caption;
        let data = current.storage;
        let utf8 = str_as_bytes(text);
        intact = intact && current.rank == i64_from_usize(position)
            && str_len_bytes(text) == 3 && byte_len(data) == 5usize
            && match byte_get(utf8, 2usize) { Option::Some { value } => value == 0u8, Option::None {} => false, }
            && match byte_get(data, 4usize) { Option::Some { value } => value == 0u8, Option::None {} => false, };
        sum = sum + current.rank;
        0
    }
    if intact { sum } else { 0 }
}
"#;
    let input = fixture.root.join("inventory.spx");
    std::fs::write(&input, alternate).unwrap();
    let ordinary = cli(&fixture.root, &["run", "inventory.spx", "--json"]);
    assert!(!ordinary.status.success());
    assert!(String::from_utf8_lossy(&ordinary.stdout).contains("SPX-T267"));

    let options = [
        "--profile",
        "fused-construction-v1",
        "--managed-bytes",
        ACCOUNT,
    ];
    let mut args = vec!["run", "inventory.spx", "--json"];
    args.extend(options);
    let interpreter = packet(&success(&fixture.root, &args));
    args.push("--native");
    let native = packet(&success(&fixture.root, &args));
    let source_sha256 = "e55485602028059df63307ba83b4ba224db0980e3a2d6f6dd4c4044b745528ee";
    for (result, target) in [(interpreter, "interpreter"), (native, "native")] {
        assert_eq!(result["outcome"], "returned");
        assert_eq!(result["result"], "1596");
        assert_eq!(result["source_sha256"], source_sha256);
        assert_eq!(result["target"], target);
    }

    let mut build = vec![
        "build",
        "inventory.spx",
        "--target",
        "wasm",
        "--output",
        "package",
    ];
    build.extend(options);
    success(&fixture.root, &build);
    let output = Command::new("node")
        .current_dir(&fixture.root)
        .args(["--input-type=module", "-e"])
        .arg(
            r#"
import assert from 'node:assert/strict';
import {install} from './package/fused-construction-v1.mjs';
const runner = await install(Number(process.argv[1]));
for (let i = 0; i < 4; i++) {
    const result = runner.invoke();
    assert.equal(result.outcome, 'returned');
    assert.equal(result.result, '1596');
    assert.equal(result.target, 'wasm');
    assert.equal(result.source_sha256, process.argv[2]);
}
"#,
        )
        .args([ACCOUNT, source_sha256])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(std::fs::read_to_string(&input).unwrap(), alternate);

    for (changed, rule) in [
        (
            alternate.replace(
                "vec_with_capacity<Entry>(57usize)",
                "vec_with_capacity<Entry>(56usize)",
            ),
            "Rows",
        ),
        (
            alternate.replace("sequence = sequence + 1", "sequence = sequence + 2"),
            "LoopBody",
        ),
    ] {
        assert_ne!(changed, alternate);
        std::fs::write(&input, changed).unwrap();
        let mut invalid = vec![
            "build",
            "inventory.spx",
            "--target",
            "wasm",
            "--output",
            "refused",
            "--json",
        ];
        invalid.extend(options);
        let refused = cli(&fixture.root, &invalid);
        assert!(!refused.status.success());
        let diagnostic = packet(&refused);
        assert_eq!(diagnostic["code"], "SPX-F130");
        assert_eq!(
            diagnostic["message"],
            format!("selected source refused: {rule}")
        );
        assert!(!fixture.root.join("refused").exists());
    }
}
