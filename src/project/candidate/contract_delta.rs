//! Whole-candidate, source-bound contract comparisons. No predicate is proved.

use std::collections::{BTreeMap, BTreeSet};
use std::io::{self, Write};
use std::sync::Arc;

use serde_json::{json, Value};

use super::{interface_delta, parse_revision, wire, ProjectCandidate};
use crate::ast::{Span, TypeDeclarationKind};
use crate::diagnostic::Diagnostic;
use crate::hir::{self, ResolvedExpr, ResolvedMatchPattern, ResolvedRecordMatchFieldPattern};
use crate::project::{ProjectRevision, ProjectSource};

type Result<T> = std::result::Result<T, Vec<Diagnostic>>;
pub const PROJECT_CANDIDATE_CONTRACT_DELTA_SCHEMA: &str =
    "semaprax.project-candidate-contract-delta.v1";
pub const PROJECT_CANDIDATE_CONTRACT_DELTA_VERIFICATION_SCHEMA: &str =
    "semaprax.project-candidate-contract-delta-verification.v1";
pub const MAX_PROJECT_CANDIDATE_CONTRACT_DELTA_BYTES: usize = 8 * 1024 * 1024;
const MAX_ITEMS: usize = 65_536;
const MAX_WORK: usize = 1_048_576;
const MAX_DEPTH: usize = 256;
const MAX_FACT_BYTES: usize = 32 * 1024 * 1024;
const FACT_DOMAIN: &[u8] = b"semaprax.candidate-contract-delta.fact.v1\0";
const SOURCE_DOMAIN: &[u8] = b"semaprax.candidate-contract-delta.source.v1\0";
const PREDICATE_DOMAIN: &[u8] = b"semaprax.candidate-contract-delta.predicate.v1\0";
const REPORT_DOMAIN: &[u8] = b"semaprax.candidate-contract-delta.report.v1\0";

#[derive(Default)]
struct Budget {
    bytes: usize,
    items: usize,
    visits: usize,
    walks: usize,
}
impl Budget {
    fn fact<T: serde::Serialize + ?Sized>(&mut self, value: &T) -> Result<()> {
        struct Count(usize);
        impl Write for Count {
            fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
                if bytes.len() > MAX_PROJECT_CANDIDATE_CONTRACT_DELTA_BYTES.saturating_sub(self.0) {
                    return Err(io::Error::other("contract fact bound"));
                }
                self.0 += bytes.len();
                Ok(bytes.len())
            }
            fn flush(&mut self) -> io::Result<()> {
                Ok(())
            }
        }
        let mut count = Count(1);
        serde_json::to_writer(&mut count, value).map_err(|_| capacity())?;
        self.bytes = self.bytes.checked_add(count.0).ok_or_else(capacity)?;
        self.items = self.items.checked_add(1).ok_or_else(capacity)?;
        if self.bytes > MAX_FACT_BYTES || self.items > MAX_ITEMS {
            return Err(capacity());
        }
        Ok(())
    }
    fn copy(&mut self, value: &Value) -> Result<Value> {
        self.fact(value)?;
        Ok(value.clone())
    }
    fn visit(&mut self, depth: usize) -> Result<()> {
        if self.visits >= MAX_WORK || depth > MAX_DEPTH {
            return Err(capacity());
        }
        self.visits += 1;
        Ok(())
    }
}

#[derive(Default)]
struct Inventory {
    functions: BTreeMap<String, Value>,
    total: usize,
    predicates: usize,
    with_contracts: usize,
    source_only: usize,
}
struct Checked<'a> {
    requires: &'a [ResolvedExpr],
    ensures: &'a [ResolvedExpr],
    availability: &'static str,
}

