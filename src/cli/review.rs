//! Closed dispatch for legacy patch review and Project semantic-transaction review.

use std::io::Read as _;
use std::path::{Path, PathBuf};

use semaprax::diagnostic::Diagnostic;
use semaprax::project::{self, SemanticWorkspaceService, MAX_SEMANTIC_TRANSACTION_BYTES};

use super::project::{is_project_manifest, resolve_positional};

const USAGE: &str =
    "review requires <file> <patch.spatch> or <project> <transaction.json> [--evidence]";

enum ReviewInput {
    Legacy {
        source: PathBuf,
        patch: PathBuf,
    },
    Transaction {
        manifest: PathBuf,
        transaction: PathBuf,
        evidence: bool,
    },
}

pub(crate) enum ReviewError {
    Usage,
    Diagnostics(Vec<Diagnostic>),
}

impl From<Vec<Diagnostic>> for ReviewError {
    fn from(diagnostics: Vec<Diagnostic>) -> Self {
        Self::Diagnostics(diagnostics)
    }
}

pub(crate) fn run(args: &[String]) -> Result<String, ReviewError> {
    match parse(args).map_err(|()| ReviewError::Usage)? {
        ReviewInput::Legacy { source, patch } => semaprax::review::preview(&source, &patch)
            .map(|report| format!("{report}\n"))
            .map_err(ReviewError::Diagnostics),
        ReviewInput::Transaction {
            manifest,
            transaction,
            evidence,
        } => project::with_authenticated_project(&manifest, |snapshot| {
            let bytes = read_transaction(&transaction)?;
            let service = SemanticWorkspaceService::open(snapshot.retain_revision())?;
            let artifacts = service.validate_transaction(&bytes)?;
            Ok(if evidence {
                artifacts.evidence().to_owned()
            } else {
                artifacts.review().to_owned()
            })
        })
        .map_err(ReviewError::Diagnostics),
    }
}

fn parse(args: &[String]) -> Result<ReviewInput, ()> {
    if !matches!(args.len(), 2 | 3)
        || args.iter().any(|argument| argument.is_empty())
        || args[..args.len().min(2)]
            .iter()
            .any(|argument| argument.starts_with('-'))
        || (args.len() == 3 && args[2] != "--evidence")
    {
        eprintln!("{USAGE}");
        return Err(());
    }
    let first = resolve_positional(PathBuf::from(&args[0]));
    if is_project_manifest(&first) {
        return Ok(ReviewInput::Transaction {
            manifest: first,
            transaction: PathBuf::from(&args[1]),
            evidence: args.len() == 3,
        });
    }
    if args.len() == 3 {
        eprintln!("review --evidence requires a Project directory or semaprax.toml");
        return Err(());
    }
    Ok(ReviewInput::Legacy {
        source: PathBuf::from(&args[0]),
        patch: PathBuf::from(&args[1]),
    })
}

fn read_transaction(path: &Path) -> Result<Vec<u8>, Vec<Diagnostic>> {
    let file = std::fs::File::open(path).map_err(|_| {
        vec![
            Diagnostic::io("SPX-I001", "cannot read semantic transaction input")
                .at_path(path.display().to_string()),
        ]
    })?;
    let mut bytes = Vec::new();
    file.take(MAX_SEMANTIC_TRANSACTION_BYTES as u64 + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| {
            vec![
                Diagnostic::io("SPX-I001", "cannot read semantic transaction input")
                    .at_path(path.display().to_string()),
            ]
        })?;
    if bytes.len() > MAX_SEMANTIC_TRANSACTION_BYTES {
        return Err(vec![Diagnostic::io(
            "SPX-G526",
            "semantic transaction exceeds its byte limit",
        )]);
    }
    Ok(bytes)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn strings(values: &[&str]) -> Vec<String> {
        values.iter().map(|value| (*value).to_owned()).collect()
    }

    #[test]
    fn grammar_preserves_legacy_and_closes_evidence_mode() {
        assert!(matches!(
            parse(&strings(&["module.spx", "change.spatch"])).unwrap(),
            ReviewInput::Legacy { .. }
        ));
        assert!(parse(&strings(&["module.spx", "change.spatch", "--evidence"])).is_err());
        assert!(parse(&strings(&["module.spx"])).is_err());
        assert!(parse(&strings(&["module.spx", "change.spatch", "--other"])).is_err());
        assert!(parse(&strings(&["--evidence", "change.spatch"])).is_err());
    }
}
