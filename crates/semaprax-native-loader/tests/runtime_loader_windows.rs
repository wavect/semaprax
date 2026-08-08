#![cfg(target_os = "windows")]

use semaprax_native_loader::{open_admitted_callable_exact, OpenError};
use std::ffi::OsString;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};

const CALLABLE_EXPECTED: &[u8] = &[
    b'S', b'P', b'X', b'N', b'A', b'B', b'I', b'2', 2, 0, 0, 0, 20, 0, 0, 0, 20, 0, 0, 0,
];
const GETTER_SYMBOL: &[u8] = b"spx_descriptor_callable";
const CALLABLE_SYMBOL: &[u8] = b"spx_callable_echo";
const GOOD_DEPENDENCY_VALUE: u32 = 0x5350_5847;
static NEXT_FIXTURE: AtomicU64 = AtomicU64::new(1);

struct CollisionFixture {
    directory: PathBuf,
    root_library: PathBuf,
    malicious_directory: PathBuf,
}

impl CollisionFixture {
    fn build(keep_root_dependency: bool) -> Self {
        let sequence = NEXT_FIXTURE.fetch_add(1, Ordering::Relaxed);
        let directory = std::env::temp_dir().join(format!(
            "semaprax-native-loader-windows-collision-{}-{sequence}",
            std::process::id()
        ));
        fs::create_dir(&directory).expect("create Windows collision fixture directory");
        let directory =
            fs::canonicalize(directory).expect("canonical Windows collision fixture directory");
        let root_directory = directory.join("root");
        let malicious_directory = directory.join("malicious-cwd-and-path");
        fs::create_dir(&root_directory).expect("create root image directory");
        fs::create_dir(&malicious_directory).expect("create malicious search directory");

        let dependency_name = format!("spx-collision-dependency-{sequence}.dll");
        let import_library_name = format!("spx-collision-dependency-{sequence}.lib");
        let root_name = format!("spx-collision-root-{sequence}.dll");
        let dependency_source_name = "dependency.c";
        let root_source_name = "root.c";

        fs::write(
            root_directory.join(dependency_source_name),
            dependency_source(GOOD_DEPENDENCY_VALUE),
        )
        .expect("write trusted dependency source");
        run_compiler(
            &root_directory,
            &[
                "-shared".to_owned(),
                "-O2".to_owned(),
                "-std=c11".to_owned(),
                "-Wall".to_owned(),
                "-Wextra".to_owned(),
                "-Werror".to_owned(),
                dependency_source_name.to_owned(),
                "-o".to_owned(),
                dependency_name.clone(),
                format!("-Wl,/implib:{import_library_name}"),
            ],
            "trusted dependency",
        );

        fs::write(root_directory.join(root_source_name), root_source())
            .expect("write root provider source");
        run_compiler(
            &root_directory,
            &[
                "-shared".to_owned(),
                "-O2".to_owned(),
                "-std=c11".to_owned(),
                "-Wall".to_owned(),
                "-Wextra".to_owned(),
                "-Werror".to_owned(),
                root_source_name.to_owned(),
                import_library_name,
                "-o".to_owned(),
                root_name.clone(),
            ],
            "root callable provider",
        );

        fs::write(
            malicious_directory.join(dependency_source_name),
            dependency_source(GOOD_DEPENDENCY_VALUE ^ u32::MAX),
        )
        .expect("write malicious dependency source");
        run_compiler(
            &malicious_directory,
            &[
                "-shared".to_owned(),
                "-O2".to_owned(),
                "-std=c11".to_owned(),
                "-Wall".to_owned(),
                "-Wextra".to_owned(),
                "-Werror".to_owned(),
                dependency_source_name.to_owned(),
                "-o".to_owned(),
                dependency_name.clone(),
            ],
            "malicious dependency",
        );

        if !keep_root_dependency {
            fs::remove_file(root_directory.join(dependency_name))
                .expect("remove trusted dependency for fail-closed case");
        }

        let root_library = fs::canonicalize(root_directory.join(root_name))
            .expect("canonical root callable provider");
        let malicious_directory =
            fs::canonicalize(malicious_directory).expect("canonical malicious search directory");
        Self {
            directory,
            root_library,
            malicious_directory,
        }
    }
}

impl Drop for CollisionFixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.directory);
    }
}

struct ProcessSearchGuard {
    original_directory: PathBuf,
    original_path: Option<OsString>,
}

impl ProcessSearchGuard {
    fn enter(malicious_directory: &Path) -> Self {
        let original_directory =
            std::env::current_dir().expect("read original process current directory");
        let original_path = std::env::var_os("PATH");
        let mut search_paths = vec![malicious_directory.to_path_buf()];
        if let Some(path) = &original_path {
            search_paths.extend(std::env::split_paths(path));
        }
        let hostile_path =
            std::env::join_paths(search_paths).expect("construct hostile legacy PATH fixture");
        std::env::set_current_dir(malicious_directory)
            .expect("select malicious process current directory");
        std::env::set_var("PATH", hostile_path);
        Self {
            original_directory,
            original_path,
        }
    }
}

