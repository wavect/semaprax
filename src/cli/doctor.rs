use std::fmt;
use std::path::{Path, PathBuf};

use semaprax::diagnostic::quote_json;

#[path = "doctor/version_token.rs"]
mod version_token;

const SCHEMA: &str = "semaprax.doctor.v1";
const VERSION: &str = env!("CARGO_PKG_VERSION");
const MIN_NODE_MAJOR: u64 = 22;
const MIN_RUST_MAJOR: u64 = 1;
const MIN_RUST_MINOR: u64 = 88;

pub(crate) trait DoctorHost {
    fn os(&self) -> &str;
    fn arch(&self) -> &str;
    fn resolve_tool(&self, name: &str) -> Result<PathBuf, DoctorError>;
    fn run_version(&self, path: &Path) -> Result<String, DoctorError>;
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct DoctorError {
    message: String,
}

impl DoctorError {
    pub(crate) fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }
}

impl fmt::Display for DoctorError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.message)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum DoctorTarget {
    Contributor,
    Native,
    Web,
    All,
}

impl DoctorTarget {
    fn text(self) -> &'static str {
        match self {
            Self::Contributor => "contributor",
            Self::Native => "native",
            Self::Web => "web",
            Self::All => "all",
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct DoctorOptions {
    target: DoctorTarget,
    json: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct Check {
    id: &'static str,
    required: bool,
    passed: bool,
    detail: String,
}

impl Check {
    fn ok(id: &'static str, detail: impl Into<String>) -> Self {
        Self {
            id,
            required: true,
            passed: true,
            detail: detail.into(),
        }
    }

    fn failed(id: &'static str, detail: impl Into<String>) -> Self {
        Self {
            id,
            required: true,
            passed: false,
            detail: detail.into(),
        }
    }
}

pub(crate) struct DoctorOutcome {
    pub(crate) output: String,
    pub(crate) exit_code: u8,
}

pub(crate) fn run(arguments: &[String]) -> Result<DoctorOutcome, DoctorError> {
    let options = parse(arguments)?;
    inspect(
        &RealDoctorHost,
        options.target,
        options.json,
        !cfg!(debug_assertions),
    )
}

pub(crate) fn inspect(
    host: &dyn DoctorHost,
    target: DoctorTarget,
    json: bool,
    release_build: bool,
) -> Result<DoctorOutcome, DoctorError> {
    validate_host_fact("operating system", host.os())?;
    validate_host_fact("architecture", host.arch())?;

    let mut checks = vec![
        Check::ok("semaprax", VERSION),
        Check::ok("os", host.os()),
        Check::ok("arch", host.arch()),
        Check::ok("release", if release_build { "release" } else { "debug" }),
    ];
    if matches!(target, DoctorTarget::Native | DoctorTarget::All) {
        checks.push(check_clang(host));
    }
    if matches!(target, DoctorTarget::Web | DoctorTarget::All) {
        checks.push(check_node(host));
    }
    if matches!(target, DoctorTarget::Contributor | DoctorTarget::All) {
        checks.push(check_rust(host));
    }

    let passed = checks.iter().all(|check| !check.required || check.passed);
    let output = if json {
        render_json(target, &checks)
    } else {
        render_human(target, &checks)
    };
    Ok(DoctorOutcome {
        output,
        exit_code: if passed { 0 } else { 1 },
    })
}

fn parse(arguments: &[String]) -> Result<DoctorOptions, DoctorError> {
    let mut target = DoctorTarget::Contributor;
    let mut target_seen = false;
    let mut json = false;
    let mut index = 0usize;
    while index < arguments.len() {
        match arguments[index].as_str() {
            "--json" => {
                if json {
                    return Err(DoctorError::new("duplicate doctor option `--json`"));
                }
                json = true;
                index += 1;
            }
            "--target" => {
                if target_seen {
                    return Err(DoctorError::new("duplicate doctor option `--target`"));
                }
                let value = arguments.get(index + 1).ok_or_else(|| {
                    DoctorError::new("doctor option `--target` requires native, web, or all")
                })?;
                target = match value.as_str() {
                    "native" => DoctorTarget::Native,
                    "web" => DoctorTarget::Web,
                    "all" => DoctorTarget::All,
                    _ => {
                        return Err(DoctorError::new(format!(
                            "unknown doctor target `{value}`; expected native, web, or all"
                        )))
                    }
                };
                target_seen = true;
                index += 2;
            }
            option => {
                return Err(DoctorError::new(format!(
                    "unknown doctor option `{option}`"
                )))
            }
        }
    }
    Ok(DoctorOptions { target, json })
}

fn validate_host_fact(name: &str, value: &str) -> Result<(), DoctorError> {
    if value.is_empty() || value.chars().any(char::is_control) {
        Err(DoctorError::new(format!(
            "doctor host returned an invalid {name} fact"
        )))
    } else {
        Ok(())
    }
}

fn check_clang(host: &dyn DoctorHost) -> Check {
    let path = match host.resolve_tool("clang") {
        Ok(path) => path,
        Err(error) => return Check::failed("clang", error.to_string()),
    };
    if !path.is_absolute() {
        return Check::failed("clang", "resolved Clang path is not absolute");
    }
    match normalized_version(host.run_version(&path)) {
        Ok(version) => Check::ok("clang", format!("{} ({version})", path.display())),
        Err(error) => Check::failed("clang", error.to_string()),
    }
}

fn check_node(host: &dyn DoctorHost) -> Check {
    let path = match host.resolve_tool("node") {
        Ok(path) => path,
        Err(error) => return Check::failed("node", error.to_string()),
    };
    if !path.is_absolute() {
        return Check::failed("node", "resolved Node path is not absolute");
    }
    let version = match normalized_version(host.run_version(&path)) {
        Ok(version) => version,
        Err(error) => return Check::failed("node", error.to_string()),
    };
    match node_major(&version) {
        Some(major) if major >= MIN_NODE_MAJOR => Check::ok("node", version),
        Some(major) => Check::failed(
            "node",
            format!("{version} (requires major version {MIN_NODE_MAJOR} or newer; found {major})"),
        ),
        None => Check::failed("node", format!("unrecognized Node version `{version}`")),
    }
}

fn check_rust(host: &dyn DoctorHost) -> Check {
    let path = match host.resolve_tool("rustc") {
        Ok(path) => path,
        Err(error) => return Check::failed("rust", error.to_string()),
    };
    if !path.is_absolute() {
        return Check::failed("rust", "resolved Rust path is not absolute");
    }
    let version = match normalized_version(host.run_version(&path)) {
        Ok(version) => version,
        Err(error) => return Check::failed("rust", error.to_string()),
    };
    match rust_version(&version) {
        Some((major, minor))
            if major > MIN_RUST_MAJOR
                || (major == MIN_RUST_MAJOR && minor >= MIN_RUST_MINOR) =>
        {
            Check::ok("rust", version)
        }
        Some((major, minor)) => Check::failed(
            "rust",
            format!(
                "{version} (requires Rust {MIN_RUST_MAJOR}.{MIN_RUST_MINOR} or newer; found {major}.{minor})"
            ),
        ),
        None => Check::failed("rust", format!("unrecognized Rust version `{version}`")),
    }
}

fn normalized_version(version: Result<String, DoctorError>) -> Result<String, DoctorError> {
    let version = version?;
    let first_line = version.lines().next().unwrap_or("").trim();
    if first_line.is_empty() || first_line.chars().any(char::is_control) {
        Err(DoctorError::new("tool returned an invalid version string"))
    } else {
        Ok(first_line.to_owned())
    }
}

fn node_major(version: &str) -> Option<u64> {
    version_token::parse(version.strip_prefix('v').unwrap_or(version)).map(|value| value.0)
}

fn rust_version(version: &str) -> Option<(u64, u64)> {
    let token = version.strip_prefix("rustc ")?.split_whitespace().next()?;
    version_token::parse(token).map(|(major, minor, _)| (major, minor))
}

fn render_human(target: DoctorTarget, checks: &[Check]) -> String {
    let mut output = format!("semaprax doctor ({})\n", target.text());
    for check in checks {
        let status = if check.passed { "ok" } else { "failed" };
        output.push_str(&format!(
            "{status} {}: {}\n",
            check.id,
            escape_human(&check.detail)
        ));
    }
    output
}

fn render_json(target: DoctorTarget, checks: &[Check]) -> String {
    let checks = checks
        .iter()
        .map(|check| {
            format!(
                "{{\"id\":{},\"required\":{},\"status\":{},\"detail\":{}}}",
                quote_json(check.id),
                check.required,
                quote_json(if check.passed { "ok" } else { "failed" }),
                quote_json(&check.detail),
            )
        })
        .collect::<Vec<_>>()
        .join(",");
    format!(
        "{{\"schema\":{},\"target\":{},\"checks\":[{checks}]}}\n",
        quote_json(SCHEMA),
        quote_json(target.text()),
    )
}

fn escape_human(value: &str) -> String {
    value.chars().flat_map(char::escape_default).collect()
}

struct RealDoctorHost;

impl DoctorHost for RealDoctorHost {
    fn os(&self) -> &str {
        std::env::consts::OS
    }

    fn arch(&self) -> &str {
        std::env::consts::ARCH
    }

    fn resolve_tool(&self, name: &str) -> Result<PathBuf, DoctorError> {
        let path = std::env::var_os("PATH")
            .ok_or_else(|| DoctorError::new(format!("tool `{name}` was not found on PATH")))?;
        for directory in std::env::split_paths(&path) {
            for executable in executable_names(name) {
                let candidate = directory.join(executable);
                // Preserve the invoked basename: multicall tools (including rustup)
                // select their operation from argv[0], not the resolved inode name.
                let Ok(candidate) = std::path::absolute(candidate) else {
                    continue;
                };
                if candidate.is_absolute() && executable_file(&candidate) {
                    return Ok(candidate);
                }
            }
        }
        Err(DoctorError::new(format!(
            "tool `{name}` was not found on PATH"
        )))
    }

    fn run_version(&self, path: &Path) -> Result<String, DoctorError> {
        if !path.is_absolute() {
            return Err(DoctorError::new("tool path is not absolute"));
        }
        let output = version_probe(path)?;
        String::from_utf8(output)
            .map_err(|_| DoctorError::new(format!("{} returned non-UTF-8 output", path.display())))
    }
}

#[cfg(any(target_os = "linux", target_os = "macos", target_os = "windows"))]
fn version_probe(path: &Path) -> Result<Vec<u8>, DoctorError> {
    use semaprax_native_rust_interop_platform::{doctor_version_probe, DoctorProbeError};
    doctor_version_probe(path).map_err(|error| {
        let reason = match error {
            DoctorProbeError::Invalid => "has an invalid probe path or environment",
            DoctorProbeError::Unsupported => "cannot be probed on this host",
            DoctorProbeError::Spawn => "could not be started",
            DoctorProbeError::Exit => "exited unsuccessfully",
            DoctorProbeError::OutputLimit => "exceeded the 65536-byte output limit",
            DoctorProbeError::Timeout => "exceeded the 10-second execution deadline",
            DoctorProbeError::Io => "failed during bounded output collection",
        };
        DoctorError::new(format!("{} --version {reason}", path.display()))
    })
}

#[cfg(not(any(target_os = "linux", target_os = "macos", target_os = "windows")))]
fn version_probe(_: &Path) -> Result<Vec<u8>, DoctorError> {
    Err(DoctorError::new(
        "bounded version probes are unsupported on this host",
    ))
}

fn executable_file(path: &Path) -> bool {
    let Ok(metadata) = std::fs::metadata(path) else {
        return false;
    };
    if !metadata.is_file() {
        return false;
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        // This is a PATH candidate filter, not an effective-user access proof.
        // Execution still owns ACL, mount, and identity-related failure reporting.
        metadata.permissions().mode() & 0o111 != 0
    }
    #[cfg(not(unix))]
    {
        true
    }
}

#[cfg(windows)]
fn executable_names(name: &str) -> Vec<String> {
    vec![format!("{name}.exe"), name.to_owned()]
}

#[cfg(not(windows))]
fn executable_names(name: &str) -> Vec<String> {
    vec![name.to_owned()]
}
