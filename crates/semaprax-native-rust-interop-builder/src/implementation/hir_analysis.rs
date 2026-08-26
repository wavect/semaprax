// Bounded post-HIR semantic analysis for the private Native Rust interop lane.
// This module has no filesystem, process, platform, settlement, or publication authority.
use super::*;

pub(super) fn scalar_type(ty: &ResolvedType) -> Option<ScalarType> {
    match ty {
        ResolvedType::Unit => Some(ScalarType::Unit),
        ResolvedType::I64 => Some(ScalarType::I64),
        ResolvedType::Bool => Some(ScalarType::Bool),
        _ => None,
    }
}

pub(super) fn scalar_text(ty: ScalarType) -> &'static str {
    match ty {
        ScalarType::Unit => "unit",
        ScalarType::I64 => "i64",
        ScalarType::Bool => "bool",
    }
}

pub(super) fn c_type(ty: ScalarType) -> &'static str {
    match ty {
        ScalarType::Unit => "void",
        ScalarType::I64 => "int64_t",
        ScalarType::Bool => "uint8_t",
    }
}

pub(super) fn rust_type(ty: ScalarType) -> &'static str {
    match ty {
        ScalarType::Unit => "()",
        ScalarType::I64 => "i64",
        ScalarType::Bool => "bool",
    }
}

pub(super) fn rust_ffi_wire_type(ty: ScalarType) -> &'static str {
    match ty {
        ScalarType::Unit => "()",
        ScalarType::I64 => "i64",
        ScalarType::Bool => "u8",
    }
}

