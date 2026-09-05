//! Validation-only `semaprax change preview` over Universal Semantic Transaction v1.

use std::path::PathBuf;

use semaprax::diagnostic::Diagnostic;
use semaprax::project::{
    self, SemanticQuery, SemanticTransaction, SemanticTransactionRenameDisplayName,
    SemanticWorkspaceService, SEMANTIC_QUERY_AVAILABLE_OPERATIONS_SCHEMA,
};

use super::project::{is_project_manifest, resolve_positional};

pub(crate) struct ChangePreview {
    manifest: PathBuf,
    target: String,
    new_name: String,
    revision: Option<String>,
    evidence: bool,
}

const USAGE: &str = "change requires preview <project> rename-display-name <stable-id> <new-name> [--revision digest] [--evidence]";

pub(crate) fn parse(args: &[String]) -> Result<ChangePreview, u8> {
    if args.first().map(String::as_str) != Some("preview")
        || args.get(2).map(String::as_str) != Some("rename-display-name")
    {
        return Err(usage());
    }
    let manifest = args.get(1).map(PathBuf::from).ok_or_else(usage)?;
    let manifest = resolve_positional(manifest);
    if !is_project_manifest(&manifest) {
        eprintln!("change preview requires a Project directory or semaprax.toml");
        return Err(2);
    }
    let target = required(args, 3)?;
    let new_name = required(args, 4)?;
    let mut revision = None;
    let mut evidence = false;
    let mut index = 5;
    while index < args.len() {
        match args[index].as_str() {
            "--evidence" if !evidence => {
                evidence = true;
                index += 1;
            }
            "--evidence" => {
                eprintln!("change preview option `--evidence` may not be repeated");
                return Err(2);
            }
            "--revision" if revision.is_none() => {
                revision = Some(required(args, index + 1)?);
                index += 2;
            }
            "--revision" => {
                eprintln!("change preview option `--revision` may not be repeated");
                return Err(2);
            }
            option => {
                eprintln!("unknown change preview option `{option}`");
                return Err(2);
            }
        }
    }
    Ok(ChangePreview {
        manifest,
        target,
        new_name,
        revision,
        evidence,
    })
}

fn required(args: &[String], index: usize) -> Result<String, u8> {
    args.get(index)
        .filter(|value| !value.is_empty() && !value.starts_with('-'))
        .cloned()
        .ok_or_else(usage)
}

fn usage() -> u8 {
    eprintln!("{USAGE}");
    2
}

pub(crate) fn run(options: ChangePreview, report: impl Fn(&[Diagnostic]) -> u8) -> Result<(), u8> {
    let output = project::with_authenticated_project(&options.manifest, |snapshot| {
        let service = SemanticWorkspaceService::open(snapshot.retain_revision())?;
        let expected = options
            .revision
            .as_deref()
            .unwrap_or_else(|| service.active_generation().workspace_revision());
        let discovery = SemanticQuery::available_operations(expected, &options.target)?;
        let discovery = service.query(discovery.to_json().as_bytes())?;
        let payload: serde_json::Value =
            serde_json::from_str(discovery.payload()).map_err(|_| {
                vec![Diagnostic::io(
                    "SPX-G531",
                    "available operations payload is not valid JSON",
                )]
            })?;
        if payload.get("schema").and_then(serde_json::Value::as_str)
            != Some(SEMANTIC_QUERY_AVAILABLE_OPERATIONS_SCHEMA)
        {
            return Err(vec![Diagnostic::io(
                "SPX-G531",
                "available operations payload schema is unsupported",
            )]);
        }
        let operation = payload
            .get("operations")
            .and_then(serde_json::Value::as_array)
            .and_then(|operations| operations.first())
            .and_then(serde_json::Value::as_object)
            .ok_or_else(|| {
                vec![Diagnostic::io(
                    "SPX-G531",
                    "available operations payload has no operation entry",
                )]
            })?;
        let old_name = operation
            .get("expected_old_value")
            .and_then(serde_json::Value::as_str)
            .unwrap_or("");
        let transaction = SemanticTransaction::rename_display_name(
            expected,
            SemanticTransactionRenameDisplayName::new(&options.target, old_name, &options.new_name),
        )?;
        let artifacts = service.validate_transaction(transaction.to_json().as_bytes())?;
        Ok(if options.evidence {
            artifacts.evidence().to_owned()
        } else {
            artifacts.result().to_owned()
        })
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
    fn preview_grammar_is_closed() {
        let parsed = parse(&strings(&[
            "preview",
            "fixtures/semaprax.toml",
            "rename-display-name",
            "app.run",
            "execute",
            "--revision",
            "sha256:abc",
            "--evidence",
        ]))
        .unwrap();
        assert_eq!(parsed.target, "app.run");
        assert_eq!(parsed.new_name, "execute");
        assert_eq!(parsed.revision.as_deref(), Some("sha256:abc"));
        assert!(parsed.evidence);
        for malformed in [
            vec![],
            vec![
                "preview",
                "m.spx",
                "rename-display-name",
                "app.run",
                "execute",
            ],
            vec![
                "apply",
                "fixtures/semaprax.toml",
                "rename-display-name",
                "app.run",
                "execute",
            ],
            vec![
                "preview",
                "fixtures/semaprax.toml",
                "rename",
                "app.run",
                "execute",
            ],
            vec![
                "preview",
                "fixtures/semaprax.toml",
                "rename-display-name",
                "app.run",
            ],
            vec![
                "preview",
                "fixtures/semaprax.toml",
                "rename-display-name",
                "app.run",
                "execute",
                "--unknown",
            ],
        ] {
            assert!(parse(&strings(&malformed)).is_err(), "{malformed:?}");
        }
    }
}
