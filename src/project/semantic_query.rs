//! Closed, canonical Universal Semantic Query v1 over one immutable service snapshot.

use serde_json::{json, Map, Value};
use sha2::{Digest, Sha256};

use crate::diagnostic::Diagnostic;
use crate::query::QueryFilters;
use crate::workspace_analysis::{
    WorkspaceAnalysisDirection, WorkspaceAnalysisTargetKind, WorkspaceContextOptions,
    WorkspaceImpactOptions,
};

use super::semantic_transaction::rename_display_name_eligibility;
use super::{SemanticWorkspaceSnapshot, SEMANTIC_TRANSACTION_SCHEMA};

pub const SEMANTIC_QUERY_SCHEMA: &str = "semaprax.semantic-query.v1";
pub const SEMANTIC_QUERY_RESULT_SCHEMA: &str = "semaprax.semantic-query-result.v1";
pub const SEMANTIC_QUERY_DECLARATIONS_SCHEMA: &str = "semaprax.semantic-query-declarations.v1";
pub const SEMANTIC_QUERY_AVAILABLE_OPERATIONS_SCHEMA: &str =
    "semaprax.semantic-query-available-operations.v1";
pub const MAX_SEMANTIC_QUERY_BYTES: usize = 65_536;
pub const MAX_SEMANTIC_QUERY_RESULT_BYTES: usize = 32 * 1024 * 1024;

const QUERY_DOMAIN: &[u8] = b"semaprax.semantic-query.intent.digest.v1\0";
const RESULT_DOMAIN: &[u8] = b"semaprax.semantic-query.result.digest.v1\0";
const SYMBOL_PAYLOAD_DOMAIN: &[u8] = b"semaprax.semantic-query.symbol.payload.digest.v1\0";
const CONTEXT_PAYLOAD_DOMAIN: &[u8] = b"semaprax.semantic-query.context.payload.digest.v1\0";
const IMPACT_PAYLOAD_DOMAIN: &[u8] = b"semaprax.semantic-query.impact.payload.digest.v1\0";
const DECLARATIONS_PAYLOAD_DOMAIN: &[u8] =
    b"semaprax.semantic-query.declarations.payload.digest.v1\0";
const AVAILABLE_OPERATIONS_PAYLOAD_DOMAIN: &[u8] =
    b"semaprax.semantic-query.available-operations.payload.digest.v1\0";
const MAX_TARGET_BYTES: usize = 4096;
pub const MAX_SEMANTIC_QUERY_DECLARATION_OFFSET: usize = 16_384;
pub const MAX_SEMANTIC_QUERY_DECLARATION_LIMIT: usize = 128;
const NONCLAIMS: &[&str] = &[
    "derived_read_only_projection",
    "no_source_execution_or_publication_authority",
    "not_behavioral_equivalence_or_complete_repository_analysis",
];
type Result<T> = std::result::Result<T, Vec<Diagnostic>>;

#[derive(Clone, Debug, Eq, PartialEq)]
enum Operation {
    Declarations {
        filters: QueryFilters,
        offset: usize,
        limit: usize,
    },
    Symbol {
        stable_id: String,
    },
    Context {
        target_kind: WorkspaceAnalysisTargetKind,
        target: String,
        options: WorkspaceContextOptions,
    },
    Impact {
        target_kind: WorkspaceAnalysisTargetKind,
        target: String,
        options: WorkspaceImpactOptions,
    },
    AvailableOperations {
        stable_id: String,
    },
}

