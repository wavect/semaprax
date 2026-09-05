//! Bounded, canonical, authority-free history of successful service outcomes.

use std::sync::Arc;

use serde_json::{json, Value};

use super::{capacity, hash, invalid, parse_value, stale, validate_digest, Result};

pub const SEMANTIC_WORKSPACE_SERVICE_HISTORY_ENTRY_SCHEMA: &str =
    "semaprax.semantic-workspace-service-history-entry.v1";
pub const SEMANTIC_WORKSPACE_SERVICE_HISTORY_QUERY_SCHEMA: &str =
    "semaprax.semantic-workspace-service-history-query.v1";
pub const SEMANTIC_WORKSPACE_SERVICE_HISTORY_RESULT_SCHEMA: &str =
    "semaprax.semantic-workspace-service-history-result.v1";
pub const MAX_SEMANTIC_WORKSPACE_SERVICE_HISTORY_ENTRIES: usize = 1024;
pub const MAX_SEMANTIC_WORKSPACE_SERVICE_HISTORY_QUERY_LIMIT: usize = 64;
pub const MAX_SEMANTIC_WORKSPACE_SERVICE_HISTORY_QUERY_BYTES: usize = 4096;
pub const MAX_SEMANTIC_WORKSPACE_SERVICE_HISTORY_RESULT_BYTES: usize = 1024 * 1024;

const ENTRY_DOMAIN: &[u8] = b"semaprax.semantic-workspace-service.history-entry.digest.v1\0";
const QUERY_DOMAIN: &[u8] = b"semaprax.semantic-workspace-service.history-query.digest.v1\0";
const RESULT_DOMAIN: &[u8] = b"semaprax.semantic-workspace-service.history-result.digest.v1\0";

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SemanticWorkspaceServiceHistoryEntry {
    ordinal: usize,
    kind: &'static str,
    base_workspace_revision: String,
    base_project_revision: String,
    outcome_workspace_revision: String,
    outcome_project_revision: String,
    transaction_digest: Option<String>,
    result_digest: Option<String>,
    refresh_receipt_digest: Option<String>,
    json: String,
    digest: String,
}

impl SemanticWorkspaceServiceHistoryEntry {
    pub fn ordinal(&self) -> usize {
        self.ordinal
    }

    pub fn kind(&self) -> &str {
        self.kind
    }

    pub fn base_workspace_revision(&self) -> &str {
        &self.base_workspace_revision
    }

    pub fn base_project_revision(&self) -> &str {
        &self.base_project_revision
    }

    pub fn outcome_workspace_revision(&self) -> &str {
        &self.outcome_workspace_revision
    }

    pub fn outcome_project_revision(&self) -> &str {
        &self.outcome_project_revision
    }

    pub fn transaction_digest(&self) -> Option<&str> {
        self.transaction_digest.as_deref()
    }

    pub fn result_digest(&self) -> Option<&str> {
        self.result_digest.as_deref()
    }

    pub fn refresh_receipt_digest(&self) -> Option<&str> {
        self.refresh_receipt_digest.as_deref()
    }

    pub fn to_json(&self) -> &str {
        &self.json
    }

    pub fn entry_digest(&self) -> &str {
        &self.digest
    }
}

pub(super) struct SemanticWorkspaceServiceHistory {
    entries: Vec<SemanticWorkspaceServiceHistoryEntry>,
}

impl SemanticWorkspaceServiceHistory {
    pub(super) fn new() -> Self {
        Self {
            entries: Vec::new(),
        }
    }

    pub(super) fn require_capacity(&self) -> Result<()> {
        if self.entries.len() >= MAX_SEMANTIC_WORKSPACE_SERVICE_HISTORY_ENTRIES {
            return Err(capacity(
                "semantic workspace service history entry limit is exhausted",
            ));
        }
        Ok(())
    }

