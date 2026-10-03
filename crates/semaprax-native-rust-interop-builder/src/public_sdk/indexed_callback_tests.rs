use super::*;
use std::{
    fs,
    process::{Command, Output},
};
const FIXTURE: &str = include_str!("../../fixtures/callback-trait/callback_fixture.rs");
const ENVELOPE: &[u8] = include_bytes!("../../fixtures/callback-trait/index-envelope.json");
const SOURCE: &str = r#"module selected.callback;
@id("callback.advance") fn advance(state:i64,value:i64)->Result<i64,i64> {
 if value < 0 {Result<i64,i64>::Err {error:state}}
 else {Result<i64,i64>::Ok {value:state+value}}
}
@id("main") fn main()->i64 {0}
"#;
fn index(envelope: &Value) -> RustApiIndex {
    let mut bytes = serde_json::to_vec(envelope).unwrap();
    bytes.push(b'\n');
    let admitted = RustApiIndex::admit_extractor_output(&bytes).unwrap();
    RustApiIndex::replay(admitted.canonical_json().as_bytes()).unwrap()
}
fn captured() -> Value {
    serde_json::from_slice(ENVELOPE).unwrap()
}
fn package(index: &RustApiIndex) -> SelectedPackage<'_> {
    SelectedPackage {
        cargo_alias: "callback_fixture",
        name: &index.package().name,
        version: &index.package().version,
        source_sha256: &index.package().source_sha256,
        target: index.target(),
        feature_digest: index.feature_digest(),
        stable_rustc_version: index.stable_rustc_version(),
    }
}
fn selection(index: &RustApiIndex) -> IndexedResultCallbackSelection {
    IndexedResultCallbackSelection {
        callback_id: "callback.advance".into(),
        method_path: "callback_fixture::Accumulator::advance".into(),
        error_type_path: "callback_fixture::Accumulator::Error".into(),
        index_digest: index.digest().into(),
    }
}
fn prepare(source: &str, index: &RustApiIndex) -> IndexedResultCallbackProjection {
    prepare_indexed_native_rust_result_callback(
        source,
        Path::new("selected.spx"),
        index.canonical_json().as_bytes(),
        package(index),
        &selection(index),
    )
    .unwrap()
}
fn refusal(result: Result<IndexedResultCallbackProjection, Vec<Diagnostic>>, reason: &str) {
    let errors = result.unwrap_err();
    assert_eq!(errors[0].code, "SPX-B154");
    assert!(errors[0].message.contains(reason), "{:?}", errors[0]);
    assert!(errors[0].span.is_some(), "source-located selection refusal");
}
#[test]
fn indexed_trait_callback_exact_identity_and_shape_refusals() {
    let baseline = index(&captured());
    assert_eq!(
        baseline.package().source_sha256,
        raw_digest(FIXTURE.as_bytes())
    );
    let first = prepare(SOURCE, &baseline);
    let canonical = semaprax::format::canonical(&semaprax::check(SOURCE, "selected.spx").unwrap());
    assert_eq!(first, prepare(&canonical, &baseline));
    let graph =
        semaprax::graph::to_json(&semaprax::check(SOURCE, "selected.spx").unwrap()).unwrap();
    assert!(graph.contains("callback.advance"));
    assert_ne!(
        first.binding_identity,
        prepare(&SOURCE.replace("state+value", "state-value"), &baseline).binding_identity
    );
    let mut stale = selection(&baseline);
    stale.index_digest = raw_digest(b"stale");
    refusal(
        prepare_indexed_native_rust_result_callback(
            SOURCE,
            Path::new("selected.spx"),
            baseline.canonical_json().as_bytes(),
            package(&baseline),
            &stale,
        ),
        "stale",
    );
    for field in [
        "name", "version", "source", "alias", "target", "features", "compiler",
    ] {
        let mut selected = package(&baseline);
        match field {
            "name" => selected.name = "different",
            "version" => selected.version = "9.9.9",
            "source" => {
                selected.source_sha256 =
                    "sha256:0000000000000000000000000000000000000000000000000000000000000000"
            }
            "alias" => selected.cargo_alias = "wrong_alias",
            "target" => selected.target = "wasm32-unknown-unknown",
            "features" => {
                selected.feature_digest =
                    "sha256:0000000000000000000000000000000000000000000000000000000000000000"
            }
            _ => selected.stable_rustc_version = "rustc 1.0.0",
        }
        refusal(
            prepare_indexed_native_rust_result_callback(
                SOURCE,
                Path::new("selected.spx"),
                baseline.canonical_json().as_bytes(),
                selected,
                &selection(&baseline),
            ),
            "identity differs",
        );
    }
    for (field, value, reason) in [
        ("receiver", serde_json::json!("shared"), "mutable method"),
        (
            "signature",
            serde_json::json!("fn advance(&mut self, value: u64) -> Result<i64,Self::Error>"),
            "requires fn",
        ),
        (
            "reason",
            serde_json::json!("sealed_trait"),
            "mutable method",
        ),
        (
            "reason",
            serde_json::json!("incomplete_type_closure"),
            "mutable method",
        ),
        (
            "generics",
            serde_json::json!({"parameters":[],"where_predicates":[r#"{"bound_predicate":{"bounds":[{"outlives":"'static"}],"generic_params":[],"type":{"generic":"Self"}}}"#]}),
            "mutable method",
        ),
    ] {
        let mut v = captured();
        v["index"]["items"][1][field] = value;
        let changed = index(&v);
        refusal(
            prepare_indexed_native_rust_result_callback(
                SOURCE,
                Path::new("selected.spx"),
                changed.canonical_json().as_bytes(),
                package(&changed),
                &selection(&changed),
            ),
            reason,
        );
    }
    let mut v = captured();
    v["index"]["items"][0]["associated_type"]["bounds"] =
        serde_json::json!([r#"{"outlives":"'static"}"#]);
    let changed = index(&v);
    refusal(
        prepare_indexed_native_rust_result_callback(
            SOURCE,
            Path::new("selected.spx"),
            changed.canonical_json().as_bytes(),
            package(&changed),
            &selection(&changed),
        ),
        "unbounded associated error",
    );
    let mut s = selection(&baseline);
    s.error_type_path = "callback_fixture::Other::Error".into();
    refusal(
        prepare_indexed_native_rust_result_callback(
            SOURCE,
            Path::new("selected.spx"),
            baseline.canonical_json().as_bytes(),
            package(&baseline),
            &s,
        ),
        "same selected trait",
    );
    let mut v = captured();
    v["index"]["types"][0]["kind"] = "struct".into();
    let changed = index(&v);
    refusal(
        prepare_indexed_native_rust_result_callback(
            SOURCE,
            Path::new("selected.spx"),
            changed.canonical_json().as_bytes(),
            package(&changed),
            &selection(&changed),
        ),
        "public, monomorphic",
    );
}
fn success(output: Output) {
    assert!(
        output.status.success(),
        "stdout:{}\nstderr:{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}
#[test]
fn indexed_trait_callback_physical_safe_impl_and_retained_state() {
    let index = index(&captured());
    let generated = prepare(SOURCE, &index).callback;
    struct Temp(PathBuf);
    impl Drop for Temp {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }
    let root = Temp(std::env::temp_dir().join(format!(
        "semaprax-indexed-trait-callback-{}",
        std::process::id()
    )));
    fs::create_dir_all(&root.0).unwrap();
    let rustc = std::env::var_os("RUSTC").unwrap_or_else(|| "rustc".into());
    let clang = std::env::var_os("CLANG").unwrap_or_else(|| "clang".into());
    // The committed rustdoc envelope is evidence for this unchanged fixture.
    // Its source hash is verified above; the physical impl uses this same file.
    let compile_fixture = |source: &str| {
        fs::write(root.0.join("fixture.rs"), source).unwrap();
        success(
            Command::new(&rustc)
                .current_dir(&root.0)
                .args([
                    "--edition=2021",
                    "--crate-type=rlib",
                    "--crate-name=callback_fixture",
                    "fixture.rs",
                    "-o",
                    "libcallback_fixture.rlib",
                ])
                .output()
                .unwrap(),
        );
    };
    fs::write(root.0.join("module.c"), &generated.c_source).unwrap();
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
    fs::write(
        root.0.join("adapter.rs"),
        format!("{}\n{}", generated.safe_rust, generated.adapter_rust),
    )
    .unwrap();
    let compile_adapter = || {
        Command::new(&rustc)
            .current_dir(&root.0)
            .args([
                "--edition=2021",
                "--crate-type=rlib",
                "--crate-name=indexed_callbacks",
                "adapter.rs",
                "--extern",
                "callback_fixture=libcallback_fixture.rlib",
                "-o",
                "libindexed_callbacks.rlib",
            ])
            .output()
            .unwrap()
    };
    compile_fixture(FIXTURE);
    success(compile_adapter());
    success(
        Command::new(&clang)
            .current_dir(&root.0)
            .args(["-std=c11", "-O2", "-c", "module.c", "-o", "module.o"])
            .output()
            .unwrap(),
    );
    fs::write(root.0.join("main.rs"), MAIN).unwrap();
    let compile_consumer = || {
        Command::new(&rustc)
            .current_dir(&root.0)
            .args([
                "--edition=2021",
                "main.rs",
                "-L",
                "dependency=.",
                "--extern",
                "indexed_callbacks=libindexed_callbacks.rlib",
                "--extern",
                "callback_fixture=libcallback_fixture.rlib",
                "-C",
                "link-arg=module.o",
                "-o",
                "consumer",
            ])
            .output()
            .unwrap()
    };
    success(compile_consumer());
    success(Command::new(root.0.join("consumer")).output().unwrap());
    // These false/partial index controls reach the final real Rust proof. A
    // safe generated impl cannot acquire unsafe authority or erase obligations.
    for (fixture, code) in [
        (
            FIXTURE.replace("pub trait Accumulator", "pub unsafe trait Accumulator"),
            "E0200",
        ),
        (
            format!(
                "mod hidden{{pub trait Sealed{{}}}}\n{}",
                FIXTURE
                    .lines()
                    .skip(1)
                    .collect::<Vec<_>>()
                    .join("\n")
                    .replace("trait Accumulator {", "trait Accumulator: hidden::Sealed {")
            ),
            "E0277",
        ),
        (
            FIXTURE.replace("type Error;", "type Error; fn missing(&self);"),
            "E0046",
        ),
        (FIXTURE.replace("value: i64", "value: u64"), "E0053"),
    ] {
        compile_fixture(&fixture);
        let output = compile_adapter();
        assert!(!output.status.success());
        assert!(
            String::from_utf8_lossy(&output.stderr).contains(code),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
    compile_fixture(FIXTURE);
    success(compile_adapter());
    for (body,code) in [
        ("let callback=SpxCallback::new(domain,1).unwrap();let once=callback.into_once();let _=once(1);let _=once(2);","E0382"),
        ("let mut proxy=SpxStatefulProxy::new(domain,1).unwrap();let f=proxy.as_fn_mut();drop(proxy);let _=f(1);","E0505"),
        ("let proxy=SpxStatefulProxy::new(domain,1).unwrap();std::thread::spawn(move||drop(proxy));","E0277"),
    ] {
        fs::write(root.0.join("main.rs"),format!("use indexed_callbacks::*;fn main(){{let domain=SpxCallbackDomain::new(4).unwrap();{body}}}")).unwrap();
        let output=compile_consumer();assert!(!output.status.success());assert!(String::from_utf8_lossy(&output.stderr).contains(code),"{}",String::from_utf8_lossy(&output.stderr));
    }
}
const MAIN: &str = r#"use indexed_callbacks::*;
use callback_fixture::Accumulator;
struct Registry { value:Option<Box<dyn Accumulator<Error=SpxCallbackError>>> }
fn install(domain:SpxCallbackDomain)->Registry {
 let proxy=SpxStatefulProxy::new(domain,40).unwrap();
 assert!(std::mem::size_of_val(&proxy)>0);
 Registry{value:Some(Box::new(proxy))}
}
fn main(){
 let domain=SpxCallbackDomain::new(4).unwrap();
 let mut registry=install(domain.clone());
 assert_eq!(domain.live_environments(),1);
 let values=(1..=3).map(|value|registry.value.as_mut().unwrap().advance(value)).collect::<Result<Vec<_>,_>>().unwrap();
 assert_eq!(values,[41,43,46]);
 assert_eq!(registry.value.as_mut().unwrap().advance(-1),Err(SpxCallbackError::SourceDomain(46)));
 assert_eq!(registry.value.as_mut().unwrap().advance(0),Ok(46));
 drop(registry.value.take());
 assert_eq!(domain.live_environments(),0);
 assert_eq!(domain.active_depth(),0);
}
"#;
