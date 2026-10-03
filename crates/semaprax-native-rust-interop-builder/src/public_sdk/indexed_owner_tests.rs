//! Physical selected-index -> held Project -> generated owner package gate.
use super::*;
use crate::indexed_binding::SelectedPackage;
use semaprax_rust_api_index::RustApiIndex;
use std::{fs, process::Command};

const SOURCE: &str = r#"module owner.fixture;
@id("owner.regex") resource Regex { @id("owner.regex.drop") drop import "owner.drop"; }
@id("owner.host") interface Host permits { } {
 @id("owner.drop") import fn drop_regex(regex: own Regex) -> unit effects { } failure infallible consumes regex always;
 @id("owner.new") import rust selected fn regex_new from "fixture_regex::Regex::new" effects { } failure infallible;
 @id("owner.match") import rust selected fn regex_match from "fixture_regex::Regex::consume_match" effects { } failure infallible;
}
@id("owner.run") fn run(pattern: i64, input: i64, divisor: i64) -> i64 {
 let spare = regex_new(99);
 let regex = regex_new(pattern);
 if regex_match(regex, input / divisor) { 1 } else { 0 }
}
@id("owner.main") fn main() -> i64 { 0 }
"#;
// Ordinary, unchanged Rust source: no SEMAPRAX traits, derives, wrappers or ABI.
const RUST: &[u8] = b"pub struct Regex { pattern:i64 } impl Regex { pub fn new(pattern:i64)->Self { Self {pattern} } pub fn consume_match(self,input:i64)->bool { self.pattern==input } }\n";
const MANIFEST: &str = "schema = \"semaprax.project.v1\"\nname = \"owner\"\nentry = \"owner.fixture\"\nsources = [\"src/app.spx\", \"src/tests.spx\"]\nweb_exports = [\"owner.run\"]\ntests = [\"owner.tests\"]\n";

fn index(source: &[u8], rustc: &str) -> Vec<u8> {
    let mut envelope: Value = serde_json::from_slice(include_bytes!(
        "../../../semaprax-rust-api-index/fixtures/local-api-fixture-v2-envelope.json"
    ))
    .unwrap();
    let index = &mut envelope["index"];
    index["package"]["name"] = "fixture_regex".into();
    index["package"]["version"] = "0.0.1".into();
    index["package"]["source_sha256"] = raw_digest(source).into();
    index["target"] = target_triple().unwrap().into();
    index["stable_rustc_version"] = rustc.into();
    let mut item = index["items"]
        .as_array()
        .unwrap()
        .iter()
        .find(|item| item["path"] == "local_api_fixture::MacroGenerated::answer")
        .unwrap()
        .clone();
    item["path"] = "fixture_regex::Regex::new".into();
    item["receiver"] = "none".into();
    item["signature"] = "fn new(pattern: i64) -> Self".into();
    item["type_roots"] = serde_json::json!(["fixture_regex::Regex"]);
    item["reachable_types"] = serde_json::json!(["fixture_regex::Regex"]);
    let mut method = item.clone();
    method["path"] = "fixture_regex::Regex::consume_match".into();
    method["receiver"] = "owned".into();
    method["signature"] = "fn consume_match(self, input: i64) -> bool".into();
    let mut ty = index["types"]
        .as_array()
        .unwrap()
        .iter()
        .find(|ty| ty["path"] == "local_api_fixture::MacroGenerated")
        .unwrap()
        .clone();
    ty["path"] = "fixture_regex::Regex".into();
    index["items"] = serde_json::json!([method, item]);
    index["types"] = serde_json::json!([ty]);
    let mut bytes = serde_json::to_vec(&envelope).unwrap();
    bytes.push(b'\n');
    RustApiIndex::admit_extractor_output(&bytes)
        .unwrap()
        .canonical_json()
        .as_bytes()
        .to_vec()
}
fn shared_receiver_index(source: &[u8], rustc: &str) -> Vec<u8> {
    let mut envelope: Value = serde_json::from_slice(include_bytes!(
        "../../../semaprax-rust-api-index/fixtures/local-api-fixture-v2-envelope.json"
    ))
    .unwrap();
    let index = &mut envelope["index"];
    index["package"]["name"] = "fixture_regex".into();
    index["package"]["version"] = "0.0.1".into();
    index["package"]["source_sha256"] = raw_digest(source).into();
    index["target"] = target_triple().unwrap().into();
    index["stable_rustc_version"] = rustc.into();
    let mut constructor = index["items"]
        .as_array()
        .unwrap()
        .iter()
        .find(|item| item["path"] == "local_api_fixture::MacroGenerated::answer")
        .unwrap()
        .clone();
    constructor["path"] = "fixture_regex::Regex::new".into();
    constructor["receiver"] = "none".into();
    constructor["signature"] = "fn new(pattern: i64) -> Self".into();
    constructor["type_roots"] = serde_json::json!(["fixture_regex::Regex"]);
    constructor["reachable_types"] = serde_json::json!(["fixture_regex::Regex"]);
    let mut shared = constructor.clone();
    shared["path"] = "fixture_regex::Regex::is_match".into();
    shared["receiver"] = "shared".into();
    shared["signature"] = "fn is_match(&self, input: &str) -> bool".into();
    let mut ty = index["types"]
        .as_array()
        .unwrap()
        .iter()
        .find(|ty| ty["path"] == "local_api_fixture::MacroGenerated")
        .unwrap()
        .clone();
    ty["path"] = "fixture_regex::Regex".into();
    index["items"] = serde_json::json!([shared, constructor]);
    index["types"] = serde_json::json!([ty]);
    let mut bytes = serde_json::to_vec(&envelope).unwrap();
    bytes.push(b'\n');
    RustApiIndex::admit_extractor_output(&bytes)
        .unwrap()
        .canonical_json()
        .as_bytes()
        .to_vec()
}

