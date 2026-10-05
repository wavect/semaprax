//! One-shot authority-free patch receipt projections over an authenticated Project.

use semaprax::diagnostic::Diagnostic;
use semaprax::project::{with_authenticated_project, SemanticWorkspaceService};
use std::path::PathBuf;

use super::project::{is_project_manifest, resolve_positional};

const USAGE: &str = "patch-receipt requires <project> <render|verify|refusal|verify-refusal|compare|compare-set|evidence-summary|evidence-page> canonical transaction, receipt, and retained-evidence operands";
const EVIDENCE_PAGE_SIZE: usize = 32;
const EVIDENCE_MAX_BYTES: usize = 65_536;

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
    Refusal {
        manifest: PathBuf,
        transaction: String,
        requested_candidate: String,
    },
    VerifyRefusal {
        manifest: PathBuf,
        transaction: String,
        requested_candidate: String,
        receipt: String,
    },
    EvidenceSummary {
        manifest: PathBuf,
        transaction: String,
        candidate: String,
    },
    EvidencePage {
        manifest: PathBuf,
        transaction: String,
        candidate: String,
        evidence_id: String,
        handle: String,
        cursor: Option<String>,
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
    CompareSet {
        manifest: PathBuf,
        entries: Vec<(String, String, String)>,
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
        [operation, transaction, requested_candidate] if operation == "refusal" => {
            Ok(Command::Refusal {
                manifest,
                transaction: transaction.clone(),
                requested_candidate: requested_candidate.clone(),
            })
        }
        [operation, transaction, requested_candidate, receipt] if operation == "verify-refusal" => {
            Ok(Command::VerifyRefusal {
                manifest,
                transaction: transaction.clone(),
                requested_candidate: requested_candidate.clone(),
                receipt: receipt.clone(),
            })
        }
        [operation, transaction, candidate] if operation == "evidence-summary" => {
            Ok(Command::EvidenceSummary {
                manifest,
                transaction: transaction.clone(),
                candidate: candidate.clone(),
            })
        }
        [operation, transaction, candidate, evidence_id, handle, cursor]
            if operation == "evidence-page" =>
        {
            Ok(Command::EvidencePage {
                manifest,
                transaction: transaction.clone(),
                candidate: candidate.clone(),
                evidence_id: evidence_id.clone(),
                handle: handle.clone(),
                cursor: (cursor != "-").then(|| cursor.clone()),
            })
        }
        [operation, left_transaction, left_candidate, left_receipt, right_transaction, right_candidate, right_receipt]
            if operation == "compare" =>
        {
            Ok(Command::Compare {
                manifest,
                left_transaction: left_transaction.clone(),
                left_candidate: left_candidate.clone(),
                left_receipt: left_receipt.clone(),
                right_transaction: right_transaction.clone(),
                right_candidate: right_candidate.clone(),
                right_receipt: right_receipt.clone(),
            })
        }
        [operation, entries @ ..]
            if operation == "compare-set" && entries.len() >= 6 && entries.len() % 3 == 0 =>
        {
            Ok(Command::CompareSet {
                manifest,
                entries: entries
                    .as_chunks::<3>()
                    .0
                    .iter()
                    .map(|[left, digest, receipt]| (left.clone(), digest.clone(), receipt.clone()))
                    .collect(),
            })
        }
        _ => usage(),
    }
}

