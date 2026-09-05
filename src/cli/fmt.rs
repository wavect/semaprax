//! The `fmt` invocation grammar and its run.
//!
//! `semaprax fmt <file>|<dir>|semaprax.toml [--check]`. A single `.spx` file
//! is formatted on its own; a project directory or manifest formats every
//! source file the manifest lists, in manifest order, through the same
//! comment-preserving projection. Every file is parsed before any file is
//! written, so a parse error in one source leaves the whole project as it was.

use std::path::{Path, PathBuf};

use semaprax::diagnostic::Diagnostic;
use semaprax::format;
use semaprax::project::ProjectManifest;

use super::manifest_hint::MISSING_MANIFEST_HELP;
use super::project::{is_project_manifest, resolve_positional};

#[derive(Debug, Eq, PartialEq)]
pub(crate) enum FmtInput {
    Source(PathBuf),
    Project(PathBuf),
}

#[derive(Debug, Eq, PartialEq)]
pub(crate) struct FmtOptions {
    pub(crate) input: FmtInput,
    pub(crate) check: bool,
}

pub(crate) fn parse(args: &[String]) -> Result<FmtOptions, u8> {
    let (path, check) = match args {
        [path] if !path.starts_with('-') => (path, false),
        [path, option] if !path.starts_with('-') && option == "--check" => (path, true),
        [option, path] if option == "--check" && !path.starts_with('-') => (path, true),
        [option] if option == "--check" => {
            eprintln!("fmt --check requires <file>|<dir>|semaprax.toml");
            return Err(2);
        }
        [option, ..] if option.starts_with('-') => {
            eprintln!("unknown fmt option `{option}`");
            return Err(2);
        }
        _ => {
            eprintln!("fmt requires exactly <file>|<dir>|semaprax.toml [--check]");
            return Err(2);
        }
    };
    let input = match resolve_positional(PathBuf::from(path)) {
        path if is_project_manifest(&path) => FmtInput::Project(path),
        path => FmtInput::Source(path),
    };
    Ok(FmtOptions { input, check })
}

/// One source file with its canonical, comment-preserving projection.
struct Formatted {
    path: PathBuf,
    source: String,
    canonical: String,
}

/// Format or check every selected file. Diagnostics are reported through
/// `report`, which returns the exit status for a failed run.
pub(crate) fn run(options: FmtOptions, report: impl Fn(&[Diagnostic]) -> u8) -> Result<(), u8> {
    let paths = match options.input {
        FmtInput::Source(path) => vec![path],
        FmtInput::Project(manifest_path) => project_sources(&manifest_path, &report)?,
    };
    let mut formatted = Vec::with_capacity(paths.len());
    for path in paths {
        reject_symlink_components(&path).map_err(|diagnostic| report(&[diagnostic]))?;
        let source = std::fs::read_to_string(&path).map_err(|error| {
            report(&[Diagnostic::io(
                "SPX-I001",
                format!("cannot read {}: {error}", path.display()),
            )
            .at_path(path.display().to_string())])
        })?;
        let (program, comments) =
            semaprax::parse_with_comments(&source, &path).map_err(|error| report(&[error]))?;
        let canonical = format::comments::canonical_with_comments(&program, &comments);
        formatted.push(Formatted {
            path,
            source,
            canonical,
        });
    }
    if options.check {
        let mut drifted = false;
        for file in &formatted {
            if file.source != file.canonical {
                eprintln!(
                    "{}:{} is not canonically formatted",
                    file.path.display(),
                    first_differing_line(&file.source, &file.canonical)
                );
                drifted = true;
            }
        }
        return if drifted { Err(1) } else { Ok(()) };
    }
    for file in &formatted {
        if file.source != file.canonical {
            std::fs::write(&file.path, &file.canonical).map_err(|error| {
                eprintln!("cannot write {}: {error}", file.path.display());
                1
            })?;
        }
    }
    Ok(())
}

/// The one-based line containing the first byte that differs. Comparing bytes
/// is intentional: it also identifies newline-only drift at end of file.
fn first_differing_line(source: &str, canonical: &str) -> usize {
    let prefix = source
        .bytes()
        .zip(canonical.bytes())
        .take_while(|(source, canonical)| source == canonical)
        .count();
    source.as_bytes()[..prefix]
        .iter()
        .filter(|byte| **byte == b'\n')
        .count()
        + 1
}

