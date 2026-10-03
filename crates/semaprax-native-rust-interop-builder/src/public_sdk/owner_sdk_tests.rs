use super::owner_sdk::prepare_opaque_owner_native;
use std::{fs, path::Path, process::Command};

const SOURCE: &str = r#"module owner.fixture;
@id("owner.regex") resource Regex { @id("owner.regex.drop") drop import "owner.drop"; }
@id("owner.host") interface Host permits { } {
 @id("owner.drop") import fn drop_regex(regex: own Regex) -> unit effects { } failure infallible consumes regex always;
 @id("owner.new") import rust fn regex_new(pattern: i64) -> Regex from "fixture_regex::Regex::new" effects { } failure infallible;
 @id("owner.match") import rust fn regex_match(regex: own Regex, input: i64) -> bool from "fixture_regex::Regex::consume_match" effects { } failure infallible;
}
@id("owner.run") fn run(pattern: i64, input: i64, divisor: i64) -> i64 {
 let spare = regex_new(99);
 let second = regex_new(88);
 let regex = regex_new(pattern);
 if regex_match(regex, input / divisor) { 1 } else { 0 }
}
@id("owner.main") fn main() -> i64 { 0 }
"#;

const FIXTURE: &str = r#"
mod fixture_regex {
    use std::cell::RefCell;
    thread_local! { pub static DROPS: RefCell<Vec<i64>> = const { RefCell::new(Vec::new()) }; }
    pub static PANIC_DROP: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
    pub struct Regex(i64);
    impl Regex {
        pub fn new(value:i64)->Self { assert_ne!(value,-999,"constructor panic"); Self(value) }
        pub fn consume_match(self,input:i64)->bool { assert_ne!(input,-777,"method panic"); self.0==input }
    }
    impl Drop for Regex { fn drop(&mut self) { DROPS.with(|drops| drops.borrow_mut().push(self.0)); if self.0==88 && PANIC_DROP.load(std::sync::atomic::Ordering::Relaxed) { panic!("drop panic"); } } }
}
fn invoke(pattern:i64,input:i64,divisor:i64,status:i32,value:i64,drops:&[i64]) {
    fixture_regex::DROPS.with(|trace|trace.borrow_mut().clear());
    let context=spx_owner_context_new(); assert_ne!(context,0);
    let mut output=1234567;
    let actual=unsafe { spx_owner_entry(context,pattern,input,divisor,&mut output) };
    assert_eq!(actual,status); assert_eq!(output,value);
    fixture_regex::DROPS.with(|trace|assert_eq!(&*trace.borrow(),drops));
    assert_eq!(spx_owner_context_close(context),0);
}
fn main() {
    assert_eq!(spx_owner_call(7,7,1),Ok(1));
    invoke(7,7,1,0,1,&[7,88,99]);
    invoke(7,8,1,0,0,&[7,88,99]);
    invoke(7,7,0,8,1234567,&[7,88,99]);
    invoke(-999,7,1,2,1234567,&[88,99]);
    invoke(7,-777,1,2,1234567,&[7,88,99]);
    fixture_regex::PANIC_DROP.store(true,std::sync::atomic::Ordering::Relaxed);
    invoke(7,7,1,2,1234567,&[7,88,99]);
    invoke(7,7,0,8,1234567,&[7,88,99]);
    fixture_regex::PANIC_DROP.store(false,std::sync::atomic::Ordering::Relaxed);
    // Hostile carriers never consume a live owner or publish an output.
    let first=spx_owner_context_new(); let other=spx_owner_context_new();
    let mut owner=SpxOwner::default(); assert_eq!(unsafe {spx_owner_new(first,42,&mut owner)},0);
    let mut output=99; assert_eq!(unsafe {spx_owner_consume(other,owner,42,&mut output)},3); assert_eq!(output,99);
    let forged=SpxOwner {slot:99,..owner}; assert_eq!(spx_owner_drop(first,forged),3);
    assert_eq!(spx_owner_context_close(first),5);
    assert_eq!(unsafe {spx_owner_consume(first,owner,42,&mut output)},0); assert_eq!(output,1);
    assert_eq!(spx_owner_drop(first,owner),3);
    let stale=owner; assert_eq!(unsafe {spx_owner_new(first,43,&mut owner)},0);
    assert_eq!(spx_owner_drop(first,stale),3); assert_eq!(spx_owner_drop(first,owner),0);
    assert_eq!(spx_owner_context_close(first),0); assert_eq!(spx_owner_context_close(other),0);
}
"#;

