//! Strict, non-authoritative parsing of submitted Structural Change Evidence.

use std::fs::File;
use std::io::Read as _;
use std::path::Path;

use serde_json::{Map, Value};

use super::artifact::SemanticWorkspaceStructuralChangeArtifacts;
use super::{limit, Diagnostic};

const EVIDENCE_SCHEMA: &str = "semaprax.workspace-semantic-structural-change-evidence.v1";
const VERIFICATION_RECEIPT_SCHEMA: &str =
    "semaprax.workspace-semantic-structural-change-evidence-verification.v1";
const APPLICATION_RECEIPT_SCHEMA: &str =
    "semaprax.workspace-semantic-structural-change-evidence-application.v1";
const FORMAT_LEAD: &str =
    "Semantic Workspace Structural Change Evidence must be one canonical JSON line with one terminal LF";
const REPLAY_MESSAGE: &str =
    "Semantic Workspace Structural Change Evidence does not exactly replay the authenticated proposal and candidate";
pub(super) const MAX_EVIDENCE_BYTES: usize = 1_048_576;
const MAX_JSON_DEPTH: usize = 8;

const TOP_KEYS: [&str; 17] = [
    "schema",
    "workspace_manifest_schema",
    "base_workspace_revision",
    "candidate_workspace_revision",
    "entry_module",
    "proposal",
    "base_workspace_graph",
    "candidate_workspace_graph",
    "candidate_manifest",
    "structural_change_preview",
    "context",
    "impact",
    "review",
    "paths",
    "limits",
    "budget",
    "nonclaims",
];
const REF_KEYS: [&str; 3] = ["schema", "digest", "bytes"];
const GRAPH_REF_KEYS: [&str; 2] = ["schema", "digest"];
const PATH_KEYS: [&str; 11] = [
    "path",
    "change",
    "peer_path",
    "base_source_graph_schema",
    "candidate_source_graph_schema",
    "base_source_revision",
    "candidate_source_revision",
    "base_source_digest",
    "candidate_source_digest",
    "base_bytes",
    "candidate_bytes",
];
const LIMIT_KEYS: [&str; 29] = [
    "max_managed_files",
    "max_operations",
    "max_affected_paths",
    "max_path_bytes",
    "max_source_bytes_per_operation",
    "max_total_base_source_bytes",
    "max_total_candidate_source_bytes",
    "max_total_supplied_source_bytes",
    "max_entry_module_bytes",
    "max_proposal_bytes",
    "max_candidate_manifest_bytes",
    "max_delta_roots",
    "max_delta_edges",
    "max_context_nodes",
    "max_impact_nodes",
    "max_impact_provenance",
    "max_impact_depth",
    "max_analysis_builder_bytes",
    "max_structural_change_preview_bytes",
    "max_context_bytes",
    "max_impact_bytes",
    "max_review_bytes",
    "max_evidence_bytes",
    "max_receipt_bytes",
    "max_total_artifact_bytes",
    "max_json_depth",
    "max_retained_generations",
    "max_staging_attempts",
    "max_unexpected_inventory_entries",
];
const LIMIT_VALUES: [usize; 29] = [
    16,
    16,
    32,
    240,
    1_048_576,
    16_777_216,
    16_777_216,
    4_194_304,
    16_777_216,
    33_554_432,
    1_048_576,
    8192,
    131_072,
    16_384,
    16_384,
    65_536,
    1024,
    33_554_432,
    33_554_432,
    16_777_216,
    33_554_432,
    16_777_216,
    1_048_576,
    65_536,
    100_663_296,
    8,
    32,
    32,
    0,
];
const BUDGET_KEYS: [&str; 31] = [
    "used_base_managed_files",
    "used_candidate_managed_files",
    "used_operations",
    "used_affected_paths",
    "used_created_files",
    "used_deleted_files",
    "used_moved_files",
    "used_replaced_files",
    "used_total_base_source_bytes",
    "used_total_candidate_source_bytes",
    "used_total_supplied_source_bytes",
    "used_entry_module_bytes",
    "used_proposal_bytes",
    "used_candidate_manifest_bytes",
    "used_delta_roots",
    "used_delta_edges",
    "used_context_nodes",
    "used_impact_nodes",
    "used_impact_provenance",
    "used_impact_depth",
    "used_analysis_builder_bytes",
    "used_structural_change_preview_bytes",
    "used_context_bytes",
    "used_impact_bytes",
    "used_review_bytes",
    "used_evidence_bytes",
    "used_receipt_bytes",
    "used_total_artifact_bytes",
    "used_retained_generations",
    "used_staging_attempts",
    "used_unexpected_inventory_entries",
];
const NONCLAIMS: [&str; 23] = [
    "not_signature_or_authenticated_provenance",
    "not_human_approval_or_policy",
    "not_safe_compatible_or_target_verified",
    "no_reusable_authorization_token",
    "no_test_or_target_execution",
    "no_target_evidence_or_machine_code_claim",
    "no_current_state_context_impact_or_review_reuse",
    "no_raw_path_create_delete_move_or_write",
    "no_existing_generation_mutation_deletion_or_cleanup",
    "no_automatic_identity_preservation_across_move",
    "no_move_swap_chain_cycle_or_destination_vacating",
    "no_typed_stable_id_operation_language",
    "no_unmanaged_path_or_raw_tree_authority",
    "no_raw_tree_git_or_editor_atomic_visibility",
    "no_commit_authority_in_preview_context_impact_review_or_evidence",
    "no_automatic_rollback_cleanup_or_gc",
    "no_power_loss_durability_guarantee",
    "no_network_distributed_nfs_or_overlay_guarantee",
    "no_acl_xattr_ads_preservation",
    "no_general_proof_system",
    "no_persistence_or_incrementality",
    "no_external_consumer_compatibility",
    "no_new_language_graph_cleanup_backend_or_runtime_semantics",
];

