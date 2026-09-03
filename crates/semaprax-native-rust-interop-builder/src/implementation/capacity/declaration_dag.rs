//! Declaration DAG expansion and the retained cleanup statistics the
//! expansion accumulates.

use super::*;

#[derive(Clone, Copy, Debug)]
pub(in crate::implementation) struct DeclarationDagExpansion {
    pub(in crate::implementation) maximum_resource_leaves: usize,
    pub(in crate::implementation) maximum_type_occurrences: usize,
    pub(in crate::implementation) maximum_shape_fields: usize,
    pub(in crate::implementation) maximum_projection_segments: usize,
    pub(in crate::implementation) maximum_shape_identity_bytes: usize,
    pub(in crate::implementation) maximum_lifecycle_identity_bytes: usize,
    pub(in crate::implementation) maximum_projection_identity_bytes: usize,
    pub(in crate::implementation) cleanup_retained: CleanupRetainedStats,
}

#[derive(Clone, Copy, Default)]
pub(in crate::implementation) struct CleanupTypeFacts {
    pub(in crate::implementation) leaves: usize,
    pub(in crate::implementation) occurrences: usize,
    pub(in crate::implementation) shape_fields: usize,
    pub(in crate::implementation) projection_segments: usize,
    pub(in crate::implementation) shape_ids: usize,
    pub(in crate::implementation) lifecycle_ids: usize,
    pub(in crate::implementation) projection_ids: usize,
}

#[derive(Clone, Copy, Debug, Default)]
pub(in crate::implementation) struct CleanupRetainedStats {
    pub(in crate::implementation) roots: usize,
    pub(in crate::implementation) occurrences: usize,
    pub(in crate::implementation) shape_fields: usize,
    pub(in crate::implementation) leaves: usize,
    pub(in crate::implementation) projection_segments: usize,
    pub(in crate::implementation) shape_ids: usize,
    pub(in crate::implementation) lifecycle_ids: usize,
    pub(in crate::implementation) projection_ids: usize,
    pub(in crate::implementation) finalizer_copies: usize,
    pub(in crate::implementation) finalizer_projection_segments: usize,
    pub(in crate::implementation) finalizer_lifecycle_ids: usize,
    pub(in crate::implementation) finalizer_projection_ids: usize,
    pub(in crate::implementation) place_copies: usize,
    pub(in crate::implementation) place_projection_segments: usize,
    pub(in crate::implementation) place_projection_ids: usize,
    pub(in crate::implementation) call_arguments: usize,
    pub(in crate::implementation) call_argument_owned_bytes: usize,
    pub(in crate::implementation) parent_local_epochs: usize,
    pub(in crate::implementation) parent_local_zero_lifetime_transfers: usize,
    pub(in crate::implementation) parent_local_partial_fields: usize,
    pub(in crate::implementation) parent_local_finalizer_copies: usize,
    pub(in crate::implementation) parent_local_finalizer_projection_segments: usize,
    pub(in crate::implementation) parent_local_finalizer_lifecycle_ids: usize,
    pub(in crate::implementation) parent_local_finalizer_projection_ids: usize,
    pub(in crate::implementation) parent_local_finalizer_storage_bytes: usize,
    pub(in crate::implementation) parent_local_projection_epochs: usize,
    pub(in crate::implementation) parent_local_projection_exit_groups: usize,
    pub(in crate::implementation) parent_local_projection_finalizer_copies: usize,
    pub(in crate::implementation) parent_local_projection_finalizer_projection_segments: usize,
    pub(in crate::implementation) parent_local_projection_finalizer_lifecycle_ids: usize,
    pub(in crate::implementation) parent_local_projection_finalizer_projection_ids: usize,
    pub(in crate::implementation) parent_local_projection_finalizer_storage_bytes: usize,
    pub(in crate::implementation) parent_local_update_prefix_fields: usize,
    pub(in crate::implementation) parent_local_update_prefix_exit_groups: usize,
    pub(in crate::implementation) parent_local_update_prefix_finalizer_copies: usize,
    pub(in crate::implementation) parent_local_update_prefix_finalizer_projection_segments: usize,
    pub(in crate::implementation) parent_local_update_prefix_finalizer_lifecycle_ids: usize,
    pub(in crate::implementation) parent_local_update_prefix_finalizer_projection_ids: usize,
    pub(in crate::implementation) parent_local_update_prefix_finalizer_storage_bytes: usize,
    pub(in crate::implementation) ordinary_slot_payload_bytes: usize,
    pub(in crate::implementation) ordinary_place_storage_bytes: usize,
    pub(in crate::implementation) ordinary_finalizer_storage_bytes: usize,
    pub(in crate::implementation) staged_results: usize,
    pub(in crate::implementation) variant_edges: usize,
    pub(in crate::implementation) stage_identity_and_type_bytes: usize,
    pub(in crate::implementation) variant_identity_bytes: usize,
    pub(in crate::implementation) fallback_roots: usize,
    pub(in crate::implementation) exit_events: usize,
}

