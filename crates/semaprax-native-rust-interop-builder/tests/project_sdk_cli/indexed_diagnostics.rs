use super::*;

fn fixture(root: &TestRoot) -> (PathBuf, PathBuf) {
    fs::create_dir(root.0.join("src")).unwrap();
    let source = r#"module test.rustc_diagnostics;
@id("rust.host") interface RustHost permits {  } {
    @id("rust.host.selected") import rust selected fn selected from "local_api_fixture::cfg_selected" effects {  } failure infallible;
}
@id("rust.host.main") fn main() -> i64 { 0 }
"#;
    fs::write(root.0.join("src/app.spx"), source).unwrap();
    let manifest = root.0.join("semaprax.toml");
    fs::write(&manifest, "[package]\nname = \"diagnostic-fixture\"\n").unwrap();
    let index = root.0.join("index.json");
    fs::write(
        &index,
        include_bytes!(
            "../../../semaprax-rust-api-index/fixtures/local-api-fixture-prepared-v2.json"
        ),
    )
    .unwrap();
    let package = root.0.join("package.rs");
    fs::write(&package, "pub fn selected(value:u8)->u8{value}\n").unwrap();
    let selections = root.0.join("selections.json");
    fs::write(&selections, serde_json::json!({
        "schema": "semaprax.indexed-project-selection.v1",
        "selections": [{"source_path":"src/app.spx", "import_id":"rust.host.selected", "index_path":index, "package_source_path":package}]
    }).to_string()).unwrap();
    (manifest, selections)
}

#[test]
fn indexed_diagnostics_maps_real_rustc_requirements_to_one_import() {
    let root = TestRoot::new();
    let (manifest, selections) = fixture(&root);
    let generated = root.0.join("generated_wrapper.rs");
    let captured = root.0.join("rustc.jsonl");
    let rustc = std::env::var("RUSTC").unwrap_or_else(|_| "rustc".to_owned());
    for (source, rust_code, mapped_code) in [
        ("trait Needed {}\nfn selected<T:Needed>(_:T){}\npub fn wrapper(){selected(1_i64);}\n", "E0277", "SPX-B150"),
        ("mod api { #[cfg(feature=\"needed\")] pub fn selected(){} }\npub fn wrapper(){api::selected();}\n", "E0425", "SPX-B151"),
        ("fn get<'a>(value:&'a str, other:&str)->&'a str{other}\n", "E0621", "SPX-B152"),
    ] {
        fs::write(&generated, source).unwrap();
        let output = Command::new(&rustc)
            .args(["--crate-type=lib", "--edition=2021", "--error-format=json", "--emit=metadata"])
            .arg(&generated)
            .arg("-o")
            .arg(root.0.join("never.rmeta"))
            .output()
            .unwrap();
        assert!(!output.status.success());
        let rustc_output = String::from_utf8(output.stderr).unwrap();
        assert!(rustc_output.contains(rust_code), "{rustc_output}");
        fs::write(&captured, &rustc_output).unwrap();
        let report = binary()
            .arg("indexed-diagnostics")
            .arg("--manifest-path").arg(&manifest)
            .arg("--selections").arg(&selections)
            .arg("--rustc-json").arg(&captured)
            .arg("--generated-file").arg(&generated)
            .output().unwrap();
        assert!(report.status.success(), "{}", stderr(&report));
        let value: serde_json::Value = serde_json::from_slice(&report.stdout).unwrap();
        assert_eq!(value["schema"], "semaprax.indexed-rustc-diagnostics.v1");
        assert_eq!(value["capture_status"], "external_unverified");
        assert_eq!(value["authority"]["tool_invocation"], false);
        assert_eq!(value["diagnostics"][0]["code"], mapped_code);
        assert_eq!(value["diagnostics"][0]["rustc"]["code"], rust_code);
        assert_eq!(value["diagnostics"][0]["source"]["path"], "src/app.spx");
        assert!(value["diagnostics"][0]["source"]["start"].as_u64().unwrap() > 0);
        assert!(value["diagnostics"][0]["message"].as_str().unwrap().contains("Rust"));
        assert!(value["identity"]["selected_stable_rustc"].as_str().unwrap().starts_with("rustc "));

        let wrong_file = binary()
            .arg("indexed-diagnostics")
            .arg("--manifest-path").arg(&manifest)
            .arg("--selections").arg(&selections)
            .arg("--rustc-json").arg(&captured)
            .arg("--generated-file").arg(root.0.join("different.rs"))
            .output().unwrap();
        assert!(!wrong_file.status.success());
        assert!(stderr(&wrong_file).contains("SPX-B149"));
    }

    fs::write(
        &captured,
        vec![b'x'; semaprax_native_rust_interop::rustc_diagnostics::MAX_RUSTC_JSON_BYTES + 1],
    )
    .unwrap();
    let oversized = binary()
        .arg("indexed-diagnostics")
        .arg("--manifest-path")
        .arg(&manifest)
        .arg("--selections")
        .arg(&selections)
        .arg("--rustc-json")
        .arg(&captured)
        .arg("--generated-file")
        .arg(&generated)
        .output()
        .unwrap();
    assert!(!oversized.status.success());
    assert!(stderr(&oversized).contains("SPX-B149"));
    assert!(oversized.stdout.is_empty());
}
