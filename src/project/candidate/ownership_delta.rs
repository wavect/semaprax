//! Exact descriptive ownership proof differences, never runtime observations.

use std::collections::{BTreeMap, BTreeSet};
use std::io::{self, Write};
use std::sync::Arc;

use serde_json::{json, Value};

use super::{parse_revision, wire, ProjectCandidate};
use crate::ast::TypeDeclarationKind;
use crate::cleanup::{CleanupInventory, CleanupStorageOrigin, FieldLivenessShape};
use crate::cleanup_plan::{CleanupTransition, StagedCopyResultSource};
use crate::diagnostic::Diagnostic;
use crate::hir::{
    DeclarationIndex, OwnershipMode, ResolvedFieldDeclaration, ResolvedFunction, ResolvedParam,
    ResolvedType, ResolvedTypeDeclaration, ResolvedTypeDeclarationKind, TypeFacts, ValueId,
};
use crate::project::ProjectRevision;

type Result<T> = std::result::Result<T, Vec<Diagnostic>>;
pub const PROJECT_CANDIDATE_OWNERSHIP_DELTA_SCHEMA: &str =
    "semaprax.project-candidate-ownership-delta.v1";
pub const PROJECT_CANDIDATE_OWNERSHIP_DELTA_VERIFICATION_SCHEMA: &str =
    "semaprax.project-candidate-ownership-delta-verification.v1";
pub const MAX_PROJECT_CANDIDATE_OWNERSHIP_DELTA_BYTES: usize = 8 * 1024 * 1024;
const MAX_FACT_BYTES: usize = 32 * 1024 * 1024;
const MAX_ITEMS: usize = 65_536;
const MAX_VISITS: usize = 1_048_576;
const MAX_DEPTH: usize = 256;
const FACT_DOMAIN: &[u8] = b"semaprax.candidate-ownership-delta.fact.v1\0";
const SOURCE_DOMAIN: &[u8] = b"semaprax.candidate-ownership-delta.source.v1\0";
const REPORT_DOMAIN: &[u8] = b"semaprax.candidate-ownership-delta.report.v1\0";

#[derive(Default)]
struct Budget {
    bytes: usize,
    items: usize,
    visits: usize,
}
impl Budget {
    fn visit(&mut self, depth: usize) -> Result<()> {
        if depth > MAX_DEPTH || self.visits >= MAX_VISITS {
            return Err(capacity());
        }
        self.visits += 1;
        Ok(())
    }
    fn items(&mut self, count: usize) -> Result<()> {
        self.items = self.items.checked_add(count).ok_or_else(capacity)?;
        if self.items > MAX_ITEMS {
            return Err(capacity());
        }
        Ok(())
    }
    fn bytes(&mut self, count: usize) -> Result<()> {
        self.bytes = self.bytes.checked_add(count).ok_or_else(capacity)?;
        if self.bytes > MAX_FACT_BYTES {
            return Err(capacity());
        }
        Ok(())
    }
    fn fact(&mut self, value: &Value) -> Result<()> {
        self.serialized_fact(|writer| serde_json::to_writer(writer, value))
    }
    fn string_fact(&mut self, value: &str) -> Result<()> {
        self.serialized_fact(|writer| serde_json::to_writer(writer, value))
    }
    fn serialized_fact(
        &mut self,
        serialize: impl FnOnce(&mut dyn Write) -> serde_json::Result<()>,
    ) -> Result<()> {
        struct Counter(usize);
        impl Write for Counter {
            fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
                if bytes.len() > MAX_PROJECT_CANDIDATE_OWNERSHIP_DELTA_BYTES.saturating_sub(self.0)
                {
                    return Err(io::Error::other("ownership fact size limit"));
                }
                self.0 += bytes.len();
                Ok(bytes.len())
            }
            fn flush(&mut self) -> io::Result<()> {
                Ok(())
            }
        }
        let mut count = Counter(1);
        serialize(&mut count).map_err(|_| capacity())?;
        self.bytes(count.0)
    }
    fn copy(&mut self, value: &Value) -> Result<Value> {
        self.fact(value)?;
        Ok(value.clone())
    }
}

struct Inventory {
    functions: BTreeMap<String, Value>,
    types: BTreeMap<String, Value>,
    instances: usize,
}

