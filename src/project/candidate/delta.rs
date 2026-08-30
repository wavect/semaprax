//! Bounded descriptive deltas recomputed from retained validated revisions.
//! Equality here is projection equality, never behavioral equivalence.
use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

use serde_json::{json, Value};

use crate::ast::{Span, TypeDeclarationKind};
use crate::diagnostic::Diagnostic;
use crate::project::{ImageFacet, ProjectRevision, ProjectSemanticImage};

use super::{parse_revision, wire, ProjectCandidate};

type Result<T> = std::result::Result<T, Vec<Diagnostic>>;
pub const PROJECT_CANDIDATE_SEMANTIC_DELTA_SCHEMA: &str =
    "semaprax.project-candidate-semantic-delta.v1";
pub const PROJECT_CANDIDATE_SEMANTIC_DELTA_CATALOG_SCHEMA: &str =
    "semaprax.project-candidate-semantic-delta-catalog.v1";
pub const MAX_PROJECT_CANDIDATE_SEMANTIC_DELTA_BYTES: usize = 8 * 1024 * 1024;
pub const MAX_PROJECT_CANDIDATE_SEMANTIC_DELTA_CATALOG_BYTES: usize = 1024 * 1024;
const MAX_ITEMS: usize = 65_536;
const MAX_VISITS: usize = 1_048_576;
const MAX_DEPTH: usize = 256;
const FACT_DOMAIN: &[u8] = b"semaprax.candidate-semantic-delta.fact.v1\0";
const REPORT_DOMAIN: &[u8] = b"semaprax.candidate-semantic-delta.report.v1\0";

impl ProjectCandidate {
    /// A selected stable declaration's source-bound before/after facts. This
    /// performs bounded compiler projections, never interpreter/target execution.
    pub fn semantic_delta(&self, expected_candidate: &str, target: &str) -> Result<String> {
        self.require_candidate(expected_candidate)?;
        target_id(target)?;
        let before_authored = authored(&self.base)?;
        let after_authored = authored(&self.revision)?;
        if self.base.semantic.image_symbol(target).is_none()
            && self.revision.semantic.image_symbol(target).is_none()
            && !before_authored.contains_key(target)
            && !after_authored.contains_key(target)
        {
            return Err(invalid(
                "semantic delta target is absent from both revisions",
            ));
        }
        let before_image =
            ProjectSemanticImage::derive(Arc::clone(&self.base), self.base.project_revision())?;
        let after_image = ProjectSemanticImage::derive(
            Arc::clone(&self.revision),
            self.revision.project_revision(),
        )?;
        let before = target_facts(&before_image, target, before_authored.get(target))?;
        let after = target_facts(&after_image, target, after_authored.get(target))?;
        let names = before
            .keys()
            .chain(after.keys())
            .cloned()
            .collect::<BTreeSet<_>>();
        let mut facets = Vec::new();
        for name in names {
            facets.push(pair(
                &name,
                before.get(&name).cloned().unwrap_or(Value::Null),
                after.get(&name).cloned().unwrap_or(Value::Null),
            )?);
        }
        let artifacts = pair(
            "complete_entry_and_test_target_artifacts",
            wire::target_facts(&self.base)?,
            wire::target_facts(&self.revision)?,
        )?;
        let value = json!({"schema":PROJECT_CANDIDATE_SEMANTIC_DELTA_SCHEMA,
            "candidate_digest":self.candidate_digest(),"target":target,
            "base_project_revision":self.base.project_revision(),"project_revision":self.revision.project_revision(),
            "base_workspace_revision":self.base.workspace_revision(),"workspace_revision":self.revision.workspace_revision(),
            "base_image_digest":before_image.image_digest(),"image_digest":after_image.image_digest(),
            "presence":presence(self.base.semantic.image_symbol(target).is_some() || before_authored.contains_key(target),self.revision.semantic.image_symbol(target).is_some() || after_authored.contains_key(target)),
            "source_bindings":{"base":source_binding(&self.base,target,before_authored.get(target)),"candidate":source_binding(&self.revision,target,after_authored.get(target))},
            "facets":facets,"target_artifacts":artifacts,
            "test_plan":serde_json::from_str::<Value>(&self.test_plan(self.candidate_digest())?).map_err(|_|invalid("retained test plan is invalid"))?,
            "evidence_class":"descriptive_recomputable_compiler_projection",
            "comparison":"exact_values_plus_separate_provenance_insensitive_projection_equality",
            "omitted_equal_payloads":true,
            "limits":{"max_report_bytes":MAX_PROJECT_CANDIDATE_SEMANTIC_DELTA_BYTES,"max_items":MAX_ITEMS,"max_expression_visits":MAX_VISITS,"max_depth":MAX_DEPTH},
            "nonclaims":["not_behavioral_equivalence","not_runtime_liveness","not_test_coverage","no_interpreter_or_target_execution","target_artifacts_are_whole_closure_not_per_symbol","no_source_or_publication_authority"]});
        render(value, MAX_PROJECT_CANDIDATE_SEMANTIC_DELTA_BYTES)
    }