impl CleanupRetainedStats {
    pub(in crate::implementation) fn add_root(&mut self, facts: CleanupTypeFacts) -> Option<()> {
        if facts.leaves == 0 {
            return Some(());
        }
        self.roots = self.roots.checked_add(1)?;
        self.occurrences = self.occurrences.checked_add(facts.occurrences)?;
        self.shape_fields = self.shape_fields.checked_add(facts.shape_fields)?;
        self.leaves = self.leaves.checked_add(facts.leaves)?;
        self.projection_segments = self
            .projection_segments
            .checked_add(facts.projection_segments)?;
        self.shape_ids = self.shape_ids.checked_add(facts.shape_ids)?;
        self.lifecycle_ids = self.lifecycle_ids.checked_add(facts.lifecycle_ids)?;
        self.projection_ids = self.projection_ids.checked_add(facts.projection_ids)?;
        Some(())
    }

    pub(in crate::implementation) fn merge(&mut self, other: Self) -> Option<()> {
        self.roots = self.roots.checked_add(other.roots)?;
        self.occurrences = self.occurrences.checked_add(other.occurrences)?;
        self.shape_fields = self.shape_fields.checked_add(other.shape_fields)?;
        self.leaves = self.leaves.checked_add(other.leaves)?;
        self.projection_segments = self
            .projection_segments
            .checked_add(other.projection_segments)?;
        self.shape_ids = self.shape_ids.checked_add(other.shape_ids)?;
        self.lifecycle_ids = self.lifecycle_ids.checked_add(other.lifecycle_ids)?;
        self.projection_ids = self.projection_ids.checked_add(other.projection_ids)?;
        self.finalizer_copies = self.finalizer_copies.checked_add(other.finalizer_copies)?;
        self.finalizer_projection_segments = self
            .finalizer_projection_segments
            .checked_add(other.finalizer_projection_segments)?;
        self.finalizer_lifecycle_ids = self
            .finalizer_lifecycle_ids
            .checked_add(other.finalizer_lifecycle_ids)?;
        self.finalizer_projection_ids = self
            .finalizer_projection_ids
            .checked_add(other.finalizer_projection_ids)?;
        self.place_copies = self.place_copies.checked_add(other.place_copies)?;
        self.place_projection_segments = self
            .place_projection_segments
            .checked_add(other.place_projection_segments)?;
        self.place_projection_ids = self
            .place_projection_ids
            .checked_add(other.place_projection_ids)?;
        self.call_arguments = self.call_arguments.checked_add(other.call_arguments)?;
        self.call_argument_owned_bytes = self
            .call_argument_owned_bytes
            .checked_add(other.call_argument_owned_bytes)?;
        self.parent_local_epochs = self
            .parent_local_epochs
            .checked_add(other.parent_local_epochs)?;
        self.parent_local_zero_lifetime_transfers = self
            .parent_local_zero_lifetime_transfers
            .checked_add(other.parent_local_zero_lifetime_transfers)?;
        self.parent_local_partial_fields = self
            .parent_local_partial_fields
            .checked_add(other.parent_local_partial_fields)?;
        self.parent_local_finalizer_copies = self
            .parent_local_finalizer_copies
            .checked_add(other.parent_local_finalizer_copies)?;
        self.parent_local_finalizer_projection_segments = self
            .parent_local_finalizer_projection_segments
            .checked_add(other.parent_local_finalizer_projection_segments)?;
        self.parent_local_finalizer_lifecycle_ids = self
            .parent_local_finalizer_lifecycle_ids
            .checked_add(other.parent_local_finalizer_lifecycle_ids)?;
        self.parent_local_finalizer_projection_ids = self
            .parent_local_finalizer_projection_ids
            .checked_add(other.parent_local_finalizer_projection_ids)?;
        self.parent_local_finalizer_storage_bytes = self
            .parent_local_finalizer_storage_bytes
            .checked_add(other.parent_local_finalizer_storage_bytes)?;
        self.parent_local_projection_epochs = self
            .parent_local_projection_epochs
            .checked_add(other.parent_local_projection_epochs)?;
        self.parent_local_projection_exit_groups = self
            .parent_local_projection_exit_groups
            .checked_add(other.parent_local_projection_exit_groups)?;
        self.parent_local_projection_finalizer_copies = self
            .parent_local_projection_finalizer_copies
            .checked_add(other.parent_local_projection_finalizer_copies)?;
        self.parent_local_projection_finalizer_projection_segments = self
            .parent_local_projection_finalizer_projection_segments
            .checked_add(other.parent_local_projection_finalizer_projection_segments)?;
        self.parent_local_projection_finalizer_lifecycle_ids = self
            .parent_local_projection_finalizer_lifecycle_ids
            .checked_add(other.parent_local_projection_finalizer_lifecycle_ids)?;
        self.parent_local_projection_finalizer_projection_ids = self
            .parent_local_projection_finalizer_projection_ids
            .checked_add(other.parent_local_projection_finalizer_projection_ids)?;
        self.parent_local_projection_finalizer_storage_bytes = self
            .parent_local_projection_finalizer_storage_bytes
            .checked_add(other.parent_local_projection_finalizer_storage_bytes)?;
        self.parent_local_update_prefix_fields = self
            .parent_local_update_prefix_fields
            .checked_add(other.parent_local_update_prefix_fields)?;
        self.parent_local_update_prefix_exit_groups =
            self.parent_local_update_prefix_exit_groups
                .checked_add(other.parent_local_update_prefix_exit_groups)?;
        self.parent_local_update_prefix_finalizer_copies = self
            .parent_local_update_prefix_finalizer_copies
            .checked_add(other.parent_local_update_prefix_finalizer_copies)?;
        self.parent_local_update_prefix_finalizer_projection_segments = self
            .parent_local_update_prefix_finalizer_projection_segments
            .checked_add(other.parent_local_update_prefix_finalizer_projection_segments)?;
        self.parent_local_update_prefix_finalizer_lifecycle_ids = self
            .parent_local_update_prefix_finalizer_lifecycle_ids
            .checked_add(other.parent_local_update_prefix_finalizer_lifecycle_ids)?;
        self.parent_local_update_prefix_finalizer_projection_ids = self
            .parent_local_update_prefix_finalizer_projection_ids
            .checked_add(other.parent_local_update_prefix_finalizer_projection_ids)?;
        self.parent_local_update_prefix_finalizer_storage_bytes = self
            .parent_local_update_prefix_finalizer_storage_bytes
            .checked_add(other.parent_local_update_prefix_finalizer_storage_bytes)?;
        self.ordinary_slot_payload_bytes = self
            .ordinary_slot_payload_bytes
            .checked_add(other.ordinary_slot_payload_bytes)?;
        self.ordinary_place_storage_bytes = self
            .ordinary_place_storage_bytes
            .checked_add(other.ordinary_place_storage_bytes)?;
        self.ordinary_finalizer_storage_bytes = self
            .ordinary_finalizer_storage_bytes
            .checked_add(other.ordinary_finalizer_storage_bytes)?;
        self.staged_results = self.staged_results.checked_add(other.staged_results)?;
        self.variant_edges = self.variant_edges.checked_add(other.variant_edges)?;
        self.stage_identity_and_type_bytes = self
            .stage_identity_and_type_bytes
            .checked_add(other.stage_identity_and_type_bytes)?;
        self.variant_identity_bytes = self
            .variant_identity_bytes
            .checked_add(other.variant_identity_bytes)?;
        self.fallback_roots = self.fallback_roots.checked_add(other.fallback_roots)?;
        self.exit_events = self.exit_events.checked_add(other.exit_events)?;
        Some(())
    }

