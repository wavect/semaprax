//! Physical ownership across ordinary checked Semaprax helper boundaries.
use super::owner_sdk::prepare_opaque_owner_native;
use semaprax::cleanup_plan::{CleanupResultSource, ExitContinuation};
use std::{fs, path::Path, process::Command};

const SOURCE: &str = r#"module owner.fixture;
@id("owner.regex") resource Regex { @id("owner.regex.drop") drop import "owner.drop"; }
@id("owner.host") interface Host permits { } {
 @id("owner.drop") import fn drop_regex(regex: own Regex) -> unit effects { } failure infallible consumes regex always;
 @id("owner.new") import rust fn regex_new(pattern: i64) -> Regex from "fixture_regex::Regex::new" effects { } failure infallible;
 @id("owner.match") import rust fn regex_match(regex: own Regex, input: i64) -> bool from "fixture_regex::Regex::consume_match" effects { } failure infallible;
}
@id("owner.make") fn make(pattern:i64, input:i64, divisor:i64) -> Regex {
 let spare = regex_new(99);
 let second = regex_new(88);
 let regex = regex_new(pattern);
 let checked = input / divisor;
 regex
}
@id("owner.forward") fn forward(regex: own Regex) -> Regex { regex }
@id("owner.run") fn run(pattern:i64, input:i64, divisor:i64) -> i64 {
 let regex = forward(make(pattern,input,divisor));
 if regex_match(regex,input) { 1 } else { 0 }
}
@id("owner.main") fn main() -> i64 { 0 }
"#;

const FIXTURE: &str = r#"
mod fixture_regex {
    use std::cell::RefCell;
    thread_local! { pub static DROPS:RefCell<Vec<i64>> = const {RefCell::new(Vec::new())}; }
    pub static PANIC_DROP:std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
    pub struct Regex(i64);
    impl Regex {
        pub fn new(value:i64)->Self { assert_ne!(value,-999,"constructor panic"); Self(value) }
        pub fn consume_match(self,input:i64)->bool { assert_ne!(input,-777,"method panic"); self.0==input }
    }
    impl Drop for Regex { fn drop(&mut self) {
        DROPS.with(|trace|trace.borrow_mut().push(self.0));
        if self.0==88 && PANIC_DROP.load(std::sync::atomic::Ordering::Relaxed) { panic!("drop panic"); }
    } }
}
fn invoke(pattern:i64,input:i64,divisor:i64,status:i32,value:i64,drops:&[i64]) {
    fixture_regex::DROPS.with(|trace|trace.borrow_mut().clear());
    let context=spx_owner_context_new(); assert_ne!(context,0);
    let mut output=1234567;
    assert_eq!(unsafe {spx_owner_entry(context,pattern,input,divisor,&mut output)},status);
    assert_eq!(output,value);
    fixture_regex::DROPS.with(|trace|assert_eq!(&*trace.borrow(),drops));
    assert_eq!(spx_owner_context_close(context),0);
}
fn main() {
    assert_eq!(spx_owner_call(7,7,1),Ok(1));
    // Successful helper cleanup precedes publication and consuming dispatch.
    invoke(7,7,1,0,1,&[88,99,7]);
    invoke(7,8,1,0,0,&[88,99,7]);
    invoke(7,7,0,8,1234567,&[7,88,99]);
    invoke(-999,7,1,2,1234567,&[88,99]);
    invoke(7,-777,1,2,1234567,&[88,99,7]);
    fixture_regex::PANIC_DROP.store(true,std::sync::atomic::Ordering::Relaxed);
    // A failed non-result destructor prevents owner publication. The guarded
    // result is disposed last, once, while the first failure stays selected.
    invoke(7,7,1,2,1234567,&[88,99,7]);
    invoke(7,7,0,8,1234567,&[7,88,99]);
}
"#;