impl ProjectCandidate {
    pub fn ownership_delta(&self, expected_candidate: &str) -> Result<String> {
        self.require_candidate(expected_candidate)?;
        let mut budget = Budget::default();
        let before = inventory(&self.base, &mut budget)?;
        let after = inventory(&self.revision, &mut budget)?;
        let ids = before
            .functions
            .keys()
            .chain(after.functions.keys())
            .collect::<BTreeSet<_>>();
        let mut functions = Vec::new();
        let mut unchanged = 0usize;
        for id in ids {
            let base = before.functions.get(id);
            let candidate = after.functions.get(id);
            if base == candidate {
                unchanged += 1;
                continue;
            }
            let (change, comparison) = comparison(base, candidate, &mut budget)?;
            let row = json!({"id":id,"change":change,"comparison":comparison,
                "base":budget.copy(base.unwrap_or(&Value::Null))?,
                "candidate":budget.copy(candidate.unwrap_or(&Value::Null))?});
            budget.fact(&row)?;
            functions.push(row);
        }
        let type_ids = before
            .types
            .keys()
            .chain(after.types.keys())
            .collect::<BTreeSet<_>>();
        let mut types = Vec::new();
        let mut unchanged_types = 0usize;
        for id in type_ids {
            let base = before.types.get(id);
            let candidate = after.types.get(id);
            if base == candidate {
                unchanged_types += 1;
                continue;
            }
            let (change, comparison) = type_comparison(base, candidate, &mut budget)?;
            let row = json!({"id":id,"change":change,"comparison":comparison,
                "base":budget.copy(base.unwrap_or(&Value::Null))?,
                "candidate":budget.copy(candidate.unwrap_or(&Value::Null))?});
            budget.fact(&row)?;
            types.push(row);
        }
        render(json!({"schema":PROJECT_CANDIDATE_OWNERSHIP_DELTA_SCHEMA,
            "candidate_digest":expected_candidate,
            "base_project_revision":self.base.project_revision(),"project_revision":self.revision.project_revision(),
            "base_workspace_revision":self.base.workspace_revision(),"workspace_revision":self.revision.workspace_revision(),
            "source_bindings":{"base":sources(&self.base),"candidate":sources(&self.revision)},
            "inventory":{"base_functions":before.functions.len(),"candidate_functions":after.functions.len(),
                "base_instances":before.instances,"candidate_instances":after.instances,
                "unchanged_functions":unchanged,"affected_functions":functions.len(),
                "base_types":before.types.len(),"candidate_types":after.types.len(),
                "unchanged_types":unchanged_types,"affected_types":types.len()},
            "functions":functions,"types":types,
            "selection_basis":"all_source_callable_checked_signatures_and_plans_plus_explicit_source_nominal_checked_shapes_and_type_facts",
            "evidence_class":"descriptive_checked_HIR_ownership_and_cleanup_delta",
            "plan_order":"exact_compiler_vector_order",
            "local_identity_scope":"expression_value_loan_storage_block_edge_and_instance_ids_are_revision_scoped",
            "execution":false,"source_authority":false,
            "limits":{"max_report_bytes":MAX_PROJECT_CANDIDATE_OWNERSHIP_DELTA_BYTES,"max_fact_work_bytes":MAX_FACT_BYTES,
                "max_items":MAX_ITEMS,"max_visits":MAX_VISITS,"max_depth":MAX_DEPTH},
            "nonclaims":["not_runtime_liveness_or_destruction_trace","not_behavioral_equivalence",
                "not_ownership_safety_promotion","not_test_coverage_or_execution","not_new_backend_admission",
                "not_inferred_return_ownership","no_plan_sorting_repair_or_normalization",
                "type_layout_keys_are_compiler_facts_not_ABI_compatibility","no_external_callable_plans",
                "no_source_or_publication_authority","not_allocator_or_RSS_accounting"]}))
    }

