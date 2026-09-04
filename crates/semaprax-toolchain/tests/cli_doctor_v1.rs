use std::cell::RefCell;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

#[path = "../src/doctor.rs"]
mod doctor;

#[path = "cli_doctor_v1/offline_profiles.rs"]
mod offline_profiles;

use doctor::{DoctorError, DoctorHost, DoctorTarget};

struct FakeHost {
    os: &'static str,
    arch: &'static str,
    tools: BTreeMap<&'static str, Result<PathBuf, DoctorError>>,
    versions: BTreeMap<PathBuf, Result<String, DoctorError>>,
    calls: RefCell<Vec<String>>,
}

impl FakeHost {
    fn healthy() -> Self {
        let clang = tool_path("clang");
        let node = tool_path("node");
        let rustc = tool_path("rustc");
        Self {
            os: "test-os",
            arch: "test-arch",
            tools: BTreeMap::from([
                ("clang", Ok(clang.clone())),
                ("node", Ok(node.clone())),
                ("rustc", Ok(rustc.clone())),
            ]),
            versions: BTreeMap::from([
                (clang, Ok("clang version 18.1.0\nTarget: fake\n".to_owned())),
                (node, Ok("v22.14.0\n".to_owned())),
                (rustc, Ok("rustc 1.88.0 (fake 2026-01-01)\n".to_owned())),
            ]),
            calls: RefCell::new(Vec::new()),
        }
    }
}

#[cfg(windows)]
fn tool_path(name: &str) -> PathBuf {
    PathBuf::from(format!(r"C:\tools\{name}.exe"))
}

#[cfg(not(windows))]
fn tool_path(name: &str) -> PathBuf {
    PathBuf::from(format!("/tools/{name}"))
}

fn version_call(name: &str) -> String {
    format!("version:{}", tool_path(name).display())
}

fn human_tool_path(name: &str) -> String {
    tool_path(name)
        .display()
        .to_string()
        .chars()
        .flat_map(char::escape_default)
        .collect()
}

impl DoctorHost for FakeHost {
    fn os(&self) -> &str {
        self.os
    }

    fn arch(&self) -> &str {
        self.arch
    }

    fn resolve_tool(&self, name: &str) -> Result<PathBuf, DoctorError> {
        self.calls.borrow_mut().push(format!("resolve:{name}"));
        self.tools
            .get(name)
            .cloned()
            .unwrap_or_else(|| Err(DoctorError::new(format!("unexpected tool `{name}`"))))
    }

    fn run_version(&self, path: &Path) -> Result<String, DoctorError> {
        self.calls
            .borrow_mut()
            .push(format!("version:{}", path.display()));
        self.versions
            .get(path)
            .cloned()
            .unwrap_or_else(|| Err(DoctorError::new("unexpected version path")))
    }
}

fn cli(arguments: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_semaprax-full"))
        .args(arguments)
        .output()
        .unwrap()
}

#[test]
fn library_entry_preserves_ordinary_profile_policy_and_errors() {
    for arguments in [
        vec![],
        vec!["--json"],
        vec!["--profile", "profile-v1", "--target", "all", "--json"],
        vec!["--profile", "bad/profile"],
        vec!["--unknown"],
    ] {
        let arguments = arguments
            .iter()
            .map(|value| (*value).to_owned())
            .collect::<Vec<_>>();
        let expected = doctor::run(&arguments)
            .map(|outcome| (outcome.output, outcome.exit_code))
            .map_err(|error| error.to_string());
        assert_eq!(semaprax_toolchain::run_doctor(&arguments), expected);
    }
}

