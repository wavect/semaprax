use super::*;
use semaprax_rust_api_index::{RustApiIndex, MAX_INDEX_BYTES};

#[test]
fn indexed_prepare_writes_one_replayable_index_without_tool_execution() {
    let root = TestRoot::new();
    let input = root.0.join("extractor.json");
    let prepared = root.0.join("prepared.json");
    fs::write(
        &input,
        include_bytes!(
            "../../../semaprax-rust-api-index/fixtures/regex-1.13.1-index-envelope.json"
        ),
    )
    .unwrap();
    let first = binary()
        .arg("indexed-prepare")
        .arg("--extractor-output")
        .arg(&input)
        .arg("--output")
        .arg(&prepared)
        .output()
        .unwrap();
    assert!(first.status.success(), "{}", stderr(&first));
    assert!(first.stdout.is_empty());
    let bytes = fs::read(&prepared).unwrap();
    let index = RustApiIndex::replay(&bytes).unwrap();
    assert_eq!(index.package().name, "regex");
    assert_eq!(index.package().version, "1.13.1");
    assert!(index.select_supported(&["regex::Regex::is_match"]).is_ok());

    let repeat = binary()
        .arg("indexed-prepare")
        .arg("--extractor-output")
        .arg(&input)
        .arg("--output")
        .arg(&prepared)
        .output()
        .unwrap();
    assert!(!repeat.status.success());
    assert!(stderr(&repeat).contains("SPX-I233"));
    assert_eq!(fs::read(&prepared).unwrap(), bytes);

    let invalid = root.0.join("invalid.json");
    let absent = root.0.join("absent.json");
    fs::write(&invalid, &bytes).unwrap();
    let direct_index = binary()
        .arg("indexed-prepare")
        .arg("--extractor-output")
        .arg(&invalid)
        .arg("--output")
        .arg(&absent)
        .output()
        .unwrap();
    assert!(!direct_index.status.success());
    assert!(stderr(&direct_index).contains("SPX-B148"));
    assert!(!absent.exists());

    fs::write(&invalid, vec![b'x'; MAX_INDEX_BYTES + 1]).unwrap();
    let oversized = binary()
        .arg("indexed-prepare")
        .arg("--extractor-output")
        .arg(&invalid)
        .arg("--output")
        .arg(&absent)
        .output()
        .unwrap();
    assert!(!oversized.status.success());
    assert!(stderr(&oversized).contains("SPX-B148"));
    assert!(!absent.exists());
}
