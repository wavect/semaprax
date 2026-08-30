//! Complete source-backed static implementation comparisons. No dispatch proof.
use std::collections::{BTreeMap, BTreeSet};
use std::io::{self, Write};
use std::sync::Arc;

use serde_json::{json, Value};

use super::{parse_revision, wire, ProjectCandidate};
use crate::ast::{Function, Span, TypeDeclarationKind};
use crate::diagnostic::Diagnostic;
use crate::hir::{self, OwnershipMode, ResolvedExpr, ResolvedParam, ResolvedType};
use crate::project::ProjectRevision;

type Result<T> = std::result::Result<T, Vec<Diagnostic>>;
pub const PROJECT_CANDIDATE_INTERFACE_DELTA_SCHEMA: &str =
    "semaprax.project-candidate-interface-delta.v1";
pub const PROJECT_CANDIDATE_INTERFACE_DELTA_VERIFICATION_SCHEMA: &str =
    "semaprax.project-candidate-interface-delta-verification.v1";
pub const MAX_PROJECT_CANDIDATE_INTERFACE_DELTA_BYTES: usize = 8 * 1024 * 1024;
const MAX_ITEMS: usize = 65_536;
const MAX_CALLS: usize = 1_048_576;
const MAX_WORK: usize = 1_048_576;
const MAX_DEPTH: usize = 256;
const FACT_DOMAIN: &[u8] = b"semaprax.candidate-interface-delta.fact.v1\0";
const SOURCE_DOMAIN: &[u8] = b"semaprax.candidate-interface-delta.source.v1\0";
const REPORT_DOMAIN: &[u8] = b"semaprax.candidate-interface-delta.report.v1\0";

#[derive(Default)]
struct Inventory {
    protocols: BTreeMap<String, Value>,
    implementations: BTreeMap<String, Value>,
    functions: BTreeMap<String, Value>,
    receivers: BTreeMap<String, Value>,
    calls: BTreeMap<String, BTreeSet<String>>,
    function_digests: BTreeMap<String, String>,
}

// Charges serialized retained facts before cloning them into repeated rows.
// This is a logical work/storage bound, not an allocator or RSS assertion.
#[derive(Default)]
struct Budget {
    bytes: usize,
    items: usize,
    calls: usize,
    walks: usize,
    visits: usize,
}
impl Budget {
    fn fact(&mut self, value: &Value) -> Result<()> {
        struct Count(usize);
        impl Write for Count {
            fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
                if bytes.len() > MAX_PROJECT_CANDIDATE_INTERFACE_DELTA_BYTES.saturating_sub(self.0)
                {
                    return Err(io::Error::other("interface fact size"));
                }
                self.0 += bytes.len();
                Ok(bytes.len())
            }
            fn flush(&mut self) -> io::Result<()> {
                Ok(())
            }
        }
        let mut count = Count(1); // canonical report terminal LF
        serde_json::to_writer(&mut count, value).map_err(|_| capacity())?;
        let bytes = count.0;
        self.bytes = self.bytes.checked_add(bytes).ok_or_else(capacity)?;
        self.items = self.items.checked_add(1).ok_or_else(capacity)?;
        if self.bytes > 32 * 1024 * 1024 || self.items > MAX_ITEMS {
            return Err(capacity());
        }
        Ok(())
    }
    fn copy(&mut self, value: &Value) -> Result<Value> {
        self.fact(value)?;
        Ok(value.clone())
    }
}