#[test]
fn default_is_source_contributor_mode_and_json_is_canonical() {
    let host = FakeHost::healthy();
    let outcome = doctor::inspect(&host, DoctorTarget::Contributor, true, false).unwrap();
    assert_eq!(outcome.exit_code, 0);
    assert_eq!(
        outcome.output,
        "{\"schema\":\"semaprax.doctor.v1\",\"target\":\"contributor\",\"checks\":[{\"id\":\"semaprax\",\"required\":true,\"status\":\"ok\",\"detail\":\"0.3.0\"},{\"id\":\"os\",\"required\":true,\"status\":\"ok\",\"detail\":\"test-os\"},{\"id\":\"arch\",\"required\":true,\"status\":\"ok\",\"detail\":\"test-arch\"},{\"id\":\"release\",\"required\":true,\"status\":\"ok\",\"detail\":\"debug\"},{\"id\":\"rust\",\"required\":true,\"status\":\"ok\",\"detail\":\"rustc 1.88.0 (fake 2026-01-01)\"}]}\n"
    );
    assert_eq!(
        *host.calls.borrow(),
        ["resolve:rustc".to_owned(), version_call("rustc")]
    );
}

#[test]
fn all_target_human_output_is_deterministic_and_reports_every_check() {
    let host = FakeHost::healthy();
    let first = doctor::inspect(&host, DoctorTarget::All, false, true).unwrap();
    let second = doctor::inspect(&FakeHost::healthy(), DoctorTarget::All, false, true).unwrap();
    assert_eq!(first.exit_code, 0);
    assert_eq!(first.output, second.output);
    assert_eq!(
        first.output,
        format!(
            "semaprax doctor (all)\n\
ok semaprax: 0.3.0\n\
ok os: test-os\n\
ok arch: test-arch\n\
ok release: release\n\
ok clang: {} (clang version 18.1.0)\n\
ok node: v22.14.0\n\
ok rust: rustc 1.88.0 (fake 2026-01-01)\n",
            human_tool_path("clang")
        )
    );
    assert!(first.output.ends_with('\n'));
}

#[test]
fn target_selection_does_not_probe_unrequested_tools() {
    let native = FakeHost::healthy();
    assert_eq!(
        doctor::inspect(&native, DoctorTarget::Native, true, false)
            .unwrap()
            .exit_code,
        0
    );
    assert_eq!(
        *native.calls.borrow(),
        ["resolve:clang".to_owned(), version_call("clang")]
    );

    let web = FakeHost::healthy();
    assert_eq!(
        doctor::inspect(&web, DoctorTarget::Web, true, false)
            .unwrap()
            .exit_code,
        0
    );
    assert_eq!(
        *web.calls.borrow(),
        ["resolve:node".to_owned(), version_call("node")]
    );
}

#[test]
fn required_tool_and_version_failures_return_status_one() {
    let mut old_node = FakeHost::healthy();
    old_node
        .versions
        .insert(tool_path("node"), Ok("v21.9.0\n".to_owned()));
    let outcome = doctor::inspect(&old_node, DoctorTarget::Web, true, false).unwrap();
    assert_eq!(outcome.exit_code, 1);
    assert!(outcome.output.contains(
        "{\"id\":\"node\",\"required\":true,\"status\":\"failed\",\"detail\":\"v21.9.0 (requires major version 22 or newer; found 21)\"}"
    ));

    let mut old_rust = FakeHost::healthy();
    old_rust
        .versions
        .insert(tool_path("rustc"), Ok("rustc 1.87.0 (fake)\n".to_owned()));
    assert_eq!(
        doctor::inspect(&old_rust, DoctorTarget::Contributor, false, false)
            .unwrap()
            .exit_code,
        1
    );

    let mut missing_clang = FakeHost::healthy();
    missing_clang.tools.insert(
        "clang",
        Err(DoctorError::new("tool `clang` was not found on PATH")),
    );
    assert_eq!(
        doctor::inspect(&missing_clang, DoctorTarget::Native, false, false)
            .unwrap()
            .exit_code,
        1
    );

    let mut broken_clang = FakeHost::healthy();
    broken_clang.versions.insert(
        tool_path("clang"),
        Err(DoctorError::new("clang --version failed")),
    );
    let outcome = doctor::inspect(&broken_clang, DoctorTarget::Native, true, false).unwrap();
    assert_eq!(outcome.exit_code, 1);
    assert!(outcome.output.contains("clang --version failed"));
}