pub(super) struct SubmittedEvidence;

pub(super) fn read_evidence(path: &Path) -> Result<String, Vec<Diagnostic>> {
    let mut file = File::open(path).map_err(|_| evidence_io("open failed"))?;
    let metadata = file
        .metadata()
        .map_err(|_| evidence_io("metadata inspection failed"))?;
    if !metadata.is_file() {
        return Err(evidence_io("input is not a regular file"));
    }
    if metadata.len() > MAX_EVIDENCE_BYTES as u64 {
        return Err(limit("evidence_bytes", MAX_EVIDENCE_BYTES));
    }
    let mut bytes = Vec::with_capacity((metadata.len() as usize).saturating_add(1));
    file.by_ref()
        .take((MAX_EVIDENCE_BYTES as u64) + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| evidence_io("read failed"))?;
    if bytes.len() > MAX_EVIDENCE_BYTES {
        return Err(limit("evidence_bytes", MAX_EVIDENCE_BYTES));
    }
    String::from_utf8(bytes).map_err(|_| evidence_io("input is not UTF-8"))
}

pub(super) fn parse_evidence(source: &str) -> Result<SubmittedEvidence, Vec<Diagnostic>> {
    if source.len() > MAX_EVIDENCE_BYTES {
        return Err(limit("evidence_bytes", MAX_EVIDENCE_BYTES));
    }
    if source.as_bytes().first() == Some(&0xef)
        || source.contains('\r')
        || !source.ends_with('\n')
        || source[..source.len() - 1].contains('\n')
    {
        return Err(format_error());
    }
    let body = &source[..source.len() - 1];
    validate_json_depth(body)?;
    let value: Value = serde_json::from_str(body).map_err(|_| format_error())?;
    let generic_top = object(&value)?;
    let schema = generic_top
        .get("schema")
        .and_then(Value::as_str)
        .ok_or_else(value_type_error)?;
    if matches!(
        schema,
        VERIFICATION_RECEIPT_SCHEMA | APPLICATION_RECEIPT_SCHEMA
    ) || schema != EVIDENCE_SCHEMA
    {
        return Err(format_error());
    }
    let top = exact_object(&value, &TOP_KEYS)?;
    validate_text_fields(
        top,
        &[
            "workspace_manifest_schema",
            "base_workspace_revision",
            "candidate_workspace_revision",
            "entry_module",
        ],
    )?;
    digest(top, "base_workspace_revision")?;
    digest(top, "candidate_workspace_revision")?;
    for key in [
        "proposal",
        "candidate_manifest",
        "structural_change_preview",
        "context",
        "impact",
        "review",
    ] {
        validate_ref(&top[key])?;
    }
    for key in ["base_workspace_graph", "candidate_workspace_graph"] {
        validate_graph_ref(&top[key])?;
    }
    validate_paths(&top["paths"])?;
    validate_number_object(&top["limits"], &LIMIT_KEYS)?;
    validate_number_object(&top["budget"], &BUDGET_KEYS)?;
    validate_nonclaims_shape(&top["nonclaims"])?;
    if render_canonical(top)? != source {
        return Err(format_error());
    }
    validate_claim_bindings(top, source.len())?;
    Ok(SubmittedEvidence)
}

