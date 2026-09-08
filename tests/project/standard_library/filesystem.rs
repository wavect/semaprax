//! Actual Project filesystem authority, typed dependencies and logical prefixes.
use super::{project, root, temporary};
use semaprax::filesystem_provider::{FileFailure, FixtureFileProvider};
use semaprax::interpreter::CommandEvaluationOutcome;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
static NEXT_PACKAGE: AtomicU64 = AtomicU64::new(0);

pub(super) fn package(label: &str, command: &str, bundled: bool) -> PathBuf {
    let directory = temporary(&format!(
        "{label}-{}",
        NEXT_PACKAGE.fetch_add(1, Ordering::Relaxed)
    ));
    std::fs::create_dir(directory.join("src")).unwrap();
    for file in ["examples.spx", "tests.spx"] {
        std::fs::copy(
            root().join("std/fs/src").join(file),
            directory.join("src").join(file),
        )
        .unwrap();
    }
    let mut manifest = std::fs::read_to_string(root().join("std/fs/semaprax.toml"))
        .unwrap()
        .replace("std.fs.examples.roundtrip", command)
        .replace("filesystem-io.v2", "filesystem-io.v1");
    if bundled {
        manifest = manifest
            .replace("name = \"std-fs\"", "name = \"std-fs-consumer\"")
            .replace(
                "\"src/examples.spx\", \"src/fs.spx\", \"src/tests.spx\"",
                "\"src/examples.spx\", \"src/tests.spx\"",
            )
            .replace(
                "std.io = \"^0.1.0\"\nstd.path.value = \"^0.1.0\"",
                "std.fs = \"^0.1.0\"",
            );
    } else {
        std::fs::copy(
            root().join("std/fs/src/fs.spx"),
            directory.join("src/fs.spx"),
        )
        .unwrap();
    }
    let path = directory.join("semaprax.toml");
    std::fs::write(&path, manifest).unwrap();
    path
}
#[test]
fn filesystem_project_typed_records_and_logical_prefixes() {
    for (label, command, name, bytes) in [
        (
            "fs-roundtrip",
            "std.fs.examples.roundtrip",
            b"o".as_slice(),
            vec![0, 255],
        ),
        ("fs-empty", "std.fs.tests.empty", b"e".as_slice(), vec![]),
        (
            "fs-prefix",
            "std.fs.tests.prefix",
            b"p".as_slice(),
            vec![65],
        ),
    ] {
        let manifest = package(label, command, false);
        let mut provider = FixtureFileProvider::new([], true).unwrap();
        project::with_authenticated_project(&manifest, |snapshot| {
            let result = snapshot.execute_filesystem_command(&mut provider, 1_000_000)?;
            assert!(
                matches!(result.outcome, CommandEvaluationOutcome::ReturnedBool(true)),
                "{result:?}"
            );
            let duplicate = snapshot.execute_filesystem_command(&mut provider, 1_000_000)?;
            let CommandEvaluationOutcome::LanguageFailure(status) = duplicate.outcome else {
                panic!("write-new must fail on existing file")
            };
            assert_eq!(status.code(), FileFailure::AlreadyExists.status_code());
            Ok(())
        })
        .unwrap();
        assert_eq!(provider.files().get(name), Some(&bytes));
        assert_eq!(provider.settlements(), 2);
        std::fs::remove_dir_all(manifest.parent().unwrap()).unwrap();
    }
}
#[test]
fn filesystem_bundled_consumer_expands_typed_dependencies() {
    let manifest = package("fs-bundled", "std.fs.examples.roundtrip", true);
    let mut provider = FixtureFileProvider::new([], true).unwrap();
    project::with_authenticated_project(&manifest, |snapshot| {
        let result = snapshot.execute_filesystem_command(&mut provider, 1_000_000)?;
        assert!(matches!(
            result.outcome,
            CommandEvaluationOutcome::ReturnedBool(true)
        ));
        Ok(())
    })
    .unwrap();
    assert_eq!(provider.files().get(b"o".as_slice()), Some(&vec![0, 255]));
    std::fs::remove_dir_all(manifest.parent().unwrap()).unwrap();
}
#[cfg(unix)]
#[test]
fn filesystem_project_scoped_unix_executes_actual_file_roundtrip() {
    use semaprax::filesystem_provider::{FileAccess, ScopedFileProvider};
    let manifest = package("fs-unix-project", "std.fs.examples.roundtrip", false);
    let storage = temporary("fs-unix-storage");
    let mut provider = ScopedFileProvider::open(&storage, FileAccess::ReadWrite).unwrap();
    project::with_authenticated_project(&manifest, |snapshot| {
        let result = snapshot.execute_filesystem_command(&mut provider, 1_000_000)?;
        assert!(
            matches!(result.outcome, CommandEvaluationOutcome::ReturnedBool(true)),
            "{result:?}"
        );
        Ok(())
    })
    .unwrap();
    assert_eq!(std::fs::read(storage.join("o")).unwrap(), [0, 255]);
    drop(provider);
    std::fs::remove_dir_all(storage).unwrap();
    std::fs::remove_dir_all(manifest.parent().unwrap()).unwrap();
}