impl ProjectCandidate {
    pub fn contract_delta(&self, expected_candidate: &str) -> Result<String> {
        self.require_candidate(expected_candidate)?;
        let mut budget = Budget::default();
        let before = inventory(&self.base, &mut budget)?;
        let after = inventory(&self.revision, &mut budget)?;
        let ids = before
            .functions
            .keys()
            .chain(after.functions.keys())
            .cloned()
            .collect::<BTreeSet<_>>();
        let mut functions = Vec::new();
        let mut unchanged = 0;
        for id in ids {
            let base = before.functions.get(&id);
            let candidate = after.functions.get(&id);
            if base.is_none_or(|side| side["predicates"].as_array().is_some_and(Vec::is_empty))
                && candidate
                    .is_none_or(|side| side["predicates"].as_array().is_some_and(Vec::is_empty))
            {
                continue;
            }
            if base == candidate {
                unchanged += 1;
                continue;
            }
            let (change, comparison) = comparison(base, candidate, &mut budget)?;
            let row = json!({"id":id,"change":change,"comparison":comparison,
                "base":budget.copy(base.unwrap_or(&Value::Null))?,"candidate":budget.copy(candidate.unwrap_or(&Value::Null))?});
            budget.fact(&row)?;
            functions.push(row);
        }
        render(json!({"schema":PROJECT_CANDIDATE_CONTRACT_DELTA_SCHEMA,
            "candidate_digest":expected_candidate,"base_project_revision":self.base.project_revision(),"project_revision":self.revision.project_revision(),
            "base_workspace_revision":self.base.workspace_revision(),"workspace_revision":self.revision.workspace_revision(),
            "source_bindings":{"base":sources(&self.base),"candidate":sources(&self.revision)},
            "inventory":{"base_functions":before.total,"candidate_functions":after.total,"base_predicates":before.predicates,"candidate_predicates":after.predicates,
                "base_functions_with_contracts":before.with_contracts,"candidate_functions_with_contracts":after.with_contracts,
                "unchanged_functions":unchanged,"affected_functions":functions.len(),"base_source_only_functions":before.source_only,"candidate_source_only_functions":after.source_only},
            "functions":functions,"predicate_addressing":"phase_and_ordinal_within_each_revision",
            "selection_basis":"all_source_callable_requires_and_ensures_with_complete_ordered_affected_inventories",
            "dependency_basis":"predicate_direct_calls_then_retained_body_and_contract_direct_call_closure",
            "evidence_class":"descriptive_source_and_checked_HIR_contract_delta","execution":false,"source_authority":false,
            "limits":{"max_report_bytes":MAX_PROJECT_CANDIDATE_CONTRACT_DELTA_BYTES,"max_items":MAX_ITEMS,"max_expression_and_pattern_visits":MAX_WORK,"max_dependency_walks":MAX_WORK,"max_depth":MAX_DEPTH,"max_fact_work_bytes":MAX_FACT_BYTES,"shared_callable_inventory_work_bytes_per_revision":MAX_FACT_BYTES},
            "nonclaims":["not_predicate_truth_or_implication","not_strengthening_or_weakening_proof","not_behavioral_equivalence","not_test_coverage_or_execution","no_external_callable_body_facts","no_runtime_contract_failure_prediction","no_source_or_publication_authority","not_allocator_or_RSS_accounting"]}))
    }

    pub fn verify_contract_delta(&self, expected_candidate: &str, bytes: &[u8]) -> Result<String> {
        self.require_candidate(expected_candidate)?;
        if bytes.len() > MAX_PROJECT_CANDIDATE_CONTRACT_DELTA_BYTES {
            return Err(capacity());
        }
        let replay = Self::replay(
            Arc::clone(&self.base),
            self.base.project_revision(),
            &self.changes,
            self.to_json().as_bytes(),
        )?;
        if replay.contract_delta(expected_candidate)?.as_bytes() != bytes {
            return Err(vec![Diagnostic::io(
                "SPX-G327",
                "contract delta failed exact independent candidate replay",
            )]);
        }
        render(
            json!({"schema":PROJECT_CANDIDATE_CONTRACT_DELTA_VERIFICATION_SCHEMA,"result":"exact_recomputation",
            "candidate_digest":expected_candidate,"base_project_revision":self.base.project_revision(),"project_revision":self.revision.project_revision(),
            "delta_digest":wire::digest(REPORT_DOMAIN,bytes),"execution":false,"source_authority":false}),
        )
    }
}

