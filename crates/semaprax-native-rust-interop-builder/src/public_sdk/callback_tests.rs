use super::*;
use std::{
    fs,
    process::{Command, Output},
    sync::atomic::{AtomicU64, Ordering},
};
static SERIAL: AtomicU64 = AtomicU64::new(0);
const SOURCE: &str = r#"
module ri08.callbacks;
@id("callback.factory")
fn factory(offset:i64)->fn(i64)->i64 { fn(value:i64)->i64 {value+offset} }
@id("callback.advance")
fn advance(state:i64,value:i64)->i64 requires value>=0 {state+value}
@id("app.main") fn main()->i64 {0}
"#;
const RI13_M2_SOURCE: &str =
    include_str!("../../../../examples/ri13-m2-record-iterator/project/app.spx");
const RI13_M2_TESTS: &str =
    include_str!("../../../../examples/ri13-m2-record-iterator/project/tests.spx");
const RI13_M2_MANIFEST: &str =
    include_str!("../../../../examples/ri13-m2-record-iterator/project/semaprax.toml");
const FIXTURE: &str = r#"
pub trait Accumulator {type Error;fn advance(&mut self,value:i64)->Result<i64,Self::Error>;}
pub struct Registry<T:Accumulator>{implementation:Option<T>}
impl<T:Accumulator> Registry<T>{
    pub fn register(implementation:T)->Self {Self{implementation:Some(implementation)}}
    pub fn dispatch(&mut self,value:i64)->Result<i64,T::Error>{self.implementation.as_mut().unwrap().advance(value)}
    pub fn unregister(mut self)->T{self.implementation.take().unwrap()}
}
"#;
const MAIN: &str = r#"
fn main(){
    let domain=SpxCallbackDomain::new(2).unwrap();
    {
        let callback=SpxCallback::new(domain.clone(),7).unwrap();
        let actual=(1..=3).map(callback.as_fn()).collect::<Result<Vec<_>,_>>().unwrap();
        assert_eq!(actual,vec![8,9,10]);
        assert_eq!(callback.snapshot(),Ok(7));
        assert_eq!(domain.live_environments(),1);
        let once=SpxCallback::new(domain.clone(),9).unwrap().into_once();
        assert_eq!(domain.live_environments(),2);
        assert_eq!(once(3),Ok(12));
        assert_eq!(domain.live_environments(),1);
        assert_eq!(callback.call_with(1,||callback.call(1).map(|_|())),Err(SpxCallbackError::Reentered));
        assert_eq!(domain.active_depth(),0);
        assert_eq!(callback.call_with(1,||callback.unregister()),Err(SpxCallbackError::Reentered));
        assert_eq!(callback.call(1),Ok(8));
        let other=SpxCallback::new(domain.clone(),2).unwrap();
        assert_eq!(callback.call_with(1,||other.call(3).map(|v|assert_eq!(v,5))),Ok(8));
        let third=SpxCallback::new(domain.clone(),3).unwrap();
        assert_eq!(callback.call_with(1,||other.call_with(2,||third.call(3).map(|_|())).map(|_|())),Err(SpxCallbackError::DepthLimit));
        assert_eq!(domain.active_depth(),0);
        assert_eq!(callback.call_with(1,||Err(SpxCallbackError::Domain{domain:"fixture.domain.v1",code:19})),Err(SpxCallbackError::Domain{domain:"fixture.domain.v1",code:19}));
        assert_eq!(callback.call_with(1,||panic!("fixture pre-call panic")),Err(SpxCallbackError::RustPanicked));
        assert_eq!(domain.active_depth(),0);
        assert_eq!(callback.call(1),Ok(8));
        assert!(matches!(callback.call(i64::MAX),Err(SpxCallbackError::Semantic(_))));
        callback.unregister().unwrap();
        assert_eq!(callback.call(1),Err(SpxCallbackError::AfterTeardown));
    }
    assert_eq!(domain.live_environments(),0);
    {
        let mut proxy=SpxStatefulProxy::new(domain.clone(),10).unwrap();
        assert!(std::mem::size_of_val(&proxy)>0);
        {
            let mut callback=proxy.as_fn_mut();
            assert_eq!(callback(1),Ok(11));assert_eq!(callback(2),Ok(13));
            assert!(matches!(callback(-1),Err(SpxCallbackError::Contract(_))));
        }
        assert_eq!(proxy.state(),Ok(13));
        let mut registry=callback_fixture::Registry::register(proxy);
        assert_eq!(registry.dispatch(4),Ok(17));
        assert_eq!(registry.dispatch(5),Ok(22));
        let proxy=registry.unregister();
        assert_eq!(proxy.state(),Ok(22));
        proxy.unregister().unwrap();
        assert_eq!(proxy.state(),Err(SpxCallbackError::AfterTeardown));
    }
    assert_eq!(domain.live_environments(),0);
    let uncertain_domain=SpxCallbackDomain::new(1).unwrap();
    {
        let callback=SpxCallback::new(uncertain_domain.clone(),4).unwrap();
        callback.quarantine();
        assert_eq!(callback.call(1),Err(SpxCallbackError::UncertainTeardown));
        assert_eq!(callback.unregister(),Err(SpxCallbackError::UncertainTeardown));
    }
    assert_eq!(uncertain_domain.live_environments(),1,"uncertain teardown must not speculate release");
}
"#;
fn selection() -> NativeCallbackSelection {
    NativeCallbackSelection {
        factory_id: "callback.factory".into(),
        transition_id: "callback.advance".into(),
        trait_path: "callback_fixture::Accumulator".into(),
        method: "advance".into(),
        error_type: "Error".into(),
    }
}

