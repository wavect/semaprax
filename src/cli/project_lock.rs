//! `semaprax lock <manifest> [--write|--verify|--compare <baseline.lock>]`:
//! render, write, verify, or compatibility-compare the deterministic
//! `semaprax.lock` beside a project. Rendering, verification, and comparison
//! are pure library functions; this boundary owns the only filesystem effects,
//! and each is explicit — the lock is never touched by `check`.

use std::path::{Path, PathBuf};

use semaprax::diagnostic::Diagnostic;
use semaprax::project::{self, ProjectSnapshot, MAX_PROJECT_LOCK_BYTES, PROJECT_LOCK_FILE};

pub(crate) enum ProjectLockCliError {
    Usage(String),
    Domain(Vec<Diagnostic>),
    /// A successful comparison whose verdict is breaking: the report is printed
    /// to stdout and the command exits nonzero so a CI gate fails.
    Breaking(String),
}

enum Mode {
    Print,
    Write,
    Verify,
    Compare(PathBuf),
    EmitInterface,
    CompareInterface(PathBuf),
}

const CODE_WRITE: &str = "SPX-J125";
const CODE_MISSING: &str = "SPX-J124";

/// Run the `lock` command and return the text to print on stdout.
pub(crate) fn run(arguments: &[String]) -> Result<String, ProjectLockCliError> {
    let (manifest, mode) = parse(arguments)?;
    let outcome = project::with_authenticated_project(&manifest, |snapshot| {
        snapshot.check()?;
        match &mode {
            Mode::Print => Ok(Outcome::Text(project::render_project_lock(snapshot)?)),
            Mode::Write => Ok(Outcome::Text(write_mode(snapshot)?)),
            Mode::Verify => Ok(Outcome::Text(verify_mode(snapshot)?)),
            Mode::Compare(baseline) => compare_mode(snapshot, baseline),
            Mode::EmitInterface => Ok(Outcome::Text(emit_interface_mode(snapshot)?)),
            Mode::CompareInterface(baseline) => compare_interface_mode(snapshot, baseline),
        }
    })
    .map_err(|errors| {
        ProjectLockCliError::Domain(super::manifest_hint::hint_missing_manifest(
            errors, &manifest,
        ))
    })?;
    match outcome {
        Outcome::Text(text) => Ok(text),
        Outcome::Breaking(report) => Err(ProjectLockCliError::Breaking(report)),
    }
}

enum Outcome {
    Text(String),
    Breaking(String),
}

fn compare_mode(snapshot: &ProjectSnapshot, baseline: &Path) -> Result<Outcome, Vec<Diagnostic>> {
    let candidate = project::render_project_lock(snapshot)?;
    let base = read_lock(baseline)?;
    let compatibility = project::classify_lock_change(&base, &candidate)?;
    Ok(if compatibility.breaking() {
        Outcome::Breaking(compatibility.report().to_owned())
    } else {
        Outcome::Text(compatibility.report().to_owned())
    })
}

/// Emit the scalar WIT interface descriptor of a Project v1 project, so it can
/// be stored as a baseline for `--compare-interface`. Non-scalar profiles have
/// no scalar WIT interface and return the existing domain diagnostic.
fn emit_interface_mode(snapshot: &ProjectSnapshot) -> Result<String, Vec<Diagnostic>> {
    let descriptor = snapshot.scalar_wit_interface_v1()?.canonical_bytes();
    String::from_utf8(descriptor).map_err(|_| {
        vec![Diagnostic::io(
            CODE_MISSING,
            "scalar interface descriptor is not UTF-8",
        )]
    })
}

fn compare_interface_mode(
    snapshot: &ProjectSnapshot,
    baseline: &Path,
) -> Result<Outcome, Vec<Diagnostic>> {
    let candidate = emit_interface_mode(snapshot)?;
    let base = read_descriptor(baseline)?;
    let compatibility = project::classify_scalar_wit_change(&base, &candidate)?;
    Ok(if compatibility.breaking() {
        Outcome::Breaking(compatibility.report().to_owned())
    } else {
        Outcome::Text(compatibility.report().to_owned())
    })
}

fn write_mode(snapshot: &ProjectSnapshot) -> Result<String, Vec<Diagnostic>> {
    let lock = project::render_project_lock(snapshot)?;
    let path = snapshot.root().join(PROJECT_LOCK_FILE);
    write_lock(&path, &lock)?;
    let verified = project::verify_project_lock(snapshot, &lock)?;
    Ok(format!(
        "wrote {PROJECT_LOCK_FILE} for {} ({})\n",
        snapshot.manifest().name(),
        verified.digest()
    ))
}

