use super::*;
use std::{
    fs,
    process::{Command, Output},
};
const SOURCE: &str = r#"module ri08.result;
@id("callback.next") fn next(state:i64,value:i64)->Result<i64,i64>
 requires value != -3 ensures value != 13 {
 if value < 0 {Result<i64,i64>::Err {error:state}} else {Result<i64,i64>::Ok {value:state+value}}
}
@id("app.main") fn main()->i64 {0}
"#;
const MAIN: &str = r#"
fn main() {
 let domain=SpxCallbackDomain::new(2).unwrap();
 {
  let callback=SpxCallback::new(domain.clone(),7).unwrap();
  assert_eq!((1..=3).map(callback.as_fn()).collect::<Result<Vec<_>,_>>(),Ok(vec![8,9,10]));
  assert_eq!(callback.call(-1),Err(SpxCallbackError::SourceDomain(7)));
  assert!(matches!(callback.call(-3),Err(SpxCallbackError::Contract(_))));
  assert!(matches!(callback.call(13),Err(SpxCallbackError::Contract(_))));
  assert!(matches!(callback.call(i64::MAX),Err(SpxCallbackError::Semantic(_))));
  assert_eq!(callback.call(0),Ok(7),"failure must not leave a stale result");
  assert_eq!(callback.call_with(1,||panic!("before C")),Err(SpxCallbackError::RustPanicked));
  assert_eq!(callback.call_with(1,||callback.call(1).map(|_|())),Err(SpxCallbackError::Reentered));
  assert_eq!(callback.call(0),Ok(7));
  for code in [i64::MIN,i64::MAX,0] {
   let c=SpxCallback::new(domain.clone(),code).unwrap();
   assert_eq!(c.call(-1),Err(SpxCallbackError::SourceDomain(code)));
  }
  let mut state=SpxStatefulProxy::new(domain.clone(),10).unwrap();
  assert_eq!(callback_fixture::Accumulator::advance(&mut state,1),Ok(11));
  assert_eq!(callback_fixture::Accumulator::advance(&mut state,-1),Err(SpxCallbackError::SourceDomain(11)));
  assert_eq!(state.state(),Ok(11),"domain failure does not commit state");
  assert!(matches!(callback_fixture::Accumulator::advance(&mut state,13),Err(SpxCallbackError::Contract(_))));
  assert_eq!(state.state(),Ok(11));
  assert_eq!(state.as_fn_mut()(2),Ok(13));
  let once=SpxCallback::new(domain.clone(),5).unwrap().into_once();
  assert_eq!(once(1),Ok(6));
  callback.unregister().unwrap();
  assert_eq!(callback.call(0),Err(SpxCallbackError::AfterTeardown));
 }
 assert_eq!(domain.live_environments(),0);
 assert_eq!(domain.active_depth(),0);
}
"#;
fn selection() -> NativeResultCallbackSelection {
    NativeResultCallbackSelection {
        callback_id: "callback.next".into(),
        trait_path: "callback_fixture::Accumulator".into(),
        method: "advance".into(),
        error_type: "Error".into(),
    }
}
fn success(output: Output) {
    assert!(
        output.status.success(),
        "stdout: {}\nstderr: {}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}
#[test]
fn rich_result_callback_checked_domain_and_physical_failure_channels() {
    let generated =
        prepare_native_rust_result_callback(SOURCE, Path::new("result.spx"), &selection()).unwrap();
    let canonical = semaprax::format::canonical(&semaprax::check(SOURCE, "result.spx").unwrap());
    assert_eq!(
        generated,
        prepare_native_rust_result_callback(&canonical, Path::new("result.spx"), &selection())
            .unwrap()
    );
    let graph = semaprax::graph::to_json(&semaprax::check(SOURCE, "result.spx").unwrap()).unwrap();
    assert!(graph.contains("callback.next"));
    struct Temp(PathBuf);
    impl Drop for Temp {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }
    let root =
        Temp(std::env::temp_dir().join(format!("semaprax-result-callback-{}", std::process::id())));
    fs::create_dir_all(&root.0).unwrap();
    let rustc = std::env::var_os("RUSTC").unwrap_or_else(|| "rustc".into());
    let clang = std::env::var_os("CLANG").unwrap_or_else(|| "clang".into());
    fs::write(root.0.join("fixture.rs"),"pub trait Accumulator{type Error;fn advance(&mut self,value:i64)->Result<i64,Self::Error>;}").unwrap();
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
    for (name, text) in [
        ("module.c", generated.c_source.as_str()),
        ("semaprax_native_rust_interop.h", generated.header.as_str()),
        (
            "semaprax_native_rust_interop_ffi.rs",
            generated.ffi_rust.as_str(),
        ),
    ] {
        fs::write(root.0.join(name), text).unwrap();
    }
    fs::write(
        root.0.join("main.rs"),
        format!("use rich_callbacks::*;\n{MAIN}"),
    )
    .unwrap();
    let compile_adapter = |adapter: &str| {
        fs::write(
            root.0.join("adapter.rs"),
            format!("{}\n{adapter}", generated.safe_rust),
        )
        .unwrap();
        success(
            Command::new(&rustc)
                .current_dir(&root.0)
                .args([
                    "--edition=2021",
                    "--crate-type=rlib",
                    "--crate-name=rich_callbacks",
                    "adapter.rs",
                    "--extern",
                    "callback_fixture=libcallback_fixture.rlib",
                    "-o",
                    "librich_callbacks.rlib",
                ])
                .output()
                .unwrap(),
        );
    };
    let link = || {
        success(
            Command::new(&rustc)
                .current_dir(&root.0)
                .args([
                    "--edition=2021",
                    "main.rs",
                    "-L",
                    "dependency=.",
                    "--extern",
                    "rich_callbacks=librich_callbacks.rlib",
                    "--extern",
                    "callback_fixture=libcallback_fixture.rlib",
                    "-C",
                    "link-arg=module.o",
                    "-o",
                    "consumer",
                ])
                .output()
                .unwrap(),
        )
    };
    compile_adapter(&generated.adapter_rust);
    for opt in ["-O0", "-O2"] {
        success(
            Command::new(&clang)
                .current_dir(&root.0)
                .args(["-std=c11", opt, "-c", "module.c", "-o", "module.o"])
                .output()
                .unwrap(),
        );
        link();
        success(Command::new(root.0.join("consumer")).output().unwrap());
    }
    let refusal = "Some((1,error))=>return Err(SpxCallbackError::SourceDomain(error))";
    assert_eq!(generated.adapter_rust.matches(refusal).count(), 1);
    compile_adapter(
        &generated
            .adapter_rust
            .replace(refusal, "Some((1,error))=>error"),
    );
    link();
    assert!(
        !Command::new(root.0.join("consumer"))
            .output()
            .unwrap()
            .status
            .success(),
        "invented success must fail consumer"
    );
    compile_adapter(&generated.adapter_rust);
    let changed = prepare_native_rust_result_callback(
        &SOURCE.replace("state+value", "state-value"),
        Path::new("result.spx"),
        &selection(),
    )
    .unwrap();
    fs::write(root.0.join("module.c"), changed.c_source).unwrap();
    success(
        Command::new(&clang)
            .current_dir(&root.0)
            .args(["-std=c11", "-O2", "-c", "module.c", "-o", "module.o"])
            .output()
            .unwrap(),
    );
    link();
    assert!(
        !Command::new(root.0.join("consumer"))
            .output()
            .unwrap()
            .status
            .success(),
        "authored body controls actual execution"
    );
}

#[test]
fn rich_result_callback_rejects_unadmitted_source_and_trait_tokens() {
    let wrong = SOURCE.replace("->Result<i64,i64>", "->i64").replace(
        "if value < 0 {Result<i64,i64>::Err {error:state}} else {Result<i64,i64>::Ok {value:state+value}}",
        "state+value",
    );
    let errors = prepare_native_rust_result_callback(&wrong, Path::new("result.spx"), &selection())
        .unwrap_err();
    assert!(
        errors
            .iter()
            .any(|e| e.code == "SPX-B154" && e.span.is_some()),
        "{errors:?}"
    );
    let mut malicious = selection();
    malicious.trait_path = "callback_fixture::Accumulator; unsafe impl Send".into();
    assert!(
        prepare_native_rust_result_callback(SOURCE, Path::new("result.spx"), &malicious).is_err()
    );
}
