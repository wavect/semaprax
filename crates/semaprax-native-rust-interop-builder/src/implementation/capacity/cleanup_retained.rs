//! The retained cleanup census over a resolved function.

use super::*;

pub(super) fn cleanup_retained_stats(
    program: &Program,
    declaration_facts: &[CleanupTypeFacts],
    node_capacity: usize,
    generic_instance_upper: usize,
) -> Result<CleanupRetainedStats, Diagnostic> {
    fn key_for_type(program: &Program, ty: &crate::ast::Type) -> CleanupTypeKey {
        match ty {
            crate::ast::Type::I64
            | crate::ast::Type::I32
            | crate::ast::Type::Char
            | crate::ast::Type::U8
            | crate::ast::Type::Usize
            | crate::ast::Type::F32
            | crate::ast::Type::F64
            | crate::ast::Type::Bool
            | crate::ast::Type::String
            | crate::ast::Type::Str
            | crate::ast::Type::ArrayU8(_)
            | crate::ast::Type::SliceU8 => CleanupTypeKey::Scalar,
            crate::ast::Type::Bytes => CleanupTypeKey::Unknown,
            crate::ast::Type::Named { name, .. } => {
                if let Some(index) = program
                    .types
                    .iter()
                    .position(|declaration| declaration.name == *name)
                {
                    CleanupTypeKey::Declaration(index)
                } else if matches!(name.as_str(), "Option" | "Result")
                    || program.types.iter().any(|declaration| {
                        declaration
                            .type_parameters
                            .iter()
                            .any(|parameter| parameter.name == *name)
                    })
                    || program.functions.iter().any(|function| {
                        function
                            .type_parameters
                            .iter()
                            .any(|parameter| parameter.name == *name)
                    })
                {
                    // Prelude Option/Result and admitted direct generic
                    // arguments are Copy-only at this boundary.
                    CleanupTypeKey::Scalar
                } else {
                    CleanupTypeKey::Unknown
                }
            }
        }
    }

    fn pattern_binding_key(
        program: &Program,
        pattern: &crate::ast::MatchPattern,
        name: &str,
    ) -> Result<Option<CleanupTypeKey>, Diagnostic> {
        Ok(match pattern {
            crate::ast::MatchPattern::Variant {
                type_name,
                case_name,
                fields,
                ..
            } => {
                let Some(declaration) = program
                    .types
                    .iter()
                    .find(|declaration| declaration.name == *type_name)
                else {
                    return Ok(None);
                };
                let crate::ast::TypeDeclarationKind::Variant { cases } = &declaration.kind else {
                    return Ok(None);
                };
                let Some(case) = cases.iter().find(|case| case.name == *case_name) else {
                    return Ok(None);
                };
                fields.iter().find_map(|binding| {
                    (binding.binding == name).then(|| {
                        case.fields
                            .iter()
                            .find(|field| field.name == binding.name)
                            .map(|field| key_for_type(program, &field.ty))
                    })?
                })
            }
            crate::ast::MatchPattern::Record {
                type_name, fields, ..
            } => {
                let Some(declaration) = program
                    .types
                    .iter()
                    .find(|declaration| declaration.name == *type_name)
                else {
                    return Ok(None);
                };
                let mut stack = [None; MAX_SEMANTIC_EXPRESSION_DEPTH + 1];
                let mut len = 1usize;
                stack[0] = Some((declaration, fields.as_slice(), 0usize, 1usize));
                let mut found = None;
                while len != 0 {
                    len -= 1;
                    let (declaration, fields, index, depth) = stack[len]
                        .take()
                        .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
                    let crate::ast::TypeDeclarationKind::Record {
                        fields: declarations,
                    } = &declaration.kind
                    else {
                        continue;
                    };
                    let Some(field) = fields.get(index) else {
                        continue;
                    };
                    let Some(declaration_field) = declarations
                        .iter()
                        .find(|candidate| candidate.name == field.name)
                    else {
                        continue;
                    };
                    match &field.pattern {
                        crate::ast::RecordMatchFieldPattern::Binding { name: binding, .. }
                            if binding == name =>
                        {
                            found = Some(key_for_type(program, &declaration_field.ty));
                            break;
                        }
                        crate::ast::RecordMatchFieldPattern::Record {
                            type_name,
                            fields: child_fields,
                            ..
                        } => {
                            let Some(child) = program
                                .types
                                .iter()
                                .find(|candidate| candidate.name == *type_name)
                            else {
                                continue;
                            };
                            let child_depth = depth.checked_add(1).ok_or_else(|| {
                                b109(
                                    "max_semantic_expression_depth",
                                    MAX_SEMANTIC_EXPRESSION_DEPTH,
                                )
                            })?;
                            if child_depth > MAX_SEMANTIC_EXPRESSION_DEPTH || len + 2 > stack.len()
                            {
                                return Err(b109(
                                    "max_semantic_expression_depth",
                                    MAX_SEMANTIC_EXPRESSION_DEPTH,
                                ));
                            }
                            stack[len] = Some((declaration, fields, index + 1, depth));
                            stack[len + 1] = Some((child, child_fields.as_slice(), 0, child_depth));
                            len += 2;
                        }
                        _ => {
                            stack[len] = Some((declaration, fields, index + 1, depth));
                            len += 1;
                        }
                    }
                }
                found
            }
            // Refutable Match v1: scalar patterns reference no named type.
            crate::ast::MatchPattern::Wildcard { .. }
            | crate::ast::MatchPattern::Literal { .. }
            | crate::ast::MatchPattern::Binding { .. } => None,
            crate::ast::MatchPattern::Or { alternatives, .. } => {
                let mut found = None;
                for alternative in alternatives {
                    found = pattern_binding_key(program, alternative, name)?;
                    if found.is_some() {
                        break;
                    }
                }
                found
            }
        })
    }

    fn facts_for_key(
        key: CleanupTypeKey,
        declaration_facts: &[CleanupTypeFacts],
        fallback: CleanupTypeFacts,
    ) -> CleanupTypeFacts {
        match key {
            CleanupTypeKey::Scalar => CleanupTypeFacts::default(),
            CleanupTypeKey::Declaration(index) => declaration_facts[index],
            CleanupTypeKey::Unknown => fallback,
        }
    }

    fn add_root(
        target: &mut CleanupRetainedStats,
        key: CleanupTypeKey,
        declaration_facts: &[CleanupTypeFacts],
        fallback: CleanupTypeFacts,
        storage_identity_bytes: usize,
        resolved_type_bytes: usize,
    ) -> Result<(), Diagnostic> {
        if matches!(key, CleanupTypeKey::Unknown) {
            target.fallback_roots = target
                .fallback_roots
                .checked_add(1)
                .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
        }
        let facts = facts_for_key(key, declaration_facts, fallback);
        if facts.leaves != 0 {
            target.ordinary_slot_payload_bytes = target
                .ordinary_slot_payload_bytes
                .checked_add(
                    storage_identity_bytes
                        .checked_add(resolved_type_bytes)
                        .and_then(|bytes| bytes.checked_mul(2))
                        .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?,
                )
                .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
            target.ordinary_place_storage_bytes = target
                .ordinary_place_storage_bytes
                .checked_add(
                    storage_identity_bytes
                        // Initialize, Transfer source/destination, and the
                        // region's raw StorageId each own the full identity.
                        .checked_mul(4)
                        .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?,
                )
                .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
        }
        target
            .add_root(facts)
            .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))
    }

    fn add_finalizer_upper(
        target: &mut CleanupRetainedStats,
        key: CleanupTypeKey,
        declaration_facts: &[CleanupTypeFacts],
        fallback: CleanupTypeFacts,
        exits_after_initialization: usize,
        storage_identity_bytes: usize,
    ) -> Result<(), Diagnostic> {
        let facts = facts_for_key(key, declaration_facts, fallback);
        target.finalizer_copies = target
            .finalizer_copies
            .checked_add(
                facts
                    .leaves
                    .checked_mul(exits_after_initialization)
                    .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?,
            )
            .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
        target.finalizer_projection_segments = target
            .finalizer_projection_segments
            .checked_add(
                facts
                    .projection_segments
                    .checked_mul(exits_after_initialization)
                    .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?,
            )
            .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
        target.finalizer_lifecycle_ids = target
            .finalizer_lifecycle_ids
            .checked_add(
                facts
                    .lifecycle_ids
                    .checked_mul(exits_after_initialization)
                    .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?,
            )
            .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
        target.finalizer_projection_ids = target
            .finalizer_projection_ids
            .checked_add(
                facts
                    .projection_ids
                    .checked_mul(exits_after_initialization)
                    .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?,
            )
            .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
        target.ordinary_finalizer_storage_bytes = target
            .ordinary_finalizer_storage_bytes
            .checked_add(
                storage_identity_bytes
                    .checked_mul(facts.leaves)
                    .and_then(|bytes| bytes.checked_mul(exits_after_initialization))
                    .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?,
            )
            .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
        Ok(())
    }

    fn add_parent_local_record_prefix(
        target: &mut CleanupRetainedStats,
        facts: CleanupTypeFacts,
        later_failure_events: usize,
        storage_identity_bytes: usize,
        field_identity_bytes: usize,
    ) -> Result<(), Diagnostic> {
        if facts.leaves == 0 || later_failure_events == 0 {
            return Ok(());
        }
        let finalizer_copies = facts
            .leaves
            .checked_mul(later_failure_events)
            .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
        let projection_segments = facts
            .projection_segments
            .checked_add(facts.leaves)
            .and_then(|segments| segments.checked_mul(later_failure_events))
            .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
        let lifecycle_ids = facts
            .lifecycle_ids
            .checked_mul(later_failure_events)
            .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
        let projection_ids = facts
            .projection_ids
            .checked_add(
                facts
                    .leaves
                    .checked_mul(field_identity_bytes)
                    .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?,
            )
            .and_then(|bytes| bytes.checked_mul(later_failure_events))
            .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
        let storage_bytes = storage_identity_bytes
            .checked_mul(finalizer_copies)
            .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;

        target.finalizer_copies = target
            .finalizer_copies
            .checked_add(finalizer_copies)
            .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
        target.finalizer_projection_segments = target
            .finalizer_projection_segments
            .checked_add(projection_segments)
            .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
        target.finalizer_lifecycle_ids = target
            .finalizer_lifecycle_ids
            .checked_add(lifecycle_ids)
            .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
        target.finalizer_projection_ids = target
            .finalizer_projection_ids
            .checked_add(projection_ids)
            .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
        target.ordinary_finalizer_storage_bytes = target
            .ordinary_finalizer_storage_bytes
            .checked_add(storage_bytes)
            .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;

        target.parent_local_partial_fields = target
            .parent_local_partial_fields
            .checked_add(1)
            .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
        target.parent_local_finalizer_copies = target
            .parent_local_finalizer_copies
            .checked_add(finalizer_copies)
            .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
        target.parent_local_finalizer_projection_segments = target
            .parent_local_finalizer_projection_segments
            .checked_add(projection_segments)
            .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
        target.parent_local_finalizer_lifecycle_ids = target
            .parent_local_finalizer_lifecycle_ids
            .checked_add(lifecycle_ids)
            .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
        target.parent_local_finalizer_projection_ids = target
            .parent_local_finalizer_projection_ids
            .checked_add(projection_ids)
            .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
        target.parent_local_finalizer_storage_bytes = target
            .parent_local_finalizer_storage_bytes
            .checked_add(storage_bytes)
            .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
        Ok(())
    }

    fn add_parent_local_update_prefix(
        target: &mut CleanupRetainedStats,
        facts: CleanupTypeFacts,
        later_failure_events: usize,
        storage_identity_bytes: usize,
        field_identity_bytes: usize,
    ) -> Result<(), Diagnostic> {
        if facts.leaves == 0 || later_failure_events == 0 {
            return Ok(());
        }
        let finalizer_copies = facts
            .leaves
            .checked_mul(later_failure_events)
            .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
        let projection_segments = facts
            .projection_segments
            .checked_add(facts.leaves)
            .and_then(|segments| segments.checked_mul(later_failure_events))
            .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
        let lifecycle_ids = facts
            .lifecycle_ids
            .checked_mul(later_failure_events)
            .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
        let projection_ids = facts
            .projection_ids
            .checked_add(
                facts
                    .leaves
                    .checked_mul(field_identity_bytes)
                    .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?,
            )
            .and_then(|bytes| bytes.checked_mul(later_failure_events))
            .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
        let storage_bytes = storage_identity_bytes
            .checked_mul(finalizer_copies)
            .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;

        target.finalizer_copies = target
            .finalizer_copies
            .checked_add(finalizer_copies)
            .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
        target.finalizer_projection_segments = target
            .finalizer_projection_segments
            .checked_add(projection_segments)
            .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
        target.finalizer_lifecycle_ids = target
            .finalizer_lifecycle_ids
            .checked_add(lifecycle_ids)
            .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
        target.finalizer_projection_ids = target
            .finalizer_projection_ids
            .checked_add(projection_ids)
            .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
        target.ordinary_finalizer_storage_bytes = target
            .ordinary_finalizer_storage_bytes
            .checked_add(storage_bytes)
            .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;

        target.parent_local_update_prefix_fields = target
            .parent_local_update_prefix_fields
            .checked_add(1)
            .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
        target.parent_local_update_prefix_exit_groups = target
            .parent_local_update_prefix_exit_groups
            .checked_add(later_failure_events)
            .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
        target.parent_local_update_prefix_finalizer_copies = target
            .parent_local_update_prefix_finalizer_copies
            .checked_add(finalizer_copies)
            .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
        target.parent_local_update_prefix_finalizer_projection_segments = target
            .parent_local_update_prefix_finalizer_projection_segments
            .checked_add(projection_segments)
            .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
        target.parent_local_update_prefix_finalizer_lifecycle_ids = target
            .parent_local_update_prefix_finalizer_lifecycle_ids
            .checked_add(lifecycle_ids)
            .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
        target.parent_local_update_prefix_finalizer_projection_ids = target
            .parent_local_update_prefix_finalizer_projection_ids
            .checked_add(projection_ids)
            .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
        target.parent_local_update_prefix_finalizer_storage_bytes = target
            .parent_local_update_prefix_finalizer_storage_bytes
            .checked_add(storage_bytes)
            .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
        Ok(())
    }

    fn add_parent_local_projection_residual(
        target: &mut CleanupRetainedStats,
        residual: CleanupTypeFacts,
        remaining_events: usize,
        storage_identity_bytes: usize,
    ) -> Result<(), Diagnostic> {
        if residual.leaves == 0 || remaining_events == 0 {
            return Ok(());
        }
        let finalizer_copies = residual
            .leaves
            .checked_mul(remaining_events)
            .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
        let projection_segments = residual
            .projection_segments
            .checked_mul(remaining_events)
            .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
        let lifecycle_ids = residual
            .lifecycle_ids
            .checked_mul(remaining_events)
            .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
        let projection_ids = residual
            .projection_ids
            .checked_mul(remaining_events)
            .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
        let storage_bytes = storage_identity_bytes
            .checked_mul(finalizer_copies)
            .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;

        target.finalizer_copies = target
            .finalizer_copies
            .checked_add(finalizer_copies)
            .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
        target.finalizer_projection_segments = target
            .finalizer_projection_segments
            .checked_add(projection_segments)
            .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
        target.finalizer_lifecycle_ids = target
            .finalizer_lifecycle_ids
            .checked_add(lifecycle_ids)
            .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
        target.finalizer_projection_ids = target
            .finalizer_projection_ids
            .checked_add(projection_ids)
            .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
        target.ordinary_finalizer_storage_bytes = target
            .ordinary_finalizer_storage_bytes
            .checked_add(storage_bytes)
            .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;

        target.parent_local_projection_epochs = target
            .parent_local_projection_epochs
            .checked_add(1)
            .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
        target.parent_local_projection_exit_groups = target
            .parent_local_projection_exit_groups
            .checked_add(remaining_events)
            .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
        target.parent_local_projection_finalizer_copies = target
            .parent_local_projection_finalizer_copies
            .checked_add(finalizer_copies)
            .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
        target.parent_local_projection_finalizer_projection_segments = target
            .parent_local_projection_finalizer_projection_segments
            .checked_add(projection_segments)
            .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
        target.parent_local_projection_finalizer_lifecycle_ids = target
            .parent_local_projection_finalizer_lifecycle_ids
            .checked_add(lifecycle_ids)
            .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
        target.parent_local_projection_finalizer_projection_ids = target
            .parent_local_projection_finalizer_projection_ids
            .checked_add(projection_ids)
            .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
        target.parent_local_projection_finalizer_storage_bytes = target
            .parent_local_projection_finalizer_storage_bytes
            .checked_add(storage_bytes)
            .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
        Ok(())
    }

    fn variable_key(
        program: &Program,
        function: &crate::ast::Function,
        name: &str,
        traversal: &[Option<(&crate::ast::Expr, usize, usize)>],
        stack_len: usize,
        results: &[CleanupTypeKey],
    ) -> Result<CleanupTypeKey, Diagnostic> {
        for (ancestor, next_child, result_start) in
            traversal[..stack_len].iter().rev().flatten().copied()
        {
            match &ancestor.kind {
                crate::ast::ExprKind::Block { statements, .. } => {
                    let active_child = ast_previous_child_path_index(next_child)
                        .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
                    let completed_statements =
                        ast_block_statement_index(statements, active_child).min(statements.len());
                    for index in (0..completed_statements).rev() {
                        let crate::ast::Statement::Let { name: binding, .. } = &statements[index]
                        else {
                            continue;
                        };
                        if binding == name {
                            let result_index = ast_block_statement_result_index(statements, index)
                                .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
                            return Ok(results
                                .get(result_start + result_index)
                                .copied()
                                .unwrap_or(CleanupTypeKey::Unknown));
                        }
                    }
                }
                crate::ast::ExprKind::Match { arms, .. } => {
                    let active_child = ast_previous_child_path_index(next_child)
                        .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
                    if let Some(arm_index) = ast_match_arm_index(arms, active_child) {
                        if let Some(key) = arms
                            .get(arm_index)
                            .map(|arm| pattern_binding_key(program, &arm.pattern, name))
                            .transpose()?
                            .flatten()
                        {
                            return Ok(key);
                        }
                    }
                }
                _ => {}
            }
        }
        Ok(function
            .params
            .iter()
            .rev()
            .find(|parameter| parameter.name == name)
            .map(|parameter| key_for_type(program, &parameter.ty))
            .unwrap_or(CleanupTypeKey::Unknown))
    }

    let fallback =
        declaration_facts
            .iter()
            .copied()
            .fold(CleanupTypeFacts::default(), |maximum, facts| {
                CleanupTypeFacts {
                    leaves: maximum.leaves.max(facts.leaves),
                    occurrences: maximum.occurrences.max(facts.occurrences),
                    shape_fields: maximum.shape_fields.max(facts.shape_fields),
                    projection_segments: maximum.projection_segments.max(facts.projection_segments),
                    shape_ids: maximum.shape_ids.max(facts.shape_ids),
                    lifecycle_ids: maximum.lifecycle_ids.max(facts.lifecycle_ids),
                    projection_ids: maximum.projection_ids.max(facts.projection_ids),
                }
            });
    // Staged Result/Option records retain compiler-owned identities even in a
    // program with no user resource declarations. Keep this list adjacent to
    // the source prelude contract; tests below bind its exact spellings.
    let prelude_identity_bytes = crate::private_capacity_contract::PRELUDE_CAPACITY_IDENTITIES
        .into_iter()
        .map(str::len)
        .max()
        .expect("private prelude identities are nonempty");
    let authored_identity_bytes = program.types.iter().fold(0usize, |maximum, declaration| {
        let maximum = maximum.max(declaration.stable_id.len());
        match &declaration.kind {
            crate::ast::TypeDeclarationKind::Resource { lifecycles } => {
                lifecycles.iter().fold(maximum, |maximum, lifecycle| {
                    maximum.max(lifecycle.stable_id.as_deref().map(str::len).unwrap_or(0))
                })
            }
            crate::ast::TypeDeclarationKind::Record { fields }
            | crate::ast::TypeDeclarationKind::Class { fields, .. } => fields
                .iter()
                .fold(maximum, |maximum, field| maximum.max(field.stable_id.len())),
            crate::ast::TypeDeclarationKind::Variant { cases } => {
                cases.iter().fold(maximum, |maximum, case| {
                    case.fields
                        .iter()
                        .fold(maximum.max(case.stable_id.len()), |maximum, field| {
                            maximum.max(field.stable_id.len())
                        })
                })
            }
        }
    });
    let maximum_declaration_identity_bytes = authored_identity_bytes.max(prelude_identity_bytes);
    let maximum_type_arguments = program
        .types
        .iter()
        .map(|declaration| declaration.type_parameters.len())
        .max()
        .unwrap_or(0)
        .max(2);
    let maximum_resolved_type_owned_bytes = maximum_declaration_identity_bytes
        .checked_add(
            maximum_type_arguments
                .checked_mul(std::mem::size_of::<ResolvedType>())
                .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?,
        )
        .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
    let mut total = CleanupRetainedStats::default();
    let mut traversal = [None; MAX_SEMANTIC_EXPRESSION_DEPTH + 1];
    let mut event_traversal = [None; MAX_SEMANTIC_EXPRESSION_DEPTH + 1];

    for function in source_functions(program) {
        let generic_instance_identity_len =
            generic_function_instance_identity_upper(program, function)
                .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
        let function_roots = function
            .requires
            .iter()
            .chain(std::iter::once(&function.body))
            .chain(&function.ensures);
        let function_node_total =
            scan_ast_capacity(function_roots, program, false, &mut traversal)?.nodes;
        let path_segment_bytes = 32usize
            .checked_add(decimal_digits(function_node_total))
            .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
        let value_storage_identity_bytes_for_path = |path_len: usize| {
            scoped_value_identity_upper(function, generic_instance_identity_len, path_len)
        };
        let expression_storage_identity_bytes_for_path = |path_len: usize| {
            scoped_expression_identity_upper(function, generic_instance_identity_len, path_len)
        };
        let type_bytes_for_key = |key: CleanupTypeKey| match key {
            CleanupTypeKey::Scalar => Some(0),
            CleanupTypeKey::Declaration(index) => program.types[index].stable_id.len().checked_add(
                program.types[index]
                    .type_parameters
                    .len()
                    .checked_mul(std::mem::size_of::<ResolvedType>())?,
            ),
            CleanupTypeKey::Unknown => Some(maximum_resolved_type_owned_bytes),
        };
        let function_exit_upper = cleanup_function_exit_events(function, &mut traversal)?;
        // These are exactly the source forms that can ask the lowerer for
        // an exit: operation failure, postfix residual, authored/update
        // scope, contract false/scope, and final success.
        let mut function_stats = CleanupRetainedStats {
            exit_events: function_exit_upper,
            ..CleanupRetainedStats::default()
        };
        let mut function_nodes = 0usize;
        let mut owned_parameters = 0usize;
        let mut has_try = false;
        for (parameter_index, parameter) in function.params.iter().enumerate() {
            if parameter.mode == crate::ast::ParamMode::Own {
                let key = key_for_type(program, &parameter.ty);
                let storage_identity_bytes =
                    value_storage_identity_bytes_for_path(decimal_digits(parameter_index))
                        .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
                add_root(
                    &mut function_stats,
                    key,
                    declaration_facts,
                    fallback,
                    storage_identity_bytes,
                    type_bytes_for_key(key)
                        .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?,
                )?;
                add_finalizer_upper(
                    &mut function_stats,
                    key,
                    declaration_facts,
                    fallback,
                    cleanup_parameter_finalizer_events(
                        function,
                        &parameter.name,
                        program,
                        &mut event_traversal,
                    )?,
                    storage_identity_bytes,
                )?;
                function_stats.ordinary_place_storage_bytes = function_stats
                    .ordinary_place_storage_bytes
                    .checked_add(storage_identity_bytes)
                    .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
                owned_parameters = owned_parameters
                    .checked_add(1)
                    .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
            }
        }

        let roots = function
            .requires
            .iter()
            .chain(std::iter::once(&function.body))
            .chain(&function.ensures);
        let mut traversal_path_lengths = [0usize; MAX_SEMANTIC_EXPRESSION_DEPTH + 1];
        for (root_index, root) in roots.enumerate() {
            let mut stack_len = 1usize;
            traversal[0] = Some((root, 0usize, 0usize));
            traversal_path_lengths[0] = ast_root_identity_path_len(function, root_index);
            let mut results = Vec::<CleanupTypeKey>::with_capacity(node_capacity);
            if results.capacity() != node_capacity {
                return Err(b109("max_builder_bytes", MAX_BUILDER_BYTES));
            }
            while stack_len != 0 {
                stack_len -= 1;
                let expression_path_len = traversal_path_lengths[stack_len];
                let (expression, next_child, result_start) = traversal[stack_len]
                    .take()
                    .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
                if next_child != 0 {
                    if let crate::ast::ExprKind::Block { statements, .. } = &expression.kind {
                        let previous = ast_previous_child_path_index(next_child)
                            .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
                        let statement_index = ast_block_statement_index(statements, previous);
                        if let Some(crate::ast::Statement::Let { name, .. }) =
                            statements.get(statement_index)
                        {
                            let key = results
                                .last()
                                .copied()
                                .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
                            let storage_identity_bytes = value_storage_identity_bytes_for_path(
                                expression_path_len
                                    .checked_add(".s".len())
                                    .and_then(|bytes| {
                                        bytes.checked_add(decimal_digits(statement_index))
                                    })
                                    .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?,
                            )
                            .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
                            add_root(
                                &mut function_stats,
                                key,
                                declaration_facts,
                                fallback,
                                storage_identity_bytes,
                                type_bytes_for_key(key)
                                    .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?,
                            )?;
                            if facts_for_key(key, declaration_facts, fallback).leaves != 0 {
                                function_stats.parent_local_epochs = function_stats
                                    .parent_local_epochs
                                    .checked_add(1)
                                    .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
                            }
                            let remaining = cleanup_block_binding_finalizer_events(
                                function,
                                expression,
                                next_child,
                                name,
                                program,
                                &mut event_traversal,
                            )?;
                            add_finalizer_upper(
                                &mut function_stats,
                                key,
                                declaration_facts,
                                fallback,
                                remaining,
                                storage_identity_bytes,
                            )?;
                        }
                    }
                }
                if next_child == 0 {
                    function_nodes = function_nodes
                        .checked_add(1)
                        .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
                    match &expression.kind {
                        crate::ast::ExprKind::Call { args, .. } => {
                            if let crate::ast::ExprKind::Call { name, .. } = &expression.kind {
                                if let Some(candidate) = program
                                    .functions
                                    .iter()
                                    .find(|candidate| candidate.name == *name)
                                {
                                    for (argument_index, parameter) in
                                        candidate.params.iter().take(args.len()).enumerate()
                                    {
                                        let key = key_for_type(program, &parameter.ty);
                                        if parameter.mode != crate::ast::ParamMode::Own
                                            || facts_for_key(key, declaration_facts, fallback)
                                                .leaves
                                                == 0
                                        {
                                            continue;
                                        }
                                        // The caller retains a distinct
                                        // CallArgument epoch in addition to
                                        // the argument expression temporary.
                                        add_root(
                                            &mut function_stats,
                                            key,
                                            declaration_facts,
                                            fallback,
                                            0,
                                            0,
                                        )?;
                                        let later_argument_events = args[argument_index + 1..]
                                            .iter()
                                            .try_fold(0usize, |events, argument| {
                                                events
                                                    .checked_add(cleanup_expression_failure_events(
                                                        argument,
                                                        &mut event_traversal,
                                                    )?)
                                                    .ok_or_else(|| {
                                                        b109("max_builder_bytes", MAX_BUILDER_BYTES)
                                                    })
                                            })?;
                                        let argument_identity_bytes = function
                                            .stable_id
                                            .len()
                                            .checked_add(
                                                stack_len
                                                    .checked_add(2)
                                                    .and_then(|depth| {
                                                        depth.checked_mul(path_segment_bytes)
                                                    })
                                                    .ok_or_else(|| {
                                                        b109("max_builder_bytes", MAX_BUILDER_BYTES)
                                                    })?,
                                            )
                                            .and_then(|bytes| bytes.checked_mul(2))
                                            .ok_or_else(|| {
                                                b109("max_builder_bytes", MAX_BUILDER_BYTES)
                                            })?;
                                        let argument_facts =
                                            facts_for_key(key, declaration_facts, fallback);
                                        // Four paired CallArgument StorageId
                                        // copies coexist (slot, region,
                                        // Transfer destination, CallCommit
                                        // source). Transfer::at and
                                        // CallCommit::call add two single
                                        // expression IDs, equal to one more
                                        // paired upper.
                                        let fixed_storage_copies = argument_identity_bytes
                                            .checked_mul(5)
                                            .and_then(|bytes| {
                                                bytes.checked_add(maximum_resolved_type_owned_bytes)
                                            })
                                            .ok_or_else(|| {
                                                b109("max_builder_bytes", MAX_BUILDER_BYTES)
                                            })?;
                                        let failure_storage_copies = argument_identity_bytes
                                            .checked_mul(argument_facts.leaves)
                                            .and_then(|bytes| {
                                                bytes.checked_mul(later_argument_events)
                                            })
                                            .ok_or_else(|| {
                                                b109("max_builder_bytes", MAX_BUILDER_BYTES)
                                            })?;
                                        function_stats.call_argument_owned_bytes = function_stats
                                            .call_argument_owned_bytes
                                            .checked_add(fixed_storage_copies)
                                            .and_then(|bytes| {
                                                bytes.checked_add(failure_storage_copies)
                                            })
                                            .ok_or_else(|| {
                                                b109("max_builder_bytes", MAX_BUILDER_BYTES)
                                            })?;
                                        add_finalizer_upper(
                                            &mut function_stats,
                                            key,
                                            declaration_facts,
                                            fallback,
                                            later_argument_events,
                                            0,
                                        )?;
                                        function_stats.call_arguments = function_stats
                                            .call_arguments
                                            .checked_add(1)
                                            .ok_or_else(|| {
                                                b109("max_builder_bytes", MAX_BUILDER_BYTES)
                                            })?;
                                        function_stats.parent_local_epochs = function_stats
                                            .parent_local_epochs
                                            .checked_add(1)
                                            .ok_or_else(|| {
                                                b109("max_builder_bytes", MAX_BUILDER_BYTES)
                                            })?;
                                    }
                                }
                            }
                        }
                        crate::ast::ExprKind::Match { arms, .. } => {
                            function_stats.variant_edges =
                                function_stats
                                    .variant_edges
                                    .checked_add(arms.len().checked_mul(2).ok_or_else(|| {
                                        b109("max_builder_bytes", MAX_BUILDER_BYTES)
                                    })?)
                                    .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
                        }
                        crate::ast::ExprKind::Try { .. } => {
                            has_try = true;
                            function_stats.staged_results = function_stats
                                .staged_results
                                .checked_add(1)
                                .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
                            function_stats.variant_edges = function_stats
                                .variant_edges
                                .checked_add(2)
                                .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
                        }
                        _ => {}
                    }
                }
                let mut child_cursor = next_child;
                if let Some((child_index, child)) = ast_child(expression, &mut child_cursor) {
                    if stack_len + 2 > traversal.len() {
                        return Err(b109(
                            "max_semantic_expression_depth",
                            MAX_SEMANTIC_EXPRESSION_DEPTH,
                        ));
                    }
                    traversal[stack_len] = Some((expression, child_cursor, result_start));
                    traversal_path_lengths[stack_len] = expression_path_len;
                    traversal[stack_len + 1] = Some((child, 0, results.len()));
                    traversal_path_lengths[stack_len + 1] = expression_path_len
                        .checked_add(ast_child_identity_path_increment(
                            expression,
                            child_index,
                            program,
                        ))
                        .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
                    stack_len += 2;
                    continue;
                }

                let children = &results[result_start..];
                let key = match &expression.kind {
                    crate::ast::ExprKind::Int(_)
                    | crate::ast::ExprKind::Int32(_)
                    | crate::ast::ExprKind::Char(_)
                    | crate::ast::ExprKind::Uint8(_)
                    | crate::ast::ExprKind::Usize(_)
                    | crate::ast::ExprKind::ArrayU8(_)
                    | crate::ast::ExprKind::RepeatArrayU8 { .. }
                    | crate::ast::ExprKind::Float32(_)
                    | crate::ast::ExprKind::Float64(_)
                    | crate::ast::ExprKind::Bool(_)
                    | crate::ast::ExprKind::String(_) => CleanupTypeKey::Scalar,
                    crate::ast::ExprKind::Var(name) => {
                        variable_key(program, function, name, &traversal, stack_len, &results)?
                    }
                    crate::ast::ExprKind::Call { name, .. } => program
                        .functions
                        .iter()
                        .find(|candidate| candidate.name == *name)
                        .map(|candidate| key_for_type(program, &candidate.return_type))
                        .unwrap_or(CleanupTypeKey::Scalar),
                    crate::ast::ExprKind::MethodCall { method, .. } => program
                        .types
                        .iter()
                        .find_map(|declaration| match &declaration.kind {
                            crate::ast::TypeDeclarationKind::Class { methods, .. } => {
                                methods.iter().find(|candidate| candidate.name == *method)
                            }
                            _ => None,
                        })
                        .map(|candidate| key_for_type(program, &candidate.return_type))
                        .unwrap_or(CleanupTypeKey::Scalar),
                    crate::ast::ExprKind::SuperMethod { method, .. } => program
                        .types
                        .iter()
                        .find_map(|declaration| match &declaration.kind {
                            crate::ast::TypeDeclarationKind::Class { methods, .. } => {
                                methods.iter().find(|candidate| candidate.name == *method)
                            }
                            _ => None,
                        })
                        .map(|candidate| key_for_type(program, &candidate.return_type))
                        .unwrap_or(CleanupTypeKey::Scalar),
                    crate::ast::ExprKind::Unary { .. } | crate::ast::ExprKind::Binary { .. } => {
                        CleanupTypeKey::Scalar
                    }
                    crate::ast::ExprKind::Block { .. } => {
                        children.last().copied().unwrap_or(CleanupTypeKey::Scalar)
                    }
                    crate::ast::ExprKind::If { .. } => {
                        children.get(1).copied().unwrap_or(CleanupTypeKey::Unknown)
                    }
                    crate::ast::ExprKind::ConstructRecord {
                        type_name, fields, ..
                    } => {
                        let declaration_index = program
                            .types
                            .iter()
                            .position(|declaration| declaration.name == *type_name);
                        if let Some(declaration_index) = declaration_index {
                            let declaration = &program.types[declaration_index];
                            let declared_fields = match &declaration.kind {
                                crate::ast::TypeDeclarationKind::Record { fields }
                                | crate::ast::TypeDeclarationKind::Class { fields, .. } => fields,
                                _ => {
                                    return Err(b109("max_builder_bytes", MAX_BUILDER_BYTES));
                                }
                            };
                            let storage_identity_bytes =
                                expression_storage_identity_bytes_for_path(expression_path_len)
                                    .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
                            for (field_index, initializer) in fields.iter().enumerate() {
                                let field_key = children
                                    .get(field_index)
                                    .copied()
                                    .unwrap_or(CleanupTypeKey::Unknown);
                                let facts = facts_for_key(field_key, declaration_facts, fallback);
                                if facts.leaves == 0 {
                                    continue;
                                }
                                let later_failure_events = fields[field_index + 1..]
                                    .iter()
                                    .try_fold(0usize, |events, later| {
                                        events
                                            .checked_add(cleanup_expression_failure_events(
                                                &later.value,
                                                &mut event_traversal,
                                            )?)
                                            .ok_or_else(|| {
                                                b109("max_builder_bytes", MAX_BUILDER_BYTES)
                                            })
                                    })?;
                                let field_identity_bytes = declared_fields
                                    .iter()
                                    .find(|field| field.name == initializer.name)
                                    .map(|field| field.stable_id.len())
                                    .unwrap_or(maximum_declaration_identity_bytes);
                                add_parent_local_record_prefix(
                                    &mut function_stats,
                                    facts,
                                    later_failure_events,
                                    storage_identity_bytes,
                                    field_identity_bytes,
                                )?;
                            }
                            CleanupTypeKey::Declaration(declaration_index)
                        } else {
                            CleanupTypeKey::Unknown
                        }
                    }
                    crate::ast::ExprKind::ConstructVariant { type_name, .. } => program
                        .types
                        .iter()
                        .position(|declaration| declaration.name == *type_name)
                        .map(CleanupTypeKey::Declaration)
                        .unwrap_or_else(|| {
                            if matches!(type_name.as_str(), "Option" | "Result") {
                                CleanupTypeKey::Scalar
                            } else {
                                CleanupTypeKey::Unknown
                            }
                        }),
                    crate::ast::ExprKind::Match { arms, .. } => {
                        if let Some(arm) = arms.first() {
                            if let crate::ast::ExprKind::Var(name) = &arm.value.kind {
                                pattern_binding_key(program, &arm.pattern, name)?
                                    .unwrap_or(CleanupTypeKey::Unknown)
                            } else {
                                ast_match_arm_value_result_index(arms, 0)
                                    .and_then(|index| children.get(index).copied())
                                    .unwrap_or(CleanupTypeKey::Unknown)
                            }
                        } else {
                            CleanupTypeKey::Unknown
                        }
                    }
                    crate::ast::ExprKind::Try { .. } => CleanupTypeKey::Scalar,
                    crate::ast::ExprKind::UpdateRecord { fields, .. } => {
                        let base = children.first().copied().unwrap_or(CleanupTypeKey::Unknown);
                        let destination_storage_identity_bytes =
                            expression_storage_identity_bytes_for_path(expression_path_len)
                                .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
                        for (field_index, initializer) in fields.iter().enumerate() {
                            let replacement_key = children
                                .get(field_index + 1)
                                .copied()
                                .unwrap_or(CleanupTypeKey::Unknown);
                            let replacement_facts =
                                facts_for_key(replacement_key, declaration_facts, fallback);
                            if replacement_facts.leaves == 0 {
                                continue;
                            }
                            let later_failure_events = fields[field_index + 1..].iter().try_fold(
                                0usize,
                                |events, later| {
                                    events
                                        .checked_add(cleanup_expression_failure_events(
                                            &later.value,
                                            &mut event_traversal,
                                        )?)
                                        .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))
                                },
                            )?;
                            let field_identity_bytes = match base {
                                CleanupTypeKey::Declaration(index) => {
                                    match &program.types[index].kind {
                                        crate::ast::TypeDeclarationKind::Record { fields }
                                        | crate::ast::TypeDeclarationKind::Class {
                                            fields, ..
                                        } => fields
                                            .iter()
                                            .find(|field| field.name == initializer.name)
                                            .map(|field| field.stable_id.len())
                                            .unwrap_or(maximum_declaration_identity_bytes),
                                        _ => maximum_declaration_identity_bytes,
                                    }
                                }
                                _ => maximum_declaration_identity_bytes,
                            };
                            add_parent_local_update_prefix(
                                &mut function_stats,
                                replacement_facts,
                                later_failure_events,
                                destination_storage_identity_bytes,
                                field_identity_bytes,
                            )?;
                        }
                        let storage_identity_bytes = expression_storage_identity_bytes_for_path(
                            expression_path_len
                                .checked_add(".base".len())
                                .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?,
                        )
                        .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
                        add_root(
                            &mut function_stats,
                            base,
                            declaration_facts,
                            fallback,
                            storage_identity_bytes,
                            type_bytes_for_key(base)
                                .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?,
                        )?;
                        let staged_base_exits = fields.iter().try_fold(
                            1usize,
                            |events, field| -> Result<usize, Diagnostic> {
                                events
                                    .checked_add(cleanup_expression_failure_events(
                                        &field.value,
                                        &mut event_traversal,
                                    )?)
                                    .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))
                            },
                        )?;
                        add_finalizer_upper(
                            &mut function_stats,
                            base,
                            declaration_facts,
                            fallback,
                            staged_base_exits,
                            storage_identity_bytes,
                        )?;
                        if facts_for_key(base, declaration_facts, fallback).leaves != 0 {
                            function_stats.parent_local_epochs = function_stats
                                .parent_local_epochs
                                .checked_add(1)
                                .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
                        }
                        base
                    }
                    crate::ast::ExprKind::Project { base, field, .. } => {
                        let base_key = children.first().copied().unwrap_or(CleanupTypeKey::Unknown);
                        let selected = match base_key {
                            CleanupTypeKey::Declaration(index) => {
                                let declaration = &program.types[index];
                                match &declaration.kind {
                                    crate::ast::TypeDeclarationKind::Record { fields }
                                    | crate::ast::TypeDeclarationKind::Class { fields, .. } => {
                                        fields
                                            .iter()
                                            .find(|candidate| candidate.name == *field)
                                            .map(|candidate| key_for_type(program, &candidate.ty))
                                    }
                                    _ => None,
                                }
                            }
                            _ => None,
                        }
                        .unwrap_or(CleanupTypeKey::Unknown);
                        if !matches!(base.kind, crate::ast::ExprKind::Var(_)) {
                            let base_facts = facts_for_key(base_key, declaration_facts, fallback);
                            let residual = if let CleanupTypeKey::Declaration(index) = base_key {
                                let selected_facts =
                                    facts_for_key(selected, declaration_facts, fallback);
                                let field_identity_bytes = match &program.types[index].kind {
                                    crate::ast::TypeDeclarationKind::Record { fields }
                                    | crate::ast::TypeDeclarationKind::Class { fields, .. } => {
                                        fields
                                            .iter()
                                            .find(|candidate| candidate.name == *field)
                                            .map(|candidate| candidate.stable_id.len())
                                            .unwrap_or(maximum_declaration_identity_bytes)
                                    }
                                    _ => maximum_declaration_identity_bytes,
                                };
                                let selected_projection_segments = selected_facts
                                    .projection_segments
                                    .checked_add(selected_facts.leaves)
                                    .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
                                let selected_projection_ids = selected_facts
                                    .projection_ids
                                    .checked_add(
                                        selected_facts
                                            .leaves
                                            .checked_mul(field_identity_bytes)
                                            .ok_or_else(|| {
                                                b109("max_builder_bytes", MAX_BUILDER_BYTES)
                                            })?,
                                    )
                                    .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
                                if selected_facts.leaves <= base_facts.leaves
                                    && selected_projection_segments
                                        <= base_facts.projection_segments
                                    && selected_facts.lifecycle_ids <= base_facts.lifecycle_ids
                                    && selected_projection_ids <= base_facts.projection_ids
                                {
                                    CleanupTypeFacts {
                                        leaves: base_facts.leaves - selected_facts.leaves,
                                        projection_segments: base_facts.projection_segments
                                            - selected_projection_segments,
                                        lifecycle_ids: base_facts.lifecycle_ids
                                            - selected_facts.lifecycle_ids,
                                        projection_ids: base_facts.projection_ids
                                            - selected_projection_ids,
                                        ..CleanupTypeFacts::default()
                                    }
                                } else {
                                    // Generic field substitution is not yet
                                    // materialized in this source census.
                                    // Keeping the complete base is the exact
                                    // admitted fallback, never a subtraction
                                    // from unrelated declaration facts.
                                    base_facts
                                }
                            } else {
                                // A valid unresolved generic projection may
                                // still instantiate to the maximum admitted
                                // resource aggregate. Retain the whole fallback
                                // rather than assuming which field transferred.
                                base_facts
                            };
                            let base_path_len = expression_path_len
                                .checked_add(ast_child_identity_path_increment(
                                    expression, 0, program,
                                ))
                                .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
                            let storage_identity_bytes =
                                expression_storage_identity_bytes_for_path(base_path_len)
                                    .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
                            let remaining_events = cleanup_parent_local_remaining_finalizer_events(
                                function,
                                root,
                                &traversal,
                                stack_len,
                                &mut event_traversal,
                            )?;
                            add_parent_local_projection_residual(
                                &mut function_stats,
                                residual,
                                remaining_events,
                                storage_identity_bytes,
                            )?;
                        }
                        selected
                    }
                };
                if !matches!(expression.kind, crate::ast::ExprKind::Var(_)) {
                    let storage_identity_bytes =
                        expression_storage_identity_bytes_for_path(expression_path_len)
                            .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
                    add_root(
                        &mut function_stats,
                        key,
                        declaration_facts,
                        fallback,
                        storage_identity_bytes,
                        type_bytes_for_key(key)
                            .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?,
                    )?;
                    if facts_for_key(key, declaration_facts, fallback).leaves != 0 {
                        function_stats.parent_local_epochs = function_stats
                            .parent_local_epochs
                            .checked_add(1)
                            .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
                        function_stats.parent_local_zero_lifetime_transfers = function_stats
                            .parent_local_zero_lifetime_transfers
                            .checked_add(1)
                            .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
                    }
                }
                results.truncate(result_start);
                results.push(key);
            }
            if results.len() != 1 {
                return Err(b109("max_builder_bytes", MAX_BUILDER_BYTES));
            }
        }
        if function_nodes != function_node_total {
            return Err(b109("max_builder_bytes", MAX_BUILDER_BYTES));
        }

        let result_key = key_for_type(program, &function.return_type);
        add_root(
            &mut function_stats,
            result_key,
            declaration_facts,
            fallback,
            0,
            type_bytes_for_key(result_key)
                .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?,
        )?;
        if facts_for_key(result_key, declaration_facts, fallback).leaves != 0 {
            function_stats.parent_local_epochs = function_stats
                .parent_local_epochs
                .checked_add(1)
                .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
        }
        if facts_for_key(result_key, declaration_facts, fallback).leaves != 0 {
            function_stats.ordinary_slot_payload_bytes = function_stats
                .ordinary_slot_payload_bytes
                .checked_add(
                    value_storage_identity_bytes_for_path(0)
                        .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?,
                )
                .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
        }
        let result_finalizer_events =
            function
                .ensures
                .iter()
                .try_fold(function.ensures.len(), |events, ensure| {
                    events
                        .checked_add(cleanup_expression_failure_events(
                            ensure,
                            &mut event_traversal,
                        )?)
                        .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))
                })?;
        add_finalizer_upper(
            &mut function_stats,
            result_key,
            declaration_facts,
            fallback,
            result_finalizer_events,
            0,
        )?;
        if has_try {
            // The plan retains one Body staging source in addition to every
            // residual source materialized by a postfix `?`.
            function_stats.staged_results = function_stats
                .staged_results
                .checked_add(1)
                .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
        }
        let expression_identity_bytes = function
            .stable_id
            .len()
            .checked_add(
                function_nodes
                    .checked_mul(path_segment_bytes)
                    .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?,
            )
            .and_then(|bytes| bytes.checked_add(fallback.shape_ids))
            .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
        let staged_owned_bytes = expression_identity_bytes
            .checked_mul(2)
            .and_then(|bytes| bytes.checked_add(maximum_resolved_type_owned_bytes.checked_mul(2)?))
            .and_then(|bytes| bytes.checked_add(maximum_declaration_identity_bytes.checked_mul(5)?))
            .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
        function_stats.stage_identity_and_type_bytes = function_stats
            .staged_results
            .checked_mul(staged_owned_bytes)
            .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
        function_stats.variant_identity_bytes = function_stats
            .variant_edges
            .checked_mul(
                expression_identity_bytes
                    .checked_add(maximum_declaration_identity_bytes)
                    .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?,
            )
            .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
        if function_stats.leaves != 0 {
            // Each cleanup storage epoch is initialized once and transferred
            // at most once; its inventory/plan slot is accounted separately.
            // CallCommit argument sources are additional projected places.
            let root_transition_copies = function_stats
                .roots
                .checked_mul(2)
                .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
            let projected_place_copies = root_transition_copies
                .checked_add(function_stats.call_arguments)
                .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
            function_stats.place_copies = function_stats
                .roots
                .checked_add(projected_place_copies)
                .and_then(|value| value.checked_add(owned_parameters))
                .and_then(|value| value.checked_add(1))
                .and_then(|value| value.checked_add(function_stats.finalizer_copies))
                .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
            function_stats.place_projection_segments = function_stats
                .projection_segments
                .checked_mul(2)
                .and_then(|segments| {
                    segments.checked_add(
                        fallback
                            .projection_segments
                            .checked_mul(function_stats.call_arguments)?,
                    )
                })
                .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
            function_stats.place_projection_ids = function_stats
                .projection_ids
                .checked_mul(2)
                .and_then(|bytes| {
                    bytes.checked_add(
                        fallback
                            .projection_ids
                            .checked_mul(function_stats.call_arguments)?,
                    )
                })
                .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
        }

        let multiplicity = if function.type_parameters.is_empty() {
            1
        } else {
            generic_instance_upper
                .checked_add(1)
                .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?
        };
        total
            .merge(
                function_stats
                    .scaled(multiplicity)
                    .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?,
            )
            .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
    }
    Ok(total)
}
