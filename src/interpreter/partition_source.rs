//! Explicit file-backed entry to the checked byte partition interpreter.
use super::{
    byte_partition::{
        direct::{self, Outcome},
        funding::Account,
    },
    Diagnostic, Interpretation,
};
use crate::{
    conformance, construction_fusion::partition::operation,
    hir::byte_partition::ValidatedPartitionProgram, patch,
};
use std::path::Path;

pub const PROFILE: &str = "byte-partition-v1";
pub const SCHEMA: &str = "semaprax.interpret.byte-partition.v1";
const MAX_SOURCE_BYTES: usize = super::internal_strings::MAX_SOURCE_BYTES;
const MAX_GRAPH_WORK: usize = 10_000_000;

pub use crate::fused_source::RuntimeAccount;

/// Run a checked zero-argument i64 source entry using an explicit finite
/// requested-layout account. The report is published after the held source
/// snapshot is rechecked. Only scalars, normalized language statuses and
/// resource counters cross this boundary.
pub fn run(path: &Path, funding: RuntimeAccount) -> Result<Interpretation, Vec<Diagnostic>> {
    std::thread::scope(|scope| {
        std::thread::Builder::new()
            .name("semaprax-interpret-partition".into())
            .stack_size(super::EVALUATION_STACK_BYTES)
            .spawn_scoped(scope, || run_on_current_thread(path, funding))
            .map_err(|error| {
                vec![super::guard_error(&format!(
                    "partition interpreter thread failed to start: {error}"
                ))]
            })?
            .join()
            .unwrap_or_else(|_| {
                Err(vec![super::guard_error(
                    "partition interpreter thread panicked",
                )])
            })
    })
}

fn run_on_current_thread(
    path: &Path,
    funding: RuntimeAccount,
) -> Result<Interpretation, Vec<Diagnostic>> {
    let canonical = patch::canonical_source_path(path)?;
    let before = patch::read_source_snapshot_bounded(&canonical, MAX_SOURCE_BYTES, "SPX-F104")?;
    let source = crate::parse(before.source(), path).map_err(|error| vec![error])?;
    let revision = crate::graph::revision(&source);
    let mut work = MAX_GRAPH_WORK;
    let program = ValidatedPartitionProgram::compile_source(&source, &mut work)
        .map_err(|error| vec![error])?;
    let mut allocations = 0;
    let mut payload = 0;
    let (outcome, cleanup, reserved) =
        match Account::funded(&mut allocations, &mut payload, funding.bytes()) {
            Ok(account) => {
                let report = direct::execute(&program, &account, &mut work);
                (report.outcome, report.cleanup_complete, account.charged())
            }
            Err(_) => (Outcome::Refused, false, 0),
        };
    patch::validate_source_unchanged_bounded(
        &canonical,
        path,
        &before,
        &revision,
        MAX_SOURCE_BYTES,
    )?;
    let mut report = serde_json::json!({
        "schema": SCHEMA,
        "profile": PROFILE,
        "source_revision": revision,
        "target": "interpreter",
        "account_limit": funding.bytes(),
        "managed_reserved": reserved,
        "backing_allocations": allocations,
        "payload_bytes": payload,
        "cleanup_complete": cleanup,
    });
    let returned = match outcome {
        Outcome::Returned(value) => {
            report["outcome"] = "returned".into();
            report["result"] = value.to_string().into();
            true
        }
        outcome => {
            let status = match outcome {
                Outcome::PartitionFault(fault) => {
                    Some((operation::STATUS_DOMAIN, operation::fault_code(fault)))
                }
                Outcome::BufferBoundExceeded => Some((
                    crate::byte_ops::SET_STATUS_DOMAIN,
                    crate::byte_ops::CAPACITY_EXCEEDED_CODE,
                )),
                Outcome::ByteStoreOutOfBounds => Some((
                    crate::byte_ops::SET_STATUS_DOMAIN,
                    crate::byte_ops::SET_INDEX_OUT_OF_BOUNDS_CODE,
                )),
                Outcome::PostconditionFailed => Some((
                    conformance::CONTRACT_STATUS_DOMAIN_V1,
                    conformance::CONTRACT_ENSURES_FALSE_CODE,
                )),
                Outcome::AllocationFailure | Outcome::Refused => None,
                Outcome::Returned(_) => unreachable!(),
            };
            if let Some((domain, code)) = status {
                report["outcome"] = "language_failure".into();
                report["status"] = serde_json::json!({"domain": domain, "code": code});
            } else {
                report["outcome"] = "administrative_refusal".into();
                report["reason"] = if matches!(outcome, Outcome::AllocationFailure) {
                    "backing_limit"
                } else {
                    "receiving_authority_or_account"
                }
                .into();
            }
            false
        }
    };
    Ok(Interpretation {
        envelope: report.to_string(),
        returned,
    })
}
