//! Bounded descriptive deltas recomputed from retained validated revisions.
//! Equality here is projection equality, never behavioral equivalence.
use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

use serde_json::{json, Value};

use crate::ast::{Span, TypeDeclarationKind};
use crate::diagnostic::Diagnostic;
use crate::hir::{
    self, PlaceProjection, ResolvedExpr, ResolvedExprKind as Expr, ResolvedMatchPattern,
    ResolvedRecordMatchFieldPattern, ResolvedRecordMatchPatternField, ResolvedStatement,
    ResolvedTypeDeclarationKind,
};
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
        if self.base.semantic.image_symbol(target).is_none()
            && self.revision.semantic.image_symbol(target).is_none()
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
        let before_authored = authored(&self.base)?;
        let after_authored = authored(&self.revision)?;
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
            "presence":presence(self.base.semantic.image_symbol(target).is_some(),self.revision.semantic.image_symbol(target).is_some()),
            "source_bindings":{"base":source_binding(&self.base,target),"candidate":source_binding(&self.revision,target)},
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
    let Some(symbol) = revision.semantic.image_symbol(target) else {
        return Ok(facts);
    };
    facts.insert("declaration_identity".to_owned(), symbol);
    facts.insert(
        "authored_declaration".to_owned(),
        authored.cloned().unwrap_or(Value::Null),
    );
    facts.insert(
        "typed_declaration".to_owned(),
        typed_declaration(revision, target),
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
    let relationships = reverse_relationships(revision, target)?;
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
    }
    Ok(output)
}
fn compact_authored(value: &Value) -> Value {
    json!({"id":value["id"],"name":value["name"],"kind":value["kind"],"path":value["path"],"module":value["module"],"fragment_digest":value["fragment_digest"]})
}
fn source_binding(revision: &ProjectRevision, target: &str) -> Value {
    let Some(symbol) = revision.semantic.image_symbol(target) else {
        return Value::Null;
    };
    let Some(path) = symbol["path"].as_str() else {
        return Value::Null;
    };
    revision.sources().iter().find(|source|source.path()==path).map(|source|json!({"path":path,"source_graph_schema":source.source_graph_schema(),"source_revision":source.source_revision(),"source_digest":source.source_digest()})).unwrap_or(Value::Null)
}
fn typed_declaration(revision: &ProjectRevision, target: &str) -> Value {
    fn field(field: &hir::ResolvedFieldDeclaration) -> Value {
        json!({"id":field.id.as_str(),"name":field.name,"type_id":field.ty.identity_key(),"index":field.index})
    }
    for module in revision.semantic.image_modules() {
        for declaration in module.types() {
            let fields = match &declaration.kind {
                ResolvedTypeDeclarationKind::Record { fields }
                | ResolvedTypeDeclarationKind::Class { fields, .. } => {
                    fields.iter().map(field).collect::<Vec<_>>()
                }
                ResolvedTypeDeclarationKind::Variant { cases } => cases
                    .iter()
                    .flat_map(|case| case.fields.iter().map(field))
                    .collect(),
                ResolvedTypeDeclarationKind::Resource { .. } => Vec::new(),
            };
            if let Some(field) = fields.iter().find(|field| field["id"] == target) {
                let owner = revision
                    .semantic
                    .image_symbol(target)
                    .map(|symbol| symbol["owner"].clone())
                    .unwrap_or(Value::Null);
                return json!({"kind":"field","owner":owner,"field":field});
            }
            if let ResolvedTypeDeclarationKind::Variant { cases } = &declaration.kind {
                if let Some(case) = cases.iter().find(|case| case.id.as_str() == target) {
                    return json!({"kind":"case","owner":declaration.id.as_str(),"id":target,"name":case.name,
                        "index":case.index,"fields":case.fields.iter().map(field).collect::<Vec<_>>()});
                }
            }
            if declaration.id.as_str() == target {
                let kind = match &declaration.kind {
                    ResolvedTypeDeclarationKind::Record { .. } => "record",
                    ResolvedTypeDeclarationKind::Class { .. } => "class",
                    ResolvedTypeDeclarationKind::Variant { .. } => "variant",
                    ResolvedTypeDeclarationKind::Resource { .. } => "resource",
                };
                return json!({"kind":kind,"id":target,"name":declaration.name,"fields":fields,
                    "type_parameters":declaration.type_parameters.iter().map(|param|json!({"name":param.name,"index":param.index})).collect::<Vec<_>>()});
            }
        }
    }
    Value::Null
}

