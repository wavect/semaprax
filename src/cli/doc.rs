//! `semaprax doc <file> [--json]`: the documentation projection of one module,
//! rendered from the checked program and bound to its graph revision.

use std::path::PathBuf;

use semaprax::diagnostic::Diagnostic;
use semaprax::{doc, verify};

pub(crate) struct DocOptions {
    pub(crate) input: PathBuf,
    pub(crate) json: bool,
}

const USAGE: &str = "doc requires exactly <file> [--json]";

pub(crate) fn parse(args: &[String]) -> Result<DocOptions, u8> {
    let mut input = None;
    let mut json = false;
    for argument in args {
        match argument.as_str() {
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
    Ok(DocOptions { input, json })
}

/// Check the file, then print its documentation. Diagnostics are reported
/// through `report`, which returns the exit status for a failed run.
pub(crate) fn run(options: DocOptions, report: impl Fn(&[Diagnostic]) -> u8) -> Result<(), u8> {
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
        for malformed in [
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