impl ProjectCandidate {
    /// Compare every source protocol and implementation, including every
    /// required member of each affected implementation, across the candidate.
    pub fn interface_delta(&self, expected_candidate: &str) -> Result<String> {
        self.require_candidate(expected_candidate)?;
        let mut budget = Budget::default();
        let before = inventory(&self.base, &mut budget)?;
        let after = inventory(&self.revision, &mut budget)?;
        let mut protocols = Vec::new();
        let mut unchanged_protocols = 0usize;
        for id in union(&before.protocols, &after.protocols) {
            let comparison = pair(
                before.protocols.get(&id),
                after.protocols.get(&id),
                &mut budget,
            )?;
            if comparison["exact_equal"] == true {
                unchanged_protocols += 1;
            } else {
                protocols.push(json!({"id":id,"comparison":comparison}));
            }
        }
        let mut implementations = Vec::new();
        let mut unchanged_implementations = 0usize;
        for id in union(&before.implementations, &after.implementations) {
            let base = implementation(&before, &id, &mut budget)?;
            let candidate = implementation(&after, &id, &mut budget)?;
            let comparison = pair(base.as_ref(), candidate.as_ref(), &mut budget)?;
            if comparison["exact_equal"] == true {
                unchanged_implementations += 1;
                continue;
            }
            let base_members = member_map(base.as_ref(), &mut budget)?;
            let candidate_members = member_map(candidate.as_ref(), &mut budget)?;
            let mut members = Vec::new();
            for method in union(&base_members, &candidate_members) {
                members.push(json!({"method_id":method,"comparison":pair(base_members.get(&method),candidate_members.get(&method),&mut budget)?}));
            }
            implementations.push(json!({"id":id,"comparison":comparison,"members":members}));
        }
        render(json!({
            "schema":PROJECT_CANDIDATE_INTERFACE_DELTA_SCHEMA,"candidate_digest":self.candidate_digest(),
            "base_project_revision":self.base.project_revision(),"project_revision":self.revision.project_revision(),
            "base_workspace_revision":self.base.workspace_revision(),"workspace_revision":self.revision.workspace_revision(),
            "source_bindings":{"base":sources(&self.base),"candidate":sources(&self.revision)},
            "protocols":protocols,"implementations":implementations,
            "inventory":{"base_protocols":before.protocols.len(),"candidate_protocols":after.protocols.len(),
                "base_implementations":before.implementations.len(),"candidate_implementations":after.implementations.len(),
                "unchanged_protocols":unchanged_protocols,"unchanged_implementations":unchanged_implementations},
            "evidence_class":"descriptive_source_backed_static_conformance_delta",
            "selection_basis":"all_protocols_and_implementations_with_complete_required_member_union",
            "dependency_basis":"transitive_retained_HIR_direct_calls_in_bodies_and_contracts",
            "source_authority":false,"execution":false,
            "limits":{"max_report_bytes":MAX_PROJECT_CANDIDATE_INTERFACE_DELTA_BYTES,"max_items":MAX_ITEMS,
                "max_call_edges":MAX_CALLS,"max_dependency_walks":MAX_WORK,"max_expression_visits":MAX_WORK,"max_depth":MAX_DEPTH,"max_fact_work_bytes":32*1024*1024},
            "nonclaims":["not_behavioral_equivalence","no_dynamic_dispatch_or_runtime_witness","no_external_dependency_body_facts",
                "no_target_or_test_execution","not_complete_runtime_impact_or_coverage","not_allocator_or_RSS_accounting","no_publication_authority"]
        }))
    }

    /// Rebuild the complete candidate before comparing exact report bytes.
    /// Submitted report bytes are never parsed into facts, source, or HIR.
    pub fn verify_interface_delta(&self, expected_candidate: &str, bytes: &[u8]) -> Result<String> {
        self.require_candidate(expected_candidate)?;
        if bytes.len() > MAX_PROJECT_CANDIDATE_INTERFACE_DELTA_BYTES {
            return Err(capacity());
        }
        let replay = Self::replay(
            Arc::clone(&self.base),
            self.base.project_revision(),
            &self.changes,
            self.to_json().as_bytes(),
        )?;
        if replay.interface_delta(expected_candidate)?.as_bytes() != bytes {
            return Err(vec![Diagnostic::io(
                "SPX-G312",
                "interface delta failed exact independent candidate replay",
            )]);
        }
        render(
            json!({"schema":PROJECT_CANDIDATE_INTERFACE_DELTA_VERIFICATION_SCHEMA,
            "result":"exact_recomputation","candidate_digest":expected_candidate,
            "base_project_revision":self.base.project_revision(),"project_revision":self.revision.project_revision(),
            "delta_digest":wire::digest(REPORT_DOMAIN,bytes),"execution":false,"source_authority":false}),
        )
    }
}

