use super::borrowed_input::{render_borrowed_input_adapter, BorrowedInputProfile};
use std::{
    fs,
    path::{Path, PathBuf},
    process::Command,
};

const REGEX_LOCK: &str = include_str!("../../../../Cargo.lock");

#[test]
fn borrowed_regex_adapters_use_direct_str_and_byte_slice_method_pointers() {
    for (profile, owner, input, method) in [
        (
            BorrowedInputProfile::RegexUtf8,
            "regex_alias::Regex",
            "&str",
            "regex_alias::Regex::is_match",
        ),
        (
            BorrowedInputProfile::RegexBytes,
            "regex_alias::bytes::Regex",
            "&[u8]",
            "regex_alias::bytes::Regex::is_match",
        ),
    ] {
        let source = render_borrowed_input_adapter(profile).unwrap();
        assert!(source.contains(&format!("let target:fn(&{owner},{input})->bool={method};")));
        assert!(source.contains("let matched=target(&self.owner,input);"));
        assert!(source.contains("let input_pointer=input.as_ptr();let input_length=input.len();"));
        assert!(source.contains("debug_assert_eq!(self.adapter_copies.get(),0);"));
        assert!(!source.contains("input.to_owned()"));
        assert!(!source.contains("input.to_vec()"));
        assert!(!source.contains("String::from(input)"));
    }
}

#[test]
fn borrowed_regex_adapter_guards_reentry_before_the_selected_target() {
    let source = render_borrowed_input_adapter(BorrowedInputProfile::RegexUtf8).unwrap();
    let rejection = source
        .find("if self.active.replace(true){self.rejected_reentries.set")
        .unwrap();
    let callback = source.find("pre_call(self);").unwrap();
    let target = source.find("let target:fn(").unwrap();
    let invocation = source
        .find("let matched=target(&self.owner,input);")
        .unwrap();
    assert!(rejection < callback);
    assert!(callback < target);
    assert!(target < invocation);
    assert!(source.contains("pub fn rejected_reentries(&self)->usize"));
    assert!(source.contains("pub fn target_calls(&self)->usize"));
    assert!(source.contains("pub fn last_input_pointer(&self)->usize"));
    assert!(source.contains("pub fn last_input_length(&self)->usize"));
}

