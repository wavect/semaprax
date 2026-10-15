//! Real standalone v8/direct-HIR v9 provider evidence, not Project activation.
//! The allocator observes only the generated C translation unit; neither a
//! successful context close nor safe Rust values establish physical freeing.
use std::fs;
use std::path::PathBuf;
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};

#[path = "../native_owned_data_string_settlement_v1/subject.rs"]
mod subject;

static SERIAL: AtomicU64 = AtomicU64::new(0);

fn run(sanitized: bool) {
    let compiler = if sanitized {
        let configured = PathBuf::from(
            std::env::var_os("SEMAPRAX_STRING_SANITIZER_CLANG")
                .expect("selected sanitizer gate requires SEMAPRAX_STRING_SANITIZER_CLANG"),
        );
        assert!(configured.is_absolute() && configured.is_file());
        configured
    } else {
        std::env::var_os("CLANG").map_or_else(|| PathBuf::from("clang"), PathBuf::from)
    };
    let root = std::env::temp_dir().join(format!(
        "semaprax-owned-data-strings-{}-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos(),
        SERIAL.fetch_add(1, Ordering::Relaxed)
    ));
    fs::create_dir(&root).unwrap();
    let mut permitted = Vec::new();
    for flat in [false, true] {
        let artifact = subject::artifact(flat);
        let descriptor: serde_json::Value = serde_json::from_slice(&artifact.descriptor).unwrap();
        assert_eq!(
            descriptor["exports"].as_array().unwrap().len(),
            artifact.selected.len()
        );
        assert!(artifact.selected.windows(2).all(|pair| pair[0] < pair[1]));
        assert_eq!(artifact.digest.len(), "sha256:".len() + 64);
        let label = if flat { "v9" } else { "v8" };
        let source_name = format!("probe-{label}.c");
        let source = root.join(&source_name);
        permitted.push(source_name);
        fs::write(
            &source,
            format!(
                "{}\n{}\n{}\n#define FIXTURE_FLAT {}\n{}",
                include_str!("../support/native_fixture_stdio.c"),
                include_str!("../native_owned_utf8_settlement_v1/allocations.c"),
                artifact.provider,
                u8::from(flat),
                include_str!("../native_owned_data_string_settlement_v1/probe.c"),
            ),
        )
        .unwrap();
        for optimization in ["-O0", "-O2"] {
            let name = format!(
                "probe-{label}{optimization}{}",
                std::env::consts::EXE_SUFFIX
            );
            let executable = root.join(&name);
            permitted.push(name);
            if cfg!(windows) {
                for extension in ["lib", "exp", "pdb", "ilk"] {
                    permitted.push(format!("probe-{label}{optimization}.{extension}"));
                }
            }
            let mut compile = Command::new(&compiler);
            compile.current_dir(&root).args([
                "-std=c11",
                optimization,
                "-Wall",
                "-Wextra",
                "-Werror",
            ]);
            if sanitized {
                compile.args([
                    "-fsanitize=address,undefined",
                    "-fno-sanitize-recover=all",
                    "-fno-omit-frame-pointer",
                ]);
            }
            let built = compile
                .arg(&source)
                .arg("-o")
                .arg(&executable)
                .output()
                .expect("Clang is required for native owned-data String evidence");
            assert!(
                built.status.success(),
                "{}: {}",
                root.display(),
                String::from_utf8_lossy(&built.stderr)
            );
            let mut execute = Command::new(&executable);
            execute.current_dir(&root);
            if sanitized {
                execute
                    .env("ASAN_OPTIONS", "halt_on_error=1")
                    .env("UBSAN_OPTIONS", "halt_on_error=1:print_stacktrace=1");
            }
            let result = execute.output().unwrap();
            assert!(
                result.status.success(),
                "{}: stdout={} stderr={}",
                root.display(),
                String::from_utf8_lossy(&result.stdout),
                String::from_utf8_lossy(&result.stderr)
            );
            assert_eq!(result.stdout, b"standalone-sdk-strings-settled\n");
            assert!(result.stderr.is_empty());
        }
    }
    // Failed fixtures remain for diagnosis. Validate the entire bounded, flat
    // successful inventory before removing any exact regular file.
    let entries = fs::read_dir(&root)
        .unwrap()
        .map(|entry| entry.unwrap())
        .collect::<Vec<_>>();
    assert!(entries.len() <= permitted.len());
    for entry in &entries {
        assert!(permitted.contains(&entry.file_name().into_string().unwrap()));
        let metadata = fs::symlink_metadata(entry.path()).unwrap();
        assert!(metadata.is_file() && !metadata.file_type().is_symlink());
        #[cfg(windows)]
        {
            use std::os::windows::fs::MetadataExt;
            assert_eq!(metadata.file_attributes() & 0x400, 0);
        }
    }
    for entry in entries {
        fs::remove_file(entry.path()).unwrap();
    }
    fs::remove_dir(root).unwrap();
}

#[test]
fn standalone_owned_data_strings_settle_at_o0_and_o2() {
    run(false);
}

#[test]
#[ignore = "requires explicitly provisioned Clang ASan/UBSan runtime"]
fn provisioned_owned_data_strings_asan_ubsan() {
    run(true);
}

#[test]
fn standalone_descriptor_does_not_widen_project_wasm_string_admission() {
    use semaprax::project::{self, PublicApiSubject};
    for flat in [false, true] {
        let source = if flat {
            r#"module guard.flat;
@id("record") record Payload { @id("field") bytes: Bytes, }
@id("call") fn value(input: borrow Slice<u8>) -> Payload {
    let kept = "internal"; Payload { bytes: bytes_copy(input) }
}
@id("main") fn main() -> i64 { 0 }
"#
        } else {
            r#"module guard.bytes;
@id("call") fn value(input: borrow Slice<u8>) -> Bytes {
    let kept = "internal"; bytes_copy(input)
}
@id("main") fn main() -> i64 { 0 }
"#
        };
        let program =
            semaprax::hir::resolve(&semaprax::check(source, "guard.spx").unwrap()).unwrap();
        let selected = ["call".to_owned()];
        let fact = "sha256:1111111111111111111111111111111111111111111111111111111111111111";
        let subject = PublicApiSubject {
            project_schema: if flat {
                project::FLAT_OWNED_RECORD_PROJECT_SCHEMA
            } else {
                project::PUBLIC_OWNED_DATA_PROJECT_SCHEMA
            },
            project_revision: fact,
            workspace_revision: fact,
            project_graph_digest: fact,
        };
        let error = if flat {
            let descriptor =
                project::derive_flat_owned_record_api_descriptor(&program, &selected, subject)
                    .unwrap();
            semaprax::wasm::emit_resolved_module_with_flat_owned_record_exports(
                &program,
                &descriptor,
            )
            .unwrap_err()
        } else {
            let descriptor =
                project::derive_public_api_descriptor(&program, &selected, subject).unwrap();
            semaprax::wasm::emit_resolved_module_with_owned_data_exports(&program, &descriptor)
                .unwrap_err()
        };
        assert_eq!(error.code, "SPX-W110");
        assert_eq!(
            error.message,
            "owned string literal reached a non-v10 WebAssembly profile"
        );
    }
}