fn inventory(revision: &ProjectRevision, budget: &mut Budget) -> Result<Inventory> {
    // Reuse the existing bounded callable fact/adjacency collector. Do not
    // construct another whole-workspace call index for this report.
    let callables = interface_delta::callable_facts(revision)?;
    let mut checked = BTreeMap::new();
    for module in revision.semantic.image_modules() {
        for function in module.functions() {
            if checked
                .insert(
                    function.id.as_str(),
                    Checked {
                        requires: &function.requires,
                        ensures: &function.ensures,
                        availability: "retained_checked_function",
                    },
                )
                .is_some()
            {
                return Err(invalid());
            }
        }
        for function in module.function_templates() {
            if checked
                .insert(
                    function.id.as_str(),
                    Checked {
                        requires: &function.requires,
                        ensures: &function.ensures,
                        availability: "retained_checked_template",
                    },
                )
                .is_some()
            {
                return Err(invalid());
            }
        }
        if checked.len() > MAX_ITEMS {
            return Err(capacity());
        }
    }
    let mut result = Inventory::default();
    for program in parse_revision(revision)? {
        let source = revision
            .sources()
            .iter()
            .find(|source| source.path() == program.path)
            .ok_or_else(invalid)?;
        let mut functions = program.functions.iter().collect::<Vec<_>>();
        for ty in &program.types {
            if let TypeDeclarationKind::Class { methods, .. } = &ty.kind {
                functions.extend(methods);
            }
        }
        for function in functions {
            result.total += 1;
            if result.total > MAX_ITEMS {
                return Err(capacity());
            }
            let checked = checked.get(function.stable_id.as_str());
            if checked.is_none() {
                result.source_only += 1;
            }
            if !function.requires.is_empty() || !function.ensures.is_empty() {
                result.with_contracts += 1;
            }
            if checked.is_some_and(|c| {
                c.requires.len() != function.requires.len()
                    || c.ensures.len() != function.ensures.len()
            }) {
                return Err(invalid());
            }
            let mut predicates = Vec::new();
            for (phase, expressions, resolved) in [
                ("requires", &function.requires, checked.map(|c| c.requires)),
                ("ensures", &function.ensures, checked.map(|c| c.ensures)),
            ] {
                for (index, expression) in expressions.iter().enumerate() {
                    result.predicates += 1;
                    if result.predicates > MAX_ITEMS {
                        return Err(capacity());
                    }
                    let resolved = resolved.map(|values| &values[index]);
                    if resolved.is_some_and(|r| r.span != expression.span) {
                        return Err(invalid());
                    }
                    let fragment = source
                        .source()
                        .get(expression.span.start..expression.span.end)
                        .ok_or_else(invalid)?;
                    if fragment.len() > MAX_PROJECT_CANDIDATE_CONTRACT_DELTA_BYTES {
                        return Err(capacity());
                    }
                    budget.fact(fragment)?;
                    let (projection, roots) = if let Some(resolved) = resolved {
                        let roots = preflight(resolved, budget)?;
                        let (projection, overflow) = crate::bounded_output::with_limit(
                            MAX_PROJECT_CANDIDATE_CONTRACT_DELTA_BYTES,
                            || crate::graph::agent_contract_expr_json(resolved),
                        );
                        if overflow {
                            return Err(capacity());
                        }
                        let projection = projection.map_err(|error| vec![error])?;
                        // Only compiler-created bounded projection JSON is read.
                        let value: Value =
                            serde_json::from_str(&projection).map_err(|_| capacity())?;
                        budget.fact(&value)?;
                        (value, roots)
                    } else {
                        (Value::Null, BTreeSet::new())
                    };
                    let projection_digest = wire::digest(
                        PREDICATE_DOMAIN,
                        render(budget.copy(&projection)?)?.as_bytes(),
                    );
                    let row = json!({"phase":phase,"index":index,"expression_id":resolved.map(|e|e.id.as_str()),"type_id":resolved.map(|e|e.ty.identity_key()),
                        "source_fragment":fragment,"source_fragment_digest":wire::digest(SOURCE_DOMAIN,fragment.as_bytes()),
                        "provenance":provenance(source,&program.module,expression.span),"expression":projection,"projection_digest":projection_digest,
                        "dependencies":dependencies(&callables,&roots,budget)?,"hir_availability":checked.map_or("source_only",|c|c.availability)});
                    budget.fact(&row)?;
                    predicates.push(row);
                }
            }
            let row = json!({"id":function.stable_id,"name":function.name,"provenance":provenance(source,&program.module,function.span),"predicates":predicates});
            budget.fact(&row)?;
            if result
                .functions
                .insert(function.stable_id.clone(), row)
                .is_some()
            {
                return Err(invalid());
            }
        }
    }
    Ok(result)
}