#[allow(clippy::too_many_arguments)]
pub(super) fn call_digest(
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

pub(super) fn visit_calls(
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

pub(super) fn resolved_expression_child<'a>(
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
        ResolvedExprKind::Match { scrutinee, arms } => {
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
pub(super) struct TraversalCallSiteCensus {
    pub(super) function_sites: usize,
    pub(super) function_id_bytes: usize,
    pub(super) import_sites: usize,
    pub(super) import_id_bytes: usize,
}

pub(super) fn expression_call_site_census(
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

pub(super) fn traversal_call_site_census(
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

fn direct_calls(
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

#[cfg(test)]
pub(super) fn btree_allocation_upper<K, V>(len: usize) -> usize {
    // A BTree allocation contains inline key/value slots plus links and node
    // metadata. Charging one complete map header per live entry is a
    // conservative upper for the separately allocated node/link storage: a
    // non-root node always contains multiple entries, while a singleton root
    // needs only one header.
    len.saturating_mul(
        std::mem::size_of::<(K, V)>().saturating_add(std::mem::size_of::<BTreeMap<K, V>>()),
    )
}

#[cfg(test)]
fn declaration_set_capacity(set: &BTreeSet<DeclarationId>) -> usize {
    btree_allocation_upper::<DeclarationId, ()>(set.len())
        .saturating_add(set.iter().map(|id| id.as_str().len()).sum::<usize>())
}

pub(super) struct SelectedClosureFrame {
    id: String,
    calls: Vec<String>,
    next: usize,
    longest: usize,
}

const _: () = assert!(std::mem::size_of::<SelectedClosureFrame>() == 64);

#[cfg(test)]
fn selected_closure_live_capacity(
    by_id: &BTreeMap<&str, &ResolvedFunction>,
    state: &BTreeMap<String, u8>,
    depths: &BTreeMap<String, usize>,
    closure: &Vec<&ResolvedFunction>,
    reached_imports: &BTreeSet<String>,
    stack: &Vec<SelectedClosureFrame>,
    pending: &Option<String>,
) -> usize {
    let stack_bytes = stack.iter().fold(
        stack.capacity() * std::mem::size_of::<SelectedClosureFrame>(),
        |bytes, frame| {
            bytes
                .saturating_add(frame.id.capacity())
                .saturating_add(frame.calls.capacity() * std::mem::size_of::<String>())
                .saturating_add(frame.calls.iter().map(String::capacity).sum::<usize>())
        },
    );
    let map_bytes = btree_allocation_upper::<&str, &ResolvedFunction>(by_id.len())
        .saturating_add(btree_allocation_upper::<String, u8>(state.len()))
        .saturating_add(state.keys().map(String::capacity).sum::<usize>())
        .saturating_add(btree_allocation_upper::<String, usize>(depths.len()))
        .saturating_add(depths.keys().map(String::capacity).sum::<usize>())
        .saturating_add(btree_allocation_upper::<String, ()>(reached_imports.len()))
        .saturating_add(reached_imports.iter().map(String::capacity).sum::<usize>());
    let closure_bytes = closure.capacity() * std::mem::size_of::<&ResolvedFunction>();
    stack_bytes
        .saturating_add(map_bytes)
        .saturating_add(closure_bytes)
        .saturating_add(pending.as_ref().map_or(0, String::capacity))
}

fn contract_reaches_native_import(
    function: &ResolvedFunction,
    by_id: &BTreeMap<&str, &ResolvedFunction>,
    #[cfg(test)] retained_outer_bytes: usize,
) -> Result<bool, Diagnostic> {
    let mut pending = BTreeSet::new();
    for contract in function.requires.iter().chain(&function.ensures) {
        let mut calls = BTreeSet::new();
        let mut imports = BTreeSet::new();
        visit_calls(
            contract,
            &mut calls,
            &mut imports,
            #[cfg(test)]
            retained_outer_bytes.saturating_add(declaration_set_capacity(&pending)),
            #[cfg(not(test))]
            0,
            0,
        )?;
        #[cfg(test)]
        note_closure_capacity_high_water(
            retained_outer_bytes
                .saturating_add(declaration_set_capacity(&pending))
                .saturating_add(declaration_set_capacity(&calls))
                .saturating_add(declaration_set_capacity(&imports)),
        );
        if !imports.is_empty() {
            return Ok(true);
        }
        pending.extend(calls);
    }
    let mut visited = BTreeSet::new();
    while let Some(id) = pending.pop_first() {
        if !visited.insert(id.clone()) {
            continue;
        }
        let helper = by_id
            .get(id.as_str())
            .ok_or_else(|| b107("selected identity missing"))?;
        let (calls, imports) = direct_calls(helper, 0, 0)?;
        #[cfg(test)]
        note_closure_capacity_high_water(
            retained_outer_bytes
                .saturating_add(declaration_set_capacity(&pending))
                .saturating_add(declaration_set_capacity(&visited))
                .saturating_add(declaration_set_capacity(&calls))
                .saturating_add(declaration_set_capacity(&imports))
                .saturating_add(id.as_str().len()),
        );
        if !imports.is_empty() {
            return Ok(true);
        }
        pending.extend(calls);
    }
    Ok(false)
}

pub(super) fn selected_closure<'a>(
    resolved: &'a ResolvedProgram,
    selected: &[String],
) -> Result<(Vec<&'a ResolvedFunction>, BTreeSet<String>), Diagnostic> {
    note_hir_post_resolve_phase(0);
    let by_id = resolved
        .functions
        .iter()
        .map(|function| (function.id.as_str(), function))
        .collect::<BTreeMap<_, _>>();
    let mut state = BTreeMap::<String, u8>::new();
    let mut depths = BTreeMap::<String, usize>::new();
    let mut closure = Vec::new();
    let mut reached_imports = BTreeSet::new();

    for root in selected {
        if state.get(root).copied() == Some(2) {
            continue;
        }
        let mut stack = Vec::<SelectedClosureFrame>::new();
        let mut pending = Some(root.clone());
        loop {
            #[cfg(test)]
            note_closure_capacity_high_water(selected_closure_live_capacity(
                &by_id,
                &state,
                &depths,
                &closure,
                &reached_imports,
                &stack,
                &pending,
            ));
            if let Some(id) = pending.take() {
                match state.get(&id).copied() {
                    Some(1) => return Err(b107("selected closure is cyclic")),
                    Some(2) => {
                        let child_depth = *depths
                            .get(&id)
                            .ok_or_else(|| b107("selected identity missing"))?;
                        if let Some(parent) = stack.last_mut() {
                            parent.longest = parent.longest.max(
                                child_depth
                                    .checked_add(1)
                                    .ok_or_else(|| b109("max_call_depth", MAX_CALL_DEPTH))?,
                            );
                            if parent.longest > MAX_CALL_DEPTH {
                                return Err(b109("max_call_depth", MAX_CALL_DEPTH));
                            }
                            continue;
                        }
                        break;
                    }
                    _ => {}
                }
                let function = by_id
                    .get(id.as_str())
                    .ok_or_else(|| b107("selected identity missing"))?;
                if contract_reaches_native_import(
                    function,
                    &by_id,
                    #[cfg(test)]
                    selected_closure_live_capacity(
                        &by_id,
                        &state,
                        &depths,
                        &closure,
                        &reached_imports,
                        &stack,
                        &pending,
                    ),
                )? {
                    return Err(b107("effect or capability mismatch"));
                }
                if state.len() >= MAX_CLOSURE_FUNCTIONS {
                    return Err(b109("max_closure_functions", MAX_CLOSURE_FUNCTIONS));
                }
                state.insert(id.clone(), 1);
                let (calls, imports) = direct_calls(function, 0, 0)?;
                #[cfg(test)]
                note_closure_capacity_high_water(
                    selected_closure_live_capacity(
                        &by_id,
                        &state,
                        &depths,
                        &closure,
                        &reached_imports,
                        &stack,
                        &pending,
                    )
                    .saturating_add(declaration_set_capacity(&calls))
                    .saturating_add(declaration_set_capacity(&imports)),
                );
                if imports.iter().any(|id| id.as_str().starts_with('\0')) {
                    return Err(b107("effect or capability mismatch"));
                }
                reached_imports.extend(imports.into_iter().map(|id| id.as_str().to_owned()));
                let mut call_ids = Vec::with_capacity(calls.len());
                let mut remaining_calls = calls;
                while let Some(id) = remaining_calls.pop_first() {
                    call_ids.push(id.as_str().to_owned());
                    #[cfg(test)]
                    note_closure_capacity_high_water(
                        selected_closure_live_capacity(
                            &by_id,
                            &state,
                            &depths,
                            &closure,
                            &reached_imports,
                            &stack,
                            &pending,
                        )
                        .saturating_add(declaration_set_capacity(&remaining_calls))
                        .saturating_add(
                            call_ids.capacity() * std::mem::size_of::<String>()
                                + call_ids.iter().map(String::capacity).sum::<usize>(),
                        ),
                    );
                }
                stack.push(SelectedClosureFrame {
                    id,
                    calls: call_ids,
                    next: 0,
                    longest: 1,
                });
                if stack.len() > MAX_CALL_DEPTH {
                    return Err(b109("max_call_depth", MAX_CALL_DEPTH));
                }
            }

            let Some(frame) = stack.last_mut() else { break };
            if let Some(call) = frame.calls.get(frame.next).cloned() {
                frame.next += 1;
                pending = Some(call);
                continue;
            }
            let frame = stack.pop().expect("checked nonempty");
            let function = by_id
                .get(frame.id.as_str())
                .ok_or_else(|| b107("selected identity missing"))?;
            state.insert(frame.id.clone(), 2);
            depths.insert(frame.id, frame.longest);
            closure.push(*function);
            if let Some(parent) = stack.last_mut() {
                parent.longest = parent.longest.max(
                    frame
                        .longest
                        .checked_add(1)
                        .ok_or_else(|| b109("max_call_depth", MAX_CALL_DEPTH))?,
                );
                if parent.longest > MAX_CALL_DEPTH {
                    return Err(b109("max_call_depth", MAX_CALL_DEPTH));
                }
            } else {
                break;
            }
        }
    }
    closure.sort_by(|left, right| left.id.cmp(&right.id));
    Ok((closure, reached_imports))
}

pub(super) fn transitive_imports(
    root: &ResolvedFunction,
    functions: &BTreeMap<&str, &ResolvedFunction>,
    pending_capacity: usize,
    _capacity_baseline: usize,
) -> Result<BTreeSet<String>, Diagnostic> {
    let mut pending = Vec::with_capacity(pending_capacity);
    pending.push(root.id.as_str().to_owned());
    let mut visited = BTreeSet::new();
    let mut imports = BTreeSet::new();
    while let Some(id) = pending.pop() {
        #[cfg(test)]
        note_post_hir_facts_live(
            _capacity_baseline,
            checked_owned_string_vec(&pending, pending.capacity())
                .and_then(|bytes| bytes.checked_add(owned_string_set_owned_capacity(&visited)))
                .and_then(|bytes| bytes.checked_add(owned_string_set_owned_capacity(&imports)))
                .and_then(|bytes| bytes.checked_add(id.capacity()))
                .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?,
        );
        if !visited.insert(id.clone()) {
            continue;
        }
        let function = functions
            .get(id.as_str())
            .ok_or_else(|| b107("selected identity missing"))?;
        #[cfg(test)]
        let traversal_scratch = checked_owned_string_vec(&pending, pending.capacity())
            .and_then(|bytes| bytes.checked_add(owned_string_set_owned_capacity(&visited)))
            .and_then(|bytes| bytes.checked_add(owned_string_set_owned_capacity(&imports)))
            .and_then(|bytes| bytes.checked_add(id.capacity()))
            .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
        #[cfg(not(test))]
        let traversal_scratch = 0;
        #[cfg(test)]
        note_post_hir_facts_live(_capacity_baseline, traversal_scratch);
        #[cfg(test)]
        let traversal_baseline = _capacity_baseline.saturating_add(traversal_scratch);
        #[cfg(not(test))]
        let traversal_baseline = 0;
        let (calls, reached) = direct_calls(function, traversal_baseline, traversal_scratch)?;
        #[cfg(test)]
        note_post_hir_facts_live(
            _capacity_baseline,
            traversal_scratch
                .saturating_add(declaration_set_capacity(&calls))
                .saturating_add(declaration_set_capacity(&reached)),
        );
        if reached.iter().any(|id| id.as_str().starts_with('\0')) {
            return Err(b107("effect or capability mismatch"));
        }
        imports.extend(reached.into_iter().map(|id| id.as_str().to_owned()));
        pending.extend(calls.into_iter().map(|id| id.as_str().to_owned()));
    }
    Ok(imports)
}

pub(super) fn parameter_facts(
    function: &ResolvedFunction,
) -> Result<Vec<ParameterFact>, Diagnostic> {
    if function.params.len() > MAX_PARAMETERS {
        return Err(b109("max_parameters", MAX_PARAMETERS));
    }
    let mut facts = Vec::with_capacity(function.params.len());
    for parameter in &function.params {
        if parameter.ownership != OwnershipMode::Value
            || parameter.name.len() > MAX_IDENTIFIER_BYTES
        {
            return Err(b107("scalar value signature required"));
        }
        facts.push(ParameterFact {
            name: parameter.name.clone(),
            ty: scalar_type(&parameter.ty)
                .filter(|ty| *ty != ScalarType::Unit)
                .ok_or_else(|| b107("scalar value signature required"))?,
        });
    }
    Ok(facts)
}

pub(super) fn import_parameter_facts(
    import: &ResolvedImport,
) -> Result<Vec<ParameterFact>, Diagnostic> {
    if import.parameters.len() > MAX_PARAMETERS {
        return Err(b109("max_parameters", MAX_PARAMETERS));
    }
    let mut facts = Vec::with_capacity(import.parameters.len());
    for parameter in &import.parameters {
        if parameter.ownership != OwnershipMode::Value
            || parameter.consumes_on_failure
            || parameter.name.len() > MAX_IDENTIFIER_BYTES
        {
            return Err(b107("scalar value signature required"));
        }
        facts.push(ParameterFact {
            name: parameter.name.clone(),
            ty: scalar_type(&parameter.ty)
                .filter(|ty| *ty != ScalarType::Unit)
                .ok_or_else(|| b107("scalar value signature required"))?,
        });
    }
    Ok(facts)
}

#[derive(Clone, Copy, Debug)]
pub(super) struct TypeIdentityMetrics {
    nodes: usize,
    all_key_bytes: usize,
    root_bytes: usize,
    maximum_encoded_bytes: usize,
}

enum TypeIdentityFrame<'a> {
    Enter(&'a ResolvedType),
    Finish(&'a DeclarationId, usize, usize, usize),
}

#[derive(Clone, Copy)]
enum TypeIdentityMetricFrame<'a> {
    Enter(&'a ResolvedType, usize),
    Finish(&'a DeclarationId, usize),
}

fn decimal_bytes(mut value: usize) -> usize {
    let mut bytes = 1usize;
    while value >= 10 {
        value /= 10;
        bytes += 1;
    }
    bytes
}

pub(super) fn type_identity_metrics(
    ty: &ResolvedType,
    initial_depth: usize,
) -> Result<TypeIdentityMetrics, Diagnostic> {
    let leaf = |root_bytes| TypeIdentityMetrics {
        nodes: 1,
        all_key_bytes: root_bytes,
        root_bytes,
        maximum_encoded_bytes: 0,
    };
    let mut frames = [None; FINGERPRINT_ACTION_SLOTS];
    let mut frame_len = 1usize;
    frames[0] = Some(TypeIdentityMetricFrame::Enter(ty, initial_depth));
    let mut results = [None; FINGERPRINT_ACTION_SLOTS];
    let mut result_len = 0usize;
    let mut work = 0usize;
    while frame_len > 0 {
        frame_len -= 1;
        let frame = frames[frame_len]
            .take()
            .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
        match frame {
            TypeIdentityMetricFrame::Enter(ty, depth) => {
                if depth > MAX_SEMANTIC_EXPRESSION_DEPTH {
                    return Err(b109(
                        "max_semantic_expression_depth",
                        MAX_SEMANTIC_EXPRESSION_DEPTH,
                    ));
                }
                work = work
                    .checked_add(1)
                    .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
                if work > FINGERPRINT_ACTION_SLOTS {
                    return Err(b109("max_builder_bytes", MAX_BUILDER_BYTES));
                }
                let metric = match ty {
                    ResolvedType::Unit => Some(leaf("unit".len())),
                    ResolvedType::I64 => Some(leaf("i64".len())),
                    ResolvedType::I32 => Some(leaf("i32".len())),
                    ResolvedType::Char => Some(leaf("char".len())),
                    ResolvedType::U8 => Some(leaf("u8".len())),
                    ResolvedType::Usize => Some(leaf("usize".len())),
                    ResolvedType::ArrayU8(length) => Some(leaf(
                        "array:u8:"
                            .len()
                            .checked_add(decimal_bytes(*length as usize))
                            .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?,
                    )),
                    ResolvedType::F32 => Some(leaf("f32".len())),
                    ResolvedType::F64 => Some(leaf("f64".len())),
                    ResolvedType::Bool => Some(leaf("bool".len())),
                    ResolvedType::String => Some(leaf("string".len())),
                    ResolvedType::Bytes => Some(leaf("bytes".len())),
                    ResolvedType::Str => Some(leaf("str".len())),
                    ResolvedType::SliceU8 => Some(leaf("slice-u8".len())),
                    ResolvedType::TypeParameter { owner, index } => {
                        let owner_bytes = owner.as_str().len();
                        let root_bytes = "parameter:"
                            .len()
                            .checked_add(decimal_bytes(owner_bytes))
                            .and_then(|bytes| bytes.checked_add(1))
                            .and_then(|bytes| bytes.checked_add(owner_bytes))
                            .and_then(|bytes| bytes.checked_add(1))
                            .and_then(|bytes| bytes.checked_add(decimal_bytes(*index as usize)))
                            .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
                        Some(leaf(root_bytes))
                    }
                    ResolvedType::Nominal {
                        declaration,
                        arguments,
                    } => {
                        if frame_len
                            .checked_add(arguments.len())
                            .and_then(|len| len.checked_add(1))
                            .is_none_or(|len| len > frames.len())
                        {
                            return Err(b109("max_builder_bytes", MAX_BUILDER_BYTES));
                        }
                        frames[frame_len] = Some(TypeIdentityMetricFrame::Finish(
                            declaration,
                            arguments.len(),
                        ));
                        frame_len += 1;
                        for argument in arguments.iter().rev() {
                            frames[frame_len] =
                                Some(TypeIdentityMetricFrame::Enter(argument, depth + 1));
                            frame_len += 1;
                        }
                        None
                    }
                };
                if let Some(metric) = metric {
                    let slot = results
                        .get_mut(result_len)
                        .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
                    *slot = Some(metric);
                    result_len += 1;
                }
            }
            TypeIdentityMetricFrame::Finish(declaration, count) => {
                let split = result_len
                    .checked_sub(count)
                    .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
                let mut nodes = 1usize;
                let mut all_key_bytes = 0usize;
                let mut encoded_bytes = 0usize;
                let mut maximum_encoded_bytes = 0usize;
                for slot in &mut results[split..result_len] {
                    let child = slot
                        .take()
                        .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
                    nodes = nodes
                        .checked_add(child.nodes)
                        .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
                    all_key_bytes = all_key_bytes
                        .checked_add(child.all_key_bytes)
                        .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
                    encoded_bytes = encoded_bytes
                        .checked_add(decimal_bytes(child.root_bytes))
                        .and_then(|bytes| bytes.checked_add(1))
                        .and_then(|bytes| bytes.checked_add(child.root_bytes))
                        .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
                    maximum_encoded_bytes = maximum_encoded_bytes.max(child.maximum_encoded_bytes);
                }
                let declaration_bytes = declaration.as_str().len();
                let root_bytes = "nominal:"
                    .len()
                    .checked_add(decimal_bytes(declaration_bytes))
                    .and_then(|bytes| bytes.checked_add(1))
                    .and_then(|bytes| bytes.checked_add(declaration_bytes))
                    .and_then(|bytes| bytes.checked_add(1))
                    .and_then(|bytes| bytes.checked_add(decimal_bytes(count)))
                    .and_then(|bytes| bytes.checked_add(1))
                    .and_then(|bytes| bytes.checked_add(encoded_bytes))
                    .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
                result_len = split;
                results[result_len] = Some(TypeIdentityMetrics {
                    nodes,
                    all_key_bytes: all_key_bytes
                        .checked_add(root_bytes)
                        .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?,
                    root_bytes,
                    maximum_encoded_bytes: maximum_encoded_bytes.max(encoded_bytes),
                });
                result_len += 1;
            }
        }
    }
    if result_len != 1 {
        return Err(b109("max_builder_bytes", MAX_BUILDER_BYTES));
    }
    results[0]
        .take()
        .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))
}

pub(super) fn type_identity_scratch_upper(ty: &ResolvedType) -> Result<usize, Diagnostic> {
    let metrics = type_identity_metrics(ty, 1)?;
    metrics
        .nodes
        .checked_mul(std::mem::size_of::<TypeIdentityFrame<'_>>())
        .and_then(|bytes| {
            bytes.checked_add(metrics.nodes.checked_mul(std::mem::size_of::<String>())?)
        })
        .and_then(|bytes| bytes.checked_add(metrics.all_key_bytes))
        .and_then(|bytes| bytes.checked_add(metrics.maximum_encoded_bytes))
        .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))
}

pub(super) fn fingerprint_type_identity(
    ty: &ResolvedType,
    _capacity_baseline: usize,
    _outer_scratch: usize,
) -> Result<String, Diagnostic> {
    let metrics = type_identity_metrics(ty, 1)?;
    let mut frames = Vec::with_capacity(metrics.nodes);
    let mut keys = Vec::<String>::with_capacity(metrics.nodes);
    frames.push(TypeIdentityFrame::Enter(ty));
    while let Some(frame) = frames.pop() {
        match frame {
            TypeIdentityFrame::Enter(ty) => match ty {
                ResolvedType::Unit
                | ResolvedType::I64
                | ResolvedType::I32
                | ResolvedType::Char
                | ResolvedType::U8
                | ResolvedType::Usize
                | ResolvedType::F32
                | ResolvedType::F64
                | ResolvedType::Bool
                | ResolvedType::String
                | ResolvedType::Bytes
                | ResolvedType::Str
                | ResolvedType::SliceU8 => {
                    let text = match ty {
                        ResolvedType::Unit => "unit",
                        ResolvedType::I64 => "i64",
                        ResolvedType::I32 => "i32",
                        ResolvedType::Char => "char",
                        ResolvedType::U8 => "u8",
                        ResolvedType::Usize => "usize",
                        ResolvedType::F32 => "f32",
                        ResolvedType::F64 => "f64",
                        ResolvedType::Bool => "bool",
                        ResolvedType::String => "string",
                        ResolvedType::Bytes => "bytes",
                        ResolvedType::Str => "str",
                        ResolvedType::SliceU8 => "slice-u8",
                        _ => unreachable!(),
                    };
                    let mut key = String::with_capacity(text.len());
                    key.push_str(text);
                    keys.push(key);
                }
                ResolvedType::ArrayU8(length) => {
                    let key_bytes = "array:u8:"
                        .len()
                        .checked_add(decimal_bytes(*length as usize))
                        .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
                    let mut key = String::with_capacity(key_bytes);
                    write!(key, "array:u8:{length}")
                        .map_err(|_| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
                    keys.push(key);
                }
                ResolvedType::TypeParameter { owner, index } => {
                    let key_bytes = type_identity_metrics(ty, 1)?.root_bytes;
                    let mut key = String::with_capacity(key_bytes);
                    write!(key, "parameter:{}:{}:{index}", owner.as_str().len(), owner)
                        .map_err(|_| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
                    keys.push(key);
                }
                ResolvedType::Nominal {
                    declaration,
                    arguments,
                } => {
                    let node = type_identity_metrics(ty, 1)?;
                    let encoded_bytes = arguments
                        .iter()
                        .try_fold(0usize, |bytes, argument| {
                            let child_bytes = type_identity_metrics(argument, 1).ok()?.root_bytes;
                            bytes
                                .checked_add(decimal_bytes(child_bytes))?
                                .checked_add(1)?
                                .checked_add(child_bytes)
                        })
                        .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
                    frames.push(TypeIdentityFrame::Finish(
                        declaration,
                        arguments.len(),
                        encoded_bytes,
                        node.root_bytes,
                    ));
                    frames.extend(arguments.iter().rev().map(TypeIdentityFrame::Enter));
                }
            },
            TypeIdentityFrame::Finish(declaration, count, encoded_bytes, result_bytes) => {
                let split = keys
                    .len()
                    .checked_sub(count)
                    .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
                let mut encoded = String::with_capacity(encoded_bytes);
                for key in &keys[split..] {
                    write!(encoded, "{}:{key}", key.len())
                        .map_err(|_| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
                }
                let mut result = String::with_capacity(result_bytes);
                write!(
                    result,
                    "nominal:{}:{}:{}:{}",
                    declaration.as_str().len(),
                    declaration,
                    count,
                    encoded
                )
                .map_err(|_| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
                #[cfg(test)]
                note_post_hir_facts_live(
                    _capacity_baseline,
                    _outer_scratch
                        .saturating_add(
                            frames
                                .capacity()
                                .saturating_mul(std::mem::size_of::<TypeIdentityFrame<'_>>()),
                        )
                        .saturating_add(
                            keys.capacity()
                                .saturating_mul(std::mem::size_of::<String>()),
                        )
                        .saturating_add(keys.iter().map(String::capacity).sum::<usize>())
                        .saturating_add(encoded.capacity())
                        .saturating_add(result.capacity()),
                );
                keys.truncate(split);
                keys.push(result);
            }
        }
        #[cfg(test)]
        note_post_hir_facts_live(
            _capacity_baseline,
            _outer_scratch
                .saturating_add(
                    frames
                        .capacity()
                        .saturating_mul(std::mem::size_of::<TypeIdentityFrame<'_>>()),
                )
                .saturating_add(
                    keys.capacity()
                        .saturating_mul(std::mem::size_of::<String>()),
                )
                .saturating_add(keys.iter().map(String::capacity).sum::<usize>()),
        );
    }
    if keys.len() != 1 {
        return Err(b109("max_builder_bytes", MAX_BUILDER_BYTES));
    }
    Ok(keys.pop().expect("one checked type identity"))
}

fn fingerprint_binding_type_scratch(
    binding: &crate::hir::ResolvedBinding,
) -> Result<usize, Diagnostic> {
    type_identity_scratch_upper(&binding.ty)
}

fn fingerprint_record_pattern_types_scratch(
    fields: &[crate::hir::ResolvedRecordMatchPatternField],
) -> Result<usize, Diagnostic> {
    fields.iter().try_fold(0usize, |maximum, field| {
        let current = match &field.pattern {
            crate::hir::ResolvedRecordMatchFieldPattern::Binding(binding) => {
                fingerprint_binding_type_scratch(binding)?
            }
            crate::hir::ResolvedRecordMatchFieldPattern::Wildcard => 0,
            crate::hir::ResolvedRecordMatchFieldPattern::Record {
                instance, fields, ..
            } => type_identity_scratch_upper(instance)?
                .max(fingerprint_record_pattern_types_scratch(fields)?),
        };
        Ok(maximum.max(current))
    })
}

fn fingerprint_pattern_types_scratch(
    pattern: &crate::hir::ResolvedMatchPattern,
) -> Result<usize, Diagnostic> {
    match pattern {
        crate::hir::ResolvedMatchPattern::Wildcard
        | crate::hir::ResolvedMatchPattern::Literal(_) => Ok(0),
        crate::hir::ResolvedMatchPattern::Binding(binding) => {
            fingerprint_binding_type_scratch(binding)
        }
        crate::hir::ResolvedMatchPattern::Or(alternatives) => alternatives
            .iter()
            .try_fold(0usize, |maximum, alternative| {
                Ok(maximum.max(fingerprint_pattern_types_scratch(alternative)?))
            }),
        crate::hir::ResolvedMatchPattern::Variant { fields, .. } => {
            fields.iter().try_fold(0usize, |maximum, field| {
                Ok(maximum.max(fingerprint_binding_type_scratch(&field.binding)?))
            })
        }
        crate::hir::ResolvedMatchPattern::Record {
            instance, fields, ..
        } => Ok(type_identity_scratch_upper(instance)?
            .max(fingerprint_record_pattern_types_scratch(fields)?)),
    }
}

pub(super) fn fingerprint_expression_types_scratch(
    expression: &ResolvedExpr,
    depth: usize,
) -> Result<usize, Diagnostic> {
    #[derive(Clone, Copy)]
    enum Frame<'a> {
        Expr(&'a ResolvedExpr, usize),
        Exprs(&'a [ResolvedExpr], usize, usize),
        Statements(&'a [ResolvedStatement], usize, usize),
        Fields(&'a [crate::hir::ResolvedFieldInitializer], usize, usize),
        Arms(&'a [crate::hir::ResolvedMatchArm], usize, usize),
    }
    fn push<'a>(
        stack: &mut [Option<Frame<'a>>],
        stack_len: &mut usize,
        frame: Frame<'a>,
    ) -> Result<(), Diagnostic> {
        let slot = stack.get_mut(*stack_len).ok_or_else(|| {
            b109(
                "max_semantic_expression_depth",
                MAX_SEMANTIC_EXPRESSION_DEPTH,
            )
        })?;
        *slot = Some(frame);
        *stack_len += 1;
        Ok(())
    }

    let mut stack = [None; FINGERPRINT_ACTION_SLOTS];
    let mut stack_len = 0usize;
    push(&mut stack, &mut stack_len, Frame::Expr(expression, depth))?;
    let mut maximum = 0usize;
    while stack_len > 0 {
        stack_len -= 1;
        let frame = stack[stack_len]
            .take()
            .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
        match frame {
            Frame::Expr(expression, depth) => {
                if depth > MAX_SEMANTIC_EXPRESSION_DEPTH {
                    return Err(b109(
                        "max_semantic_expression_depth",
                        MAX_SEMANTIC_EXPRESSION_DEPTH,
                    ));
                }
                let child_depth = depth.checked_add(1).ok_or_else(|| {
                    b109(
                        "max_semantic_expression_depth",
                        MAX_SEMANTIC_EXPRESSION_DEPTH,
                    )
                })?;
                maximum = maximum.max(type_identity_scratch_upper(&expression.ty)?);
                match &expression.kind {
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
                    | ResolvedExprKind::BorrowPlace { .. } => {}
                    ResolvedExprKind::Call {
                        type_arguments,
                        args,
                        ..
                    } => {
                        for ty in type_arguments {
                            maximum = maximum.max(type_identity_scratch_upper(ty)?);
                        }
                        push(
                            &mut stack,
                            &mut stack_len,
                            Frame::Exprs(args, 0, child_depth),
                        )?;
                    }
                    ResolvedExprKind::NativeRustImportCall(call) => push(
                        &mut stack,
                        &mut stack_len,
                        Frame::Exprs(&call.args, 0, child_depth),
                    )?,
                    ResolvedExprKind::HostCommandCall(call) => push(
                        &mut stack,
                        &mut stack_len,
                        Frame::Exprs(&call.args, 0, child_depth),
                    )?,
                    ResolvedExprKind::Unary { value, .. } => {
                        push(&mut stack, &mut stack_len, Frame::Expr(value, child_depth))?
                    }
                    ResolvedExprKind::Binary { left, right, .. } => {
                        push(&mut stack, &mut stack_len, Frame::Expr(right, child_depth))?;
                        push(&mut stack, &mut stack_len, Frame::Expr(left, child_depth))?;
                    }
                    ResolvedExprKind::Block { statements, tail } => {
                        push(&mut stack, &mut stack_len, Frame::Expr(tail, child_depth))?;
                        push(
                            &mut stack,
                            &mut stack_len,
                            Frame::Statements(statements, 0, child_depth),
                        )?;
                    }
                    ResolvedExprKind::If {
                        condition,
                        then_branch,
                        else_branch,
                    } => {
                        push(
                            &mut stack,
                            &mut stack_len,
                            Frame::Expr(else_branch, child_depth),
                        )?;
                        push(
                            &mut stack,
                            &mut stack_len,
                            Frame::Expr(then_branch, child_depth),
                        )?;
                        push(
                            &mut stack,
                            &mut stack_len,
                            Frame::Expr(condition, child_depth),
                        )?;
                    }
                    ResolvedExprKind::ConstructRecord { fields, .. }
                    | ResolvedExprKind::ConstructVariant { fields, .. } => push(
                        &mut stack,
                        &mut stack_len,
                        Frame::Fields(fields, 0, child_depth),
                    )?,
                    ResolvedExprKind::Match { scrutinee, arms } => {
                        push(
                            &mut stack,
                            &mut stack_len,
                            Frame::Arms(arms, 0, child_depth),
                        )?;
                        push(
                            &mut stack,
                            &mut stack_len,
                            Frame::Expr(scrutinee, child_depth),
                        )?;
                    }
                    ResolvedExprKind::Try {
                        operand,
                        residual_type,
                        ..
                    }
                    | ResolvedExprKind::TryOption {
                        operand,
                        residual_type,
                        ..
                    } => {
                        maximum = maximum.max(type_identity_scratch_upper(residual_type)?);
                        push(
                            &mut stack,
                            &mut stack_len,
                            Frame::Expr(operand, child_depth),
                        )?;
                    }
                    ResolvedExprKind::UpdateRecord { base, fields, .. } => {
                        push(
                            &mut stack,
                            &mut stack_len,
                            Frame::Fields(fields, 0, child_depth),
                        )?;
                        push(&mut stack, &mut stack_len, Frame::Expr(base, child_depth))?;
                    }
                    ResolvedExprKind::Project { base, .. } => {
                        push(&mut stack, &mut stack_len, Frame::Expr(base, child_depth))?
                    }
                    ResolvedExprKind::Upcast { source } => {
                        push(&mut stack, &mut stack_len, Frame::Expr(source, child_depth))?
                    }
                }
            }
            Frame::Exprs(expressions, index, depth) => {
                if let Some(expression) = expressions.get(index) {
                    push(
                        &mut stack,
                        &mut stack_len,
                        Frame::Exprs(expressions, index + 1, depth),
                    )?;
                    push(&mut stack, &mut stack_len, Frame::Expr(expression, depth))?;
                }
            }
            Frame::Statements(statements, index, depth) => {
                if let Some(statement) = statements.get(index) {
                    push(
                        &mut stack,
                        &mut stack_len,
                        Frame::Statements(statements, index + 1, depth),
                    )?;
                    match statement {
                        ResolvedStatement::Let { binding, value, .. }
                        | ResolvedStatement::Assign { binding, value, .. } => {
                            maximum = maximum.max(fingerprint_binding_type_scratch(binding)?);
                            push(&mut stack, &mut stack_len, Frame::Expr(value, depth))?;
                        }
                        ResolvedStatement::Unsafe { body, .. } => {
                            push(&mut stack, &mut stack_len, Frame::Expr(body, depth))?;
                        }
                        ResolvedStatement::While {
                            condition, body, ..
                        } => {
                            push(&mut stack, &mut stack_len, Frame::Expr(body, depth))?;
                            push(&mut stack, &mut stack_len, Frame::Expr(condition, depth))?;
                        }
                    }
                }
            }
            Frame::Fields(fields, index, depth) => {
                if let Some(field) = fields.get(index) {
                    push(
                        &mut stack,
                        &mut stack_len,
                        Frame::Fields(fields, index + 1, depth),
                    )?;
                    push(&mut stack, &mut stack_len, Frame::Expr(&field.value, depth))?;
                }
            }
            Frame::Arms(arms, index, depth) => {
                if let Some(arm) = arms.get(index) {
                    maximum = maximum.max(fingerprint_pattern_types_scratch(&arm.pattern)?);
                    push(
                        &mut stack,
                        &mut stack_len,
                        Frame::Arms(arms, index + 1, depth),
                    )?;
                    push(&mut stack, &mut stack_len, Frame::Expr(&arm.value, depth))?;
                    if let Some(guard) = &arm.guard {
                        push(&mut stack, &mut stack_len, Frame::Expr(guard, depth))?;
                    }
                }
            }
        }
    }
    Ok(maximum)
}

pub(super) fn fingerprint_type_scratch_upper(
    closure: &[&ResolvedFunction],
) -> Result<usize, Diagnostic> {
    closure.iter().try_fold(0usize, |mut maximum, function| {
        maximum = maximum.max(type_identity_scratch_upper(&function.return_type)?);
        for parameter in &function.params {
            maximum = maximum.max(type_identity_scratch_upper(&parameter.ty)?);
        }
        for expression in function
            .requires
            .iter()
            .chain(std::iter::once(&function.body))
            .chain(&function.ensures)
        {
            maximum = maximum.max(fingerprint_expression_types_scratch(expression, 1)?);
        }
        Ok(maximum)
    })
}

pub(super) fn hir_fingerprint(
    closure: &[&ResolvedFunction],
    imports: &[ImportFact],
    _capacity_baseline: usize,
) -> Result<String, Diagnostic> {
    let mut hasher = Sha256::new();
    hasher.update(HIR_DOMAIN);
    hash_count(&mut hasher, "functions", closure.len());
    for function in closure {
        frame(&mut hasher, b"function");
        frame(&mut hasher, function.id.as_str().as_bytes());
        frame(&mut hasher, function.name.as_bytes());
        frame(&mut hasher, function.result_id.as_str().as_bytes());
        frame(&mut hasher, function.body.id.as_str().as_bytes());
        let return_identity =
            fingerprint_type_identity(&function.return_type, _capacity_baseline, 0)?;
        #[cfg(test)]
        note_post_hir_facts_live(_capacity_baseline, return_identity.capacity());
        frame(&mut hasher, return_identity.as_bytes());
        hash_count(&mut hasher, "effects", function.effects.len());
        for effect in &function.effects {
            frame(&mut hasher, effect.as_bytes());
        }
        hash_count(&mut hasher, "parameters", function.params.len());
        for parameter in &function.params {
            frame(&mut hasher, parameter.id.as_str().as_bytes());
            frame(&mut hasher, parameter.name.as_bytes());
            frame(
                &mut hasher,
                match parameter.ownership {
                    OwnershipMode::Value => b"value",
                    OwnershipMode::Own => b"own",
                    OwnershipMode::Borrow => b"borrow",
                    OwnershipMode::Shared => b"shared",
                },
            );
            let parameter_identity =
                fingerprint_type_identity(&parameter.ty, _capacity_baseline, 0)?;
            #[cfg(test)]
            note_post_hir_facts_live(_capacity_baseline, parameter_identity.capacity());
            frame(&mut hasher, parameter_identity.as_bytes());
        }
        hash_count(&mut hasher, "requires", function.requires.len());
        for requirement in &function.requires {
            hash_expr(&mut hasher, requirement, _capacity_baseline)?;
        }
        frame(&mut hasher, b"body");
        hash_expr(&mut hasher, &function.body, _capacity_baseline)?;
        hash_count(&mut hasher, "ensures", function.ensures.len());
        for guarantee in &function.ensures {
            hash_expr(&mut hasher, guarantee, _capacity_baseline)?;
        }
    }
    hash_count(&mut hasher, "imports", imports.len());
    for import in imports {
        frame(&mut hasher, b"import");
        frame(&mut hasher, import.id.as_bytes());
        frame(&mut hasher, import.interface.as_bytes());
        frame(&mut hasher, import.import_key.as_bytes());
        frame(&mut hasher, scalar_text(import.result).as_bytes());
        hash_count(&mut hasher, "import-parameters", import.parameters.len());
        for parameter in &import.parameters {
            frame(&mut hasher, parameter.name.as_bytes());
            frame(&mut hasher, scalar_text(parameter.ty).as_bytes());
        }
        hash_count(&mut hasher, "import-effects", import.effects.len());
        for effect in &import.effects {
            frame(&mut hasher, effect.as_bytes());
        }
        frame(
            &mut hasher,
            import.failure.as_deref().unwrap_or("infallible").as_bytes(),
        );
        frame(&mut hasher, import.call_contract_digest.as_bytes());
    }
    let digest = format!(
        "sha256:{:x}",
        semaprax::digest_hex::LowerHex(hasher.finalize())
    );
    #[cfg(test)]
    note_post_hir_facts_live(_capacity_baseline, digest.capacity());
    Ok(digest)
}

fn hash_count(hasher: &mut Sha256, label: &str, count: usize) {
    frame(hasher, label.as_bytes());
    frame(
        hasher,
        &u64::try_from(count).unwrap_or(u64::MAX).to_be_bytes(),
    );
}

pub(super) enum HirFingerprintAction<'a> {
    Expr(&'a ResolvedExpr, usize),
    Exprs(&'a [ResolvedExpr], usize, usize),
    Statement(&'a ResolvedStatement, usize),
    Statements(&'a [ResolvedStatement], usize, usize),
    Field(&'a crate::hir::ResolvedFieldInitializer, usize),
    Fields(&'a [crate::hir::ResolvedFieldInitializer], usize, usize),
    Pattern(&'a crate::hir::ResolvedMatchPattern),
    Patterns(&'a [crate::hir::ResolvedMatchPattern], usize),
    RecordPatternField(&'a crate::hir::ResolvedRecordMatchPatternField),
    RecordPatternFields(&'a [crate::hir::ResolvedRecordMatchPatternField], usize),
    Arms(&'a [crate::hir::ResolvedMatchArm], usize, usize),
    Arm(&'a crate::hir::ResolvedMatchArm, usize),
    TryIds([&'a DeclarationId; 5], usize),
    OptionIds([&'a DeclarationId; 4], usize),
    Bytes(&'a [u8]),
    Type(&'a ResolvedType),
}

pub(super) fn hash_expr(
    hasher: &mut Sha256,
    expression: &ResolvedExpr,
    _capacity_baseline: usize,
) -> Result<(), Diagnostic> {
    let ownership = |ownership| match ownership {
        OwnershipMode::Value => b"value".as_slice(),
        OwnershipMode::Own => b"own".as_slice(),
        OwnershipMode::Borrow => b"borrow".as_slice(),
        OwnershipMode::Shared => b"shared".as_slice(),
    };
    let mut actions = Vec::with_capacity(MAX_SEMANTIC_EXPRESSION_DEPTH * 4 + 8);
    actions.push(HirFingerprintAction::Expr(expression, 1));
    while let Some(action) = actions.pop() {
        if actions.len() + 4 > actions.capacity() {
            return Err(b109(
                "max_semantic_expression_depth",
                MAX_SEMANTIC_EXPRESSION_DEPTH,
            ));
        }
        match action {
            HirFingerprintAction::Bytes(value) => frame(hasher, value),
            HirFingerprintAction::Type(ty) => {
                let action_bytes = actions
                    .capacity()
                    .saturating_mul(std::mem::size_of::<HirFingerprintAction<'_>>());
                let identity = fingerprint_type_identity(ty, _capacity_baseline, action_bytes)?;
                #[cfg(test)]
                note_post_hir_facts_live(
                    _capacity_baseline,
                    actions
                        .capacity()
                        .saturating_mul(std::mem::size_of::<HirFingerprintAction<'_>>())
                        .saturating_add(identity.capacity()),
                );
                frame(hasher, identity.as_bytes());
            }
            HirFingerprintAction::Statement(statement, depth) => {
                match statement {
                    ResolvedStatement::Let { binding, value, .. }
                    | ResolvedStatement::Assign { binding, value, .. } => {
                        // These tags and the following byte sequence preserve
                        // the pre-While fingerprint contract exactly.
                        frame(
                            hasher,
                            if matches!(statement, ResolvedStatement::Assign { .. }) {
                                b"assign"
                            } else {
                                b"let"
                            },
                        );
                        frame(hasher, binding.id.as_str().as_bytes());
                        frame(hasher, binding.name.as_bytes());
                        let action_bytes = actions
                            .capacity()
                            .saturating_mul(std::mem::size_of::<HirFingerprintAction<'_>>());
                        let binding_identity = fingerprint_type_identity(
                            &binding.ty,
                            _capacity_baseline,
                            action_bytes,
                        )?;
                        #[cfg(test)]
                        note_post_hir_facts_live(
                            _capacity_baseline,
                            actions
                                .capacity()
                                .saturating_mul(std::mem::size_of::<HirFingerprintAction<'_>>())
                                .saturating_add(binding_identity.capacity()),
                        );
                        frame(hasher, binding_identity.as_bytes());
                        frame(hasher, ownership(binding.ownership));
                        actions.push(HirFingerprintAction::Expr(value, depth));
                    }
                    ResolvedStatement::Unsafe { audit, body, .. } => {
                        frame(hasher, b"unsafe");
                        frame(hasher, audit.as_bytes());
                        actions.push(HirFingerprintAction::Expr(body, depth));
                    }
                    ResolvedStatement::While {
                        condition, body, ..
                    } => {
                        frame(hasher, b"while");
                        actions.push(HirFingerprintAction::Expr(body, depth));
                        actions.push(HirFingerprintAction::Expr(condition, depth));
                    }
                }
            }
            HirFingerprintAction::Statements(statements, index, depth) => {
                if let Some(statement) = statements.get(index) {
                    actions.push(HirFingerprintAction::Statements(
                        statements,
                        index + 1,
                        depth,
                    ));
                    actions.push(HirFingerprintAction::Statement(statement, depth));
                }
            }
            HirFingerprintAction::Exprs(expressions, index, depth) => {
                if let Some(expression) = expressions.get(index) {
                    actions.push(HirFingerprintAction::Exprs(expressions, index + 1, depth));
                    actions.push(HirFingerprintAction::Expr(expression, depth));
                }
            }
            HirFingerprintAction::TryIds(ids, index) => {
                if let Some(id) = ids.get(index) {
                    actions.push(HirFingerprintAction::TryIds(ids, index + 1));
                    actions.push(HirFingerprintAction::Bytes(id.as_str().as_bytes()));
                }
            }
            HirFingerprintAction::OptionIds(ids, index) => {
                if let Some(id) = ids.get(index) {
                    actions.push(HirFingerprintAction::OptionIds(ids, index + 1));
                    actions.push(HirFingerprintAction::Bytes(id.as_str().as_bytes()));
                }
            }
            HirFingerprintAction::Field(field, depth) => {
                frame(hasher, field.field.as_str().as_bytes());
                actions.push(HirFingerprintAction::Expr(&field.value, depth));
            }
            HirFingerprintAction::Fields(fields, index, depth) => {
                if index == 0 {
                    hash_count(hasher, "fields", fields.len());
                }
                if let Some(field) = fields.get(index) {
                    actions.push(HirFingerprintAction::Fields(fields, index + 1, depth));
                    actions.push(HirFingerprintAction::Field(field, depth));
                }
            }
            HirFingerprintAction::Arms(arms, index, depth) => {
                if index == 0 {
                    hash_count(hasher, "arms", arms.len());
                }
                if let Some(arm) = arms.get(index) {
                    actions.push(HirFingerprintAction::Arms(arms, index + 1, depth));
                    actions.push(HirFingerprintAction::Arm(arm, depth));
                }
            }
            HirFingerprintAction::Arm(arm, depth) => {
                actions.push(HirFingerprintAction::Expr(&arm.value, depth));
                if let Some(guard) = &arm.guard {
                    actions.push(HirFingerprintAction::Expr(guard, depth));
                    actions.push(HirFingerprintAction::Bytes(b"guard"));
                }
                actions.push(HirFingerprintAction::Pattern(&arm.pattern));
            }
            HirFingerprintAction::RecordPatternFields(fields, index) => {
                if let Some(field) = fields.get(index) {
                    actions.push(HirFingerprintAction::RecordPatternFields(fields, index + 1));
                    actions.push(HirFingerprintAction::RecordPatternField(field));
                }
            }
            HirFingerprintAction::RecordPatternField(field) => {
                frame(hasher, field.field.as_str().as_bytes());
                match &field.pattern {
                    crate::hir::ResolvedRecordMatchFieldPattern::Binding(binding) => {
                        frame(hasher, b"binding");
                        hash_binding(
                            hasher,
                            binding,
                            _capacity_baseline,
                            actions
                                .capacity()
                                .saturating_mul(std::mem::size_of::<HirFingerprintAction<'_>>()),
                        )?;
                    }
                    crate::hir::ResolvedRecordMatchFieldPattern::Wildcard => {
                        frame(hasher, b"wildcard");
                    }
                    crate::hir::ResolvedRecordMatchFieldPattern::Record {
                        record,
                        instance,
                        fields,
                    } => {
                        frame(hasher, b"record");
                        frame(hasher, record.as_str().as_bytes());
                        let action_bytes = actions
                            .capacity()
                            .saturating_mul(std::mem::size_of::<HirFingerprintAction<'_>>());
                        let instance_identity =
                            fingerprint_type_identity(instance, _capacity_baseline, action_bytes)?;
                        #[cfg(test)]
                        note_post_hir_facts_live(
                            _capacity_baseline,
                            actions
                                .capacity()
                                .saturating_mul(std::mem::size_of::<HirFingerprintAction<'_>>())
                                .saturating_add(instance_identity.capacity()),
                        );
                        frame(hasher, instance_identity.as_bytes());
                        hash_count(hasher, "record-pattern-fields", fields.len());
                        actions.push(HirFingerprintAction::RecordPatternFields(fields, 0));
                    }
                }
            }
            HirFingerprintAction::Pattern(pattern) => match pattern {
                crate::hir::ResolvedMatchPattern::Wildcard => frame(hasher, b"wildcard"),
                // Refutable Match v1: deterministic fingerprints for the
                // additive pattern spellings.
                crate::hir::ResolvedMatchPattern::Literal(value) => {
                    frame(hasher, b"literal");
                    match value {
                        crate::hir::PatternValue::Int(inner) => {
                            frame(hasher, b"int");
                            frame(hasher, inner.to_le_bytes().as_slice());
                        }
                        crate::hir::PatternValue::Int32(inner) => {
                            frame(hasher, b"int32");
                            frame(hasher, inner.to_le_bytes().as_slice());
                        }
                        crate::hir::PatternValue::Uint8(inner) => {
                            frame(hasher, b"uint8");
                            frame(hasher, [*inner].as_slice());
                        }
                        crate::hir::PatternValue::Usize(inner) => {
                            frame(hasher, b"usize");
                            frame(hasher, inner.to_le_bytes().as_slice());
                        }
                        crate::hir::PatternValue::Char(inner) => {
                            frame(hasher, b"char");
                            frame(hasher, inner.to_le_bytes().as_slice());
                        }
                        crate::hir::PatternValue::Bool(inner) => {
                            frame(hasher, b"bool");
                            frame(hasher, [*inner as u8].as_slice());
                        }
                    }
                }
                crate::hir::ResolvedMatchPattern::Or(alternatives) => {
                    frame(hasher, b"or");
                    hash_count(hasher, "or-alternatives", alternatives.len());
                    actions.push(HirFingerprintAction::Patterns(alternatives, 0));
                }
                crate::hir::ResolvedMatchPattern::Binding(binding) => {
                    frame(hasher, b"binding");
                    hash_binding(
                        hasher,
                        binding,
                        _capacity_baseline,
                        actions
                            .capacity()
                            .saturating_mul(std::mem::size_of::<HirFingerprintAction<'_>>()),
                    )?;
                }
                crate::hir::ResolvedMatchPattern::Variant {
                    variant,
                    case,
                    fields,
                } => {
                    frame(hasher, b"variant");
                    frame(hasher, variant.as_str().as_bytes());
                    frame(hasher, case.as_str().as_bytes());
                    hash_count(hasher, "variant-pattern-fields", fields.len());
                    for field in fields {
                        frame(hasher, field.field.as_str().as_bytes());
                        hash_binding(
                            hasher,
                            &field.binding,
                            _capacity_baseline,
                            actions
                                .capacity()
                                .saturating_mul(std::mem::size_of::<HirFingerprintAction<'_>>()),
                        )?;
                    }
                }
                crate::hir::ResolvedMatchPattern::Record {
                    record,
                    instance,
                    fields,
                } => {
                    frame(hasher, b"record");
                    frame(hasher, record.as_str().as_bytes());
                    let action_bytes = actions
                        .capacity()
                        .saturating_mul(std::mem::size_of::<HirFingerprintAction<'_>>());
                    let instance_identity =
                        fingerprint_type_identity(instance, _capacity_baseline, action_bytes)?;
                    #[cfg(test)]
                    note_post_hir_facts_live(
                        _capacity_baseline,
                        actions
                            .capacity()
                            .saturating_mul(std::mem::size_of::<HirFingerprintAction<'_>>())
                            .saturating_add(instance_identity.capacity()),
                    );
                    frame(hasher, instance_identity.as_bytes());
                    hash_count(hasher, "record-pattern-fields", fields.len());
                    actions.push(HirFingerprintAction::RecordPatternFields(fields, 0));
                }
            },
            HirFingerprintAction::Patterns(patterns, index) => {
                if let Some(pattern) = patterns.get(index) {
                    actions.push(HirFingerprintAction::Patterns(patterns, index + 1));
                    actions.push(HirFingerprintAction::Pattern(pattern));
                }
            }
            HirFingerprintAction::Expr(expression, depth) => {
                if depth > MAX_SEMANTIC_EXPRESSION_DEPTH {
                    return Err(b109(
                        "max_semantic_expression_depth",
                        MAX_SEMANTIC_EXPRESSION_DEPTH,
                    ));
                }
                let child_depth = depth.checked_add(1).ok_or_else(|| {
                    b109(
                        "max_semantic_expression_depth",
                        MAX_SEMANTIC_EXPRESSION_DEPTH,
                    )
                })?;
                frame(hasher, b"expression");
                frame(hasher, expression.id.as_str().as_bytes());
                let action_bytes = actions
                    .capacity()
                    .saturating_mul(std::mem::size_of::<HirFingerprintAction<'_>>());
                let identity =
                    fingerprint_type_identity(&expression.ty, _capacity_baseline, action_bytes)?;
                #[cfg(test)]
                note_post_hir_facts_live(
                    _capacity_baseline,
                    actions
                        .capacity()
                        .saturating_mul(std::mem::size_of::<HirFingerprintAction<'_>>())
                        .saturating_add(identity.capacity()),
                );
                frame(hasher, identity.as_bytes());
                frame(hasher, ownership(expression.ownership));
                match &expression.kind {
                    ResolvedExprKind::Int32(_)
                    | ResolvedExprKind::Char(_)
                    | ResolvedExprKind::Uint8(_)
                    | ResolvedExprKind::Usize(_)
                    | ResolvedExprKind::ArrayU8(_)
                    | ResolvedExprKind::RepeatArrayU8 { .. }
                    | ResolvedExprKind::Float32(_)
                    | ResolvedExprKind::Float64(_)
                    | ResolvedExprKind::String(_)
                    | ResolvedExprKind::BorrowPlace { .. } => {
                        // Non-i64 scalar signatures are outside the scalar
                        // native boundary; admission rejects them first.
                        return Err(b107("scalar value signature required"));
                    }
                    ResolvedExprKind::Int(value) => {
                        frame(hasher, b"int");
                        frame(hasher, &value.to_be_bytes());
                    }
                    ResolvedExprKind::Bool(value) => {
                        frame(hasher, b"bool");
                        frame(hasher, &[*value as u8]);
                    }
                    ResolvedExprKind::Place(place) => {
                        frame(hasher, b"place");
                        frame(hasher, place.root.as_str().as_bytes());
                        hash_count(hasher, "projections", place.projections.len());
                        for projection in &place.projections {
                            match projection {
                                crate::hir::PlaceProjection::Field(field) => {
                                    frame(hasher, b"field");
                                    frame(hasher, field.as_str().as_bytes());
                                }
                                crate::hir::PlaceProjection::VariantField { case, field } => {
                                    frame(hasher, b"variant-field");
                                    frame(hasher, case.as_str().as_bytes());
                                    frame(hasher, field.as_str().as_bytes());
                                }
                            }
                        }
                    }
                    ResolvedExprKind::Call {
                        callee,
                        type_arguments,
                        instance,
                        args,
                    } => {
                        frame(hasher, b"call");
                        frame(hasher, callee.as_str().as_bytes());
                        hash_count(hasher, "type-arguments", type_arguments.len());
                        for argument in type_arguments {
                            let action_bytes = actions
                                .capacity()
                                .saturating_mul(std::mem::size_of::<HirFingerprintAction<'_>>());
                            let argument_identity = fingerprint_type_identity(
                                argument,
                                _capacity_baseline,
                                action_bytes,
                            )?;
                            #[cfg(test)]
                            note_post_hir_facts_live(
                                _capacity_baseline,
                                actions
                                    .capacity()
                                    .saturating_mul(std::mem::size_of::<HirFingerprintAction<'_>>())
                                    .saturating_add(argument_identity.capacity()),
                            );
                            frame(hasher, argument_identity.as_bytes());
                        }
                        frame(
                            hasher,
                            instance
                                .as_ref()
                                .map_or(b"".as_slice(), |value| value.as_str().as_bytes()),
                        );
                        hash_count(hasher, "arguments", args.len());
                        actions.push(HirFingerprintAction::Exprs(args, 0, child_depth));
                    }
                    ResolvedExprKind::NativeRustImportCall(call) => {
                        frame(hasher, b"native-rust-import");
                        frame(hasher, call.expression.as_str().as_bytes());
                        frame(hasher, call.import.as_str().as_bytes());
                        frame(
                            hasher,
                            match call.result {
                                ResolvedImportResultKind::Unit => b"unit",
                                ResolvedImportResultKind::I64 => b"i64",
                                ResolvedImportResultKind::Bool => b"bool",
                            },
                        );
                        hash_count(hasher, "arguments", call.args.len());
                        actions.push(HirFingerprintAction::Exprs(&call.args, 0, child_depth));
                    }
                    ResolvedExprKind::HostCommandCall(_) => {
                        // Public Native Rust SDKs do not carry invocation
                        // command-input/output authority.
                        return Err(b107("scalar value signature required"));
                    }
                    ResolvedExprKind::Unary { op, value } => {
                        frame(
                            hasher,
                            match op {
                                crate::ast::UnaryOp::Neg => b"unary-neg",
                                crate::ast::UnaryOp::Not => b"unary-not",
                            },
                        );
                        actions.push(HirFingerprintAction::Expr(value, child_depth));
                    }
                    ResolvedExprKind::Binary { op, left, right } => {
                        frame(
                            hasher,
                            match op {
                                crate::ast::BinaryOp::Add => b"binary-add",
                                crate::ast::BinaryOp::Sub => b"binary-sub",
                                crate::ast::BinaryOp::Mul => b"binary-mul",
                                crate::ast::BinaryOp::Div => b"binary-div",
                                crate::ast::BinaryOp::Rem => b"binary-rem",
                                crate::ast::BinaryOp::Eq => b"binary-eq",
                                crate::ast::BinaryOp::Ne => b"binary-ne",
                                crate::ast::BinaryOp::Lt => b"binary-lt",
                                crate::ast::BinaryOp::Le => b"binary-le",
                                crate::ast::BinaryOp::Gt => b"binary-gt",
                                crate::ast::BinaryOp::Ge => b"binary-ge",
                                crate::ast::BinaryOp::And => b"binary-and",
                                crate::ast::BinaryOp::Or => b"binary-or",
                            },
                        );
                        actions.push(HirFingerprintAction::Expr(right, child_depth));
                        actions.push(HirFingerprintAction::Expr(left, child_depth));
                    }
                    ResolvedExprKind::Block { statements, tail } => {
                        frame(hasher, b"block");
                        hash_count(hasher, "statements", statements.len());
                        actions.push(HirFingerprintAction::Expr(tail, child_depth));
                        actions.push(HirFingerprintAction::Statements(statements, 0, child_depth));
                    }
                    ResolvedExprKind::If {
                        condition,
                        then_branch,
                        else_branch,
                    } => {
                        frame(hasher, b"if");
                        actions.push(HirFingerprintAction::Expr(else_branch, child_depth));
                        actions.push(HirFingerprintAction::Expr(then_branch, child_depth));
                        actions.push(HirFingerprintAction::Expr(condition, child_depth));
                    }
                    ResolvedExprKind::ConstructRecord { record, fields } => {
                        frame(hasher, b"construct-record");
                        frame(hasher, record.as_str().as_bytes());
                        actions.push(HirFingerprintAction::Fields(fields, 0, child_depth));
                    }
                    ResolvedExprKind::ConstructVariant {
                        variant,
                        case,
                        fields,
                    } => {
                        frame(hasher, b"construct-variant");
                        frame(hasher, variant.as_str().as_bytes());
                        frame(hasher, case.as_str().as_bytes());
                        actions.push(HirFingerprintAction::Fields(fields, 0, child_depth));
                    }
                    ResolvedExprKind::Match { scrutinee, arms } => {
                        frame(hasher, b"match");
                        actions.push(HirFingerprintAction::Arms(arms, 0, child_depth));
                        actions.push(HirFingerprintAction::Expr(scrutinee, child_depth));
                    }
                    ResolvedExprKind::Try {
                        operand,
                        result,
                        ok_case,
                        ok_field,
                        err_case,
                        err_field,
                        residual_type,
                    } => {
                        frame(hasher, b"try");
                        let ids = [result, ok_case, ok_field, err_case, err_field];
                        actions.push(HirFingerprintAction::Type(residual_type));
                        actions.push(HirFingerprintAction::TryIds(ids, 0));
                        actions.push(HirFingerprintAction::Expr(operand, child_depth));
                    }
                    ResolvedExprKind::TryOption {
                        operand,
                        option,
                        some_case,
                        some_field,
                        none_case,
                        residual_type,
                    } => {
                        frame(hasher, b"try-option");
                        let ids = [option, some_case, some_field, none_case];
                        actions.push(HirFingerprintAction::Type(residual_type));
                        actions.push(HirFingerprintAction::OptionIds(ids, 0));
                        actions.push(HirFingerprintAction::Expr(operand, child_depth));
                    }
                    ResolvedExprKind::UpdateRecord {
                        base,
                        record,
                        fields,
                    } => {
                        frame(hasher, b"update-record");
                        actions.push(HirFingerprintAction::Fields(fields, 0, child_depth));
                        actions.push(HirFingerprintAction::Bytes(record.as_str().as_bytes()));
                        actions.push(HirFingerprintAction::Expr(base, child_depth));
                    }
                    ResolvedExprKind::Project { base, field } => {
                        frame(hasher, b"project");
                        actions.push(HirFingerprintAction::Bytes(field.as_str().as_bytes()));
                        actions.push(HirFingerprintAction::Expr(base, child_depth));
                    }
                    ResolvedExprKind::Upcast { source } => {
                        frame(hasher, b"upcast");
                        actions.push(HirFingerprintAction::Expr(source, child_depth));
                    }
                }
            }
        }
        #[cfg(test)]
        note_post_hir_facts_live(
            _capacity_baseline,
            actions
                .capacity()
                .saturating_mul(std::mem::size_of::<HirFingerprintAction<'_>>()),
        );
    }
    Ok(())
}

fn hash_binding(
    hasher: &mut Sha256,
    binding: &crate::hir::ResolvedBinding,
    _capacity_baseline: usize,
    _action_bytes: usize,
) -> Result<(), Diagnostic> {
    frame(hasher, binding.id.as_str().as_bytes());
    frame(hasher, binding.name.as_bytes());
    let binding_identity =
        fingerprint_type_identity(&binding.ty, _capacity_baseline, _action_bytes)?;
    #[cfg(test)]
    note_post_hir_facts_live(
        _capacity_baseline,
        _action_bytes.saturating_add(binding_identity.capacity()),
    );
    frame(hasher, binding_identity.as_bytes());
    frame(
        hasher,
        match binding.ownership {
            OwnershipMode::Value => b"value",
            OwnershipMode::Own => b"own",
            OwnershipMode::Borrow => b"borrow",
            OwnershipMode::Shared => b"shared",
        },
    );
    Ok(())
}