pub(super) fn verify_replay(
    _submitted: &SubmittedEvidence,
    source: &str,
    artifacts: &SemanticWorkspaceStructuralChangeArtifacts,
) -> Result<(), Vec<Diagnostic>> {
    if source == artifacts.evidence() {
        Ok(())
    } else {
        Err(replay_error())
    }
}

fn validate_claim_bindings(
    top: &Map<String, Value>,
    source_len: usize,
) -> Result<(), Vec<Diagnostic>> {
    let limits = object(&top["limits"])?;
    if LIMIT_KEYS
        .iter()
        .zip(LIMIT_VALUES)
        .any(|(key, expected)| number(limits, key).ok() != Some(expected))
    {
        return Err(replay_error());
    }
    let nonclaims = top["nonclaims"].as_array().ok_or_else(value_type_error)?;
    if nonclaims.len() != NONCLAIMS.len()
        || nonclaims
            .iter()
            .zip(NONCLAIMS)
            .any(|(actual, expected)| actual.as_str() != Some(expected))
    {
        return Err(replay_error());
    }
    let budget = object(&top["budget"])?;
    for (field, maximum) in [
        ("used_base_managed_files", 16),
        ("used_candidate_managed_files", 16),
        ("used_operations", 16),
        ("used_affected_paths", 32),
        ("used_created_files", 16),
        ("used_deleted_files", 16),
        ("used_moved_files", 16),
        ("used_replaced_files", 16),
        ("used_total_base_source_bytes", 16_777_216),
        ("used_total_candidate_source_bytes", 16_777_216),
        ("used_total_supplied_source_bytes", 4_194_304),
        ("used_entry_module_bytes", 16_777_216),
        ("used_proposal_bytes", 33_554_432),
        ("used_candidate_manifest_bytes", 1_048_576),
        ("used_delta_roots", 8192),
        ("used_delta_edges", 131_072),
        ("used_context_nodes", 16_384),
        ("used_impact_nodes", 16_384),
        ("used_impact_provenance", 65_536),
        ("used_impact_depth", 1024),
        ("used_analysis_builder_bytes", 33_554_432),
        ("used_structural_change_preview_bytes", 33_554_432),
        ("used_context_bytes", 16_777_216),
        ("used_impact_bytes", 33_554_432),
        ("used_review_bytes", 16_777_216),
        ("used_evidence_bytes", 1_048_576),
        ("used_receipt_bytes", 65_536),
        ("used_total_artifact_bytes", 100_663_296),
        ("used_retained_generations", 32),
        ("used_staging_attempts", 32),
        ("used_unexpected_inventory_entries", 0),
    ] {
        if number(budget, field)? > maximum {
            return Err(replay_error());
        }
    }
    if number(budget, "used_evidence_bytes")? != source_len
        || number(budget, "used_receipt_bytes")? != 0
        || number(budget, "used_unexpected_inventory_entries")? != 0
    {
        return Err(replay_error());
    }
    let proposal_bytes = ref_bytes(&top["proposal"])?;
    let preview_bytes = ref_bytes(&top["structural_change_preview"])?;
    let context_bytes = ref_bytes(&top["context"])?;
    let impact_bytes = ref_bytes(&top["impact"])?;
    let review_bytes = ref_bytes(&top["review"])?;
    let expected_total = [
        proposal_bytes,
        preview_bytes,
        context_bytes,
        impact_bytes,
        review_bytes,
        source_len,
    ]
    .into_iter()
    .try_fold(0usize, usize::checked_add)
    .ok_or_else(replay_error)?;
    let paths = top["paths"].as_array().ok_or_else(value_type_error)?;
    if number(budget, "used_total_artifact_bytes")? != expected_total
        || number(budget, "used_affected_paths")? != paths.len()
        || number(budget, "used_proposal_bytes")? != proposal_bytes
        || number(budget, "used_candidate_manifest_bytes")?
            != ref_bytes(&top["candidate_manifest"])?
        || number(budget, "used_structural_change_preview_bytes")? != preview_bytes
        || number(budget, "used_context_bytes")? != context_bytes
        || number(budget, "used_impact_bytes")? != impact_bytes
        || number(budget, "used_review_bytes")? != review_bytes
    {
        return Err(replay_error());
    }
    let operation_count = [
        "used_created_files",
        "used_deleted_files",
        "used_moved_files",
        "used_replaced_files",
    ]
    .into_iter()
    .try_fold(0usize, |total, field| {
        total.checked_add(number(budget, field).ok()?)
    })
    .ok_or_else(replay_error)?;
    if operation_count != number(budget, "used_operations")? {
        return Err(replay_error());
    }
    Ok(())
}