const C_PROVIDER: &str = r#"
struct fixture { uint8_t bytes[4]; uint64_t length; uint64_t calls; uint64_t settles; bool exists; };
static uint32_t fixture_read(void *opaque, spx_slice_u8_v1 path, uint64_t length, uint8_t *out, uint64_t max, uint64_t *written) {
    struct fixture *f = opaque; f->calls++;
    if (length != UINT64_C(1) || path.ptr[0] != EXPECTED_PATH) return UINT32_C(1);
    if (!f->exists) return UINT32_C(2);
    if (f->length > max) return UINT32_C(4);
    for (uint64_t i=0;i<f->length;i++) out[i]=f->bytes[i];
    *written=f->length; return UINT32_C(0);
}
static uint32_t fixture_write(void *opaque, spx_slice_u8_v1 path, uint64_t length, spx_slice_u8_v1 data, uint64_t count, uint64_t *written) {
    struct fixture *f = opaque; f->calls++;
    if (length != UINT64_C(1) || path.ptr[0] != EXPECTED_PATH) return UINT32_C(1);
    if (f->exists) return UINT32_C(3);
    if (count != EXPECTED_LENGTH) return UINT32_C(5);
    for (uint64_t i=0;i<count;i++) f->bytes[i]=data.ptr[i];
    f->length=count; f->exists=true; *written=count; return UINT32_C(0);
}
static void fixture_settle(void *opaque) { ((struct fixture *)opaque)->settles++; }
int main(void) {
    for (unsigned iteration=0;iteration<3;iteration++) {
        struct fixture state={0};
        struct spx_filesystem_callbacks_v1 callbacks={.context=&state,.read=fixture_read,.write_new=fixture_write,.settle=fixture_settle};
        struct spx_filesystem_command_result_v1 result;
        if (spx_run_filesystem_command_v1(&callbacks,&result)!=1 || !result.semantic_success || !result.matched || state.settles!=UINT64_C(1) || state.calls!=EXPECTED_CALLS || !state.exists || state.length!=EXPECTED_LENGTH) return 1;
        if (EXPECTED_LENGTH==UINT64_C(2) && (state.bytes[0]!=0 || state.bytes[1]!=255)) return 2;
        if (EXPECTED_LENGTH==UINT64_C(1) && state.bytes[0]!=65) return 3;
    }
    return 0;
}
"#;

/// The standard-library catalogue runner calls the real effectful commands,
/// not the pure package main used only as a linking anchor.
pub(super) fn run_conformance() {
    for (label, command, path, length, calls) in [
        ("fs-all-roundtrip", "std.fs.examples.roundtrip", 111, 2, 2),
        ("fs-all-empty", "std.fs.tests.empty", 101, 0, 2),
        ("fs-all-prefix", "std.fs.tests.prefix", 112, 1, 1),
    ] {
        let manifest = package(label, command, false);
        project::with_authenticated_project(&manifest,|snapshot| {
            let mut provider=FixtureFileProvider::new([],true).unwrap();
            let result=snapshot.execute_filesystem_command(&mut provider,1_000_000)?;
            assert!(matches!(result.outcome,CommandEvaluationOutcome::ReturnedBool(true)),"{result:?}");
            let revision=snapshot.retain_revision();
            let generated=revision.filesystem_c_source()?;
            let source=format!("{generated}\n#define EXPECTED_PATH {path}\n#define EXPECTED_LENGTH UINT64_C({length})\n#define EXPECTED_CALLS UINT64_C({calls})\n{C_PROVIDER}");
            for optimization in ["-O0","-O2"] {
                super::compile_and_run_c(&source,manifest.parent().unwrap(),optimization,"");
            }
            let wasm=revision.filesystem_wasm_module()?;
            let wasm_path=manifest.with_extension("wasm");
            let facade_path=manifest.with_extension("mjs");
            std::fs::write(&wasm_path,wasm).unwrap();
            let facade=include_str!("../../useful_data/filesystem_ops_wasm_facade.mjs")
                .replace("const env={", "const env={\n spx_bytes_zeroed:count=>allocate(new Uint8Array(Number(count))),spx_bytes_set:(value,index,byte)=>{const data=bytes(value);if(Number(index)>=data.length)throw Error('set-bounds');data[Number(index)]=byte;return value},")
                .replace("files.clear()", &format!("if(files.size!==1||!files.has('{path:02x}')||files.get('{path:02x}').length!=={length})throw Error('file-result');if({length}===2&&(files.get('{path:02x}')[0]!==0||files.get('{path:02x}')[1]!==255))throw Error('content');if({length}===1&&files.get('{path:02x}')[0]!==65)throw Error('prefix');files.clear()"));
            std::fs::write(&facade_path,facade).unwrap();
            let symbol=format!("spx_data_{}",command.bytes().map(|byte|format!("{byte:02x}")).collect::<String>());
            let result=std::process::Command::new("node").arg(&facade_path).arg(&wasm_path).arg(symbol).output().unwrap();
            assert!(result.status.success(),"{}",String::from_utf8_lossy(&result.stderr));
            assert_eq!(String::from_utf8(result.stdout).unwrap(),"validate 1\nrun 1 0 0 0\nrun 1 0 0 0\n");
            Ok(())
        }).unwrap();
        std::fs::remove_dir_all(manifest.parent().unwrap()).unwrap();
    }
}

#[test]
fn filesystem_standard_commands_execute_on_all_three_backends() {
    run_conformance();
}
