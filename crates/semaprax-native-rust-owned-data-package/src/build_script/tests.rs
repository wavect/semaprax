//! Actual generated scripts, not a handwritten substitute. Runtime cases are
//! authored but unrun: two current-host compilations, plus five-target static
//! preservation checks. Standalone script compilation only: no SDK archive
//! linking, provider execution, or Cargo consumer invocation.
use crate::{descriptor, flat_descriptor, flat_render, render, HostTarget, PackageMode};
use std::ffi::OsStr;
use std::fs::{self, OpenOptions};
use std::io::Write as _;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};

static SERIAL: AtomicU64 = AtomicU64::new(0);
const TARGETS: [HostTarget; 5] = [
    HostTarget::X86_64LinuxGnu,
    HostTarget::Aarch64LinuxGnu,
    HostTarget::X86_64Darwin,
    HostTarget::Aarch64Darwin,
    HostTarget::X86_64WindowsMsvc,
];
const GUARD: &str = "let root=std::env::var_os(\"CARGO_MANIFEST_DIR\").expect(\"Cargo must set CARGO_MANIFEST_DIR\");let native=std::path::PathBuf::from(root);let native=native.to_str().filter(|path|!path.contains(['\\r','\\n'])).expect(\"generated SDK package path must be Unicode without CR/LF\");";

fn owned_descriptor(utf8: bool) -> descriptor::Descriptor {
    let (schema, project, result) = if utf8 {
        (
            crate::PUBLIC_OWNED_UTF8_API_SCHEMA,
            crate::PUBLIC_OWNED_UTF8_PROJECT_SCHEMA,
            "owned-utf8",
        )
    } else {
        (
            crate::PUBLIC_OWNED_DATA_API_SCHEMA,
            crate::PUBLIC_OWNED_DATA_PROJECT_SCHEMA,
            "owned-bytes",
        )
    };
    let bytes = format!("{{\"schema\":\"{schema}\",\"project_schema\":\"{project}\",\"project_revision\":\"sha256:{}\",\"workspace_revision\":\"sha256:{}\",\"project_graph_digest\":\"sha256:{}\",\"exports\":[{{\"stable_id\":\"fixture.value\",\"typescript_name\":\"fixture.value\",\"rust_method_name\":\"spx_fixture_dot_value\",\"parameters\":[],\"result\":\"{result}\"}}],\"limits\":{{\"max_exports\":32,\"max_parameters\":8,\"max_closure_functions\":256,\"max_borrowed_input_bytes\":65536,\"max_owned_output_bytes\":65536,\"max_descriptor_bytes\":1048576}}}}\n", "1".repeat(64), "2".repeat(64), "3".repeat(64)).into_bytes();
    let digest = crate::descriptor_digest_for_schema(schema, &bytes).unwrap();
    descriptor::replay(&bytes, &digest, &["fixture.value".to_owned()]).unwrap()
}

fn scripts(target: HostTarget) -> [String; 2] {
    let owned = owned_descriptor(false);
    let v8 = render::render_sources(&owned, target, PackageMode::ProjectV8).build_rs;
    assert_eq!(
        render::render_sources(&owned, target, PackageMode::StandaloneEvidence).build_rs,
        v8
    );
    assert_eq!(
        render::render_sources(
            &owned_descriptor(true),
            target,
            PackageMode::ProjectV10OwnedUtf8
        )
        .build_rs,
        v8
    );
    let bytes = include_bytes!("../../../../tests/fixtures/flat_descriptor_retained_names.json");
    let flat = flat_descriptor::replay(
        bytes,
        &crate::flat_descriptor_digest(bytes),
        &["api.value".to_owned()],
    )
    .unwrap();
    [v8, flat_render::render_sources(&flat, target).build_rs]
}

fn mismatch(flat: bool) -> &'static str {
    if flat {
        "generated SEMAPRAX flat-record SDK target mismatch"
    } else {
        "generated SEMAPRAX owned-data SDK target mismatch"
    }
}

#[test]
fn five_target_scripts_change_only_path_validation_and_interpolation() {
    for target in TARGETS {
        for (index, script) in scripts(target).into_iter().enumerate() {
            assert_eq!(script.matches(GUARD).count(), 1);
            assert_eq!(script.matches("println!").count(), 3);
            assert!(script.find(GUARD).unwrap() < script.find("println!").unwrap());
            assert!(script.find(mismatch(index == 1)).unwrap() < script.find(GUARD).unwrap());
            let restored = script.replace(GUARD, "").replace(
                "println!(\"cargo:rustc-link-search=native={native}\");",
                "println!(\"cargo:rustc-link-search=native={}\",std::env::var(\"CARGO_MANIFEST_DIR\").unwrap());",
            );
            // Full historical template, independently retained as the exact
            // preservation oracle; no digest is recomputed or weakened.
            let archive = target.archive_name();
            let triple = target.triple();
            let message = mismatch(index == 1);
            let historical = format!("#![forbid(unsafe_code)]\nfn main(){{if std::env::var(\"TARGET\").unwrap_or_default()!={triple:?}{{panic!({message:?})}}println!(\"cargo:rerun-if-changed={archive}\");println!(\"cargo:rustc-link-search=native={{}}\",std::env::var(\"CARGO_MANIFEST_DIR\").unwrap());println!(\"cargo:rustc-link-lib=static=semaprax_native_rust_owned_data_sdk\");}}\n");
            assert_eq!(restored, historical);
        }
    }
}

