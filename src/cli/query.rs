//! `semaprax query`: legacy declaration search plus Universal Semantic Query v1.

use std::collections::BTreeSet;
use std::path::PathBuf;

use semaprax::diagnostic::Diagnostic;
use semaprax::project::{self, SemanticQuery, SemanticWorkspaceService};
use semaprax::query::{self, QueryFilters};
use semaprax::verify;
use semaprax::workspace_analysis::{
    WorkspaceAnalysisDirection, WorkspaceAnalysisTargetKind, WorkspaceContextOptions,
    WorkspaceImpactOptions,
};

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

pub(crate) enum QueryCommand {
    Legacy(QueryOptions),
    Universal(UniversalQueryOptions),
}

pub(crate) struct UniversalQueryOptions {
    manifest: PathBuf,
    revision: Option<String>,
    operation: UniversalOperation,
}

enum UniversalOperation {
    Declarations(QueryFilters, usize, usize),
    Symbol(String),
    Context(WorkspaceAnalysisTargetKind, String, WorkspaceContextOptions),
    Impact(WorkspaceAnalysisTargetKind, String, WorkspaceImpactOptions),
    AvailableOperations(String),
}

const LEGACY_USAGE: &str = "query requires exactly <file|project> [--kind <kind>[,<kind>]] [--name <text>] [--id <prefix>] [--effect <effect>] [--calls <stable-id>] [--called-by <stable-id>] [--json]";
const UNIVERSAL_USAGE: &str = "semantic query requires <project> <declarations|symbol|context|impact|available-operations> and that operation's exact operands; see --help";

pub(crate) fn parse_command(args: &[String]) -> Result<QueryCommand, u8> {
    if args.get(1).is_some_and(|value| {
        matches!(
            value.as_str(),
            "declarations" | "symbol" | "context" | "impact" | "available-operations"
        )
    }) {
        parse_universal(args).map(QueryCommand::Universal)
    } else {
        parse(args).map(QueryCommand::Legacy)
    }
}