#[derive(Default)]
struct Relations {
    rows: Vec<Value>,
    users: BTreeSet<String>,
    callers: BTreeMap<String, BTreeSet<String>>,
    visits: usize,
    calls: usize,
    pattern_visits: usize,
}
fn reverse_relationships(revision: &ProjectRevision, target: &str) -> Result<Value> {
    let mut selected = BTreeSet::from([target.to_owned()]);
    // Record/class/variant queries include their actual persistent field IDs.
    let typed = typed_declaration(revision, target);
    if let Some(fields) = typed["fields"].as_array() {
        for field in fields {
            if let Some(id) = field["id"].as_str() {
                selected.insert(id.to_owned());
            }
        }
    }
    let mut relations = Relations::default();
    for module in revision.semantic.image_modules() {
        for function in module.functions() {
            scan_function(
                &mut relations,
                module.path(),
                function.id.as_str(),
                &function.requires,
                &function.body,
                &function.ensures,
                &selected,
            )?;
        }
        for function in module.function_templates() {
            scan_function(
                &mut relations,
                module.path(),
                function.id.as_str(),
                &function.requires,
                &function.body,
                &function.ensures,
                &selected,
            )?;
        }
    }
    let mut closure = relations.users.clone();
    if revision.semantic.image_modules().iter().any(|module| {
        module
            .functions()
            .iter()
            .any(|function| function.id.as_str() == target)
            || module
                .function_templates()
                .iter()
                .any(|function| function.id.as_str() == target)
    }) {
        closure.insert(target.to_owned());
    }
    let mut pending = closure.iter().cloned().collect::<Vec<_>>();
    while let Some(id) = pending.pop() {
        if let Some(callers) = relations.callers.get(&id) {
            for caller in callers {
                if closure.insert(caller.clone()) {
                    pending.push(caller.clone());
                }
            }
        }
        if closure.len() > MAX_ITEMS {
            return Err(capacity("semantic delta caller closure exceeds its bound"));
        }
    }
    let test_root = revision.test_program().entrypoint.as_str();
    Ok(
        json!({"direct_field_sites":relations.rows,"direct_field_user_functions":relations.users,
        "reverse_callable_closure":closure,"declared_test_root":test_root,"test_reachable":closure.contains(test_root),
        "basis":"retained_HIR_field_ID_accesses_and_local_or_imported_direct_calls","coverage":"not_inferred","executed":false,
        "limitations":["no_external_or_dynamic_callers","aggregate_whole_value_reads_not_expanded_to_every_leaf","no_runtime_liveness_or_path_feasibility"]}),
    )
}
fn scan_function(
    relations: &mut Relations,
    path: &str,
    function: &str,
    requires: &[ResolvedExpr],
    body: &ResolvedExpr,
    ensures: &[ResolvedExpr],
    selected: &BTreeSet<String>,
) -> Result<()> {
    for (phase, roots) in [
        ("requires", requires),
        ("body", std::slice::from_ref(body)),
        ("ensures", ensures),
    ] {
        let mut pending = roots.iter().rev().map(|node| (node, 0)).collect::<Vec<_>>();
        while let Some((node, depth)) = pending.pop() {
            relations.visits += 1;
            if relations.visits > MAX_VISITS || depth > MAX_DEPTH {
                return Err(capacity(
                    "semantic delta HIR relationship traversal exceeds its bound",
                ));
            }
            if let Expr::Call { callee, .. } = &node.kind {
                relations.calls += 1;
                if relations.calls > MAX_ITEMS {
                    return Err(capacity("semantic delta call-site index exceeds its bound"));
                }
                relations
                    .callers
                    .entry(callee.as_str().to_owned())
                    .or_default()
                    .insert(function.to_owned());
            }
            let rows = &mut relations.rows;
            let users = &mut relations.users;
            let mut access = |id: &str, kind: &str| -> Result<()> {
                if selected.contains(id) {
                    if rows.len() >= MAX_ITEMS {
                        return Err(capacity("semantic delta field sites exceed their bound"));
                    }
                    users.insert(function.to_owned());
                    rows.push(json!({"field_or_type_id":id,"function_id":function,"path":path,"phase":phase,"expression_id":node.id.as_str(),"access":kind}));
                }
                Ok(())
            };
            match &node.kind {
                Expr::Place(place) | Expr::BorrowPlace { place, .. } => {
                    for projection in &place.projections {
                        match projection {
                            PlaceProjection::Field(field)
                            | PlaceProjection::VariantField { field, .. } => access(
                                field.as_str(),
                                if matches!(node.kind, Expr::BorrowPlace { .. }) {
                                    "borrow"
                                } else {
                                    "read_or_move"
                                },
                            )?,
                        }
                    }
                }
                Expr::Project { field, .. } => access(field.as_str(), "projection_read")?,
                Expr::ConstructRecord { record, fields } => {
                    access(record.as_str(), "construct_record")?;
                    for field in fields {
                        access(field.field.as_str(), "initialize")?;
                    }
                }
                Expr::ConstructVariant {
                    variant,
                    case,
                    fields,
                } => {
                    access(variant.as_str(), "construct_variant")?;
                    access(case.as_str(), "construct_case")?;
                    for field in fields {
                        access(field.field.as_str(), "initialize")?;
                    }
                }
                Expr::UpdateRecord { fields, .. } => {
                    for field in fields {
                        access(field.field.as_str(), "update_result_field")?;
                    }
                }
                Expr::Block { statements, .. } => {
                    for statement in statements {
                        if let ResolvedStatement::Assign {
                            field: Some(field), ..
                        } = statement
                        {
                            access(field.as_str(), "in_place_write")?;
                        }
                    }
                }
                Expr::Match { arms, .. } => {
                    for arm in arms {
                        pattern_accesses(
                            &arm.pattern,
                            &mut access,
                            0,
                            &mut relations.pattern_visits,
                        )?;
                    }
                }
                _ => {}
            }
            let mut children = Vec::new();
            hir::push_resolved_expression_children_in_authored_order(node, &mut children);
            if children.len() > MAX_VISITS.saturating_sub(relations.visits + pending.len()) {
                return Err(capacity(
                    "semantic delta pending HIR inventory exceeds its bound",
                ));
            }
            pending.extend(children.into_iter().map(|child| (child, depth + 1)));
        }
    }
    Ok(())
}
fn pattern_accesses(
    pattern: &ResolvedMatchPattern,
    access: &mut impl FnMut(&str, &str) -> Result<()>,
    depth: usize,
    visits: &mut usize,
) -> Result<()> {
    *visits += 1;
    if *visits > MAX_ITEMS {
        return Err(capacity(
            "semantic delta pattern item inventory exceeds its bound",
        ));
    }
    if depth > MAX_DEPTH {
        return Err(capacity("semantic delta pattern depth exceeds its bound"));
    }
    match pattern {
        ResolvedMatchPattern::Record { record, fields, .. } => {
            access(record.as_str(), "record_pattern")?;
            record_pattern_accesses(fields, access, depth + 1, visits)?;
        }
        ResolvedMatchPattern::Variant {
            variant,
            case,
            fields,
        } => {
            access(variant.as_str(), "variant_pattern")?;
            access(case.as_str(), "case_pattern")?;
            for field in fields {
                *visits += 1;
                if *visits > MAX_ITEMS {
                    return Err(capacity(
                        "semantic delta pattern field inventory exceeds its bound",
                    ));
                }
                access(field.field.as_str(), "pattern_bind")?;
            }
        }
        ResolvedMatchPattern::Or(alternatives) => {
            for alternative in alternatives {
                pattern_accesses(alternative, access, depth + 1, visits)?;
            }
        }
        ResolvedMatchPattern::Wildcard
        | ResolvedMatchPattern::Literal(_)
        | ResolvedMatchPattern::Binding(_) => {}
    }
    Ok(())
}
fn record_pattern_accesses(
    fields: &[ResolvedRecordMatchPatternField],
    access: &mut impl FnMut(&str, &str) -> Result<()>,
    depth: usize,
    visits: &mut usize,
) -> Result<()> {
    *visits += 1;
    if *visits > MAX_ITEMS {
        return Err(capacity(
            "semantic delta pattern item inventory exceeds its bound",
        ));
    }
    if depth > MAX_DEPTH {
        return Err(capacity(
            "semantic delta record pattern depth exceeds its bound",
        ));
    }
    for field in fields {
        *visits += 1;
        if *visits > MAX_ITEMS {
            return Err(capacity(
                "semantic delta pattern field inventory exceeds its bound",
            ));
        }
        match &field.pattern {
            ResolvedRecordMatchFieldPattern::Binding(_) => {
                access(field.field.as_str(), "pattern_bind")?
            }
            ResolvedRecordMatchFieldPattern::Wildcard => {
                access(field.field.as_str(), "pattern_ignore")?
            }
            ResolvedRecordMatchFieldPattern::Record { record, fields, .. } => {
                access(field.field.as_str(), "nested_pattern")?;
                access(record.as_str(), "record_pattern")?;
                record_pattern_accesses(fields, access, depth + 1, visits)?;
            }
        }
    }
    Ok(())
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
