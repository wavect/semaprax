use super::{private_target_root, require_disk_space};
use crate::public_sdk::borrowed_input::{
    render_borrowed_input_adapter, render_borrowed_url_adapter, BorrowedInputProfile,
};
use std::{
    fs,
    path::{Path, PathBuf},
    process::Command,
};

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

#[test]
fn borrowed_view_generated_regex_url_scope_and_negative_controls() {
    let cargo = std::env::var("CARGO").expect("Cargo provides its absolute executable");
    assert!(Path::new(&cargo).is_absolute());
    let root =
        std::env::temp_dir().join(format!("semaprax-ri06-scoped-view-{}", std::process::id()));
    let target =
        private_target_root().with_file_name(format!("ri06-scoped-view-{}", std::process::id()));
    require_disk_space(&target);
    fs::create_dir(&root).unwrap();
    let _scratch = Scratch {
        root: root.clone(),
        target: target.clone(),
    };
    fs::create_dir(root.join("src")).unwrap();
    fs::write(root.join("src/main.rs"), "fn main() {}\n").unwrap();
    fs::write(root.join("Cargo.toml"), "[package]\nname = \"semaprax-ri06-scoped-view\"\nversion = \"0.1.0\"\nedition = \"2021\"\n[dependencies]\nregex_alias = { package = \"regex\", version = \"=1.13.1\" }\nurl_alias = { package = \"url\", version = \"=2.5.8\" }\n[workspace]\n").unwrap();
    let lock = Command::new(&cargo)
        .args(["generate-lockfile", "--offline"])
        .current_dir(&root)
        .output()
        .unwrap();
    assert!(
        lock.status.success(),
        "{}",
        String::from_utf8_lossy(&lock.stderr)
    );
    let regex = render_borrowed_input_adapter(BorrowedInputProfile::RegexUtf8).unwrap();
    let bytes = render_borrowed_input_adapter(BorrowedInputProfile::RegexBytes).unwrap();
    let url = render_borrowed_url_adapter();
    let source = |regex: &str, bytes: &str, url: &str, body: &str| {
        format!(
        "mod regex_adapter{{{regex}}}\nmod bytes_adapter{{{bytes}}}\nmod url_adapter{{{url}}}\n{ALLOCATOR}\nfn main(){{{body}}}\n"
    )
    };
    let compile = |text: &str, optimization: &str, check_only: bool| {
        fs::write(root.join("src/main.rs"), text).unwrap();
        Command::new(&cargo)
            .args([
                if check_only { "check" } else { "build" },
                "--locked",
                "--offline",
                "--quiet",
            ])
            .current_dir(&root)
            .env("CARGO_TARGET_DIR", target.join(format!("o{optimization}")))
            .env("CARGO_PROFILE_DEV_OPT_LEVEL", optimization)
            .env("CARGO_PROFILE_DEV_DEBUG", "0")
            .env("CARGO_INCREMENTAL", "0")
            .env("CARGO_BUILD_JOBS", "1")
            .output()
            .unwrap()
    };
    for optimization in ["0", "2"] {
        let built = compile(&source(&regex, &bytes, &url, SUCCESS), optimization, false);
        assert!(
            built.status.success(),
            "O{optimization}: {}",
            String::from_utf8_lossy(&built.stderr)
        );
        let run =
            Command::new(target.join(format!("o{optimization}/debug/semaprax-ri06-scoped-view")))
                .output()
                .unwrap();
        assert!(
            run.status.success(),
            "O{optimization}: {}",
            String::from_utf8_lossy(&run.stderr)
        );
    }
    let guard = "if self.active.replace(true){self.rejected_reentries.set(self.rejected_reentries.get()+1);return Err(SpxBorrowedInputError::Reentered)}";
    assert!(url.contains(guard));
    let unguarded = url.replacen(guard, "", 1);
    let copied = url.replace(
        "let value=target(&self.owner);",
        "let copied=target(&self.owner).to_owned();let value=copied.as_str();",
    );
    for (name, mutation) in [("guard removed", unguarded), ("view copied", copied)] {
        let built = compile(&source(&regex, &bytes, &mutation, SUCCESS), "2", false);
        assert!(
            built.status.success(),
            "{name} control must compile: {}",
            String::from_utf8_lossy(&built.stderr)
        );
        let run = Command::new(target.join("o2/debug/semaprax-ri06-scoped-view"))
            .output()
            .unwrap();
        assert!(!run.status.success(), "{name} control bypassed assertions");
        assert!(
            String::from_utf8_lossy(&run.stderr).contains("assertion"),
            "{name} did not fail an assertion"
        );
    }
    for (name, body, expected) in [
        ("owner drop", "let adapter=make();adapter.with_str_view(|v|{drop(adapter);v.as_str().len()}).unwrap();", "E0505"),
        ("owner move", "let adapter=make();adapter.with_str_view(|v|{let _=adapter.into_owner();v.as_str().len()}).unwrap();", "E0505"),
        ("replacement", "let mut adapter=make();adapter.with_str_view(|v|{adapter.replace_owner(url_alias::Url::parse(\"https://replacement.invalid\").unwrap());v.as_str().len()}).unwrap();", "E0502"),
        ("exclusive mutation", "let mut adapter=make();adapter.with_str_view(|v|{adapter.with_exclusive(|u|u.set_path(\"changed\")).unwrap();v.as_str().len()}).unwrap();", "E0502"),
        ("temporary escape", "let view=make().with_str_view(|v|v).unwrap();let _=view.as_str();", "lifetime may not live long enough"),
        ("stored escape", "let adapter=make();let mut saved=None;adapter.with_str_view(|v|saved=Some(v)).unwrap();drop(saved);", "E0521"),
        ("async escape", "let adapter=make();let _future=adapter.with_str_view(|v|async move {v.as_str().len()}).unwrap();", "lifetime may not live long enough"),
    ] {
        let body = format!("fn make()->url_adapter::SpxBorrowedInputAdapter{{url_adapter::SpxBorrowedInputAdapter::new(url_alias::Url::parse(\"https://example.invalid\").unwrap())}}{body}");
        let output = compile(&source(&regex, &bytes, &url, &body), "2", true);
        let errors = String::from_utf8_lossy(&output.stderr);
        assert!(!output.status.success(), "{name} escaped its scope");
        assert!(errors.contains(expected), "{name} failed for an unrelated reason: {errors}");
    }
    fs::remove_dir_all(root).unwrap();
    fs::remove_dir_all(target).unwrap();
}