#[test]
fn generated_opaque_owner_source_and_cleanup_execute_physically() {
    let rustc = std::env::var("RUSTC").expect("configure absolute RUSTC for RI-05 execution");
    let clang = std::env::var("CLANG").expect("configure absolute CLANG for RI-05 execution");
    assert!(Path::new(&rustc).is_absolute() && Path::new(&clang).is_absolute());
    let program = semaprax::check(SOURCE, "owner-sdk.spx").unwrap();
    let canonical = semaprax::format::canonical(&program);
    let round_trip = semaprax::check(&canonical, "owner-round-trip.spx").unwrap();
    assert_eq!(semaprax::format::canonical(&round_trip), canonical);
    let generated = prepare_opaque_owner_native(&program, "owner.run").unwrap();
    let repeated = prepare_opaque_owner_native(&program, "owner.run").unwrap();
    assert_eq!(generated.c_source, repeated.c_source);
    assert_eq!(generated.rust_adapter, repeated.rust_adapter);
    let root = std::env::temp_dir().join(format!("semaprax-owner-native-{}", std::process::id()));
    fs::create_dir(&root).unwrap();
    fs::write(root.join("owner.h"), &generated.header).unwrap();
    for (label, optimization, c, adapter, success) in [
        (
            "o0",
            "-O0",
            generated.c_source.clone(),
            generated.rust_adapter.clone(),
            true,
        ),
        (
            "o2",
            "-O2",
            generated.c_source.clone(),
            generated.rust_adapter.clone(),
            true,
        ),
        (
            "missing-drop",
            "-O2",
            generated.c_source.replace(
                "spx_owner_drop(context,f->owners[",
                "spx_control_skip_drop(context,f->owners[",
            ),
            generated.rust_adapter.clone(),
            false,
        ),
        (
            "flipped-method",
            "-O2",
            generated.c_source.clone(),
            generated
                .rust_adapter
                .replace("u8::from(result)", "u8::from(!result)"),
            false,
        ),
    ] {
        let c = if label == "missing-drop" {
            c.replace("#include <limits.h>","#include <limits.h>\nint32_t spx_control_skip_drop(uint64_t c,spx_owner h){(void)c;(void)h;return 0;}")
        } else {
            c
        };
        fs::write(root.join("owner.c"), c).unwrap();
        let compile = Command::new(&clang)
            .args([
                "-std=c11",
                "-Wall",
                "-Wextra",
                "-Werror",
                optimization,
                "-c",
            ])
            .arg(root.join("owner.c"))
            .arg("-o")
            .arg(root.join("owner.o"))
            .output()
            .unwrap();
        assert!(
            compile.status.success(),
            "{label}: {}",
            String::from_utf8_lossy(&compile.stderr)
        );
        fs::write(root.join("main.rs"), format!("{adapter}\n{FIXTURE}")).unwrap();
        let binary = root.join(label);
        let compile = Command::new(&rustc)
            .args(["--edition=2021", "-Dwarnings"])
            .arg(root.join("main.rs"))
            .arg("-C")
            .arg(format!("link-arg={}", root.join("owner.o").display()))
            .arg("-o")
            .arg(&binary)
            .output()
            .unwrap();
        assert!(
            compile.status.success(),
            "{label}: {}",
            String::from_utf8_lossy(&compile.stderr)
        );
        let run = Command::new(binary).output().unwrap();
        assert_eq!(
            run.status.success(),
            success,
            "{label}: {}",
            String::from_utf8_lossy(&run.stderr)
        );
    }
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn opaque_owner_renderer_rejects_trivial_drop_and_unsupported_body() {
    let source = SOURCE.replace("drop import \"owner.drop\";", "drop trivial;");
    let program = semaprax::check(&source, "trivial.spx").unwrap();
    assert_eq!(
        prepare_opaque_owner_native(&program, "owner.run")
            .err()
            .unwrap()
            .code,
        "SPX-B112"
    );
    let source = SOURCE.replace("input / divisor", "input + divisor");
    let program = semaprax::check(&source, "body.spx").unwrap();
    assert_eq!(
        prepare_opaque_owner_native(&program, "owner.run")
            .err()
            .unwrap()
            .code,
        "SPX-B112"
    );
}

#[test]
fn opaque_owner_nested_statement_cleanup_is_refused() {
    let source = SOURCE.replace(
        "let spare = regex_new(99);",
        "let spare = { let local = regex_new(55); regex_new(99) };",
    );
    let program = semaprax::check(&source, "nested.spx").unwrap();
    assert_eq!(
        prepare_opaque_owner_native(&program, "owner.run")
            .err()
            .unwrap()
            .code,
        "SPX-B112"
    );
}

#[path = "owner_context_tests.rs"]
mod context;