#[test]
fn relative_clang_path_fails_without_execution() {
    let mut host = FakeHost::healthy();
    host.tools.insert("clang", Ok(PathBuf::from("tools/clang")));
    let outcome = doctor::inspect(&host, DoctorTarget::Native, false, false).unwrap();
    assert_eq!(outcome.exit_code, 1);
    assert!(outcome
        .output
        .contains("resolved Clang path is not absolute"));
    assert_eq!(*host.calls.borrow(), ["resolve:clang"]);
}

#[test]
fn malformed_invocations_and_internal_invariants_are_status_two_conditions() {
    for arguments in [
        &["--target"][..],
        &["--target", "source"][..],
        &["--target", "native", "--target", "web"][..],
        &["--json", "--json"][..],
        &["extra"][..],
    ] {
        let arguments = arguments
            .iter()
            .map(|argument| (*argument).to_owned())
            .collect::<Vec<_>>();
        assert!(doctor::run(&arguments).is_err());
    }
    for arguments in [
        &["doctor", "--target"][..],
        &["doctor", "--target", "source"][..],
        &["doctor", "--target", "native", "--target", "web"][..],
        &["doctor", "--json", "--json"][..],
        &["doctor", "extra"][..],
    ] {
        let output = cli(arguments);
        assert_eq!(output.status.code(), Some(2), "{arguments:?}");
        assert!(output.stdout.is_empty(), "{arguments:?}");
        assert!(!output.stderr.is_empty(), "{arguments:?}");
    }

    let mut invalid = FakeHost::healthy();
    invalid.os = "";
    assert!(doctor::inspect(&invalid, DoctorTarget::All, true, false).is_err());
    assert!(invalid.calls.borrow().is_empty());
}

#[test]
fn complete_version_tokens_reject_partial_numbers_and_malformed_suffixes() {
    for token in [
        "22",
        "22.1",
        "22.garbage",
        "22.1.garbage",
        "22.1.0.1",
        "+22.1.0",
        "022.1.0",
        "22.01.0",
        "22.1.00",
        "22.1.0-",
        "22.1.0+",
        "22.1.0-alpha..1",
        "22.1.0-alpha.01",
        "22.1.0+a+b",
        "22.1.0-β",
        "18446744073709551616.0.0",
    ] {
        for (tool, target, text) in [
            ("node", DoctorTarget::Web, format!("v{token}")),
            (
                "rustc",
                DoctorTarget::Contributor,
                format!("rustc {token} (fixture)"),
            ),
        ] {
            let mut host = FakeHost::healthy();
            host.versions.insert(tool_path(tool), Ok(text));
            let outcome = doctor::inspect(&host, target, true, false).unwrap();
            assert_eq!(outcome.exit_code, 1, "{tool}: {token}");
            assert!(outcome.output.contains("unrecognized"));
        }
    }
}

#[test]
fn complete_version_tokens_preserve_numeric_threshold_and_channel_policy() {
    for token in [
        "22.0.0",
        "22.1.0-rc.1",
        "22.1.0+build.01",
        "22.1.0-nightly+build",
    ] {
        let mut host = FakeHost::healthy();
        host.versions
            .insert(tool_path("node"), Ok(format!("v{token}")));
        assert_eq!(
            doctor::inspect(&host, DoctorTarget::Web, true, false)
                .unwrap()
                .exit_code,
            0
        );
    }
    for token in [
        "1.88.0",
        "1.88.0-nightly",
        "1.88.0-beta.1",
        "1.88.0-dev+local.01",
    ] {
        let mut host = FakeHost::healthy();
        host.versions
            .insert(tool_path("rustc"), Ok(format!("rustc {token} (fixture)")));
        assert_eq!(
            doctor::inspect(&host, DoctorTarget::Contributor, true, false)
                .unwrap()
                .exit_code,
            0
        );
    }
}