const ALLOCATOR: &str = r#"
use std::sync::atomic::{AtomicBool,AtomicUsize,Ordering};
static MEASURE:AtomicBool=AtomicBool::new(false);
static ALLOCATIONS:AtomicUsize=AtomicUsize::new(0);
struct Measured;
unsafe impl std::alloc::GlobalAlloc for Measured {
 unsafe fn alloc(&self,l:std::alloc::Layout)->*mut u8{if MEASURE.load(Ordering::Relaxed){ALLOCATIONS.fetch_add(1,Ordering::Relaxed);}std::alloc::System.alloc(l)}
 unsafe fn dealloc(&self,p:*mut u8,l:std::alloc::Layout){std::alloc::System.dealloc(p,l)}
 unsafe fn realloc(&self,p:*mut u8,l:std::alloc::Layout,n:usize)->*mut u8{if MEASURE.load(Ordering::Relaxed){ALLOCATIONS.fetch_add(1,Ordering::Relaxed);}std::alloc::System.realloc(p,l,n)}
}
#[global_allocator]static ALLOCATOR:Measured=Measured;
"#;

const SUCCESS: &str = r#"
let original=regex_alias::Regex::new(r"example\.invalid").unwrap();
let pointer=original.as_str().as_ptr();
let regex=regex_adapter::SpxBorrowedInputAdapter::new(original);
let independent=regex_adapter::SpxBorrowedInputAdapter::new(regex_alias::Regex::new("independent").unwrap());
regex.with_str_view(|view|{
 assert_eq!(view.as_str(),r"example\.invalid");assert_eq!(view.as_str().as_ptr(),pointer);
 let calls=regex.target_calls();
 assert!(matches!(regex.invoke("example.invalid"),Err(regex_adapter::SpxBorrowedInputError::Reentered)));
 assert_eq!(regex.target_calls(),calls);
 assert!(independent.invoke("independent").unwrap());
 std::mem::forget(view);
 assert!(regex.with_str_view(|_|()).is_err());
 assert_eq!(regex.target_calls(),calls);
}).unwrap();
assert!(regex.invoke("https://example.invalid").unwrap());
let bytes_owner=regex_alias::bytes::Regex::new(r"(?-u:\xFF)").unwrap();
let bytes_pointer=bytes_owner.as_str().as_ptr();
let bytes=bytes_adapter::SpxBorrowedInputAdapter::new(bytes_owner);
bytes.with_str_view(|v|assert_eq!(v.as_str().as_ptr(),bytes_pointer)).unwrap();
assert!(bytes.invoke(&[255]).unwrap());
let original=url_alias::Url::parse("https://example.invalid/path?q=1").unwrap();
let pointer=original.as_str().as_ptr();
let mut url=url_adapter::SpxBorrowedInputAdapter::new(original);
MEASURE.store(true,Ordering::Relaxed);
url.with_str_view(|view|{
 assert_eq!(view.as_str(),"https://example.invalid/path?q=1");assert_eq!(view.as_str().as_ptr(),pointer);
 let calls=url.target_calls();
 assert!(url.with_str_view(|_|()).is_err());assert_eq!(url.target_calls(),calls);
}).unwrap();
MEASURE.store(false,Ordering::Relaxed);
assert_eq!(ALLOCATIONS.load(Ordering::Relaxed),0);
assert_eq!(url.rejected_reentries(),1);
let unwind=std::panic::catch_unwind(std::panic::AssertUnwindSafe(||{let _=url.with_str_view::<()>(|_|panic!("selected callback panic"));}));
assert!(unwind.is_err());
url.with_exclusive(|owner|owner.set_path("a-longer-new-path")).unwrap();
url.with_str_view(|view|assert!(view.as_str().contains("a-longer-new-path"))).unwrap();
let old=url.replace_owner(url_alias::Url::parse("https://replacement.invalid").unwrap());
assert!(old.as_str().contains("a-longer-new-path"));
assert_eq!(url.into_owner().host_str(),Some("replacement.invalid"));
"#;
