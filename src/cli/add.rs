//! `semaprax add <dir>|semaprax.toml <package> <range>`: append one
//! `[dependencies]` row to a Package Manifest v1 table manifest and rewrite it
//! canonically. The manifest is parsed, extended, re-rendered, and re-parsed
//! before one write; a rejected name, range, layout, or duplicate leaves the
//! file untouched. Nothing is fetched, resolved, or contacted.

use std::path::PathBuf;

use semaprax::diagnostic::Diagnostic;
use semaprax::project::{ProjectManifest, MAX_MANIFEST_BYTES};

const USAGE: &str = "add requires exactly <dir>|semaprax.toml <package> <range>";

pub(crate) struct AddOptions {
    pub(crate) manifest: PathBuf,
    pub(crate) package: String,
    pub(crate) range: String,
}

pub(crate) fn parse(args: &[String]) -> Result<AddOptions, u8> {
    match args {
        [manifest, package, range]
            if args
                .iter()
                .all(|argument| !argument.is_empty() && !argument.starts_with('-')) =>
        {
            Ok(AddOptions {
                manifest: super::project::resolve_positional(PathBuf::from(manifest)),
                package: package.clone(),
                range: range.clone(),
            })
        }
        _ => {
            eprintln!("{USAGE}");
            Err(2)
        }
    }
}

/// Extend the manifest and print what changed. Diagnostics are reported
/// through `report`, which returns the exit status for a failed run.
pub(crate) fn run(options: &AddOptions, report: impl Fn(&[Diagnostic]) -> u8) -> Result<(), u8> {
    let path = &options.manifest;
    let metadata = std::fs::metadata(path).map_err(|error| {
        report(&[Diagnostic::io(
            "SPX-I001",
            format!("cannot read {}: {error}", path.display()),
        )])
    })?;
    if !metadata.is_file() || metadata.len() > MAX_MANIFEST_BYTES as u64 {
        return Err(report(&[Diagnostic::io(
            "SPX-J101",
            format!(
                "{} must be a manifest file of at most {MAX_MANIFEST_BYTES} bytes",
                path.display()
            ),
        )]));
    }
    let source = std::fs::read_to_string(path).map_err(|error| {
        report(&[Diagnostic::io(
            "SPX-I001",
            format!("cannot read {}: {error}", path.display()),
        )])
    })?;
    let manifest = ProjectManifest::parse(&source).map_err(|errors| report(&errors))?;
    let rewritten = manifest
        .with_dependency(&options.package, &options.range)
        .map_err(|errors| report(&errors))?;
    std::fs::write(path, &rewritten).map_err(|error| {
        report(&[Diagnostic::io(
            "SPX-I001",
            format!("cannot write {}: {error}", path.display()),
        )])
    })?;
    println!(
        "added {} = \"{}\" to {}",
        options.package,
        options.range,
        path.display()
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn strings(values: &[&str]) -> Vec<String> {
        values.iter().map(|value| (*value).to_owned()).collect()
    }

    #[test]
    fn add_grammar_is_closed() {
        let options = parse(&strings(&["semaprax.toml", "bytes-util", "^1.2.0"])).unwrap();
        // Bare manifest names are normalized like every project operand.
        assert!(options.manifest.ends_with("semaprax.toml"));
        assert_eq!(options.package, "bytes-util");
        assert_eq!(options.range, "^1.2.0");
        for malformed in [
            &[][..],
            &["semaprax.toml"][..],
            &["semaprax.toml", "bytes-util"][..],
            &["semaprax.toml", "bytes-util", "^1.2.0", "extra"][..],
            &["semaprax.toml", "--name", "^1.2.0"][..],
            &["semaprax.toml", "", "^1.2.0"][..],
        ] {
            assert!(parse(&strings(malformed)).is_err(), "{malformed:?}");
        }
    }
}
