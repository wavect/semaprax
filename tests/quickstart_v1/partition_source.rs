use super::*;

fn precondition_before_bad_store(source: &str) -> String {
    source
        .replace("fn main() -> i64", "fn main() -> i64 requires false")
        .replace("bytes_set(item, 1usize", "bytes_set(item, 2usize")
}

#[test]
fn explicit_partition_source_run_publishes_scalar_and_failure_after_cleanup() {
    let fixture = Fixture::new("partition-source");
    let path = fixture.root.join("input.spx");
    let source = include_str!("../fixtures/byte-partition-private-clone.spx");
    let invoke = |account: &str| {
        cli(
            &fixture.root,
            &[
                "run",
                "input.spx",
                "--profile",
                "byte-partition-v1",
                "--managed-bytes",
                account,
                "--json",
            ],
        )
    };
    let packet = |output: &Output| -> serde_json::Value {
        serde_json::from_slice(&output.stdout)
            .unwrap_or_else(|error| panic!("invalid report: {error}; {output:?}"))
    };
    std::fs::write(&path, source).unwrap();
    let ordinary = cli(&fixture.root, &["run", "input.spx", "--json"]);
    assert!(!ordinary.status.success());
    for args in [
        vec!["run", "input.spx", "--profile", "byte-partition-v1"],
        vec![
            "run",
            "--profile",
            "byte-partition-v1",
            "--managed-bytes",
            "8388608",
        ],
        vec![
            "run",
            "input.spx",
            "--profile",
            "byte-partition-v1",
            "--managed-bytes",
            "8388608",
            "--max-steps",
            "10",
        ],
    ] {
        assert_eq!(cli(&fixture.root, &args).status.code(), Some(2));
    }
    let output = invoke("8388608");
    assert!(output.status.success(), "{output:?}");
    let report = packet(&output);
    assert_eq!(report["schema"], "semaprax.interpret.byte-partition.v1");
    assert_eq!(report["result"], "1");
    assert_eq!(report["backing_allocations"], 2);
    assert_eq!(report["payload_bytes"], 8);
    assert_eq!(report["cleanup_complete"], true);
    assert_eq!(output.stdout, invoke("8388608").stdout);
    assert_eq!(std::fs::read_to_string(&path).unwrap(), source);

    let reserved = report["managed_reserved"].as_u64().unwrap();
    assert!(invoke(&reserved.to_string()).status.success());
    let short = invoke(&(reserved - 1).to_string());
    assert_eq!(short.status.code(), Some(1));
    let short = packet(&short);
    assert_eq!(short["outcome"], "administrative_refusal");
    assert!(short.get("result").is_none());

    let allowed = source.replace("fn main() -> i64", "fn main() -> i64 requires true");
    std::fs::write(&path, allowed).unwrap();
    let output = invoke("8388608");
    assert!(output.status.success(), "{output:?}");
    let report = packet(&output);
    assert_eq!(report["result"], "1");
    assert_eq!(report["backing_allocations"], 2);

    std::fs::write(&path, precondition_before_bad_store(source)).unwrap();
    let output = invoke("8388608");
    assert_eq!(output.status.code(), Some(1));
    let report = packet(&output);
    assert_eq!(
        report["status"],
        serde_json::json!({"domain": "semaprax.contract.v1", "code": 1})
    );
    assert_eq!(report["backing_allocations"], 0);
    assert_eq!(report["payload_bytes"], 0);
    assert_eq!(report["cleanup_complete"], true);
    assert!(report.get("result").is_none());

    let postcondition = source.replace("fn main() -> i64", "fn main() -> i64 ensures result == 0");
    std::fs::write(&path, &postcondition).unwrap();
    let output = invoke("8388608");
    assert_eq!(output.status.code(), Some(1));
    let report = packet(&output);
    assert_eq!(
        report["status"],
        serde_json::json!({"domain": "semaprax.contract.v1", "code": 2})
    );
    assert_eq!(report["cleanup_complete"], true);
    assert!(report.get("result").is_none());

    let failure = postcondition.replace("bytes_set(item, 1usize", "bytes_set(item, 2usize");
    std::fs::write(&path, &failure).unwrap();
    let output = invoke("8388608");
    assert_eq!(output.status.code(), Some(1));
    let report = packet(&output);
    assert_eq!(
        report["status"],
        serde_json::json!({"domain": "semaprax.byte-buffer.v1", "code": 1})
    );
    assert_eq!(report["cleanup_complete"], true);
    assert!(report.get("result").is_none());
    assert_eq!(std::fs::read_to_string(path).unwrap(), failure);
}

