//! Source-created transactional receivers consumed through generated safe Rust.
use super::*;
use std::process::Command;

const SOURCE: &str = r#"module mutable.fixture;
@id("mutable.update") fn update(state:i64, arg:i64)->i64
    requires arg != -9 ensures result != 99 { state + arg }
@id("mutable.make") fn make(state:i64)->FnMutI64(i64)->i64 {
    mut fn(arg:i64)->i64 { update(state, arg) }
}
@id("app.main") fn main()->i64 { 0 }
"#;

#[test]
fn mutable_callback_renderer_binds_exact_canonical_source_and_refuses_bad_body() {
    let rendered =
        prepare_native_rust_mutable_callback(SOURCE, Path::new("mutable.spx"), "mutable.make")
            .unwrap();
    let canonical = semaprax::format::canonical(&semaprax::check(SOURCE, "mutable.spx").unwrap());
    assert_eq!(
        rendered,
        prepare_native_rust_mutable_callback(&canonical, Path::new("mutable.spx"), "mutable.make")
            .unwrap()
    );
    let errors = prepare_native_rust_mutable_callback(
        &SOURCE.replace("update(state, arg)", "state + arg"),
        Path::new("mutable.spx"),
        "mutable.make",
    )
    .unwrap_err();
    assert!(errors.iter().any(|e| e.code == "SPX-T308"), "{errors:?}");
}