    pub fn verify_ownership_delta(&self, expected_candidate: &str, bytes: &[u8]) -> Result<String> {
        self.require_candidate(expected_candidate)?;
        if bytes.len() > MAX_PROJECT_CANDIDATE_OWNERSHIP_DELTA_BYTES {
            return Err(capacity());
        }
        let replay = Self::replay(
            Arc::clone(&self.base),
            self.base.project_revision(),
            &self.changes,
            self.to_json().as_bytes(),
        )?;
        if replay.ownership_delta(expected_candidate)?.as_bytes() != bytes {
            return Err(vec![Diagnostic::io(
                "SPX-G330",
                "ownership delta failed exact independent candidate replay",
            )]);
        }
        render(
            json!({"schema":PROJECT_CANDIDATE_OWNERSHIP_DELTA_VERIFICATION_SCHEMA,
            "result":"exact_recomputation","candidate_digest":expected_candidate,
            "base_project_revision":self.base.project_revision(),"project_revision":self.revision.project_revision(),
            "delta_digest":wire::digest(REPORT_DOMAIN,bytes),"execution":false,"source_authority":false}),
        )
    }
}

fn inventory(revision: &ProjectRevision, budget: &mut Budget) -> Result<Inventory> {
    let mut result = Inventory {
        functions: BTreeMap::new(),
        types: BTreeMap::new(),
        instances: 0,
    };
    for program in parse_revision(revision)? {
        let source = revision
            .sources()
            .iter()
            .find(|s| s.path() == program.path)
            .ok_or_else(invalid)?;
        for declaration in &program.types {
            if !declaration.explicit_id {
                continue;
            }
            budget.items(1)?;
            let fragment = source
                .source()
                .get(declaration.span.start..declaration.span.end)
                .ok_or_else(invalid)?;
            budget.string_fact(declaration.stable_id.as_str())?;
            budget.string_fact(declaration.name.as_str())?;
            let row = json!({"id":declaration.stable_id,"name":declaration.name,
                "provenance":{"path":source.path(),"module":program.module,"source_revision":source.source_revision(),
                    "source_digest":source.source_digest(),"span":{"start":declaration.span.start,"end":declaration.span.end}},
                "source_declaration_digest":wire::digest(SOURCE_DOMAIN,fragment.as_bytes()),
                "hir_availability":"source_only","declaration_kind":source_type_kind(&declaration.kind),
                "type_parameters":null,"members":null,"type_facts":null,
                "type_facts_availability":"source_only"});
            budget.fact(&row)?;
            if result
                .types
                .insert(declaration.stable_id.clone(), row)
                .is_some()
            {
                return Err(invalid());
            }
        }
        let mut functions = program.functions.iter().collect::<Vec<_>>();
        for ty in &program.types {
            if let TypeDeclarationKind::Class { methods, .. } = &ty.kind {
                functions.extend(methods);
            }
        }
        for function in functions {
            budget.items(1)?;
            let fragment = source
                .source()
                .get(function.span.start..function.span.end)
                .ok_or_else(invalid)?;
            budget.string_fact(function.stable_id.as_str())?;
            budget.string_fact(function.name.as_str())?;
            let row = json!({"id":function.stable_id,"name":function.name,
                "provenance":{"path":source.path(),"module":program.module,"source_revision":source.source_revision(),
                    "source_digest":source.source_digest(),"span":{"start":function.span.start,"end":function.span.end}},
                "source_declaration_digest":wire::digest(SOURCE_DOMAIN,fragment.as_bytes()),
                "hir_availability":"source_only","signature":null,"cleanup_inventory":null,
                "loan_plan":null,"cleanup_plan":null,"instances":[]});
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
    let declarations = revision
        .semantic
        .image_modules()
        .iter()
        .flat_map(|module| module.types().iter())
        .map(|declaration| (declaration.id.as_str(), declaration))
        .collect::<BTreeMap<_, _>>();
    for module in revision.semantic.image_modules() {
        for declaration in module.types() {
            let Some(row) = result.types.get_mut(declaration.id.as_str()) else {
                continue;
            };
            attach_source(row, module.path(), "retained_checked_type")?;
            attach_type(row, declaration, &declarations, budget)?;
        }
        for function in module.functions() {
            let row = result
                .functions
                .get_mut(function.id.as_str())
                .ok_or_else(invalid)?;
            attach_source(row, module.path(), "retained_checked_function")?;
            attach_function(row, function, budget)?;
        }
        for function in module.function_templates() {
            let row = result
                .functions
                .get_mut(function.id.as_str())
                .ok_or_else(invalid)?;
            attach_source(row, module.path(), "retained_checked_template")?;
            row["signature"] = signature(
                &function.params,
                &function.result_id,
                &function.return_type,
                budget,
            )?;
            budget.fact(row)?;
        }
        // Instance order is an inventory order, not a plan vector. Key by the
        // actual compiler instance identity; never call it a source declaration.
        let mut instances = BTreeMap::new();
        budget.items(module.function_instances().len())?;
        for instance in module.function_instances() {
            let mut args = Vec::new();
            for ty in &instance.type_arguments {
                args.push(type_key(ty, budget)?);
            }
            let mut row = json!({"id":instance.id.as_str(),"template":instance.template.as_str(),
                "type_arguments":args,"signature":null,"cleanup_inventory":null,"loan_plan":null,"cleanup_plan":null});
            attach_function(&mut row, &instance.function, budget)?;
            budget.fact(&row)?;
            if instances
                .insert(instance.id.as_str(), (instance.template.as_str(), row))
                .is_some()
            {
                return Err(invalid());
            }
        }
        for (_, (template, instance)) in instances {
            let row = result.functions.get_mut(template).ok_or_else(invalid)?;
            if row["provenance"]["path"].as_str() != Some(module.path())
                || row["hir_availability"] != "retained_checked_template"
            {
                return Err(invalid());
            }
            row["instances"]
                .as_array_mut()
                .ok_or_else(invalid)?
                .push(instance);
            result.instances += 1;
        }
    }
    for row in result.functions.values() {
        budget.fact(row)?;
    }
    for row in result.types.values() {
        budget.fact(row)?;
    }
    Ok(result)
}

fn attach_source(row: &mut Value, path: &str, availability: &str) -> Result<()> {
    if row["provenance"]["path"].as_str() != Some(path) || row["hir_availability"] != "source_only"
    {
        return Err(invalid());
    }
    row["hir_availability"] = json!(availability);
    Ok(())
}

fn source_type_kind(kind: &TypeDeclarationKind) -> &'static str {
    match kind {
        TypeDeclarationKind::Resource { .. } => "resource",
        TypeDeclarationKind::Record { .. } => "record",
        TypeDeclarationKind::Class { .. } => "class",
        TypeDeclarationKind::Variant { .. } => "variant",
    }
}

fn resolved_type_kind(kind: &ResolvedTypeDeclarationKind) -> &'static str {
    match kind {
        ResolvedTypeDeclarationKind::Resource { .. } => "resource",
        ResolvedTypeDeclarationKind::Record { .. } => "record",
        ResolvedTypeDeclarationKind::Class { .. } => "class",
        ResolvedTypeDeclarationKind::Variant { .. } => "variant",
    }
}

