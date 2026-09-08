//! Typed standard filesystem transitions through the authenticated Project route.
use super::{filesystem, project};
use semaprax::filesystem_provider::FixtureFileProvider;
use semaprax::interpreter::CommandEvaluationOutcome;

fn package(command: &str) -> std::path::PathBuf {
    let path = filesystem::package("fs-v2", command, false);
    let text = std::fs::read_to_string(&path)
        .unwrap()
        .replace("filesystem-io.v1", "filesystem-io.v2");
    std::fs::write(&path, text).unwrap();
    path
}

#[test]
fn filesystem_v2_typed_fixture_transitions_and_raw_names() {
    for command in ["std.fs.tests.directory", "std.fs.tests.raw-names"] {
        let manifest = package(command);
        let seed = if command.ends_with("raw-names") {
            vec![(b"a".to_vec(), vec![]), (vec![255], vec![0, 255])]
        } else {
            vec![]
        };
        let mut provider = FixtureFileProvider::new(seed, true).unwrap();
        project::with_authenticated_project(&manifest, |snapshot| {
            for _ in 0..2 {
                let run = snapshot.execute_filesystem_command(&mut provider, 1_000_000)?;
                assert!(
                    matches!(run.outcome, CommandEvaluationOutcome::ReturnedBool(true)),
                    "{run:?}"
                );
            }
            Ok(())
        })
        .unwrap();
        assert_eq!(provider.settlements(), 2);
        if command.ends_with("directory") {
            assert!(provider.files().is_empty());
        }
        std::fs::remove_dir_all(manifest.parent().unwrap()).unwrap();
    }
}

#[cfg(unix)]
#[test]
fn filesystem_v2_typed_physical_directory_settles() {
    use semaprax::filesystem_provider::{FileAccess, ScopedFileProvider};
    let manifest = package("std.fs.tests.directory");
    let directory = manifest.parent().unwrap().join("authority");
    std::fs::create_dir(&directory).unwrap();
    let mut provider = ScopedFileProvider::open(&directory, FileAccess::ReadWrite).unwrap();
    project::with_authenticated_project(&manifest, |snapshot| {
        let run = snapshot.execute_filesystem_command(&mut provider, 1_000_000)?;
        assert!(
            matches!(run.outcome, CommandEvaluationOutcome::ReturnedBool(true)),
            "{run:?}"
        );
        Ok(())
    })
    .unwrap();
    assert_eq!(std::fs::read_dir(&directory).unwrap().count(), 0);
    drop(provider);
    std::fs::remove_dir_all(manifest.parent().unwrap()).unwrap();
}