fn selections<'a>(
    source: &'a str,
    bytes: &'a [u8],
    rust: &'a [u8],
    version: &'a str,
    digest: &'a str,
    feature: &'a str,
) -> [IndexedProjectScalarSelection<'a>; 2] {
    ["owner.new", "owner.match"].map(|import_id| IndexedProjectScalarSelection {
        source_path: "src/app.spx",
        source,
        selection: IndexedScalarSelection {
            import_id,
            index_bytes: bytes,
            package_source_bytes: rust,
            package: SelectedPackage {
                cargo_alias: "fixture_regex",
                name: "fixture_regex",
                version: "0.0.1",
                source_sha256: digest,
                target: target_triple().unwrap(),
                feature_digest: feature,
                stable_rustc_version: version,
            },
        },
    })
}
fn canonical(source: &str) -> String {
    semaprax::format::canonical(&semaprax::parse(source, "src/app.spx").unwrap())
}
fn root(label: &str) -> PathBuf {
    let path = fs::canonicalize(std::env::temp_dir())
        .unwrap()
        .join(format!(
            "semaprax-indexed-owner-{label}-{}",
            std::process::id()
        ));
    fs::create_dir(&path).unwrap();
    fs::create_dir(path.join("src")).unwrap();
    path
}
fn write_project(root: &Path, source: &str) {
    fs::write(root.join("src/app.spx"), source).unwrap();
    fs::write(
        root.join("src/tests.spx"),
        canonical("module owner.tests; @id(\"owner.tests.main\") fn main() -> i64 { 0 }"),
    )
    .unwrap();
    fs::write(root.join("semaprax.toml"), MANIFEST).unwrap();
}
fn execute(rustc: &str, clang: &str, root: &Path, output: &Path, label: &str) -> bool {
    let library = root.join(format!("lib{label}.rlib"));
    let compiled = Command::new(rustc)
        .args([
            "--edition=2021",
            "--crate-type=rlib",
            "--crate-name=owner_sdk",
        ])
        .arg(output.join("lib.rs"))
        .arg("-o")
        .arg(&library)
        .output()
        .unwrap();
    assert!(
        compiled.status.success(),
        "{}",
        String::from_utf8_lossy(&compiled.stderr)
    );
    let source = root.join("consumer.rs");
    fs::write(&source,"fn main(){assert_eq!(owner_sdk::spx_owner_call(7,7,1),Ok(1));assert_eq!(owner_sdk::spx_owner_call(7,8,1),Ok(0));assert_eq!(owner_sdk::spx_owner_call(7,7,0),Err(8));}\n").unwrap();
    let binary = root.join(label);
    let compile = Command::new(rustc)
        .args(["--edition=2021", "--extern"])
        .arg(format!("owner_sdk={}", library.display()))
        .arg(&source)
        .args(["-C", &format!("linker={clang}"), "-L"])
        .arg(output)
        .args(["-l", "static=semaprax_native_rust_owned_data_sdk", "-o"])
        .arg(&binary)
        .output()
        .unwrap();
    assert!(
        compile.status.success(),
        "{}",
        String::from_utf8_lossy(&compile.stderr)
    );
    Command::new(binary).output().unwrap().status.success()
}