#[test]
fn one_checked_record_and_stateful_callback_share_exact_source_revision() {
    let source = SOURCE.replace(
        "@id(\"app.main\")",
        "@id(\"ri13.event\") record Event { @id(\"ri13.event.value\") value: i64, @id(\"ri13.event.label\") label: string, }\n@id(\"app.main\")",
    );
    assert_ne!(source, SOURCE);
    let path = Path::new("ri13-m2.spx");
    let checked = semaprax::check(&source, path).unwrap();
    assert_eq!(checked.types.len(), 1);
    assert_eq!(checked.types[0].stable_id, "ri13.event");
    let ordinary = prepare_native_rust_callbacks(&source, path, &selection()).unwrap_err();
    assert_eq!(ordinary[0].code, "SPX-B154");
    let combined =
        prepare_native_rust_serde_callbacks(&source, path, "ri13.event", &selection()).unwrap();
    assert_eq!(combined.record.record_id, "ri13.event");
    assert_eq!(combined.source_revision, combined.callback.source_revision);
    assert!(combined
        .record
        .rust_source
        .contains("::serde_json::from_str"));
    assert!(combined
        .record
        .rust_source
        .contains("::serde_json::to_string"));
    assert!(combined.callback.adapter_rust.contains("SpxStatefulProxy"));
    let iterator = prepare_native_rust_serde_iterator_callbacks(
        &source,
        path,
        "ri13.event",
        "callback.factory",
        "callback.advance",
    )
    .unwrap();
    assert_eq!(iterator.source_revision, combined.source_revision);
    assert!(iterator.callback.adapter_rust.contains("pub fn as_fn_mut"));
    assert!(!iterator.callback.adapter_rust.contains("$TRAIT"));
    assert!(!iterator
        .callback
        .adapter_rust
        .contains("impl callback_fixture::Accumulator"));

    let changed = source.replace("label: string", "label: bool");
    assert_ne!(source, changed);
    let changed =
        prepare_native_rust_serde_callbacks(&changed, path, "ri13.event", &selection()).unwrap();
    assert_ne!(combined.source_revision, changed.source_revision);
    let wrong =
        prepare_native_rust_serde_callbacks(&source, path, "ri13.other", &selection()).unwrap_err();
    assert_eq!(wrong[0].code, "SPX-B154");
    assert!(wrong[0].message.contains("its selected record"));
}