fn validate_ref(value: &Value) -> Result<(), Vec<Diagnostic>> {
    let object = exact_object(value, &REF_KEYS)?;
    validate_text_fields(object, &["schema", "digest"])?;
    digest(object, "digest")?;
    number(object, "bytes")?;
    Ok(())
}

fn validate_graph_ref(value: &Value) -> Result<(), Vec<Diagnostic>> {
    let object = exact_object(value, &GRAPH_REF_KEYS)?;
    validate_text_fields(object, &["schema", "digest"])?;
    digest(object, "digest")?;
    Ok(())
}

fn validate_paths(value: &Value) -> Result<(), Vec<Diagnostic>> {
    let values = value.as_array().ok_or_else(value_type_error)?;
    if values.is_empty() || values.len() > 32 {
        return Err(format_error());
    }
    let mut prior = None::<&str>;
    for value in values {
        let path = exact_object(value, &PATH_KEYS)?;
        string(path, "path")?;
        string(path, "change")?;
        optional_string(path, "peer_path")?;
        for key in [
            "base_source_graph_schema",
            "candidate_source_graph_schema",
            "base_source_revision",
            "candidate_source_revision",
            "base_source_digest",
            "candidate_source_digest",
        ] {
            optional_string(path, key)?;
        }
        for key in [
            "base_source_revision",
            "candidate_source_revision",
            "base_source_digest",
            "candidate_source_digest",
        ] {
            optional_digest(path, key)?;
        }
        optional_number(path, "base_bytes")?;
        optional_number(path, "candidate_bytes")?;
        let value = string(path, "path")?;
        if prior.is_some_and(|previous| previous >= value) {
            return Err(format_error());
        }
        prior = Some(value);
    }
    Ok(())
}

fn validate_number_object(value: &Value, keys: &[&str]) -> Result<(), Vec<Diagnostic>> {
    let object = exact_object(value, keys)?;
    for key in keys {
        number(object, key)?;
    }
    Ok(())
}

fn validate_nonclaims_shape(value: &Value) -> Result<(), Vec<Diagnostic>> {
    let values = value.as_array().ok_or_else(value_type_error)?;
    if values.len() != NONCLAIMS.len() || values.iter().any(|value| value.as_str().is_none()) {
        return Err(format_error());
    }
    Ok(())
}

fn render_canonical(top: &Map<String, Value>) -> Result<String, Vec<Diagnostic>> {
    let mut output = String::new();
    output.push('{');
    for (index, key) in TOP_KEYS.iter().enumerate() {
        if index != 0 {
            output.push(',');
        }
        push_scalar(&mut output, &Value::String((*key).to_owned()))?;
        output.push(':');
        match *key {
            "proposal"
            | "candidate_manifest"
            | "structural_change_preview"
            | "context"
            | "impact"
            | "review" => {
                push_object(&mut output, object(&top[*key])?, &REF_KEYS)?;
            }
            "base_workspace_graph" | "candidate_workspace_graph" => {
                push_object(&mut output, object(&top[*key])?, &GRAPH_REF_KEYS)?;
            }
            "paths" => {
                output.push('[');
                for (path_index, path) in top[*key]
                    .as_array()
                    .ok_or_else(value_type_error)?
                    .iter()
                    .enumerate()
                {
                    if path_index != 0 {
                        output.push(',');
                    }
                    push_object(&mut output, object(path)?, &PATH_KEYS)?;
                }
                output.push(']');
            }
            "limits" => push_object(&mut output, object(&top[*key])?, &LIMIT_KEYS)?,
            "budget" => push_object(&mut output, object(&top[*key])?, &BUDGET_KEYS)?,
            _ => push_scalar(&mut output, &top[*key])?,
        }
    }
    output.push_str("}\n");
    Ok(output)
}

fn push_object(
    output: &mut String,
    object: &Map<String, Value>,
    keys: &[&str],
) -> Result<(), Vec<Diagnostic>> {
    output.push('{');
    for (index, key) in keys.iter().enumerate() {
        if index != 0 {
            output.push(',');
        }
        push_scalar(output, &Value::String((*key).to_owned()))?;
        output.push(':');
        push_scalar(output, &object[*key])?;
    }
    output.push('}');
    Ok(())
}

