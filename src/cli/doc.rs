//! `semaprax doc <file|project> [--module <path>] [--json]`: documentation,
//! rendered from the checked program and bound to its graph revision.

use std::path::PathBuf;

use semaprax::diagnostic::Diagnostic;
use semaprax::{doc, verify};

pub(crate) struct DocOptions {
    pub(crate) input: PathBuf,
    pub(crate) json: bool,
    pub(crate) module: Option<String>,
}

const USAGE: &str = "doc requires exactly <file|project> [--module <source-path>] [--json]";

pub(crate) fn parse(args: &[String]) -> Result<DocOptions, u8> {
    let mut input = None;
    let mut json = false;
    let mut module = None;
    let mut args = args.iter();
    while let Some(argument) = args.next() {
        match argument.as_str() {
            "--module" if module.is_none() => {
                module = Some(
                    args.next()
                        .filter(|value| !value.starts_with('-'))
                        .ok_or_else(|| {
                            eprintln!("{USAGE}");
                            2
                        })?
                        .clone(),
                );
            }
            "--json" if !json => json = true,
            "--json" => {
                eprintln!("duplicate doc option --json");
                return Err(2);
            }
            option if option.starts_with('-') => {
                eprintln!("unknown doc option `{option}`");
                return Err(2);
            }
            path if input.is_none() => input = Some(PathBuf::from(path)),
            _ => {
                eprintln!("{USAGE}");
                return Err(2);
            }
        }
    }
    let input = input.ok_or_else(|| {
        eprintln!("{USAGE}");
        2
    })?;
    Ok(DocOptions {
        input,
        json,
        module,
    })
}

/// Check the file, then print its documentation. Diagnostics are reported
/// through `report`, which returns the exit status for a failed run.
pub(crate) fn run(options: DocOptions, report: impl Fn(&[Diagnostic]) -> u8) -> Result<(), u8> {
    let input = super::project::resolve_positional(options.input.clone());
    if super::project::is_project_manifest(&input) {
        let output = semaprax::project::with_authenticated_project(&input, |snapshot| {
            doc::project::render(
                &snapshot.retain_revision(),
                options.module.as_deref(),
                options.json,
            )
        })
        .map_err(|diagnostics| report(&diagnostics))?;
        print!("{output}");
        return Ok(());
    }
    if options.module.is_some() {
        eprintln!("--module requires a Project input");
        return Err(2);
    }
    let source = std::fs::read_to_string(&options.input).map_err(|error| {
        report(&[Diagnostic::io(
            "SPX-I001",
            format!("cannot read {}: {error}", options.input.display()),
        )])
    })?;
    let (program, comments) =
        semaprax::parse_with_comments(&source, &options.input).map_err(|error| report(&[error]))?;
    let diagnostics = verify::verify(&program);
    if diagnostics.iter().any(|item| item.severity.is_error()) {
        return Err(report(&diagnostics));
    }
    let output = if options.json {
        doc::json(&program, &comments)
    } else {
        doc::markdown(&program, &comments)
    };
    print!("{output}");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn strings(values: &[&str]) -> Vec<String> {
        values.iter().map(|value| (*value).to_owned()).collect()
    }

    #[test]
    fn doc_grammar_is_closed() {
        let options = parse(&strings(&["source.spx"])).unwrap();
        assert_eq!(options.input, PathBuf::from("source.spx"));
        assert!(!options.json);
        let options = parse(&strings(&["--json", "source.spx"])).unwrap();
        assert_eq!(options.input, PathBuf::from("source.spx"));
        assert!(options.json);
        assert!(parse(&strings(&["source.spx", "--json"])).unwrap().json);
        assert_eq!(
            parse(&strings(&[
                "semaprax.toml",
                "--module",
                "src/core.spx",
                "--json"
            ]))
            .unwrap()
            .module
            .as_deref(),
            Some("src/core.spx")
        );
        for malformed in [
            &["semaprax.toml", "--module"][..],
            &["semaprax.toml", "--module", "--json"][..],
            &[
                "semaprax.toml",
                "--module",
                "src/core.spx",
                "--module",
                "src/app.spx",
            ][..],
            &[][..],
            &["--json"][..],
            &["--unknown", "source.spx"][..],
            &["source.spx", "extra"][..],
            &["source.spx", "--json", "--json"][..],
        ] {
            assert!(parse(&strings(malformed)).is_err(), "{malformed:?}");
        }
    }
}