fn attach_type(
    row: &mut Value,
    declaration: &ResolvedTypeDeclaration,
    declarations: &BTreeMap<&str, &ResolvedTypeDeclaration>,
    budget: &mut Budget,
) -> Result<()> {
    let kind = resolved_type_kind(&declaration.kind);
    if row["declaration_kind"] != kind || row["name"] != declaration.name {
        return Err(invalid());
    }
    budget.items(declaration.type_parameters.len())?;
    row["type_parameters"] = json!(declaration
        .type_parameters
        .iter()
        .map(|parameter| json!({"name":parameter.name,"index":parameter.index}))
        .collect::<Vec<_>>());
    row["members"] = match &declaration.kind {
        ResolvedTypeDeclarationKind::Resource { .. } => json!([]),
        ResolvedTypeDeclarationKind::Record { fields } => {
            json!({"fields":resolved_fields(fields, budget)?})
        }
        ResolvedTypeDeclarationKind::Class { fields, methods } => {
            budget.items(methods.len())?;
            json!({"fields":resolved_fields(fields,budget)?,
                "methods":methods.iter().map(|id|id.as_str()).collect::<Vec<_>>()})
        }
        ResolvedTypeDeclarationKind::Variant { cases } => {
            budget.items(cases.len())?;
            let mut rows = Vec::new();
            for case in cases {
                budget.string_fact(case.id.as_str())?;
                budget.string_fact(case.name.as_str())?;
                rows.push(
                    json!({"id":case.id.as_str(),"name":case.name,"index":case.index,
                    "fields":resolved_fields(&case.fields,budget)?}),
                );
            }
            json!({"cases":rows})
        }
    };
    row["type_facts"] = if !declaration.type_parameters.is_empty() {
        row["type_facts_availability"] = json!("generic_uninstantiated");
        Value::Null
    } else {
        match DeclarationIndex::record_evolution_type_facts(&declaration.id, declarations)
            .map_err(|_| capacity())?
        {
            Some(facts) => {
                row["type_facts_availability"] = json!("retained_checked");
                type_facts(&facts, budget)?
            }
            None => {
                row["type_facts_availability"] = json!("unsupported_or_incomplete_type_closure");
                Value::Null
            }
        }
    };
    budget.fact(row)
}

