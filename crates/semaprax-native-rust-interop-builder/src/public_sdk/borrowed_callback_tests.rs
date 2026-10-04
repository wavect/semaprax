//! Physical RI-06 owner view to source-created RI-08 borrowed closure.
use super::borrowed_input_tests::{private_target_root, require_disk_space};
use super::*;
use std::{fs, process::Command};
const SOURCE: &str = r#"module borrowed.fixture;
@id("borrow.compute") fn compute(view:borrow str,arg:i64)->i64
    requires arg != -9 ensures result != 99 {
    let bytes=str_as_bytes(view);
    match byte_get(bytes,0usize) {
        Option::Some { value:first } => if first==104u8 { str_len_bytes(view)+arg } else { -999 },
        Option::None {} => -999,
    }
}
@id("borrow.run") fn run(view:borrow str,arg:i64)->i64 {
    let callback=fn(value:i64)->i64 { compute(view,value) };
    callback(arg)
}
@id("app.main") fn main()->i64 {0}
"#;

#[test]
fn borrowed_callback_renderer_authenticates_actual_source_capture() {
    let baseline =
        prepare_native_rust_borrowed_callback(SOURCE, Path::new("borrow.spx"), "borrow.run")
            .unwrap();
    let canonical = semaprax::format::canonical(&semaprax::check(SOURCE, "borrow.spx").unwrap());
    assert_eq!(
        baseline,
        prepare_native_rust_borrowed_callback(&canonical, Path::new("borrow.spx"), "borrow.run")
            .unwrap()
    );
    for source in [
        SOURCE.replace("compute(view,value)", "value+1"),
        SOURCE.replace("callback(arg)", "callback(arg)+1"),
    ] {
        let errors =
            prepare_native_rust_borrowed_callback(&source, Path::new("borrow.spx"), "borrow.run")
                .unwrap_err();
        assert!(errors.iter().any(|e| e.code == "SPX-B154"), "{errors:?}");
    }
}

struct Scratch {
    root: PathBuf,
    target: PathBuf,
}
impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
        let _ = fs::remove_dir_all(&self.target);
    }
}

// Runs nested Cargo serially. Invoke this exact test binary directly when
// another compiler task is using the machine's second Cargo slot.
#[test]
fn borrowed_callback_real_url_view_iterator_scope_and_negative_controls() {
    let cargo = std::env::var("CARGO").expect("Cargo executable");
    assert!(Path::new(&cargo).is_absolute());
    let root = std::env::temp_dir().join(format!("spx-borrowed-callback-{}", std::process::id()));
    let target = private_target_root()
        .with_file_name(format!("ri08-borrowed-callback-{}", std::process::id()));
    require_disk_space(&target);
    fs::create_dir(&root).unwrap();
    let _scratch = Scratch {
        root: root.clone(),
        target: target.clone(),
    };
    fs::create_dir(root.join("src")).unwrap();
    fs::write(root.join("Cargo.toml"),"[package]\nname=\"borrowed-callback-fixture\"\nversion=\"0.1.0\"\nedition=\"2021\"\n[dependencies]\nurl_alias={package=\"url\",version=\"=2.5.8\"}\n[workspace]\n").unwrap();
    fs::write(root.join("build.rs"),"fn main(){println!(\"cargo:rustc-link-arg={}\",std::path::Path::new(&std::env::var(\"CARGO_MANIFEST_DIR\").unwrap()).join(\"module.o\").display());}\n").unwrap();
    fs::write(root.join("src/main.rs"), "fn main(){}\n").unwrap();
    let lock = Command::new(&cargo)
        .current_dir(&root)
        .args(["generate-lockfile", "--offline"])
        .output()
        .unwrap();
    assert!(
        lock.status.success(),
        "{}",
        String::from_utf8_lossy(&lock.stderr)
    );
    let url = borrowed_input::render_borrowed_url_adapter();
    let render = |source: &str, copy: bool| {
        let projection =
            prepare_native_rust_borrowed_callback(source, Path::new("borrow.spx"), "borrow.run")
                .unwrap();
        fs::write(
            root.join("module.c"),
            format!("{}\n{C_PROBE}", projection.c_source),
        )
        .unwrap();
        let adapter = if copy {
            projection.safe_rust.replace("consumer(BorrowedCallback{view,", "let copied=view.to_owned(); let view=copied.as_str(); consumer(BorrowedCallback{view,")
        } else {
            projection.safe_rust
        };
        fs::write(root.join("src/lib.rs"),format!("pub mod callback{{{adapter}}}\npub mod owner{{{url}}}\nextern \"C\"{{fn borrowed_failure_probe()->i32;}}\npub fn failure_probe()->bool{{unsafe{{borrowed_failure_probe()==0}}}}\n")).unwrap();
    };
    let compile = |main: &str, optimization: &str, check: bool| {
        fs::write(root.join("src/main.rs"), main).unwrap();
        Command::new(&cargo)
            .current_dir(&root)
            .args([
                if check { "check" } else { "build" },
                "--locked",
                "--offline",
                "--quiet",
            ])
            .env("CARGO_TARGET_DIR", &target)
            .env("CARGO_BUILD_JOBS", "1")
            .env("CARGO_INCREMENTAL", "0")
            .env("CARGO_PROFILE_DEV_DEBUG", "0")
            .env("CARGO_PROFILE_DEV_OPT_LEVEL", optimization)
            .output()
            .unwrap()
    };
    for (label, source, copy, optimization, expected) in [
        ("O0", SOURCE.to_owned(), false, "0", true),
        ("O2", SOURCE.to_owned(), false, "2", true),
        (
            "authored-body",
            SOURCE.replace("str_len_bytes(view)+arg", "str_len_bytes(view)+arg+1"),
            false,
            "2",
            false,
        ),
        ("copied-view", SOURCE.to_owned(), true, "2", false),
    ] {
        render(&source, copy);
        let c = Command::new("clang")
            .current_dir(&root)
            .args([
                "-std=c11",
                &format!("-O{optimization}"),
                "-c",
                "module.c",
                "-o",
                "module.o",
            ])
            .output()
            .unwrap();
        assert!(
            c.status.success(),
            "{label}: {}",
            String::from_utf8_lossy(&c.stderr)
        );
        let built = compile(MAIN, optimization, false);
        assert!(
            built.status.success(),
            "{label}: {}",
            String::from_utf8_lossy(&built.stderr)
        );
        let run = Command::new(target.join("debug/borrowed-callback-fixture"))
            .output()
            .unwrap();
        assert_eq!(
            run.status.success(),
            expected,
            "{label}: {}",
            String::from_utf8_lossy(&run.stderr)
        );
    }
    render(SOURCE, false);
    for (label,body) in [
        ("callback-escape","let _escaped=with_borrowed_callback(\"hello\",|c|c);"),
        ("function-escape","let _escaped=with_borrowed_callback(\"hello\",|c|c.as_fn());"),
        ("async-escape","let _escaped=with_borrowed_callback(\"hello\",|c|async move {c.call(1)});"),
        ("thread","with_borrowed_callback(\"hello\",|c|{std::thread::scope(|s|{s.spawn(move||c.call(1));});});"),
        ("owner-move","let owner=owner();owner.with_str_view(|v|with_borrowed_callback(v.as_str(),|c|{drop(owner);c.call(1)})).unwrap();"),
        ("owner-mutation","let mut owner=owner();owner.with_str_view(|v|with_borrowed_callback(v.as_str(),|c|{owner.with_exclusive(|u|u.set_path(\"changed\")).unwrap();c.call(1)})).unwrap();"),
        ("view-escape","let owner=owner();let _escaped=owner.with_str_view(|v|v);"),
    ] {
        let main=format!("#![forbid(unsafe_code)]\nuse borrowed_callback_fixture::callback::*;\nfn owner()->borrowed_callback_fixture::owner::SpxBorrowedInputAdapter{{borrowed_callback_fixture::owner::SpxBorrowedInputAdapter::new(url_alias::Url::parse(\"https://example.invalid/\").unwrap())}}\nfn main(){{{body}}}");
        let output=compile(&main,"0",true);
        assert!(!output.status.success(),"{label} unexpectedly compiled");
        let stderr=String::from_utf8_lossy(&output.stderr);
        assert!(stderr.contains("lifetime")||stderr.contains("E0277")||stderr.contains("E0505")||stderr.contains("E0502")||stderr.contains("E0515"),"{label}: {stderr}");
    }
}

