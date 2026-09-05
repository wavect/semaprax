//! Bounded process-resident indexes for exact semantic-service generations.
//!
//! The index is derived only from admitted retained HIR and source identity
//! facts. It owns no path, handle, executor, or publication capability.

use std::collections::{BTreeMap, BTreeSet};

use serde_json::{json, Value};
use sha2::{Digest, Sha256};

use crate::diagnostic::Diagnostic;
use crate::hir::{self, IdentityOrigin, ResolvedProgram, ResolvedType};

use super::{ProjectRevision, SemanticWorkspaceSnapshot, MAX_STABLE_ID_BYTES};

pub const SEMANTIC_SERVICE_INDEX_QUERY_SCHEMA: &str =
    "semaprax.semantic-workspace-service-index-query.v1";
pub const SEMANTIC_SERVICE_INDEX_RESULT_SCHEMA: &str =
    "semaprax.semantic-workspace-service-index-result.v1";
pub const MAX_SEMANTIC_SERVICE_INDEX_QUERY_BYTES: usize = 4_096;
pub const MAX_SEMANTIC_SERVICE_INDEX_RESULT_BYTES: usize = 1024 * 1024;
pub const MAX_SEMANTIC_SERVICE_INDEX_ITEMS: usize = 8_192;
const MAX_INDEX_WALK: usize = 65_536;

const QUERY_DOMAIN: &[u8] = b"semaprax.semantic-workspace-service.index-query.digest.v1\0";
const RESULT_DOMAIN: &[u8] = b"semaprax.semantic-workspace-service.index-result.digest.v1\0";

type Result<T> = std::result::Result<T, Vec<Diagnostic>>;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SemanticServiceIndexOperation {
    TestsCoveringDeclaration,
    FunctionsReachingEffect,
}

