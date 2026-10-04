//! One-shot authority-free patch receipt projections over an authenticated Project.

use semaprax::diagnostic::Diagnostic;
use semaprax::project::{SemanticWorkspaceService, with_authenticated_project};
use std::path::PathBuf;

use super::project::{is_project_manifest, resolve_positional};

const USAGE: &str = "patch-receipt requires <project> <render|verify|compare> canonical transaction and receipt operands";

pub(crate) enum Command {
    Render {
        manifest: PathBuf,
        transaction: String,
        candidate: String,
    },
    Verify {
        manifest: PathBuf,
        transaction: String,
        candidate: String,
        receipt: String,
    },
    Compare {
        manifest: PathBuf,
        left_transaction: String,
        left_candidate: String,
        left_receipt: String,
        right_transaction: String,
        right_candidate: String,
        right_receipt: String,
    },
}

pub(crate) fn parse(args: &[String]) -> Result<Command, u8> {
    let Some((project, rest)) = args.split_first() else {
        return usage();
    };
    if project.is_empty() || project.starts_with('-') {
        return usage();
    }
    let manifest = resolve_positional(PathBuf::from(project));
    if !is_project_manifest(&manifest) {
        eprintln!("patch-receipt requires a Project directory or semaprax.toml");
        return Err(2);
    }
    match rest {
        [operation, transaction, candidate] if operation == "render" => Ok(Command::Render {
            manifest,
            transaction: transaction.clone(),
            candidate: candidate.clone(),
        }),
        [operation, transaction, candidate, receipt] if operation == "verify" => {
            Ok(Command::Verify {
                manifest,
                transaction: transaction.clone(),
                candidate: candidate.clone(),
                receipt: receipt.clone(),
            })
        }
        [
            operation,
            left_transaction,
            left_candidate,
            left_receipt,
            right_transaction,
            right_candidate,
            right_receipt,
        ] if operation == "compare" => Ok(Command::Compare {
            manifest,
            left_transaction: left_transaction.clone(),
            left_candidate: left_candidate.clone(),
            left_receipt: left_receipt.clone(),
            right_transaction: right_transaction.clone(),
            right_candidate: right_candidate.clone(),
            right_receipt: right_receipt.clone(),
        }),
        _ => usage(),
    }
}

pub(crate) fn run(command: Command, report: impl Fn(&[Diagnostic]) -> u8) -> Result<(), u8> {
    let manifest = match &command {
        Command::Render { manifest, .. }
        | Command::Verify { manifest, .. }
        | Command::Compare { manifest, .. } => manifest,
    }
    .clone();
    let output = with_authenticated_project(&manifest, |snapshot| {
        let revision = snapshot.retain_revision();
        let service = SemanticWorkspaceService::open(revision)?;
        match command {
            Command::Render {
                transaction,
                candidate,
                ..
            } => service.patch_receipt(transaction.as_bytes(), &candidate),
            Command::Verify {
                transaction,
                candidate,
                receipt,
                ..
            } => {
                service.verify_patch_receipt(transaction.as_bytes(), &candidate, receipt.as_bytes())
            }
            Command::Compare {
                left_transaction,
                left_candidate,
                left_receipt,
                right_transaction,
                right_candidate,
                right_receipt,
                ..
            } => service.compare_patch_receipts(
                left_transaction.as_bytes(),
                &left_candidate,
                left_receipt.as_bytes(),
                right_transaction.as_bytes(),
                &right_candidate,
                right_receipt.as_bytes(),
            ),
        }
    })
    .map_err(|errors| report(&errors))?;
    print!("{output}");
    Ok(())
}

fn usage<T>() -> Result<T, u8> {
    eprintln!("{USAGE}");
    Err(2)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn grammar_is_closed_and_receipts_are_values_not_paths() {
        let args = |values: &[&str]| {
            values
                .iter()
                .map(|value| (*value).to_owned())
                .collect::<Vec<_>>()
        };
        assert!(matches!(
            parse(&args(&[
                "examples/calculator-project",
                "render",
                "{\"schema\":\"semaprax.semantic-transaction.v1\"}",
                "sha256:abc"
            ])),
            Ok(Command::Render { .. })
        ));
        for malformed in [
            args(&[]),
            args(&[
                "examples/calculator-project",
                "render",
                "receipt.json",
                "sha256:abc",
                "extra",
            ]),
            args(&["examples/calculator-project", "verify", "tx", "digest"]),
            args(&["examples/calculator-project", "page", "tx", "digest"]),
        ] {
            assert!(parse(&malformed).is_err());
        }
    }
}