    pub(super) fn transaction_entry(
        &self,
        base_project_revision: &str,
        base_workspace_revision: &str,
        outcome_project_revision: &str,
        outcome_workspace_revision: &str,
        transaction_digest: &str,
        result_digest: &str,
    ) -> Result<SemanticWorkspaceServiceHistoryEntry> {
        self.entry(
            "transaction_validation",
            base_project_revision,
            base_workspace_revision,
            outcome_project_revision,
            outcome_workspace_revision,
            Some(transaction_digest),
            Some(result_digest),
            None,
        )
    }

    pub(super) fn refresh_entry(
        &self,
        base_project_revision: &str,
        base_workspace_revision: &str,
        outcome_project_revision: &str,
        outcome_workspace_revision: &str,
        receipt_digest: &str,
    ) -> Result<SemanticWorkspaceServiceHistoryEntry> {
        self.entry(
            "refresh",
            base_project_revision,
            base_workspace_revision,
            outcome_project_revision,
            outcome_workspace_revision,
            None,
            None,
            Some(receipt_digest),
        )
    }

    #[allow(clippy::too_many_arguments)]
    fn entry(
        &self,
        kind: &'static str,
        base_project_revision: &str,
        base_workspace_revision: &str,
        outcome_project_revision: &str,
        outcome_workspace_revision: &str,
        transaction_digest: Option<&str>,
        result_digest: Option<&str>,
        refresh_receipt_digest: Option<&str>,
    ) -> Result<SemanticWorkspaceServiceHistoryEntry> {
        self.require_capacity()?;
        for digest in [
            base_project_revision,
            base_workspace_revision,
            outcome_project_revision,
            outcome_workspace_revision,
        ] {
            validate_digest(digest)?;
        }
        for digest in [transaction_digest, result_digest, refresh_receipt_digest]
            .into_iter()
            .flatten()
        {
            validate_digest(digest)?;
        }
        let ordinal = self.entries.len();
        let previous_entry_digest = self.entries.last().map(|entry| entry.entry_digest());
        let json = render_bounded(
            json!({
                "authority": false,
                "base_project_revision": base_project_revision,
                "base_workspace_revision": base_workspace_revision,
                "kind": kind,
                "ordinal": ordinal,
                "outcome_project_revision": outcome_project_revision,
                "outcome_workspace_revision": outcome_workspace_revision,
                "previous_entry_digest": previous_entry_digest,
                "refresh_receipt_digest": refresh_receipt_digest,
                "result_digest": result_digest,
                "schema": SEMANTIC_WORKSPACE_SERVICE_HISTORY_ENTRY_SCHEMA,
                "transaction_digest": transaction_digest,
            }),
            MAX_SEMANTIC_WORKSPACE_SERVICE_HISTORY_RESULT_BYTES,
            "semantic workspace service history entry exceeds its byte limit",
        )?;
        Ok(SemanticWorkspaceServiceHistoryEntry {
            ordinal,
            kind,
            base_workspace_revision: base_workspace_revision.to_owned(),
            base_project_revision: base_project_revision.to_owned(),
            outcome_workspace_revision: outcome_workspace_revision.to_owned(),
            outcome_project_revision: outcome_project_revision.to_owned(),
            transaction_digest: transaction_digest.map(str::to_owned),
            result_digest: result_digest.map(str::to_owned),
            refresh_receipt_digest: refresh_receipt_digest.map(str::to_owned),
            digest: hash(ENTRY_DOMAIN, json.as_bytes()),
            json,
        })
    }

    pub(super) fn append(&mut self, entry: SemanticWorkspaceServiceHistoryEntry) {
        debug_assert_eq!(entry.ordinal, self.entries.len());
        self.entries.push(entry);
    }

