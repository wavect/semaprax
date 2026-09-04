//! `semaprax lock <manifest> [--write|--verify]`: render, write, or verify the
//! deterministic `semaprax.lock` beside a project. Rendering and verification
//! are pure library functions; this boundary owns the only filesystem effects,
//! and each is explicit — the lock is never touched by `check`.

use std::path::{Path, PathBuf};

use semaprax::diagnostic::Diagnostic;
use semaprax::project::{self, ProjectSnapshot, MAX_PROJECT_LOCK_BYTES, PROJECT_LOCK_FILE};

pub(crate) enum ProjectLockCliError {
    Usage(String),
    Domain(Vec<Diagnostic>),
}

enum Mode {
    Print,
    Write,
    Verify,
}

const CODE_WRITE: &str = "SPX-J125";
const CODE_MISSING: &str = "SPX-J124";

/// Run the `lock` command and return the text to print on stdout.
pub(crate) fn run(arguments: &[String]) -> Result<String, ProjectLockCliError> {
    let (manifest, mode) = parse(arguments)?;
    project::with_authenticated_project(&manifest, |snapshot| {
        snapshot.check()?;
        match mode {
            Mode::Print => project::render_project_lock(snapshot),
            Mode::Write => write_mode(snapshot),
            Mode::Verify => verify_mode(snapshot),
        }
    })
    .map_err(ProjectLockCliError::Domain)
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
    let mut mode = None;
    for argument in arguments {
        let next = match argument.as_str() {
            "--write" => Mode::Write,
            "--verify" => Mode::Verify,
            option if option.starts_with('-') => {
                return Err(usage(format!("unknown lock option `{option}`")))
            }
            path if manifest.is_none() => {
                manifest = Some(PathBuf::from(path));
                continue;
            }
            _ => return Err(usage("lock accepts exactly one manifest path")),
        };
        if mode.is_some() {
            return Err(usage("lock accepts at most one of `--write` or `--verify`"));
        }
        mode = Some(next);
    }
    let manifest = manifest.ok_or_else(|| usage("lock requires a manifest path"))?;
    Ok((manifest, mode.unwrap_or(Mode::Print)))
}

fn read_lock(path: &Path) -> Result<String, Vec<Diagnostic>> {
    let metadata = std::fs::symlink_metadata(path).map_err(|error| {
        vec![Diagnostic::io(
            CODE_MISSING,
            format!("{PROJECT_LOCK_FILE} is not present beside the manifest: {error}"),
        )]
    })?;
    if !metadata.is_file() || metadata.len() > MAX_PROJECT_LOCK_BYTES as u64 {
        return Err(vec![Diagnostic::io(
            CODE_MISSING,
            format!(
                "{PROJECT_LOCK_FILE} must be a plain file of at most {MAX_PROJECT_LOCK_BYTES} bytes"
            ),
        )]);
    }
    std::fs::read_to_string(path).map_err(|error| {
        vec![Diagnostic::io(
            CODE_MISSING,
            format!("{PROJECT_LOCK_FILE} is not readable UTF-8: {error}"),
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
