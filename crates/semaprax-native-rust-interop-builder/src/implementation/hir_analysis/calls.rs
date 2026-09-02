//! Call-contract digests, positional resolved-expression traversal, and
//! call-site census used by the bounded capacity proofs.

use super::*;

#[allow(clippy::too_many_arguments)]
pub(in crate::implementation) fn call_digest(
    direction: &str,
    id: &str,
    parameters: &[ParameterFact],
    result: ScalarType,
    effects: &[String],
    capabilities: &[String],
    required_imports: &[String],
    required_import_contracts: &[(String, String)],
    failure: &str,
    _capacity_baseline: usize,
    target: &Target,
) -> Result<String, Diagnostic> {
    let parameter_values = parameters
        .iter()
        .map(|parameter| format!("{}:{}:value", parameter.name, scalar_text(parameter.ty)))
        .collect::<Vec<_>>();
    let params = parameter_values.join("\0");
    let target = target_json(target);
    let effects = effects.join("\0");
    let capabilities = capabilities.join("\0");
    #[cfg(test)]
    {
        let scratch = checked_owned_string_vec(&parameter_values, parameter_values.capacity())
            .and_then(|bytes| bytes.checked_add(params.capacity()))
            .and_then(|bytes| bytes.checked_add(target.capacity()))
            .and_then(|bytes| bytes.checked_add(effects.capacity()))
            .and_then(|bytes| bytes.checked_add(capabilities.capacity()))
            .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
        note_post_hir_facts_live(_capacity_baseline, scratch);
    }
    let abi = "1\0C\0u64-domain16-code32-class8-retry1-reserved7\0u8-0-or-1\0signed-two-complement-i64\0SPXNRCTX1\0SPXNRIMP1\0caller-owned-uninitialized-success-only\0none-across-boundary\0caught-before-ffi-return\0same-thread\0rejected";
    let mut hasher = Sha256::new();
    hasher.update(CALL_DOMAIN);
    for value in [
        direction.as_bytes(),
        id.as_bytes(),
        params.as_bytes(),
        scalar_text(result).as_bytes(),
        effects.as_bytes(),
        capabilities.as_bytes(),
        failure.as_bytes(),
        target.as_bytes(),
        abi.as_bytes(),
    ] {
        frame(&mut hasher, value);
    }
    hash_count(&mut hasher, "required-imports", required_imports.len());
    for import in required_imports {
        frame(&mut hasher, import.as_bytes());
    }
    hash_count(
        &mut hasher,
        "required-import-contracts",
        required_import_contracts.len(),
    );
    for (id, digest) in required_import_contracts {
        frame(&mut hasher, id.as_bytes());
        frame(&mut hasher, digest.as_bytes());
    }
    let digest = format!(
        "sha256:{:x}",
        semaprax::digest_hex::LowerHex(hasher.finalize())
    );
    #[cfg(test)]
    {
        let scratch = checked_owned_string_vec(&parameter_values, parameter_values.capacity())
            .and_then(|bytes| bytes.checked_add(params.capacity()))
            .and_then(|bytes| bytes.checked_add(target.capacity()))
            .and_then(|bytes| bytes.checked_add(effects.capacity()))
            .and_then(|bytes| bytes.checked_add(capabilities.capacity()))
            .and_then(|bytes| bytes.checked_add(digest.capacity()))
            .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
        note_post_hir_facts_live(_capacity_baseline, scratch);
    }
    Ok(digest)
}

pub(in crate::implementation) fn visit_calls(
    expression: &ResolvedExpr,
    functions: &mut BTreeSet<DeclarationId>,
    imports: &mut BTreeSet<DeclarationId>,
    _capacity_baseline: usize,
    _scratch_baseline: usize,
) -> Result<(), Diagnostic> {
    let mut frames = Vec::with_capacity(MAX_SEMANTIC_EXPRESSION_DEPTH + 1);
    frames.push((expression, 0usize));
    while let Some((expression, next)) = frames.pop() {
        if next == 0 {
            match &expression.kind {
                ResolvedExprKind::Call { callee, .. } => {
                    functions.insert(callee.clone());
                }
                ResolvedExprKind::NativeRustImportCall(call) => {
                    imports.insert(call.import.clone());
                }
                _ => {}
            }
        }
        let mut child_cursor = next;
        if let Some((_, child)) = resolved_expression_child(expression, &mut child_cursor) {
            if frames.len() + 2 > frames.capacity() {
                return Err(b109(
                    "max_semantic_expression_depth",
                    MAX_SEMANTIC_EXPRESSION_DEPTH,
                ));
            }
            frames.push((expression, child_cursor));
            frames.push((child, 0));
        }
        #[cfg(test)]
        {
            let scratch = frames.capacity() * std::mem::size_of::<(&ResolvedExpr, usize)>()
                + declaration_set_capacity(functions)
                + declaration_set_capacity(imports);
            note_post_hir_facts_scratch(_scratch_baseline.saturating_add(scratch));
            note_post_hir_facts_capacity(_capacity_baseline.saturating_add(scratch));
        }
    }
    Ok(())
}