fn dependencies(
    callables: &interface_delta::CallableFacts,
    roots: &BTreeSet<String>,
    budget: &mut Budget,
) -> Result<Vec<Value>> {
    let mut seen = BTreeSet::new();
    let mut pending = roots.iter().cloned().collect::<Vec<_>>();
    while let Some(id) = pending.pop() {
        if budget.walks >= MAX_WORK {
            return Err(capacity());
        }
        budget.walks += 1;
        if !seen.insert(id.clone()) {
            continue;
        }
        if seen.len() > MAX_ITEMS {
            return Err(capacity());
        }
        if let Some(children) = callables.calls.get(&id) {
            if children.len()
                > MAX_WORK
                    .saturating_sub(budget.walks)
                    .saturating_sub(pending.len())
            {
                return Err(capacity());
            }
            pending.extend(children.iter().cloned());
        }
    }
    seen.into_iter().map(|id| {
        let fact = callables.functions.get(&id);
        let provenance = fact.map(|fact| budget.copy(&fact["provenance"])).transpose()?.unwrap_or(Value::Null);
        let row = json!({"id":id,"fact_digest":callables.digests.get(&id),"provenance":provenance,
            "reason":if roots.contains(&id){"contract_direct_callee"}else{"transitive_contract_callee"},
            "fact_availability":if fact.is_some(){"retained_source_callable"}else{"external_or_unretained_callable"},
            "evidence_owner":"validated_workspace_HIR_and_canonical_source"});
        budget.fact(&row)?; Ok(row)
    }).collect()
}

