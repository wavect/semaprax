//! Explicit retention-metadata persistence and exact-selector restoration.
//! The CLI supplies file and store capabilities; restored metadata carries none.

use std::path::Path;

use semaprax::diagnostic::Diagnostic;
use semaprax::semantic_retention::{
    checkpoint, restore_checkpoint, restore_observation_inventory, restore_plan, RetentionPolicy,
    MAX_RETENTION_CHECKPOINT_BYTES, MAX_RETENTION_INVENTORY_BYTES, MAX_RETENTION_PLAN_BYTES,
};
use semaprax::semantic_retention_store;
use serde_json::json;

use super::project_image::read_bounded;

pub(crate) struct PlanOptions<'a> {
    pub inventory: &'a Path,
    pub sequence: &'a str,
    pub max_subjects: &'a str,
    pub max_bytes: &'a str,
    pub protected_generations: &'a str,
    pub previous_checkpoint: Option<&'a Path>,
    pub expected_previous: Option<&'a str>,
    pub expected_previous_predecessor: Option<&'a str>,
}

pub(crate) fn plan(options: PlanOptions<'_>) -> Result<String, Vec<Diagnostic>> {
    let inventory = read_bounded(options.inventory, MAX_RETENTION_INVENTORY_BYTES)
        .map_err(|error| vec![error])?;
    let observations = restore_observation_inventory(&inventory)?;
    let sequence = decimal(options.sequence, "retention sequence")?;
    let max_subjects = usize::try_from(decimal(options.max_subjects, "retention subject limit")?)
        .map_err(|_| cli_input("retention subject limit exceeds this host"))?;
    let policy = RetentionPolicy::new(
        max_subjects,
        decimal(options.max_bytes, "retention byte limit")?,
        decimal(
            options.protected_generations,
            "retention protected-generation count",
        )?,
    )?;
    let previous_bytes = options
        .previous_checkpoint
        .map(|path| read_bounded(path, MAX_RETENTION_CHECKPOINT_BYTES).map_err(|error| vec![error]))
        .transpose()?;
    let previous = match (previous_bytes.as_deref(), options.expected_previous) {
        (None, None) => None,
        (Some(bytes), Some(expected)) => Some(restore_checkpoint(
            bytes,
            expected,
            options.expected_previous_predecessor,
        )?),
        _ => return Err(cli_input("retention predecessor file and selector differ")),
    };
    let transition = checkpoint(
        previous.as_ref(),
        options.expected_previous,
        sequence,
        policy,
        &observations,
    )?;
    let mut value = json!({
        "schema":"semaprax.semantic-retention-metadata-plan-output.v1",
        "checkpoint_digest":transition.checkpoint().checkpoint_digest(),
        "checkpoint_json":transition.checkpoint().to_json(),
        "plan_digest":transition.plan_digest(),
        "plan_json":transition.plan_json(),
        "authority":"none",
        "gc_execution":false,
        "source_authority":false,
        "approval_authority":false,
        "publication_authority":false,
        "nonclaims":[
            "caller_inventory_not_store_discovery_or_freshness_evidence",
            "not_source_candidate_image_or_environment_restoration",
            "not_gc_deletion_plan_execution_or_metadata_persistence",
        ],
    });
    value.sort_all_objects();
    Ok(format!("{value}\n"))
}

fn decimal(value: &str, field: &'static str) -> Result<u64, Vec<Diagnostic>> {
    if value.is_empty()
        || !value.bytes().all(|byte| byte.is_ascii_digit())
        || (value.len() > 1 && value.starts_with('0'))
    {
        return Err(cli_input(format!("{field} must be canonical decimal")));
    }
    value
        .parse()
        .map_err(|_| cli_input(format!("{field} exceeds u64")))
}

fn cli_input(message: impl Into<String>) -> Vec<Diagnostic> {
    vec![Diagnostic::io("SPX-G443", message)]
}

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
