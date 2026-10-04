//! The runtime carrier is staged independently of source admission.
use super::*;
use std::process::Command;

#[test]
fn mutable_callback_renderer_keeps_source_admission_closed() {
    let source = r#"module mutable.fixture;
@id("mutable.update") fn update(state:i64, arg:i64)->i64 { state + arg }
@id("mutable.make") fn make(state:i64)->FnMutI64(i64)->i64 {
    mut fn(arg:i64)->i64 { state = update(state, arg); state }
}
@id("app.main") fn main()->i64 { 0 }
"#;
    let errors =
        prepare_native_rust_mutable_callback(source, Path::new("mutable.spx"), "mutable.make")
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