    pub(in crate::implementation) fn scaled(self, multiplier: usize) -> Option<Self> {
        Some(Self {
            roots: self.roots.checked_mul(multiplier)?,
            occurrences: self.occurrences.checked_mul(multiplier)?,
            shape_fields: self.shape_fields.checked_mul(multiplier)?,
            leaves: self.leaves.checked_mul(multiplier)?,
            projection_segments: self.projection_segments.checked_mul(multiplier)?,
            shape_ids: self.shape_ids.checked_mul(multiplier)?,
            lifecycle_ids: self.lifecycle_ids.checked_mul(multiplier)?,
            projection_ids: self.projection_ids.checked_mul(multiplier)?,
            finalizer_copies: self.finalizer_copies.checked_mul(multiplier)?,
            finalizer_projection_segments: self
                .finalizer_projection_segments
                .checked_mul(multiplier)?,
            finalizer_lifecycle_ids: self.finalizer_lifecycle_ids.checked_mul(multiplier)?,
            finalizer_projection_ids: self.finalizer_projection_ids.checked_mul(multiplier)?,
            place_copies: self.place_copies.checked_mul(multiplier)?,
            place_projection_segments: self.place_projection_segments.checked_mul(multiplier)?,
            place_projection_ids: self.place_projection_ids.checked_mul(multiplier)?,
            call_arguments: self.call_arguments.checked_mul(multiplier)?,
            call_argument_owned_bytes: self.call_argument_owned_bytes.checked_mul(multiplier)?,
            parent_local_epochs: self.parent_local_epochs.checked_mul(multiplier)?,
            parent_local_zero_lifetime_transfers: self
                .parent_local_zero_lifetime_transfers
                .checked_mul(multiplier)?,
            parent_local_partial_fields: self
                .parent_local_partial_fields
                .checked_mul(multiplier)?,
            parent_local_finalizer_copies: self
                .parent_local_finalizer_copies
                .checked_mul(multiplier)?,
            parent_local_finalizer_projection_segments: self
                .parent_local_finalizer_projection_segments
                .checked_mul(multiplier)?,
            parent_local_finalizer_lifecycle_ids: self
                .parent_local_finalizer_lifecycle_ids
                .checked_mul(multiplier)?,
            parent_local_finalizer_projection_ids: self
                .parent_local_finalizer_projection_ids
                .checked_mul(multiplier)?,
            parent_local_finalizer_storage_bytes: self
                .parent_local_finalizer_storage_bytes
                .checked_mul(multiplier)?,
            parent_local_projection_epochs: self
                .parent_local_projection_epochs
                .checked_mul(multiplier)?,
            parent_local_projection_exit_groups: self
                .parent_local_projection_exit_groups
                .checked_mul(multiplier)?,
            parent_local_projection_finalizer_copies: self
                .parent_local_projection_finalizer_copies
                .checked_mul(multiplier)?,
            parent_local_projection_finalizer_projection_segments: self
                .parent_local_projection_finalizer_projection_segments
                .checked_mul(multiplier)?,
            parent_local_projection_finalizer_lifecycle_ids: self
                .parent_local_projection_finalizer_lifecycle_ids
                .checked_mul(multiplier)?,
            parent_local_projection_finalizer_projection_ids: self
                .parent_local_projection_finalizer_projection_ids
                .checked_mul(multiplier)?,
            parent_local_projection_finalizer_storage_bytes: self
                .parent_local_projection_finalizer_storage_bytes
                .checked_mul(multiplier)?,
            parent_local_update_prefix_fields: self
                .parent_local_update_prefix_fields
                .checked_mul(multiplier)?,
            parent_local_update_prefix_exit_groups: self
                .parent_local_update_prefix_exit_groups
                .checked_mul(multiplier)?,
            parent_local_update_prefix_finalizer_copies: self
                .parent_local_update_prefix_finalizer_copies
                .checked_mul(multiplier)?,
            parent_local_update_prefix_finalizer_projection_segments: self
                .parent_local_update_prefix_finalizer_projection_segments
                .checked_mul(multiplier)?,
            parent_local_update_prefix_finalizer_lifecycle_ids: self
                .parent_local_update_prefix_finalizer_lifecycle_ids
                .checked_mul(multiplier)?,
            parent_local_update_prefix_finalizer_projection_ids: self
                .parent_local_update_prefix_finalizer_projection_ids
                .checked_mul(multiplier)?,
            parent_local_update_prefix_finalizer_storage_bytes: self
                .parent_local_update_prefix_finalizer_storage_bytes
                .checked_mul(multiplier)?,
            ordinary_slot_payload_bytes: self
                .ordinary_slot_payload_bytes
                .checked_mul(multiplier)?,
            ordinary_place_storage_bytes: self
                .ordinary_place_storage_bytes
                .checked_mul(multiplier)?,
            ordinary_finalizer_storage_bytes: self
                .ordinary_finalizer_storage_bytes
                .checked_mul(multiplier)?,
            staged_results: self.staged_results.checked_mul(multiplier)?,
            variant_edges: self.variant_edges.checked_mul(multiplier)?,
            stage_identity_and_type_bytes: self
                .stage_identity_and_type_bytes
                .checked_mul(multiplier)?,
            variant_identity_bytes: self.variant_identity_bytes.checked_mul(multiplier)?,
            fallback_roots: self.fallback_roots.checked_mul(multiplier)?,
            exit_events: self.exit_events.checked_mul(multiplier)?,
        })
    }
}