#[test]
fn indexed_owner_project_package_executes_and_refuses_stale_or_flipped_code() {
    let rustc = std::env::var("RUSTC").expect("absolute RUSTC");
    let clang = std::env::var("CLANG").expect("absolute CLANG");
    let version = Command::new(&rustc).arg("--version").output().unwrap();
    let version = std::str::from_utf8(&version.stdout).unwrap().trim();
    let source = canonical(SOURCE);
    let index = index(RUST, version);
    let replay = RustApiIndex::replay(&index).unwrap();
    let digest = raw_digest(RUST);
    let selections = selections(
        &source,
        &index,
        RUST,
        version,
        &digest,
        replay.feature_digest(),
    );
    let root = root("physical");
    write_project(&root, &source);
    let output = root.join("sdk");
    let bundle =
        build_indexed_project_native_rust_sdk(&root.join("semaprax.toml"), &selections, &output)
            .unwrap();
    assert_eq!(bundle.sdk().output_directory(), output);
    let descriptor: Value =
        serde_json::from_slice(&fs::read(output.join("descriptor.json")).unwrap()).unwrap();
    assert_eq!(descriptor["binding"]["resource_id"], "owner.regex");
    assert_eq!(descriptor["binding"]["rust_type"], "fixture_regex::Regex");
    assert_eq!(descriptor["binding"]["package_source_sha256"], digest);
    assert!(execute(&rustc, &clang, &root, &output, "consumer-good"));
    let before = fs::read(output.join("semaprax.native-rust-sdk.json")).unwrap();
    assert!(build_indexed_project_native_rust_sdk(
        &root.join("semaprax.toml"),
        &selections,
        &output
    )
    .is_err());
    assert_eq!(
        fs::read(output.join("semaprax.native-rust-sdk.json")).unwrap(),
        before
    );
    let flipped = String::from_utf8(RUST.to_vec()).unwrap().replace("==", "<");
    let mut stale = selections;
    stale[0].selection.package_source_bytes = flipped.as_bytes();
    let absent = root.join("stale");
    assert_eq!(
        build_indexed_project_native_rust_sdk(&root.join("semaprax.toml"), &stale, &absent)
            .unwrap_err()[0]
            .code,
        "SPX-B142"
    );
    assert!(!absent.exists());
    let flipped_index = self::index(flipped.as_bytes(), version);
    let replay = RustApiIndex::replay(&flipped_index).unwrap();
    let digest = raw_digest(flipped.as_bytes());
    let flipped_selections = self::selections(
        &source,
        &flipped_index,
        flipped.as_bytes(),
        version,
        &digest,
        replay.feature_digest(),
    );
    let output = root.join("flipped");
    build_indexed_project_native_rust_sdk(
        &root.join("semaprax.toml"),
        &flipped_selections,
        &output,
    )
    .unwrap();
    assert!(
        !execute(&rustc, &clang, &root, &output, "consumer-flipped"),
        "same consumer must detect changed Rust behavior"
    );
    // The index lies about a real result type; exact rustc function types reject it.
    let wrong = String::from_utf8(RUST.to_vec()).unwrap().replace(
        "->bool { self.pattern==input }",
        "->u8 { (self.pattern==input) as u8 }",
    );
    let index = self::index(wrong.as_bytes(), version);
    let replay = RustApiIndex::replay(&index).unwrap();
    let digest = raw_digest(wrong.as_bytes());
    let wrong_selections = self::selections(
        &source,
        &index,
        wrong.as_bytes(),
        version,
        &digest,
        replay.feature_digest(),
    );
    let output = root.join("wrong-result");
    assert!(build_indexed_project_native_rust_sdk(
        &root.join("semaprax.toml"),
        &wrong_selections,
        &output
    )
    .is_err());
    assert!(!output.exists());
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn indexed_owner_binding_roundtrip_graph_and_move_refusal() {
    use semaprax::project::{ProjectFrontendCache, ProjectFrontendSource, ProjectManifest};
    let version = "rustc 1.88.0 (6b00bc388 2025-06-23)";
    let source = canonical(SOURCE);
    let index = index(RUST, version);
    let replay = RustApiIndex::replay(&index).unwrap();
    let digest = raw_digest(RUST);
    let selected = selections(
        &source,
        &index,
        RUST,
        version,
        &digest,
        replay.feature_digest(),
    );
    let bindings = indexed_project::prepare_project_bindings(&selected).unwrap();
    assert_eq!(bindings[1].receiver, "owned");
    let tests = canonical("module owner.tests; @id(\"owner.tests.main\") fn main() -> i64 { 0 }");
    let sources = [
        ProjectFrontendSource::new("src/app.spx", &source).unwrap(),
        ProjectFrontendSource::new("src/tests.spx", &tests).unwrap(),
    ];
    let manifest = ProjectManifest::parse(MANIFEST).unwrap();
    let mut cache = ProjectFrontendCache::new_with_semantic_cache();
    let build = cache
        .build_indexed_rust(&manifest, &sources, &bindings)
        .unwrap();
    let graph = build.revision().semantic_graph().to_owned();
    let projected: Value = serde_json::from_str(&graph).unwrap();
    let imports = projected["indexed_rust_imports"]["imports"]
        .as_array()
        .unwrap();
    assert_eq!(imports.len(), 2);
    assert_eq!(imports[0]["id"], "owner.match");
    assert_eq!(imports[0]["receiver"], "owned");
    assert_eq!(imports[0]["selected_index_digest"], replay.digest());
    assert_eq!(imports[1]["id"], "owner.new");
    assert_eq!(imports[1]["result"], "opaque resource");
    assert!(graph.contains("owner.regex"));
    assert_eq!(
        cache
            .build_indexed_rust(&manifest, &sources, &bindings)
            .unwrap()
            .revision()
            .semantic_graph(),
        graph
    );
    let moved = canonical(&SOURCE.replace(
        "if regex_match(regex, input / divisor)",
        "let taken = regex_match(regex, input); if regex_match(regex, input / divisor)",
    ));
    let selected = selections(
        &moved,
        &index,
        RUST,
        version,
        &digest,
        replay.feature_digest(),
    );
    let bindings = indexed_project::prepare_project_bindings(&selected).unwrap();
    let sources = [
        ProjectFrontendSource::new("src/app.spx", &moved).unwrap(),
        ProjectFrontendSource::new("src/tests.spx", &tests).unwrap(),
    ];
    let errors = cache
        .build_indexed_rust(&manifest, &sources, &bindings)
        .err()
        .unwrap();
    assert!(
        errors.iter().any(|error| error.code == "SPX-O101"),
        "{errors:?}"
    );
    let wrong = canonical(
        &SOURCE
            .replace("resource Regex", "resource Other")
            .replace("own Regex", "own Other"),
    );
    let selected = selections(
        &wrong,
        &index,
        RUST,
        version,
        &digest,
        replay.feature_digest(),
    );
    let bindings = indexed_project::prepare_project_bindings(&selected).unwrap();
    let sources = [
        ProjectFrontendSource::new("src/app.spx", &wrong).unwrap(),
        ProjectFrontendSource::new("src/tests.spx", &tests).unwrap(),
    ];
    assert!(cache
        .build_indexed_rust(&manifest, &sources, &bindings)
        .is_err());
    let ordinary = canonical(
        r#"module owner.fixture;
@id("owner.regex") resource Regex { @id("owner.regex.drop") drop import "owner.drop"; }
@id("owner.host") interface Host permits { } { @id("owner.drop") import fn drop_regex(regex: own Regex) -> unit effects { } failure infallible consumes regex always; }
@id("owner.run") fn run(pattern:i64,input:i64,divisor:i64) -> i64 { 0 }
@id("owner.main") fn main() -> i64 { 0 }"#,
    );
    let sources = [
        ProjectFrontendSource::new("src/app.spx", &ordinary).unwrap(),
        ProjectFrontendSource::new("src/tests.spx", &tests).unwrap(),
    ];
    let errors = cache
        .build(&manifest, &sources)
        .err()
        .expect("ordinary finalizer must remain outside scalar linker");
    assert!(
        errors.iter().any(|e| e.code == "SPX-H006"
            && e.message.contains("outside the pure scalar linker profile")),
        "{errors:?}"
    );
}

#[test]
fn indexed_owner_return_helper_package_executes() {
    let rustc = std::env::var("RUSTC").expect("absolute RUSTC");
    let clang = std::env::var("CLANG").expect("absolute CLANG");
    let version = Command::new(&rustc).arg("--version").output().unwrap();
    let version = std::str::from_utf8(&version.stdout).unwrap().trim();
    let source = SOURCE
        .replace(
            "@id(\"owner.run\")",
            r#"@id("owner.make") fn make(pattern:i64, divisor:i64) -> Regex {
 let spare = regex_new(91);
 let regex = regex_new(pattern);
 let checked = pattern / divisor;
 regex
}
@id("owner.forward") fn forward(regex: own Regex) -> Regex { regex }
@id("owner.run")"#,
        )
        .replace(
            "let regex = regex_new(pattern);\n if",
            "let regex = forward(make(pattern,divisor));\n if",
        );
    let source = canonical(&source);
    let index = index(RUST, version);
    let replay = RustApiIndex::replay(&index).unwrap();
    let digest = raw_digest(RUST);
    let selections = selections(
        &source,
        &index,
        RUST,
        version,
        &digest,
        replay.feature_digest(),
    );
    let root = root("return-helper");
    write_project(&root, &source);
    let output = root.join("sdk");
    build_indexed_project_native_rust_sdk(&root.join("semaprax.toml"), &selections, &output)
        .unwrap();
    assert!(execute(
        &rustc,
        &clang,
        &root,
        &output,
        "consumer-return-helper"
    ));
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn selected_regex_shared_receiver_refuses_before_package_creation_without_ri06_loan_route() {
    let rustc = std::env::var("RUSTC").expect("absolute RUSTC");
    let version = Command::new(&rustc).arg("--version").output().unwrap();
    let version = std::str::from_utf8(&version.stdout).unwrap().trim();
    let source = canonical(
        r#"module owner.fixture;
@id("owner.regex") resource Regex { @id("owner.regex.drop") drop import "owner.drop"; }
@id("owner.host") interface Host permits { } {
 @id("owner.drop") import fn drop_regex(regex: own Regex) -> unit effects { } failure infallible consumes regex always;
 @id("owner.new") import rust selected fn regex_new from "fixture_regex::Regex::new" effects { } failure infallible;
 @id("owner.match") import rust selected fn regex_match from "fixture_regex::Regex::is_match" effects { } failure infallible;
}
@id("owner.run") fn run() -> i64 { 0 }
@id("owner.main") fn main() -> i64 { 0 }
"#,
    );
    let index = shared_receiver_index(RUST, version);
    let replay = RustApiIndex::replay(&index).unwrap();
    let digest = raw_digest(RUST);
    let selections = selections(
        &source,
        &index,
        RUST,
        version,
        &digest,
        replay.feature_digest(),
    );
    let root = root("shared-receiver-refusal");
    write_project(&root, &source);
    let output = root.join("sdk");
    let errors = build_indexed_project_native_rust_sdk(
        &root.join("semaprax.toml"),
        &selections,
        &output,
    )
    .unwrap_err();
    assert_eq!(errors[0].code, "SPX-B145");
    assert!(
        errors[0].message.contains("RI-06 loan routing"),
        "{errors:?}"
    );
    assert!(!output.exists(), "refusal must precede package creation");
    fs::remove_dir_all(root).unwrap();
}