const RESOLVED_COMPLEX_CURSOR: usize = 1usize << (usize::BITS - 1);
const RESOLVED_CURSOR_INDEX_MASK: usize = RESOLVED_COMPLEX_CURSOR - 1;

pub(in crate::implementation) fn resolved_expression_child<'a>(
    expression: &'a ResolvedExpr,
    cursor: &mut usize,
) -> Option<(usize, &'a ResolvedExpr)> {
    let complex = *cursor & RESOLVED_COMPLEX_CURSOR != 0;
    let mut index = *cursor & RESOLVED_CURSOR_INDEX_MASK;
    let mut advance = |next: usize, path_index: usize, child| {
        *cursor = usize::from(complex)
            .checked_mul(RESOLVED_COMPLEX_CURSOR)?
            .checked_add(next)?;
        Some((path_index, child))
    };
    match &expression.kind {
        ResolvedExprKind::Call { args, .. } => {
            advance(index.checked_add(1)?, index, args.get(index)?)
        }
        ResolvedExprKind::NativeRustImportCall(call) => {
            advance(index.checked_add(1)?, index, call.args.get(index)?)
        }
        ResolvedExprKind::HostCommandCall(call) => {
            advance(index.checked_add(1)?, index, call.args.get(index)?)
        }
        ResolvedExprKind::ByteRange {
            source, start, end, ..
        } => {
            let child = [source.as_ref(), start.as_ref(), end.as_ref()]
                .get(index)
                .copied()?;
            advance(index.checked_add(1)?, index, child)
        }
        ResolvedExprKind::Unary { value, .. }
        | ResolvedExprKind::Try { operand: value, .. }
        | ResolvedExprKind::TryOption { operand: value, .. }
        | ResolvedExprKind::Project { base: value, .. }
        | ResolvedExprKind::Upcast { source: value } => {
            (index == 0).then(|| advance(1, 0, value.as_ref()))?
        }
        ResolvedExprKind::Binary { left, right, .. } => {
            let child = [left.as_ref(), right.as_ref()].get(index).copied()?;
            advance(index.checked_add(1)?, index, child)
        }
        ResolvedExprKind::Block { statements, tail } => {
            let has_while = complex
                || (index == 0
                    && statements
                        .iter()
                        .any(|statement| matches!(statement, ResolvedStatement::While { .. })));
            if has_while {
                if !complex {
                    *cursor = RESOLVED_COMPLEX_CURSOR;
                    index = 0;
                }
                loop {
                    let slot_limit = statements.len().checked_mul(2)?;
                    if index < slot_limit {
                        let statement_index = index / 2;
                        let statement_child = index % 2;
                        let path_index = index;
                        index = index.checked_add(1)?;
                        *cursor = RESOLVED_COMPLEX_CURSOR.checked_add(index)?;
                        if let Some(child) = statements[statement_index].child(statement_child) {
                            return Some((path_index, child));
                        }
                        continue;
                    }
                    if index == slot_limit {
                        *cursor = RESOLVED_COMPLEX_CURSOR.checked_add(index.checked_add(1)?)?;
                        return Some((index, tail));
                    }
                    return None;
                }
            }
            let child = if index < statements.len() {
                statements[index].child(0)?
            } else if index == statements.len() {
                tail
            } else {
                return None;
            };
            advance(index.checked_add(1)?, index, child)
        }
        ResolvedExprKind::If {
            condition,
            then_branch,
            else_branch,
        } => [
            condition.as_ref(),
            then_branch.as_ref(),
            else_branch.as_ref(),
        ]
        .get(index)
        .copied()
        .and_then(|child| advance(index.checked_add(1)?, index, child)),
        ResolvedExprKind::ConstructRecord { fields, .. }
        | ResolvedExprKind::ConstructVariant { fields, .. } => {
            let child = &fields.get(index)?.value;
            advance(index.checked_add(1)?, index, child)
        }
        ResolvedExprKind::Match {
            scrutinee, arms, ..
        } => {
            let has_guard = complex || (index == 0 && arms.iter().any(|arm| arm.guard.is_some()));
            if has_guard {
                if !complex {
                    *cursor = RESOLVED_COMPLEX_CURSOR;
                    index = 0;
                }
                loop {
                    if index == 0 {
                        *cursor = RESOLVED_COMPLEX_CURSOR + 1;
                        return Some((0, scrutinee));
                    }
                    let arm_slot = index.checked_sub(1)?;
                    let arm_index = arm_slot / 2;
                    let arm_child = arm_slot % 2;
                    let arm = arms.get(arm_index)?;
                    let path_index = index;
                    index = index.checked_add(1)?;
                    *cursor = RESOLVED_COMPLEX_CURSOR.checked_add(index)?;
                    if arm_child == 0 {
                        if let Some(guard) = &arm.guard {
                            return Some((path_index, guard));
                        }
                    } else {
                        return Some((path_index, &arm.value));
                    }
                }
            }
            let child = if index == 0 {
                scrutinee.as_ref()
            } else {
                &arms.get(index - 1)?.value
            };
            advance(index.checked_add(1)?, index, child)
        }
        ResolvedExprKind::UpdateRecord { base, fields, .. } => {
            let child = if index == 0 {
                base.as_ref()
            } else {
                &fields.get(index - 1)?.value
            };
            advance(index.checked_add(1)?, index, child)
        }
        ResolvedExprKind::Int(_)
        | ResolvedExprKind::Int32(_)
        | ResolvedExprKind::Char(_)
        | ResolvedExprKind::Uint8(_)
        | ResolvedExprKind::Usize(_)
        | ResolvedExprKind::ArrayU8(_)
        | ResolvedExprKind::RepeatArrayU8 { .. }
        | ResolvedExprKind::Float32(_)
        | ResolvedExprKind::Float64(_)
        | ResolvedExprKind::Bool(_)
        | ResolvedExprKind::String(_)
        | ResolvedExprKind::Place(_)
        | ResolvedExprKind::BorrowPlace { .. } => None,
    }
}