const C_PROBE: &str = r#"
int borrowed_failure_probe(void){
 const uint8_t text[]={104,195,169};int64_t out=777;uint32_t cls=0,code=0;
 if(spx_borrowed_callback_invoke(text,3,-9,&out,&cls,&code)!=1||out!=777||cls!=1||code!=1)return 1;
 if(spx_borrowed_callback_invoke(text,3,96,&out,&cls,&code)!=1||out!=777||cls!=1||code!=2)return 2;
 if(spx_borrowed_callback_invoke(text,3,INT64_MAX,&out,&cls,&code)!=1||out!=777||cls!=2||code!=1)return 3;
 return 0;
}
"#;
const MAIN: &str = r#"#![forbid(unsafe_code)]
use borrowed_callback_fixture::{callback::*,owner::SpxBorrowedInputAdapter};
fn main(){
 let original=url_alias::Url::parse("https://example.invalid/path?q=1").unwrap();
 let pointer=original.as_str().as_ptr();let length=original.as_str().len() as i64;
 let mut owner=SpxBorrowedInputAdapter::new(original);
 owner.with_str_view(|view|with_borrowed_callback(view.as_str(),|callback|{
  assert_eq!(view.as_str().as_ptr(),pointer);assert_eq!(callback.view_pointer(),pointer);assert_eq!(callback.view_length() as i64,length);
  assert_eq!([1,2,3].into_iter().map(callback.as_fn()).collect::<Result<Vec<_>,_>>().unwrap(),vec![length+1,length+2,length+3]);
  assert!(matches!(callback.call(-9),Err(BorrowedCallbackError::Source{class:1,code:1})));
  assert_eq!(callback.call_with_pre_call(1,|same|assert_eq!(same.call(2),Err(BorrowedCallbackError::Reentered))).unwrap(),length+1);
  assert!(owner.with_str_view(|_|()).is_err());
  let panic=std::panic::catch_unwind(std::panic::AssertUnwindSafe(||callback.call_with_pre_call(1,|_|panic!("selected Rust panic"))));
  assert!(panic.is_err());assert_eq!(callback.call(2).unwrap(),length+2);
 })).unwrap();
 let panic=std::panic::catch_unwind(std::panic::AssertUnwindSafe(||owner.with_str_view::<()>(|view|with_borrowed_callback(view.as_str(),|_|panic!("scope panic")))));
 assert!(panic.is_err());
 owner.with_exclusive(|u|u.set_path("changed")).unwrap();
 owner.with_str_view(|view|with_borrowed_callback(view.as_str(),|callback|assert_eq!(callback.call(1).unwrap(),view.as_str().len() as i64+1))).unwrap();
 with_borrowed_callback("xyz",|callback|assert_eq!(callback.call(1).unwrap(),-999));
 with_borrowed_callback("",|callback|assert_eq!(callback.call(1).unwrap(),-999));
 assert!(borrowed_callback_fixture::failure_probe());
 drop(owner.into_owner());
}
"#;