#[cfg(unix)]
fn assert_offline_profile_failure(stdout: &[u8], selector: Option<&str>) {
    let detail = match selector {
        None => "an explicit offline profile is required; use --profile <id>",
        Some("fixture-v1") => "offline profile `fixture-v1` is unavailable on this host",
        Some(_) => panic!("unexpected fixture selector"),
    };
    let version = env!("CARGO_PKG_VERSION");
    let os = std::env::consts::OS;
    let arch = std::env::consts::ARCH;
    let release = if cfg!(debug_assertions) {
        "debug"
    } else {
        "release"
    };
    let expected = format!(
        "{{\"schema\":\"semaprax.doctor.v1\",\"target\":\"all\",\"checks\":[\
{{\"id\":\"semaprax\",\"required\":true,\"status\":\"ok\",\"detail\":\"{version}\"}},\
{{\"id\":\"os\",\"required\":true,\"status\":\"ok\",\"detail\":\"{os}\"}},\
{{\"id\":\"arch\",\"required\":true,\"status\":\"ok\",\"detail\":\"{arch}\"}},\
{{\"id\":\"release\",\"required\":true,\"status\":\"ok\",\"detail\":\"{release}\"}},\
{{\"id\":\"profile\",\"required\":true,\"status\":\"failed\",\"detail\":\"{detail}\"}},\
{{\"id\":\"clang\",\"required\":true,\"status\":\"failed\",\"detail\":\"not probed: no admitted offline profile\"}},\
{{\"id\":\"node\",\"required\":true,\"status\":\"failed\",\"detail\":\"not probed: no admitted offline profile\"}},\
{{\"id\":\"rust\",\"required\":true,\"status\":\"failed\",\"detail\":\"not probed: no admitted offline profile\"}}]}}\n"
    );
    assert_eq!(stdout, expected.as_bytes());
}

#[test]
fn malformed_offline_profile_selectors_are_cli_errors() {
    for selector in ["", "Fixture-v1", "../fixture-v1", "fixture_v1", "a/b"] {
        let output = cli(&["doctor", "--profile", selector, "--json"]);
        assert_eq!(output.status.code(), Some(2), "{selector:?}");
        assert!(output.stdout.is_empty(), "{selector:?}");
        assert_eq!(
            output.stderr,
            b"doctor: invalid doctor profile identifier; expected [a-z][a-z0-9-]{0,63}\nhint: run `semaprax doctor --help` for usage\n"
        );
    }
}