impl Operation {
    fn name(&self) -> &'static str {
        match self {
            Self::Declarations { .. } => "declarations",
            Self::Symbol { .. } => "symbol",
            Self::Context { .. } => "context",
            Self::Impact { .. } => "impact",
            Self::AvailableOperations { .. } => "available_operations",
        }
    }

    fn payload_domain(&self) -> &'static [u8] {
        match self {
            Self::Declarations { .. } => DECLARATIONS_PAYLOAD_DOMAIN,
            Self::Symbol { .. } => SYMBOL_PAYLOAD_DOMAIN,
            Self::Context { .. } => CONTEXT_PAYLOAD_DOMAIN,
            Self::Impact { .. } => IMPACT_PAYLOAD_DOMAIN,
            Self::AvailableOperations { .. } => AVAILABLE_OPERATIONS_PAYLOAD_DOMAIN,
        }
    }

    fn value(&self) -> Value {
        match self {
            Self::Declarations {
                filters,
                offset,
                limit,
            } => json!({
                "filters": filters_value(filters),
                "kind": "declarations",
                "limit": limit,
                "offset": offset,
            }),
            Self::Symbol { stable_id } => {
                json!({"kind": "symbol", "stable_id": stable_id})
            }
            Self::Context {
                target_kind,
                target,
                options,
            } => {
                let (direction, depth, max_bytes, max_nodes) = options.semantic_query_parts();
                json!({
                    "depth": depth,
                    "direction": direction_name(direction),
                    "kind": "context",
                    "max_bytes": max_bytes,
                    "max_nodes": max_nodes,
                    "target": target,
                    "target_kind": target_kind_name(*target_kind),
                })
            }
            Self::Impact {
                target_kind,
                target,
                options,
            } => {
                let (depth, max_bytes, max_nodes) = options.semantic_query_parts();
                json!({
                    "depth": depth,
                    "kind": "impact",
                    "max_bytes": max_bytes,
                    "max_nodes": max_nodes,
                    "target": target,
                    "target_kind": target_kind_name(*target_kind),
                })
            }
            Self::AvailableOperations { stable_id } => {
                json!({"kind": "available_operations", "stable_id": stable_id})
            }
        }
    }
}

/// One exact canonical read intention over a canonical semantic workspace revision.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SemanticQuery {
    expected_workspace_revision: String,
    operation: Operation,
    json: String,
    digest: String,
}

impl SemanticQuery {
    pub fn declarations(
        expected_workspace_revision: &str,
        filters: &QueryFilters,
        offset: usize,
        limit: usize,
    ) -> Result<Self> {
        validate_digest(expected_workspace_revision)?;
        if offset > MAX_SEMANTIC_QUERY_DECLARATION_OFFSET
            || limit == 0
            || limit > MAX_SEMANTIC_QUERY_DECLARATION_LIMIT
        {
            return Err(invalid("semantic declaration query paging is invalid"));
        }
        let filters = normalize_filters(filters)?;
        Self::new(
            expected_workspace_revision,
            Operation::Declarations {
                filters,
                offset,
                limit,
            },
        )
    }

    pub fn symbol(expected_workspace_revision: &str, stable_id: &str) -> Result<Self> {
        validate_digest(expected_workspace_revision)?;
        validate_target(stable_id)?;
        Self::new(
            expected_workspace_revision,
            Operation::Symbol {
                stable_id: stable_id.to_owned(),
            },
        )
    }

    pub fn context(
        expected_workspace_revision: &str,
        target_kind: WorkspaceAnalysisTargetKind,
        target: &str,
        options: WorkspaceContextOptions,
    ) -> Result<Self> {
        validate_digest(expected_workspace_revision)?;
        validate_target(target)?;
        Self::new(
            expected_workspace_revision,
            Operation::Context {
                target_kind,
                target: target.to_owned(),
                options,
            },
        )
    }

    pub fn impact(
        expected_workspace_revision: &str,
        target_kind: WorkspaceAnalysisTargetKind,
        target: &str,
        options: WorkspaceImpactOptions,
    ) -> Result<Self> {
        validate_digest(expected_workspace_revision)?;
        validate_target(target)?;
        Self::new(
            expected_workspace_revision,
            Operation::Impact {
                target_kind,
                target: target.to_owned(),
                options,
            },
        )
    }

    pub fn available_operations(
        expected_workspace_revision: &str,
        stable_id: &str,
    ) -> Result<Self> {
        validate_digest(expected_workspace_revision)?;
        validate_target(stable_id)?;
        Self::new(
            expected_workspace_revision,
            Operation::AvailableOperations {
                stable_id: stable_id.to_owned(),
            },
        )
    }