#[derive(Clone, Copy, Default)]
pub(in crate::implementation) struct TraversalCallSiteCensus {
    pub(in crate::implementation) function_sites: usize,
    pub(in crate::implementation) function_id_bytes: usize,
    pub(in crate::implementation) import_sites: usize,
    pub(in crate::implementation) import_id_bytes: usize,
}

pub(in crate::implementation) fn expression_call_site_census(
    root: &ResolvedExpr,
) -> Result<TraversalCallSiteCensus, Diagnostic> {
    let mut frames = [None; MAX_SEMANTIC_EXPRESSION_DEPTH + 1];
    let mut frame_len = 1usize;
    frames[0] = Some((root, 0usize));
    let mut census = TraversalCallSiteCensus::default();
    while frame_len > 0 {
        let (expression, next) = frames[frame_len - 1]
            .take()
            .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
        frame_len -= 1;
        if next == 0 {
            match &expression.kind {
                ResolvedExprKind::Call { callee, .. } => {
                    census.function_sites = census
                        .function_sites
                        .checked_add(1)
                        .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
                    census.function_id_bytes = census
                        .function_id_bytes
                        .checked_add(callee.as_str().len())
                        .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
                }
                ResolvedExprKind::NativeRustImportCall(call) => {
                    census.import_sites = census
                        .import_sites
                        .checked_add(1)
                        .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
                    census.import_id_bytes = census
                        .import_id_bytes
                        .checked_add(call.import.as_str().len())
                        .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
                }
                _ => {}
            }
        }
        let mut child_cursor = next;
        if let Some((_, child)) = resolved_expression_child(expression, &mut child_cursor) {
            if frame_len + 2 > frames.len() {
                return Err(b109(
                    "max_semantic_expression_depth",
                    MAX_SEMANTIC_EXPRESSION_DEPTH,
                ));
            }
            frames[frame_len] = Some((expression, child_cursor));
            frames[frame_len + 1] = Some((child, 0));
            frame_len += 2;
        }
    }
    Ok(census)
}