fn preflight(root: &ResolvedExpr, budget: &mut Budget) -> Result<BTreeSet<String>> {
    let mut pending = vec![(root, 0usize)];
    let mut calls = BTreeSet::new();
    while let Some((node, depth)) = pending.pop() {
        budget.visit(depth)?;
        if let hir::ResolvedExprKind::Call { callee, .. } = &node.kind {
            if calls.len() >= MAX_ITEMS && !calls.contains(callee.as_str()) {
                return Err(capacity());
            }
            calls.insert(callee.as_str().to_owned());
        }
        if let hir::ResolvedExprKind::Match { arms, .. } = &node.kind {
            for arm in arms {
                pattern(&arm.pattern, depth + 1, budget)?;
            }
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
    Ok(calls)
}
fn pattern(value: &ResolvedMatchPattern, depth: usize, budget: &mut Budget) -> Result<()> {
    budget.visit(depth)?;
    match value {
        ResolvedMatchPattern::Record { fields, .. } => {
            for field in fields {
                record_pattern(&field.pattern, depth + 1, budget)?;
            }
        }
        ResolvedMatchPattern::Or(alternatives) => {
            for alternative in alternatives {
                pattern(alternative, depth + 1, budget)?;
            }
        }
        ResolvedMatchPattern::Variant { fields, .. } => {
            for _ in fields {
                budget.visit(depth + 1)?;
            }
        }
        ResolvedMatchPattern::Wildcard
        | ResolvedMatchPattern::Literal(_)
        | ResolvedMatchPattern::Binding(_) => {}
    }
    Ok(())
}
fn record_pattern(
    value: &ResolvedRecordMatchFieldPattern,
    depth: usize,
    budget: &mut Budget,
) -> Result<()> {
    budget.visit(depth)?;
    if let ResolvedRecordMatchFieldPattern::Record { fields, .. } = value {
        for field in fields {
            record_pattern(&field.pattern, depth + 1, budget)?;
        }
    }
    Ok(())
}

fn comparison(
    base: Option<&Value>,
    candidate: Option<&Value>,
    budget: &mut Budget,
) -> Result<(&'static str, Value)> {
    let exact = base == candidate;
    let mut projection_available = true;
    let mut views = |side: Option<&Value>| -> Result<(Value, Value, Value)> {
        let Some(side) = side else {
            return Ok((Value::Null, Value::Null, Value::Null));
        };
        let mut projections = Vec::new();
        let mut dependencies = Vec::new();
        let mut sources = Vec::new();
        for predicate in side["predicates"].as_array().ok_or_else(invalid)? {
            projection_available &= predicate["hir_availability"] != "source_only";
            projections.push(json!({"phase":predicate["phase"],"index":predicate["index"],"expression":budget.copy(&predicate["expression"])?}));
            let mut dependency_rows = Vec::new();
            for dep in predicate["dependencies"].as_array().ok_or_else(invalid)? {
                let row = json!({"id":budget.copy(&dep["id"])?,"fact_digest":budget.copy(&dep["fact_digest"])?,"fact_availability":budget.copy(&dep["fact_availability"])?});
                budget.fact(&row)?;
                dependency_rows.push(row);
            }
            dependencies.push(json!({"phase":predicate["phase"],"index":predicate["index"],"dependencies":dependency_rows}));
            sources.push(json!({"phase":predicate["phase"],"index":predicate["index"],"source_fragment_digest":predicate["source_fragment_digest"]}));
        }
        let result = (
            json!(projections),
            json!(dependencies),
            json!({"name":side["name"],"predicates":sources}),
        );
        budget.fact(&result.0)?;
        budget.fact(&result.1)?;
        budget.fact(&result.2)?;
        Ok(result)
    };
    let left = views(base)?;
    let right = views(candidate)?;
    let projection = projection_available.then_some(left.0 == right.0);
    let dependency = projection_available.then_some(left.1 == right.1);
    let source = left.2 == right.2;
    let mut reasons = Vec::new();
    if base.is_none() {
        reasons.push("added");
    } else if candidate.is_none() {
        reasons.push("removed");
    }
    if projection == Some(false) {
        reasons.push("predicate_projection_changed");
    }
    if projection.is_none() {
        reasons.push("predicate_projection_unavailable");
    }
    if dependency == Some(false) {
        reasons.push("dependency_changed");
    }
    if dependency.is_none() {
        reasons.push("dependency_unavailable");
    }
    if !source {
        reasons.push("source_changed");
    }
    if !exact && projection == Some(true) && dependency == Some(true) && source {
        reasons.push("provenance_only");
    }
    let change = if exact {
        "unchanged"
    } else if base.is_none() {
        "added"
    } else if candidate.is_none() {
        "removed"
    } else if reasons == ["provenance_only"] {
        "provenance_only"
    } else {
        "modified"
    };
    let base_digest = wire::digest(
        FACT_DOMAIN,
        render(budget.copy(base.unwrap_or(&Value::Null))?)?.as_bytes(),
    );
    let candidate_digest = wire::digest(
        FACT_DOMAIN,
        render(budget.copy(candidate.unwrap_or(&Value::Null))?)?.as_bytes(),
    );
    Ok((
        change,
        json!({"exact_equal":exact,"predicate_projection_equal":projection,"dependency_equal":dependency,"source_equal":source,"base_digest":base_digest,"candidate_digest":candidate_digest,"reasons":reasons}),
    ))
}

fn provenance(source: &ProjectSource, module: &str, span: Span) -> Value {
    json!({"path":source.path(),"module":module,"source_revision":source.source_revision(),"source_digest":source.source_digest(),"span":{"start":span.start,"end":span.end}})
}
fn sources(revision: &ProjectRevision) -> Vec<Value> {
    revision.sources().iter().map(|source|json!({"path":source.path(),"source_revision":source.source_revision(),"source_digest":source.source_digest()})).collect()
}
fn render(value: Value) -> Result<String> {
    wire::render(value, MAX_PROJECT_CANDIDATE_CONTRACT_DELTA_BYTES).map_err(|_| capacity())
}
fn invalid() -> Vec<Diagnostic> {
    vec![Diagnostic::io(
        "SPX-G325",
        "source-backed contract delta inventory is inconsistent",
    )]
}
fn capacity() -> Vec<Diagnostic> {
    vec![Diagnostic::io(
        "SPX-G326",
        "source-backed contract delta exceeds its bounded inventory or output",
    )]
}