pub(crate) fn run(command: Command, report: impl Fn(&[Diagnostic]) -> u8) -> Result<(), u8> {
    let manifest = match &command {
        Command::Render { manifest, .. }
        | Command::Verify { manifest, .. }
        | Command::Refusal { manifest, .. }
        | Command::VerifyRefusal { manifest, .. }
        | Command::EvidenceSummary { manifest, .. }
        | Command::EvidencePage { manifest, .. }
        | Command::Compare { manifest, .. }
        | Command::CompareSet { manifest, .. } => manifest,
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
            Command::Refusal {
                transaction,
                requested_candidate,
                ..
            } => service.patch_receipt_refusal(transaction.as_bytes(), &requested_candidate),
            Command::VerifyRefusal {
                transaction,
                requested_candidate,
                receipt,
                ..
            } => service.verify_patch_receipt_refusal(
                transaction.as_bytes(),
                &requested_candidate,
                receipt.as_bytes(),
            ),
            Command::EvidenceSummary {
                transaction,
                candidate,
                ..
            } => service.patch_receipt_evidence_summary(transaction.as_bytes(), &candidate),
            Command::EvidencePage {
                transaction,
                candidate,
                evidence_id,
                handle,
                cursor,
                ..
            } => service.patch_receipt_evidence_page(
                transaction.as_bytes(),
                &candidate,
                &evidence_id,
                &handle,
                cursor.as_deref(),
                EVIDENCE_PAGE_SIZE,
                EVIDENCE_MAX_BYTES,
            ),
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
            Command::CompareSet { entries, .. } => {
                let inputs = entries
                    .iter()
                    .map(|(transaction, candidate, receipt)| {
                        (
                            transaction.as_bytes(),
                            candidate.as_str(),
                            receipt.as_bytes(),
                        )
                    })
                    .collect::<Vec<_>>();
                service.compare_patch_receipt_set(&inputs)
            }
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
        // This module is tested by both the root CLI and semaprax-toolchain.
        let project = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .ancestors()
            .map(|root| root.join("examples/calculator-project"))
            .find(|path| path.join("semaprax.toml").is_file())
            .expect("the checked-in calculator Project fixture must resolve");
        let project = project.to_str().unwrap();
        let args = |values: &[&str]| {
            values
                .iter()
                .map(|value| (*value).to_owned())
                .collect::<Vec<_>>()
        };
        assert!(matches!(
            parse(&args(&[
                project,
                "render",
                "{\"schema\":\"semaprax.semantic-transaction.v1\"}",
                "sha256:abc"
            ])),
            Ok(Command::Render { .. })
        ));
        assert!(matches!(
            parse(&args(&[
                project,
                "evidence-summary",
                "{\"schema\":\"semaprax.semantic-transaction.v1\"}",
                "sha256:abc"
            ])),
            Ok(Command::EvidenceSummary { .. })
        ));
        assert!(matches!(
            parse(&args(&[
                project,
                "refusal",
                "{\"schema\":\"semaprax.semantic-transaction.v1\"}",
                "sha256:stale"
            ])),
            Ok(Command::Refusal { .. })
        ));
        assert!(matches!(
            parse(&args(&[
                project,
                "verify-refusal",
                "{\"schema\":\"semaprax.semantic-transaction.v1\"}",
                "sha256:stale",
                "{\"schema\":\"semaprax.patch-receipt.v1\"}",
            ])),
            Ok(Command::VerifyRefusal { .. })
        ));
        assert!(matches!(
            parse(&args(&[
                project,
                "evidence-page",
                "{\"schema\":\"semaprax.semantic-transaction.v1\"}",
                "sha256:abc",
                "declaration_catalog",
                "sha256:handle",
                "-",
            ])),
            Ok(Command::EvidencePage { cursor: None, .. })
        ));
        assert!(matches!(
            parse(&args(&[
                project,
                "compare-set",
                "{\"schema\":\"semaprax.semantic-transaction.v1\"}",
                "sha256:one",
                "{\"schema\":\"semaprax.patch-receipt.v1\"}",
                "{\"schema\":\"semaprax.semantic-transaction.v1\"}",
                "sha256:two",
                "{\"schema\":\"semaprax.patch-receipt.v1\"}",
                "{\"schema\":\"semaprax.semantic-transaction.v1\"}",
                "sha256:three",
                "{\"schema\":\"semaprax.patch-receipt.v1\"}",
            ])),
            Ok(Command::CompareSet { entries, .. }) if entries.len() == 3
        ));
        for malformed in [
            args(&[]),
            args(&[project, "render", "receipt.json", "sha256:abc", "extra"]),
            args(&[project, "verify", "tx", "digest"]),
            args(&[project, "evidence-page", "tx", "digest"]),
        ] {
            assert!(parse(&malformed).is_err());
        }
    }
}