fn resolved_fields(fields: &[ResolvedFieldDeclaration], budget: &mut Budget) -> Result<Vec<Value>> {
    budget.items(fields.len())?;
    let mut rows = Vec::new();
    for field in fields {
        budget.string_fact(field.id.as_str())?;
        budget.string_fact(field.name.as_str())?;
        rows.push(
            json!({"id":field.id.as_str(),"name":field.name,"index":field.index,
            "type_id":type_key(&field.ty,budget)?}),
        );
    }
    Ok(rows)
}

fn type_facts(facts: &TypeFacts, budget: &mut Budget) -> Result<Value> {
    budget.string_fact(&facts.layout_key)?;
    Ok(json!({"copy":facts.copy,"needs_drop":facts.needs_drop,
        "contains_resource":facts.contains_resource,"sized":facts.sized,
        "layout_key":facts.layout_key}))
}

fn signature(
    params: &[ResolvedParam],
    result: &ValueId,
    ty: &ResolvedType,
    budget: &mut Budget,
) -> Result<Value> {
    budget.items(params.len())?;
    let mut parameters = Vec::new();
    for (index, param) in params.iter().enumerate() {
        budget.string_fact(param.id.as_str())?;
        budget.string_fact(param.name.as_str())?;
        parameters.push(
            json!({"index":index,"id":param.id.as_str(),"name":param.name,
            "type_id":type_key(&param.ty,budget)?,"ownership":ownership(param.ownership)}),
        );
    }
    let row = json!({"parameters":parameters,"result":{"id":result.as_str(),"type_id":type_key(ty,budget)?}});
    budget.fact(&row)?;
    Ok(row)
}

fn attach_function(
    row: &mut Value,
    function: &ResolvedFunction,
    budget: &mut Budget,
) -> Result<()> {
    row["signature"] = signature(
        &function.params,
        &function.result_id,
        &function.return_type,
        budget,
    )?;
    row["cleanup_inventory"] = cleanup_inventory(&function.cleanup, budget)?;
    let plan = &function.cleanup_plan;
    budget.items(
        plan.slots.len()
            + plan.status_sources.len()
            + plan.blocks.len()
            + plan.edges.len()
            + plan.regions.len()
            + plan.exits.len(),
    )?;
    for slot in &plan.slots {
        preflight_type(&slot.ty, budget, 0)?;
        preflight_shape(&slot.field_liveness_shape, budget, 0)?;
    }
    for block in &plan.blocks {
        budget.items(block.transitions.len())?;
        for transition in &block.transitions {
            if let CleanupTransition::StageCopyResult { source } = transition {
                match source {
                    StagedCopyResultSource::Body { instance, .. } => {
                        preflight_type(instance, budget, 0)?
                    }
                    StagedCopyResultSource::TryResidual {
                        source_instance,
                        target_instance,
                        ..
                    }
                    | StagedCopyResultSource::TryOptionNone {
                        source_instance,
                        target_instance,
                        ..
                    } => {
                        preflight_type(source_instance, budget, 0)?;
                        preflight_type(target_instance, budget, 0)?;
                    }
                }
            }
        }
    }
    let loan = &function.loan_plan;
    budget.items(loan.loans.len() + loan.endpoints.len() + loan.edges.len())?;
    row["loan_plan"] = plan_value(|| crate::graph_loan::loan_plan_json(loan), budget)?;
    row["cleanup_plan"] = plan_value(|| crate::graph_cleanup::cleanup_plan_json(plan), budget)?;
    Ok(())
}

fn type_key(ty: &ResolvedType, budget: &mut Budget) -> Result<String> {
    preflight_type(ty, budget, 0)?;
    let key = ty.identity_key();
    budget.string_fact(&key)?;
    Ok(key)
}
fn preflight_type(ty: &ResolvedType, budget: &mut Budget, depth: usize) -> Result<()> {
    budget.visit(depth)?;
    if let ResolvedType::Nominal {
        declaration,
        arguments,
    } = ty
    {
        budget.string_fact(declaration.as_str())?;
        for arg in arguments {
            preflight_type(arg, budget, depth + 1)?;
        }
    }
    Ok(())
}

