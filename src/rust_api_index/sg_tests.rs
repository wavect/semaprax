#[test]
fn sg20_sg21_actual_rustdoc_extraction_replays_with_payload_and_alias() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let repo = if root.join("crates/semaprax-rust-api-index").is_dir() { root.to_path_buf() } else { root.join("../..").canonicalize().unwrap() };
    let fixture = repo.join("crates/semaprax-rust-api-index/fixtures/sg-format61");
    let captures: Value = serde_json::from_slice(&std::fs::read(fixture.join("capture-results.json")).unwrap()).unwrap();
    let args = captures.as_array().unwrap().iter().find(|row| row["name"] == "full").unwrap()["args"].as_array().unwrap();
    let mut command = std::process::Command::new(std::env::var("HARNESS_PYTHON").unwrap_or_else(|_| "python3".into()));
    command.arg(repo.join("crates/semaprax-rust-api-index/tools/rustdoc_json_to_index.py"));
    for pair in args[2..].chunks_exact(2) {
        let flag = pair[0].as_str().unwrap();
        if flag == "--output" { continue; }
        command.arg(flag);
        if flag == "--rustdoc-json" { command.arg(fixture.join("review_index.json")); }
        else { command.arg(pair[1].as_str().unwrap()); }
    }
    let output = command.output().unwrap();
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
    let index = RustApiIndex::admit_extractor_output(&output.stdout).unwrap();
    let replayed = RustApiIndex::replay(index.canonical_json().as_bytes()).unwrap();
    let event = replayed.items().iter().find(|row| row.path == "review_index::make_event").unwrap();
    assert!(event.closure_complete);
    assert_eq!(event.reachable_types, ["review_index::Event", "review_index::Payload"]);
    let alias = replayed.items().iter().find(|row| row.path == "review_index::public_api::increment").unwrap();
    assert_eq!(alias.support, Support::Supported);
}
