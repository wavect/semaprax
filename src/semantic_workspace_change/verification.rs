//! Strict, non-authoritative parsing of submitted Change Evidence capsules.

use std::fs::File;
use std::io::Read as _;
use std::path::Path;

use serde_json::{Map, Value};

use super::artifact::{
    SemanticWorkspaceChangeArtifacts, EVIDENCE_SCHEMA, MAX_EVIDENCE_BYTES, RECEIPT_SCHEMA,
};
use super::{limit, MAX_CHANGED_FILES};
use crate::diagnostic::Diagnostic;

const FORMAT_LEAD: &str =
    "Semantic Workspace Change Evidence must be one canonical JSON line with one terminal LF";
const REPLAY_MESSAGE: &str =
    "Semantic Workspace Change Evidence does not exactly replay the authenticated proposal and candidate";
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
    "change_preview",
    "context",
    "impact",
    "review",
    "files",
    "limits",
    "budget",
    "nonclaims",
];
const REF_KEYS: [&str; 3] = ["schema", "digest", "bytes"];
const GRAPH_REF_KEYS: [&str; 2] = ["schema", "digest"];
const FILE_KEYS: [&str; 9] = [
    "path",
    "base_source_graph_schema",
    "candidate_source_graph_schema",
    "base_source_revision",
    "candidate_source_revision",
    "base_source_digest",
    "candidate_source_digest",
    "base_bytes",
    "candidate_bytes",
];
const LIMIT_KEYS: [&str; 27] = [
    "max_managed_files",
    "max_changed_files",
    "max_source_bytes_per_change",
    "max_total_base_source_bytes",
    "max_total_candidate_source_bytes",
    "max_total_replacement_source_bytes",
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
    "max_change_preview_bytes",
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
const LIMIT_VALUES: [usize; 27] = [
    16,
    16,
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
const BUDGET_KEYS: [&str; 25] = [
    "used_managed_files",
    "used_changed_files",
    "used_total_base_source_bytes",
    "used_total_candidate_source_bytes",
    "used_total_replacement_source_bytes",
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
    "used_change_preview_bytes",
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
const NONCLAIMS: [&str; 19] = [
    "not_signature_or_authenticated_provenance",
    "not_human_approval_or_policy",
    "not_safe_compatible_or_target_verified",
    "no_reusable_authorization_token",
    "no_test_or_target_execution",
    "no_target_evidence_or_machine_code_claim",
    "no_current_state_context_impact_or_review_reuse",
    "no_create_delete_move_or_path_set_change",
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
        return Err(format_error(None));
    }
    let body = &source[..source.len() - 1];
    validate_json_depth(body)?;
    let value: Value =
        serde_json::from_str(body).map_err(|_| format_error(Some("UTF-8 JSON is invalid")))?;
    let generic_top = object(&value)?;
    let schema = generic_top
        .get("schema")
        .and_then(Value::as_str)
        .ok_or_else(value_type_error)?;
    if schema == RECEIPT_SCHEMA {
        return Err(format_error(Some(
            "receipt and capsule schemas are confused",
        )));
    }
    if schema != EVIDENCE_SCHEMA {
        return Err(format_error(Some("schema is unsupported")));
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
        "change_preview",
        "context",
        "impact",
        "review",
    ] {
        validate_ref(&top[key])?;
    }
    for key in ["base_workspace_graph", "candidate_workspace_graph"] {
        validate_graph_ref(&top[key])?;
    }
    validate_files(&top["files"])?;
    validate_number_object(&top["limits"], &LIMIT_KEYS)?;
    validate_number_object(&top["budget"], &BUDGET_KEYS)?;
    validate_nonclaims_shape(&top["nonclaims"])?;
    let canonical = render_canonical(top)?;
    if canonical != source {
        return Err(format_error(None));
    }
    validate_claim_bindings(top, source.len())?;
    Ok(SubmittedEvidence)
}

pub(super) fn verify_replay(
    _submitted: &SubmittedEvidence,
    source: &str,
    artifacts: &SemanticWorkspaceChangeArtifacts,
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
    let maxima = [
        ("used_managed_files", 16),
        ("used_changed_files", 16),
        ("used_total_base_source_bytes", 16_777_216),
        ("used_total_candidate_source_bytes", 16_777_216),
        ("used_total_replacement_source_bytes", 4_194_304),
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
        ("used_change_preview_bytes", 33_554_432),
        ("used_context_bytes", 16_777_216),
        ("used_impact_bytes", 33_554_432),
        ("used_review_bytes", 16_777_216),
        ("used_evidence_bytes", 1_048_576),
        ("used_receipt_bytes", 65_536),
        ("used_total_artifact_bytes", 100_663_296),
        ("used_retained_generations", 32),
        ("used_staging_attempts", 32),
        ("used_unexpected_inventory_entries", 0),
    ];
    for (field, maximum) in maxima {
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
    let preview_bytes = ref_bytes(&top["change_preview"])?;
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
    if number(budget, "used_total_artifact_bytes")? != expected_total {
        return Err(replay_error());
    }
    let files = top["files"].as_array().ok_or_else(value_type_error)?;
    if number(budget, "used_changed_files")? != files.len()
        || number(budget, "used_proposal_bytes")? != proposal_bytes
        || number(budget, "used_candidate_manifest_bytes")?
            != ref_bytes(&top["candidate_manifest"])?
        || number(budget, "used_change_preview_bytes")? != preview_bytes
        || number(budget, "used_context_bytes")? != context_bytes
        || number(budget, "used_impact_bytes")? != impact_bytes
        || number(budget, "used_review_bytes")? != review_bytes
    {
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

fn validate_files(value: &Value) -> Result<(), Vec<Diagnostic>> {
    let values = value.as_array().ok_or_else(value_type_error)?;
    if values.len() < 2 || values.len() > MAX_CHANGED_FILES {
        return Err(format_error(Some(
            "array order or uniqueness is noncanonical",
        )));
    }
    let mut prior = None::<&str>;
    for value in values {
        let file = exact_object(value, &FILE_KEYS)?;
        validate_text_fields(
            file,
            &[
                "path",
                "base_source_graph_schema",
                "candidate_source_graph_schema",
                "base_source_revision",
                "candidate_source_revision",
                "base_source_digest",
                "candidate_source_digest",
            ],
        )?;
        for key in [
            "base_source_revision",
            "candidate_source_revision",
            "base_source_digest",
            "candidate_source_digest",
        ] {
            digest(file, key)?;
        }
        number(file, "base_bytes")?;
        number(file, "candidate_bytes")?;
        let path = string(file, "path")?;
        if prior.is_some_and(|previous| previous >= path) {
            return Err(format_error(Some(
                "array order or uniqueness is noncanonical",
            )));
        }
        prior = Some(path);
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
        return Err(format_error(Some(
            "array order or uniqueness is noncanonical",
        )));
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
            "proposal" | "candidate_manifest" | "change_preview" | "context" | "impact"
            | "review" => push_object(&mut output, object(&top[*key])?, &REF_KEYS)?,
            "base_workspace_graph" | "candidate_workspace_graph" => {
                push_object(&mut output, object(&top[*key])?, &GRAPH_REF_KEYS)?;
            }
            "files" => {
                output.push('[');
                for (file_index, file) in top[*key]
                    .as_array()
                    .ok_or_else(value_type_error)?
                    .iter()
                    .enumerate()
                {
                    if file_index != 0 {
                        output.push(',');
                    }
                    push_object(&mut output, object(file)?, &FILE_KEYS)?;
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
                    return Err(format_error(Some("JSON depth exceeds 8")));
                }
                stack.push(byte);
            }
            b'}' if stack.pop() != Some(b'{') => return Err(format_error(None)),
            b']' if stack.pop() != Some(b'[') => return Err(format_error(None)),
            b'}' | b']' => {}
            _ => {}
        }
    }
    if in_string || escaped || !stack.is_empty() {
        return Err(format_error(None));
    }
    Ok(())
}

fn exact_object<'a>(
    value: &'a Value,
    keys: &[&str],
) -> Result<&'a Map<String, Value>, Vec<Diagnostic>> {
    let object = object(value)?;
    if object.len() != keys.len() || keys.iter().any(|key| !object.contains_key(*key)) {
        return Err(format_error(Some("object keys are noncanonical")));
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

fn number(object: &Map<String, Value>, key: &str) -> Result<usize, Vec<Diagnostic>> {
    object[key]
        .as_u64()
        .and_then(|value| usize::try_from(value).ok())
        .ok_or_else(value_type_error)
}

fn digest(object: &Map<String, Value>, key: &str) -> Result<(), Vec<Diagnostic>> {
    let value = string(object, key)?;
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

fn format_error(suffix: Option<&str>) -> Vec<Diagnostic> {
    let message = suffix.map_or_else(
        || FORMAT_LEAD.to_owned(),
        |suffix| format!("{FORMAT_LEAD}: {suffix}"),
    );
    vec![Diagnostic::io("SPX-G185", message)]
}

fn value_type_error() -> Vec<Diagnostic> {
    format_error(Some("value type is invalid"))
}

fn replay_error() -> Vec<Diagnostic> {
    vec![Diagnostic::io("SPX-G187", REPLAY_MESSAGE)]
}

fn evidence_io(detail: &'static str) -> Vec<Diagnostic> {
    vec![Diagnostic::io(
        "SPX-I214",
        format!("could not read Semantic Workspace Change Evidence: {detail}"),
    )]
}