/// Preserve the original declaration-query parser byte-for-byte in behavior.
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
            }
            "--json" => {
                eprintln!("duplicate query option --json");
                return Err(2);
            }
            "--kind" | "--name" | "--id" | "--effect" | "--calls" | "--called-by" => {
                parse_filter(args, &mut index, &mut filters, argument)?;
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
                eprintln!("{LEGACY_USAGE}");
                return Err(2);
            }
        }
    }
    let input = input.ok_or_else(|| {
        eprintln!("{LEGACY_USAGE}");
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

fn parse_universal(args: &[String]) -> Result<UniversalQueryOptions, u8> {
    let manifest = resolve_positional(PathBuf::from(&args[0]));
    if !is_project_manifest(&manifest) {
        eprintln!("universal semantic query requires a Project directory or semaprax.toml");
        return Err(2);
    }
    let (operation, revision) = match args[1].as_str() {
        "declarations" => parse_declarations(&args[2..])?,
        "symbol" => {
            let (target, revision) = target_and_revision("symbol", &args[2..])?;
            (UniversalOperation::Symbol(target), revision)
        }
        "available-operations" => {
            let (target, revision) = target_and_revision("available-operations", &args[2..])?;
            (UniversalOperation::AvailableOperations(target), revision)
        }
        "context" => parse_context(&args[2..])?,
        "impact" => parse_impact(&args[2..])?,
        _ => unreachable!("closed universal query operation"),
    };
    Ok(UniversalQueryOptions {
        manifest,
        revision,
        operation,
    })
}

fn parse_declarations(args: &[String]) -> Result<(UniversalOperation, Option<String>), u8> {
    let mut filters = QueryFilters::default();
    let mut offset = 0;
    let mut limit = project::MAX_SEMANTIC_QUERY_DECLARATION_LIMIT;
    let mut revision = None;
    let mut seen = BTreeSet::new();
    let mut index = 0;
    while index < args.len() {
        let option = args[index].as_str();
        match option {
            "--kind" | "--name" | "--id" | "--effect" | "--calls" | "--called-by" => {
                parse_filter(args, &mut index, &mut filters, option)?;
            }
            "--offset" | "--limit" | "--revision" => {
                unique(&mut seen, option, "declarations")?;
                let value = option_value(args, index, option, "declarations")?;
                match option {
                    "--offset" => offset = number(value, option, "declarations")?,
                    "--limit" => limit = number(value, option, "declarations")?,
                    _ => revision = Some(value.to_owned()),
                }
                index += 2;
            }
            _ => return unknown(option, "declarations"),
        }
    }
    Ok((
        UniversalOperation::Declarations(filters, offset, limit),
        revision,
    ))
}

fn target_and_revision(command: &str, args: &[String]) -> Result<(String, Option<String>), u8> {
    let target = args
        .first()
        .filter(|value| !value.is_empty() && !value.starts_with('-'))
        .ok_or_else(usage)?
        .to_owned();
    let revision = match &args[1..] {
        [] => None,
        [option, value] if option == "--revision" && !value.is_empty() => Some(value.clone()),
        [option, ..] => return unknown(option, command),
    };
    Ok((target, revision))
}

fn parse_context(args: &[String]) -> Result<(UniversalOperation, Option<String>), u8> {
    let (kind, target) = analysis_target("context", args)?;
    let mut direction = WorkspaceAnalysisDirection::Both;
    let mut depth = 4;
    let mut max_bytes = 1024 * 1024;
    let mut max_nodes = 1024;
    let mut revision = None;
    let mut seen = BTreeSet::new();
    let mut index = 2;
    while index < args.len() {
        let option = args[index].as_str();
        if !matches!(
            option,
            "--direction" | "--depth" | "--max-bytes" | "--max-nodes" | "--revision"
        ) {
            return unknown(option, "context");
        }
        unique(&mut seen, option, "context")?;
        let value = option_value(args, index, option, "context")?;
        match option {
            "--direction" => {
                direction = match value {
                    "forward" => WorkspaceAnalysisDirection::Forward,
                    "reverse" => WorkspaceAnalysisDirection::Reverse,
                    "both" => WorkspaceAnalysisDirection::Both,
                    _ => return unknown(value, "context direction"),
                }
            }
            "--depth" => depth = number(value, option, "context")?,
            "--max-bytes" => max_bytes = number(value, option, "context")?,
            "--max-nodes" => max_nodes = number(value, option, "context")?,
            _ => revision = Some(value.to_owned()),
        }
        index += 2;
    }
    let options = WorkspaceContextOptions::new(direction, depth, max_bytes, max_nodes)
        .map_err(|error| usage_error(&error.to_string()))?;
    Ok((UniversalOperation::Context(kind, target, options), revision))
}

fn parse_impact(args: &[String]) -> Result<(UniversalOperation, Option<String>), u8> {
    let (kind, target) = analysis_target("impact", args)?;
    let mut depth = 16;
    let mut max_bytes = 1024 * 1024;
    let mut max_nodes = 1024;
    let mut revision = None;
    let mut seen = BTreeSet::new();
    let mut index = 2;
    while index < args.len() {
        let option = args[index].as_str();
        if !matches!(
            option,
            "--depth" | "--max-bytes" | "--max-nodes" | "--revision"
        ) {
            return unknown(option, "impact");
        }
        unique(&mut seen, option, "impact")?;
        let value = option_value(args, index, option, "impact")?;
        match option {
            "--depth" => depth = number(value, option, "impact")?,
            "--max-bytes" => max_bytes = number(value, option, "impact")?,
            "--max-nodes" => max_nodes = number(value, option, "impact")?,
            _ => revision = Some(value.to_owned()),
        }
        index += 2;
    }
    let options = WorkspaceImpactOptions::new(depth, max_bytes, max_nodes)
        .map_err(|error| usage_error(&error.to_string()))?;
    Ok((UniversalOperation::Impact(kind, target, options), revision))
}

fn analysis_target(
    command: &str,
    args: &[String],
) -> Result<(WorkspaceAnalysisTargetKind, String), u8> {
    let kind = match args.first().map(String::as_str) {
        Some("declaration") => WorkspaceAnalysisTargetKind::Declaration,
        Some("capability") => WorkspaceAnalysisTargetKind::Capability,
        _ => {
            eprintln!("semantic query {command} target kind must be `declaration` or `capability`");
            return Err(2);
        }
    };
    let target = args
        .get(1)
        .filter(|value| !value.is_empty() && !value.starts_with('-'))
        .ok_or_else(usage)?
        .to_owned();
    Ok((kind, target))
}

fn parse_filter(
    args: &[String],
    index: &mut usize,
    filters: &mut QueryFilters,
    option: &str,
) -> Result<(), u8> {
    let Some(value) = args
        .get(*index + 1)
        .filter(|value| !value.is_empty() && !value.starts_with('-'))
    else {
        eprintln!("query option {option} requires one value");
        return Err(2);
    };
    let slot = match option {
        "--kind" => {
            filters.kinds.extend(value.split(',').map(str::to_owned));
            *index += 2;
            return Ok(());
        }
        "--name" => &mut filters.name,
        "--id" => &mut filters.id_prefix,
        "--effect" => &mut filters.effect,
        "--calls" => &mut filters.calls,
        _ => &mut filters.called_by,
    };
    if slot.is_some() {
        eprintln!("duplicate query option {option}");
        return Err(2);
    }
    *slot = Some(value.clone());
    *index += 2;
    Ok(())
}

fn option_value<'a>(
    args: &'a [String],
    index: usize,
    option: &str,
    command: &str,
) -> Result<&'a str, u8> {
    args.get(index + 1)
        .filter(|value| !value.is_empty() && !value.starts_with('-'))
        .map(String::as_str)
        .ok_or_else(|| {
            eprintln!("semantic query {command} option `{option}` requires a value");
            2
        })
}