fn inventory(revision: &ProjectRevision, budget: &mut Budget) -> Result<Inventory> {
    let mut result = Inventory::default();
    for program in parse_revision(revision)? {
        let source = revision
            .sources()
            .iter()
            .find(|s| s.path() == program.path)
            .ok_or_else(invalid)?;
        let provenance = |span: Span| {
            json!({"path":source.path(),"module":program.module,
            "source_revision":source.source_revision(),"source_digest":source.source_digest(),"span":{"start":span.start,"end":span.end}})
        };
        let declarations = crate::static_protocol::declaration_facts(&program)?;
        for protocol in declarations["protocols"].as_array().ok_or_else(invalid)? {
            let id = protocol["id"].as_str().ok_or_else(invalid)?;
            let ast = program
                .protocols
                .iter()
                .find(|p| p.stable_id == id)
                .ok_or_else(invalid)?;
            let fact = json!({"id":id,"declaration":protocol,"provenance":provenance(ast.span)});
            insert(&mut result.protocols, id, fact, budget)?;
        }
        for implementation in declarations["implementations"]
            .as_array()
            .ok_or_else(invalid)?
        {
            let id = implementation["id"].as_str().ok_or_else(invalid)?;
            let ast = program
                .implementations
                .iter()
                .find(|i| i.stable_id == id)
                .ok_or_else(invalid)?;
            let fact =
                json!({"id":id,"declaration":implementation,"provenance":provenance(ast.span)});
            insert(&mut result.implementations, id, fact, budget)?;
        }
        for ty in &program.types {
            let fact = json!({"id":ty.stable_id,"name":ty.name,"declaration_digest":fragment(source.source(),ty.span)?,"provenance":provenance(ty.span)});
            insert(&mut result.receivers, &ty.stable_id, fact, budget)?;
        }
        let mut functions = program.functions.iter().collect::<Vec<_>>();
        for ty in &program.types {
            if let TypeDeclarationKind::Class { methods, .. } = &ty.kind {
                functions.extend(methods);
            }
        }
        for function in functions {
            let fact = source_function(function, source.source(), provenance(function.span))?;
            insert(&mut result.functions, &function.stable_id, fact, budget)?;
        }
    }
    for module in revision.semantic.image_modules() {
        for function in module.functions() {
            checked_function(
                &mut result,
                function.id.as_str(),
                &function.params,
                &function.return_type,
                &function.effects,
                &function.requires,
                &function.body,
                &function.ensures,
                budget,
            )?;
            if let Some(fact) = result.functions.get_mut(function.id.as_str()) {
                fact["cleanup_plan"] =
                    plan(|| crate::graph_cleanup::cleanup_plan_json(&function.cleanup_plan))?;
                fact["loan_plan"] =
                    plan(|| crate::graph_loan::loan_plan_json(&function.loan_plan))?;
                budget.fact(fact)?;
            }
        }
        for function in module.function_templates() {
            checked_function(
                &mut result,
                function.id.as_str(),
                &function.params,
                &function.return_type,
                &function.effects,
                &function.requires,
                &function.body,
                &function.ensures,
                budget,
            )?;
        }
    }
    for (id, fact) in &result.functions {
        result
            .function_digests
            .insert(id.clone(), fact_digest(normalize(fact.clone()))?);
    }
    Ok(result)
}

fn source_function(function: &Function, source: &str, provenance: Value) -> Result<Value> {
    Ok(
        json!({"id":function.stable_id,"name":function.name,"declaration_digest":fragment(source,function.span)?,
        "body_digest":fragment(source,function.body.span)?,
        "requires":function.requires.iter().map(|e|fragment(source,e.span)).collect::<Result<Vec<_>>>()?,
        "ensures":function.ensures.iter().map(|e|fragment(source,e.span)).collect::<Result<Vec<_>>>()?,
        "provenance":provenance,"checked_signature":Value::Null}),
    )
}