    fn new(expected_workspace_revision: &str, operation: Operation) -> Result<Self> {
        let json = render(
            json!({
                "expected_workspace_revision": expected_workspace_revision,
                "operation": operation.value(),
                "schema": SEMANTIC_QUERY_SCHEMA,
            }),
            MAX_SEMANTIC_QUERY_BYTES,
            true,
        )?;
        Ok(Self {
            expected_workspace_revision: expected_workspace_revision.to_owned(),
            operation,
            digest: hash(QUERY_DOMAIN, json.as_bytes()),
            json,
        })
    }

    /// Admit only an exact canonical, closed v1 query envelope.
    pub fn from_json(bytes: &[u8]) -> Result<Self> {
        if bytes.len() > MAX_SEMANTIC_QUERY_BYTES {
            return Err(capacity("semantic query exceeds its byte limit"));
        }
        let value: Value = serde_json::from_slice(bytes)
            .map_err(|_| invalid("semantic query is not valid JSON"))?;
        let object = exact_object(
            &value,
            &["expected_workspace_revision", "operation", "schema"],
        )?;
        if object["schema"] != SEMANTIC_QUERY_SCHEMA {
            return Err(invalid("semantic query schema is unsupported"));
        }
        let expected = object["expected_workspace_revision"]
            .as_str()
            .ok_or_else(|| invalid("semantic query expected revision is invalid"))?;
        let operation = object["operation"]
            .as_object()
            .ok_or_else(|| invalid("semantic query operation is not an object"))?;
        let kind = operation
            .get("kind")
            .and_then(Value::as_str)
            .ok_or_else(|| invalid("semantic query operation kind is invalid"))?;
        let query = match kind {
            "declarations" => {
                exact_map(operation, &["filters", "kind", "limit", "offset"])?;
                let filters = parse_filters(&operation["filters"])?;
                Self::declarations(
                    expected,
                    &filters,
                    integer(operation, "offset")?,
                    integer(operation, "limit")?,
                )?
            }
            "symbol" => {
                exact_map(operation, &["kind", "stable_id"])?;
                Self::symbol(expected, text(operation, "stable_id")?)?
            }
            "context" => {
                exact_map(
                    operation,
                    &[
                        "depth",
                        "direction",
                        "kind",
                        "max_bytes",
                        "max_nodes",
                        "target",
                        "target_kind",
                    ],
                )?;
                let options = context_options(
                    parse_direction(text(operation, "direction")?)?,
                    integer(operation, "depth")?,
                    integer(operation, "max_bytes")?,
                    integer(operation, "max_nodes")?,
                )?;
                Self::context(
                    expected,
                    parse_target_kind(text(operation, "target_kind")?)?,
                    text(operation, "target")?,
                    options,
                )?
            }
            "impact" => {
                exact_map(
                    operation,
                    &[
                        "depth",
                        "kind",
                        "max_bytes",
                        "max_nodes",
                        "target",
                        "target_kind",
                    ],
                )?;
                let options = impact_options(
                    integer(operation, "depth")?,
                    integer(operation, "max_bytes")?,
                    integer(operation, "max_nodes")?,
                )?;
                Self::impact(
                    expected,
                    parse_target_kind(text(operation, "target_kind")?)?,
                    text(operation, "target")?,
                    options,
                )?
            }
            "available_operations" => {
                exact_map(operation, &["kind", "stable_id"])?;
                Self::available_operations(expected, text(operation, "stable_id")?)?
            }
            _ => return Err(invalid("semantic query operation is unsupported")),
        };
        if query.json.as_bytes() != bytes {
            return Err(invalid("semantic query is not exact canonical v1 JSON"));
        }
        Ok(query)
    }

    pub fn to_json(&self) -> &str {
        &self.json
    }

    pub fn query_digest(&self) -> &str {
        &self.digest
    }

    pub fn expected_workspace_revision(&self) -> &str {
        &self.expected_workspace_revision
    }

