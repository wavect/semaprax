//! Doctor: offline tool-profile admission and version policy, shared by the
//! standalone and full-toolchain binaries. Selection is not authority: the
//! real CLI reports unavailable required checks until a platform backend
//! admits the offline closure, and this module never spawns a process itself.
//! The settled-observation renderer that consumes the private platform
//! crate's types stays in `crates/semaprax-toolchain`.
use std::fmt;
use std::path::{Path, PathBuf};

use semaprax::diagnostic::quote_json;

#[path = "doctor/offline_profile.rs"]
mod offline_profile;
#[path = "doctor/version_token.rs"]
mod version_token;

#[cfg(test)]
// Used by the path-included integration harness, not the library test target.
#[allow(unused_imports)]
pub use offline_profile::{inspect_profile, AdmittedProfile};
pub use offline_profile::{run_with_profile_host, OfflineProfileHost};

const SCHEMA: &str = "semaprax.doctor.v1";
const VERSION: &str = env!("CARGO_PKG_VERSION");
const MIN_NODE_MAJOR: u64 = 22;
const MIN_RUST_MAJOR: u64 = 1;
const MIN_RUST_MINOR: u64 = 88;

pub trait DoctorHost {
    fn os(&self) -> &str;
    fn arch(&self) -> &str;
    fn resolve_tool(&self, name: &str) -> Result<PathBuf, DoctorError>;
    fn run_version(&self, path: &Path) -> Result<String, DoctorError>;
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DoctorError {
    message: String,
}

impl DoctorError {
    pub fn new(message: impl Into<String>) -> Self {
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
pub enum DoctorTarget {
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
    profile: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Check {
    id: &'static str,
    required: bool,
    passed: bool,
    detail: String,
}

impl Check {
    pub fn ok(id: &'static str, detail: impl Into<String>) -> Self {
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

pub struct DoctorOutcome {
    pub output: String,
    pub exit_code: u8,
}

pub fn run(arguments: &[String]) -> Result<DoctorOutcome, DoctorError> {
    let host: &dyn OfflineProfileHost = &offline_profile::RealOfflineProfileHost;
    run_with_profile_host(arguments, host)
}

#[cfg(test)]
// Preserves the legacy injected-harness view; production uses explicit profiles.
#[allow(dead_code)]
pub fn inspect(
    host: &dyn DoctorHost,
    target: DoctorTarget,
    json: bool,
    release_build: bool,
) -> Result<DoctorOutcome, DoctorError> {
    let mut checks = base_checks(host.os(), host.arch(), release_build)?;
    append_tool_checks(&mut checks, host, target);
    Ok(report(target, json, &checks))
}

fn base_checks(os: &str, arch: &str, release_build: bool) -> Result<Vec<Check>, DoctorError> {
    validate_host_fact("operating system", os)?;
    validate_host_fact("architecture", arch)?;
    Ok(platform_checks(os, arch, release_build))
}

// Callers either validate free-form host facts or supply closed platform enums.
pub fn platform_checks(os: &str, arch: &str, release_build: bool) -> Vec<Check> {
    vec![
        Check::ok("semaprax", VERSION),
        Check::ok("os", os),
        Check::ok("arch", arch),
        Check::ok("release", if release_build { "release" } else { "debug" }),
    ]
}

fn append_tool_checks(checks: &mut Vec<Check>, host: &dyn DoctorHost, target: DoctorTarget) {
    if matches!(target, DoctorTarget::Native | DoctorTarget::All) {
        checks.push(check_clang(host));
    }
    if matches!(target, DoctorTarget::Web | DoctorTarget::All) {
        checks.push(check_node(host));
    }
    if matches!(target, DoctorTarget::Contributor | DoctorTarget::All) {
        checks.push(check_rust(host));
    }
}

pub fn report(target: DoctorTarget, json: bool, checks: &[Check]) -> DoctorOutcome {
    let passed = checks.iter().all(|check| !check.required || check.passed);
    let output = if json {
        render_json(target, checks)
    } else {
        render_human(target, checks)
    };
    DoctorOutcome {
        output,
        exit_code: if passed { 0 } else { 1 },
    }
}

fn parse(arguments: &[String]) -> Result<DoctorOptions, DoctorError> {
    let mut target = DoctorTarget::Contributor;
    let mut target_seen = false;
    let mut json = false;
    let mut profile = None;
    let mut index = 0usize;
    while index < arguments.len() {
        match arguments[index].as_str() {
            "--profile" => {
                if profile.is_some() {
                    return Err(DoctorError::new("duplicate doctor option `--profile`"));
                }
                let value = arguments.get(index + 1).ok_or_else(|| {
                    DoctorError::new(
                        "doctor option `--profile` requires an offline profile identifier",
                    )
                })?;
                offline_profile::validate_selector(value)?;
                profile = Some(value.clone());
                index += 2;
            }
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
    Ok(DoctorOptions {
        target,
        json,
        profile,
    })
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
    clang_version(&path.display().to_string(), host.run_version(&path))
}

pub fn clang_version(path: &str, output: Result<String, DoctorError>) -> Check {
    match normalized_version(output) {
        Ok(version) => Check::ok("clang", format!("{path} ({version})")),
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
    node_version(host.run_version(&path))
}

pub fn node_version(output: Result<String, DoctorError>) -> Check {
    let version = match normalized_version(output) {
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
    rust_version_check(host.run_version(&path))
}

pub fn rust_version_check(output: Result<String, DoctorError>) -> Check {
    let version = match normalized_version(output) {
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
