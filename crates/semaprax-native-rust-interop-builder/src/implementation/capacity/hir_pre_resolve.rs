//! Pre-resolution HIR capacity: the complete conservative reservation
//! proved before the resolver may run.

use super::*;

#[derive(Clone, Copy)]
pub(in crate::implementation) struct HirPreResolveCapacity {
    pub(in crate::implementation) retained_upper: usize,
    pub(in crate::implementation) scratch_upper: usize,
    pub(in crate::implementation) declaration_index_upper: usize,
    pub(in crate::implementation) cleanup_retained_upper: usize,
    pub(in crate::implementation) cleanup_authority_upper: usize,
    pub(in crate::implementation) cleanup_exit_events_upper: usize,
    pub(in crate::implementation) cleanup_fallback_roots: usize,
    pub(in crate::implementation) cleanup_call_argument_owned_upper: usize,
    pub(in crate::implementation) cleanup_plan_structural_upper: usize,
    #[cfg(test)]
    pub(in crate::implementation) cleanup_parent_local_lifetime_upper: usize,
    #[cfg(test)]
    pub(in crate::implementation) cleanup_parent_local_projection_lifetime_upper: usize,
    #[cfg(test)]
    pub(in crate::implementation) cleanup_parent_local_update_prefix_lifetime_upper: usize,
    #[cfg(test)]
    pub(in crate::implementation) cleanup_proof: CleanupCapacityProofTerms,
    pub(in crate::implementation) phase_peaks: [usize; 8],
    pub(in crate::implementation) disposal_frames: usize,
}

#[cfg(test)]
#[derive(Clone, Copy, Debug)]
pub(in crate::implementation) struct CleanupCapacityProofTerms {
    pub(in crate::implementation) stats: CleanupRetainedStats,
    pub(in crate::implementation) inventory_slot_capacity_entries: usize,
    pub(in crate::implementation) inventory_flag_capacity_entries: usize,
    pub(in crate::implementation) inventory_entry_capacity_entries: usize,
    pub(in crate::implementation) plan_slot_capacity_entries: usize,
    pub(in crate::implementation) plan_entry_capacity_entries: usize,
    pub(in crate::implementation) shape_field_capacity_entries: usize,
    pub(in crate::implementation) flag_projection_capacity_entries: usize,
    pub(in crate::implementation) place_projection_capacity_entries: usize,
    pub(in crate::implementation) finalizer_projection_capacity_entries: usize,
    pub(in crate::implementation) finalizer_capacity_entries: usize,
    pub(in crate::implementation) block_capacity_entries: usize,
    pub(in crate::implementation) edge_capacity_entries: usize,
    pub(in crate::implementation) region_capacity_entries: usize,
    pub(in crate::implementation) exit_capacity_entries: usize,
    pub(in crate::implementation) status_capacity_entries: usize,
    pub(in crate::implementation) transition_capacity_entries: usize,
    pub(in crate::implementation) branch_edge_capacity_entries: usize,
    pub(in crate::implementation) region_slot_capacity_entries: usize,
    pub(in crate::implementation) exit_region_capacity_entries: usize,
    pub(in crate::implementation) status_case_capacity_entries: usize,
}

impl HirPreResolveCapacity {
    pub(in crate::implementation) fn complete(self) -> Option<usize> {
        self.retained_upper.checked_add(self.scratch_upper)
    }

    #[cfg(test)]
    pub(in crate::implementation) fn phase_peaks(self) -> [usize; 8] {
        self.phase_peaks
    }
}

#[cfg(test)]
pub(in crate::implementation) fn hir_capacity_terms_for_test(
    program: &Program,
    source_bytes: usize,
) -> Result<(usize, usize, usize), Diagnostic> {
    let mut stack = [None; MAX_SEMANTIC_EXPRESSION_DEPTH + 1];
    let capacity = hir_pre_resolve_capacity(program, source_bytes, &mut stack)?;
    Ok((
        capacity.retained_upper,
        capacity.scratch_upper,
        capacity.cleanup_retained_upper,
    ))
}