#[test]
fn mutable_callback_safe_owner_requires_exclusive_borrow_and_same_thread() {
    let root = std::env::temp_dir().join(format!("spx-mutable-sdk-{}", std::process::id()));
    std::fs::create_dir_all(&root).unwrap();
    std::fs::write(
        root.join("adapter.rs"),
        include_str!("mutable_callback_rust.template"),
    )
    .unwrap();
    let rustc = std::env::var_os("RUSTC").unwrap_or_else(|| "rustc".into());
    let library = Command::new(&rustc)
        .current_dir(&root)
        .args([
            "--edition=2021",
            "--crate-name=mutable_adapter",
            "--crate-type=rlib",
            "adapter.rs",
            "-o",
            "libmutable_adapter.rlib",
        ])
        .output()
        .unwrap();
    assert!(
        library.status.success(),
        "{}",
        String::from_utf8_lossy(&library.stderr)
    );
    for (label, body, diagnostic) in [
        ("healthy", "pub fn use_owner(c: &mut MutableCallback) { let mut f=c.as_fn_mut(); let _=f(1); let _=f(2); }", None),
        ("copy", "fn requires<T:Copy>() {} pub fn bad() { requires::<MutableCallback>(); }", Some("E0277")),
        ("clone", "fn requires<T:Clone>() {} pub fn bad() { requires::<MutableCallback>(); }", Some("E0277")),
        ("send", "fn requires<T:Send>() {} pub fn bad() { requires::<MutableCallback>(); }", Some("E0277")),
        ("sync", "fn requires<T:Sync>() {} pub fn bad() { requires::<MutableCallback>(); }", Some("E0277")),
        ("escape", "pub fn bad() -> impl FnMut(i64)->Result<i64,MutableCallbackError> { let mut c=MutableCallback::new(1).unwrap(); c.as_fn_mut() }", Some("E0597")),
        ("overlap", "pub fn bad(c: &mut MutableCallback) { let mut f=c.as_fn_mut(); let _=c.call(1); let _=f(2); }", Some("E0499")),
    ] {
        let source = format!("#![forbid(unsafe_code)]\nuse mutable_adapter::*;\n{body}\n");
        std::fs::write(root.join(format!("{label}.rs")), source).unwrap();
        let output = Command::new(&rustc).current_dir(&root)
            .args(["--edition=2021", "--crate-type=lib", "--emit=metadata", "--extern", "mutable_adapter=libmutable_adapter.rlib"])
            .arg(format!("{label}.rs")).output().unwrap();
        let stderr = String::from_utf8_lossy(&output.stderr);
        if let Some(diagnostic) = diagnostic {
            assert!(!output.status.success(), "{label} unexpectedly compiled");
            assert!(stderr.contains(diagnostic), "{label}: {stderr}");
        } else {
            assert!(output.status.success(), "{label}: {stderr}");
        }
    }
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn mutable_callback_physical_consumer_preserves_state_on_checked_failures() {
    let root = std::env::temp_dir().join(format!("spx-mutable-consumer-{}", std::process::id()));
    std::fs::create_dir_all(&root).unwrap();
    let rustc = std::env::var_os("RUSTC").unwrap_or_else(|| "rustc".into());
    let clang = std::env::var_os("CLANG").unwrap_or_else(|| "clang".into());
    for (label, source, main, optimization, expected) in [
        ("baseline-o0", SOURCE.to_owned(), MAIN, "-O0", true),
        ("baseline-o2", SOURCE.to_owned(), MAIN, "-O2", true),
        (
            "source-mutant",
            SOURCE.replace("state + arg", "state + arg + 1"),
            MAIN,
            "-O2",
            false,
        ),
        (
            "factory-failure",
            SOURCE.replace(
                "fn make(state:i64)->FnMutI64(i64)->i64 {",
                "fn make(state:i64)->FnMutI64(i64)->i64 ensures false {",
            ),
            CREATE_FAILURE,
            "-O2",
            true,
        ),
    ] {
        let rendered =
            prepare_native_rust_mutable_callback(&source, Path::new("mutable.spx"), "mutable.make")
                .unwrap();
        std::fs::write(
            root.join("module.c"),
            format!("{ALLOCATOR}\n{}\n{PROBE}", rendered.c_source),
        )
        .unwrap();
        std::fs::write(root.join("adapter.rs"), rendered.safe_rust).unwrap();
        std::fs::write(root.join("main.rs"), main).unwrap();
        for output in [
            Command::new(&clang)
                .current_dir(&root)
                .args(["-std=c11", optimization, "-c", "module.c", "-o", "module.o"])
                .output()
                .unwrap(),
            Command::new(&rustc)
                .current_dir(&root)
                .args([
                    "--edition=2021",
                    "--crate-name=mutable_adapter",
                    "--crate-type=rlib",
                    "adapter.rs",
                    "-o",
                    "libmutable_adapter.rlib",
                ])
                .output()
                .unwrap(),
        ] {
            assert!(
                output.status.success(),
                "{label}: {}",
                String::from_utf8_lossy(&output.stderr)
            );
        }
        let linked = Command::new(&rustc)
            .current_dir(&root)
            .args([
                "--edition=2021",
                "main.rs",
                "--extern",
                "mutable_adapter=libmutable_adapter.rlib",
                "-C",
                "link-arg=module.o",
                "-o",
                "consumer",
            ])
            .output()
            .unwrap();
        assert!(
            linked.status.success(),
            "{label}: {}",
            String::from_utf8_lossy(&linked.stderr)
        );
        let ran = Command::new(root.join("consumer")).output().unwrap();
        assert_eq!(
            ran.status.success(),
            expected,
            "{label}: {}",
            String::from_utf8_lossy(&ran.stderr)
        );
    }
    std::fs::remove_dir_all(root).unwrap();
}
const ALLOCATOR: &str = r#"
#include <stdlib.h>
#include <stdint.h>
static uint64_t allocations, frees;
static void *fixture_calloc(size_t n,size_t s){void *p=calloc(n,s);if(p)allocations++;return p;}
static void fixture_free(void *p){if(p)frees++;free(p);}
#define calloc fixture_calloc
#define free fixture_free
"#;
const PROBE: &str = r#"
#undef calloc
#undef free
uint64_t mutable_live(void){return allocations-frees;}
int mutable_raw_probe(void){
 struct spx_mutable_owner *owner=spx_mutable_create(10);
 if(!owner)return 1;
 int64_t out=777;uint32_t cls=0,code=0;
 if(spx_mutable_invoke(owner,-9,&out,&cls,&code)!=1 || out!=777 || owner->callback.state!=10 || cls!=1 || code!=1)return 2;
 if(spx_mutable_invoke(owner,89,&out,&cls,&code)!=1 || out!=777 || owner->callback.state!=10 || cls!=1 || code!=2)return 3;
 if(spx_mutable_invoke(owner,INT64_MAX,&out,&cls,&code)!=1 || out!=777 || owner->callback.state!=10 || cls!=2 || code!=1)return 4;
 owner->callback.active=true;
 if(spx_mutable_invoke(owner,1,&out,&cls,&code)!=2 || out!=777 || owner->callback.state!=10)return 5;
 owner->callback.active=false;
 spx_mutable_destroy(owner);return 0;
}
"#;
const MAIN: &str = r#"
use mutable_adapter::*;
extern "C"{fn mutable_live()->u64;fn mutable_raw_probe()->i32;}
fn main(){
 let mut receiver=MutableCallback::new(10).unwrap();
 assert_eq!(receiver.state().unwrap(),10);
 let values:Vec<_>=[2,3].into_iter().map(receiver.as_fn_mut()).collect();
 assert_eq!(values,vec![Ok(12),Ok(15)]);
 for _ in 0..20 {
  assert_eq!(receiver.call(-9),Err(MutableCallbackError::Checked{class:1,code:1}));
  assert_eq!(receiver.call(84),Err(MutableCallbackError::Checked{class:1,code:2}));
  assert_eq!(receiver.call(i64::MAX),Err(MutableCallbackError::Checked{class:2,code:1}));
  assert_eq!(receiver.state().unwrap(),15);
 }
 assert_eq!(receiver.call(1),Ok(16));
 drop(receiver);
 unsafe{assert_eq!(mutable_raw_probe(),0);assert_eq!(mutable_live(),0);}
 let unused=MutableCallback::new(1).unwrap();drop(unused);
 unsafe{assert_eq!(mutable_live(),0);}
}
"#;
const CREATE_FAILURE: &str = r#"
use mutable_adapter::*;
extern "C"{fn mutable_live()->u64;}
fn main(){assert!(matches!(MutableCallback::new(10),Err(MutableCallbackError::Creation)));unsafe{assert_eq!(mutable_live(),0);}}
"#;