    /// Compact roots derived from every authored stable declaration, including
    /// fields and types; no function-only filter or duplicate whole graph.
    pub fn semantic_delta_catalog(&self, expected_candidate: &str) -> Result<String> {
        self.require_candidate(expected_candidate)?;
        let before = authored(&self.base)?;
        let after = authored(&self.revision)?;
        let ids = before
            .keys()
            .chain(after.keys())
            .cloned()
            .collect::<BTreeSet<_>>();
        let mut roots = Vec::new();
        for id in ids {
            let base = before.get(&id);
            let candidate = after.get(&id);
            if base == candidate {
                continue;
            }
            let moved = base.zip(candidate).is_some_and(|(before, after)| {
                before["path"] != after["path"] || before["module"] != after["module"]
            });
            roots.push(json!({"target":id,"change":if moved {"moved"} else {presence(base.is_some(),candidate.is_some())},
                "base":base.map(compact_authored),"candidate":candidate.map(compact_authored)}));
            if roots.len() > MAX_ITEMS {
                return Err(capacity("semantic delta root inventory exceeds its bound"));
            }
        }
        render(
            json!({"schema":PROJECT_CANDIDATE_SEMANTIC_DELTA_CATALOG_SCHEMA,
            "candidate_digest":self.candidate_digest(),"base_project_revision":self.base.project_revision(),"project_revision":self.revision.project_revision(),
            "roots":roots,"selection_basis":"authored_declaration_identity_origin_and_canonical_fragment_changes",
            "source_changes":self.base.sources().iter().zip(self.revision.sources()).filter(|(before,after)|before.source()!=after.source()).map(|(before,after)|json!({"path":before.path(),"base_source_digest":before.source_digest(),"source_digest":after.source_digest()})).collect::<Vec<_>>(),
            "nonclaims":["not_complete_dynamic_impact","not_behavioral_equivalence","no_target_or_test_execution","no_source_authority"]}),
            MAX_PROJECT_CANDIDATE_SEMANTIC_DELTA_CATALOG_BYTES,
        )
    }

    /// Replays candidate source/evidence first, then recomputes the selected
    /// projection and compares all exact bytes; submitted JSON grants no trust.
    pub fn verify_semantic_delta(
        &self,
        expected_candidate: &str,
        target: &str,
        bytes: &[u8],
    ) -> Result<String> {
        self.require_candidate(expected_candidate)?;
        if bytes.len() > MAX_PROJECT_CANDIDATE_SEMANTIC_DELTA_BYTES {
            return Err(capacity("semantic delta replay input exceeds its bound"));
        }
        let replay = Self::replay(
            Arc::clone(&self.base),
            self.base.project_revision(),
            &self.changes,
            self.to_json().as_bytes(),
        )?;
        let expected = replay.semantic_delta(expected_candidate, target)?;
        if expected.as_bytes() != bytes {
            return Err(stale(
                "semantic delta failed exact independent recomputation",
            ));
        }
        render(
            json!({"schema":"semaprax.project-candidate-semantic-delta-verification.v1","result":"exact_recomputation",
            "candidate_digest":expected_candidate,"target":target,"delta_digest":wire::digest(REPORT_DOMAIN,bytes),
            "base_project_revision":self.base.project_revision(),"project_revision":self.revision.project_revision(),
            "execution":false,"source_authority":false}),
            65_536,
        )
    }
}