#[test]
fn opaque_owner_return_checked_graph_and_move_refusals() {
    let program = semaprax::check(SOURCE, "owner-return.spx").unwrap();
    let canonical = semaprax::format::canonical(&program);
    let round_trip = semaprax::check(&canonical, "owner-return.spx").unwrap();
    assert_eq!(semaprax::format::canonical(&round_trip), canonical);
    let graph = semaprax::graph::to_json(&program).unwrap();
    assert_eq!(semaprax::graph::to_json(&round_trip).unwrap(), graph);
    assert!(graph.contains("\"kind\":\"commit_result\",\"source\":{\"kind\":\"owned\""));
    assert!(graph.contains("\"kind\":\"provisional_result\""));
    let resolved = semaprax::hir::resolve(&program).unwrap();
    semaprax::hir::validate(&resolved).unwrap();
    for id in ["owner.make", "owner.forward"] {
        let function = resolved
            .functions
            .iter()
            .find(|f| f.id.as_str() == id)
            .unwrap();
        assert!(function.cleanup_plan.exits.iter().any(|exit| matches!(
            exit.continuation,
            ExitContinuation::CommitResult {
                source: CleanupResultSource::Owned { .. }
            }
        )));
    }
    for replacement in [
        "{ let used = regex_match(regex,7); regex }",
        "{ let moved = forward(regex); forward(regex) }",
        "{ let copied = regex; regex }",
    ] {
        let moved = SOURCE.replace("-> Regex { regex }", &format!("-> Regex {replacement}"));
        let errors = semaprax::check(&moved, "owner-return-moved.spx").unwrap_err();
        assert!(
            errors.iter().any(|error| error.code == "SPX-O101"),
            "{errors:?}"
        );
    }
    let recursive = SOURCE.replace("-> Regex { regex }", "-> Regex { forward(regex) }");
    let recursive = semaprax::check(&recursive, "owner-return-recursive.spx").unwrap();
    let error = prepare_opaque_owner_native(&recursive, "owner.run")
        .err()
        .unwrap();
    assert_eq!(error.code, "SPX-B112");
    assert!(error.message.contains("recursive helper"));
    assert!(
        prepare_opaque_owner_native(&program, "owner.make").is_err(),
        "owner-valued public SDK exports remain outside this batch"
    );
}

#[test]
fn opaque_owner_return_helpers_execute_with_cleanup_failure_controls() {
    let rustc = std::env::var("RUSTC").expect("configure absolute RUSTC");
    let clang = std::env::var("CLANG").expect("configure absolute CLANG");
    assert!(Path::new(&rustc).is_absolute() && Path::new(&clang).is_absolute());
    let program = semaprax::check(SOURCE, "owner-return.spx").unwrap();
    let generated = prepare_opaque_owner_native(&program, "owner.run").unwrap();
    let repeated = prepare_opaque_owner_native(&program, "owner.run").unwrap();
    assert_eq!(generated.c_source, repeated.c_source);
    let root = std::env::temp_dir().join(format!("semaprax-owner-return-{}", std::process::id()));
    fs::create_dir(&root).unwrap();
    fs::write(root.join("owner.h"), &generated.header).unwrap();
    let premature = generated
        .c_source
        .replace("if(status) { f->live[", "if(0) { f->live[");
    assert_ne!(
        premature, generated.c_source,
        "negative control must alter result publication guard"
    );
    let missing = generated.c_source.replace("spx_owner_drop(context,f->owners[", "spx_control_skip_drop(context,f->owners[")
        .replace("#include <limits.h>","#include <limits.h>\nint32_t spx_control_skip_drop(uint64_t c,spx_owner h){(void)c;(void)h;return 0;}");
    for (label, optimization, c, success) in [
        ("o0", "-O0", generated.c_source.clone(), true),
        ("o2", "-O2", generated.c_source.clone(), true),
        ("missing-drop", "-O2", missing, false),
        ("premature-publication", "-O2", premature, false),
    ] {
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
        fs::write(
            root.join("main.rs"),
            format!("{}\n{FIXTURE}", generated.rust_adapter),
        )
        .unwrap();
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