fn verify_mode(snapshot: &ProjectSnapshot) -> Result<String, Vec<Diagnostic>> {
    let path = snapshot.root().join(PROJECT_LOCK_FILE);
    let lock = read_lock(&path)?;
    let verified = project::verify_project_lock(snapshot, &lock)?;
    Ok(format!(
        "verified {PROJECT_LOCK_FILE} for {} ({})\n",
        snapshot.manifest().name(),
        verified.digest()
    ))
}

fn parse(arguments: &[String]) -> Result<(PathBuf, Mode), ProjectLockCliError> {
    let mut manifest = None;
    let mut mode: Option<Mode> = None;
    let mut index = 0usize;
    while index < arguments.len() {
        let next = match arguments[index].as_str() {
            "--write" => Mode::Write,
            "--verify" => Mode::Verify,
            "--compare" => {
                let baseline = arguments.get(index + 1).ok_or_else(|| {
                    usage("lock option `--compare` requires a baseline lock path")
                })?;
                index += 1;
                Mode::Compare(PathBuf::from(baseline))
            }
            "--emit-interface" => Mode::EmitInterface,
            "--compare-interface" => {
                let baseline = arguments.get(index + 1).ok_or_else(|| {
                    usage("lock option `--compare-interface` requires a baseline descriptor path")
                })?;
                index += 1;
                Mode::CompareInterface(PathBuf::from(baseline))
            }
            option if option.starts_with('-') => {
                return Err(usage(format!("unknown lock option `{option}`")))
            }
            path if manifest.is_none() => {
                manifest = Some(PathBuf::from(path));
                index += 1;
                continue;
            }
            _ => return Err(usage("lock accepts at most one manifest path")),
        };
        if mode.is_some() {
            return Err(usage(
                "lock accepts at most one of `--write`, `--verify`, `--compare`, `--emit-interface`, or `--compare-interface`",
            ));
        }
        mode = Some(next);
        index += 1;
    }
    let manifest = manifest
        .map(super::project::resolve_positional)
        .unwrap_or_else(|| PathBuf::from(super::project::DEFAULT_MANIFEST));
    Ok((manifest, mode.unwrap_or(Mode::Print)))
}

fn read_lock(path: &Path) -> Result<String, Vec<Diagnostic>> {
    let subject = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or(PROJECT_LOCK_FILE);
    let metadata = std::fs::symlink_metadata(path).map_err(|error| {
        vec![Diagnostic::io(
            CODE_MISSING,
            format!("{subject} is not present: {error}"),
        )]
    })?;
    if !metadata.is_file() || metadata.len() > MAX_PROJECT_LOCK_BYTES as u64 {
        return Err(vec![Diagnostic::io(
            CODE_MISSING,
            format!("{subject} must be a plain file of at most {MAX_PROJECT_LOCK_BYTES} bytes"),
        )]);
    }
    std::fs::read_to_string(path).map_err(|error| {
        vec![Diagnostic::io(
            CODE_MISSING,
            format!("{subject} is not readable UTF-8: {error}"),
        )]
    })
}

fn read_descriptor(path: &Path) -> Result<String, Vec<Diagnostic>> {
    let subject = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("scalar interface descriptor");
    let metadata = std::fs::symlink_metadata(path).map_err(|error| {
        vec![Diagnostic::io(
            CODE_MISSING,
            format!("{subject} is not present: {error}"),
        )]
    })?;
    if !metadata.is_file()
        || metadata.len() > semaprax::project::MAX_SCALAR_WIT_DESCRIPTOR_BYTES as u64
    {
        return Err(vec![Diagnostic::io(
            CODE_MISSING,
            format!(
                "{subject} must be a plain file of at most {} bytes",
                semaprax::project::MAX_SCALAR_WIT_DESCRIPTOR_BYTES
            ),
        )]);
    }
    std::fs::read_to_string(path).map_err(|error| {
        vec![Diagnostic::io(
            CODE_MISSING,
            format!("{subject} is not readable UTF-8: {error}"),
        )]
    })
}

fn write_lock(path: &Path, lock: &str) -> Result<(), Vec<Diagnostic>> {
    let staged = path.with_file_name(format!(
        "{PROJECT_LOCK_FILE}.{}.staging",
        std::process::id()
    ));
    let write = std::fs::write(&staged, lock).and_then(|()| std::fs::rename(&staged, path));
    if let Err(error) = write {
        let _ = std::fs::remove_file(&staged);
        return Err(vec![Diagnostic::io(
            CODE_WRITE,
            format!("{PROJECT_LOCK_FILE} could not be written: {error}"),
        )]);
    }
    Ok(())
}

fn usage(message: impl Into<String>) -> ProjectLockCliError {
    ProjectLockCliError::Usage(format!(
        "{}\nhint: run `semaprax lock --help` for usage",
        message.into()
    ))
}