fn target_facts(
    image: &ProjectSemanticImage,
    target: &str,
    authored: Option<&Value>,
) -> Result<BTreeMap<String, Value>> {
    let revision = image.revision();
    let mut facts = BTreeMap::new();
    let symbol = revision.semantic.image_symbol(target);
    if symbol.is_none() && authored.is_none() {
        return Ok(facts);
    }
    let conformance = super::interface::related(revision, target)?;
    if !conformance.is_empty() {
        facts.insert("source_static_conformance".to_owned(), json!(conformance));
    }
    if symbol.is_none() {
        facts.insert(
            "authored_declaration".to_owned(),
            authored.cloned().unwrap_or(Value::Null),
        );
        facts.insert("runtime_graph_applicability".to_owned(), json!({"available":false,"reason":"source_declaration_not_projected_into_runtime_graph"}));
        return Ok(facts);
    }
    let symbol = symbol.unwrap();
    facts.insert("declaration_identity".to_owned(), symbol);
    facts.insert(
        "authored_declaration".to_owned(),
        authored.cloned().unwrap_or(Value::Null),
    );
    facts.insert(
        "typed_declaration".to_owned(),
        image.dependency_index()?.typed_declaration(target),
    );
    let mut function_found = false;
    for module in revision.semantic.image_modules() {
        for function in module
            .functions()
            .iter()
            .filter(|function| function.id.as_str() == target)
        {
            if function_found {
                return Err(invalid("semantic delta function identity is ambiguous"));
            }
            function_found = true;
            for facet in ImageFacet::ALL {
                facts.insert(
                    facet.name().to_owned(),
                    json!(image.facet_items(module, function, facet)?),
                );
            }
        }
    }
    facts.insert("function_facet_applicability".to_owned(),json!({"available":function_found,
        "reason":if function_found {"retained_resolved_function"}else{"not_a_retained_resolved_function; authored_and_typed_declaration_facts_remain_available"}}));
    let relationships = image
        .dependency_index()?
        .reverse_relationships(revision, target)?;
    facts.insert(
        "reverse_field_and_call_relationships".to_owned(),
        relationships,
    );
    Ok(facts)
}

fn pair(name: &str, before: Value, after: Value) -> Result<Value> {
    let before_bytes = render(before.clone(), MAX_PROJECT_CANDIDATE_SEMANTIC_DELTA_BYTES)?;
    let after_bytes = render(after.clone(), MAX_PROJECT_CANDIDATE_SEMANTIC_DELTA_BYTES)?;
    let before_semantic = normalize(before.clone());
    let after_semantic = normalize(after.clone());
    let exact = before == after;
    let semantic = before_semantic == after_semantic;
    let mut result = json!({"facet":name,"change":if exact {"unchanged"}else if semantic {"provenance_only"}else{presence(!before.is_null(),!after.is_null())},
        "exact_equal":exact,"projection_equal_without_provenance":semantic,
        "base_digest":wire::digest(FACT_DOMAIN,before_bytes.as_bytes()),"candidate_digest":wire::digest(FACT_DOMAIN,after_bytes.as_bytes()),
        "base_bytes":before_bytes.len(),"candidate_bytes":after_bytes.len()});
    if !semantic {
        result["base"] = before;
        result["candidate"] = after;
    }
    Ok(result)
}

fn normalize(mut value: Value) -> Value {
    match &mut value {
        Value::Object(items) => {
            for key in [
                "image_revision",
                "project_revision",
                "source_revision",
                "source_digest",
                "span",
                "source_span",
                "expression_id",
                "initializer_expression_id",
                "base_expression_id",
                "container_expression_id",
            ] {
                items.remove(key);
            }
            for value in items.values_mut() {
                *value = normalize(std::mem::take(value));
            }
        }
        Value::Array(items) => {
            for value in items {
                *value = normalize(std::mem::take(value));
            }
        }
        _ => {}
    }
    value
}