#[test]
fn saved_m2_project_refuses_stale_source_before_iterator_projection_releases() {
    let root = Temp(std::env::temp_dir().join(format!(
        "semaprax-ri13-m2-stale-{}-{}",
        std::process::id(),
        SERIAL.fetch_add(1, Ordering::Relaxed)
    )));
    fs::create_dir(&root.0).unwrap();
    fs::write(root.0.join("app.spx"), RI13_M2_SOURCE).unwrap();
    fs::write(root.0.join("tests.spx"), RI13_M2_TESTS).unwrap();
    let manifest = root.0.join("semaprax.toml");
    fs::write(&manifest, RI13_M2_MANIFEST).unwrap();

    let source_path = root.0.join("app.spx");
    let stale = semaprax::project::with_authenticated_project(&manifest, |snapshot| {
        snapshot.check()?;
        let source = snapshot
            .sources()
            .iter()
            .find(|source| source.path() == "app.spx")
            .expect("saved M2 manifest authenticates app.spx");
        let projection = prepare_native_rust_serde_iterator_callbacks(
            source.source(),
            &source_path,
            "ri13.event",
            "callback.factory",
            "callback.advance",
        )?;
        assert_eq!(projection.record.record_id, "ri13.event");
        assert!(projection
            .callback
            .adapter_rust
            .contains("pub fn as_fn_mut"));

        let changed = source.source().replace("state + value", "state - value");
        assert_ne!(source.source(), changed);
        fs::write(&source_path, changed).unwrap();
        Ok(())
    });
    assert!(
        stale.is_err(),
        "stale M2 source must refuse the authenticated projection release"
    );
}

