//! Independent configuration/order schema; actual checked-source CLI, no host codec.
use super::*;

const EXAMPLE: &str = "examples/nested-order-json-project";
const PROFILE: project::JsonCodecProfile = project::JsonCodecProfile::NestedRequest {
    max_string_bytes: 16,
    max_array_items: 8,
};

fn install(label: &str) -> std::path::PathBuf {
    let root = super::super::temporary(label);
    std::fs::create_dir_all(root.join("src")).unwrap();
    let source = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(EXAMPLE);
    for path in ["semaprax.toml", "src/schema.spx", "src/app.spx"] {
        std::fs::copy(source.join(path), root.join(path)).unwrap();
    }
    let original = std::fs::read(root.join("src/schema.spx")).unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_semaprax"))
        .args([
            "json-codec",
            ".",
            "--source",
            "src/schema.spx",
            "--type",
            "orders.request",
            "--profile",
            "bounded-nested-request.v1",
            "--max-string-bytes",
            "16",
            "--max-array-items",
            "8",
            "--output",
            "src/schema.generated.spx",
        ])
        .current_dir(&root)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        std::fs::read(root.join("src/schema.spx")).unwrap(),
        original
    );
    let generated = std::fs::read_to_string(root.join("src/schema.generated.spx")).unwrap();
    assert_eq!(canonical(&generated), generated);
    project::with_authenticated_project(&root.join("semaprax.toml"), |snapshot| {
        let revision = snapshot.retain_revision();
        project::verify_json_codec_source_with_profile(
            &revision,
            "src/schema.spx",
            "orders.request",
            &generated,
            PROFILE,
        )?;
        for changed in [
            project::JsonCodecProfile::NestedRequest {
                max_string_bytes: 15,
                max_array_items: 8,
            },
            project::JsonCodecProfile::NestedRequest {
                max_string_bytes: 16,
                max_array_items: 7,
            },
        ] {
            assert_eq!(
                project::verify_json_codec_source_with_profile(
                    &revision,
                    "src/schema.spx",
                    "orders.request",
                    &generated,
                    changed
                )
                .unwrap_err()[0]
                    .code,
                "SPX-J180"
            );
        }
        Ok(())
    })
    .unwrap();
    std::fs::copy(
        root.join("src/schema.generated.spx"),
        root.join("src/schema.spx"),
    )
    .unwrap();
    std::fs::write(
        root.join("src/app.spx"),
        canonical(&std::fs::read_to_string(source.join("src/app.consumer.spx")).unwrap()),
    )
    .unwrap();
    root
}

fn qualify(root: &std::path::Path) {
    project::with_authenticated_project(&root.join("semaprax.toml"), |snapshot| {
        semaprax::hir::validate(snapshot.entry_program()).map_err(|error| vec![error])?;
        assert!(snapshot
            .retain_revision()
            .semantic_graph()
            .contains("orders.request.json.nested.decode"));
        let options = project::ProjectExecutionOptions::new(16 * 1024 * 1024, 160_000_000)
            .map_err(|error| vec![error])?;
        assert_eq!(
            snapshot.execute_entry(&options)?.outcome(),
            &project::ProjectExecutionOutcome::Returned(729)
        );
        let c = codegen::emit_hir_c(snapshot.entry_program()).map_err(|error| vec![error])?;
        for optimization in ["-O0", "-O2"] {
            super::super::compile_and_run_c(&c, root, optimization, "729");
        }
        let bytes =
            wasm::emit_resolved_module(snapshot.entry_program()).map_err(|error| vec![error])?;
        wasmparser::Validator::new().validate_all(&bytes).unwrap();
        std::fs::write(root.join("app.wasm"), bytes).unwrap();
        Ok(())
    })
    .unwrap();
    let host = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/owned_data/owned_leaf_vec/host.js");
    let output = Command::new("node")
        .arg(host)
        .arg(root.join("app.wasm"))
        .args(["0", "729", "none"])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn nested_order_example_derives_detached_real_owners_and_agrees_on_three_backends() {
    let root = install("nested-order-public-example");
    qualify(&root);
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn nested_order_errors_and_exact_schema_capacities_agree_on_three_backends() {
    let root = install("nested-order-errors");
    let source = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(EXAMPLE);
    let mut app = r#"module orders.app;
use type @id("orders.request.json.nested.decode-result") from orders.schema as Outcome;
use function @id("orders.request.json.nested.decode") from orders.schema as decode;
@id("orders.error") fn error(input:borrow Slice<u8>,limit:usize,wanted:i64,at:usize,target:i64)->i64 {
let result=decode(input,limit);match own result {
Outcome::Ready{value}=>1,
Outcome::Error{code,offset,field}=>if code==wanted && offset==at && field==target{0}else{1},
}}
@id("orders.main") fn main()->i64 { let mut failures=0;
"#.to_owned();
    for (index, (file, code, offset, field)) in [
        ("malformed.json", 1, 93, 0),
        ("duplicate.json", 2, 31, 2),
        ("string-capacity.json", 6, 26, 2),
        ("array-capacity.json", 8, 251, 4),
    ]
    .into_iter()
    .enumerate()
    {
        let data = std::fs::read(source.join("fixtures").join(file)).unwrap();
        app.push_str(&format!("let bad_{index}={};failures=failures+error(array_as_slice(bad_{index}),4096usize,{code},{offset}usize,{field});\n", array(&data)));
    }
    for (index, (data, code, offset, field)) in [
        (br#"{"unknown":0}"#.as_slice(), 4, 1, 0),
        (b"{}", 3, 2, 1),
        (br#"{"configuration":false}"#, 5, 17, 1),
        (br#"{"configuration":{"label":"x","retry":-1}}"#, 6, 38, 3),
        (
            br#"{"configuration":{"label":"x","retry":0},"lines":[{"sku":"x","quantity":256}]}"#,
            6,
            72,
            6,
        ),
    ]
    .into_iter()
    .enumerate()
    {
        app.push_str(&format!("let scalar_{index}={};failures=failures+error(array_as_slice(scalar_{index}),4096usize,{code},{offset}usize,{field});\n", array(data)));
    }
    let valid = std::fs::read(source.join("fixtures/request.json")).unwrap();
    app.push_str(&format!("let exact={};let success=decode(array_as_slice(exact),{}usize);failures=failures+match own success{{Outcome::Ready{{value}}=>0,Outcome::Error{{code,offset,field}}=>1,}};failures=failures+error(array_as_slice(exact),{}usize,7,0usize,0);\n",array(&valid),valid.len(),valid.len()-1));
    let row = r#"{"sku":"1234567890123456","quantity":255}"#;
    let eight = format!(
        r#"{{"configuration":{{"label":"1234567890123456","retry":18446744073709551615}},"lines":[{}],"urgent":false}}"#,
        std::iter::repeat_n(row, 8).collect::<Vec<_>>().join(",")
    );
    let empty = br#"{"configuration":{"label":"","retry":0},"lines":[],"urgent":false}"#;
    for (index, bytes) in [eight.as_bytes(), empty.as_slice()].into_iter().enumerate() {
        app.push_str(&format!("let boundary_{index}={};let result_{index}=decode(array_as_slice(boundary_{index}),{}usize);failures=failures+match own result_{index}{{Outcome::Ready{{value}}=>0,Outcome::Error{{code,offset,field}}=>1,}};\n",array(bytes),bytes.len()));
    }
    app.push_str("if failures==0{729}else{0-failures}\n}");
    std::fs::write(root.join("src/app.spx"), canonical(&app)).unwrap();
    qualify(&root);
    std::fs::remove_dir_all(root).unwrap();
}