    pub fn execute(&self, snapshot: &SemanticWorkspaceSnapshot) -> Result<SemanticQueryResult> {
        if self.expected_workspace_revision != snapshot.workspace_revision() {
            return Err(stale("semantic query workspace revision is stale"));
        }
        let payload = match &self.operation {
            Operation::Declarations {
                filters,
                offset,
                limit,
            } => declarations_payload(snapshot, filters, *offset, *limit)?,
            Operation::Symbol { stable_id } => snapshot.symbol(stable_id)?,
            Operation::Context {
                target_kind,
                target,
                options,
            } => snapshot.context(*target_kind, target, *options)?,
            Operation::Impact {
                target_kind,
                target,
                options,
            } => snapshot.impact(*target_kind, target, *options)?,
            Operation::AvailableOperations { stable_id } => {
                available_operations_payload(snapshot, stable_id)?
            }
        };
        let payload_value: Value = serde_json::from_str(&payload)
            .map_err(|_| invalid("semantic query derived payload is not valid JSON"))?;
        let payload_digest = hash(self.operation.payload_domain(), payload.as_bytes());
        let generation = snapshot.generation();
        let canonical = generation.canonical();
        let json = render(
            json!({
                "authority": false,
                "component_digests": {
                    "dependency_lock": canonical.dependency_lock_digest(),
                    "manifest": canonical.manifest_digest(),
                    "semantic": canonical.semantic_digest(),
                    "source_projection": canonical.source_projection_digest(),
                },
                "image_digest": generation.image().image_digest(),
                "limits": {
                    "max_query_bytes": MAX_SEMANTIC_QUERY_BYTES,
                    "max_result_bytes": MAX_SEMANTIC_QUERY_RESULT_BYTES,
                },
                "nonclaims": NONCLAIMS,
                "operation": self.operation.name(),
                "payload": payload_value,
                "payload_digest": payload_digest,
                "project_revision": generation.revision().project_revision(),
                "query_digest": self.digest,
                "schema": SEMANTIC_QUERY_RESULT_SCHEMA,
                "workspace_revision": canonical.workspace_revision(),
            }),
            MAX_SEMANTIC_QUERY_RESULT_BYTES,
            true,
        )?;
        let digest = hash(RESULT_DOMAIN, json.as_bytes());
        Ok(SemanticQueryResult {
            json,
            digest,
            query_digest: self.digest.clone(),
            payload,
            payload_digest,
            workspace_revision: self.expected_workspace_revision.clone(),
        })
    }

    /// Freshly execute one canonical query against an immutable snapshot and
    /// exact-compare the complete canonical result.
    pub fn replay(
        snapshot: &SemanticWorkspaceSnapshot,
        query_bytes: &[u8],
        expected_result_digest: &str,
        result_bytes: &[u8],
    ) -> Result<SemanticQueryResult> {
        let query = Self::from_json(query_bytes)?;
        if result_bytes.len() > MAX_SEMANTIC_QUERY_RESULT_BYTES {
            return Err(capacity("semantic query result exceeds its byte limit"));
        }
        validate_result_wire(result_bytes)?;
        validate_digest(expected_result_digest)?;
        if hash(RESULT_DOMAIN, result_bytes) != expected_result_digest {
            return Err(stale("semantic query result digest is stale"));
        }
        let result = query.execute(snapshot)?;
        if result.result_digest() != expected_result_digest
            || result.to_json().as_bytes() != result_bytes
        {
            return Err(stale("semantic query result failed exact replay"));
        }
        Ok(result)
    }
}

/// One canonical result retaining the exact inner legacy payload bytes.
pub struct SemanticQueryResult {
    json: String,
    digest: String,
    query_digest: String,
    payload: String,
    payload_digest: String,
    workspace_revision: String,
}

impl SemanticQueryResult {
    pub fn to_json(&self) -> &str {
        &self.json
    }
    pub fn result_digest(&self) -> &str {
        &self.digest
    }
    pub fn query_digest(&self) -> &str {
        &self.query_digest
    }
    pub fn payload(&self) -> &str {
        &self.payload
    }
    pub fn payload_digest(&self) -> &str {
        &self.payload_digest
    }
    pub fn workspace_revision(&self) -> &str {
        &self.workspace_revision
    }
}

