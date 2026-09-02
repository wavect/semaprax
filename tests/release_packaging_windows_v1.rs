#![cfg(windows)]

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::sync::atomic::{AtomicU64, Ordering};

const TARGET: &str = "x86_64-pc-windows-msvc";
const PACKAGE: &str = "semaprax-v0.2.0-x86_64-pc-windows-msvc";
const COMMIT: &str = "0123456789abcdef0123456789abcdef01234567";
const STALE: &[u8] = b"stale binary must not enter the archive\n";
const OUTPUT: &str = "release [output] space";
static NEXT: AtomicU64 = AtomicU64::new(0);

fn fresh_root() -> PathBuf {
    for _ in 0..64 {
        let path = std::env::temp_dir().join(format!(
            "spx-release-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        assert!(path.is_absolute());
        match fs::create_dir(&path) {
            Ok(()) => {
                // `temp_dir` on hosted Windows may be the short `RUNNER~1` form
                // while PowerShell's `GetUnresolvedProviderPathFromPSPath` for
                // the release output resolves to the long `runneradmin` form.
                // Canonicalize and strip the verbatim `\\?\` prefix so
                // `build.display()` in the expected `target-dir` line matches
                // the long non-verbatim path logged by the fixture tool.
                let canonical = path.canonicalize().unwrap();
                let text = canonical.to_string_lossy();
                let long = if text.starts_with(r"\\?\") {
                    if text.starts_with(r"\\?\UNC\") {
                        PathBuf::from(format!(r"\\{}", &text[8..]))
                    } else {
                        PathBuf::from(text[4..].to_string())
                    }
                } else {
                    canonical
                };
                eprintln!("retained release packaging fixture: {}", long.display());
                return long;
            }
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(error) => panic!("create fixture: {error}"),
        }
    }
    panic!("cannot reserve release fixture");
}

fn require_success(output: &Output) {
    assert!(
        output.status.success(),
        "status {:?}\nstdout: {}\nstderr: {}",
        output.status.code(),
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

struct Fixture {
    root: PathBuf,
    tools: PathBuf,
    launcher: PathBuf,
}

impl Fixture {
    fn new() -> Self {
        let root = fresh_root();
        let tools = root.join("tools");
        fs::create_dir(&tools).unwrap();
        let source = root.join("tool.rs");
        fs::write(
            &source,
            include_str!("release_packaging_windows_v1/tool.rs"),
        )
        .unwrap();
        let executable = tools.join("cargo.exe");
        // One compilation of a std-only executable stand-in; this does not
        // build SEMAPRAX or prove the real release compiler/toolchain works.
        require_success(
            &Command::new(std::env::var_os("RUSTC").unwrap_or_else(|| "rustc".into()))
                .args(["--edition=2021", "--crate-name", "release_fixture_tool"])
                .arg(&source)
                .arg("-o")
                .arg(&executable)
                .output()
                .unwrap(),
        );
        fs::copy(&executable, tools.join("rustc.exe")).unwrap();
        let launcher = root.join("launch.ps1");
        fs::write(
            &launcher,
            include_str!("release_packaging_windows_v1/launch.ps1"),
        )
        .unwrap();
        Self {
            root,
            tools,
            launcher,
        }
    }

    fn case(&self, label: &str) -> Case<'_> {
        let root = self.root.join(label);
        fs::create_dir(&root).unwrap();
        let repository = root.join("repository space");
        let process_cwd = root.join("process space");
        fs::create_dir(&repository).unwrap();
        fs::create_dir(&process_cwd).unwrap();
        fs::write(
            repository.join("Cargo.toml"),
            "[package]\nname = \"semaprax\"\nversion = \"0.2.0\"\n",
        )
        .unwrap();
        fs::write(repository.join("LICENSE"), b"fixture license\n").unwrap();
        fs::write(repository.join("README.md"), b"fixture readme\n").unwrap();
        let stale = repository.join("target").join(TARGET).join("release");
        fs::create_dir_all(&stale).unwrap();
        // A stale CLI can pass the same version/smoke checks. The daemon's
        // distinct bytes prove the archive must use this invocation's build.
        fs::copy(
            self.tools.join("cargo.exe"),
            stale.join("semaprax-full.exe"),
        )
        .unwrap();
        fs::write(stale.join("semapraxd.exe"), STALE).unwrap();
        let ambient = root.join("ambient cargo target");
        fs::create_dir(&ambient).unwrap();
        fs::write(ambient.join("sentinel"), b"ambient unchanged\n").unwrap();
        Case {
            fixture: self,
            root,
            repository,
            process_cwd,
            ambient,
        }
    }
}

struct Case<'a> {
    fixture: &'a Fixture,
    root: PathBuf,
    repository: PathBuf,
    process_cwd: PathBuf,
    ambient: PathBuf,
}

impl Case<'_> {
    fn output(&self) -> PathBuf {
        self.repository.join(OUTPUT)
    }

    fn run(&self, commit: &str, absolute: bool) -> Output {
        let mut paths = vec![self.fixture.tools.clone()];
        paths.extend(std::env::split_paths(&std::env::var_os("PATH").unwrap()));
        Command::new(std::env::var_os("SEMAPRAX_POWERSHELL").unwrap_or_else(|| "pwsh".into()))
            .args(["-NoLogo", "-NoProfile", "-NonInteractive", "-File"])
            .arg(&self.fixture.launcher)
            .current_dir(&self.process_cwd)
            .env("PATH", std::env::join_paths(paths).unwrap())
            .env("CARGO_TARGET_DIR", &self.ambient)
            .env("RELEASE_FIXTURE_LOG", self.root.join("calls.log"))
            .env("RELEASE_FIXTURE_REPOSITORY", &self.repository)
            .env("RELEASE_FIXTURE_PROCESS_CWD", &self.process_cwd)
            .env(
                "RELEASE_FIXTURE_SCRIPT",
                Path::new(env!("CARGO_MANIFEST_DIR")).join("scripts/package-release.ps1"),
            )
            .env("RELEASE_FIXTURE_COMMIT", commit)
            .env(
                "RELEASE_FIXTURE_OUTPUT",
                if absolute {
                    self.output()
                } else {
                    PathBuf::from(OUTPUT)
                },
            )
            .env(
                "RELEASE_FIXTURE_UNPACKED_BINARY",
                self.output()
                    .join(format!("smoke-{TARGET}"))
                    .join(PACKAGE)
                    .join("semaprax.exe"),
            )
            .output()
            .unwrap()
    }

    fn unchanged_sentinels(&self) {
        let stale = self.repository.join("target").join(TARGET).join("release");
        assert_eq!(
            fs::read(stale.join("semaprax-full.exe")).unwrap(),
            fs::read(self.fixture.tools.join("cargo.exe")).unwrap()
        );
        assert_eq!(fs::read(stale.join("semapraxd.exe")).unwrap(), STALE);
        assert_eq!(names(&self.ambient), ["sentinel"]);
        assert_eq!(
            fs::read(self.ambient.join("sentinel")).unwrap(),
            b"ambient unchanged\n"
        );
        assert!(!self.process_cwd.join(OUTPUT).exists());
    }
}

fn names(path: &Path) -> Vec<String> {
    let mut names = fs::read_dir(path)
        .unwrap()
        .map(|entry| entry.unwrap().file_name().into_string().unwrap())
        .collect::<Vec<_>>();
    names.sort();
    names
}

#[test]
fn windows_packager_uses_fresh_explicit_builds_and_powershell_paths() {
    let fixture = Fixture::new();
    for (label, absolute, existing_output) in [("relative", false, false), ("absolute", true, true)]
    {
        let case = fixture.case(label);
        if existing_output {
            fs::create_dir(case.output()).unwrap();
        }
        require_success(&case.run(COMMIT, absolute));
        case.unchanged_sentinels();
        let build = case.output().join(format!("build-{TARGET}"));
        assert_eq!(
            fs::read(build.join(TARGET).join("release/current-build-marker")).unwrap(),
            b"fresh fake build\n"
        );
        let expected_calls = format!("rustc\ncargo\ntarget-dir:{}\nsmoke:--version\nsmoke:version-json\nsmoke:check\nsmoke:run\n", build.display());
        assert_eq!(
            fs::read_to_string(case.root.join("calls.log")).unwrap(),
            expected_calls
        );
        let unpacked = case.output().join(format!("smoke-{TARGET}")).join(PACKAGE);
        assert_eq!(
            names(&unpacked),
            [
                "LICENSE",
                "README.md",
                "release-manifest.json",
                "semaprax.exe",
                "semapraxd.exe",
                "smoke"
            ]
        );
        assert_eq!(names(&unpacked.join("smoke")), ["meaning.spx"]);
        for name in ["semaprax.exe", "semapraxd.exe"] {
            assert_eq!(
                fs::read(unpacked.join(name)).unwrap(),
                fs::read(fixture.tools.join("cargo.exe")).unwrap()
            );
        }
        assert_eq!(
            fs::read(unpacked.join("LICENSE")).unwrap(),
            b"fixture license\n"
        );
        assert_eq!(
            fs::read(unpacked.join("README.md")).unwrap(),
            b"fixture readme\n"
        );
        let manifest = fs::read_to_string(unpacked.join("release-manifest.json")).unwrap();
        assert!(!manifest.contains('\r') && !manifest.starts_with('\u{feff}'));
        let expected_manifest = format!(
            "{{\n  \"schema\": \"semaprax.release-artifact.v1\",\n  \"version\": \"0.2.0\",\n  \"commit\": \"{COMMIT}\",\n  \"target\": \"{TARGET}\",\n  \"maturity\": \"pre-alpha\",\n  \"binaries\": [\"semaprax\", \"semapraxd\"],\n  \"nonclaims\": [\n    \"production-ready\",\n    \"stable language ABI\",\n    \"stable public protocol\",\n    \"safety-critical suitability\"\n  ]\n}}\n"
        );
        assert_eq!(manifest, expected_manifest);
        assert!(case.output().join(format!("{PACKAGE}.zip")).is_file());
    }

    for (label, commit) in [
        ("linefeed", format!("{COMMIT}\n")),
        ("crlf", format!("{COMMIT}\r\n")),
        ("uppercase", "A".repeat(40)),
        ("short", "0".repeat(39)),
    ] {
        let case = fixture.case(label);
        let output = case.run(&commit, false);
        assert!(!output.status.success());
        assert!(String::from_utf8_lossy(&output.stderr)
            .contains("commit must be exactly 40 lowercase hexadecimal characters"));
        assert!(!case.output().exists());
        assert!(
            !case.root.join("calls.log").exists(),
            "invalid identity reached a tool"
        );
        case.unchanged_sentinels();
    }

    let case = fixture.case("occupied build");
    let build = case.output().join(format!("build-{TARGET}"));
    fs::create_dir_all(&build).unwrap();
    fs::write(build.join("sentinel"), b"do not reuse\n").unwrap();
    let output = case.run(COMMIT, false);
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("output path already exists"));
    assert_eq!(names(&case.output()), [format!("build-{TARGET}")]);
    assert_eq!(fs::read(build.join("sentinel")).unwrap(), b"do not reuse\n");
    assert_eq!(
        fs::read_to_string(case.root.join("calls.log")).unwrap(),
        "rustc\n"
    );
    case.unchanged_sentinels();

    // Directory junctions do not require symbolic-link creation privilege.
    // Keep the broken fixture link and all other residue for inspection.
    let case = fixture.case("dangling build");
    fs::create_dir(case.output()).unwrap();
    let target = case.root.join("junction target");
    fs::create_dir(&target).unwrap();
    let link = case.output().join(format!("build-{TARGET}"));
    require_success(
        &Command::new(
            std::env::var_os("SEMAPRAX_POWERSHELL").unwrap_or_else(|| "pwsh".into()),
        )
        .args(["-NoLogo", "-NoProfile", "-NonInteractive", "-Command"])
        .arg("$ErrorActionPreference = 'Stop'; New-Item -ItemType Junction -Path $env:RELEASE_FIXTURE_LINK -Value $env:RELEASE_FIXTURE_LINK_TARGET | Out-Null")
        .env("RELEASE_FIXTURE_LINK", &link)
        .env("RELEASE_FIXTURE_LINK_TARGET", &target)
        .output()
        .unwrap(),
    );
    use std::os::windows::fs::MetadataExt;
    let target_metadata = fs::symlink_metadata(&target).unwrap();
    assert!(target_metadata.is_dir());
    assert_eq!(target_metadata.file_attributes() & 0x400, 0);
    assert!(names(&target).is_empty());
    fs::remove_dir(&target).unwrap();
    assert_ne!(
        fs::symlink_metadata(&link).unwrap().file_attributes() & 0x400,
        0
    );
    assert!(fs::metadata(&link).is_err(), "junction must be dangling");
    let output = case.run(COMMIT, false);
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("output path already exists"));
    assert_eq!(names(&case.output()), [format!("build-{TARGET}")]);
    assert_eq!(
        fs::read_to_string(case.root.join("calls.log")).unwrap(),
        "rustc\n"
    );
    assert_ne!(
        fs::symlink_metadata(&link).unwrap().file_attributes() & 0x400,
        0
    );
    case.unchanged_sentinels();
}
