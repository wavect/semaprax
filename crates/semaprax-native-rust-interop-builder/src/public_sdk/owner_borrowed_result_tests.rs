use super::owner_borrowed_result::render_regex_result_owner_carrier;
use std::{fs, path::Path, process::Command};

#[test]
fn regex_result_owner_carrier_is_closed_and_direct() {
    let source = render_regex_result_owner_carrier(
        "regex_alias::Regex",
        "regex_alias::Regex::new",
        "regex_alias::Regex::is_match",
        "regex_alias::Error",
    )
    .unwrap();
    assert!(!source.contains("@OWNER@"));
    assert!(source.contains("fn with_utf8<T>(data: *const u8"));
    assert!(source.contains("target is invoked before this borrowed range can escape the carrier"));
    assert!(source.contains("let value = slot.value.as_ref().ok_or(3)?"));
    assert!(source.contains("Ok(regex_alias::Regex::is_match(value, text))"));
    assert!(source.contains("contexts.try_borrow_mut().map_err(|_| 6)?"));
    assert!(source.contains("#[cfg(test)]\nfn test_hold_context_borrow"));
    assert!(!source.contains("String::from(text)"));
    assert!(!source.contains("text.to_owned()"));
    assert!(!source.contains("text.to_string()"));
    assert!(render_regex_result_owner_carrier(
        "regex_alias::Regex",
        "regex_alias::Regex::new",
        "regex_alias::Regex::find",
        "regex_alias::Error",
    )
    .is_err());
}

#[test]
fn pinned_regex_result_owner_executes_without_adapter_copies() {
    let cargo = std::env::var("CARGO").expect("configured absolute CARGO");
    assert!(Path::new(&cargo).is_absolute());
    let root =
        std::env::temp_dir().join(format!("semaprax-ri06-result-owner-{}", std::process::id()));
    let target = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("interop crate checkout root")
        .join("target")
        .join(format!("ri06-result-owner-{}", std::process::id()));
    let _ = fs::remove_dir_all(&root);
    let _ = fs::remove_dir_all(&target);
    fs::create_dir(&root).unwrap();
    fs::create_dir(root.join("src")).unwrap();
    fs::write(
        root.join("Cargo.toml"),
        "[package]\nname=\"semaprax-ri06-result-owner\"\nversion=\"0.1.0\"\nedition=\"2021\"\npublish=false\n[dependencies]\nregex_alias={package=\"regex\",version=\"=1.13.1\"}\n",
    )
    .unwrap();
    let carrier = render_regex_result_owner_carrier(
        "regex_alias::Regex",
        "regex_alias::Regex::new",
        "regex_alias::Regex::is_match",
        "regex_alias::Error",
    )
    .unwrap();
    fs::write(root.join("src/lib.rs"), harness(&carrier)).unwrap();
    let locked = Command::new(&cargo)
        .args(["generate-lockfile", "--offline"])
        .current_dir(&root)
        .env("CARGO_BUILD_JOBS", "1")
        .env("CARGO_INCREMENTAL", "0")
        .output()
        .unwrap();
    assert!(
        locked.status.success(),
        "{}",
        String::from_utf8_lossy(&locked.stderr)
    );
    for level in ["0", "2"] {
        let run = Command::new(&cargo)
            .args(["test", "--locked", "--offline", "--quiet"])
            .current_dir(&root)
            .env("CARGO_BUILD_JOBS", "1")
            .env("CARGO_INCREMENTAL", "0")
            .env("CARGO_TARGET_DIR", target.join(format!("o{level}")))
            .env("CARGO_PROFILE_TEST_OPT_LEVEL", level)
            .output()
            .unwrap();
        assert!(
            run.status.success(),
            "O{level}: {}",
            String::from_utf8_lossy(&run.stderr)
        );
    }
    let _ = fs::remove_dir_all(root);
    let _ = fs::remove_dir_all(target);
}

fn harness(carrier: &str) -> String {
    format!(
        "extern crate regex_alias;\n{carrier}\n#[cfg(test)]#[test]fn pinned_physical_result_owner(){{\nlet context=spx_result_owner_context_new();assert_ne!(context,0);assert_eq!(test_hold_context_borrow(context),6);\nlet pattern=String::from(r\"example\\.invalid\");let mut created=SpxOwnerResult::default();\nassert_eq!(unsafe{{spx_result_owner_new_utf8(context,pattern.as_ptr(),pattern.len() as u64,&mut created)}},0);assert_eq!(created.tag,1);assert_eq!(spx_result_owner_adapter_copies(),0);assert_eq!(spx_result_owner_last_input_pointer(),pattern.as_ptr() as usize);assert_eq!(spx_result_owner_last_input_length(),pattern.len());\nlet input=String::from(\"https://example.invalid/🦀\");let mut matched=9u8;assert_eq!(unsafe{{spx_result_owner_is_match_utf8(context,created.owner,input.as_ptr(),input.len() as u64,&mut matched)}},0);assert_eq!(matched,1);assert_eq!(spx_result_owner_adapter_copies(),0);assert_eq!(spx_result_owner_last_input_pointer(),input.as_ptr() as usize);assert_eq!(spx_result_owner_last_input_length(),input.len());\nlet mut invalid=SpxOwnerResult{{tag:9,..Default::default()}};assert_eq!(unsafe{{spx_result_owner_new_utf8(context,b\"(\".as_ptr(),1,&mut invalid)}},0);assert_eq!(invalid.tag,2);assert_eq!(invalid.error,1);\nlet mut rejected=SpxOwnerResult{{tag:9,..Default::default()}};assert_eq!(unsafe{{spx_result_owner_new_utf8(context,[0xffu8].as_ptr(),1,&mut rejected)}},3);assert_eq!(rejected.tag,9);\nlet other=spx_result_owner_context_new();assert_ne!(other,0);assert_eq!(unsafe{{spx_result_owner_is_match_utf8(other,created.owner,input.as_ptr(),input.len() as u64,&mut matched)}},3);assert_eq!(unsafe{{spx_result_owner_drop(context,created.owner)}},0);assert_eq!(spx_result_owner_context_close(context),0);assert_eq!(spx_result_owner_context_close(other),0);\n}}"
    )
}