#[test]
fn explicit_partition_source_build_runs_native_and_refuses_other_targets() {
    let fixture = Fixture::new("partition-native");
    let path = fixture.root.join("input.spx");
    let source = include_str!("../fixtures/byte-partition-private-clone.spx");
    let build = |output: &str| {
        cli(
            &fixture.root,
            &[
                "build",
                "input.spx",
                "--profile",
                "byte-partition-v1",
                "--target",
                "native",
                "--output",
                output,
            ],
        )
    };
    let run = |name: &str| Command::new(fixture.root.join(name)).output().unwrap();
    std::fs::write(&path, source).unwrap();
    let unsupported = cli(
        &fixture.root,
        &[
            "build",
            "input.spx",
            "--profile",
            "byte-partition-v1",
            "--target",
            "web",
            "--output",
            "unsupported",
        ],
    );
    assert_eq!(unsupported.status.code(), Some(2));
    assert!(!fixture.root.join("unsupported").exists());
    let ordinary = cli(
        &fixture.root,
        &["build", "input.spx", "--output", "ordinary"],
    );
    assert!(!ordinary.status.success());
    assert!(!fixture.root.join("ordinary").exists());

    let output = build("valid");
    assert!(output.status.success(), "{output:?}");
    let output = run("valid");
    assert!(output.status.success(), "{output:?}");
    assert_eq!(output.stdout, b"1\n");
    assert_eq!(std::fs::read_to_string(&path).unwrap(), source);

    std::fs::write(&path, precondition_before_bad_store(source)).unwrap();
    let output = build("precondition");
    assert!(output.status.success(), "{output:?}");
    let output = run("precondition");
    assert_eq!(output.status.code(), Some(1));
    assert_eq!(output.stderr, b"semaprax.contract.v1:1\n");
    assert!(output.stdout.is_empty());

    let postcondition = source.replace("fn main() -> i64", "fn main() -> i64 ensures result == 0");
    std::fs::write(&path, &postcondition).unwrap();
    let output = build("postcondition");
    assert!(output.status.success(), "{output:?}");
    for _ in 0..2 {
        let output = run("postcondition");
        assert_eq!(output.status.code(), Some(1));
        assert_eq!(output.stderr, b"semaprax.contract.v1:2\n");
        assert!(output.stdout.is_empty());
    }

    let failure = postcondition.replace("bytes_set(item, 1usize", "bytes_set(item, 2usize");
    std::fs::write(&path, &failure).unwrap();
    let output = build("failure");
    assert!(output.status.success(), "{output:?}");
    let output = run("failure");
    assert_eq!(output.status.code(), Some(1));
    assert_eq!(output.stderr, b"semaprax.byte-buffer.v1:1\n");
    assert!(output.stdout.is_empty());
    assert_eq!(std::fs::read_to_string(path).unwrap(), failure);
}

#[test]
fn explicit_partition_source_build_publishes_paired_wasm_runner() {
    let fixture = Fixture::new("partition-wasm");
    let source = include_str!("../fixtures/byte-partition-private-clone.spx");
    let path = fixture.root.join("input.spx");
    std::fs::write(&path, source).unwrap();
    let build = |destination: &str| {
        cli(
            &fixture.root,
            &[
                "build",
                "input.spx",
                "--profile",
                "byte-partition-v1",
                "--target",
                "wasm",
                "--output",
                destination,
            ],
        )
    };
    assert!(build("first").status.success());
    assert!(build("second").status.success());
    for leaf in [
        "byte-partition-v1.json",
        "entry.mjs",
        "host.mjs",
        "registry.mjs",
    ] {
        let first = std::fs::read(fixture.root.join("first").join(leaf)).unwrap();
        assert_eq!(
            first,
            std::fs::read(fixture.root.join("second").join(leaf)).unwrap()
        );
    }
    assert!(!build("first").status.success());
    assert_eq!(std::fs::read_to_string(&path).unwrap(), source);
    std::fs::write(&path, precondition_before_bad_store(source)).unwrap();
    assert!(build("precondition").status.success());
    let postcondition = source.replace("fn main() -> i64", "fn main() -> i64 ensures result == 0");
    std::fs::write(&path, &postcondition).unwrap();
    assert!(build("postcondition").status.success());
    let failure = postcondition.replace("bytes_set(item, 1usize", "bytes_set(item, 2usize");
    std::fs::write(&path, &failure).unwrap();
    assert!(build("failure").status.success());

    if Command::new("node")
        .arg("--version")
        .output()
        .is_ok_and(|r| r.status.success())
    {
        for package in ["first", "precondition", "postcondition", "failure"] {
            let output = Command::new("node")
                .current_dir(&fixture.root)
                .args([
                    "--input-type=module",
                    "-e",
                    r#"
import assert from 'node:assert/strict';
const packageName = process.argv[1];
const { install } = await import(`./${packageName}/entry.mjs`);
const runner = await install(8388608);
const baseline = runner.account();
const expected = {
  first: {outcome:'returned', result:'1'},
  precondition: {outcome:'language_failure', status:{domain:'semaprax.contract.v1', code:1}},
  postcondition: {outcome:'language_failure', status:{domain:'semaprax.contract.v1', code:2}},
  failure: {outcome:'language_failure', status:{domain:'semaprax.byte-buffer.v1', code:1}},
}[packageName];
for (let n = 0; n < 2; n++) {
  assert.deepEqual(runner.invoke(), expected);
  assert.equal(runner.account().reserved, baseline.reserved);
  if (packageName === "precondition") assert.equal(runner.account().peak, baseline.peak);
}
"#,
                    package,
                ])
                .output()
                .unwrap();
            assert!(output.status.success(), "{package}: {output:?}");
        }
    }
}
