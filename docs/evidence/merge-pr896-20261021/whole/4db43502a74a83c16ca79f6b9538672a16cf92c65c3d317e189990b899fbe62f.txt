//! Hosted observation of the unchanged native JSON fixture authentication prefix.
//! This does not execute derivation, native code, allocation assertions or backends.
#[test]
#[ignore = "explicit hosted JSON authentication diagnostic; not native qualification"]
fn native_json_codec_authentication_prefix_reports_ledger_and_preserves_failure() {
    use std::fs;
    let owner = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/native/string_settlement/codec_allocation.rs"
    ));
    let marker = "const SCHEMA: &str = r#\"";
    assert_eq!(owner.matches(marker).count(), 1);
    let schema = owner
        .split_once(marker)
        .unwrap()
        .1
        .split_once("\"#;")
        .unwrap()
        .0;
    assert_eq!(schema.matches("Vec<Row>").count(), 1);
    let schema = schema.replace("Vec<Row>", "Vec<string>");
    let canonical = |source: &str| {
        crate::format::canonical(&crate::parse(source, "codec-allocation.spx").unwrap())
    };
    let root = std::env::temp_dir().join(format!(
        "native-codec-allocation-{}-{}-0",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    fs::create_dir(&root).unwrap();
    let root = fs::canonicalize(root).unwrap();
    fs::create_dir(root.join("src")).unwrap();
    let manifest = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/native/string_settlement/bulk_utf8/semaprax.toml"
    ));
    fs::write(root.join("semaprax.toml"), manifest).unwrap();
    for (path, source) in [
        ("src/schema.spx", canonical(&schema)),
        (
            "src/app.spx",
            canonical("module consumer.app;@id(\"consumer.main\") fn main()->i64{0}"),
        ),
        (
            "src/tests.spx",
            canonical("module consumer.tests;@id(\"consumer.tests\") fn main()->i64{0}"),
        ),
    ] {
        eprintln!("JSON_PREFIX input {path} bytes={:?}", source.as_bytes());
        fs::write(root.join(path), source).unwrap();
    }
    eprintln!(
        "JSON_PREFIX original manifest bytes={:?}",
        manifest.as_bytes()
    );
    let _observation = super::observe();
    super::super::take_completed();
    super::super::take_last_completed();
    let result = std::panic::catch_unwind(|| {
        // The original native generated_v2 fixture authenticates these exact
        // initial files before its derive callback. Observe only that prefix.
        crate::project::with_authenticated_project(&root.join("semaprax.toml"), |_snapshot| Ok(()))
            .unwrap()
    });
    let maximum = super::super::take_completed();
    let last = super::super::take_last_completed();
    eprintln!("JSON_PREFIX unit-observation profile; no integration, derivation, native or application qualification");
    eprintln!("JSON_PREFIX maximum core trace: {maximum:?}");
    eprintln!("JSON_PREFIX final core trace: {last:?}");
    super::print_settled();
    let cleanup = fs::remove_dir_all(root);
    if let Err(original) = result {
        if let Err(error) = cleanup {
            eprintln!("JSON_PREFIX fixture cleanup error after selected compiler failure: {error}");
        }
        std::panic::resume_unwind(original);
    }
    cleanup.unwrap();
}