fn push_scalar(output: &mut String, value: &Value) -> Result<(), Vec<Diagnostic>> {
    output.push_str(&serde_json::to_string(value).map_err(|_| value_type_error())?);
    Ok(())
}

fn validate_json_depth(source: &str) -> Result<(), Vec<Diagnostic>> {
    let mut stack = Vec::with_capacity(MAX_JSON_DEPTH);
    let mut in_string = false;
    let mut escaped = false;
    for byte in source.bytes() {
        if in_string {
            if escaped {
                escaped = false;
            } else if byte == b'\\' {
                escaped = true;
            } else if byte == b'"' {
                in_string = false;
            }
            continue;
        }
        match byte {
            b'"' => in_string = true,
            b'{' | b'[' => {
                if stack.len() == MAX_JSON_DEPTH {
                    return Err(format_error());
                }
                stack.push(byte);
            }
            b'}' if stack.pop() != Some(b'{') => return Err(format_error()),
            b']' if stack.pop() != Some(b'[') => return Err(format_error()),
            b'}' | b']' => {}
            _ => {}
        }
    }
    if in_string || escaped || !stack.is_empty() {
        return Err(format_error());
    }
    Ok(())
}

fn exact_object<'a>(
    value: &'a Value,
    keys: &[&str],
) -> Result<&'a Map<String, Value>, Vec<Diagnostic>> {
    let object = object(value)?;
    if object.len() != keys.len() || keys.iter().any(|key| !object.contains_key(*key)) {
        return Err(format_error());
    }
    Ok(object)
}

fn object(value: &Value) -> Result<&Map<String, Value>, Vec<Diagnostic>> {
    value.as_object().ok_or_else(value_type_error)
}

fn validate_text_fields(object: &Map<String, Value>, keys: &[&str]) -> Result<(), Vec<Diagnostic>> {
    for key in keys {
        string(object, key)?;
    }
    Ok(())
}

fn string<'a>(object: &'a Map<String, Value>, key: &str) -> Result<&'a str, Vec<Diagnostic>> {
    object[key].as_str().ok_or_else(value_type_error)
}

fn optional_string<'a>(
    object: &'a Map<String, Value>,
    key: &str,
) -> Result<Option<&'a str>, Vec<Diagnostic>> {
    if object[key].is_null() {
        Ok(None)
    } else {
        string(object, key).map(Some)
    }
}

fn number(object: &Map<String, Value>, key: &str) -> Result<usize, Vec<Diagnostic>> {
    object[key]
        .as_u64()
        .and_then(|value| usize::try_from(value).ok())
        .ok_or_else(value_type_error)
}

fn optional_number(object: &Map<String, Value>, key: &str) -> Result<(), Vec<Diagnostic>> {
    if !object[key].is_null() {
        number(object, key)?;
    }
    Ok(())
}

fn digest(object: &Map<String, Value>, key: &str) -> Result<(), Vec<Diagnostic>> {
    validate_digest_value(string(object, key)?)
}

fn optional_digest(object: &Map<String, Value>, key: &str) -> Result<(), Vec<Diagnostic>> {
    if let Some(value) = optional_string(object, key)? {
        validate_digest_value(value)?;
    }
    Ok(())
}

fn validate_digest_value(value: &str) -> Result<(), Vec<Diagnostic>> {
    if value.len() != 71
        || !value.starts_with("sha256:")
        || !value[7..]
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        return Err(value_type_error());
    }
    Ok(())
}

fn ref_bytes(value: &Value) -> Result<usize, Vec<Diagnostic>> {
    number(object(value)?, "bytes")
}

fn format_error() -> Vec<Diagnostic> {
    vec![Diagnostic::io("SPX-G193", FORMAT_LEAD)]
}

fn value_type_error() -> Vec<Diagnostic> {
    vec![Diagnostic::io(
        "SPX-G193",
        format!("{FORMAT_LEAD}: value type is invalid"),
    )]
}

fn replay_error() -> Vec<Diagnostic> {
    vec![Diagnostic::io("SPX-G195", REPLAY_MESSAGE)]
}

fn evidence_io(detail: &'static str) -> Vec<Diagnostic> {
    vec![Diagnostic::io(
        "SPX-I215",
        format!("could not read Semantic Workspace Structural Change Evidence: {detail}"),
    )]
}