fn declarations_payload(
    snapshot: &SemanticWorkspaceSnapshot,
    filters: &QueryFilters,
    offset: usize,
    limit: usize,
) -> Result<String> {
    let result = crate::query::run_project(snapshot.generation().revision(), filters)?;
    let legacy = crate::query::project_json(&result);
    if legacy.len() > MAX_SEMANTIC_QUERY_RESULT_BYTES {
        return Err(capacity(
            "semantic declaration query source result exceeds its byte limit",
        ));
    }
    let value: Value = serde_json::from_str(&legacy)
        .map_err(|_| invalid("semantic declaration query source result is invalid"))?;
    let matches = value["matches"]
        .as_array()
        .ok_or_else(|| invalid("semantic declaration query has no match inventory"))?;
    let end = offset.saturating_add(limit).min(matches.len());
    let page = if offset < matches.len() {
        matches[offset..end].to_vec()
    } else {
        Vec::new()
    };
    render(
        json!({
            "filters": value["filters"],
            "graph_revision": value["graph_revision"],
            "limit": limit,
            "matches": page,
            "next_offset": (end < matches.len()).then_some(end),
            "offset": offset,
            "project": value["project"],
            "project_revision": value["project_revision"],
            "schema": SEMANTIC_QUERY_DECLARATIONS_SCHEMA,
            "source_schema": crate::query::PROJECT_SCHEMA_V1,
            "total_matches": matches.len(),
        }),
        MAX_SEMANTIC_QUERY_RESULT_BYTES,
        true,
    )
}

fn available_operations_payload(
    snapshot: &SemanticWorkspaceSnapshot,
    stable_id: &str,
) -> Result<String> {
    // Require an actual retained declaration before classifying the operation.
    snapshot.symbol(stable_id)?;
    let eligibility = rename_display_name_eligibility(snapshot.generation().revision(), stable_id)?;
    render(
        json!({
            "operations": [{
                "available": eligibility.available(),
                "constraints": {
                    "comment_free_canonical_workspace": eligibility.comment_free_canonical_workspace,
                    "explicit_identity": eligibility.explicit_identity,
                    "monomorphic": eligibility.monomorphic,
                    "non_main": eligibility.non_main,
                },
                "expected_old_value": eligibility.expected_old_value,
                "kind": "rename_display_name",
                "nonclaim": "availability_does_not_claim_that_an_arbitrary_new_value_validates",
                "transaction_schema": SEMANTIC_TRANSACTION_SCHEMA,
            }],
            "schema": SEMANTIC_QUERY_AVAILABLE_OPERATIONS_SCHEMA,
            "stable_id": stable_id,
        }),
        MAX_SEMANTIC_QUERY_RESULT_BYTES,
        true,
    )
}

fn normalize_filters(filters: &QueryFilters) -> Result<QueryFilters> {
    let mut kinds = filters.kinds.clone();
    kinds.sort_by_key(|kind| crate::query::KINDS.iter().position(|known| known == kind));
    kinds.dedup();
    for kind in &kinds {
        if !crate::query::KINDS.contains(&kind.as_str()) {
            return Err(invalid("semantic declaration query kind is unsupported"));
        }
    }
    for value in [
        filters.name.as_deref(),
        filters.id_prefix.as_deref(),
        filters.effect.as_deref(),
        filters.calls.as_deref(),
        filters.called_by.as_deref(),
    ]
    .into_iter()
    .flatten()
    {
        validate_text(value)?;
    }
    Ok(QueryFilters {
        kinds,
        name: filters.name.clone(),
        id_prefix: filters.id_prefix.clone(),
        effect: filters.effect.clone(),
        calls: filters.calls.clone(),
        called_by: filters.called_by.clone(),
    })
}

fn filters_value(filters: &QueryFilters) -> Value {
    json!({
        "called_by": filters.called_by,
        "calls": filters.calls,
        "effect": filters.effect,
        "id_prefix": filters.id_prefix,
        "kinds": filters.kinds,
        "name": filters.name,
    })
}

