//! Reverse structural dependencies on the compiler's retained cleanup/loan
//! proof carriers. Never a liveness simulation or physical-finalizer authority.

use super::*;
use crate::cleanup::{CleanupStorageOrigin, FieldLivenessShape};
use crate::cleanup_plan::{self as cp, CleanupPlace, CleanupTransition as Transition, StorageId};

pub const IMAGE_CLEANUP_DEPENDENCIES_SCHEMA: &str = "semaprax.image-cleanup-dependencies.v1";
pub const IMAGE_CLEANUP_DEPENDENCIES_VERIFICATION_SCHEMA: &str =
    "semaprax.image-cleanup-dependencies-verification.v1";
pub const MAX_IMAGE_CLEANUP_DEPENDENCIES_BYTES: usize = 8 * 1024 * 1024;
const MAX_BYTES: usize = 32 * 1024 * 1024;
type Ids = BTreeSet<String>;
type ShapeNodes = Vec<(Vec<String>, String)>;
type Slots = BTreeMap<StorageId, ShapeNodes>;

#[derive(Default)]
pub(super) struct CleanupDependencyIndex {
    rows: Vec<Value>,
    by_id: BTreeMap<String, Vec<usize>>,
    unavailable: Vec<Value>,
    items: usize,
    visits: usize,
    bytes: usize,
    functions: usize,
    instances: usize,
}