fn authored(revision: &ProjectRevision) -> Result<BTreeMap<String, Value>> {
    let mut output = BTreeMap::new();
    let mut bytes = 0usize;
    for program in parse_revision(revision)? {
        let source = revision
            .sources()
            .iter()
            .find(|source| source.path() == program.path)
            .ok_or_else(|| invalid("authored delta source is absent"))?;
        let mut put = |id: &str, name: &str, kind: &str, span: Span| -> Result<()> {
            let fragment = source
                .source()
                .get(span.start..span.end)
                .ok_or_else(|| invalid("authored delta span is outside authenticated source"))?;
            bytes = bytes
                .checked_add(fragment.len())
                .ok_or_else(|| capacity("authored delta inventory overflow"))?;
            if bytes > 32 * 1024 * 1024 || output.len() >= MAX_ITEMS {
                return Err(capacity("authored delta inventory exceeds its bound"));
            }
            let value = json!({"id":id,"name":name,"kind":kind,"path":program.path,"module":program.module,
                "canonical_fragment":fragment,"fragment_digest":wire::digest(b"semaprax.candidate-semantic-delta.authored.v1\0",fragment.as_bytes())});
            if output.insert(id.to_owned(), value).is_some() {
                return Err(invalid("authored semantic delta identity is duplicated"));
            }
            Ok(())
        };
        for function in &program.functions {
            put(
                &function.stable_id,
                &function.name,
                "function",
                function.span,
            )?;
        }
        for declaration in &program.types {
            put(
                &declaration.stable_id,
                &declaration.name,
                "type",
                declaration.span,
            )?;
            match &declaration.kind {
                TypeDeclarationKind::Record { fields }
                | TypeDeclarationKind::Class { fields, .. } => {
                    for field in fields {
                        put(&field.stable_id, &field.name, "field", field.span)?;
                    }
                    if let TypeDeclarationKind::Class { methods, .. } = &declaration.kind {
                        for method in methods {
                            put(&method.stable_id, &method.name, "function", method.span)?;
                        }
                    }
                }
                TypeDeclarationKind::Variant { cases } => {
                    for case in cases {
                        put(&case.stable_id, &case.name, "case", case.span)?;
                        for field in &case.fields {
                            put(&field.stable_id, &field.name, "field", field.span)?;
                        }
                    }
                }
                TypeDeclarationKind::Resource { lifecycles } => {
                    for lifecycle in lifecycles {
                        if let Some(id) = &lifecycle.stable_id {
                            put(id, "lifecycle", "lifecycle", lifecycle.span)?;
                        }
                    }
                }
            }
        }
        for interface in &program.interfaces {
            put(
                &interface.stable_id,
                &interface.name,
                "interface",
                interface.span,
            )?;
            for import in &interface.imports {
                put(&import.stable_id, &import.name, "import", import.span)?;
            }
        }
        for protocol in &program.protocols {
            put(
                &protocol.stable_id,
                &protocol.name,
                "protocol",
                protocol.span,
            )?;
            for method in &protocol.methods {
                put(
                    &method.stable_id,
                    &method.name,
                    "protocol_method",
                    method.span,
                )?;
            }
        }
        for implementation in &program.implementations {
            put(
                &implementation.stable_id,
                "impl",
                "protocol_implementation",
                implementation.span,
            )?;
        }
    }
    Ok(output)
}
fn compact_authored(value: &Value) -> Value {
    json!({"id":value["id"],"name":value["name"],"kind":value["kind"],"path":value["path"],"module":value["module"],"fragment_digest":value["fragment_digest"]})
}
fn source_binding(revision: &ProjectRevision, target: &str, authored: Option<&Value>) -> Value {
    let symbol = revision.semantic.image_symbol(target);
    let Some(path) = symbol
        .as_ref()
        .and_then(|symbol| symbol["path"].as_str())
        .or_else(|| authored.and_then(|fact| fact["path"].as_str()))
    else {
        return Value::Null;
    };
    revision.sources().iter().find(|source|source.path()==path).map(|source|json!({"path":path,"source_graph_schema":source.source_graph_schema(),"source_revision":source.source_revision(),"source_digest":source.source_digest()})).unwrap_or(Value::Null)
}
fn target_id(id: &str) -> Result<()> {
    if id.is_empty() || id.len() > 4096 || id.contains('\0') {
        Err(invalid("semantic delta target must be a bounded stable ID"))
    } else {
        Ok(())
    }
}
fn presence(before: bool, after: bool) -> &'static str {
    match (before, after) {
        (false, true) => "added",
        (true, false) => "removed",
        (true, true) => "modified",
        _ => "absent",
    }
}
fn render(value: Value, bound: usize) -> Result<String> {
    wire::render(value, bound)
        .map_err(|_| capacity("semantic delta exceeds its exact output byte bound"))
}
fn invalid(message: &'static str) -> Vec<Diagnostic> {
    vec![Diagnostic::io("SPX-G252", message)]
}
fn capacity(message: &'static str) -> Vec<Diagnostic> {
    vec![Diagnostic::io("SPX-G253", message)]
}
fn stale(message: &'static str) -> Vec<Diagnostic> {
    vec![Diagnostic::io("SPX-G254", message)]
}