fn parse_filters(value: &Value) -> Result<QueryFilters> {
    let object = value
        .as_object()
        .ok_or_else(|| invalid("semantic declaration query filters are invalid"))?;
    exact_map(
        object,
        &["called_by", "calls", "effect", "id_prefix", "kinds", "name"],
    )?;
    let kinds = object["kinds"]
        .as_array()
        .ok_or_else(|| invalid("semantic declaration query kinds are invalid"))?
        .iter()
        .map(|value| {
            value
                .as_str()
                .map(str::to_owned)
                .ok_or_else(|| invalid("semantic declaration query kind is invalid"))
        })
        .collect::<Result<Vec<_>>>()?;
    let optional = |key: &str| -> Result<Option<String>> {
        match &object[key] {
            Value::Null => Ok(None),
            Value::String(value) => Ok(Some(value.clone())),
            _ => Err(invalid("semantic declaration query filter text is invalid")),
        }
    };
    Ok(QueryFilters {
        kinds,
        name: optional("name")?,
        id_prefix: optional("id_prefix")?,
        effect: optional("effect")?,
        calls: optional("calls")?,
        called_by: optional("called_by")?,
    })
}

fn validate_result_wire(bytes: &[u8]) -> Result<()> {
    let value: Value = serde_json::from_slice(bytes)
        .map_err(|_| invalid("semantic query result is not valid JSON"))?;
    let object = exact_object(
        &value,
        &[
            "authority",
            "component_digests",
            "image_digest",
            "limits",
            "nonclaims",
            "operation",
            "payload",
            "payload_digest",
            "project_revision",
            "query_digest",
            "schema",
            "workspace_revision",
        ],
    )?;
    if object["schema"] != SEMANTIC_QUERY_RESULT_SCHEMA
        || object["authority"] != false
        || !matches!(
            object["operation"].as_str(),
            Some("declarations" | "symbol" | "context" | "impact" | "available_operations")
        )
        || object["nonclaims"] != json!(NONCLAIMS)
    {
        return Err(invalid("semantic query result has invalid fixed fields"));
    }
    for key in [
        "image_digest",
        "payload_digest",
        "project_revision",
        "query_digest",
        "workspace_revision",
    ] {
        validate_digest(
            object[key]
                .as_str()
                .ok_or_else(|| invalid("semantic query result digest field is invalid"))?,
        )?;
    }
    let components = object["component_digests"]
        .as_object()
        .ok_or_else(|| invalid("semantic query result component digests are invalid"))?;
    exact_map(
        components,
        &[
            "dependency_lock",
            "manifest",
            "semantic",
            "source_projection",
        ],
    )?;
    for value in components.values() {
        validate_digest(
            value
                .as_str()
                .ok_or_else(|| invalid("semantic query component digest is invalid"))?,
        )?;
    }
    if render(value, MAX_SEMANTIC_QUERY_RESULT_BYTES, true)?.as_bytes() != bytes {
        return Err(invalid("semantic query result is not exact canonical JSON"));
    }
    Ok(())
}

fn exact_object<'a>(value: &'a Value, keys: &[&str]) -> Result<&'a Map<String, Value>> {
    let object = value
        .as_object()
        .ok_or_else(|| invalid("semantic query document is not an object"))?;
    exact_map(object, keys)?;
    Ok(object)
}

fn exact_map(object: &Map<String, Value>, keys: &[&str]) -> Result<()> {
    if object.len() != keys.len() || keys.iter().any(|key| !object.contains_key(*key)) {
        return Err(invalid("semantic query document has an invalid field set"));
    }
    Ok(())
}

fn text<'a>(object: &'a Map<String, Value>, key: &str) -> Result<&'a str> {
    object[key]
        .as_str()
        .ok_or_else(|| invalid("semantic query text field is invalid"))
}

fn integer(object: &Map<String, Value>, key: &str) -> Result<usize> {
    object[key]
        .as_u64()
        .and_then(|value| usize::try_from(value).ok())
        .ok_or_else(|| invalid("semantic query integer field is invalid"))
}

fn parse_target_kind(value: &str) -> Result<WorkspaceAnalysisTargetKind> {
    match value {
        "declaration" => Ok(WorkspaceAnalysisTargetKind::Declaration),
        "capability" => Ok(WorkspaceAnalysisTargetKind::Capability),
        _ => Err(invalid("semantic query target kind is unsupported")),
    }
}