impl CleanupDependencyIndex {
    fn visit(&mut self, depth: usize) -> Result<()> {
        self.visits += 1;
        if depth > MAX_DEPTH || self.visits > MAX_VISITS {
            return Err(limit());
        }
        Ok(())
    }
    fn items(&mut self, count: usize) -> Result<()> {
        self.items = self.items.checked_add(count).ok_or_else(limit)?;
        if self.items > MAX_ITEMS {
            return Err(limit());
        }
        Ok(())
    }
    fn charge(&mut self, count: usize) -> Result<()> {
        self.bytes = self.bytes.checked_add(count).ok_or_else(limit)?;
        if self.bytes > MAX_BYTES {
            return Err(limit());
        }
        Ok(())
    }
    fn value(&mut self, value: &Value) -> Result<()> {
        self.charge(super::value_bytes(value).map_err(|_| limit())?)
    }
    fn add(
        &mut self,
        origin: &Value,
        facet: &str,
        coordinate: Value,
        fact: Value,
        ids: Ids,
        reason: &str,
    ) -> Result<()> {
        self.items(1)?;
        if ids.is_empty() {
            return Ok(());
        }
        let mut row = origin.clone();
        row["facet"] = json!(facet);
        row["coordinate"] = coordinate;
        row["fact"] = fact;
        row["matched_declaration_ids"] = json!(ids);
        row["reason"] = json!(reason);
        self.value(&row)?;
        for id in &ids {
            self.charge(id.len() + 128 + std::mem::size_of::<usize>())?;
            self.by_id
                .entry(id.clone())
                .or_default()
                .push(self.rows.len());
        }
        self.rows.push(row);
        Ok(())
    }
    fn build(revision: &ProjectRevision) -> Result<Self> {
        let mut index = Self::default();
        for module in revision.semantic.image_modules() {
            for function in module.functions() {
                let origin = origin(revision, module, function.id.as_str(), None)?;
                index.function(function, &origin)?;
                index.functions += 1;
            }
            for template in module.function_templates() {
                index.items(1)?;
                let mut row = origin(revision, module, template.id.as_str(), None)?;
                row["availability"] = json!("template_has_no_executable_cleanup_or_loan_plan");
                index.value(&row)?;
                index.unavailable.push(row);
            }
            for instance in module.function_instances() {
                let mut origin = origin(
                    revision,
                    module,
                    instance.template.as_str(),
                    Some(instance.id.as_str()),
                )?;
                let mut arguments = Vec::new();
                for ty in &instance.type_arguments {
                    type_ids(ty, &mut Ids::new(), &mut index, 0)?;
                    arguments.push(ty.identity_key());
                }
                origin["type_arguments"] = json!(arguments);
                index.function(&instance.function, &origin)?;
                index.instances += 1;
            }
        }
        Ok(index)
    }
    fn function(&mut self, function: &hir::ResolvedFunction, origin: &Value) -> Result<()> {
        self.items(1)?;
        let mut provenance = origin.clone();
        provenance["cleanup_inventory_schema"] = json!(function.cleanup.schema);
        provenance["cleanup_plan_schema"] = json!(function.cleanup_plan.schema);
        provenance["loan_plan_schema"] = json!(function.loan_plan.schema);
        let origin = &provenance;
        let plan = &function.cleanup_plan;
        let mut slots = Slots::new();
        for slot in &plan.slots {
            self.items(1)?;
            let nodes = self.nodes(&slot.ty, &slot.field_liveness_shape)?;
            if slots.insert(slot.storage.clone(), nodes).is_some() {
                return Err(invalid());
            }
        }
        // Bound all dynamic vectors before either canonical plan renderer runs.
        self.items(
            plan.blocks.len()
                + plan.edges.len()
                + plan.regions.len()
                + plan.exits.len()
                + plan.status_sources.len(),
        )?;
        for block in &plan.blocks {
            self.items(block.transitions.len())?;
            for transition in &block.transitions {
                if let Transition::StageCopyResult { source } = transition {
                    let mut ids = Ids::new();
                    stage_ids(source, &mut ids, self)?;
                }
            }
        }
        for region in &plan.regions {
            self.items(region.slots.len())?;
        }
        self.items(
            plan.entry_state.live_owned_parameters.len()
                + plan.entry_state.conditional_owned_parameters.len(),
        )?;
        for entry in &plan.entry_state.conditional_owned_parameters {
            self.items(entry.cases.len())?;
            for case in &entry.cases {
                self.items(case.live_places.len())?;
            }
        }
        for exit in &plan.exits {
            self.items(exit.finalize_in_order.len() + exit.leaves_regions.len())?;
        }
        let loan = &function.loan_plan;
        self.items(loan.loans.len() + loan.edges.len() + loan.endpoints.len())?;
        for item in &loan.loans {
            self.items(item.origin.projections.len() + item.ends.len() + item.end_edges.len())?;
        }
        for item in &loan.edges {
            self.items(item.live.len())?;
        }
        for item in &loan.endpoints {
            self.items(
                item.live_before.len()
                    + item.starts.len()
                    + item.kills.len()
                    + item.live_after.len(),
            )?;
        }
        let cleanup_json = self.plan_value(|| crate::graph_cleanup::cleanup_plan_json(plan))?;
        let loan_json = self.plan_value(|| crate::graph_loan::loan_plan_json(loan))?;
        self.inventory(function, origin)?;
        for (slot, actual) in plan.slots.iter().enumerate() {
            let ids = place_ids(
                &slots,
                &CleanupPlace {
                    storage: actual.storage.clone(),
                    projections: Vec::new(),
                },
                self,
            )?;
            self.add(
                origin,
                "cleanup_slot",
                json!({"slot":slot}),
                cleanup_json["slots"][slot].clone(),
                ids,
                "structural_storage_shape_not_runtime_liveness",
            )?;
        }
        let mut entry_ids = Ids::new();
        for place in &plan.entry_state.live_owned_parameters {
            entry_ids.extend(place_ids(&slots, place, self)?);
        }
        for entry in &plan.entry_state.conditional_owned_parameters {
            entry_ids.insert(entry.variant.as_str().to_owned());
            for case in &entry.cases {
                entry_ids.insert(case.case.as_str().to_owned());
                for place in &case.live_places {
                    entry_ids.extend(place_ids(&slots, place, self)?);
                }
            }
        }
        self.add(
            origin,
            "cleanup_entry",
            json!({}),
            cleanup_json["entry_state"].clone(),
            entry_ids,
            "declared_entry_state_not_observed_liveness",
        )?;
        for (block_index, block) in plan.blocks.iter().enumerate() {
            for (transition_index, transition) in block.transitions.iter().enumerate() {
                let mut ids = Ids::new();
                match transition {
                    Transition::Initialize { destination, .. } => {
                        ids.extend(place_ids(&slots, destination, self)?)
                    }
                    Transition::InitializeVariant {
                        destination,
                        variant,
                        ..
                    } => {
                        ids.extend(place_ids(&slots, destination, self)?);
                        ids.insert(variant.as_str().to_owned());
                    }
                    Transition::Transfer {
                        source,
                        destination,
                        ..
                    }
                    | Transition::TransferVariant {
                        source,
                        destination,
                        ..
                    } => {
                        ids.extend(place_ids(&slots, source, self)?);
                        ids.extend(place_ids(&slots, destination, self)?);
                    }
                    Transition::AuthenticateVariantCase {
                        source,
                        variant,
                        case,
                        ..
                    } => {
                        ids.extend(place_ids(&slots, source, self)?);
                        ids.insert(variant.as_str().to_owned());
                        ids.insert(case.as_str().to_owned());
                    }
                    Transition::CallCommit { arguments, .. } => {
                        for argument in arguments {
                            ids.extend(place_ids(&slots, &argument.source, self)?);
                        }
                    }
                    Transition::StageCopyResult { source } => stage_ids(source, &mut ids, self)?,
                    Transition::SelectFailure { .. } => {}
                }
                self.add(origin,"cleanup_transition",json!({"block":block_index,"transition":transition_index}),cleanup_json["blocks"][block_index]["transitions"][transition_index].clone(),ids,"plan_place_prefix_or_explicit_identity;whole_places_are_structural_associations")?;
            }
        }
        for (edge_index, edge) in plan.edges.iter().enumerate() {
            let ids = match &edge.condition {
                cp::EdgeCondition::VariantCase { case, .. } => {
                    Ids::from([case.as_str().to_owned()])
                }
                _ => Ids::new(),
            };
            self.add(
                origin,
                "cleanup_edge",
                json!({"edge":edge_index}),
                cleanup_json["edges"][edge_index].clone(),
                ids,
                "explicit_variant_case_control_dependency",
            )?;
        }
        for (region_index, region) in plan.regions.iter().enumerate() {
            let mut ids = Ids::new();
            for storage in &region.slots {
                ids.extend(place_ids(
                    &slots,
                    &CleanupPlace {
                        storage: storage.clone(),
                        projections: Vec::new(),
                    },
                    self,
                )?);
            }
            self.add(
                origin,
                "cleanup_region",
                json!({"region":region_index}),
                cleanup_json["regions"][region_index].clone(),
                ids,
                "structural_region_storage_membership_not_live_cleanup",
            )?;
        }
        for (exit_index, exit) in plan.exits.iter().enumerate() {
            let mut exit_ids = Ids::new();
            for (action_index, action) in exit.finalize_in_order.iter().enumerate() {
                let mut ids = place_ids(&slots, &action.source, self)?;
                if let Some(guard) = &action.active_case {
                    ids.insert(guard.variant.as_str().to_owned());
                    ids.insert(guard.case.as_str().to_owned());
                }
                exit_ids.extend(ids.iter().cloned());
                self.add(
                    origin,
                    "cleanup_finalize",
                    json!({"exit":exit_index,"action":action_index}),
                    cleanup_json["exits"][exit_index]["finalize_in_order"][action_index].clone(),
                    ids,
                    "exact_guarded_finalize_source_not_permission_to_finalize",
                )?;
            }
            if let cp::ExitContinuation::CommitResult {
                source: cp::CleanupResultSource::Owned { storage },
            } = &exit.continuation
            {
                exit_ids.extend(place_ids(&slots, storage, self)?);
            }
            self.add(
                origin,
                "cleanup_exit",
                json!({"exit":exit_index}),
                cleanup_json["exits"][exit_index].clone(),
                exit_ids,
                "ordered_exit_contains_selected_finalize_or_owned_result",
            )?;
        }
        let mut loan_ids = BTreeMap::new();
        for (loan_index, item) in loan.loans.iter().enumerate() {
            let projections = item
                .origin
                .projections
                .iter()
                .flat_map(|projection| match projection {
                    PlaceProjection::Field(field) => vec![field.clone()],
                    PlaceProjection::VariantField { case, field } => {
                        vec![case.clone(), field.clone()]
                    }
                })
                .collect();
            let ids = place_ids(
                &slots,
                &CleanupPlace {
                    storage: StorageId::Value(item.origin.root.clone()),
                    projections,
                },
                self,
            )?;
            loan_ids.insert(item.id, ids.clone());
            self.add(
                origin,
                "loan",
                json!({"loan":loan_index}),
                loan_json["loans"][loan_index].clone(),
                ids,
                "exact_retained_loan_origin_prefix",
            )?;
        }
        self.charge((loan.edges.len() + loan.endpoints.len()) * std::mem::size_of::<Ids>())?;
        let mut edge_ids = vec![Ids::new(); loan.edges.len()];
        let mut endpoint_ids = vec![Ids::new(); loan.endpoints.len()];
        for item in &loan.loans {
            for edge in &item.end_edges {
                self.visit(0)?;
                extend_ids(
                    edge_ids.get_mut(usize::from(*edge)).ok_or_else(invalid)?,
                    loan_ids.get(&item.id).ok_or_else(invalid)?,
                    self,
                )?;
            }
        }
        for (edge_index, edge) in loan.edges.iter().enumerate() {
            let ids = &mut edge_ids[edge_index];
            for id in &edge.live {
                extend_ids(ids, loan_ids.get(id).ok_or_else(invalid)?, self)?;
            }
            extend_ids(
                endpoint_ids
                    .get_mut(usize::from(edge.from))
                    .ok_or_else(invalid)?,
                ids,
                self,
            )?;
            extend_ids(
                endpoint_ids
                    .get_mut(usize::from(edge.to))
                    .ok_or_else(invalid)?,
                ids,
                self,
            )?;
            self.add(
                origin,
                "loan_edge",
                json!({"edge":edge_index}),
                loan_json["edges"][edge_index].clone(),
                ids.clone(),
                "exact_live_or_terminating_loan_edge",
            )?;
        }
        for (endpoint_index, endpoint) in loan.endpoints.iter().enumerate() {
            let ids = &mut endpoint_ids[endpoint_index];
            for id in endpoint
                .live_before
                .iter()
                .chain(&endpoint.starts)
                .chain(&endpoint.kills)
                .chain(&endpoint.live_after)
            {
                extend_ids(ids, loan_ids.get(id).ok_or_else(invalid)?, self)?;
            }
            self.add(
                origin,
                "loan_endpoint",
                json!({"endpoint":endpoint_index}),
                loan_json["endpoints"][endpoint_index].clone(),
                ids.clone(),
                "node_union_or_exact_selected_edge_boundary;edge_vectors_remain_authoritative",
            )?;
        }
        Ok(())
    }
    fn nodes(&mut self, ty: &hir::ResolvedType, shape: &FieldLivenessShape) -> Result<ShapeNodes> {
        let mut ids = Ids::new();
        type_ids(ty, &mut ids, self, 0)?;
        let mut nodes = Vec::new();
        for id in ids {
            push_node(&mut nodes, &[], &id, self)?;
        }
        shape_nodes(shape, &mut Vec::new(), &mut nodes, self, 0)?;
        Ok(nodes)
    }
    fn plan_value(&mut self, render: impl FnOnce() -> String) -> Result<Value> {
        let remaining = (MAX_BYTES - self.bytes).min(MAX_IMAGE_CLEANUP_DEPENDENCIES_BYTES);
        let (text, overflow, work) = crate::bounded_output::with_limit_usage(remaining, render);
        if overflow {
            return Err(limit());
        }
        self.charge(work)?;
        let mut depth = 0usize;
        let mut quoted = false;
        let mut escaped = false;
        for byte in text.bytes() {
            if quoted {
                if escaped {
                    escaped = false;
                } else if byte == b'\\' {
                    escaped = true;
                } else if byte == b'"' {
                    quoted = false;
                }
            } else {
                match byte {
                    b'"' => quoted = true,
                    b'{' | b'[' => {
                        depth += 1;
                        self.visit(depth)?;
                    }
                    b'}' | b']' => depth = depth.checked_sub(1).ok_or_else(invalid)?,
                    _ => {}
                }
            }
        }
        let value: Value = serde_json::from_str(&text).map_err(|_| invalid())?;
        self.value(&value)?;
        Ok(value)
    }
    fn inventory(&mut self, function: &hir::ResolvedFunction, origin: &Value) -> Result<()> {
        let inventory = &function.cleanup;
        let mut slots = BTreeMap::new();
        for (slot_index, slot) in inventory.slots.iter().enumerate() {
            self.items(1)?;
            let nodes = self.nodes(&slot.ty, &slot.shape)?;
            let ids = nodes.iter().map(|(_, id)| id.clone()).collect();
            let storage_origin = match &slot.origin {
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
            let fact = json!({"id":slot.id.0,"discovery_index":slot.discovery_index,"origin":storage_origin,"type_id":slot.ty.identity_key(),"shape":shape_value(&slot.shape,self,0)?});
            self.add(
                origin,
                "inventory_slot",
                json!({"slot":slot_index}),
                fact,
                ids,
                "structural_discovery_not_runtime_destruction",
            )?;
            if slots.insert(slot.id, nodes).is_some() {
                return Err(invalid());
            }
        }
        let mut flags = BTreeMap::new();
        for (flag_index, flag) in inventory.flags.iter().enumerate() {
            let nodes = slots.get(&flag.place.storage).ok_or_else(invalid)?;
            let ids = prefix_ids(nodes, &flag.place.projections, self)?;
            flags.insert(flag.id, ids.clone());
            let fact = json!({"id":flag.id.0,"place":{"storage":flag.place.storage.0,"projections":flag.place.projections.iter().map(|id|id.as_str()).collect::<Vec<_>>()},"lifecycle":flag.lifecycle.as_str()});
            self.add(
                origin,
                "inventory_flag",
                json!({"flag":flag_index}),
                fact,
                ids,
                "exact_inventory_leaf_flag_not_observed_liveness",
            )?;
        }
        let mut ids = Ids::new();
        for storage in &inventory.entry_state.live_owned_parameters {
            ids.extend(prefix_ids(
                slots.get(storage).ok_or_else(invalid)?,
                &[],
                self,
            )?);
        }
        let mut conditional = Vec::new();
        for entry in &inventory.entry_state.conditional_owned_parameters {
            ids.insert(entry.variant.as_str().to_owned());
            let mut cases = Vec::new();
            for case in &entry.cases {
                ids.insert(case.case.as_str().to_owned());
                for flag in &case.live_flags {
                    ids.extend(flags.get(flag).ok_or_else(invalid)?.iter().cloned());
                }
                cases.push(json!({"case":case.case.as_str(),"live_flags":case.live_flags.iter().map(|flag|flag.0).collect::<Vec<_>>()}));
            }
            conditional.push(
                json!({"storage":entry.storage.0,"variant":entry.variant.as_str(),"cases":cases}),
            );
        }
        self.add(origin,"inventory_entry",json!({}),json!({"live_owned_parameters":inventory.entry_state.live_owned_parameters.iter().map(|id|id.0).collect::<Vec<_>>(),"conditional_owned_parameters":conditional}),ids,"declared_inventory_entry_not_runtime_liveness")
    }
}

fn type_ids(
    ty: &hir::ResolvedType,
    ids: &mut Ids,
    index: &mut CleanupDependencyIndex,
    depth: usize,
) -> Result<()> {
    index.visit(depth)?;
    if let hir::ResolvedType::Nominal {
        declaration,
        arguments,
    } = ty
    {
        index.charge(declaration.as_str().len() + 96)?;
        ids.insert(declaration.as_str().to_owned());
        for argument in arguments {
            type_ids(argument, ids, index, depth + 1)?;
        }
    }
    Ok(())
}
fn extend_ids(
    destination: &mut Ids,
    source: &Ids,
    index: &mut CleanupDependencyIndex,
) -> Result<()> {
    for id in source {
        index.visit(0)?;
        if !destination.contains(id) {
            index.charge(id.len() + 96)?;
            destination.insert(id.clone());
        }
    }
    Ok(())
}
fn shape_nodes(
    shape: &FieldLivenessShape,
    path: &mut Vec<String>,
    nodes: &mut ShapeNodes,
    index: &mut CleanupDependencyIndex,
    depth: usize,
) -> Result<()> {
    index.visit(depth)?;
    match shape {
        FieldLivenessShape::Record {
            declaration,
            fields,
        } => {
            push_node(nodes, path, declaration.as_str(), index)?;
            for field in fields {
                path.push(field.field.as_str().to_owned());
                push_node(nodes, path, field.field.as_str(), index)?;
                shape_nodes(&field.shape, path, nodes, index, depth + 1)?;
                path.pop();
            }
        }
        FieldLivenessShape::Variant { declaration, cases } => {
            push_node(nodes, path, declaration.as_str(), index)?;
            for case in cases {
                index.visit(depth + 1)?;
                path.push(case.case.as_str().to_owned());
                push_node(nodes, path, case.case.as_str(), index)?;
                for field in &case.fields {
                    path.push(field.field.as_str().to_owned());
                    push_node(nodes, path, field.field.as_str(), index)?;
                    shape_nodes(&field.shape, path, nodes, index, depth + 2)?;
                    path.pop();
                }
                path.pop();
            }
        }
        FieldLivenessShape::Leaf { .. } | FieldLivenessShape::NoDrop => {}
    }
    Ok(())
}
fn push_node(
    nodes: &mut ShapeNodes,
    path: &[String],
    id: &str,
    index: &mut CleanupDependencyIndex,
) -> Result<()> {
    index.items(1)?;
    index.charge(id.len() + 128 + path.iter().map(|id| id.len() + 32).sum::<usize>())?;
    nodes.push((path.to_vec(), id.to_owned()));
    Ok(())
}
fn prefix_ids(
    nodes: &ShapeNodes,
    projections: &[hir::DeclarationId],
    index: &mut CleanupDependencyIndex,
) -> Result<Ids> {
    index.visit(projections.len())?;
    index.charge(projections.len() * std::mem::size_of::<&str>())?;
    let path = projections.iter().map(|id| id.as_str()).collect::<Vec<_>>();
    let mut ids = Ids::new();
    for (node, id) in nodes {
        index.visit(0)?;
        let mut common = true;
        for (left, right) in node.iter().map(String::as_str).zip(&path) {
            index.visit(0)?;
            if left != *right {
                common = false;
                break;
            }
        }
        if common {
            index.charge(id.len() + 96)?;
            ids.insert(id.clone());
        }
    }
    Ok(ids)
}
fn place_ids(
    slots: &Slots,
    place: &CleanupPlace,
    index: &mut CleanupDependencyIndex,
) -> Result<Ids> {
    let nodes = slots.get(&place.storage).ok_or_else(invalid)?;
    prefix_ids(nodes, &place.projections, index)
}
fn stage_ids(
    source: &cp::StagedCopyResultSource,
    ids: &mut Ids,
    index: &mut CleanupDependencyIndex,
) -> Result<()> {
    match source {
        cp::StagedCopyResultSource::Body { instance, .. } => type_ids(instance, ids, index, 0)?,
        cp::StagedCopyResultSource::TryResidual {
            source_instance,
            target_instance,
            result,
            ok_case,
            ok_field,
            err_case,
            err_field,
            ..
        } => {
            type_ids(source_instance, ids, index, 0)?;
            type_ids(target_instance, ids, index, 0)?;
            ids.extend(
                [result, ok_case, ok_field, err_case, err_field]
                    .into_iter()
                    .map(|id| id.as_str().to_owned()),
            );
        }
        cp::StagedCopyResultSource::TryOptionNone {
            source_instance,
            target_instance,
            option,
            some_case,
            some_field,
            none_case,
            ..
        } => {
            type_ids(source_instance, ids, index, 0)?;
            type_ids(target_instance, ids, index, 0)?;
            ids.extend(
                [option, some_case, some_field, none_case]
                    .into_iter()
                    .map(|id| id.as_str().to_owned()),
            );
        }
    }
    Ok(())
}
fn shape_value(
    shape: &FieldLivenessShape,
    index: &mut CleanupDependencyIndex,
    depth: usize,
) -> Result<Value> {
    index.visit(depth)?;
    Ok(match shape {
        FieldLivenessShape::NoDrop => json!({"kind":"no_drop"}),
        FieldLivenessShape::Leaf { flag, lifecycle } => {
            json!({"kind":"leaf","flag":flag.0,"lifecycle":lifecycle.as_str()})
        }
        FieldLivenessShape::Record {
            declaration,
            fields,
        } => {
            let mut values = Vec::new();
            for field in fields {
                values.push(json!({"field":field.field.as_str(),"field_index":field.field_index,"shape":shape_value(&field.shape,index,depth+1)?}));
            }
            json!({"kind":"record","declaration":declaration.as_str(),"fields":values})
        }
        FieldLivenessShape::Variant { declaration, cases } => {
            let mut values = Vec::new();
            for case in cases {
                index.visit(depth + 1)?;
                let mut fields = Vec::new();
                for field in &case.fields {
                    fields.push(json!({"field":field.field.as_str(),"field_index":field.field_index,"shape":shape_value(&field.shape,index,depth+2)?}));
                }
                values.push(
                    json!({"case":case.case.as_str(),"case_index":case.case_index,"fields":fields}),
                );
            }
            json!({"kind":"variant","declaration":declaration.as_str(),"cases":values})
        }
    })
}
fn origin(
    revision: &ProjectRevision,
    module: &WorkspaceGraphProjectionModule,
    function: &str,
    instance: Option<&str>,
) -> Result<Value> {
    let declaration = revision
        .semantic
        .image_symbol(function)
        .ok_or_else(invalid)?;
    if declaration["path"].as_str() != Some(module.path()) {
        return Err(invalid());
    }
    Ok(
        json!({"function_id":function,"instance_id":instance,"path":module.path(),"module":module.module(),"source_revision":module.source_revision(),"source_digest":module.source_digest(),"source_declaration":declaration,"cleanup_inventory_schema":null,"evidence_owner":"retained_checked_hir_cleanup_inventory_cleanup_plan_loan_plan"}),
    )
}

impl ProjectSemanticImage {
    /// Structural reverse dependencies; never executes a plan or infers that
    /// a selected flag, case, branch, loan, or finalizer is live at runtime.
    pub fn cleanup_dependencies(&self, expected_image: &str, target: &str) -> Result<String> {
        self.require_digest(expected_image)?;
        let selection = self.cleanup_selection(target)?;
        let dependency = self.dependency_index()?;
        let index = dependency
            .cleanup
            .get_or_init(|| CleanupDependencyIndex::build(self.revision()))
            .as_ref()
            .map_err(Clone::clone)?;
        self.cleanup_report(target, selection, index)
    }
    /// Recomputes this descriptive facet from the retained checked HIR without
    /// consulting its cached child. This is not cold-source or target replay.
    pub fn verify_cleanup_dependencies(
        &self,
        expected_image: &str,
        target: &str,
        bytes: &[u8],
    ) -> Result<String> {
        self.require_digest(expected_image)?;
        let selection = self.cleanup_selection(target)?;
        if bytes.len() > MAX_IMAGE_CLEANUP_DEPENDENCIES_BYTES {
            return Err(limit());
        }
        let fresh = CleanupDependencyIndex::build(self.revision())?;
        let expected = self.cleanup_report(target, selection, &fresh)?;
        if expected.as_bytes() != bytes {
            return Err(vec![Diagnostic::io(
                "SPX-G336",
                "cleanup dependency report does not exactly replay the selected retained image",
            )]);
        }
        super::super::image::render(json!({"schema":IMAGE_CLEANUP_DEPENDENCIES_VERIFICATION_SCHEMA,"image_digest":self.image_digest(),"target":target,"result":"exact_retained_hir_recomputation","source_authority":false,"execution":false,"nonclaims":["not_cold_source_rebuild","no_execution_or_source_authority"]}),true,4096).map_err(|_|limit())
    }
    fn cleanup_selection(&self, target: &str) -> Result<Ids> {
        if target.is_empty() || target.len() > 4096 || target.contains('\0') {
            return Err(invalid());
        }
        let revision = self.revision();
        let declaration = revision.semantic.image_symbol(target).ok_or_else(invalid)?;
        let path = declaration["path"].as_str().ok_or_else(invalid)?;
        if !revision
            .sources()
            .iter()
            .any(|source| source.path() == path)
        {
            return Err(invalid());
        }
        let dependency = self.dependency_index()?;
        if !dependency.typed.contains_key(target) {
            return Err(invalid());
        }
        let mut selected = Ids::from([target.to_owned()]);
        let mut pending = vec![target.to_owned()];
        while let Some(id) = pending.pop() {
            if let Some(members) = dependency.members.get(&id) {
                for member in members {
                    if selected.insert(member.clone()) {
                        pending.push(member.clone());
                    }
                }
            }
            if selected.len() > MAX_ITEMS {
                return Err(limit());
            }
        }
        Ok(selected)
    }
    fn cleanup_report(
        &self,
        target: &str,
        selected: Ids,
        index: &CleanupDependencyIndex,
    ) -> Result<String> {
        let revision = self.revision();
        let declaration = revision.semantic.image_symbol(target).ok_or_else(invalid)?;
        let path = declaration["path"].as_str().ok_or_else(invalid)?;
        let source = revision
            .sources()
            .iter()
            .find(|source| source.path() == path)
            .ok_or_else(invalid)?;
        let mut ordinals = BTreeSet::new();
        for id in &selected {
            if let Some(rows) = index.by_id.get(id) {
                ordinals.extend(rows.iter().copied());
            }
        }
        let mut rows = Vec::new();
        let mut bytes = 0usize;
        for ordinal in ordinals {
            let mut row = index.rows[ordinal].clone();
            row["matched_declaration_ids"] = json!(row["matched_declaration_ids"]
                .as_array()
                .ok_or_else(invalid)?
                .iter()
                .filter(|id| id.as_str().is_some_and(|id| selected.contains(id)))
                .cloned()
                .collect::<Vec<_>>());
            bytes = bytes
                .checked_add(super::value_bytes(&row).map_err(|_| limit())?)
                .ok_or_else(limit)?;
            if bytes > MAX_BYTES {
                return Err(limit());
            }
            rows.push(row);
        }
        let report = json!({"schema":IMAGE_CLEANUP_DEPENDENCIES_SCHEMA,"image_digest":self.image_digest(),"project_revision":revision.project_revision(),"workspace_revision":revision.workspace_revision(),"target":target,"declaration":declaration,"source_binding":{"path":source.path(),"source_revision":source.source_revision(),"source_digest":source.source_digest()},"typed_declaration":self.dependency_index()?.typed_declaration(target),"selected_declaration_ids":selected,"availability":"retained_checked_plans_with_explicit_template_gaps","obligations":rows,"unavailable_templates":index.unavailable,"evidence_owner":"retained_checked_hir_cleanup_inventory_cleanup_plan_loan_plan","evidence_class":"descriptive_recomputable_compiler_projection","limits":{"max_report_bytes":MAX_IMAGE_CLEANUP_DEPENDENCIES_BYTES,"max_retained_bytes":MAX_BYTES,"max_items":MAX_ITEMS,"max_work":MAX_VISITS,"max_depth":MAX_DEPTH},"index_work":{"items":index.items,"visits":index.visits,"charged_bytes":index.bytes,"functions":index.functions,"instances":index.instances,"rows":index.rows.len()},"nonclaims":["no_runtime_liveness_or_path_feasibility","structural_whole_storage_association_is_not_field_read_or_live_obligation","no_finalizer_execution_or_authority","no_alias_inference_or_external_callers","template_has_no_executable_plan;retained_instances_are_separate","no_second_expression_reference_index","no_source_test_target_or_filesystem_access"]});
        super::super::image::render(report, true, MAX_IMAGE_CLEANUP_DEPENDENCIES_BYTES)
            .map_err(|_| limit())
    }
}
fn invalid() -> Vec<Diagnostic> {
    vec![Diagnostic::io(
        "SPX-G334",
        "cleanup dependency target or retained proof binding is invalid",
    )]
}
fn limit() -> Vec<Diagnostic> {
    vec![Diagnostic::io(
        "SPX-G335",
        "cleanup dependency facet exceeds its bounded work or output",
    )]
}