#[test]
fn selected_record_and_scalar_callbacks_admit_other_checked_type_declarations() {
    let source = SOURCE.replace(
        "@id(\"app.main\")",
        "@id(\"ri13.event\") record Event { @id(\"ri13.event.value\") value: i64, @id(\"ri13.event.label\") label: string, }\n@id(\"ri13.other\") record Other { @id(\"ri13.other.value\") value: bool, }\n@id(\"app.main\")",
    );
    let projection = prepare_native_rust_serde_iterator_callbacks(
        &source,
        Path::new("ri13-m2-linked.spx"),
        "ri13.event",
        "callback.factory",
        "callback.advance",
    )
    .unwrap();
    assert_eq!(projection.record.record_id, "ri13.event");
    assert!(projection.record.rust_source.contains("SpxMirrorri13event"));
}
struct Temp(std::path::PathBuf);
impl Drop for Temp {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}
fn success(output: Output) -> Output {
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    output
}
#[test]
fn rich_callback_iterator_and_stateful_trait_execute_with_lifecycle_controls() {
    let generated =
        prepare_native_rust_callbacks(SOURCE, Path::new("callbacks.spx"), &selection()).unwrap();
    let program = semaprax::check(SOURCE, Path::new("callbacks.spx")).unwrap();
    let canonical = semaprax::format::canonical(&program);
    let replay =
        prepare_native_rust_callbacks(&canonical, Path::new("callbacks.spx"), &selection())
            .unwrap();
    assert_eq!(generated.c_source, replay.c_source);
    assert_eq!(generated.adapter_rust, replay.adapter_rust);
    assert_eq!(generated.source_revision, replay.source_revision);
    let graph = semaprax::graph::to_json(&program).unwrap();
    assert!(graph.contains(&generated.closure_identity));
    let root = Temp(std::env::temp_dir().join(format!(
        "semaprax-ri08-callback-{}-{}",
        std::process::id(),
        SERIAL.fetch_add(1, Ordering::Relaxed)
    )));
    fs::create_dir(&root.0).unwrap();
    let rustc = std::env::var_os("RUSTC").unwrap_or_else(|| "rustc".into());
    let clang = std::env::var_os("CLANG").unwrap_or_else(|| "clang".into());
    fs::write(root.0.join("fixture.rs"), FIXTURE).unwrap();
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
    fs::write(root.0.join("module.c"), &generated.c_source).unwrap();
    let prefix = format!("{}\n{}\n", generated.safe_rust, generated.adapter_rust);
    fs::write(root.0.join("adapter.rs"), &prefix).unwrap();
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
    fs::write(
        root.0.join("main.rs"),
        format!("use rich_callbacks::*;\n{MAIN}"),
    )
    .unwrap();
    for opt in ["-O0", "-O2"] {
        success(
            Command::new(&clang)
                .current_dir(&root.0)
                .args(["-std=c11", opt, "-c", "module.c", "-o", "module.o"])
                .output()
                .unwrap(),
        );
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
        );
        success(Command::new(root.0.join("consumer")).output().unwrap());
    }
    for (label, main, codes) in [
        (
            "once",
            r#"fn main(){let d=SpxCallbackDomain::new(1).unwrap();let f=SpxCallback::new(d,1).unwrap().into_once();let _=f(1);let _=f(2);}"#,
            vec!["E0382"],
        ),
        (
            "thread",
            r#"fn main(){let d=SpxCallbackDomain::new(1).unwrap();let f=SpxCallback::new(d,1).unwrap();std::thread::spawn(move||f.call(1));}"#,
            vec!["E0277"],
        ),
        (
            "receiver-conflict",
            r#"fn main(){let d=SpxCallbackDomain::new(1).unwrap();let mut proxy=SpxStatefulProxy::new(d,1).unwrap();let mut f=proxy.as_fn_mut();let _=proxy.state();let _=f(1);}"#,
            vec!["E0502"],
        ),
        (
            "escape",
            r#"fn escape()->impl Fn(i64)->Result<i64,SpxCallbackError>{let d=SpxCallbackDomain::new(1).unwrap();let f=SpxCallback::new(d,1).unwrap();f.as_fn()}fn main(){}"#,
            vec!["E0597", "E0515"],
        ),
    ] {
        fs::write(
            root.0.join("negative.rs"),
            format!("use rich_callbacks::*;\n{main}"),
        )
        .unwrap();
        let output = Command::new(&rustc)
            .current_dir(&root.0)
            .args([
                "--edition=2021",
                "negative.rs",
                "-L",
                "dependency=.",
                "--extern",
                "rich_callbacks=librich_callbacks.rlib",
                "--emit=metadata",
                "--error-format=json",
                "--extern",
                "callback_fixture=libcallback_fixture.rlib",
            ])
            .output()
            .unwrap();
        assert!(!output.status.success(), "{label} compiled");
        let found = output
            .stderr
            .split(|b| *b == b'\n')
            .filter_map(|line| serde_json::from_slice::<Value>(line).ok())
            .any(|row| {
                row["level"] == "error" && codes.iter().any(|code| row["code"]["code"] == *code)
            });
        assert!(
            found,
            "{label}: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
    // The accepting-after-teardown mutation must fail the physical oracle.
    let mutant =
        generated
            .adapter_rust
            .replacen("1=>Err(SpxCallbackError::AfterTeardown)", "1=>Ok(())", 1);
    assert_ne!(mutant, generated.adapter_rust);
    fs::write(
        root.0.join("mutant.rs"),
        format!("{}\n{mutant}\n{MAIN}", generated.safe_rust),
    )
    .unwrap();
    success(
        Command::new(&rustc)
            .current_dir(&root.0)
            .args([
                "--edition=2021",
                "mutant.rs",
                "--extern",
                "callback_fixture=libcallback_fixture.rlib",
                "-C",
                "link-arg=module.o",
                "-o",
                "mutant",
            ])
            .output()
            .unwrap(),
    );
    let bad = Command::new(root.0.join("mutant")).output().unwrap();
    assert!(!bad.status.success(), "after-teardown mutant passed");
    assert!(
        String::from_utf8_lossy(&bad.stderr).contains("AfterTeardown"),
        "mutant failed outside teardown oracle: {}",
        String::from_utf8_lossy(&bad.stderr)
    );
}
#[test]
fn rich_callback_admission_rejects_unsupported_factory_and_trait_tokens() {
    let mut requested = selection();
    requested.trait_path = "x; unsafe impl Injection".into();
    let error =
        prepare_native_rust_callbacks(SOURCE, Path::new("callbacks.spx"), &requested).unwrap_err();
    assert_eq!(error[0].code, "SPX-B154");
    let source = SOURCE.replace(
        "fn(value:i64)->i64 {value+offset}",
        "fn(value:i64)->i64 {value}",
    );
    let error = prepare_native_rust_callbacks(&source, Path::new("callbacks.spx"), &selection())
        .unwrap_err();
    assert_eq!(error[0].code, "SPX-B154");
    let source = SOURCE.replace("{ fn(value:i64)", "{ let unrelated=1; fn(value:i64)");
    let error = prepare_native_rust_callbacks(&source, Path::new("callbacks.spx"), &selection())
        .unwrap_err();
    assert_eq!(error[0].code, "SPX-B154");
}