#[allow(clippy::too_many_arguments)]
fn checked_function(
    inventory: &mut Inventory,
    id: &str,
    params: &[ResolvedParam],
    result: &ResolvedType,
    effects: &[String],
    requires: &[ResolvedExpr],
    body: &ResolvedExpr,
    ensures: &[ResolvedExpr],
    budget: &mut Budget,
) -> Result<()> {
    let mut calls = BTreeSet::new();
    for expression in requires.iter().chain(std::iter::once(body)).chain(ensures) {
        let mut pending = vec![(expression, 0usize)];
        while let Some((node, depth)) = pending.pop() {
            budget.visits = budget.visits.saturating_add(1);
            if budget.visits > MAX_WORK || depth > MAX_DEPTH {
                return Err(capacity());
            }
            if let hir::ResolvedExprKind::Call { callee, .. } = &node.kind {
                budget.calls = budget.calls.saturating_add(1);
                if budget.calls > MAX_CALLS {
                    return Err(capacity());
                }
                calls.insert(callee.as_str().to_owned());
            }
            let mut children = Vec::new();
            hir::push_resolved_expression_children_in_authored_order(node, &mut children);
            if children.len()
                > MAX_WORK
                    .saturating_sub(budget.visits)
                    .saturating_sub(pending.len())
            {
                return Err(capacity());
            }
            pending.extend(children.into_iter().map(|child| (child, depth + 1)));
        }
    }
    if let Some(fact) = inventory.functions.get_mut(id) {
        fact["direct_calls"] = json!(calls);
    }
    if inventory.calls.insert(id.to_owned(), calls).is_some() {
        return Err(invalid());
    }
    if let Some(fact) = inventory.functions.get_mut(id) {
        fact["checked_signature"] = json!({"parameters":params.iter().map(|p|json!({"name":p.name,"type_id":p.ty.identity_key(),"ownership":ownership(p.ownership)})).collect::<Vec<_>>(),
            "return_type_id":result.identity_key(),"effects":effects});
        budget.fact(fact)?;
    }
    Ok(())
}

fn implementation(inventory: &Inventory, id: &str, budget: &mut Budget) -> Result<Option<Value>> {
    let Some(binding) = inventory.implementations.get(id) else {
        return Ok(None);
    };
    let declaration = &binding["declaration"];
    let protocol = inventory
        .protocols
        .get(declaration["protocol_id"].as_str().ok_or_else(invalid)?)
        .ok_or_else(invalid)?;
    let receiver = inventory
        .receivers
        .get(declaration["receiver_id"].as_str().ok_or_else(invalid)?)
        .ok_or_else(invalid)?;
    let requirements = protocol["declaration"]["methods"]
        .as_array()
        .ok_or_else(invalid)?;
    let mut members = Vec::new();
    for requirement in requirements {
        let method = requirement["id"].as_str().ok_or_else(invalid)?;
        let binding = declaration["members"]
            .as_array()
            .ok_or_else(invalid)?
            .iter()
            .find(|m| m["method_id"] == method)
            .ok_or_else(invalid)?;
        let function_id = binding["function_id"].as_str().ok_or_else(invalid)?;
        let function = inventory.functions.get(function_id).ok_or_else(invalid)?;
        if function["checked_signature"].is_null() {
            return Err(invalid());
        }
        members.push(json!({"method_id":method,"requirement":budget.copy(requirement)?,"function_id":function_id,
            "function":budget.copy(function)?,"dependencies":dependencies(inventory,function_id,budget)?}));
    }
    let value = json!({"binding":budget.copy(binding)?,"protocol":budget.copy(protocol)?,"receiver":budget.copy(receiver)?,"members":members});
    budget.fact(&value)?;
    Ok(Some(value))
}

fn dependencies(inventory: &Inventory, root: &str, budget: &mut Budget) -> Result<Vec<Value>> {
    let mut seen = BTreeSet::new();
    let mut pending = vec![root.to_owned()];
    while let Some(id) = pending.pop() {
        budget.walks = budget.walks.saturating_add(1);
        if budget.walks > MAX_WORK {
            return Err(capacity());
        }
        if !seen.insert(id.clone()) {
            continue;
        }
        if seen.len() > MAX_ITEMS {
            return Err(capacity());
        }
        if let Some(calls) = inventory.calls.get(&id) {
            if calls.len()
                > MAX_WORK
                    .saturating_sub(budget.walks)
                    .saturating_sub(pending.len())
            {
                return Err(capacity());
            }
            pending.extend(calls.iter().cloned());
        }
    }
    seen.into_iter()
        .map(|id| {
            let digest = inventory.function_digests.get(&id);
        let function=inventory.functions.get(&id);
        let provenance=match function { Some(fact)=>budget.copy(&fact["provenance"])?,None=>Value::Null };
        let row = json!({"id":id,"fact_digest":digest,"provenance":provenance,
            "reason":if id==root {"implementation_member_root"} else {"reachable_via_retained_HIR_direct_calls"},
            "fact_availability":if function.is_some() {"retained_source_callable"}else{"external_or_unretained_callable"},
            "evidence_owner":"validated_workspace_HIR_and_canonical_source",
            "evidence_class":"descriptive_static_call_dependency_not_execution_or_dispatch"});
            budget.fact(&row)?;
            Ok(row)
        })
        .collect()
}

