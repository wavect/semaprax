//! Authored subprocess evidence for generated SDK protocol handling. The small
//! native ABI double is deliberately hostile; it is not provider-semantic proof.
use std::path::PathBuf;
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};

use super::*;

mod fatal_oracle;

static NEXT: AtomicU64 = AtomicU64::new(0);

#[test]
fn generated_v8_and_v10_boundaries_close_before_publication() {
    for (kind, result) in [
        (0, "owned-bytes"),
        (1, "option-owned-bytes"),
        (2, "result-owned-bytes-i64"),
        (3, "owned-utf8"),
        (4, "bool"),
        (5, "i64"),
        (6, "usize"),
    ] {
        let mut bytes = descriptor_bytes(result);
        let mode = if kind == 3 {
            bytes = String::from_utf8(bytes)
                .unwrap()
                .replace(
                    "semaprax.public-owned-data-api.v1",
                    "semaprax.public-owned-utf8-api.v1",
                )
                .replace("semaprax.project.v8", "semaprax.project.v10")
                .into_bytes();
            PackageMode::ProjectV10OwnedUtf8
        } else {
            PackageMode::ProjectV8
        };
        let digest = descriptor_digest_for_bytes(&bytes).unwrap();
        let descriptor =
            descriptor::replay(&bytes, &digest, &["fixture.value".to_owned()]).unwrap();
        let sources = render::render_sources(&descriptor, HostTarget::current().unwrap(), mode);
        run_boundary_fixture(
            kind,
            &sources.lib_rs,
            &sources.ffi_rs,
            "spx_fixture_dot_value",
        );
    }
}

pub(crate) fn run_boundary_fixture(kind: u32, lib: &str, ffi: &str, method: &str) {
    let root = std::env::temp_dir().join(format!(
        "semaprax-sdk-boundaries-{}-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos(),
        NEXT.fetch_add(1, Ordering::Relaxed),
    ));
    std::fs::create_dir(&root).unwrap();
    // Deliberately retain this bounded evidence fixture; never recursively
    // delete a caller-controlled temporary-directory tree.
    std::fs::write(root.join("sdk.rs"), lib).unwrap();
    let marker = "let mut guard=Guard{context:self,handle,armed:true};";
    let has_owner = kind <= 3 || kind == 7;
    assert_eq!(ffi.matches(marker).count(), usize::from(has_owner));
    let instrumented = ffi.replace(
        marker,
        &format!("{marker}\nif matches!(crate::mode(),21|28|29){{crate::event(\"unwind\");panic!(\"host unwind after owner guard\")}}"),
    );
    std::fs::write(root.join("owned_data_ffi.rs"), &instrumented).unwrap();
    std::fs::write(
        root.join("provider.rs"),
        include_str!("fixtures/owned_boundary_provider.rs"),
    )
    .unwrap();
    let entry = entry(kind, method);
    let source = format!(
        r#"
#![allow(dead_code)]
#[path="sdk.rs"]mod sdk;
include!("provider.rs");
const KIND:u32={kind};
{entry}
fn main(){{
    let selected:u32=std::env::args().nth(1).unwrap().parse().unwrap();
    MODE.store(selected,Ordering::Relaxed);
    let mut sdk=match sdk::NativeRustOwnedDataSdk::new(){{
        Ok(value)=>value,
        Err(error)=>{{assert_eq!(selected,18);assert_eq!(error,sdk::CallError::AdapterRejected);println!("constructor-rejected");return}}
    }};
    let result=std::panic::catch_unwind(std::panic::AssertUnwindSafe(||sdk.{method}()));
    // A returned error or caught panic is not process fail-stop. Publish this
    // witness before any harness assertion can itself terminate the process.
    {{use std::io::Write;let mut out=std::io::stdout().lock();writeln!(out,"call-completed").unwrap();out.flush().unwrap();}}
    if selected==21{{assert!(result.is_err());println!("unwind-settled")}}
    else{{
        let result=result.unwrap();
        let error=matches!(selected,4|5|6|11|12|13|14|20)||(selected==17&&KIND==3);
        assert_eq!(result.is_err(),error);
        if error{{assert_eq!(result.as_ref().err(),Some(&match selected{{12=>sdk::CallError::HostFailure,13=>sdk::CallError::SemanticFailure,_=>sdk::CallError::AdapterRejected}}));}}
        {value_assertions}
        println!("returned:{{}}",if error{{"error"}}else{{"value"}});
    }}
    if selected==19{{
        assert_eq!(sdk.{method}().err(),Some(sdk::CallError::AdapterRejected));println!("reinit-rejected");
    }}else{{
        MODE.store(0,Ordering::Relaxed);
        assert!(sdk.{method}().is_ok());println!("recovered");
    }}
    drop(sdk);
    println!("finished");
}}
"#,
        value_assertions = value_assertions(kind)
    );
    let harness = root.join("boundary.rs");
    std::fs::write(&harness, &source).unwrap();
    for optimization in ["0", "2"] {
        let executable: PathBuf = root.join(format!(
            "boundary-o{optimization}{}",
            std::env::consts::EXE_SUFFIX
        ));
        let output = Command::new("rustc")
            .args([
                "--edition=2021",
                "-C",
                "panic=unwind",
                "-C",
                &format!("opt-level={optimization}"),
            ])
            .arg(&harness)
            .arg("-o")
            .arg(&executable)
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        let mut ordinary = vec![0, 12, 13, 14, 18, 19];
        let mut fatal = vec![8, 22];
        if kind <= 3 || kind == 7 {
            ordinary.extend([4, 5, 6, 15, 16, 21]);
            fatal.extend([1, 7, 26, 27, 28, 29]);
        } else {
            fatal.push(1);
        }
        if kind <= 3 {
            ordinary.push(11);
            fatal.push(9);
        }
        if kind == 1 {
            ordinary.push(23);
            fatal.extend([2, 10]);
        }
        if kind == 2 {
            ordinary.push(24);
            fatal.extend([3, 10]);
        }
        if kind == 3 {
            ordinary.push(17);
        }
        if kind == 4 || kind == 7 {
            ordinary.push(20);
        }
        for selected in ordinary {
            let output = Command::new(&executable)
                .arg(selected.to_string())
                .output()
                .unwrap();
            let stdout = String::from_utf8(output.stdout).unwrap();
            assert!(
                output.status.success(),
                "kind {kind}, mode {selected}, O{optimization}: {stdout}\n{}",
                String::from_utf8_lossy(&output.stderr)
            );
            if selected == 18 {
                assert_eq!(stdout, "event:init\nconstructor-rejected\n");
                continue;
            }
            let publication = stdout
                .find(if selected == 21 {
                    "unwind-settled"
                } else {
                    "returned:"
                })
                .unwrap();
            let closed = stdout.find("event:close").unwrap();
            assert_eq!(stdout.matches("call-completed").count(), 1, "{stdout}");
            let completed = stdout.find("call-completed").unwrap();
            assert!(closed < completed && completed < publication, "{stdout}");
            let owned_first =
                (kind <= 3 || kind == 7) && !matches!(selected, 12 | 13 | 14 | 23 | 24);
            assert_eq!(
                stdout[..closed].matches("event:drop").count(),
                usize::from(owned_first),
                "{stdout}"
            );
            let expected_closes = if selected == 19 { 1 } else { 2 };
            assert_eq!(
                stdout.matches("event:close").count(),
                expected_closes,
                "{stdout}"
            );
            assert_eq!(stdout.matches("event:init").count(), 2, "{stdout}");
            if selected == 21 || matches!(selected, 4 | 5 | 6 | 11 | 20) && (kind <= 3 || kind == 7)
            {
                assert!(stdout.find("event:drop").unwrap() < closed, "{stdout}");
            }
        }
        for selected in fatal {
            let output = Command::new(&executable)
                .arg(selected.to_string())
                .output()
                .unwrap();
            let stdout = String::from_utf8(output.stdout).unwrap();
            assert_eq!(
                fatal_oracle::check(selected, output.status.success(), &stdout),
                Ok(()),
                "kind {kind}, mode {selected}, O{optimization}: {stdout}"
            );
        }
        if kind == 0 {
            fatal_oracle::calibrate(&root, lib, &instrumented, &source, optimization);
        }
    }
}