pub(super) fn retained_vec_capacity_extra(
    logical_entries: usize,
    container_upper: usize,
) -> Option<usize> {
    if logical_entries == 0 {
        return Some(0);
    }
    let nonempty_containers = container_upper.min(logical_entries);
    nonempty_containers
        .checked_mul(8)
        .and_then(|capacity| capacity.checked_add(logical_entries.checked_mul(2)?))
        .and_then(|capacity| capacity.checked_sub(logical_entries))
}

pub(in crate::implementation) fn declaration_dag_expansion(
    program: &Program,
    generic_instance_upper: usize,
) -> Result<DeclarationDagExpansion, Diagnostic> {
    fn add_child(
        parent: &mut CleanupTypeFacts,
        child: CleanupTypeFacts,
        edge_ids: usize,
    ) -> Option<()> {
        parent.leaves = parent.leaves.checked_add(child.leaves)?;
        parent.occurrences = parent.occurrences.checked_add(child.occurrences)?;
        parent.shape_fields = parent
            .shape_fields
            .checked_add(1)?
            .checked_add(child.shape_fields)?;
        parent.projection_segments = parent
            .projection_segments
            .checked_add(child.projection_segments)?
            .checked_add(child.leaves)?;
        parent.shape_ids = parent
            .shape_ids
            .checked_add(edge_ids)?
            .checked_add(child.shape_ids)?;
        parent.lifecycle_ids = parent.lifecycle_ids.checked_add(child.lifecycle_ids)?;
        parent.projection_ids = parent
            .projection_ids
            .checked_add(child.projection_ids)?
            .checked_add(child.leaves.checked_mul(edge_ids)?)?;
        Some(())
    }

    let mut cleanup_node_count = 0usize;
    let mut cleanup_scan = [None; MAX_SEMANTIC_EXPRESSION_DEPTH + 1];
    for function in source_functions(program) {
        for root in function
            .requires
            .iter()
            .chain(std::iter::once(&function.body))
            .chain(&function.ensures)
        {
            let mut len = 1usize;
            cleanup_scan[0] = Some((root, 0usize, 0usize));
            while len != 0 {
                len -= 1;
                let (expression, next_child, _) = cleanup_scan[len]
                    .take()
                    .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
                if next_child == 0 {
                    cleanup_node_count = cleanup_node_count
                        .checked_add(1)
                        .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
                }
                let mut child_cursor = next_child;
                if let Some((_, child)) = ast_child(expression, &mut child_cursor) {
                    if len + 2 > cleanup_scan.len() {
                        return Err(b109(
                            "max_semantic_expression_depth",
                            MAX_SEMANTIC_EXPRESSION_DEPTH,
                        ));
                    }
                    cleanup_scan[len] = Some((expression, child_cursor, 0));
                    cleanup_scan[len + 1] = Some((child, 0, 0));
                    len += 2;
                }
            }
        }
    }
    let cleanup_node_capacity = cleanup_node_count.max(1);
    let count = program.types.len().max(1);
    let table_bytes = count
        .checked_mul(
            std::mem::size_of::<u8>()
                + std::mem::size_of::<CleanupTypeFacts>()
                + std::mem::size_of::<Option<(usize, usize, CleanupTypeFacts)>>(),
        )
        .and_then(|bytes| {
            bytes.checked_add(
                cleanup_node_capacity.checked_mul(std::mem::size_of::<CleanupTypeKey>())?,
            )
        })
        .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
    let _table_budget = reserve_temporary_exact(table_bytes)?;
    let mut state = Vec::with_capacity(count);
    let mut facts = Vec::with_capacity(count);
    let mut stack: Vec<Option<(usize, usize, CleanupTypeFacts)>> = Vec::with_capacity(count);
    state.resize(count, 0u8);
    facts.resize(count, CleanupTypeFacts::default());
    stack.resize(count, None);
    if state.capacity() != count || facts.capacity() != count || stack.capacity() != count {
        return Err(b109("max_builder_bytes", MAX_BUILDER_BYTES));
    }
    let mut maximum_resource_leaves = 0usize;
    let mut maximum_type_occurrences = 1usize;
    let mut maximum_shape_fields = 0usize;
    let mut maximum_projection_segments = 0usize;
    let mut maximum_shape_identity_bytes = 0usize;
    let mut maximum_lifecycle_identity_bytes = 0usize;
    let mut maximum_projection_identity_bytes = 0usize;
    for root in 0..program.types.len() {
        if state[root] == 2 {
            continue;
        }
        stack[0] = Some((
            root,
            0,
            CleanupTypeFacts {
                occurrences: 1,
                shape_ids: program.types[root].stable_id.len(),
                ..CleanupTypeFacts::default()
            },
        ));
        state[root] = 1;
        let mut len = 1usize;
        while len != 0 {
            len -= 1;
            let (index, next, total) = stack[len]
                .take()
                .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
            let declaration = &program.types[index];
            if matches!(
                declaration.kind,
                crate::ast::TypeDeclarationKind::Resource { .. }
            ) {
                let lifecycle_bytes = match &declaration.kind {
                    crate::ast::TypeDeclarationKind::Resource { lifecycles } => lifecycles
                        .iter()
                        .filter_map(|lifecycle| lifecycle.stable_id.as_deref())
                        .try_fold(0usize, |bytes, id| bytes.checked_add(id.len()))
                        .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?,
                    _ => unreachable!(),
                };
                facts[index] = CleanupTypeFacts {
                    leaves: 1,
                    occurrences: 1,
                    shape_ids: lifecycle_bytes,
                    lifecycle_ids: lifecycle_bytes,
                    projection_ids: 0,
                    ..CleanupTypeFacts::default()
                };
                maximum_resource_leaves = maximum_resource_leaves.max(1);
                maximum_type_occurrences = maximum_type_occurrences.max(1);
                maximum_shape_identity_bytes = maximum_shape_identity_bytes.max(lifecycle_bytes);
                maximum_lifecycle_identity_bytes =
                    maximum_lifecycle_identity_bytes.max(lifecycle_bytes);
                state[index] = 2;
                if let Some(parent) = len.checked_sub(1).and_then(|parent| stack[parent].as_mut()) {
                    let parent_decl = &program.types[parent.0];
                    let edge = declaration_field_identity_bytes(parent_decl, parent.1 - 1)
                        .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
                    add_child(&mut parent.2, facts[index], edge)
                        .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
                }
                continue;
            }
            let Some(child) = declaration_field_type(declaration, next) else {
                facts[index] = total;
                state[index] = 2;
                maximum_resource_leaves = maximum_resource_leaves.max(total.leaves);
                maximum_type_occurrences = maximum_type_occurrences.max(total.occurrences);
                maximum_shape_fields = maximum_shape_fields.max(total.shape_fields);
                maximum_projection_segments =
                    maximum_projection_segments.max(total.projection_segments);
                maximum_shape_identity_bytes = maximum_shape_identity_bytes.max(total.shape_ids);
                maximum_lifecycle_identity_bytes =
                    maximum_lifecycle_identity_bytes.max(total.lifecycle_ids);
                maximum_projection_identity_bytes =
                    maximum_projection_identity_bytes.max(total.projection_ids);
                if let Some(parent) = len.checked_sub(1).and_then(|parent| stack[parent].as_mut()) {
                    let parent_decl = &program.types[parent.0];
                    let edge = declaration_field_identity_bytes(parent_decl, parent.1 - 1)
                        .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
                    add_child(&mut parent.2, total, edge)
                        .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
                }
                continue;
            };
            stack[len] = Some((index, next + 1, total));
            len += 1;
            let crate::ast::Type::Named { name, .. } = child else {
                let parent = stack[len - 1].as_mut().expect("parent retained");
                parent.2.occurrences = parent
                    .2
                    .occurrences
                    .checked_add(1)
                    .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
                parent.2.shape_fields = parent
                    .2
                    .shape_fields
                    .checked_add(1)
                    .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
                parent.2.shape_ids = parent
                    .2
                    .shape_ids
                    .checked_add(
                        declaration_field_identity_bytes(declaration, next)
                            .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?,
                    )
                    .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
                continue;
            };
            let Some(child_index) = program.types.iter().position(|value| value.name == *name)
            else {
                let parent = stack[len - 1].as_mut().expect("parent retained");
                parent.2.occurrences = parent
                    .2
                    .occurrences
                    .checked_add(1)
                    .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
                parent.2.shape_fields = parent
                    .2
                    .shape_fields
                    .checked_add(1)
                    .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
                parent.2.shape_ids = parent
                    .2
                    .shape_ids
                    .checked_add(
                        declaration_field_identity_bytes(declaration, next)
                            .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?,
                    )
                    .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
                continue;
            };
            match state[child_index] {
                2 => {
                    let parent = stack[len - 1].as_mut().expect("parent retained");
                    let edge = declaration_field_identity_bytes(declaration, next)
                        .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
                    add_child(&mut parent.2, facts[child_index], edge)
                        .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
                }
                1 => return Err(b107("selected identity missing")),
                _ => {
                    if len == stack.len() {
                        return Err(b109("max_builder_bytes", MAX_BUILDER_BYTES));
                    }
                    state[child_index] = 1;
                    stack[len] = Some((
                        child_index,
                        0,
                        CleanupTypeFacts {
                            occurrences: 1,
                            shape_ids: program.types[child_index].stable_id.len(),
                            ..CleanupTypeFacts::default()
                        },
                    ));
                    len += 1;
                }
            }
        }
    }
    let cleanup_retained = cleanup_retained_stats(
        program,
        &facts,
        cleanup_node_capacity,
        generic_instance_upper,
    )?;
    Ok(DeclarationDagExpansion {
        maximum_resource_leaves,
        maximum_type_occurrences,
        maximum_shape_fields,
        maximum_projection_segments,
        maximum_shape_identity_bytes,
        maximum_lifecycle_identity_bytes,
        maximum_projection_identity_bytes,
        cleanup_retained,
    })
}