fn preflight_shape(shape: &FieldLivenessShape, budget: &mut Budget, depth: usize) -> Result<()> {
    budget.visit(depth)?;
    match shape {
        FieldLivenessShape::Record { fields, .. } => {
            for field in fields {
                preflight_shape(&field.shape, budget, depth + 1)?;
            }
        }
        FieldLivenessShape::Variant { cases, .. } => {
            for case in cases {
                budget.visit(depth + 1)?;
                for field in &case.fields {
                    preflight_shape(&field.shape, budget, depth + 2)?;
                }
            }
        }
        FieldLivenessShape::NoDrop | FieldLivenessShape::Leaf { .. } => {}
    }
    Ok(())
}

fn cleanup_inventory(inventory: &CleanupInventory, budget: &mut Budget) -> Result<Value> {
    budget.items(inventory.slots.len() + inventory.flags.len())?;
    let mut slots = Vec::new();
    for slot in &inventory.slots {
        let origin = match &slot.origin {
            CleanupStorageOrigin::Parameter {
                value,
                parameter_index,
            } => {
                json!({"kind":"parameter","value":value.as_str(),"parameter_index":parameter_index})
            }
            CleanupStorageOrigin::Binding { value } => {
                json!({"kind":"binding","value":value.as_str()})
            }
            CleanupStorageOrigin::Temporary { expression } => {
                json!({"kind":"temporary","expression":expression.as_str()})
            }
            CleanupStorageOrigin::ProvisionalResult { value } => {
                json!({"kind":"provisional_result","value":value.as_str()})
            }
        };
        let row = json!({"id":slot.id.0,"discovery_index":slot.discovery_index,"origin":origin,
            "type_id":type_key(&slot.ty,budget)?,"shape":shape_value(&slot.shape,budget,0)?});
        budget.fact(&row)?;
        slots.push(row);
    }
    let mut flags = Vec::new();
    for flag in &inventory.flags {
        budget.items(flag.place.projections.len())?;
        let row = json!({"id":flag.id.0,"place":{"storage":flag.place.storage.0,
            "projections":flag.place.projections.iter().map(|id|id.as_str()).collect::<Vec<_>>()},"lifecycle":flag.lifecycle.as_str()});
        budget.fact(&row)?;
        flags.push(row);
    }
    budget.items(
        inventory.entry_state.live_owned_parameters.len()
            + inventory.entry_state.conditional_owned_parameters.len(),
    )?;
    let mut conditional = Vec::new();
    for entry in &inventory.entry_state.conditional_owned_parameters {
        budget.items(entry.cases.len())?;
        let mut cases = Vec::new();
        for case in &entry.cases {
            budget.items(case.live_flags.len())?;
            cases.push(json!({"case":case.case.as_str(),"live_flags":case.live_flags.iter().map(|id|id.0).collect::<Vec<_>>()}));
        }
        conditional.push(
            json!({"storage":entry.storage.0,"variant":entry.variant.as_str(),"cases":cases}),
        );
    }
    let row = json!({"schema":inventory.schema,"slots":slots,"flags":flags,
        "entry_state":{"live_owned_parameters":inventory.entry_state.live_owned_parameters.iter().map(|id|id.0).collect::<Vec<_>>(),
            "conditional_owned_parameters":conditional},"order_meaning":"structural_discovery_not_runtime_destruction"});
    budget.fact(&row)?;
    Ok(row)
}

fn shape_value(shape: &FieldLivenessShape, budget: &mut Budget, depth: usize) -> Result<Value> {
    budget.visit(depth)?;
    let row = match shape {
        FieldLivenessShape::NoDrop => json!({"kind":"no_drop"}),
        FieldLivenessShape::Leaf { flag, lifecycle } => {
            json!({"kind":"leaf","flag":flag.0,"lifecycle":lifecycle.as_str()})
        }
        FieldLivenessShape::Record {
            declaration,
            fields,
        } => {
            let mut rows = Vec::new();
            budget.items(fields.len())?;
            for field in fields {
                rows.push(json!({"field":field.field.as_str(),"field_index":field.field_index,"shape":shape_value(&field.shape,budget,depth+1)?}));
            }
            json!({"kind":"record","declaration":declaration.as_str(),"fields":rows})
        }
        FieldLivenessShape::Variant { declaration, cases } => {
            budget.items(cases.len())?;
            let mut rows = Vec::new();
            for case in cases {
                budget.visit(depth + 1)?;
                budget.items(case.fields.len())?;
                let mut fields = Vec::new();
                for field in &case.fields {
                    fields.push(json!({"field":field.field.as_str(),"field_index":field.field_index,"shape":shape_value(&field.shape,budget,depth+2)?}));
                }
                rows.push(
                    json!({"case":case.case.as_str(),"case_index":case.case_index,"fields":fields}),
                );
            }
            json!({"kind":"variant","declaration":declaration.as_str(),"cases":rows})
        }
    };
    budget.fact(&row)?;
    Ok(row)
}