/// The manifest's source files, in manifest order, relative to its directory.
/// The manifest parser has already required canonical relative `.spx` paths.
fn project_sources(
    manifest_path: &Path,
    report: &impl Fn(&[Diagnostic]) -> u8,
) -> Result<Vec<PathBuf>, u8> {
    reject_symlink_components(manifest_path).map_err(|diagnostic| report(&[diagnostic]))?;
    let manifest_source = std::fs::read_to_string(manifest_path).map_err(|error| {
        report(&[Diagnostic::io(
            "SPX-J102",
            format!(
                "cannot read Project v1 manifest {}: {error}",
                manifest_path.display()
            ),
        )
        .at_path(manifest_path.display().to_string())
        .with_help(MISSING_MANIFEST_HELP)])
    })?;
    let manifest = ProjectManifest::parse(&manifest_source).map_err(|errors| report(&errors))?;
    let root = manifest_path.parent().unwrap_or_else(|| Path::new(""));
    Ok(manifest
        .sources()
        .iter()
        .map(|relative| root.join(relative))
        .collect())
}

/// Formatting is a write-capable Project command, so it rejects the same
/// symlink/reparse aliases as authenticated Project selection. Missing paths
/// are left to the existing read diagnostic.
fn reject_symlink_components(path: &Path) -> Result<(), Diagnostic> {
    let absolute = if path.is_absolute() {
        path.to_path_buf()
    } else {
        std::env::current_dir()
            .map_err(|error| {
                Diagnostic::io(
                    "SPX-J102",
                    format!("cannot inspect fmt input {}: {error}", path.display()),
                )
            })?
            .join(path)
    };
    for component in absolute.ancestors() {
        let Ok(metadata) = std::fs::symlink_metadata(component) else {
            continue;
        };
        if metadata.file_type().is_symlink() || metadata_is_reparse(&metadata) {
            return Err(Diagnostic::io(
                "SPX-J102",
                format!(
                    "fmt input {} traverses a symlink or reparse point at {}",
                    path.display(),
                    component.display()
                ),
            ));
        }
    }
    Ok(())
}

#[cfg(windows)]
fn metadata_is_reparse(metadata: &std::fs::Metadata) -> bool {
    use std::os::windows::fs::MetadataExt;
    metadata.file_attributes() & 0x400 != 0
}

#[cfg(not(windows))]
fn metadata_is_reparse(_: &std::fs::Metadata) -> bool {
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    fn strings(values: &[&str]) -> Vec<String> {
        values.iter().map(|value| (*value).to_owned()).collect()
    }

    #[test]
    fn formatter_grammar_is_closed() {
        assert_eq!(
            parse(&strings(&["source.spx"])).unwrap(),
            FmtOptions {
                input: FmtInput::Source(PathBuf::from("source.spx")),
                check: false,
            }
        );
        assert_eq!(
            parse(&strings(&["source.spx", "--check"])).unwrap(),
            FmtOptions {
                input: FmtInput::Source(PathBuf::from("source.spx")),
                check: true,
            }
        );
        assert_eq!(
            parse(&strings(&["--check", "source.spx"])).unwrap(),
            FmtOptions {
                input: FmtInput::Source(PathBuf::from("source.spx")),
                check: true,
            }
        );
        assert_eq!(
            parse(&strings(&["fixtures/semaprax.toml", "--check"])).unwrap(),
            FmtOptions {
                input: FmtInput::Project(super::project::normalize_project_path(PathBuf::from(
                    "fixtures/semaprax.toml",
                )),),
                check: true,
            }
        );
        for malformed in [
            &[][..],
            &["--check"][..],
            &["--unknown"][..],
            &["source.spx", "extra"][..],
            &["source.spx", "--unknown"][..],
            &["source.spx", "--check", "--check"][..],
        ] {
            assert!(parse(&strings(malformed)).is_err(), "{malformed:?}");
        }
    }

    #[test]
    fn differing_line_includes_content_and_eof_drift() {
        assert_eq!(first_differing_line("same\nold\n", "same\nnew\n"), 2);
        assert_eq!(first_differing_line("same\n", "same"), 1);
        assert_eq!(first_differing_line("same", "same\n"), 1);
    }

    #[test]
    fn directory_operand_selects_the_manifest_inside_it() {
        let directory = std::env::temp_dir().join(format!(
            "semaprax-fmt-directory-operand-{}",
            std::process::id()
        ));
        std::fs::create_dir_all(&directory).unwrap();
        let expected = directory.join(super::super::project::DEFAULT_MANIFEST);
        assert_eq!(
            parse(&[directory.to_string_lossy().into_owned()]).unwrap(),
            FmtOptions {
                input: FmtInput::Project(expected.clone()),
                check: false,
            }
        );
        // A missing path is still a source operand; nothing is probed further.
        let missing = directory.join("missing.spx");
        assert_eq!(
            parse(&[missing.to_string_lossy().into_owned()])
                .unwrap()
                .input,
            FmtInput::Source(missing)
        );
        std::fs::remove_dir(&directory).unwrap();
    }
}
