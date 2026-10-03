//! Generated public Rust facade: affine owners and context-tied lifetimes.
use super::*;

struct Scratch(std::path::PathBuf);
impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

#[test]
fn owned_context_public_lifetimes_and_isolated_panic_policy() {
    let rustc = std::env::var("RUSTC").expect("configured absolute rustc");
    let clang = std::env::var("CLANG").expect("configured absolute clang");
    let root = std::env::temp_dir().join(format!("semaprax-owner-context-{}", std::process::id()));
    fs::create_dir(&root).unwrap();
    let _scratch = Scratch(root.clone());
    let source = semaprax::check(SOURCE, "context.spx").unwrap();
    let generated = prepare_opaque_owner_native(&source, "owner.run").unwrap();
    fs::write(root.join("owner.h"), &generated.header).unwrap();
    fs::write(root.join("owner.c"), &generated.c_source).unwrap();
    let compiled = Command::new(&clang)
        .args(["-std=c11", "-Wall", "-Wextra", "-Werror", "-O2", "-c"])
        .arg(root.join("owner.c"))
        .arg("-o")
        .arg(root.join("owner.o"))
        .output()
        .unwrap();
    assert!(
        compiled.status.success(),
        "{}",
        String::from_utf8_lossy(&compiled.stderr)
    );
    let fixture = FIXTURE.split_once("fn invoke(").unwrap().0;
    // This is a separate crate: downstream code gets only the public API.
    let sdk = format!(
        "{}\n{}",
        generated.rust_adapter,
        fixture.replace("mod fixture_regex", "pub mod fixture_regex")
    );
    fs::write(root.join("sdk.rs"), &sdk).unwrap();
    for panic in ["unwind", "abort"] {
        let library = root.join(format!("libgenerated_sdk_{panic}.rlib"));
        let compiled = Command::new(&rustc)
            .args([
                "--edition=2021",
                "--crate-name",
                "generated_sdk",
                "--crate-type=rlib",
                "-Dwarnings",
                "-C",
                &format!("panic={panic}"),
            ])
            .arg(root.join("sdk.rs"))
            .arg("-o")
            .arg(&library)
            .output()
            .unwrap();
        assert!(
            compiled.status.success(),
            "{}",
            String::from_utf8_lossy(&compiled.stderr)
        );
        fs::write(root.join("main.rs"), MAIN).unwrap();
        let binary = root.join(format!("consumer-{panic}"));
        let compiled = Command::new(&rustc)
            .args([
                "--edition=2021",
                "-Dwarnings",
                "-C",
                &format!("panic={panic}"),
                "--extern",
            ])
            .arg(format!("generated_sdk={}", library.display()))
            .arg(root.join("main.rs"))
            .arg("-C")
            .arg(format!("link-arg={}", root.join("owner.o").display()))
            .arg("-o")
            .arg(&binary)
            .output()
            .unwrap();
        assert!(
            compiled.status.success(),
            "{}",
            String::from_utf8_lossy(&compiled.stderr)
        );
        let ran = Command::new(&binary).output().unwrap();
        assert!(
            ran.status.success(),
            "{}",
            String::from_utf8_lossy(&ran.stderr)
        );
        let dropped = Command::new(&binary).arg("drop-panic").output().unwrap();
        assert!(String::from_utf8_lossy(&dropped.stderr).contains("drop panic"));
        if panic == "unwind" {
            assert!(
                dropped.status.success(),
                "{}",
                String::from_utf8_lossy(&dropped.stderr)
            );
            assert_eq!(dropped.stdout, b"drop-panic-contained-once\n");
        } else {
            assert!(!dropped.status.success());
            #[cfg(unix)]
            {
                use std::os::unix::process::ExitStatusExt;
                assert_eq!(dropped.status.signal(), Some(6));
            }
            assert!(dropped.stdout.is_empty());
        }
        if panic == "unwind" {
            for (label, body, diagnostic) in [
                ("close-live", "let c=SpxOwnerContext::new().unwrap();let o=c.construct(7).unwrap();c.close().unwrap();o.consume(7).unwrap();", "E0505"),
                ("closed-context", "let c=SpxOwnerContext::new().unwrap();c.close().unwrap();c.construct(7).unwrap();", "E0382"),
                ("double-consume", "let c=SpxOwnerContext::new().unwrap();let o=c.construct(7).unwrap();o.consume(7).unwrap();o.consume(7).unwrap();", "E0382"),
                ("implicit-copy", "let c=SpxOwnerContext::new().unwrap();let o=c.construct(7).unwrap();let copied=o;o.consume(7).unwrap();drop(copied);", "E0382"),
                ("escape", "let _o={let c=SpxOwnerContext::new().unwrap();c.construct(7).unwrap()};", "E0597"),
                ("wrong-thread", "let c=SpxOwnerContext::new().unwrap();std::thread::spawn(move||{c.close().unwrap();});", "E0277"),
            ] {
                fs::write(root.join("negative.rs"), format!("use generated_sdk::SpxOwnerContext;fn main(){{{body}}}")).unwrap();
                let compiled = Command::new(&rustc).args(["--edition=2021", "--emit=metadata", "--extern"])
                    .arg(format!("generated_sdk={}",library.display())).arg(root.join("negative.rs"))
                    .arg("-o").arg(root.join("negative.rmeta")).output().unwrap();
                assert!(!compiled.status.success(), "{label} accepted");
                assert!(String::from_utf8_lossy(&compiled.stderr).contains(diagnostic), "{label}: {}", String::from_utf8_lossy(&compiled.stderr));
            }
        }
    }
}

const MAIN: &str = r#"
use generated_sdk::{SpxOwnerContext,fixture_regex,spx_owner_call};
fn trace(expected:&[i64]){fixture_regex::DROPS.with(|d|assert_eq!(&*d.borrow(),expected));}
fn clear(){fixture_regex::DROPS.with(|d|d.borrow_mut().clear());}
fn main(){
 if std::env::args().nth(1).as_deref()==Some("drop-panic"){
  fixture_regex::PANIC_DROP.store(true,std::sync::atomic::Ordering::Relaxed);
  let context=SpxOwnerContext::new().unwrap();
  let owner=context.construct(88).unwrap();
  assert_eq!(owner.dispose(),Err(2));trace(&[88]);context.close().unwrap();
  println!("drop-panic-contained-once");return;
 }
 assert_eq!(spx_owner_call(7,7,1),Ok(1));trace(&[7,88,99]);clear();
 let context=SpxOwnerContext::new().unwrap();
 let first=context.construct(7).unwrap();let second=context.construct(8).unwrap();
 assert_eq!(first.consume(7),Ok(true));drop(second);trace(&[7,8]);context.close().unwrap();clear();
 let context=SpxOwnerContext::new().unwrap();
 context.construct(9).unwrap().dispose().unwrap();trace(&[9]);context.close().unwrap();clear();
 if cfg!(panic="unwind"){
  let context=SpxOwnerContext::new().unwrap();
  assert_eq!(context.construct(7).unwrap().consume(-777),Err(2));trace(&[7]);context.close().unwrap();
 }
}
"#;
