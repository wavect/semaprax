use super::*;
use std::{
    fs,
    process::{Command, Output},
    sync::atomic::{AtomicU64, Ordering},
};
static SERIAL: AtomicU64 = AtomicU64::new(0);
const SOURCE: &str = r#"
module ri08.registration;
permit {callback.registry}
@id("callback.host") interface RegistryHost permits {callback.registry} {
 @id("callback.register") import rust fn register(seed:i64)->i64 effects {callback.registry} failure status "semaprax.rich-callback-registry.v1";
 @id("callback.dispatch") import rust fn dispatch(value:i64)->i64 effects {callback.registry} failure status "semaprax.rich-callback-registry.v1";
 @id("callback.unregister") import rust fn unregister()->i64 effects {callback.registry} failure status "semaprax.rich-callback-registry.v1";
}
@id("callback.factory") fn factory(offset:i64)->fn(i64)->i64 {fn(value:i64)->i64 {value+offset}}
@id("callback.advance") fn advance(state:i64,value:i64)->i64 requires value>=0 {state+value}
@id("callback.install") fn install(seed:i64)->i64 uses {callback.registry} {register(seed)}
@id("callback.apply") fn apply(value:i64)->i64 uses {callback.registry} {dispatch(value)}
@id("callback.close") fn close()->i64 uses {callback.registry} {unregister()}
@id("app.main") fn main()->i64 {0}
"#;
const FIXTURE: &str = r#"
use std::sync::atomic::{AtomicU8,Ordering};
static MODE:AtomicU8=AtomicU8::new(0);
pub fn mode(value:u8){MODE.store(value,Ordering::SeqCst)}
pub trait Accumulator {type Error;fn advance(&mut self,value:i64)->Result<i64,Self::Error>;}
pub struct Registry<T:Accumulator>{implementation:Option<T>}
impl<T:Accumulator> Registry<T>{
 pub fn register(implementation:T)->Self{if MODE.load(Ordering::SeqCst)==1{panic!("register panic")};Self{implementation:Some(implementation)}}
 pub fn dispatch(&mut self,value:i64)->Result<i64,T::Error>{if MODE.load(Ordering::SeqCst)==2{panic!("dispatch panic")};self.implementation.as_mut().unwrap().advance(value)}
 pub fn unregister(mut self)->T{if MODE.load(Ordering::SeqCst)==3{panic!("unregister panic")};self.implementation.take().unwrap()}
}
"#;
const MAIN: &str = r#"
fn main(){
 let domain=SpxCallbackDomain::new(2).unwrap();
 assert!(matches!(SpxRegisteredCallbacks::new(domain.clone(),&[]),Err(SpxCallbackError::AdapterRejected)));
 {
  let registered=SpxRegisteredCallbacks::new(domain.clone(),&["callback.registry"]).unwrap();
  registered.install(10).unwrap();assert_eq!(domain.live_environments(),1);
  assert_eq!(registered.apply(4),Ok(14));
  assert_eq!(registered.apply(5),Ok(19));
  assert!(matches!(registered.apply(-1),Err(SpxCallbackError::Contract(_))));
  assert_eq!(registered.apply(0),Ok(19));
  assert!(matches!(registered.apply(i64::MAX),Err(SpxCallbackError::Semantic(_))));
  assert_eq!(registered.apply(0),Ok(19));
  registered.close().unwrap();
  assert_eq!(registered.apply(0),Err(SpxCallbackError::AfterTeardown));
  assert_eq!(domain.live_environments(),0);
 }
 assert_eq!(domain.active_depth(),0);
 {
  let registered=SpxRegisteredCallbacks::new(domain.clone(),&["callback.registry"]).unwrap();
  registered.install(3).unwrap();assert_eq!(registered.apply(2),Ok(5));
 }
 assert_eq!(domain.live_environments(),0,"ordinary Drop must execute authored close");
 let shallow=SpxCallbackDomain::new(1).unwrap();
 {
  let registered=SpxRegisteredCallbacks::new(shallow.clone(),&["callback.registry"]).unwrap();
  registered.install(7).unwrap();
  assert_eq!(registered.apply(1),Err(SpxCallbackError::DepthLimit));
  registered.close().unwrap();
 }
 assert_eq!(shallow.active_depth(),0);assert_eq!(shallow.live_environments(),0);
 for mode in 1..=3 {
  let uncertain=SpxCallbackDomain::new(2).unwrap();
  {
   let registered=SpxRegisteredCallbacks::new(uncertain.clone(),&["callback.registry"]).unwrap();
   if mode!=1 {registered.install(9).unwrap()}
   callback_fixture::mode(mode);
   let failed=match mode {1=>registered.install(9).map(|_|0),2=>registered.apply(1),_=>registered.close().map(|_|0)};
   assert_eq!(failed,Err(SpxCallbackError::RustPanicked));
   assert_eq!(registered.apply(1),Err(SpxCallbackError::UncertainTeardown));
   callback_fixture::mode(0);
  }
  assert_eq!(uncertain.active_depth(),0);
  assert_eq!(uncertain.live_environments(),1,"uncertain foreign retention must retain environment lease");
 }
}
"#;
struct Temp(std::path::PathBuf);
impl Drop for Temp {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}
fn success(output: Output) {
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
}
fn selection() -> NativeRegistrySelection {
    NativeRegistrySelection {
        callback: NativeCallbackSelection {
            factory_id: "callback.factory".into(),
            transition_id: "callback.advance".into(),
            trait_path: "callback_fixture::Accumulator".into(),
            method: "advance".into(),
            error_type: "Error".into(),
        },
        registry_path: "callback_fixture::Registry".into(),
        install_export: "callback.install".into(),
        apply_export: "callback.apply".into(),
        close_export: "callback.close".into(),
        register_import: "callback.register".into(),
        dispatch_import: "callback.dispatch".into(),
        unregister_import: "callback.unregister".into(),
    }
}
#[test]
fn registered_callback_nested_c_rust_round_trip_and_uncertain_teardown() {
    let generated =
        prepare_registered_native_rust_callbacks(SOURCE, Path::new("registry.spx"), &selection())
            .unwrap();
    let canonical = semaprax::format::canonical(&semaprax::check(SOURCE, "registry.spx").unwrap());
    let replay = prepare_registered_native_rust_callbacks(
        &canonical,
        Path::new("registry.spx"),
        &selection(),
    )
    .unwrap();
    assert_eq!(generated, replay);
    let root = Temp(std::env::temp_dir().join(format!(
        "semaprax-ri08-registry-{}-{}",
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
    for (name, text) in [
        ("module.c", generated.callback.c_source.as_str()),
        (
            "semaprax_native_rust_interop.h",
            generated.callback.header.as_str(),
        ),
        (
            "semaprax_native_rust_interop_ffi.rs",
            generated.callback.ffi_rust.as_str(),
        ),
        ("registry.c", generated.registry_c_source.as_str()),
        ("registry_boundary.h", generated.registry_header.as_str()),
        (
            "registry_boundary.rs",
            generated.registry_safe_rust.as_str(),
        ),
        ("registry_ffi.rs", generated.registry_ffi_rust.as_str()),
    ] {
        fs::write(root.0.join(name), text).unwrap()
    }
    let prefix = format!(
        "{}\n{}\n{}",
        generated.callback.safe_rust, generated.callback.adapter_rust, generated.registry_adapter
    );
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
        for (source, object) in [("module.c", "module.o"), ("registry.c", "registry.o")] {
            success(
                Command::new(&clang)
                    .current_dir(&root.0)
                    .args(["-std=c11", opt, "-c", source, "-o", object])
                    .output()
                    .unwrap(),
            );
        }
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
                    "-C",
                    "link-arg=registry.o",
                    "-o",
                    "consumer",
                ])
                .output()
                .unwrap(),
        );
        success(Command::new(root.0.join("consumer")).output().unwrap());
    }
    let mutant = prefix.replacen(
        "depth.get()>=self.domain.0.limit",
        "depth.get()>self.domain.0.limit",
        1,
    );
    assert_ne!(mutant, prefix);
    fs::write(root.0.join("mutant.rs"), format!("{mutant}\n{MAIN}")).unwrap();
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
                "-C",
                "link-arg=registry.o",
                "-o",
                "mutant",
            ])
            .output()
            .unwrap(),
    );
    let failure = Command::new(root.0.join("mutant")).output().unwrap();
    assert!(!failure.status.success());
    assert!(String::from_utf8_lossy(&failure.stderr).contains("DepthLimit"));
}