const C_PROVIDER: &str = r#"
struct fs_fixture { uint64_t calls, settled; int directory, exists; uint8_t byte; };
static uint32_t metadata(void *ctx, spx_slice_u8_v1 path, uint64_t length, uint64_t *out) {
 struct fs_fixture *s=ctx; ++s->calls;
 if(RAW_NAMES) { if(length!=0)return 5; *out=2; return 0; }
 if(length!=3||memcmp(path.ptr,"d/a",3)||!s->exists)return 5; *out=5; return 0;
}
static uint32_t listing(void *ctx, spx_slice_u8_v1 path, uint64_t length, uint8_t *data, uint64_t capacity, uint64_t *out) {
 struct fs_fixture *s=ctx; ++s->calls;
 if(RAW_NAMES) { if(length||capacity<4)return 5; data[0]=97;data[1]=0;data[2]=255;data[3]=0;*out=4;return 0; }
 if(length!=1||path.ptr[0]!=100||!s->directory||!s->exists||s->byte!=255||capacity<2)return 5;
 data[0]=97;data[1]=0;*out=2;return 0;
}
static uint32_t mkdir_provider(void *ctx, spx_slice_u8_v1 path, uint64_t length, uint64_t *out) {
 struct fs_fixture *s=ctx;++s->calls;if(length!=1||path.ptr[0]!=100||s->directory)return 5;s->directory=1;*out=0;return 0;
}
static uint32_t atomic_provider(void *ctx, spx_slice_u8_v1 path, uint64_t length, spx_slice_u8_v1 data, uint64_t count, uint64_t *out) {
 struct fs_fixture *s=ctx;++s->calls;if(length!=3||memcmp(path.ptr,"d/a",3)||count!=1||!s->directory)return 5;
 if(data.ptr[0]!=(s->exists?255:65))return 5;s->exists=1;s->byte=data.ptr[0];*out=1;return 0;
}
static uint32_t remove_provider(void *ctx, spx_slice_u8_v1 path, uint64_t length, uint64_t *out) {
 struct fs_fixture *s=ctx;++s->calls;
 if(length==3&&!memcmp(path.ptr,"d/a",3)&&s->exists)s->exists=0;
 else if(length==1&&path.ptr[0]==100&&s->directory&&!s->exists)s->directory=0;
 else return 5;*out=0;return 0;
}
static void settle(void *ctx) { ++((struct fs_fixture*)ctx)->settled; }
int main(void) {
 for(int i=0;i<2;++i) {
  struct fs_fixture state={0};struct spx_filesystem_callbacks_v2 callbacks={.context=&state,.stat=metadata,.list=listing,.create_dir=mkdir_provider,.remove=remove_provider,.write_atomic=atomic_provider,.settle=settle};struct spx_filesystem_command_result_v2 result;
  if(spx_run_filesystem_command_v2(&callbacks,&result)!=1||!result.semantic_success||!result.matched||state.settled!=1||state.calls!=(RAW_NAMES?2:7)||state.exists||state.directory)return 1;
 }
 return 0;
}
"#;

pub(super) fn run_conformance() {
    for (command, raw) in [
        ("std.fs.tests.directory", false),
        ("std.fs.tests.raw-names", true),
    ] {
        let manifest = package(command);
        project::with_authenticated_project(&manifest, |snapshot| {
            let seed = if raw {
                vec![(b"a".to_vec(), vec![]), (vec![255], vec![0, 255])]
            } else {
                vec![]
            };
            let mut provider = FixtureFileProvider::new(seed, true).unwrap();
            let run = snapshot.execute_filesystem_command(&mut provider, 1_000_000)?;
            assert!(
                matches!(run.outcome, CommandEvaluationOutcome::ReturnedBool(true)),
                "{run:?}"
            );
            assert_eq!(provider.settlements(), 1);
            let revision = snapshot.retain_revision();
            let source = format!(
                "{}\n#define RAW_NAMES {}\n{C_PROVIDER}",
                revision.filesystem_c_source()?,
                usize::from(raw)
            );
            for optimization in ["-O0", "-O2"] {
                super::compile_and_run_c(&source, manifest.parent().unwrap(), optimization, "");
            }
            let wasm_path = manifest.with_extension("wasm");
            let facade_path = manifest.with_extension("mjs");
            std::fs::write(&wasm_path, revision.filesystem_wasm_module()?).unwrap();
            std::fs::write(&facade_path, include_str!("filesystem_v2_facade.mjs")).unwrap();
            let symbol = format!(
                "spx_data_{}",
                command
                    .bytes()
                    .map(|byte| format!("{byte:02x}"))
                    .collect::<String>()
            );
            let result = std::process::Command::new("node")
                .arg(&facade_path)
                .arg(&wasm_path)
                .arg(symbol)
                .arg(if raw { "1" } else { "0" })
                .output()
                .unwrap();
            assert!(
                result.status.success(),
                "{}",
                String::from_utf8_lossy(&result.stderr)
            );
            assert_eq!(
                String::from_utf8(result.stdout).unwrap(),
                "validate 1\nrun 1 0 0 0\nrun 1 0 0 0\n"
            );
            Ok(())
        })
        .unwrap();
        std::fs::remove_dir_all(manifest.parent().unwrap()).unwrap();
    }
}

#[test]
fn filesystem_v2_standard_commands_execute_on_all_three_backends() {
    run_conformance();
}