#[cfg(unix)]
#[test]
fn real_cli_requires_offline_profile_without_path_or_home_fallback() {
    use std::os::unix::fs::{symlink, MetadataExt, PermissionsExt};
    const IMPLEMENTATION: &[u8] = b"#!/bin/sh\n[ \"$#\" = 1 ] && [ \"$1\" = --version ] || exit 91\nprintf '%s\\n' \"${0##*/}\" >> probe-marker\ncase \"${0##*/}\" in\nrustc) printf 'rustc 1.88.0 (fixture)\\n' ;;\nnode) printf 'v22.14.0\\n' ;;\nclang) printf 'clang version 18.1.0\\n' ;;\n*) exit 92 ;;\nesac\n";
    let root = std::env::temp_dir().join(format!(
        "semaprax-doctor-no-fallback-{}",
        std::process::id()
    ));
    std::fs::create_dir(&root).unwrap();
    let root_identity = std::fs::symlink_metadata(&root).unwrap();
    for directory in ["control", "probe", "tools", "home-a", "home-b"] {
        std::fs::create_dir(root.join(directory)).unwrap();
    }
    let implementation = root.join("tools/multicall");
    std::fs::write(&implementation, IMPLEMENTATION).unwrap();
    std::fs::set_permissions(&implementation, std::fs::Permissions::from_mode(0o700)).unwrap();
    let implementation_identity = std::fs::symlink_metadata(&implementation).unwrap();
    for tool in ["rustc", "node", "clang"] {
        symlink("multicall", root.join("tools").join(tool)).unwrap();
    }
    // Calibrate actual execution and healthy version bytes independently of doctor.
    // Its separate CWD retains the marker; the doctor CWD must remain empty.
    for (tool, expected) in [
        ("rustc", "rustc 1.88.0 (fixture)\n"),
        ("node", "v22.14.0\n"),
        ("clang", "clang version 18.1.0\n"),
    ] {
        let control = Command::new(root.join("tools").join(tool))
            .arg("--version")
            .current_dir(root.join("control"))
            .output()
            .unwrap();
        assert!(control.status.success());
        assert_eq!(control.stdout, expected.as_bytes());
        assert!(control.stderr.is_empty());
    }
    assert_eq!(
        std::fs::read(root.join("control/probe-marker")).unwrap(),
        b"rustc\nnode\nclang\n"
    );

    for (path, home) in [
        (root.join("tools"), "home-a"),
        (PathBuf::from("../tools"), "home-b"),
    ] {
        for selector in [None, Some("fixture-v1")] {
            let mut command = Command::new(env!("CARGO_BIN_EXE_semaprax-full"));
            command.args(["doctor", "--target", "all", "--json"]);
            if let Some(selector) = selector {
                command.args(["--profile", selector]);
            }
            command.current_dir(root.join("probe")).env("PATH", &path);
            for key in ["HOME", "CARGO_HOME", "RUSTUP_HOME", "USERPROFILE"] {
                command.env(key, root.join(home));
            }
            command
                .env("RUSTUP_TOOLCHAIN", home)
                .env("RUSTUP_AUTO_INSTALL", "1");
            let output = command.output().unwrap();
            assert_eq!(output.status.code(), Some(1));
            assert!(output.stderr.is_empty());
            assert_offline_profile_failure(&output.stdout, selector);
            assert!(std::fs::read_dir(root.join("probe"))
                .unwrap()
                .next()
                .is_none());
            let current = std::fs::symlink_metadata(&implementation).unwrap();
            assert!(current.is_file() && !current.file_type().is_symlink());
            assert_eq!(
                (current.dev(), current.ino()),
                (implementation_identity.dev(), implementation_identity.ino())
            );
            assert_eq!(std::fs::read(&implementation).unwrap(), IMPLEMENTATION);
        }
    }

    // Preflight the complete fixed inventory before any cleanup; failures retain it.
    let names = |directory: &Path| {
        let mut names = std::fs::read_dir(directory)
            .unwrap()
            .map(|entry| entry.unwrap().file_name())
            .collect::<Vec<_>>();
        names.sort();
        names
    };
    assert_eq!(
        names(&root),
        ["control", "home-a", "home-b", "probe", "tools"].map(std::ffi::OsString::from)
    );
    let current_root = std::fs::symlink_metadata(&root).unwrap();
    assert!(current_root.is_dir() && !current_root.file_type().is_symlink());
    assert_eq!(
        (current_root.dev(), current_root.ino()),
        (root_identity.dev(), root_identity.ino())
    );
    for directory in ["control", "probe", "tools", "home-a", "home-b"] {
        let metadata = std::fs::symlink_metadata(root.join(directory)).unwrap();
        assert!(metadata.is_dir() && !metadata.file_type().is_symlink());
    }
    assert_eq!(
        names(&root.join("control")),
        [std::ffi::OsString::from("probe-marker")]
    );
    let marker = std::fs::symlink_metadata(root.join("control/probe-marker")).unwrap();
    assert!(marker.is_file() && !marker.file_type().is_symlink());
    assert_eq!(
        std::fs::read(root.join("control/probe-marker")).unwrap(),
        b"rustc\nnode\nclang\n"
    );
    for directory in ["probe", "home-a", "home-b"] {
        assert!(names(&root.join(directory)).is_empty());
    }
    assert_eq!(
        names(&root.join("tools")),
        ["clang", "multicall", "node", "rustc"].map(std::ffi::OsString::from)
    );
    for tool in ["rustc", "node", "clang"] {
        let link = root.join("tools").join(tool);
        assert!(std::fs::symlink_metadata(&link)
            .unwrap()
            .file_type()
            .is_symlink());
        assert_eq!(std::fs::read_link(link).unwrap(), Path::new("multicall"));
    }
    for file in [
        "control/probe-marker",
        "tools/rustc",
        "tools/node",
        "tools/clang",
        "tools/multicall",
    ] {
        std::fs::remove_file(root.join(file)).unwrap();
    }
    for directory in ["control", "probe", "tools", "home-a", "home-b"] {
        std::fs::remove_dir(root.join(directory)).unwrap();
    }
    std::fs::remove_dir(root).unwrap();
}