#[test]
fn borrowed_regex_adapters_execute_physically_at_o0_o2_and_reject_guard_copy_controls() {
    let cargo = std::env::var("CARGO").expect("Cargo configures an absolute CARGO for RI-06");
    assert!(Path::new(&cargo).is_absolute());
    assert!(REGEX_LOCK.contains("name = \"regex\"\nversion = \"1.13.1\""));

    let root = std::env::temp_dir().join(format!(
        "semaprax-ri06-borrowed-input-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&root);
    fs::create_dir(&root).unwrap();
    let target_root = private_target_root();
    let _ = fs::remove_dir_all(&target_root);
    require_disk_space(&target_root);

    let utf8 = render_borrowed_input_adapter(BorrowedInputProfile::RegexUtf8).unwrap();
    let bytes = render_borrowed_input_adapter(BorrowedInputProfile::RegexBytes).unwrap();
    let cases: Vec<(&str, &str, String, bool)> = vec![
        ("utf8-o0", "utf8", utf8.clone(), true),
        ("utf8-o2", "utf8", utf8.clone(), true),
        ("bytes-o0", "bytes", bytes.clone(), true),
        ("bytes-o2", "bytes", bytes, true),
        (
            "guard-removed-o2",
            "utf8",
            utf8.replace(
                "if self.active.replace(true){self.rejected_reentries.set(self.rejected_reentries.get()+1);return Err(SpxBorrowedInputError::Reentered)}",
                "if false{self.rejected_reentries.set(self.rejected_reentries.get()+1);return Err(SpxBorrowedInputError::Reentered)}",
            ),
            false,
        ),
        (
            "copy-introduced-o2",
            "utf8",
            utf8.replace(
                "let matched=target(&self.owner,input);",
                "let copied=input.to_owned();self.adapter_copies.set(self.adapter_copies.get()+copied.len());let matched=target(&self.owner,&copied);",
            ),
            false,
        ),
    ];
    // `output` waits for each consumer before the next one starts: the private
    // target directory is never shared with another physical invocation.
    for (label, profile, source, expected_success) in cases {
        let optimization = if label.ends_with("o0") { "0" } else { "2" };
        let source = harness_source(&source, profile);
        let case = root.join(label);
        fs::create_dir(&case).unwrap();
        fs::write(case.join("Cargo.toml"), manifest(label)).unwrap();
        fs::write(case.join("src.rs"), source).unwrap();
        let source = case.join("src.rs");
        fs::create_dir(case.join("src")).unwrap();
        fs::rename(&source, case.join("src/main.rs")).unwrap();

        let lock = Command::new(&cargo)
            .args(["generate-lockfile", "--offline"])
            .current_dir(&case)
            .output()
            .unwrap();
        assert!(
            lock.status.success(),
            "{label} offline lockfile: {}",
            String::from_utf8_lossy(&lock.stderr)
        );

        let run = Command::new(&cargo)
            .args(["run", "--locked", "--offline", "--quiet"])
            .current_dir(&case)
            .env(
                "CARGO_TARGET_DIR",
                target_root.join(format!("o{optimization}")),
            )
            .env("CARGO_PROFILE_DEV_OPT_LEVEL", optimization)
            .output()
            .unwrap();
        assert_eq!(
            run.status.success(),
            expected_success,
            "{label}: {}",
            String::from_utf8_lossy(&run.stderr)
        );
    }
    fs::remove_dir_all(root).unwrap();
    fs::remove_dir_all(target_root).unwrap();
}

fn private_target_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("native Rust interop crate is beneath the checkout root")
        .join("target")
        .join(format!("ri06-borrowed-input-{}", std::process::id()))
}

fn require_disk_space(target: &Path) {
    let checkout = target
        .parent()
        .expect("private target has checkout target parent");
    let output = Command::new("df")
        .args(["-k", checkout.to_str().expect("checkout target is UTF-8")])
        .output()
        .expect("df is required for the RI-06 physical harness");
    assert!(
        output.status.success(),
        "df failed before RI-06 physical harness: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let available_kib = String::from_utf8(output.stdout)
        .unwrap()
        .lines()
        .last()
        .and_then(|line| line.split_whitespace().nth(3))
        .and_then(|value| value.parse::<u64>().ok())
        .expect("df output has available KiB");
    assert!(
        available_kib >= 524_288,
        "RI-06 physical harness needs at least 512 MiB free under {}",
        checkout.display()
    );
}

fn manifest(label: &str) -> String {
    format!(
        "[package]\nname = \"semaprax-ri06-{label}\"\nversion = \"0.1.0\"\nedition = \"2021\"\npublish = false\n\n[dependencies]\nregex_alias = {{ package = \"regex\", version = \"=1.13.1\" }}\n"
    )
}

fn harness_source(adapter: &str, profile: &str) -> String {
    let body = match profile {
        "utf8" => {
            r#"
let input=String::from("https://example.invalid/🦀");
let pointer=input.as_ptr() as usize;let length=input.len();
let adapter=SpxBorrowedInputAdapter::new(regex_alias::Regex::new(r"example\.invalid").unwrap());
assert!(adapter.invoke_with_pre_call(input.as_str(),|nested|{
    assert_eq!(nested.target_calls(),0);
    assert!(matches!(nested.invoke(input.as_str()),Err(SpxBorrowedInputError::Reentered)));
    assert_eq!(nested.rejected_reentries(),1);
    assert_eq!(nested.target_calls(),0);
}).unwrap());
assert_eq!(adapter.adapter_copies(),0);
assert_eq!(adapter.last_input_pointer(),pointer);
assert_eq!(adapter.last_input_length(),length);
assert_eq!(adapter.target_calls(),1);
"#
        }
        "bytes" => {
            r#"
let input=b"https://example.invalid/\xF0\x9F\xA6\x80".to_vec();
let pointer=input.as_ptr() as usize;let length=input.len();
let adapter=SpxBorrowedInputAdapter::new(regex_alias::bytes::Regex::new(r"example\.invalid").unwrap());
assert!(adapter.invoke_with_pre_call(input.as_slice(),|nested|{
    assert_eq!(nested.target_calls(),0);
    assert!(matches!(nested.invoke(input.as_slice()),Err(SpxBorrowedInputError::Reentered)));
    assert_eq!(nested.rejected_reentries(),1);
    assert_eq!(nested.target_calls(),0);
}).unwrap());
assert_eq!(adapter.adapter_copies(),0);
assert_eq!(adapter.last_input_pointer(),pointer);
assert_eq!(adapter.last_input_length(),length);
assert_eq!(adapter.target_calls(),1);
"#
        }
        _ => panic!("unsupported borrowed-input physical profile"),
    };
    format!(
        "extern crate regex_alias as regex_external;\nmod regex_alias{{pub use super::regex_external::Regex;pub mod bytes{{pub use super::super::regex_external::bytes::Regex;}}}}\n{adapter}\nfn main(){{{body}}}\n"
    )
}