impl SemanticServiceIndexOperation {
    pub const fn name(self) -> &'static str {
        match self {
            Self::TestsCoveringDeclaration => "tests_covering_declaration",
            Self::FunctionsReachingEffect => "functions_reaching_effect",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SemanticServiceIndexItemKind {
    TestMain,
    TestCase,
    Function,
}

impl SemanticServiceIndexItemKind {
    const fn name(self) -> &'static str {
        match self {
            Self::TestMain => "test_main",
            Self::TestCase => "test_case",
            Self::Function => "function",
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SemanticServiceIndexItem {
    stable_id: String,
    name: String,
    kind: SemanticServiceIndexItemKind,
}

impl SemanticServiceIndexItem {
    pub fn stable_id(&self) -> &str {
        &self.stable_id
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub const fn kind(&self) -> SemanticServiceIndexItemKind {
        self.kind
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SemanticServiceIndexQuery {
    expected_workspace_revision: String,
    operation: SemanticServiceIndexOperation,
    selector: String,
    json: String,
    digest: String,
}

impl SemanticServiceIndexQuery {
    pub fn tests_covering_declaration(
        expected_workspace_revision: &str,
        stable_id: &str,
    ) -> Result<Self> {
        Self::new(
            expected_workspace_revision,
            SemanticServiceIndexOperation::TestsCoveringDeclaration,
            stable_id,
        )
    }

    pub fn functions_reaching_effect(
        expected_workspace_revision: &str,
        effect: &str,
    ) -> Result<Self> {
        Self::new(
            expected_workspace_revision,
            SemanticServiceIndexOperation::FunctionsReachingEffect,
            effect,
        )
    }

    fn new(
        expected_workspace_revision: &str,
        operation: SemanticServiceIndexOperation,
        selector: &str,
    ) -> Result<Self> {
        validate_digest(expected_workspace_revision)?;
        validate_selector(selector)?;
        let value = match operation {
            SemanticServiceIndexOperation::TestsCoveringDeclaration => json!({
                "expected_workspace_revision": expected_workspace_revision,
                "operation": operation.name(),
                "schema": SEMANTIC_SERVICE_INDEX_QUERY_SCHEMA,
                "stable_id": selector,
            }),
            SemanticServiceIndexOperation::FunctionsReachingEffect => json!({
                "effect": selector,
                "expected_workspace_revision": expected_workspace_revision,
                "operation": operation.name(),
                "schema": SEMANTIC_SERVICE_INDEX_QUERY_SCHEMA,
            }),
        };
        let json = render(value, MAX_SEMANTIC_SERVICE_INDEX_QUERY_BYTES)?;
        Ok(Self {
            expected_workspace_revision: expected_workspace_revision.to_owned(),
            operation,
            selector: selector.to_owned(),
            digest: hash(QUERY_DOMAIN, json.as_bytes()),
            json,
        })
    }

    pub fn from_json(bytes: &[u8]) -> Result<Self> {
        if bytes.len() > MAX_SEMANTIC_SERVICE_INDEX_QUERY_BYTES {
            return Err(capacity(
                "semantic service index query exceeds its byte limit",
            ));
        }
        let value: Value = serde_json::from_slice(bytes)
            .map_err(|_| invalid("semantic service index query is not valid JSON"))?;
        let object = value
            .as_object()
            .ok_or_else(|| invalid("semantic service index query is not an object"))?;
        let operation = object
            .get("operation")
            .and_then(Value::as_str)
            .ok_or_else(|| invalid("semantic service index query operation is invalid"))?;
        let expected = object
            .get("expected_workspace_revision")
            .and_then(Value::as_str)
            .ok_or_else(|| invalid("semantic service index query revision is invalid"))?;
        if object.get("schema").and_then(Value::as_str) != Some(SEMANTIC_SERVICE_INDEX_QUERY_SCHEMA)
        {
            return Err(invalid(
                "semantic service index query schema is unsupported",
            ));
        }
        let query = match operation {
            "tests_covering_declaration" => {
                exact_keys(
                    object,
                    &[
                        "expected_workspace_revision",
                        "operation",
                        "schema",
                        "stable_id",
                    ],
                )?;
                Self::tests_covering_declaration(
                    expected,
                    object
                        .get("stable_id")
                        .and_then(Value::as_str)
                        .ok_or_else(|| invalid("semantic service index stable ID is invalid"))?,
                )?
            }
            "functions_reaching_effect" => {
                exact_keys(
                    object,
                    &[
                        "effect",
                        "expected_workspace_revision",
                        "operation",
                        "schema",
                    ],
                )?;
                Self::functions_reaching_effect(
                    expected,
                    object
                        .get("effect")
                        .and_then(Value::as_str)
                        .ok_or_else(|| invalid("semantic service index effect is invalid"))?,
                )?
            }
            _ => {
                return Err(invalid(
                    "semantic service index query operation is unsupported",
                ))
            }
        };
        if query.json.as_bytes() != bytes {
            return Err(invalid(
                "semantic service index query is not exact canonical JSON",
            ));
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

    pub const fn operation(&self) -> SemanticServiceIndexOperation {
        self.operation
    }

    pub fn selector(&self) -> &str {
        &self.selector
    }

    pub fn execute(
        &self,
        snapshot: &SemanticWorkspaceSnapshot,
    ) -> Result<SemanticServiceIndexResult> {
        if self.expected_workspace_revision != snapshot.workspace_revision() {
            return Err(stale(
                "semantic service index query workspace revision is stale",
            ));
        }
        let items = match self.operation {
            SemanticServiceIndexOperation::TestsCoveringDeclaration => snapshot
                .generation()
                .indexes()
                .tests_covering(&self.selector)?,
            SemanticServiceIndexOperation::FunctionsReachingEffect => snapshot
                .generation()
                .indexes()
                .functions_reaching_effect(&self.selector)?,
        };
        let generation = snapshot.generation();
        let json = render(
            json!({
                "authority": false,
                "image_digest": generation.image().image_digest(),
                "items": items.iter().map(|item| json!({
                    "kind": item.kind.name(),
                    "name": item.name.as_str(),
                    "stable_id": item.stable_id.as_str(),
                })).collect::<Vec<_>>(),
                "limits": {
                    "max_index_items": MAX_SEMANTIC_SERVICE_INDEX_ITEMS,
                    "max_query_bytes": MAX_SEMANTIC_SERVICE_INDEX_QUERY_BYTES,
                    "max_result_bytes": MAX_SEMANTIC_SERVICE_INDEX_RESULT_BYTES,
                    "max_walk": MAX_INDEX_WALK,
                },
                "nonclaims": [
                    "no_runtime_coverage_or_path_feasibility",
                    "no_filesystem_network_process_execution_or_publication_authority",
                    "effect_reachability_is_static_retained_HIR_reachability",
                ],
                "operation": self.operation.name(),
                "project_revision": generation.revision().project_revision(),
                "query_digest": self.digest,
                "schema": SEMANTIC_SERVICE_INDEX_RESULT_SCHEMA,
                "selector": self.selector,
                "workspace_revision": generation.workspace_revision(),
            }),
            MAX_SEMANTIC_SERVICE_INDEX_RESULT_BYTES,
        )?;
        Ok(SemanticServiceIndexResult {
            digest: hash(RESULT_DOMAIN, json.as_bytes()),
            json,
            items,
            operation: self.operation,
            query_digest: self.digest.clone(),
            workspace_revision: self.expected_workspace_revision.clone(),
        })
    }

    /// Re-execute a canonical query against the exact snapshot and compare the
    /// complete result bytes and digest. No retained state is changed.
    pub fn replay(
        snapshot: &SemanticWorkspaceSnapshot,
        query_bytes: &[u8],
        expected_result_digest: &str,
        result_bytes: &[u8],
    ) -> Result<SemanticServiceIndexResult> {
        if result_bytes.len() > MAX_SEMANTIC_SERVICE_INDEX_RESULT_BYTES {
            return Err(capacity(
                "semantic service index result exceeds its byte limit",
            ));
        }
        validate_digest(expected_result_digest)?;
        if hash(RESULT_DOMAIN, result_bytes) != expected_result_digest {
            return Err(stale("semantic service index result digest is stale"));
        }
        let query = Self::from_json(query_bytes)?;
        let result = query.execute(snapshot)?;
        if result.result_digest() != expected_result_digest
            || result.to_json().as_bytes() != result_bytes
        {
            return Err(stale("semantic service index result failed exact replay"));
        }
        Ok(result)
    }
}

pub struct SemanticServiceIndexResult {
    json: String,
    digest: String,
    query_digest: String,
    workspace_revision: String,
    operation: SemanticServiceIndexOperation,
    items: Vec<SemanticServiceIndexItem>,
}

impl SemanticServiceIndexResult {
    pub fn to_json(&self) -> &str {
        &self.json
    }
    pub fn result_digest(&self) -> &str {
        &self.digest
    }
    pub fn query_digest(&self) -> &str {
        &self.query_digest
    }
    pub fn workspace_revision(&self) -> &str {
        &self.workspace_revision
    }
    pub const fn operation(&self) -> SemanticServiceIndexOperation {
        self.operation
    }
    pub fn items(&self) -> &[SemanticServiceIndexItem] {
        &self.items
    }
}

#[derive(Clone)]
struct IndexedFunction {
    name: String,
    effects: BTreeSet<String>,
}

pub(crate) struct SemanticServiceIndexes {
    functions: BTreeMap<String, IndexedFunction>,
    reverse_calls: BTreeMap<String, BTreeSet<String>>,
    test_roots: BTreeMap<String, SemanticServiceIndexItemKind>,
}

impl SemanticServiceIndexes {
    pub(crate) fn derive(revision: &ProjectRevision) -> Result<Self> {
        let mut functions = BTreeMap::new();
        let mut reverse_calls = BTreeMap::<String, BTreeSet<String>>::new();
        for program in [
            revision.entry_program(),
            revision.public_api_program(),
            revision.test_program(),
        ] {
            index_program(program, &mut functions, &mut reverse_calls)?;
        }
        if functions.len() > MAX_SEMANTIC_SERVICE_INDEX_ITEMS
            || reverse_calls.values().map(BTreeSet::len).sum::<usize>() > MAX_INDEX_WALK
        {
            return Err(capacity(
                "semantic service index exceeds its retained bounds",
            ));
        }

        let program = revision.test_program();
        let mut test_roots = BTreeMap::new();
        test_roots.insert(
            program.entrypoint.as_str().to_owned(),
            SemanticServiceIndexItemKind::TestMain,
        );
        for function in &program.functions {
            let explicit = program
                .declarations
                .declaration(&function.id)
                .is_some_and(|declaration| declaration.identity_origin == IdentityOrigin::Explicit);
            let declared_here = revision
                .semantic
                .rename_function(function.id.as_str())
                .is_some_and(|declared| declared.module == revision.manifest().test_module());
            if function.name.starts_with(super::TEST_CASE_PREFIX)
                && function.params.is_empty()
                && function.return_type == ResolvedType::I64
                && explicit
                && declared_here
            {
                test_roots.insert(
                    function.id.as_str().to_owned(),
                    SemanticServiceIndexItemKind::TestCase,
                );
            }
        }
        Ok(Self {
            functions,
            reverse_calls,
            test_roots,
        })
    }

    fn tests_covering(&self, stable_id: &str) -> Result<Vec<SemanticServiceIndexItem>> {
        if !self.functions.contains_key(stable_id) {
            return Err(invalid(
                "semantic service index stable function declaration is unknown",
            ));
        }
        let reachable = self.reverse_closure(std::iter::once(stable_id))?;
        Ok(self
            .test_roots
            .iter()
            .filter(|(id, _)| reachable.contains(*id))
            .filter_map(|(id, kind)| self.item(id, *kind))
            .collect())
    }

    fn functions_reaching_effect(&self, effect: &str) -> Result<Vec<SemanticServiceIndexItem>> {
        let seeds = self
            .functions
            .iter()
            .filter(|(_, function)| function.effects.contains(effect))
            .map(|(id, _)| id.as_str());
        let reachable = self.reverse_closure(seeds)?;
        Ok(reachable
            .iter()
            .filter_map(|id| self.item(id, SemanticServiceIndexItemKind::Function))
            .collect())
    }

    fn reverse_closure<'a>(
        &self,
        seeds: impl Iterator<Item = &'a str>,
    ) -> Result<BTreeSet<String>> {
        let mut reached = seeds.map(str::to_owned).collect::<BTreeSet<_>>();
        let mut pending = reached.iter().cloned().collect::<Vec<_>>();
        let mut visits = 0usize;
        while let Some(id) = pending.pop() {
            visits = visits.saturating_add(1);
            if visits > MAX_INDEX_WALK {
                return Err(capacity(
                    "semantic service index query exceeds its walk bound",
                ));
            }
            if let Some(callers) = self.reverse_calls.get(&id) {
                for caller in callers {
                    if reached.insert(caller.clone()) {
                        pending.push(caller.clone());
                    }
                }
            }
        }
        if reached.len() > MAX_SEMANTIC_SERVICE_INDEX_ITEMS {
            return Err(capacity(
                "semantic service index query exceeds its item bound",
            ));
        }
        Ok(reached)
    }

    fn item(
        &self,
        id: &str,
        kind: SemanticServiceIndexItemKind,
    ) -> Option<SemanticServiceIndexItem> {
        self.functions
            .get(id)
            .map(|function| SemanticServiceIndexItem {
                stable_id: id.to_owned(),
                name: function.name.clone(),
                kind,
            })
    }
}

fn index_program(
    program: &ResolvedProgram,
    functions: &mut BTreeMap<String, IndexedFunction>,
    reverse_calls: &mut BTreeMap<String, BTreeSet<String>>,
) -> Result<()> {
    for function in &program.functions {
        insert_function_fact(
            functions,
            function.id.as_str(),
            &function.name,
            &function.effects,
        )?;
        for expression in function
            .requires
            .iter()
            .chain(std::iter::once(&function.body))
            .chain(&function.ensures)
        {
            hir::visit_resolved_calls(expression, &mut |callee, _, _| {
                reverse_calls
                    .entry(callee.as_str().to_owned())
                    .or_default()
                    .insert(function.id.as_str().to_owned());
            });
        }
    }
    // Generic templates remain stable declarations even though their retained
    // concrete instances have revision-scoped execution identities.
    for function in &program.function_templates {
        insert_function_fact(
            functions,
            function.id.as_str(),
            &function.name,
            &function.effects,
        )?;
        for expression in function
            .requires
            .iter()
            .chain(std::iter::once(&function.body))
            .chain(&function.ensures)
        {
            hir::visit_resolved_calls(expression, &mut |callee, _, _| {
                reverse_calls
                    .entry(callee.as_str().to_owned())
                    .or_default()
                    .insert(function.id.as_str().to_owned());
            });
        }
    }
    Ok(())
}

fn insert_function_fact(
    functions: &mut BTreeMap<String, IndexedFunction>,
    id: &str,
    name: &str,
    effects: &[String],
) -> Result<()> {
    let fact = IndexedFunction {
        name: name.to_owned(),
        effects: effects.iter().cloned().collect(),
    };
    if let Some(previous) = functions.insert(id.to_owned(), fact.clone()) {
        if previous.name != fact.name || previous.effects != fact.effects {
            return Err(invalid(
                "semantic service index found conflicting retained function facts",
            ));
        }
    }
    Ok(())
}

fn exact_keys(object: &serde_json::Map<String, Value>, expected: &[&str]) -> Result<()> {
    if object.len() != expected.len() || !expected.iter().all(|key| object.contains_key(*key)) {
        return Err(invalid("semantic service index query keys are invalid"));
    }
    Ok(())
}

fn validate_selector(value: &str) -> Result<()> {
    if value.is_empty() || value.len() > MAX_STABLE_ID_BYTES || value.contains('\0') {
        return Err(invalid("semantic service index selector is invalid"));
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
        return Err(invalid("semantic service index revision digest is invalid"));
    }
    Ok(())
}

fn render(mut value: Value, max_bytes: usize) -> Result<String> {
    value.sort_all_objects();
    let mut output = serde_json::to_string(&value)
        .map_err(|_| invalid("semantic service index value cannot be rendered"))?;
    output.push('\n');
    if output.len() > max_bytes {
        return Err(capacity(
            "semantic service index value exceeds its byte limit",
        ));
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
    vec![Diagnostic::io("SPX-G528", message)]
}
fn capacity(message: &'static str) -> Vec<Diagnostic> {
    vec![Diagnostic::io("SPX-G529", message)]
}
fn stale(message: &'static str) -> Vec<Diagnostic> {
    vec![Diagnostic::io("SPX-G530", message)]
}