fn write_new(path: &Path, bytes: &[u8]) {
    OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
        .unwrap()
        .write_all(bytes)
        .unwrap();
}

fn invoke(
    executable: &Path,
    root: &Path,
    target: Option<&str>,
    manifest: Option<&OsStr>,
) -> std::process::Output {
    let mut command = Command::new(executable);
    command
        .current_dir(root)
        .env("RUST_BACKTRACE", "0")
        .env_remove("TARGET")
        .env_remove("CARGO_MANIFEST_DIR");
    if let Some(target) = target {
        command.env("TARGET", target);
    }
    if let Some(manifest) = manifest {
        command.env("CARGO_MANIFEST_DIR", manifest);
    }
    command.output().unwrap()
}

fn rejected(
    executable: &Path,
    root: &Path,
    target: Option<&str>,
    manifest: Option<&OsStr>,
    message: &str,
) {
    let output = invoke(executable, root, target, manifest);
    assert!(!output.status.success());
    assert!(
        output.stdout.is_empty(),
        "rejection emitted Cargo instructions: {:?}",
        output.stdout
    );
    assert!(String::from_utf8_lossy(&output.stderr).contains(message));
}

#[test]
fn generated_host_scripts_reject_directive_injection_before_any_stdout() {
    let target =
        HostTarget::current().expect("generated script execution requires a supported native host");
    let root = std::env::temp_dir().join(format!(
        "semaprax-owned-build-script-{}-{}-{}",
        std::process::id(),
        SERIAL.fetch_add(1, Ordering::Relaxed),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    fs::create_dir(&root).unwrap();
    let root = root.canonicalize().unwrap();
    // Keep the bounded fixture, including any compiler-owned platform sidecars.
    // No guessed sidecar inventory grants recursive cleanup authority.
    eprintln!(
        "retained generated build-script fixture: {}",
        root.display()
    );
    for (index, script) in scripts(target).into_iter().enumerate() {
        let source = root.join(format!("build_{index}.rs"));
        let executable: PathBuf =
            root.join(format!("build_{index}{}", std::env::consts::EXE_SUFFIX));
        write_new(&source, script.as_bytes());
        let output = Command::new("rustc")
            .args(["--edition=2021", "--crate-name", "owned_sdk_build_script"])
            .arg(&source)
            .arg("-o")
            .arg(&executable)
            .current_dir(&root)
            .output()
            .expect("Rust compiler is required for generated build-script regression");
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        for path in [
            "package with spaces λ",
            "package-cargo:rustc-cfg=not_a_directive",
        ] {
            let manifest = root.join(path);
            let output = invoke(
                &executable,
                &root,
                Some(target.triple()),
                Some(manifest.as_os_str()),
            );
            assert!(
                output.status.success(),
                "{}",
                String::from_utf8_lossy(&output.stderr)
            );
            let expected = format!("cargo:rerun-if-changed={}\ncargo:rustc-link-search=native={}\ncargo:rustc-link-lib=static=semaprax_native_rust_owned_data_sdk\n", target.archive_name(), manifest.to_str().unwrap());
            assert_eq!(output.stdout, expected.as_bytes());
        }
        for path in [
            "package\rpath",
            "package\npath",
            "sdk\ncargo:rustc-cfg=semaprax_injected",
            "sdk\r\ncargo:rustc-env=INJECTED=yes",
        ] {
            rejected(
                &executable,
                &root,
                Some(target.triple()),
                Some(OsStr::new(path)),
                "generated SDK package path must be Unicode without CR/LF",
            );
        }
        rejected(
            &executable,
            &root,
            Some(target.triple()),
            None,
            "Cargo must set CARGO_MANIFEST_DIR",
        );
        for wrong_target in [None, Some("not-the-generated-target")] {
            rejected(
                &executable,
                &root,
                wrong_target,
                Some(OsStr::new("path\ncargo:rustc-cfg=injected")),
                mismatch(index == 1),
            );
            rejected(&executable, &root, wrong_target, None, mismatch(index == 1));
        }
        #[cfg(unix)]
        {
            use std::ffi::OsString;
            use std::os::unix::ffi::OsStringExt as _;
            let path = OsString::from_vec(b"non-unicode-\xff".to_vec());
            rejected(
                &executable,
                &root,
                Some(target.triple()),
                Some(&path),
                "generated SDK package path must be Unicode without CR/LF",
            );
        }
        #[cfg(windows)]
        {
            use std::ffi::OsString;
            use std::os::windows::ffi::OsStringExt as _;
            let path = OsString::from_wide(&[0xd800]);
            rejected(
                &executable,
                &root,
                Some(target.triple()),
                Some(&path),
                "generated SDK package path must be Unicode without CR/LF",
            );
        }
        assert_eq!(fs::read(&source).unwrap(), script.as_bytes());
    }
}
