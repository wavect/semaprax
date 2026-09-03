//! Explicit retention-metadata persistence and exact-selector restoration.
//! The CLI supplies file and store capabilities; restored metadata carries none.

use std::path::Path;

use semaprax::diagnostic::Diagnostic;
use semaprax::semantic_retention::{
    restore_checkpoint, restore_plan, MAX_RETENTION_CHECKPOINT_BYTES, MAX_RETENTION_PLAN_BYTES,
};
use semaprax::semantic_retention_store;
use serde_json::json;

use super::project_image::read_bounded;

pub(crate) fn persist(
    root: &Path,
    checkpoint_path: &Path,
    expected_checkpoint: &str,
    expected_previous: Option<&str>,
    plan_path: &Path,
    expected_plan: &str,
) -> Result<String, Vec<Diagnostic>> {
    let checkpoint_bytes = read_bounded(checkpoint_path, MAX_RETENTION_CHECKPOINT_BYTES)
        .map_err(|error| vec![error])?;
    let checkpoint = restore_checkpoint(&checkpoint_bytes, expected_checkpoint, expected_previous)?;
    let plan_bytes =
        read_bounded(plan_path, MAX_RETENTION_PLAN_BYTES).map_err(|error| vec![error])?;
    let plan = restore_plan(&plan_bytes, expected_plan, &checkpoint)?;
    let receipt = semantic_retention_store::persist(
        root,
        &checkpoint,
        expected_checkpoint,
        expected_previous,
        &plan,
        expected_plan,
    )?;
    let mut value = json!({
        "schema":"semaprax.semantic-retention-metadata-store-receipt.v1",
        "checkpoint_digest":receipt.checkpoint_digest(),
        "plan_digest":receipt.plan_digest(),
        "envelope_digest":receipt.envelope_digest(),
        "envelope_bytes":receipt.envelope_bytes(),
        "authority":"none",
        "gc_execution":false,
        "source_authority":false,
        "approval_authority":false,
        "publication_authority":false,
        "nonclaims":[
            "not_store_discovery_or_freshness_evidence",
            "not_source_candidate_image_or_environment_restoration",
            "not_gc_deletion_or_plan_execution",
        ],
    });
    value.sort_all_objects();
    Ok(format!("{value}\n"))
}

pub(crate) fn load(
    root: &Path,
    expected_checkpoint: &str,
    expected_previous: Option<&str>,
    expected_plan: &str,
) -> Result<String, Vec<Diagnostic>> {
    let stored = semantic_retention_store::load(
        root,
        expected_checkpoint,
        expected_previous,
        expected_plan,
    )?;
    let mut value = json!({
        "schema":"semaprax.semantic-retention-metadata-store-load.v1",
        "checkpoint_digest":stored.checkpoint().checkpoint_digest(),
        "checkpoint_json":stored.checkpoint().to_json(),
        "plan_digest":stored.plan().plan_digest(),
        "plan_json":stored.plan().to_json(),
        "authority":"none",
        "gc_execution":false,
        "source_authority":false,
        "approval_authority":false,
        "publication_authority":false,
        "nonclaims":[
            "not_store_discovery_or_freshness_evidence",
            "not_source_candidate_image_or_environment_restoration",
            "not_gc_deletion_or_plan_execution",
        ],
    });
    value.sort_all_objects();
    Ok(format!("{value}\n"))
}
