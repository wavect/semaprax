use super::*;

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

    let failure = source.replace("bytes_set(item, 1usize", "bytes_set(item, 2usize");
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