pub(in crate::implementation) fn traversal_call_site_census(
    closure: &[&ResolvedFunction],
) -> Result<TraversalCallSiteCensus, Diagnostic> {
    closure
        .iter()
        .try_fold(TraversalCallSiteCensus::default(), |mut total, function| {
            for expression in function
                .requires
                .iter()
                .chain(std::iter::once(&function.body))
                .chain(&function.ensures)
            {
                let current = expression_call_site_census(expression)?;
                total.function_sites = total
                    .function_sites
                    .checked_add(current.function_sites)
                    .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
                total.function_id_bytes = total
                    .function_id_bytes
                    .checked_add(current.function_id_bytes)
                    .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
                total.import_sites = total
                    .import_sites
                    .checked_add(current.import_sites)
                    .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
                total.import_id_bytes = total
                    .import_id_bytes
                    .checked_add(current.import_id_bytes)
                    .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
            }
            Ok(total)
        })
}

pub(super) fn direct_calls(
    function: &ResolvedFunction,
    _capacity_baseline: usize,
    _scratch_baseline: usize,
) -> Result<(BTreeSet<DeclarationId>, BTreeSet<DeclarationId>), Diagnostic> {
    let mut functions = BTreeSet::new();
    let mut imports = BTreeSet::new();
    for contract in &function.requires {
        let mut contract_functions = BTreeSet::new();
        let mut contract_imports = BTreeSet::new();
        #[cfg(test)]
        let nested_baseline = _capacity_baseline
            + declaration_set_capacity(&functions)
            + declaration_set_capacity(&imports);
        #[cfg(not(test))]
        let nested_baseline = 0;
        #[cfg(test)]
        let nested_scratch = _scratch_baseline
            + declaration_set_capacity(&functions)
            + declaration_set_capacity(&imports);
        #[cfg(not(test))]
        let nested_scratch = 0;
        visit_calls(
            contract,
            &mut contract_functions,
            &mut contract_imports,
            nested_baseline,
            nested_scratch,
        )?;
        #[cfg(test)]
        note_post_hir_facts_live(
            _capacity_baseline,
            _scratch_baseline
                .saturating_add(declaration_set_capacity(&functions))
                .saturating_add(declaration_set_capacity(&imports))
                .saturating_add(declaration_set_capacity(&contract_functions))
                .saturating_add(declaration_set_capacity(&contract_imports)),
        );
        if !contract_imports.is_empty() {
            imports.insert(DeclarationId::new("\0native-rust-contract-call".to_owned()));
        }
        functions.extend(contract_functions);
    }
    visit_calls(
        &function.body,
        &mut functions,
        &mut imports,
        _capacity_baseline,
        _scratch_baseline,
    )?;
    for contract in &function.ensures {
        let mut contract_functions = BTreeSet::new();
        let mut contract_imports = BTreeSet::new();
        #[cfg(test)]
        let nested_baseline = _capacity_baseline
            + declaration_set_capacity(&functions)
            + declaration_set_capacity(&imports);
        #[cfg(not(test))]
        let nested_baseline = 0;
        #[cfg(test)]
        let nested_scratch = _scratch_baseline
            + declaration_set_capacity(&functions)
            + declaration_set_capacity(&imports);
        #[cfg(not(test))]
        let nested_scratch = 0;
        visit_calls(
            contract,
            &mut contract_functions,
            &mut contract_imports,
            nested_baseline,
            nested_scratch,
        )?;
        #[cfg(test)]
        note_post_hir_facts_live(
            _capacity_baseline,
            _scratch_baseline
                .saturating_add(declaration_set_capacity(&functions))
                .saturating_add(declaration_set_capacity(&imports))
                .saturating_add(declaration_set_capacity(&contract_functions))
                .saturating_add(declaration_set_capacity(&contract_imports)),
        );
        if !contract_imports.is_empty() {
            imports.insert(DeclarationId::new("\0native-rust-contract-call".to_owned()));
        }
        functions.extend(contract_functions);
    }
    Ok((functions, imports))
}