fn plan_value(operation: impl FnOnce() -> String, budget: &mut Budget) -> Result<Value> {
    let remaining = MAX_FACT_BYTES
        .saturating_sub(budget.bytes)
        .min(MAX_PROJECT_CANDIDATE_OWNERSHIP_DELTA_BYTES);
    let (text, overflow, work) = crate::bounded_output::with_limit_usage(remaining, operation);
    if overflow {
        return Err(capacity());
    }
    budget.bytes(work)?;
    // Only compiler-created JSON reaches this parser. Bound aggregate syntax
    // work and container depth before allocating its Value tree.
    let mut quoted = false;
    let mut escaped = false;
    let mut depth = 0usize;
    for byte in text.bytes() {
        if quoted {
            if escaped {
                escaped = false;
            } else if byte == b'\\' {
                escaped = true;
            } else if byte == b'"' {
                quoted = false;
            }
            continue;
        }
        match byte {
            b'"' => {
                quoted = true;
                budget.visit(depth)?;
            }
            b'{' | b'[' => {
                depth += 1;
                budget.visit(depth)?;
            }
            b'}' | b']' => {
                depth = depth.checked_sub(1).ok_or_else(invalid)?;
            }
            b',' | b':' => budget.visit(depth)?,
            _ => {}
        }
    }
    if quoted || depth != 0 {
        return Err(invalid());
    }
    let value: Value = serde_json::from_str(&text).map_err(|_| capacity())?;
    budget.fact(&value)?;
    Ok(value)
}

fn comparison(
    base: Option<&Value>,
    candidate: Option<&Value>,
    budget: &mut Budget,
) -> Result<(&'static str, Value)> {
    let mut reasons = Vec::new();
    if base.is_none() {
        reasons.push("added");
    } else if candidate.is_none() {
        reasons.push("removed");
    }
    let mut equal = serde_json::Map::new();
    let mut proofs_equal = true;
    for (field, key, changed, unavailable) in [
        (
            "signature",
            "signature_equal",
            "signature_changed",
            "signature_unavailable",
        ),
        (
            "cleanup_inventory",
            "cleanup_inventory_equal",
            "cleanup_inventory_changed",
            "cleanup_inventory_unavailable",
        ),
        (
            "loan_plan",
            "loan_plan_equal",
            "loan_plan_changed",
            "loan_plan_unavailable",
        ),
        (
            "cleanup_plan",
            "cleanup_plan_equal",
            "cleanup_plan_changed",
            "cleanup_plan_unavailable",
        ),
    ] {
        let left = base.map(|row| &row[field]);
        let right = candidate.map(|row| &row[field]);
        let available = left.is_none_or(|v| !v.is_null()) && right.is_none_or(|v| !v.is_null());
        let same = available.then_some(left == right);
        if same == Some(false) {
            reasons.push(changed);
        } else if same.is_none() {
            reasons.push(unavailable);
        }
        proofs_equal &= same == Some(true);
        equal.insert(key.into(), json!(same));
    }
    let instances_equal =
        base.map(|row| &row["instances"]) == candidate.map(|row| &row["instances"]);
    if !instances_equal {
        reasons.push("instances_changed");
    }
    let source_equal = base.map(|row| &row["source_declaration_digest"])
        == candidate.map(|row| &row["source_declaration_digest"]);
    if !source_equal {
        reasons.push("source_changed");
    }
    let provenance_only = proofs_equal && instances_equal && source_equal;
    if provenance_only {
        reasons.push("provenance_only");
    }
    let change = if base.is_none() {
        "added"
    } else if candidate.is_none() {
        "removed"
    } else if provenance_only {
        "provenance_only"
    } else {
        "modified"
    };
    equal.insert("exact_equal".into(), json!(base == candidate));
    equal.insert("instances_equal".into(), json!(instances_equal));
    equal.insert("source_equal".into(), json!(source_equal));
    equal.insert("reasons".into(), json!(reasons));
    for (key, side) in [("base_digest", base), ("candidate_digest", candidate)] {
        let bytes = render(budget.copy(side.unwrap_or(&Value::Null))?)?;
        equal.insert(
            key.into(),
            json!(wire::digest(FACT_DOMAIN, bytes.as_bytes())),
        );
    }
    Ok((change, Value::Object(equal)))
}

