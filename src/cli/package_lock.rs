//! Explicit-input, stdout-only Offline Package Lock v1 CLI boundary.

use std::io::Read as _;
use std::path::PathBuf;

use semaprax::diagnostic::Diagnostic;
use semaprax::package_lock::{self, PackageLockOptions};

pub(crate) enum PackageLockCliError {
    Usage(String),
    Domain(Vec<Diagnostic>),
}

pub(crate) fn run(arguments: &[String]) -> Result<String, PackageLockCliError> {
    let (paths, options) = parse(arguments)?;
    let subjects =
        read_subjects(&paths).map_err(|error| PackageLockCliError::Domain(vec![error]))?;
    package_lock::generate(&subjects, &options).map_err(PackageLockCliError::Domain)
}

fn parse(arguments: &[String]) -> Result<(Vec<PathBuf>, PackageLockOptions), PackageLockCliError> {
    let mut paths = Vec::new();
    let mut max_bytes = PackageLockOptions::default().max_bytes();
    let mut seen_max_bytes = false;
    let mut index = 0usize;
    while index < arguments.len() {
        match arguments[index].as_str() {
            "--max-bytes" => {
                if seen_max_bytes {
                    return Err(usage("duplicate package-lock option `--max-bytes`"));
                }
                seen_max_bytes = true;
                let value = arguments
                    .get(index + 1)
                    .ok_or_else(|| usage("package-lock option `--max-bytes` requires a value"))?;
                max_bytes = canonical_number("--max-bytes", value)?;
                index += 2;
                if index != arguments.len() {
                    return Err(usage("package-lock subjects must precede `--max-bytes`"));
                }
            }
            option if option.starts_with('-') => {
                return Err(usage(format!("unknown package-lock option `{option}`")))
            }
            path => {
                paths.push(PathBuf::from(path));
                index += 1;
            }
        }
    }
    if !(1..=package_lock::MAX_PACKAGES).contains(&paths.len()) {
        return Err(usage(format!(
            "package-lock requires 1..{} explicit subject files",
            package_lock::MAX_PACKAGES
        )));
    }
    let options = PackageLockOptions::new(max_bytes).map_err(|error| usage(error.to_string()))?;
    Ok((paths, options))
}

fn canonical_number(option: &str, value: &str) -> Result<usize, PackageLockCliError> {
    if value.is_empty()
        || !value.bytes().all(|byte| byte.is_ascii_digit())
        || (value.len() > 1 && value.starts_with('0'))
    {
        return Err(usage(format!(
            "package-lock option `{option}` requires a canonical positive integer"
        )));
    }
    value.parse::<usize>().map_err(|_| {
        usage(format!(
            "package-lock option `{option}` requires a canonical positive integer"
        ))
    })
}

fn read_subjects(paths: &[PathBuf]) -> Result<Vec<String>, Diagnostic> {
    struct HeldInput {
        file: std::fs::File,
        bytes: usize,
    }

    let mut held = Vec::with_capacity(paths.len());
    let mut identities = std::collections::BTreeSet::new();
    let mut total_bytes = 0usize;
    for path in paths {
        let file = std::fs::File::open(path).map_err(|_| {
            Diagnostic::io(
                "SPX-I215",
                "cannot open package-lock subject input".to_owned(),
            )
        })?;
        let metadata = file.metadata().map_err(|_| {
            Diagnostic::io(
                "SPX-I215",
                "cannot inspect held package-lock subject input".to_owned(),
            )
        })?;
        if !metadata.is_file() {
            return Err(Diagnostic::io(
                "SPX-I215",
                "package-lock subject input must be a regular file".to_owned(),
            ));
        }
        let identity = held_input_identity(&metadata)?;
        if !identities.insert(identity) {
            return Err(Diagnostic::io(
                "SPX-I215",
                "package-lock subject inputs must have distinct held file identities".to_owned(),
            ));
        }
        let bytes = usize::try_from(metadata.len()).map_err(|_| {
            Diagnostic::io(
                "SPX-L406",
                "package-lock subject byte count does not fit the host size".to_owned(),
            )
        })?;
        if bytes > package_lock::MAX_SUBJECT_BYTES {
            return Err(subject_limit());
        }
        total_bytes = total_bytes.checked_add(bytes).ok_or_else(|| {
            Diagnostic::io(
                "SPX-L406",
                "package-lock total_subject_bytes overflow".to_owned(),
            )
        })?;
        if total_bytes > package_lock::MAX_TOTAL_SUBJECT_BYTES {
            return Err(Diagnostic::io(
                "SPX-L406",
                format!(
                    "package-lock total_subject_bytes exceeds {}",
                    package_lock::MAX_TOTAL_SUBJECT_BYTES
                ),
            ));
        }
        held.push(HeldInput { file, bytes });
    }

    let mut subjects = Vec::with_capacity(held.len());
    for HeldInput { file, bytes } in held {
        let mut content = Vec::with_capacity(bytes);
        file.take((package_lock::MAX_SUBJECT_BYTES + 1) as u64)
            .read_to_end(&mut content)
            .map_err(|_| {
                Diagnostic::io(
                    "SPX-I215",
                    "cannot read held package-lock subject input".to_owned(),
                )
            })?;
        if content.len() > package_lock::MAX_SUBJECT_BYTES {
            return Err(subject_limit());
        }
        if content.len() != bytes {
            return Err(Diagnostic::io(
                "SPX-I215",
                "held package-lock subject input changed during its single read".to_owned(),
            ));
        }
        subjects.push(String::from_utf8(content).map_err(|_| {
            Diagnostic::io(
                "SPX-I215",
                "package-lock subject input must be valid UTF-8".to_owned(),
            )
        })?);
    }
    Ok(subjects)
}

fn subject_limit() -> Diagnostic {
    Diagnostic::io(
        "SPX-L406",
        format!(
            "package-lock subject_bytes exceeds {}",
            package_lock::MAX_SUBJECT_BYTES
        ),
    )
}

#[cfg(unix)]
fn held_input_identity(metadata: &std::fs::Metadata) -> Result<(u64, u64), Diagnostic> {
    use std::os::unix::fs::MetadataExt as _;
    Ok((metadata.dev(), metadata.ino()))
}

#[cfg(windows)]
fn held_input_identity(metadata: &std::fs::Metadata) -> Result<(u64, u64), Diagnostic> {
    use std::os::windows::fs::MetadataExt as _;
    let volume = metadata.volume_serial_number().ok_or_else(|| {
        Diagnostic::io(
            "SPX-I215",
            "held package-lock subject volume identity is unavailable".to_owned(),
        )
    })?;
    let index = metadata.file_index().ok_or_else(|| {
        Diagnostic::io(
            "SPX-I215",
            "held package-lock subject file identity is unavailable".to_owned(),
        )
    })?;
    Ok((u64::from(volume), index))
}

#[cfg(not(any(unix, windows)))]
fn held_input_identity(_metadata: &std::fs::Metadata) -> Result<(u64, u64), Diagnostic> {
    Err(Diagnostic::io(
        "SPX-I215",
        "held package-lock subject identity is unsupported on this host".to_owned(),
    ))
}

fn usage(message: impl Into<String>) -> PackageLockCliError {
    PackageLockCliError::Usage(message.into())
}