fn entry(kind: u32, method: &str) -> String {
    let symbol = format!("spx_owned_data_call_{method}_v1");
    if kind <= 3 {
        format!("#[no_mangle]unsafe extern \"C\" fn {symbol}(context:*mut State,tag:*mut u32,handle:*mut u64,error:*mut i64)->u32{{unsafe{{owned_call(context,tag,handle,error)}}}}")
    } else if kind == 7 {
        format!("#[no_mangle]unsafe extern \"C\" fn {symbol}(context:*mut State,record:*mut u64)->u32{{unsafe{{flat_call(context,record)}}}}")
    } else {
        let ty = if kind == 4 {
            "u8"
        } else if kind == 5 {
            "i64"
        } else {
            "u64"
        };
        format!("#[no_mangle]unsafe extern \"C\" fn {symbol}(context:*mut State,value:*mut {ty})->u32{{event(\"call\");if let Some(status)=unsafe{{returned_status(context)}}{{return status}}unsafe{{(*context).live=if mode()==1{{1}}else{{0}};*value=if mode()==20{{2}}else{{1}};}}0}}")
    }
}

fn value_assertions(kind: u32) -> &'static str {
    match kind {
        0 => "if let Ok(bytes)=&result{assert_eq!(bytes.len(),if selected==15{0}else if selected==16{65536}else{3});assert!(bytes.iter().all(|b|*b==b'a'));}",
        1 => "if selected==23{assert_eq!(result,Ok(None));}else if !error{let bytes=match &result{Ok(Some(bytes))=>bytes,_=>panic!(\"expected active Some result\")};assert_eq!(bytes.len(),if selected==15{0}else if selected==16{65536}else{3});assert!(bytes.iter().all(|b|*b==b'a'));}",
        2 => "if selected==24{assert_eq!(result,Ok(Err(42)));}else if !error{let bytes=match &result{Ok(Ok(bytes))=>bytes,_=>panic!(\"expected active Ok result\")};assert_eq!(bytes.len(),if selected==15{0}else if selected==16{65536}else{3});assert!(bytes.iter().all(|b|*b==b'a'));}",
        3 => "if let Ok(text)=&result{assert_eq!(text.len(),if selected==15{0}else if selected==16{65536}else{3});assert!(text.bytes().all(|b|b==b'a'));}",
        4 => "if !error{assert_eq!(result,Ok(true));}",
        5 | 6 => "if !error{assert_eq!(result,Ok(1));}",
        7 => "if let Ok(record)=&result{assert_eq!(record.spx_field_id_6669656c642e636f756e74,42);assert!(record.spx_field_id_6669656c642e666c6167);let bytes=&record.spx_field_id_6669656c642e6279746573;assert_eq!(bytes.len(),if selected==15{0}else if selected==16{65536}else{3});assert!(bytes.iter().all(|b|*b==b'a'));}",
        _ => "",
    }
}