impl Drop for ProcessSearchGuard {
    fn drop(&mut self) {
        let _ = std::env::set_current_dir(&self.original_directory);
        if let Some(path) = &self.original_path {
            std::env::set_var("PATH", path);
        } else {
            std::env::remove_var("PATH");
        }
    }
}

fn run_compiler(directory: &Path, arguments: &[String], context: &str) {
    let compiler = std::env::var_os("CC").unwrap_or_else(|| "clang".into());
    let output = Command::new(compiler)
        .current_dir(directory)
        .args(arguments)
        .output()
        .unwrap_or_else(|error| panic!("start compiler for {context}: {error}"));
    assert!(
        output.status.success(),
        "{context} compiler failed:\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

fn dependency_source(value: u32) -> String {
    format!(
        r#"#include <stdint.h>
#define SPX_EXPORT __declspec(dllexport)
#define SPX_CALL __cdecl

SPX_EXPORT uint32_t SPX_CALL spx_collision_value(void) {{
  return UINT32_C({value});
}}
"#
    )
}

fn root_source() -> &'static str {
    r#"#include <stdint.h>
#include <string.h>
#define SPX_EXPORT __declspec(dllexport)
#define SPX_IMPORT __declspec(dllimport)
#define SPX_CALL __cdecl

SPX_IMPORT uint32_t SPX_CALL spx_collision_value(void);

static const uint8_t good_descriptor[] = {
  0x53, 0x50, 0x58, 0x4e, 0x41, 0x42, 0x49, 0x32,
  0x02, 0x00, 0x00, 0x00, 0x14, 0x00, 0x00, 0x00,
  0x14, 0x00, 0x00, 0x00
};
static const uint8_t wrong_descriptor[] = {
  0x00, 0x50, 0x58, 0x4e, 0x41, 0x42, 0x49, 0x32,
  0x02, 0x00, 0x00, 0x00, 0x14, 0x00, 0x00, 0x00,
  0x14, 0x00, 0x00, 0x00
};

SPX_EXPORT const uint8_t *SPX_CALL spx_descriptor_callable(void) {
  return spx_collision_value() == UINT32_C(0x53505847)
      ? good_descriptor
      : wrong_descriptor;
}

SPX_EXPORT uint32_t SPX_CALL spx_callable_echo(
    const uint8_t *request,
    uint32_t request_len,
    uint8_t *response,
    uint32_t response_capacity
) {
  if (spx_collision_value() != UINT32_C(0x53505847)) return UINT32_C(91);
  if (request == NULL || response == NULL) return UINT32_C(92);
  if (request_len > response_capacity) return UINT32_C(93);
  memcpy(response, request, request_len);
  return UINT32_C(0);
}
"#
}

fn require_callable_error(
    result: Result<semaprax_native_loader::NativeCallableModuleLease, OpenError>,
) -> OpenError {
    match result {
        Ok(_) => panic!("Windows dependency search unexpectedly admitted the root image"),
        Err(error) => error,
    }
}

#[test]
fn windows_callable_dependency_search_excludes_cwd_and_legacy_path() {
    let positive = CollisionFixture::build(true);
    {
        let _search = ProcessSearchGuard::enter(&positive.malicious_directory);
        // SAFETY: The root image and its sibling dependency were generated in
        // this private fixture. Both expose the exact declared C ABI, immutable
        // descriptor storage, and one synchronous bounded no-escape callable.
        let lease = unsafe {
            open_admitted_callable_exact(
                &positive.root_library,
                GETTER_SYMBOL,
                CALLABLE_SYMBOL,
                CALLABLE_EXPECTED,
            )
        }
        .expect("root-directory dependency must beat malicious CWD and legacy PATH entries");
        assert_eq!(lease.canonical_path(), positive.root_library);
        assert_eq!(lease.descriptor_len(), CALLABLE_EXPECTED.len());
        let request = CALLABLE_EXPECTED.to_vec();
        let mut call = lease
            .prepare_call(request.clone(), request.len())
            .expect("prepare exact bounded collision call");
        assert_eq!(lease.invoke(&mut call), Ok(0));
        assert_eq!(call.response_storage(), request);
    }

    let negative = CollisionFixture::build(false);
    {
        let _search = ProcessSearchGuard::enter(&negative.malicious_directory);
        // SAFETY: The root image remains trusted, while its deliberately absent
        // sibling dependency must make platform loading fail before any getter
        // or callable executes. The same-name CWD/PATH image is not admitted.
        let error = require_callable_error(unsafe {
            open_admitted_callable_exact(
                &negative.root_library,
                GETTER_SYMBOL,
                CALLABLE_SYMBOL,
                CALLABLE_EXPECTED,
            )
        });
        assert!(
            matches!(error, OpenError::LibraryOpen(_)),
            "missing sibling dependency must fail at library open without CWD/PATH fallback; got {error}"
        );
    }
}