fn target_kind_name(value: WorkspaceAnalysisTargetKind) -> &'static str {
    match value {
        WorkspaceAnalysisTargetKind::Declaration => "declaration",
        WorkspaceAnalysisTargetKind::Capability => "capability",
    }
}

fn parse_direction(value: &str) -> Result<WorkspaceAnalysisDirection> {
    match value {
        "forward" => Ok(WorkspaceAnalysisDirection::Forward),
        "reverse" => Ok(WorkspaceAnalysisDirection::Reverse),
        "both" => Ok(WorkspaceAnalysisDirection::Both),
        _ => Err(invalid("semantic query context direction is unsupported")),
    }
}

fn context_options(
    direction: WorkspaceAnalysisDirection,
    depth: usize,
    max_bytes: usize,
    max_nodes: usize,
) -> Result<WorkspaceContextOptions> {
    validate_analysis_bounds(depth, max_bytes, max_nodes)?;
    WorkspaceContextOptions::new(direction, depth, max_bytes, max_nodes)
        .map_err(|_| invalid("semantic query context options are invalid"))
}

fn impact_options(
    depth: usize,
    max_bytes: usize,
    max_nodes: usize,
) -> Result<WorkspaceImpactOptions> {
    validate_analysis_bounds(depth, max_bytes, max_nodes)?;
    WorkspaceImpactOptions::new(depth, max_bytes, max_nodes)
        .map_err(|_| invalid("semantic query impact options are invalid"))
}

fn validate_analysis_bounds(depth: usize, max_bytes: usize, max_nodes: usize) -> Result<()> {
    if max_bytes < 4096 || max_nodes == 0 {
        return Err(invalid("semantic query analysis options are invalid"));
    }
    if depth > 1024 || max_bytes > 16 * 1024 * 1024 || max_nodes > 8208 {
        return Err(capacity(
            "semantic query analysis options exceed their limits",
        ));
    }
    Ok(())
}

fn direction_name(value: WorkspaceAnalysisDirection) -> &'static str {
    match value {
        WorkspaceAnalysisDirection::Forward => "forward",
        WorkspaceAnalysisDirection::Reverse => "reverse",
        WorkspaceAnalysisDirection::Both => "both",
    }
}

fn validate_target(value: &str) -> Result<()> {
    if value.is_empty() || value.len() > MAX_TARGET_BYTES || value.contains('\0') {
        return Err(invalid("semantic query target is invalid"));
    }
    Ok(())
}

fn validate_text(value: &str) -> Result<()> {
    if value.len() > MAX_TARGET_BYTES || value.contains('\0') {
        return Err(invalid("semantic query filter text is invalid"));
    }
    Ok(())
}

fn validate_digest(value: &str) -> Result<()> {
    if value.len() != 71
        || !value.starts_with("sha256:")
        || !value.as_bytes()[7..]
            .iter()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(byte))
    {
        return Err(invalid("semantic query digest is invalid"));
    }
    Ok(())
}

fn render(mut value: Value, maximum: usize, terminal_lf: bool) -> Result<String> {
    value.sort_all_objects();
    let mut output = serde_json::to_string(&value)
        .map_err(|_| invalid("semantic query JSON cannot be rendered"))?;
    if terminal_lf {
        output.push('\n');
    }
    if output.len() > maximum {
        return Err(capacity("semantic query output exceeds its byte limit"));
    }
    Ok(output)
}

fn hash(domain: &[u8], bytes: &[u8]) -> String {
    let mut digest = Sha256::new();
    digest.update(domain);
    digest.update((bytes.len() as u64).to_le_bytes());
    digest.update(bytes);
    format!(
        "sha256:{:x}",
        crate::digest_hex::LowerHex(digest.finalize())
    )
}

fn invalid(message: &'static str) -> Vec<Diagnostic> {
    vec![Diagnostic::io("SPX-G531", message)]
}

fn capacity(message: &'static str) -> Vec<Diagnostic> {
    vec![Diagnostic::io("SPX-G532", message)]
}

fn stale(message: &'static str) -> Vec<Diagnostic> {
    vec![Diagnostic::io("SPX-G533", message)]
}
