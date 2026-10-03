use super::*;
use std::{
    fs,
    process::{Command, Output},
};

const SOURCE: &str = r#"module demanded.fixture;
@id("demand.host") interface Host permits { } {
 @id("demand.three") import rust fn three(value: i64) -> i64 effects { } failure infallible;
 @id("demand.five") import rust fn five(value: i64) -> i64 effects { } failure infallible;
 @id("demand.again") import rust fn again(value: i64) -> i64 effects { } failure infallible;
 @id("demand.assoc") import rust fn associated(value: i64) -> i64 effects { } failure infallible;
}
@id("demand.run") fn run(value: i64) -> i64 {
 three(value) + five(value) + again(value) + associated(value)
}
@id("demand.main") fn main() -> i64 { 0 }
"#;
const RUST: &str = r#"
pub fn multiply<const N:usize>(value:i64)->i64 { value * N as i64 }
pub trait Measures { type Output; fn measure(value:i64)->Self::Output; }
pub struct Meter;
impl Measures for Meter { type Output=i64; fn measure(value:i64)->i64 {value+1} }
pub fn measured<T:Measures>(value:i64)->T::Output {T::measure(value)}
pub fn iterator_bound<T:Iterator>(value:i64)->i64 {value}
"#;
struct Fixture {
    index: Vec<u8>,
    source_digest: String,
    rustc_version: String,
}
impl Fixture {
    fn new(rustc: &str) -> Self {
        let version = Command::new(rustc).arg("--version").output().unwrap();
        assert!(version.status.success());
        let rustc_version = String::from_utf8(version.stdout).unwrap().trim().to_owned();
        let source_digest = raw_digest(RUST.as_bytes());
        let mut envelope: Value = serde_json::from_slice(include_bytes!(
            "../../../semaprax-rust-api-index/fixtures/local-api-fixture-v2-envelope.json"
        ))
        .unwrap();
        let index = &mut envelope["index"];
        index["package"]["name"] = "fixture_math".into();
        index["package"]["version"] = "0.0.1".into();
        index["package"]["source_sha256"] = source_digest.clone().into();
        index["target"] = target_triple().unwrap().into();
        index["stable_rustc_version"] = rustc_version.clone().into();
        let base = index["items"]
            .as_array()
            .unwrap()
            .iter()
            .find(|v| v["path"] == "local_api_fixture::cfg_selected")
            .unwrap()
            .clone();
        let make = |name: &str, signature: &str, kind: &str, const_type: Value| {
            let mut item = base.clone();
            item["path"] = format!("fixture_math::{name}").into();
            item["signature"] = signature.into();
            item["support"] = "rejected".into();
            item["reason"] = "unsupported_generic".into();
            item["type_roots"] = serde_json::json!([]);
            item["reachable_types"] = serde_json::json!([]);
            item["type_closure_depth"] = 0.into();
            item["closure_complete"] = true.into();
            item["generics"] = serde_json::json!({"parameters":[{"bounds":[],"const_type":const_type,"default":null,"kind":kind,"name":if kind=="const"{"N"}else{"T"}}],"where_predicates":[]});
            item
        };
        let mut associated = index["items"]
            .as_array()
            .unwrap()
            .iter()
            .find(|v| v["path"] == "local_api_fixture::Measures::Output")
            .unwrap()
            .clone();
        associated["path"] = "fixture_math::Measures::Output".into();
        associated["type_roots"] = serde_json::json!([]);
        associated["reachable_types"] = serde_json::json!([]);
        associated["type_closure_depth"] = 0.into();
        associated["closure_complete"] = true.into();
        index["items"] = serde_json::json!([
            associated,
            make(
                "iterator_bound",
                "fn iterator_bound<T: Iterator>(value: i64) -> i64",
                "type",
                Value::Null
            ),
            make(
                "measured",
                "fn measured<T: Measures>(value: i64) -> T::Output",
                "type",
                Value::Null
            ),
            make(
                "multiply",
                "fn multiply<const N: usize>(value: i64) -> i64",
                "const",
                "usize".into()
            )
        ]);
        let mut ty = index["types"]
            .as_array()
            .unwrap()
            .iter()
            .find(|v| v["path"] == "local_api_fixture::MacroGenerated")
            .unwrap()
            .clone();
        ty["path"] = "fixture_math::Meter".into();
        index["types"] = serde_json::json!([ty]);
        let mut bytes = serde_json::to_vec(&envelope).unwrap();
        bytes.push(b'\n');
        let index = RustApiIndex::admit_extractor_output(&bytes)
            .unwrap()
            .canonical_json()
            .as_bytes()
            .to_vec();
        Self {
            index,
            source_digest,
            rustc_version,
        }
    }
    fn package(&self) -> SelectedPackage<'_> {
        SelectedPackage {
            cargo_alias: "fixture_math",
            name: "fixture_math",
            version: "0.0.1",
            source_sha256: &self.source_digest,
            target: target_triple().unwrap(),
            feature_digest:
                "sha256:e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
            stable_rustc_version: &self.rustc_version,
        }
    }
}
fn options() -> NativeRustSdkOptions {
    NativeRustSdkOptions {
        exports: vec!["demand.run".into()],
        imports: vec![
            "demand.three".into(),
            "demand.five".into(),
            "demand.again".into(),
            "demand.assoc".into(),
        ],
        capabilities: vec![],
    }
}
fn requests() -> Vec<RustDemandSelection> {
    let repeated = |id: &str, n: &str| RustDemandSelection {
        import_id: id.into(),
        request: InstantiationRequest {
            item_path: "fixture_math::multiply".into(),
            type_arguments: vec![],
            const_arguments: vec![ConstArgument::decimal("usize", n).unwrap()],
        },
        associated_result: None,
    };
    vec![
        repeated("demand.three", "3"),
        repeated("demand.five", "5"),
        repeated("demand.again", "3"),
        RustDemandSelection {
            import_id: "demand.assoc".into(),
            request: InstantiationRequest {
                item_path: "fixture_math::measured".into(),
                type_arguments: vec![ConcreteType::parse("fixture_math::Meter").unwrap()],
                const_arguments: vec![],
            },
            associated_result: Some(AssociatedTypeRequest {
                associated_type_path: "fixture_math::Measures::Output".into(),
                implementor: ConcreteType::parse("fixture_math::Meter").unwrap(),
            }),
        },
    ]
}
struct Temp(PathBuf);
impl Drop for Temp {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}
fn succeeded(output: Output) -> Output {
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    output
}