fn member_map(value: Option<&Value>, budget: &mut Budget) -> Result<BTreeMap<String, Value>> {
    let mut result = BTreeMap::new();
    if let Some(value) = value {
        for member in value["members"].as_array().ok_or_else(invalid)? {
            let id = member["method_id"].as_str().ok_or_else(invalid)?;
            if result.insert(id.to_owned(), budget.copy(member)?).is_some() {
                return Err(invalid());
            }
        }
    }
    Ok(result)
}
fn pair(base: Option<&Value>, candidate: Option<&Value>, budget: &mut Budget) -> Result<Value> {
    let base = budget.copy(base.unwrap_or(&Value::Null))?;
    let candidate = budget.copy(candidate.unwrap_or(&Value::Null))?;
    let equal = base == candidate;
    let projection = normalize(base.clone()) == normalize(candidate.clone());
    Ok(
        json!({"change":if equal{"unchanged"}else if projection{"provenance_only"}else if base.is_null(){"added"}else if candidate.is_null(){"removed"}else{"modified"},
        "exact_equal":equal,"projection_equal_without_provenance":projection,
        "base_digest":fact_digest(base.clone())?,"candidate_digest":fact_digest(candidate.clone())?,"base":base,"candidate":candidate}),
    )
}
fn normalize(mut value: Value) -> Value {
    match &mut value {
        Value::Object(fields) => {
            for key in [
                "provenance",
                "span",
                "source_span",
                "source_revision",
                "source_digest",
                "expression_id",
                "exact_digest",
            ] {
                fields.remove(key);
            }
            for value in fields.values_mut() {
                *value = normalize(std::mem::take(value));
            }
        }
        Value::Array(values) => {
            for value in values {
                *value = normalize(std::mem::take(value));
            }
        }
        _ => {}
    }
    value
}
fn plan(produce: impl FnOnce() -> String) -> Result<Value> {
    let (text, overflow) =
        crate::bounded_output::with_limit(MAX_PROJECT_CANDIDATE_INTERFACE_DELTA_BYTES, produce);
    if overflow {
        return Err(capacity());
    }
    let value: Value = serde_json::from_str(&text).map_err(|_| invalid())?;
    let schema = value["schema"].clone();
    Ok(
        json!({"schema":schema,"exact_digest":wire::digest(FACT_DOMAIN,text.as_bytes()),"projection_digest":fact_digest(normalize(value))?}),
    )
}
fn sources(revision: &ProjectRevision) -> Vec<Value> {
    revision.sources().iter().map(|s|json!({"path":s.path(),"source_revision":s.source_revision(),"source_digest":s.source_digest()})).collect()
}
fn union(a: &BTreeMap<String, Value>, b: &BTreeMap<String, Value>) -> BTreeSet<String> {
    a.keys().chain(b.keys()).cloned().collect()
}
fn insert(
    map: &mut BTreeMap<String, Value>,
    id: &str,
    value: Value,
    budget: &mut Budget,
) -> Result<()> {
    budget.fact(&value)?;
    if map.insert(id.to_owned(), value).is_some() {
        return Err(invalid());
    }
    Ok(())
}
fn fragment(source: &str, span: Span) -> Result<String> {
    Ok(wire::digest(
        SOURCE_DOMAIN,
        source
            .get(span.start..span.end)
            .ok_or_else(invalid)?
            .as_bytes(),
    ))
}
fn fact_digest(value: Value) -> Result<String> {
    Ok(wire::digest(FACT_DOMAIN, render(value)?.as_bytes()))
}
fn render(value: Value) -> Result<String> {
    wire::render(value, MAX_PROJECT_CANDIDATE_INTERFACE_DELTA_BYTES).map_err(|_| capacity())
}
fn ownership(mode: OwnershipMode) -> &'static str {
    match mode {
        OwnershipMode::Value => "value",
        OwnershipMode::Own => "own",
        OwnershipMode::Borrow => "borrow",
        OwnershipMode::Shared => "shared",
    }
}
fn invalid() -> Vec<Diagnostic> {
    vec![Diagnostic::io(
        "SPX-G310",
        "source-backed interface delta inventory is inconsistent",
    )]
}
fn capacity() -> Vec<Diagnostic> {
    vec![Diagnostic::io(
        "SPX-G311",
        "interface delta exceeds its bounded inventory, work, or output",
    )]
}
