#[path = "support/full_toolchain.rs"]
mod full_toolchain;
#[path = "support/native_rust_cargo.rs"]
mod native_rust_cargo;

use std::cell::RefCell;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

#[path = "../crates/semaprax-toolchain/src/doctor.rs"]
mod doctor;

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
    Command::new(full_toolchain::binary())
        .args(arguments)
        .output()
        .unwrap()
}

#[test]
fn default_is_source_contributor_mode_and_json_is_canonical() {
    let host = FakeHost::healthy();
    let outcome = doctor::inspect(&host, DoctorTarget::Contributor, true, false).unwrap();
    assert_eq!(outcome.exit_code, 0);
    assert_eq!(
        outcome.output,
        "{\"schema\":\"semaprax.doctor.v1\",\"target\":\"contributor\",\"checks\":[{\"id\":\"semaprax\",\"required\":true,\"status\":\"ok\",\"detail\":\"0.2.0\"},{\"id\":\"os\",\"required\":true,\"status\":\"ok\",\"detail\":\"test-os\"},{\"id\":\"arch\",\"required\":true,\"status\":\"ok\",\"detail\":\"test-arch\"},{\"id\":\"release\",\"required\":true,\"status\":\"ok\",\"detail\":\"debug\"},{\"id\":\"rust\",\"required\":true,\"status\":\"ok\",\"detail\":\"rustc 1.88.0 (fake 2026-01-01)\"}]}\n"
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
ok semaprax: 0.2.0\n\
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
#[test]
fn real_path_resolution_preserves_multicall_name_and_skips_unusable_candidates() {
    use std::os::unix::fs::{symlink, PermissionsExt};
    let root =
        std::env::temp_dir().join(format!("semaprax-doctor-resolution-{}", std::process::id()));
    std::fs::create_dir(&root).unwrap();
    for directory in ["broken", "shadow", "tools"] {
        std::fs::create_dir(root.join(directory)).unwrap();
    }
    symlink("absent", root.join("broken/rustc")).unwrap();
    std::fs::write(root.join("shadow/rustc"), b"not executable").unwrap();
    std::fs::set_permissions(
        root.join("shadow/rustc"),
        std::fs::Permissions::from_mode(0o600),
    )
    .unwrap();
    let implementation = root.join("tools/multicall");
    std::fs::write(&implementation, b"#!/bin/sh\ncase \"${0##*/}\" in\nrustc) printf 'rustc 1.88.0 (fixture)\\n' ;;\n*) printf 'multicall 0.1.0\\n' ;;\nesac\n").unwrap();
    std::fs::set_permissions(&implementation, std::fs::Permissions::from_mode(0o700)).unwrap();
    symlink("multicall", root.join("tools/rustc")).unwrap();
    let run = |path: std::ffi::OsString| {
        Command::new(full_toolchain::binary())
            .args(["doctor", "--json"])
            .current_dir(&root)
            .env("PATH", path)
            .output()
            .unwrap()
    };
    let path = std::env::join_paths(["broken", "shadow", "tools"]).unwrap();
    let output = run(path.clone());
    assert_eq!(
        output.status.code(),
        Some(0),
        "status={:?}\nstdout={}\nstderr={}",
        output.status,
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(String::from_utf8_lossy(&output.stdout).contains("rustc 1.88.0 (fixture)"));
    assert!(output.stderr.is_empty());
    // A found executable that fails is not silently replaced by another tool.
    std::fs::write(&implementation, b"#!/bin/sh\nexit 7\n").unwrap();
    let failed = run(path);
    assert_eq!(failed.status.code(), Some(1));
    assert!(String::from_utf8_lossy(&failed.stdout).contains("--version exited unsuccessfully"));
    let missing = run(std::env::join_paths(["broken", "shadow"]).unwrap());
    assert_eq!(missing.status.code(), Some(1));
    assert!(String::from_utf8_lossy(&missing.stdout).contains("was not found on PATH"));
    for file in [
        "broken/rustc",
        "shadow/rustc",
        "tools/rustc",
        "tools/multicall",
    ] {
        std::fs::remove_file(root.join(file)).unwrap();
    }
    for directory in ["broken", "shadow", "tools"] {
        std::fs::remove_dir(root.join(directory)).unwrap();
    }
    std::fs::remove_dir(root).unwrap();
}