    pub(super) fn snapshot(
        &self,
        workspace_revision: &str,
        project_revision: &str,
    ) -> Result<SemanticWorkspaceServiceHistorySnapshot> {
        validate_digest(workspace_revision)?;
        validate_digest(project_revision)?;
        Ok(SemanticWorkspaceServiceHistorySnapshot {
            workspace_revision: workspace_revision.to_owned(),
            project_revision: project_revision.to_owned(),
            entries: Arc::from(self.entries.clone()),
        })
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SemanticWorkspaceServiceHistoryQuery {
    expected_workspace_revision: String,
    offset: usize,
    limit: usize,
    json: String,
    digest: String,
}

impl SemanticWorkspaceServiceHistoryQuery {
    pub fn new(expected_workspace_revision: &str, offset: usize, limit: usize) -> Result<Self> {
        validate_digest(expected_workspace_revision)?;
        if offset > MAX_SEMANTIC_WORKSPACE_SERVICE_HISTORY_ENTRIES {
            return Err(capacity(
                "semantic workspace service history query offset exceeds its limit",
            ));
        }
        if limit == 0 || limit > MAX_SEMANTIC_WORKSPACE_SERVICE_HISTORY_QUERY_LIMIT {
            return Err(capacity(
                "semantic workspace service history query limit is invalid",
            ));
        }
        let json = render_bounded(
            json!({
                "expected_workspace_revision": expected_workspace_revision,
                "limit": limit,
                "offset": offset,
                "schema": SEMANTIC_WORKSPACE_SERVICE_HISTORY_QUERY_SCHEMA,
            }),
            MAX_SEMANTIC_WORKSPACE_SERVICE_HISTORY_QUERY_BYTES,
            "semantic workspace service history query exceeds its byte limit",
        )?;
        Ok(Self {
            expected_workspace_revision: expected_workspace_revision.to_owned(),
            offset,
            limit,
            digest: hash(QUERY_DOMAIN, json.as_bytes()),
            json,
        })
    }

    pub fn from_json(bytes: &[u8]) -> Result<Self> {
        if bytes.len() > MAX_SEMANTIC_WORKSPACE_SERVICE_HISTORY_QUERY_BYTES {
            return Err(capacity(
                "semantic workspace service history query exceeds its byte limit",
            ));
        }
        let value: Value = serde_json::from_slice(bytes)
            .map_err(|_| invalid("semantic workspace service history query is not valid JSON"))?;
        let object = value
            .as_object()
            .ok_or_else(|| invalid("semantic workspace service history query is not an object"))?;
        let keys = ["expected_workspace_revision", "limit", "offset", "schema"];
        if object.len() != keys.len()
            || keys.iter().any(|key| !object.contains_key(*key))
            || value["schema"] != SEMANTIC_WORKSPACE_SERVICE_HISTORY_QUERY_SCHEMA
        {
            return Err(invalid(
                "semantic workspace service history query has an invalid field set",
            ));
        }
        let expected = value["expected_workspace_revision"]
            .as_str()
            .ok_or_else(|| {
                invalid("semantic workspace service history query revision is invalid")
            })?;
        let offset = value["offset"]
            .as_u64()
            .and_then(|value| usize::try_from(value).ok())
            .ok_or_else(|| invalid("semantic workspace service history query offset is invalid"))?;
        let limit = value["limit"]
            .as_u64()
            .and_then(|value| usize::try_from(value).ok())
            .ok_or_else(|| invalid("semantic workspace service history query limit is invalid"))?;
        let query = Self::new(expected, offset, limit)?;
        if query.json.as_bytes() != bytes {
            return Err(invalid(
                "semantic workspace service history query is not exact canonical JSON",
            ));
        }
        Ok(query)
    }

    pub fn expected_workspace_revision(&self) -> &str {
        &self.expected_workspace_revision
    }

    pub fn offset(&self) -> usize {
        self.offset
    }

    pub fn limit(&self) -> usize {
        self.limit
    }

    pub fn to_json(&self) -> &str {
        &self.json
    }

    pub fn query_digest(&self) -> &str {
        &self.digest
    }

    pub fn replay(
        snapshot: &SemanticWorkspaceServiceHistorySnapshot,
        query_bytes: &[u8],
        expected_result_digest: &str,
        result_bytes: &[u8],
    ) -> Result<SemanticWorkspaceServiceHistoryResult> {
        validate_digest(expected_result_digest)?;
        if result_bytes.len() > MAX_SEMANTIC_WORKSPACE_SERVICE_HISTORY_RESULT_BYTES {
            return Err(capacity(
                "semantic workspace service history result exceeds its byte limit",
            ));
        }
        let query = Self::from_json(query_bytes)?;
        let result = snapshot.query(&query)?;
        if result.result_digest() != expected_result_digest
            || result.to_json().as_bytes() != result_bytes
        {
            return Err(stale(
                "semantic workspace service history result failed exact replay",
            ));
        }
        Ok(result)
    }
}

#[derive(Clone)]
pub struct SemanticWorkspaceServiceHistorySnapshot {
    workspace_revision: String,
    project_revision: String,
    entries: Arc<[SemanticWorkspaceServiceHistoryEntry]>,
}

impl SemanticWorkspaceServiceHistorySnapshot {
    pub fn workspace_revision(&self) -> &str {
        &self.workspace_revision
    }

    pub fn project_revision(&self) -> &str {
        &self.project_revision
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn query(
        &self,
        query: &SemanticWorkspaceServiceHistoryQuery,
    ) -> Result<SemanticWorkspaceServiceHistoryResult> {
        if query.expected_workspace_revision != self.workspace_revision {
            return Err(stale(
                "semantic workspace service history query revision is stale",
            ));
        }
        let items = self
            .entries
            .iter()
            .skip(query.offset)
            .take(query.limit)
            .cloned()
            .collect::<Vec<_>>();
        let next_offset =
            (query.offset + items.len() < self.entries.len()).then_some(query.offset + items.len());
        let head_digest = self.entries.last().map(|entry| entry.entry_digest());
        let values = items
            .iter()
            .map(|entry| {
                Ok(json!({
                    "digest": entry.entry_digest(),
                    "value": parse_value(entry.to_json())?,
                }))
            })
            .collect::<Result<Vec<_>>>()?;
        let json = render_bounded(
            json!({
                "authority": false,
                "history_head_digest": head_digest,
                "history_length": self.entries.len(),
                "items": values,
                "next_offset": next_offset,
                "project_revision": self.project_revision,
                "query_digest": query.query_digest(),
                "schema": SEMANTIC_WORKSPACE_SERVICE_HISTORY_RESULT_SCHEMA,
                "workspace_revision": self.workspace_revision,
            }),
            MAX_SEMANTIC_WORKSPACE_SERVICE_HISTORY_RESULT_BYTES,
            "semantic workspace service history result exceeds its byte limit",
        )?;
        Ok(SemanticWorkspaceServiceHistoryResult {
            workspace_revision: self.workspace_revision.clone(),
            project_revision: self.project_revision.clone(),
            query_digest: query.digest.clone(),
            digest: hash(RESULT_DOMAIN, json.as_bytes()),
            items,
            next_offset,
            history_length: self.entries.len(),
            json,
        })
    }
}

pub struct SemanticWorkspaceServiceHistoryResult {
    workspace_revision: String,
    project_revision: String,
    query_digest: String,
    digest: String,
    items: Vec<SemanticWorkspaceServiceHistoryEntry>,
    next_offset: Option<usize>,
    history_length: usize,
    json: String,
}

impl SemanticWorkspaceServiceHistoryResult {
    pub fn workspace_revision(&self) -> &str {
        &self.workspace_revision
    }

    pub fn project_revision(&self) -> &str {
        &self.project_revision
    }

    pub fn query_digest(&self) -> &str {
        &self.query_digest
    }

    pub fn result_digest(&self) -> &str {
        &self.digest
    }

    pub fn items(&self) -> &[SemanticWorkspaceServiceHistoryEntry] {
        &self.items
    }

    pub fn next_offset(&self) -> Option<usize> {
        self.next_offset
    }

    pub fn history_length(&self) -> usize {
        self.history_length
    }

    pub fn to_json(&self) -> &str {
        &self.json
    }
}

fn render_bounded(mut value: Value, limit: usize, message: &'static str) -> Result<String> {
    value.sort_all_objects();
    let mut bytes = serde_json::to_vec(&value)
        .map_err(|_| invalid("semantic workspace service history cannot be rendered"))?;
    bytes.push(b'\n');
    if bytes.len() > limit {
        return Err(capacity(message));
    }
    String::from_utf8(bytes).map_err(|_| invalid("semantic workspace service history is not UTF-8"))
}