#[test]
fn demanded_bindings_execute_checked_semaprax_calls_and_map_trait_refusal() {
    let rustc = std::env::var("RUSTC").expect("configure absolute RUSTC for demanded binding gate");
    let clang = std::env::var("CLANG").expect("configure absolute CLANG for demanded binding gate");
    assert!(Path::new(&rustc).is_absolute() && Path::new(&clang).is_absolute());
    let fixture = Fixture::new(&rustc);
    let index = RustApiIndex::replay(&fixture.index).unwrap();
    let mut package = fixture.package();
    package.feature_digest = index.feature_digest();
    let selected = requests();
    let generated = prepare_demanded_native_rust(
        SOURCE,
        Path::new("demanded.spx"),
        options(),
        &fixture.index,
        package,
        &selected,
    )
    .unwrap();
    let canonical = semaprax::format::canonical(&semaprax::check(SOURCE, "demanded.spx").unwrap());
    let replay = prepare_demanded_native_rust(
        &canonical,
        Path::new("demanded.spx"),
        options(),
        &fixture.index,
        package,
        &selected,
    )
    .unwrap();
    assert_eq!(generated.c_source, replay.c_source);
    assert_eq!(generated.rust_adapter, replay.rust_adapter);
    let plan = |id: &str| {
        generated
            .bindings
            .iter()
            .find(|p| p.import_id == id)
            .unwrap()
    };
    assert_eq!(
        plan("demand.three").physical_symbol,
        plan("demand.again").physical_symbol
    );
    assert_ne!(
        plan("demand.three").physical_symbol,
        plan("demand.five").physical_symbol
    );
    assert_eq!(generated.rust_adapter.matches("fn spx_ri07_").count(), 3);
    assert_eq!(plan("demand.assoc").use_sites.len(), 1);
    assert!(generated
        .rust_adapter
        .contains("<fixture_math::Meter as fixture_math::Measures>::Output"));
    let root = Temp(std::env::temp_dir().join(format!("semaprax-demanded-{}", std::process::id())));
    fs::create_dir(&root.0).unwrap();
    fs::write(root.0.join("fixture.rs"), RUST).unwrap();
    succeeded(
        Command::new(&rustc)
            .current_dir(&root.0)
            .args([
                "--edition=2021",
                "--crate-name=fixture_math",
                "--crate-type=rlib",
                "fixture.rs",
                "-o",
                "libfixture_math.rlib",
            ])
            .output()
            .unwrap(),
    );
    fs::write(
        root.0.join("semaprax_native_rust_interop.h"),
        &generated.header,
    )
    .unwrap();
    fs::write(
        root.0.join("semaprax_native_rust_interop_ffi.rs"),
        &generated.ffi_rust,
    )
    .unwrap();
    fs::write(root.0.join("adapter.rs"), &generated.rust_adapter).unwrap();
    let export = serde_json::from_str::<Value>(&generated.descriptor).unwrap()["exports"][0]
        ["rust_method"]
        .as_str()
        .unwrap()
        .to_owned();
    let consumer=format!("{}\ninclude!(\"adapter.rs\");\nfn main(){{let c=NativeRustCapabilities::new(&[]).unwrap_or_else(|_|panic!(\"admission\"));let mut b=NativeRustBridge::new(DemandedRustHost,c);assert_eq!(b.{export}(7).unwrap_or_else(|_|panic!(\"call\")),85);assert_eq!(b.{export}(0).unwrap_or_else(|_|panic!(\"call\")),1);}}",generated.safe_rust);
    fs::write(root.0.join("main.rs"), consumer).unwrap();
    for (name, opt, c_source, expected) in [
        ("o0", "-O0", generated.c_source.clone(), true),
        ("o2", "-O2", generated.c_source.clone(), true),
    ] {
        fs::write(root.0.join("module.c"), c_source).unwrap();
        succeeded(
            Command::new(&clang)
                .current_dir(&root.0)
                .args(["-std=c11", opt, "-c", "module.c", "-o", "module.o"])
                .output()
                .unwrap(),
        );
        succeeded(
            Command::new(&rustc)
                .current_dir(&root.0)
                .args([
                    "--edition=2021",
                    "main.rs",
                    "--extern",
                    "fixture_math=libfixture_math.rlib",
                    "-C",
                    "link-arg=module.o",
                    "-o",
                    name,
                ])
                .output()
                .unwrap(),
        );
        assert_eq!(
            Command::new(root.0.join(name))
                .output()
                .unwrap()
                .status
                .success(),
            expected
        );
    }
    let flipped = generated
        .rust_adapter
        .replace("multiply::<5>", "multiply::<4>");
    assert_ne!(flipped, generated.rust_adapter);
    fs::write(root.0.join("adapter.rs"), flipped).unwrap();
    succeeded(
        Command::new(&rustc)
            .current_dir(&root.0)
            .args([
                "--edition=2021",
                "main.rs",
                "--extern",
                "fixture_math=libfixture_math.rlib",
                "-C",
                "link-arg=module.o",
                "-o",
                "flipped",
            ])
            .output()
            .unwrap(),
    );
    assert!(!Command::new(root.0.join("flipped"))
        .output()
        .unwrap()
        .status
        .success());
    let mut bad = selected.clone();
    bad[0].request = InstantiationRequest {
        item_path: "fixture_math::iterator_bound".into(),
        type_arguments: vec![ConcreteType::parse("i64").unwrap()],
        const_arguments: vec![],
    };
    let rejected = prepare_demanded_native_rust(
        SOURCE,
        Path::new("demanded.spx"),
        options(),
        &fixture.index,
        package,
        &bad,
    )
    .unwrap();
    fs::write(root.0.join("adapter.rs"), &rejected.rust_adapter).unwrap();
    let failure = Command::new(&rustc)
        .current_dir(&root.0)
        .args([
            "--edition=2021",
            "--error-format=json",
            "main.rs",
            "--extern",
            "fixture_math=libfixture_math.rlib",
            "-C",
            "link-arg=module.o",
            "-o",
            "bad",
        ])
        .output()
        .unwrap();
    assert!(!failure.status.success());
    let diagnostics = rejected
        .map_captured_rustc_errors("adapter.rs", &failure.stderr)
        .unwrap();
    assert!(
        !diagnostics.is_empty(),
        "{}",
        String::from_utf8_lossy(&failure.stderr)
    );
    assert!(diagnostics.iter().all(|d| d.code == "SPX-B150"
        && d.path.as_deref() == Some("demanded.spx")
        && d.message.contains("Iterator")
        && d.message.contains("fixture_math::iterator_bound")));
    let span = diagnostics[0].span.unwrap();
    assert!(SOURCE[span.start..span.end].starts_with("three("));
    assert!(rejected
        .map_captured_rustc_errors("wrong-file.rs", &failure.stderr)
        .unwrap()
        .is_empty());
    bad = selected.clone();
    bad[0].request.const_arguments[0].value = "3; panic!()".into();
    assert_eq!(
        prepare_demanded_native_rust(
            SOURCE,
            Path::new("demanded.spx"),
            options(),
            &fixture.index,
            package,
            &bad
        )
        .unwrap_err()[0]
            .code,
        "SPX-B153"
    );
    // Every hostility document is itself a valid index; refusal must come
    // from selected demand admission, not a malformed-fixture accident.
    for (path, reason, private, unsafe_signature) in [
        ("fixture_math::multiply", "private", true, false),
        (
            "fixture_math::Measures::Output",
            "sealed_trait",
            false,
            false,
        ),
        ("fixture_math::multiply", "unsupported_generic", false, true),
    ] {
        let mut document: Value = serde_json::from_slice(&fixture.index).unwrap();
        let item = document["items"]
            .as_array_mut()
            .unwrap()
            .iter_mut()
            .find(|item| item["path"] == path)
            .unwrap();
        item["support"] = "rejected".into();
        item["reason"] = reason.into();
        if private {
            item["visibility"] = "private".into();
        }
        if unsafe_signature {
            item["signature"] = "unsafe fn multiply<const N: usize>(value: i64) -> i64".into();
        }
        let mut bytes = serde_json::to_vec(&document).unwrap();
        bytes.push(b'\n');
        RustApiIndex::replay(&bytes).unwrap();
        let errors = prepare_demanded_native_rust(
            SOURCE,
            Path::new("demanded.spx"),
            options(),
            &bytes,
            package,
            &selected,
        )
        .unwrap_err();
        assert_eq!(errors[0].code, "SPX-B153");
    }
    assert_eq!(
        ConcreteType::parse("Vec<Vec<Vec<i64>>>"),
        Err(DemandError::MalformedType)
    );
    let stale = SelectedPackage {
        version: "9.9.9",
        ..package
    };
    assert!(prepare_demanded_native_rust(
        SOURCE,
        Path::new("demanded.spx"),
        options(),
        &fixture.index,
        stale,
        &selected
    )
    .is_err());
}