pub(in crate::implementation) fn hir_pre_resolve_capacity<'a>(
    program: &'a Program,
    source_bytes: usize,
    stack: &mut [Option<(&'a crate::ast::Expr, usize, usize)>; MAX_SEMANTIC_EXPRESSION_DEPTH + 1],
) -> Result<HirPreResolveCapacity, Diagnostic> {
    let all_roots = source_functions(program).flat_map(|function| {
        function
            .requires
            .iter()
            .chain(std::iter::once(&function.body))
            .chain(&function.ensures)
    });
    let stats = scan_ast_capacity(all_roots, program, false, stack)?;
    let contract_index_digits = source_functions(program).fold(1usize, |digits, function| {
        digits
            .max(decimal_digits(function.requires.len().saturating_sub(1)))
            .max(decimal_digits(function.ensures.len().saturating_sub(1)))
            .max(decimal_digits(function.params.len().saturating_sub(1)))
    });
    let monomorphic_roots = source_functions(program)
        .filter(|function| function.type_parameters.is_empty())
        .flat_map(|function| {
            function
                .requires
                .iter()
                .chain(std::iter::once(&function.body))
                .chain(&function.ensures)
        });
    let reachable_generic_calls =
        scan_ast_capacity(monomorphic_roots, program, true, stack)?.generic_calls;
    let mut largest_template = AstCapacityStats::default();
    for function in source_functions(program) {
        if function.type_parameters.is_empty() {
            continue;
        }
        let roots = function
            .requires
            .iter()
            .chain(std::iter::once(&function.body))
            .chain(&function.ensures);
        let template = scan_ast_capacity(roots, program, false, stack)?;
        largest_template.nodes = largest_template.nodes.max(template.nodes);
        largest_template.cumulative_depth = largest_template
            .cumulative_depth
            .max(template.cumulative_depth);
        largest_template.max_depth = largest_template.max_depth.max(template.max_depth);
        largest_template.max_match_arms =
            largest_template.max_match_arms.max(template.max_match_arms);
        largest_template.max_indexed_children = largest_template
            .max_indexed_children
            .max(template.max_indexed_children);
        largest_template.depth_arm_product_sum = largest_template
            .depth_arm_product_sum
            .max(template.depth_arm_product_sum);
        largest_template.depth_width_product_sum = largest_template
            .depth_width_product_sum
            .max(template.depth_width_product_sum);
        largest_template.local_bindings =
            largest_template.local_bindings.max(template.local_bindings);
        largest_template.pattern_bindings = largest_template
            .pattern_bindings
            .max(template.pattern_bindings);
        largest_template.binding_name_bytes = largest_template
            .binding_name_bytes
            .max(template.binding_name_bytes);
        largest_template.binding_depth_sum = largest_template
            .binding_depth_sum
            .max(template.binding_depth_sum);
        largest_template.max_index_digits = largest_template
            .max_index_digits
            .max(template.max_index_digits);
    }
    let declarations = program
        .types
        .len()
        .checked_add(program.interfaces.len())
        .and_then(|value| value.checked_add(source_functions(program).count()))
        .and_then(|value| {
            program
                .interfaces
                .iter()
                .try_fold(value, |value, interface| {
                    value.checked_add(interface.imports.len())
                })
        })
        .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
    let nested_declarations = program
        .types
        .iter()
        .try_fold(declarations, |count, declaration| {
            let count = count.checked_add(declaration.type_parameters.len())?;
            match &declaration.kind {
                crate::ast::TypeDeclarationKind::Resource { lifecycles } => {
                    count.checked_add(lifecycles.len())
                }
                crate::ast::TypeDeclarationKind::Record { fields }
                | crate::ast::TypeDeclarationKind::Class { fields, .. } => {
                    count.checked_add(fields.len())
                }
                crate::ast::TypeDeclarationKind::Variant { cases } => cases
                    .iter()
                    .try_fold(count.checked_add(cases.len())?, |count, case| {
                        count.checked_add(case.fields.len())
                    }),
            }
        })
        .and_then(|count| {
            source_functions(program).try_fold(count, |count, function| {
                count
                    .checked_add(function.type_parameters.len())?
                    .checked_add(function.params.len())
            })
        })
        .and_then(|count| {
            program
                .interfaces
                .iter()
                .try_fold(count, |count, interface| {
                    interface.imports.iter().try_fold(count, |count, import| {
                        count.checked_add(import.params.len())
                    })
                })
        })
        .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
    // The longest indexed segment is `.arm.<i>.binding.<j>`; derive its digit
    // widths from the widest admitted authored node instead of assuming a
    // machine-usize textual width. Resolved
    // expression identity, value identity, cleanup inventory, cleanup plan,
    // and validation/index ownership can retain at most six path-bearing
    // copies. Fixed node/declaration terms cover enum/vector/BTree node bodies.
    let maximum_index_digits = stats.max_index_digits.max(contract_index_digits);
    let indexed_path_segment_bytes = 15usize
        .checked_add(
            maximum_index_digits
                .checked_mul(2)
                .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?,
        )
        .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
    let cleanup_node_inline = std::mem::size_of::<semaprax::cleanup::CleanupStorageSlot>()
        .checked_add(std::mem::size_of::<semaprax::cleanup::CleanupFlag>())
        .and_then(|bytes| {
            bytes.checked_add(std::mem::size_of::<semaprax::cleanup_plan::CleanupBlock>())
        })
        .and_then(|bytes| {
            bytes.checked_add(std::mem::size_of::<semaprax::cleanup_plan::CleanupEdge>())
        })
        .and_then(|bytes| {
            bytes.checked_add(std::mem::size_of::<semaprax::cleanup_plan::CleanupRegion>())
        })
        .and_then(|bytes| {
            bytes.checked_add(std::mem::size_of::<semaprax::cleanup_plan::ExitTarget>())
        })
        .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
    let retained_node_inline = std::mem::size_of::<ResolvedExpr>()
        .checked_add(cleanup_node_inline)
        .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
    let type_expansion = declaration_dag_expansion(program, reachable_generic_calls)?;
    let maximum_resource_leaves = type_expansion.maximum_resource_leaves;
    let disposal_frames = stats
        .max_depth
        .checked_mul(4)
        .and_then(|frames| {
            frames.checked_add(type_expansion.maximum_type_occurrences.checked_mul(2)?)
        })
        .and_then(|frames| frames.checked_add(16))
        .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
    let path_copy_upper = 1usize
        .checked_add(maximum_resource_leaves.min(5))
        .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
    let cleanup_path_copies = maximum_resource_leaves.min(5);
    let mut exact_expression_identity_bytes = 0usize;
    let mut cleanup_plan_uncovered_identity_bytes = 0usize;
    for function in source_functions(program) {
        let multiplicity = if function.type_parameters.is_empty() {
            1
        } else {
            reachable_generic_calls
                .checked_add(1)
                .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?
        };
        let (function_expression_bytes, function_plan_bytes) =
            cleanup_plan_variable_identity_bytes(function, program, cleanup_path_copies)?;
        exact_expression_identity_bytes = exact_expression_identity_bytes
            .checked_add(
                function_expression_bytes
                    .checked_mul(multiplicity)
                    .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?,
            )
            .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
        cleanup_plan_uncovered_identity_bytes = cleanup_plan_uncovered_identity_bytes
            .checked_add(
                function_plan_bytes
                    .checked_mul(multiplicity)
                    .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?,
            )
            .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
    }
    let node_bytes = stats
        .nodes
        .checked_mul(retained_node_inline)
        .and_then(|bytes| {
            bytes.checked_add(exact_expression_identity_bytes.checked_mul(path_copy_upper)?)
        })
        .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
    // Peak iterative resolver/validator/cleanup scratch. The declaration
    // census is a conservative upper for simultaneously live bindings/flags.
    // Branch continuations retain at most depth copies; Match retains one
    // FlowState per authored arm. Indexed child vectors/commit lists are
    // bounded by the widest authored node.
    let parameter_bindings = source_functions(program)
        .try_fold(0usize, |count, function| {
            count.checked_add(function.params.len())
        })
        .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
    let maximum_declared_fields = program
        .types
        .iter()
        .try_fold(0usize, |total, declaration| {
            let fields = match &declaration.kind {
                crate::ast::TypeDeclarationKind::Resource { .. } => 1,
                crate::ast::TypeDeclarationKind::Record { fields }
                | crate::ast::TypeDeclarationKind::Class { fields, .. } => fields.len(),
                crate::ast::TypeDeclarationKind::Variant { cases } => cases
                    .iter()
                    .try_fold(0usize, |count, case| count.checked_add(case.fields.len()))?,
            };
            total.checked_add(fields)
        })
        .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?
        .max(1);
    let binding_slots = parameter_bindings
        .checked_add(stats.local_bindings)
        .and_then(|width| width.checked_add(stats.pattern_bindings))
        .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
    // A binding of an aggregate can contribute one ownership/partial-place
    // fact per resource leaf. The declaration-field sum is a no-allocation
    // upper for an acyclic declaration graph, while the declaration verifier
    // rejects cycles before semantic admission.
    let live_state_width = binding_slots
        .checked_mul(maximum_resource_leaves)
        .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?
        .max(1);
    let branch_scope_copies = stats
        .depth_arm_product_sum
        .checked_add(stats.max_depth)
        .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
    let parameter_name_bytes = program
        .functions
        .iter()
        .try_fold(0usize, |bytes, function| {
            function.params.iter().try_fold(bytes, |bytes, parameter| {
                bytes.checked_add(parameter.name.len())
            })
        })
        .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
    let binding_identity_bytes = stats
        .binding_name_bytes
        .checked_add(parameter_name_bytes)
        .and_then(|bytes| {
            bytes.checked_add(
                stats
                    .binding_depth_sum
                    .checked_mul(indexed_path_segment_bytes)?,
            )
        })
        .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
    let scope_entry_inline =
        std::mem::size_of::<(crate::hir::ValueId, ResolvedType, OwnershipMode)>();
    let scope_payload_bytes = live_state_width
        .checked_mul(scope_entry_inline)
        .and_then(|bytes| {
            bytes.checked_add(binding_identity_bytes.checked_mul(maximum_declared_fields)?)
        })
        .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
    let indexed_result_bytes = std::mem::size_of::<ResolvedExpr>()
        .checked_add(std::mem::size_of::<ResolvedStatement>())
        .and_then(|bytes| {
            bytes.checked_add(std::mem::size_of::<crate::hir::ResolvedFieldInitializer>())
        })
        .and_then(|bytes| bytes.checked_add(CLEANUP_EVAL_RESULT_BYTES))
        .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
    let declaration_work_bytes = std::mem::size_of::<crate::hir::ResolvedTypeDeclaration>()
        .checked_add(std::mem::size_of::<crate::hir::ResolvedFieldDeclaration>())
        .and_then(|bytes| {
            bytes.checked_add(std::mem::size_of::<
                crate::hir::ResolvedVariantCaseDeclaration,
            >())
        })
        .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
    let source_phase = stats
        .max_depth
        .checked_mul(SOURCE_VERIFIER_FRAME_BYTES)
        .and_then(|bytes| bytes.checked_add(branch_scope_copies.checked_mul(scope_payload_bytes)?))
        .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
    let resolver_phase = stats
        .max_depth
        .checked_mul(HIR_RESOLVER_FRAME_BYTES)
        .and_then(|bytes| {
            bytes.checked_add(
                stats
                    .max_depth
                    .checked_mul(stats.max_indexed_children.max(1))?
                    .checked_mul(indexed_result_bytes)?,
            )
        })
        .and_then(|bytes| bytes.checked_add(branch_scope_copies.checked_mul(scope_payload_bytes)?))
        .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
    let validator_phase = stats
        .max_depth
        .checked_mul(HIR_VALIDATOR_FRAME_BYTES)
        .and_then(|bytes| bytes.checked_add(branch_scope_copies.checked_mul(scope_payload_bytes)?))
        .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
    let inventory_phase = maximum_resource_leaves
        .checked_mul(
            CLEANUP_INVENTORY_SHAPE_FRAME_BYTES
                + std::mem::size_of::<DeclarationId>()
                + std::mem::size_of::<semaprax::cleanup::FieldLivenessShape>(),
        )
        .and_then(|bytes| {
            bytes.checked_add(
                stats
                    .max_depth
                    .checked_mul(CLEANUP_INVENTORY_EXPR_FRAME_BYTES)?,
            )
        })
        .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
    let plan_entry_bytes = std::mem::size_of::<semaprax::cleanup_plan::CleanupBlock>()
        + std::mem::size_of::<semaprax::cleanup_plan::CleanupEdge>()
        + std::mem::size_of::<semaprax::cleanup_plan::CleanupRegion>()
        + std::mem::size_of::<semaprax::cleanup_plan::ExitTarget>()
        + std::mem::size_of::<semaprax::cleanup_plan::StatusSource>();
    let cleanup_phase = stats
        .max_depth
        .checked_mul(CLEANUP_LOWER_FRAME_BYTES)
        .and_then(|bytes| {
            bytes.checked_add(
                stats
                    .max_depth
                    .checked_mul(stats.max_indexed_children.max(1))?
                    .checked_mul(CLEANUP_EVAL_RESULT_BYTES)?,
            )
        })
        .and_then(|bytes| {
            bytes.checked_add(
                stats
                    .nodes
                    .checked_mul(maximum_resource_leaves)?
                    .checked_mul(plan_entry_bytes)?,
            )
        })
        .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
    let call_index_phase = stats
        .max_depth
        .checked_mul(CALL_INDEX_FRAME_BYTES)
        .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
    let closure_identity_entries = program
        .functions
        .len()
        .checked_add(program.interfaces.len())
        .and_then(|entries| entries.checked_add(nested_declarations))
        .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?
        .max(1);
    let closure_btree_entry_overhead = std::mem::size_of::<BTreeMap<String, usize>>();
    let closure_reference_headers = program
        .functions
        .len()
        .checked_mul(std::mem::size_of::<&ResolvedFunction>())
        .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
    let closure_phase = Some(closure_reference_headers)
        // The selected closure borrows functions from the live resolved
        // program. Only the sorted reference vector is retained; expression
        // and cleanup trees are neither cloned nor separately dropped.
        // by_id, state, depths, reached-imports, pending/visited/direct-call
        // sets, and contract traversal sets can overlap. One separately
        // allocated BTree node per identity plus the full authored source as
        // every key payload is conservative for each of the nine containers.
        .and_then(|bytes| {
            bytes.checked_add(
                closure_identity_entries
                    .checked_mul(
                        std::mem::size_of::<(String, usize)>()
                            .checked_add(closure_btree_entry_overhead)?,
                    )?
                    .checked_mul(9)?,
            )
        })
        .and_then(|bytes| bytes.checked_add(source_bytes.checked_mul(9)?))
        // DFS retains one ID and one indexed direct-call vector per depth.
        .and_then(|bytes| {
            bytes.checked_add(
                MAX_CALL_DEPTH.checked_mul(
                    std::mem::size_of::<SelectedClosureFrame>()
                        .checked_add(indexed_path_segment_bytes)?,
                )?,
            )
        })
        // While converting a direct-call set into the frame Vec, both
        // container backings and all ID strings coexist.
        .and_then(|bytes| {
            bytes.checked_add(
                closure_identity_entries.checked_mul(
                    std::mem::size_of::<String>()
                        .checked_add(std::mem::size_of::<DeclarationId>())?
                        .checked_add(closure_btree_entry_overhead)?,
                )?,
            )
        })
        .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
    let frame_machine_scratch = source_phase
        .max(resolver_phase)
        .max(validator_phase)
        .max(inventory_phase)
        .max(cleanup_phase)
        .max(call_index_phase)
        .max(closure_phase)
        .checked_add(
            nested_declarations
                .checked_mul(declaration_work_bytes)
                .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?,
        )
        .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
    let declaration_phase_overlap = nested_declarations
        .checked_mul(declaration_work_bytes)
        .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
    // Each distinct reachable specialization may clone and resolve one whole
    // template while the resolved template remains live. Count every call
    // site (even duplicate instances) against the largest template, which is
    // conservative without allocating a pre-resolution identity set.
    let specialization_bytes = largest_template
        .nodes
        .checked_mul(
            retained_node_inline
                .checked_mul(2)
                .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?,
        )
        .and_then(|bytes| {
            bytes.checked_add(
                largest_template
                    .cumulative_depth
                    .checked_mul(indexed_path_segment_bytes.checked_mul(2 * 6)?)?,
            )
        })
        .and_then(|bytes| {
            bytes.checked_add(
                largest_template
                    .max_depth
                    .checked_mul(HIR_RESOLVER_FRAME_BYTES)?,
            )
        })
        .and_then(|bytes| {
            bytes.checked_add(
                largest_template
                    .depth_arm_product_sum
                    .checked_add(largest_template.max_depth)?
                    .checked_mul(live_state_width)?
                    .checked_mul(scope_entry_inline)?,
            )
        })
        .and_then(|bytes| {
            bytes.checked_add(
                largest_template
                    .max_depth
                    .checked_mul(largest_template.max_indexed_children.max(1))?
                    .checked_mul(indexed_result_bytes)?,
            )
        })
        .and_then(|bytes| bytes.checked_mul(reachable_generic_calls))
        .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
    // TypeFacts layout keys recursively embed each child key. The fixed
    // per-occurrence syntax consists of four decimal lengths/separators plus
    let type_fact_layout_upper = crate::private_capacity_contract::type_facts_layout_upper(
        source_bytes,
        program.types.len(),
        type_expansion.maximum_type_occurrences,
    )
    .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
    let type_facts_frame_bytes = std::mem::size_of::<(
        ResolvedType,
        String,
        DeclarationId,
        crate::hir::DeclarationKind,
        usize,
    )>();
    let type_facts_scratch = type_expansion
        .maximum_type_occurrences
        .checked_mul(type_facts_frame_bytes)
        .and_then(|bytes| bytes.checked_add(type_fact_layout_upper.checked_mul(2)?))
        .and_then(|bytes| {
            bytes.checked_add(
                program
                    .types
                    .len()
                    .checked_mul(std::mem::size_of::<(String, crate::hir::TypeFacts)>())?,
            )
        })
        .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
    let declaration_index_upper = crate::private_capacity_contract::declaration_index_upper(
        source_bytes,
        program.types.len(),
        program.interfaces.len(),
        program.functions.len(),
        type_fact_layout_upper,
    )
    .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
    // The declaration-DAG pass also performs a typed source-flow census while
    // its exact temporary memo is still authorized. Unlike the former
    // `all roots * largest type * all nodes` product, every persistent shape,
    // flag and projection below is charged against the authored type of the
    // storage root that can create it. Plan places and exits are separate
    // copies because they coexist with the inventory and plan-slot shapes.
    let cleanup = type_expansion.cleanup_retained;
    let cleanup_function_instance_upper = program
        .functions
        .iter()
        .try_fold(0usize, |instances, function| {
            let multiplicity = if function.type_parameters.is_empty() {
                1
            } else {
                reachable_generic_calls.checked_add(1)?
            };
            instances.checked_add(multiplicity)
        })
        .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
    let flag_capacity_extra =
        retained_vec_capacity_extra(cleanup.leaves, cleanup_function_instance_upper)
            .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
    let shape_field_capacity_extra = retained_vec_capacity_extra(
        cleanup.shape_fields,
        cleanup.occurrences.min(cleanup.shape_fields),
    )
    .and_then(|extra| extra.checked_mul(2))
    .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
    let flag_projection_capacity_extra = retained_vec_capacity_extra(
        cleanup.projection_segments,
        cleanup.leaves.min(cleanup.projection_segments),
    )
    .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
    let place_projection_capacity_extra = retained_vec_capacity_extra(
        cleanup.place_projection_segments,
        cleanup.place_copies.min(cleanup.place_projection_segments),
    )
    .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
    let finalizer_projection_capacity_extra = retained_vec_capacity_extra(
        cleanup.finalizer_projection_segments,
        cleanup
            .finalizer_copies
            .min(cleanup.finalizer_projection_segments),
    )
    .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
    let finalizer_capacity_extra = retained_vec_capacity_extra(
        cleanup.finalizer_copies,
        cleanup.exit_events.min(cleanup.finalizer_copies),
    )
    .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
    let entry_state_capacity_extra = retained_vec_capacity_extra(
        cleanup.roots,
        cleanup_function_instance_upper.min(cleanup.roots),
    )
    .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
    let inventory_slot_capacity_extra = retained_vec_capacity_extra(
        cleanup.roots,
        cleanup_function_instance_upper.min(cleanup.roots),
    )
    .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
    let plan_slot_capacity_extra = retained_vec_capacity_extra(
        cleanup.roots,
        cleanup_function_instance_upper.min(cleanup.roots),
    )
    .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
    let inventory_entry_capacity_entries = cleanup
        .roots
        .checked_add(entry_state_capacity_extra)
        .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
    #[cfg(test)]
    let cleanup_parent_local_lifetime_upper = cleanup
        .parent_local_finalizer_copies
        .checked_mul(std::mem::size_of::<semaprax::cleanup_plan::FinalizeAction>())
        .and_then(|bytes| {
            bytes.checked_add(
                cleanup
                    .parent_local_finalizer_projection_segments
                    .checked_mul(std::mem::size_of::<DeclarationId>())?,
            )
        })
        .and_then(|bytes| bytes.checked_add(cleanup.parent_local_finalizer_lifecycle_ids))
        .and_then(|bytes| bytes.checked_add(cleanup.parent_local_finalizer_projection_ids))
        .and_then(|bytes| bytes.checked_add(cleanup.parent_local_finalizer_storage_bytes))
        .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
    #[cfg(test)]
    let cleanup_parent_local_projection_lifetime_upper = {
        let action_capacity_extra = retained_vec_capacity_extra(
            cleanup.parent_local_projection_finalizer_copies,
            cleanup
                .parent_local_projection_exit_groups
                .min(cleanup.parent_local_projection_finalizer_copies),
        )
        .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
        let projection_capacity_extra = retained_vec_capacity_extra(
            cleanup.parent_local_projection_finalizer_projection_segments,
            cleanup
                .parent_local_projection_finalizer_copies
                .min(cleanup.parent_local_projection_finalizer_projection_segments),
        )
        .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
        cleanup
            .parent_local_projection_finalizer_copies
            .checked_add(action_capacity_extra)
            .and_then(|entries| {
                entries.checked_mul(std::mem::size_of::<semaprax::cleanup_plan::FinalizeAction>())
            })
            .and_then(|bytes| {
                bytes.checked_add(
                    cleanup
                        .parent_local_projection_finalizer_projection_segments
                        .checked_add(projection_capacity_extra)?
                        .checked_mul(std::mem::size_of::<DeclarationId>())?,
                )
            })
            .and_then(|bytes| {
                bytes.checked_add(cleanup.parent_local_projection_finalizer_lifecycle_ids)
            })
            .and_then(|bytes| {
                bytes.checked_add(cleanup.parent_local_projection_finalizer_projection_ids)
            })
            .and_then(|bytes| {
                bytes.checked_add(cleanup.parent_local_projection_finalizer_storage_bytes)
            })
            .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?
    };
    #[cfg(test)]
    let cleanup_parent_local_update_prefix_lifetime_upper = {
        let action_capacity_extra = retained_vec_capacity_extra(
            cleanup.parent_local_update_prefix_finalizer_copies,
            cleanup
                .parent_local_update_prefix_exit_groups
                .min(cleanup.parent_local_update_prefix_finalizer_copies),
        )
        .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
        let projection_capacity_extra = retained_vec_capacity_extra(
            cleanup.parent_local_update_prefix_finalizer_projection_segments,
            cleanup
                .parent_local_update_prefix_finalizer_copies
                .min(cleanup.parent_local_update_prefix_finalizer_projection_segments),
        )
        .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
        cleanup
            .parent_local_update_prefix_finalizer_copies
            .checked_add(action_capacity_extra)
            .and_then(|entries| {
                entries.checked_mul(std::mem::size_of::<semaprax::cleanup_plan::FinalizeAction>())
            })
            .and_then(|bytes| {
                bytes.checked_add(
                    cleanup
                        .parent_local_update_prefix_finalizer_projection_segments
                        .checked_add(projection_capacity_extra)?
                        .checked_mul(std::mem::size_of::<DeclarationId>())?,
                )
            })
            .and_then(|bytes| {
                bytes.checked_add(cleanup.parent_local_update_prefix_finalizer_lifecycle_ids)
            })
            .and_then(|bytes| {
                bytes.checked_add(cleanup.parent_local_update_prefix_finalizer_projection_ids)
            })
            .and_then(|bytes| {
                bytes.checked_add(cleanup.parent_local_update_prefix_finalizer_storage_bytes)
            })
            .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?
    };
    let cleanup_retained_upper = cleanup
        .roots
        .checked_mul(
            std::mem::size_of::<semaprax::cleanup::CleanupStorageSlot>()
                + std::mem::size_of::<semaprax::cleanup_plan::CleanupSlot>(),
        )
        .and_then(|bytes| {
            bytes.checked_add(
                cleanup
                    .occurrences
                    .checked_mul(2)?
                    .checked_mul(std::mem::size_of::<semaprax::cleanup::FieldLivenessShape>())?,
            )
        })
        .and_then(|bytes| {
            bytes.checked_add(
                cleanup
                    .shape_fields
                    .checked_mul(2)?
                    .checked_mul(std::mem::size_of::<semaprax::cleanup::FieldLiveness>())?,
            )
        })
        .and_then(|bytes| bytes.checked_add(cleanup.shape_ids.checked_mul(2)?))
        .and_then(|bytes| {
            bytes.checked_add(
                cleanup
                    .leaves
                    .checked_mul(std::mem::size_of::<semaprax::cleanup::CleanupFlag>())?,
            )
        })
        .and_then(|bytes| bytes.checked_add(cleanup.lifecycle_ids))
        .and_then(|bytes| {
            bytes.checked_add(
                cleanup
                    .projection_segments
                    .checked_mul(std::mem::size_of::<DeclarationId>())?,
            )
        })
        .and_then(|bytes| bytes.checked_add(cleanup.projection_ids))
        .and_then(|bytes| {
            bytes.checked_add(
                cleanup
                    .place_copies
                    .checked_mul(std::mem::size_of::<semaprax::cleanup_plan::CleanupPlace>())?,
            )
        })
        .and_then(|bytes| {
            bytes.checked_add(
                cleanup
                    .place_projection_segments
                    .checked_mul(std::mem::size_of::<DeclarationId>())?,
            )
        })
        .and_then(|bytes| bytes.checked_add(cleanup.place_projection_ids))
        .and_then(|bytes| {
            bytes.checked_add(
                cleanup
                    .finalizer_copies
                    .checked_mul(std::mem::size_of::<semaprax::cleanup_plan::FinalizeAction>())?,
            )
        })
        .and_then(|bytes| {
            bytes.checked_add(
                cleanup
                    .finalizer_projection_segments
                    .checked_mul(std::mem::size_of::<DeclarationId>())?,
            )
        })
        .and_then(|bytes| bytes.checked_add(cleanup.finalizer_lifecycle_ids))
        .and_then(|bytes| bytes.checked_add(cleanup.finalizer_projection_ids))
        .and_then(|bytes| {
            bytes.checked_add(cleanup.staged_results.checked_mul(std::mem::size_of::<
                semaprax::cleanup_plan::StagedCopyResultSource,
            >())?)
        })
        .and_then(|bytes| {
            bytes.checked_add(
                cleanup
                    .variant_edges
                    .checked_mul(std::mem::size_of::<semaprax::cleanup_plan::EdgeCondition>())?,
            )
        })
        .and_then(|bytes| bytes.checked_add(cleanup.stage_identity_and_type_bytes))
        .and_then(|bytes| bytes.checked_add(cleanup.variant_identity_bytes))
        .and_then(|bytes| {
            bytes.checked_add(cleanup.call_arguments.checked_mul(std::mem::size_of::<
                semaprax::cleanup_plan::CallArgumentTransfer,
            >())?)
        })
        .and_then(|bytes| bytes.checked_add(cleanup.call_argument_owned_bytes))
        .and_then(|bytes| bytes.checked_add(cleanup.ordinary_slot_payload_bytes))
        .and_then(|bytes| bytes.checked_add(cleanup.ordinary_place_storage_bytes))
        .and_then(|bytes| bytes.checked_add(cleanup.ordinary_finalizer_storage_bytes))
        .and_then(|bytes| {
            bytes.checked_add(
                flag_capacity_extra
                    .checked_mul(std::mem::size_of::<semaprax::cleanup::CleanupFlag>())?,
            )
        })
        .and_then(|bytes| {
            bytes.checked_add(
                shape_field_capacity_extra
                    .checked_mul(std::mem::size_of::<semaprax::cleanup::FieldLiveness>())?,
            )
        })
        .and_then(|bytes| {
            bytes.checked_add(
                flag_projection_capacity_extra.checked_mul(std::mem::size_of::<DeclarationId>())?,
            )
        })
        .and_then(|bytes| {
            bytes.checked_add(
                place_projection_capacity_extra.checked_mul(std::mem::size_of::<DeclarationId>())?,
            )
        })
        .and_then(|bytes| {
            bytes.checked_add(
                finalizer_projection_capacity_extra
                    .checked_mul(std::mem::size_of::<DeclarationId>())?,
            )
        })
        .and_then(|bytes| {
            bytes.checked_add(
                finalizer_capacity_extra
                    .checked_mul(std::mem::size_of::<semaprax::cleanup_plan::FinalizeAction>())?,
            )
        })
        .and_then(|bytes| {
            bytes.checked_add(
                entry_state_capacity_extra
                    .checked_mul(std::mem::size_of::<semaprax::cleanup_plan::CleanupPlace>())?,
            )
        })
        .and_then(|bytes| {
            bytes.checked_add(
                inventory_slot_capacity_extra
                    .checked_mul(std::mem::size_of::<semaprax::cleanup::CleanupStorageSlot>())?,
            )
        })
        .and_then(|bytes| {
            bytes.checked_add(
                plan_slot_capacity_extra
                    .checked_mul(std::mem::size_of::<semaprax::cleanup_plan::CleanupSlot>())?,
            )
        })
        .and_then(|bytes| {
            bytes.checked_add(
                inventory_entry_capacity_entries
                    .checked_mul(std::mem::size_of::<semaprax::cleanup::CleanupStorageId>())?,
            )
        })
        .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
    let mut cleanup_structural_nodes = 0usize;
    let mut cleanup_structural_depth = 0usize;
    let mut cleanup_failure_events = 0usize;
    let mut cleanup_call_events = 0usize;
    let mut cleanup_boolean_branch_events = 0usize;
    let mut cleanup_contracts = 0usize;
    let mut cleanup_function_instances = 0usize;
    let cleanup_expression_identity_bytes = exact_expression_identity_bytes;
    for function in source_functions(program) {
        let function_stats = scan_ast_capacity(
            function
                .requires
                .iter()
                .chain(std::iter::once(&function.body))
                .chain(&function.ensures),
            program,
            false,
            stack,
        )?;
        let multiplicity = if function.type_parameters.is_empty() {
            1
        } else {
            reachable_generic_calls
                .checked_add(1)
                .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?
        };
        cleanup_structural_nodes = cleanup_structural_nodes
            .checked_add(
                function_stats
                    .nodes
                    .checked_mul(multiplicity)
                    .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?,
            )
            .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
        cleanup_structural_depth =
            cleanup_structural_depth.max(cleanup_function_region_depth(function, stack)?);
        cleanup_function_instances = cleanup_function_instances
            .checked_add(multiplicity)
            .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
        cleanup_contracts = cleanup_contracts
            .checked_add(
                function
                    .requires
                    .len()
                    .checked_add(function.ensures.len())
                    .and_then(|contracts| contracts.checked_mul(multiplicity))
                    .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?,
            )
            .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
        cleanup_failure_events = cleanup_failure_events
            .checked_add(
                cleanup_function_finalizer_events(function, stack)?
                    .checked_sub(1)
                    .and_then(|events| events.checked_mul(multiplicity))
                    .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?,
            )
            .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
        let mut function_call_events = 0usize;
        for root in function
            .requires
            .iter()
            .chain(std::iter::once(&function.body))
            .chain(&function.ensures)
        {
            function_call_events = function_call_events
                .checked_add(cleanup_expression_call_events(root, program, stack)?)
                .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
        }
        cleanup_call_events = cleanup_call_events
            .checked_add(
                function_call_events
                    .checked_mul(multiplicity)
                    .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?,
            )
            .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
        let mut function_boolean_branch_events = 0usize;
        for root in function
            .requires
            .iter()
            .chain(std::iter::once(&function.body))
            .chain(&function.ensures)
        {
            function_boolean_branch_events = function_boolean_branch_events
                .checked_add(cleanup_expression_boolean_branch_events(root, stack)?)
                .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
        }
        cleanup_boolean_branch_events = cleanup_boolean_branch_events
            .checked_add(
                function_boolean_branch_events
                    .checked_mul(multiplicity)
                    .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?,
            )
            .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
    }
    let cleanup_structural_upper = cleanup_structural_nodes
        .checked_mul(cleanup_node_inline)
        .and_then(|bytes| {
            bytes.checked_add(cleanup_expression_identity_bytes.checked_mul(cleanup_path_copies)?)
        })
        .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
    // Retained CleanupPlan container backing and identity payloads are
    // distinct from inventory/slot shapes above. Derive each family from the
    // source events that can create it. Four headers per logical entry covers
    // the current target's minimum-capacity floor for independently allocated
    // small Vecs as well as geometric growth.
    let transition_entries = cleanup
        .occurrences
        .checked_mul(2)
        // Every failing status path owns one SelectFailure transition. Only
        // ordinary calls additionally own CallCommit; checked arithmetic and
        // contract-false paths do not. Native Rust imports have neither.
        .and_then(|entries| entries.checked_add(cleanup_failure_events))
        .and_then(|entries| entries.checked_add(cleanup_call_events))
        .and_then(|entries| entries.checked_add(cleanup.call_arguments))
        .and_then(|entries| entries.checked_add(cleanup.staged_results))
        .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
    let cleanup_callee_identity_bytes = program
        .functions
        .iter()
        .map(|function| function.stable_id.len())
        .chain(program.interfaces.iter().flat_map(|interface| {
            interface
                .imports
                .iter()
                .map(|import| import.stable_id.len())
        }))
        .max()
        .unwrap_or(0);
    let expression_identity_fixed_bytes = "function-execution:"
        .len()
        .checked_add("semaprax.function-execution.v1:generic:".len())
        .and_then(|bytes| bytes.checked_add("declaration:".len()))
        .and_then(|bytes| bytes.checked_add(":expression:".len()))
        .and_then(|bytes| bytes.checked_add(decimal_digits(source_bytes).checked_mul(4)?))
        .and_then(|bytes| bytes.checked_add(8))
        .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
    let extra_block_headers = cleanup_structural_nodes
        .checked_mul(2)
        .and_then(|entries| entries.checked_add(cleanup_contracts))
        .and_then(|entries| entries.checked_add(cleanup_function_instances))
        .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
    let extra_edge_headers = cleanup_structural_nodes
        .checked_mul(3)
        .and_then(|entries| entries.checked_add(cleanup_contracts.checked_mul(2)?))
        .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
    let extra_region_headers = cleanup_contracts
        .checked_add(cleanup_function_instances)
        .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
    let extra_exit_headers = cleanup
        .exit_events
        .checked_sub(cleanup.exit_events.min(cleanup_structural_nodes))
        .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
    let block_entries = cleanup_structural_nodes
        .checked_add(extra_block_headers)
        .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
    let edge_entries = cleanup_structural_nodes
        .checked_add(extra_edge_headers)
        .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
    let region_entries = cleanup_structural_nodes
        .checked_add(extra_region_headers)
        .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
    let exit_entries = cleanup_structural_nodes
        .checked_add(extra_exit_headers)
        .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
    let block_capacity_extra =
        retained_vec_capacity_extra(block_entries, cleanup_function_instances.min(block_entries))
            .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
    let edge_capacity_extra =
        retained_vec_capacity_extra(edge_entries, cleanup_function_instances.min(edge_entries))
            .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
    let region_capacity_extra = retained_vec_capacity_extra(
        region_entries,
        cleanup_function_instances.min(region_entries),
    )
    .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
    let exit_capacity_extra =
        retained_vec_capacity_extra(exit_entries, cleanup_function_instances.min(exit_entries))
            .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
    let status_capacity_extra = retained_vec_capacity_extra(
        cleanup_failure_events,
        cleanup_function_instances.min(cleanup_failure_events),
    )
    .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
    let plan_expression_identity_copies = cleanup
        .occurrences
        .checked_mul(2)
        .and_then(|copies| copies.checked_add(cleanup_failure_events.checked_mul(5)?))
        .and_then(|copies| copies.checked_add(cleanup_call_events))
        .and_then(|copies| copies.checked_add(cleanup_boolean_branch_events.checked_mul(2)?))
        .and_then(|copies| copies.checked_add(cleanup_function_instances))
        .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
    let transition_capacity_entries = transition_entries
        .checked_add(
            retained_vec_capacity_extra(transition_entries, block_entries.min(transition_entries))
                .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?,
        )
        .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
    let branch_edge_entries = cleanup_structural_nodes
        .checked_mul(3)
        .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
    let branch_edge_capacity_entries = branch_edge_entries
        .checked_add(
            retained_vec_capacity_extra(
                branch_edge_entries,
                block_entries.min(branch_edge_entries),
            )
            .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?,
        )
        .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
    let region_slot_capacity_entries = cleanup
        .roots
        .checked_add(
            retained_vec_capacity_extra(cleanup.roots, region_entries.min(cleanup.roots))
                .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?,
        )
        .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
    let exit_region_entries = cleanup
        .exit_events
        .checked_mul(cleanup_structural_depth.max(1))
        .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
    let exit_region_capacity_entries = exit_region_entries
        .checked_add(
            retained_vec_capacity_extra(exit_region_entries, exit_entries.min(exit_region_entries))
                .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?,
        )
        .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
    let status_case_capacity_entries = cleanup_failure_events
        .checked_add(
            retained_vec_capacity_extra(cleanup_failure_events, cleanup_failure_events)
                .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?,
        )
        .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
    let cleanup_plan_structural_upper = transition_capacity_entries
        .checked_mul(std::mem::size_of::<semaprax::cleanup_plan::CleanupTransition>())
        .and_then(|bytes| {
            bytes.checked_add(
                branch_edge_capacity_entries
                    .checked_mul(std::mem::size_of::<semaprax::cleanup_plan::EdgeId>())?,
            )
        })
        .and_then(|bytes| {
            bytes.checked_add(
                extra_block_headers
                    .checked_mul(std::mem::size_of::<semaprax::cleanup_plan::CleanupBlock>())?,
            )
        })
        .and_then(|bytes| {
            bytes.checked_add(
                extra_edge_headers
                    .checked_mul(std::mem::size_of::<semaprax::cleanup_plan::CleanupEdge>())?,
            )
        })
        .and_then(|bytes| {
            bytes.checked_add(
                extra_region_headers
                    .checked_mul(std::mem::size_of::<semaprax::cleanup_plan::CleanupRegion>())?,
            )
        })
        .and_then(|bytes| {
            bytes.checked_add(
                extra_exit_headers
                    .checked_mul(std::mem::size_of::<semaprax::cleanup_plan::ExitTarget>())?,
            )
        })
        .and_then(|bytes| {
            bytes.checked_add(
                region_slot_capacity_entries
                    .checked_mul(std::mem::size_of::<semaprax::cleanup_plan::StorageId>())?,
            )
        })
        .and_then(|bytes| {
            bytes.checked_add(
                exit_region_capacity_entries
                    .checked_mul(std::mem::size_of::<semaprax::cleanup_plan::CleanupRegionId>())?,
            )
        })
        .and_then(|bytes| {
            bytes.checked_add(
                cleanup_failure_events
                    .checked_mul(std::mem::size_of::<semaprax::cleanup_plan::StatusSource>())?,
            )
        })
        .and_then(|bytes| {
            bytes.checked_add(
                status_case_capacity_entries
                    .checked_mul(std::mem::size_of::<semaprax::cleanup_plan::StatusCase>())?,
            )
        })
        // The full path payload has one source-derived copy in
        // cleanup_structural_upper. Each status/edge/continuation clone also
        // owns the fixed scoped-identity framing around that path.
        .and_then(|bytes| {
            bytes.checked_add(
                plan_expression_identity_copies.checked_mul(expression_identity_fixed_bytes)?,
            )
        })
        .and_then(|bytes| {
            bytes.checked_add(cleanup_failure_events.checked_mul(cleanup_callee_identity_bytes)?)
        })
        .and_then(|bytes| bytes.checked_add(cleanup_plan_uncovered_identity_bytes))
        .and_then(|bytes| {
            bytes.checked_add(
                block_capacity_extra
                    .checked_mul(std::mem::size_of::<semaprax::cleanup_plan::CleanupBlock>())?,
            )
        })
        .and_then(|bytes| {
            bytes.checked_add(
                edge_capacity_extra
                    .checked_mul(std::mem::size_of::<semaprax::cleanup_plan::CleanupEdge>())?,
            )
        })
        .and_then(|bytes| {
            bytes.checked_add(
                region_capacity_extra
                    .checked_mul(std::mem::size_of::<semaprax::cleanup_plan::CleanupRegion>())?,
            )
        })
        .and_then(|bytes| {
            bytes.checked_add(
                exit_capacity_extra
                    .checked_mul(std::mem::size_of::<semaprax::cleanup_plan::ExitTarget>())?,
            )
        })
        .and_then(|bytes| {
            bytes.checked_add(
                status_capacity_extra
                    .checked_mul(std::mem::size_of::<semaprax::cleanup_plan::StatusSource>())?,
            )
        })
        .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
    let cleanup_authority_upper = cleanup_retained_upper
        .checked_add(cleanup_structural_upper)
        .and_then(|bytes| bytes.checked_add(cleanup_plan_structural_upper))
        .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
    #[cfg(test)]
    let cleanup_proof =
        CleanupCapacityProofTerms {
            stats: cleanup,
            inventory_slot_capacity_entries: cleanup
                .roots
                .checked_add(inventory_slot_capacity_extra)
                .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?,
            inventory_flag_capacity_entries: cleanup
                .leaves
                .checked_add(flag_capacity_extra)
                .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?,
            inventory_entry_capacity_entries,
            plan_slot_capacity_entries: cleanup
                .roots
                .checked_add(plan_slot_capacity_extra)
                .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?,
            plan_entry_capacity_entries: cleanup
                .roots
                .checked_add(entry_state_capacity_extra)
                .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?,
            shape_field_capacity_entries: cleanup
                .shape_fields
                .checked_mul(2)
                .and_then(|entries| entries.checked_add(shape_field_capacity_extra))
                .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?,
            flag_projection_capacity_entries: cleanup
                .projection_segments
                .checked_add(flag_projection_capacity_extra)
                .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?,
            place_projection_capacity_entries: cleanup
                .place_projection_segments
                .checked_add(place_projection_capacity_extra)
                .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?,
            finalizer_projection_capacity_entries: cleanup
                .finalizer_projection_segments
                .checked_add(finalizer_projection_capacity_extra)
                .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?,
            finalizer_capacity_entries: cleanup
                .finalizer_copies
                .checked_add(finalizer_capacity_extra)
                .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?,
            block_capacity_entries: block_entries
                .checked_add(block_capacity_extra)
                .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?,
            edge_capacity_entries: edge_entries
                .checked_add(edge_capacity_extra)
                .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?,
            region_capacity_entries: region_entries
                .checked_add(region_capacity_extra)
                .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?,
            exit_capacity_entries: exit_entries
                .checked_add(exit_capacity_extra)
                .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?,
            status_capacity_entries: cleanup_failure_events
                .checked_add(status_capacity_extra)
                .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?,
            transition_capacity_entries,
            branch_edge_capacity_entries,
            region_slot_capacity_entries,
            exit_region_capacity_entries,
            status_case_capacity_entries,
        };
    let disposal_workspace_bytes = disposal_frames
        .checked_mul(std::mem::size_of::<ResolvedDisposeFrame>())
        .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
    let resolved_function_headers = program
        .functions
        .len()
        .checked_add(reachable_generic_calls)
        .and_then(|functions| functions.checked_mul(std::mem::size_of::<ResolvedFunction>()))
        .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
    let retained_upper = source_bytes
        .checked_mul(8)
        .and_then(|bytes| bytes.checked_add(node_bytes))
        .and_then(|bytes| bytes.checked_add(specialization_bytes))
        .and_then(|bytes| {
            bytes.checked_add(nested_declarations.checked_mul(declaration_work_bytes)?)
        })
        .and_then(|bytes| bytes.checked_add(declaration_index_upper))
        .and_then(|bytes| bytes.checked_add(cleanup_retained_upper))
        .and_then(|bytes| bytes.checked_add(cleanup_plan_structural_upper))
        .and_then(|bytes| bytes.checked_add(disposal_workspace_bytes))
        .and_then(|bytes| bytes.checked_add(resolved_function_headers))
        .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
    Ok(HirPreResolveCapacity {
        retained_upper,
        scratch_upper: frame_machine_scratch.max(type_facts_scratch),
        declaration_index_upper,
        cleanup_retained_upper,
        cleanup_authority_upper,
        cleanup_exit_events_upper: cleanup.exit_events,
        cleanup_fallback_roots: cleanup.fallback_roots,
        cleanup_call_argument_owned_upper: cleanup.call_argument_owned_bytes,
        cleanup_plan_structural_upper,
        #[cfg(test)]
        cleanup_parent_local_lifetime_upper,
        #[cfg(test)]
        cleanup_parent_local_projection_lifetime_upper,
        #[cfg(test)]
        cleanup_parent_local_update_prefix_lifetime_upper,
        #[cfg(test)]
        cleanup_proof,
        phase_peaks: [
            source_phase
                .checked_add(declaration_phase_overlap)
                .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?,
            resolver_phase
                .checked_add(declaration_phase_overlap)
                .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?,
            validator_phase
                .checked_add(declaration_phase_overlap)
                .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?,
            inventory_phase
                .checked_add(declaration_phase_overlap)
                .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?,
            cleanup_phase
                .checked_add(declaration_phase_overlap)
                .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?,
            call_index_phase
                .checked_add(declaration_phase_overlap)
                .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?,
            closure_phase
                .checked_add(declaration_phase_overlap)
                .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?,
            type_facts_scratch,
        ],
        disposal_frames,
    })
}