fn unique(seen: &mut BTreeSet<String>, option: &str, command: &str) -> Result<(), u8> {
    if seen.insert(option.to_owned()) {
        Ok(())
    } else {
        eprintln!("duplicate semantic query {command} option `{option}`");
        Err(2)
    }
}

fn number(value: &str, option: &str, command: &str) -> Result<usize, u8> {
    if value.is_empty()
        || !value.bytes().all(|byte| byte.is_ascii_digit())
        || (value.len() > 1 && value.starts_with('0'))
    {
        eprintln!(
            "semantic query {command} option `{option}` requires a canonical nonnegative integer"
        );
        return Err(2);
    }
    value.parse().map_err(|_| {
        eprintln!(
            "semantic query {command} option `{option}` requires a canonical nonnegative integer"
        );
        2
    })
}

fn unknown<T>(value: &str, command: &str) -> Result<T, u8> {
    eprintln!("unknown semantic query {command} option or operand `{value}`");
    Err(2)
}

fn usage() -> u8 {
    eprintln!("{UNIVERSAL_USAGE}");
    2
}

fn usage_error(message: &str) -> u8 {
    eprintln!("{message}");
    2
}

pub(crate) fn run_command(
    command: QueryCommand,
    report: impl Fn(&[Diagnostic]) -> u8,
) -> Result<(), u8> {
    match command {
        QueryCommand::Legacy(options) => run(options, report),
        QueryCommand::Universal(options) => run_universal(options, report),
    }
}

/// Check the input and print the original declaration-query representation.
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
                query::run_project(&snapshot.retain_revision(), &options.filters)
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

fn run_universal(
    options: UniversalQueryOptions,
    report: impl Fn(&[Diagnostic]) -> u8,
) -> Result<(), u8> {
    let output = project::with_authenticated_project(&options.manifest, |snapshot| {
        let service = SemanticWorkspaceService::open(snapshot.retain_revision())?;
        let expected = options
            .revision
            .as_deref()
            .unwrap_or_else(|| service.active_generation().workspace_revision());
        let query = match options.operation {
            UniversalOperation::Declarations(ref filters, offset, limit) => {
                SemanticQuery::declarations(expected, filters, offset, limit)?
            }
            UniversalOperation::Symbol(ref target) => SemanticQuery::symbol(expected, target)?,
            UniversalOperation::Context(kind, ref target, options) => {
                SemanticQuery::context(expected, kind, target, options)?
            }
            UniversalOperation::Impact(kind, ref target, options) => {
                SemanticQuery::impact(expected, kind, target, options)?
            }
            UniversalOperation::AvailableOperations(ref target) => {
                SemanticQuery::available_operations(expected, target)?
            }
        };
        Ok(service
            .query(query.to_json().as_bytes())?
            .to_json()
            .to_owned())
    })
    .map_err(|errors| report(&errors))?;
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
    fn legacy_query_grammar_is_unchanged() {
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
        for malformed in [
            &[][..],
            &["--json"][..],
            &["m.spx", "extra"][..],
            &["m.spx", "--unknown"][..],
            &["m.spx", "--name"][..],
            &["m.spx", "--name", "a", "--name", "b"][..],
            &["m.spx", "--json", "--json"][..],
        ] {
            assert!(parse(&strings(malformed)).is_err(), "{malformed:?}");
        }
    }

    #[test]
    fn universal_query_grammar_is_closed() {
        for arguments in [
            vec![
                "fixtures/semaprax.toml",
                "declarations",
                "--offset",
                "0",
                "--limit",
                "2",
            ],
            vec!["fixtures/semaprax.toml", "symbol", "app.main"],
            vec!["fixtures/semaprax.toml", "available-operations", "app.main"],
            vec![
                "fixtures/semaprax.toml",
                "context",
                "declaration",
                "app.main",
                "--depth",
                "1",
            ],
            vec![
                "fixtures/semaprax.toml",
                "impact",
                "capability",
                "clock.read",
                "--max-nodes",
                "8",
            ],
        ] {
            assert!(matches!(
                parse_command(&strings(&arguments)).unwrap(),
                QueryCommand::Universal(_)
            ));
        }
        for malformed in [
            vec!["m.spx", "symbol", "app.main"],
            vec!["fixtures/semaprax.toml", "symbol"],
            vec!["fixtures/semaprax.toml", "symbol", "app.main", "extra"],
            vec!["fixtures/semaprax.toml", "context", "other", "app.main"],
            vec!["fixtures/semaprax.toml", "declarations", "--limit", "01"],
            vec![
                "fixtures/semaprax.toml",
                "impact",
                "declaration",
                "app.main",
                "--depth",
            ],
        ] {
            assert!(
                parse_command(&strings(&malformed)).is_err(),
                "{malformed:?}"
            );
        }
    }
}