fn type_comparison(
    base: Option<&Value>,
    candidate: Option<&Value>,
    budget: &mut Budget,
) -> Result<(&'static str, Value)> {
    let mut reasons = Vec::new();
    if base.is_none() {
        reasons.push("added");
    } else if candidate.is_none() {
        reasons.push("removed");
    }
    let mut equal = serde_json::Map::new();
    let mut facts_equal = true;
    for (field, key, changed, unavailable) in [
        (
            "declaration_kind",
            "declaration_kind_equal",
            "declaration_kind_changed",
            "declaration_kind_unavailable",
        ),
        (
            "type_parameters",
            "type_parameters_equal",
            "type_parameters_changed",
            "type_parameters_unavailable",
        ),
        (
            "members",
            "members_equal",
            "members_changed",
            "members_unavailable",
        ),
        (
            "type_facts",
            "type_facts_equal",
            "type_facts_changed",
            "type_facts_unavailable",
        ),
        (
            "type_facts_availability",
            "type_facts_availability_equal",
            "type_facts_availability_changed",
            "type_facts_availability_unavailable",
        ),
    ] {
        let left = base.map(|row| &row[field]);
        let right = candidate.map(|row| &row[field]);
        let available =
            left.is_none_or(|value| !value.is_null()) && right.is_none_or(|value| !value.is_null());
        let same = available.then_some(left == right);
        if same == Some(false) {
            reasons.push(changed);
        } else if same.is_none() {
            reasons.push(unavailable);
        }
        facts_equal &= same == Some(true);
        equal.insert(key.into(), json!(same));
    }
    let source_equal = base.map(|row| &row["source_declaration_digest"])
        == candidate.map(|row| &row["source_declaration_digest"]);
    if !source_equal {
        reasons.push("source_changed");
    }
    let provenance_only = facts_equal && source_equal;
    if provenance_only {
        reasons.push("provenance_only");
    }
    let change = if base.is_none() {
        "added"
    } else if candidate.is_none() {
        "removed"
    } else if provenance_only {
        "provenance_only"
    } else {
        "modified"
    };
    equal.insert("exact_equal".into(), json!(base == candidate));
    equal.insert("source_equal".into(), json!(source_equal));
    equal.insert("reasons".into(), json!(reasons));
    for (key, side) in [("base_digest", base), ("candidate_digest", candidate)] {
        let bytes = render(budget.copy(side.unwrap_or(&Value::Null))?)?;
        equal.insert(
            key.into(),
            json!(wire::digest(FACT_DOMAIN, bytes.as_bytes())),
        );
    }
    Ok((change, Value::Object(equal)))
}

fn ownership(mode: OwnershipMode) -> &'static str {
    match mode {
        OwnershipMode::Value => "value",
        OwnershipMode::Own => "own",
        OwnershipMode::Borrow => "borrow",
        OwnershipMode::Shared => "shared",
    }
}
fn sources(revision: &ProjectRevision) -> Vec<Value> {
    revision.sources().iter().map(|s|json!({"path":s.path(),"source_revision":s.source_revision(),"source_digest":s.source_digest()})).collect()
}
fn render(value: Value) -> Result<String> {
    wire::render(value, MAX_PROJECT_CANDIDATE_OWNERSHIP_DELTA_BYTES).map_err(|_| capacity())
}
fn invalid() -> Vec<Diagnostic> {
    vec![Diagnostic::io(
        "SPX-G328",
        "source-backed ownership delta inventory is inconsistent",
    )]
}
fn capacity() -> Vec<Diagnostic> {
    vec![Diagnostic::io(
        "SPX-G329",
        "source-backed ownership delta exceeds its bounded inventory or output",
    )]
}
