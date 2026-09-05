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
                eprintln!("{} is not canonically formatted", file.path.display());
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

/// The manifest's source files, in manifest order, relative to its directory.
/// The manifest parser has already required canonical relative `.spx` paths.
fn project_sources(
    manifest_path: &Path,
    report: &impl Fn(&[Diagnostic]) -> u8,
) -> Result<Vec<PathBuf>, u8> {
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
                input: FmtInput::Project(PathBuf::from("fixtures/semaprax.toml")),
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
