//! `semaprax query <file|project> [filters] [--json]`: search one checked
//! module or authenticated Project by kind, name, identity, effect, or calls.

use std::path::PathBuf;

use semaprax::diagnostic::Diagnostic;
use semaprax::project;
use semaprax::query::{self, QueryFilters};
use semaprax::verify;

use super::project::{is_project_manifest, resolve_positional};

#[derive(Debug, Eq, PartialEq)]
pub(crate) enum QueryInput {
    Source(PathBuf),
    Project(PathBuf),
}

pub(crate) struct QueryOptions {
    pub(crate) input: QueryInput,
    pub(crate) filters: QueryFilters,
    pub(crate) json: bool,
}

const USAGE: &str = "query requires exactly <file|project> [--kind <kind>[,<kind>]] [--name <text>] [--id <prefix>] [--effect <effect>] [--calls <stable-id>] [--called-by <stable-id>] [--json]";

pub(crate) fn parse(args: &[String]) -> Result<QueryOptions, u8> {
    let mut input = None;
    let mut filters = QueryFilters::default();
    let mut json = false;
    let mut index = 0;
    while index < args.len() {
        let argument = args[index].as_str();
        match argument {
            "--json" if !json => {
                json = true;
                index += 1;
                continue;
            }
            "--json" => {
                eprintln!("duplicate query option --json");
                return Err(2);
            }
            "--kind" | "--name" | "--id" | "--effect" | "--calls" | "--called-by" => {
                let Some(value) = args
                    .get(index + 1)
                    .filter(|value| !value.is_empty() && !value.starts_with('-'))
                else {
                    eprintln!("query option {argument} requires one value");
                    return Err(2);
                };
                let slot = match argument {
                    "--kind" => {
                        filters.kinds.extend(value.split(',').map(str::to_owned));
                        index += 2;
                        continue;
                    }
                    "--name" => &mut filters.name,
                    "--id" => &mut filters.id_prefix,
                    "--effect" => &mut filters.effect,
                    "--calls" => &mut filters.calls,
                    _ => &mut filters.called_by,
                };
                if slot.is_some() {
                    eprintln!("duplicate query option {argument}");
                    return Err(2);
                }
                *slot = Some(value.clone());
                index += 2;
            }
            option if option.starts_with('-') => {
                eprintln!("unknown query option `{option}`");
                return Err(2);
            }
            path if input.is_none() => {
                input = Some(PathBuf::from(path));
                index += 1;
            }
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
    let input = match resolve_positional(input) {
        path if is_project_manifest(&path) => QueryInput::Project(path),
        path => QueryInput::Source(path),
    };
    Ok(QueryOptions {
        input,
        filters,
        json,
    })
}

/// Check the file, then print the matches. Diagnostics are reported through
/// `report`, which returns the exit status for a failed run.
pub(crate) fn run(options: QueryOptions, report: impl Fn(&[Diagnostic]) -> u8) -> Result<(), u8> {
    let output = match options.input {
        QueryInput::Source(input) => {
            let source = std::fs::read_to_string(&input).map_err(|error| {
                report(&[Diagnostic::io(
                    "SPX-I001",
                    format!("cannot read {}: {error}", input.display()),
                )])
            })?;
            let (program, comments) =
                semaprax::parse_with_comments(&source, &input).map_err(|error| report(&[error]))?;
            let diagnostics = verify::verify(&program);
            if diagnostics.iter().any(|item| item.severity.is_error()) {
                return Err(report(&diagnostics));
            }
            let result = query::run(&program, &comments, &options.filters)
                .map_err(|errors| report(&errors))?;
            if options.json {
                query::json(&result)
            } else {
                query::text(&result)
            }
        }
        QueryInput::Project(manifest) => {
            let result = project::with_authenticated_project(&manifest, |snapshot| {
                let revision = snapshot.retain_revision();
                query::run_project(&revision, &options.filters)
            })
            .map_err(|errors| report(&errors))?;
            if options.json {
                query::project_json(&result)
            } else {
                query::project_text(&result)
            }
        }
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
    fn query_grammar_is_closed() {
        let options = parse(&strings(&[
            "m.spx",
            "--kind",
            "function,method",
            "--name",
            "tick",
            "--id",
            "app.",
            "--effect",
            "clock.read",
            "--calls",
            "a.b",
            "--called-by",
            "c.d",
            "--json",
        ]))
        .unwrap();
        assert_eq!(options.input, QueryInput::Source(PathBuf::from("m.spx")));
        assert_eq!(options.filters.kinds, vec!["function", "method"]);
        assert_eq!(options.filters.name.as_deref(), Some("tick"));
        assert_eq!(options.filters.id_prefix.as_deref(), Some("app."));
        assert_eq!(options.filters.effect.as_deref(), Some("clock.read"));
        assert_eq!(options.filters.calls.as_deref(), Some("a.b"));
        assert_eq!(options.filters.called_by.as_deref(), Some("c.d"));
        assert!(options.json);
        assert!(parse(&strings(&["--json", "m.spx"])).unwrap().json);
        assert!(matches!(
            parse(&strings(&["fixtures/semaprax.toml"])).unwrap().input,
            QueryInput::Project(path) if path.ends_with("fixtures/semaprax.toml")
        ));
        for malformed in [
            &[][..],
            &["--json"][..],
            &["m.spx", "extra"][..],
            &["m.spx", "--unknown"][..],
            &["m.spx", "--name"][..],
            &["m.spx", "--name", "--json"][..],
            &["m.spx", "--name", "a", "--name", "b"][..],
            &["m.spx", "--json", "--json"][..],
        ] {
            assert!(parse(&strings(malformed)).is_err(), "{malformed:?}");
        }
    }
}
