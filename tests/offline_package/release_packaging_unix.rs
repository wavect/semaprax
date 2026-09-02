//! Packaging-mechanics fixtures with fake Rust tools, not release/build evidence.
//! Fixtures are retained for inspection; no compiler or release workflow is run.
#![cfg(unix)]

use std::fs;
use std::os::unix::fs::{symlink, PermissionsExt};
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::sync::atomic::{AtomicU64, Ordering};

const TARGET: &str = "x86_64-unknown-linux-gnu";
const COMMIT: &str = "64aec43b52277a53cb0f18d19fce9a37ca2dccaf";
const VERSION: &str = env!("CARGO_PKG_VERSION");
static SERIAL: AtomicU64 = AtomicU64::new(0);

struct Fixture {
    root: PathBuf,
    output: PathBuf,
}

fn executable(path: &Path, text: &str) {
    fs::write(path, text).unwrap();
    fs::set_permissions(path, fs::Permissions::from_mode(0o755)).unwrap();
}

impl Fixture {
    fn new() -> Self {
        Self::with_output(true)
    }

    fn with_output(create_output: bool) -> Self {
        let root = std::env::temp_dir().join(format!(
            "semaprax release mechanics {} {}",
            std::process::id(),
            SERIAL.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&root).unwrap();
        let root = root.canonicalize().unwrap();
        eprintln!("retained release mechanics fixture: {}", root.display());
        let output = root.join("output with spaces");
        if create_output {
            fs::create_dir(&output).unwrap();
        }
        fs::create_dir(root.join("tools")).unwrap();
        fs::write(
            root.join("Cargo.toml"),
            format!("version = \"{VERSION}\"\n"),
        )
        .unwrap();
        fs::write(root.join("LICENSE"), "fixture license\n").unwrap();
        fs::write(root.join("README.md"), "fixture readme\n").unwrap();
        fs::write(
            root.join("package-release.sh"),
            include_str!("../../scripts/package-release.sh"),
        )
        .unwrap();
        executable(
            &root.join("tools/rustc"),
            "#!/bin/sh\n[ \"$1\" = -vV ] || exit 9\nprintf '%s\\n' 'host: x86_64-unknown-linux-gnu'\nexit \"$FAKE_RUSTC_STATUS\"\n",
        );
        executable(
            &root.join("tools/cargo"),
            include_str!("../release_packaging_unix_v1/cargo.sh"),
        );
        executable(
            &root.join("cli"),
            include_str!("../release_packaging_unix_v1/cli.sh"),
        );
        fs::write(root.join("daemon"), "fresh daemon from selected build\n").unwrap();
        let stale = root.join(format!("target/{TARGET}/release"));
        fs::create_dir_all(&stale).unwrap();
        executable(
            &stale.join("semaprax-full"),
            &format!(
                "{}\n# stale CLI with matching public responses\n",
                include_str!("../release_packaging_unix_v1/cli.sh")
            ),
        );
        fs::write(stale.join("semapraxd"), "stale daemon must not package\n").unwrap();
        Self { root, output }
    }

    fn package_name(&self) -> String {
        format!("semaprax-v{VERSION}-{TARGET}")
    }

    fn command(&self) -> Command {
        let mut command = Command::new("sh");
        let mut paths = vec![self.root.join("tools")];
        paths.extend(std::env::split_paths(
            &std::env::var_os("PATH").unwrap_or_default(),
        ));
        command
            .arg(self.root.join("package-release.sh"))
            .args([format!("v{VERSION}"), COMMIT.to_owned(), TARGET.to_owned()])
            .current_dir(&self.root)
            .env("PATH", std::env::join_paths(paths).unwrap())
            .env("FIXTURE_ROOT", &self.root)
            .env("FIXTURE_VERSION", VERSION)
            .env("FIXTURE_COMMIT", COMMIT)
            .env("CARGO_TARGET_DIR", self.root.join("ambient target ignored"))
            .env("FAKE_CARGO_FAIL", "0")
            .env("FAKE_RUSTC_STATUS", "0")
            .env("FAKE_SMOKE_FAIL", "none");
        command
    }

    fn run(&self) -> Output {
        self.command().arg(&self.output).output().unwrap()
    }
}

#[test]
fn packages_only_explicit_build_outputs_and_smokes_unpacked_paths() {
    let fixture = Fixture::new();
    let result = fixture.run();
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    let build = fixture.output.join(format!("build-{TARGET}"));
    let arguments = fs::read_to_string(fixture.root.join("cargo-arguments")).unwrap();
    assert_eq!(arguments, format!("build\n--locked\n--release\n--target\n{TARGET}\n--target-dir\n{}\n-p\nsemaprax\n-p\nsemaprax-toolchain\n--bin\nsemaprax-full\n--bin\nsemapraxd\n", build.display()));
    assert_eq!(
        fs::read_to_string(fixture.root.join("cargo-commit")).unwrap(),
        COMMIT
    );
    assert!(!fixture.root.join("ambient target ignored").exists());
    let unpacked = fixture
        .output
        .join(format!("smoke-{TARGET}"))
        .join(fixture.package_name());
    assert_eq!(
        fs::read(unpacked.join("semapraxd")).unwrap(),
        fs::read(fixture.root.join("daemon")).unwrap()
    );
    assert_eq!(
        fs::read(unpacked.join("semaprax")).unwrap(),
        fs::read(fixture.root.join("cli")).unwrap()
    );
    let smoke_calls = fs::read_to_string(fixture.root.join("smoke-calls")).unwrap();
    assert_eq!(
        smoke_calls,
        format!("{0}\n{0}\n{0}\n{0}\n", unpacked.join("semaprax").display())
    );
    let mut names = fs::read_dir(&unpacked)
        .unwrap()
        .map(|entry| entry.unwrap().file_name())
        .collect::<Vec<_>>();
    names.sort();
    assert_eq!(
        names,
        [
            "LICENSE",
            "README.md",
            "release-manifest.json",
            "semaprax",
            "semapraxd",
            "smoke"
        ]
        .map(std::ffi::OsString::from)
    );
    assert_eq!(fs::read_dir(unpacked.join("smoke")).unwrap().count(), 1);
    assert_eq!(
        fs::read_to_string(unpacked.join("smoke/meaning.spx")).unwrap(),
        "module release.smoke;\n\n@id(\"release.smoke.main\")\nfn main() -> i64 { 42 }\n"
    );
    assert_eq!(
        String::from_utf8(result.stdout).unwrap(),
        format!(
            "{}\n",
            fixture
                .output
                .join(format!("{}.tar.gz", fixture.package_name()))
                .display()
        )
    );
}

#[test]
fn every_reserved_path_rejects_existing_entries_before_build() {
    for entry_type in ["file", "directory", "dangling", "symlink"] {
        for kind in ["build", "stage", "archive", "smoke"] {
            let fixture = Fixture::new();
            let name = match kind {
                "build" => format!("build-{TARGET}"),
                "stage" => fixture.package_name(),
                "archive" => format!("{}.tar.gz", fixture.package_name()),
                "smoke" => format!("smoke-{TARGET}"),
                _ => unreachable!(),
            };
            let occupied = fixture.output.join(&name);
            let absent = fixture.root.join("must not create referent");
            match entry_type {
                "dangling" => symlink(&absent, &occupied).unwrap(),
                "symlink" => symlink(fixture.root.join("README.md"), &occupied).unwrap(),
                "directory" => fs::create_dir(&occupied).unwrap(),
                "file" => fs::write(&occupied, "preserve sentinel").unwrap(),
                _ => unreachable!(),
            }
            let result = fixture.run();
            assert!(!result.status.success());
            assert!(result.stdout.is_empty());
            assert!(String::from_utf8_lossy(&result.stderr).contains("release package rejected:"));
            assert!(!fixture.root.join("cargo-arguments").exists());
            assert_eq!(fs::read_dir(&fixture.output).unwrap().count(), 1);
            assert!(!absent.exists());
            match entry_type {
                "dangling" => assert_eq!(fs::read_link(&occupied).unwrap(), absent),
                "symlink" => assert_eq!(
                    fs::read_link(&occupied).unwrap(),
                    fixture.root.join("README.md")
                ),
                "directory" => assert_eq!(fs::read_dir(&occupied).unwrap().count(), 0),
                "file" => assert_eq!(fs::read_to_string(&occupied).unwrap(), "preserve sentinel"),
                _ => unreachable!(),
            }
        }
    }
}

#[test]
fn failed_build_does_not_package_stale_binaries() {
    let fixture = Fixture::new();
    let result = fixture
        .command()
        .env("FAKE_CARGO_FAIL", "1")
        .arg(&fixture.output)
        .output()
        .unwrap();
    assert!(!result.status.success());
    assert!(!fixture
        .output
        .join(format!("{}.tar.gz", fixture.package_name()))
        .exists());
    assert!(!fixture
        .output
        .join(fixture.package_name())
        .join("semaprax")
        .exists());
    assert!(!fixture.root.join("smoke-calls").exists());
}

#[test]
fn empty_output_root_is_rejected_before_build() {
    let fixture = Fixture::new();
    let result = fixture.command().arg("").output().unwrap();
    assert!(!result.status.success());
    assert!(result.stdout.is_empty());
    assert!(!fixture.root.join("cargo-arguments").exists());
    assert_eq!(fs::read_dir(&fixture.output).unwrap().count(), 0);
}

#[test]
fn fresh_relative_output_root_with_spaces_is_supported() {
    let fixture = Fixture::with_output(false);
    let result = fixture
        .command()
        .arg("output with spaces")
        .output()
        .unwrap();
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    assert!(fixture
        .output
        .join(format!("{}.tar.gz", fixture.package_name()))
        .is_file());
}

#[test]
fn correct_stdout_cannot_mask_failed_host_query_or_smoke_commands() {
    let fixture = Fixture::with_output(false);
    let result = fixture
        .command()
        .env("FAKE_RUSTC_STATUS", "7")
        .arg(&fixture.output)
        .output()
        .unwrap();
    assert!(!result.status.success());
    assert!(String::from_utf8_lossy(&result.stderr).contains("Rust host query failed"));
    assert!(!fixture.output.exists());
    assert!(!fixture.root.join("cargo-arguments").exists());
    for mode in ["--version", "version", "check", "run"] {
        let fixture = Fixture::new();
        let result = fixture
            .command()
            .env("FAKE_SMOKE_FAIL", mode)
            .arg(&fixture.output)
            .output()
            .unwrap();
        assert!(!result.status.success(), "failed {mode} smoke was accepted");
        assert!(
            result.stdout.is_empty(),
            "failed smoke printed success archive path"
        );
        // The completed but unaccepted archive is retained, not advertised as success.
        assert!(fixture
            .output
            .join(format!("{}.tar.gz", fixture.package_name()))
            .is_file());
    }
}
