//! Selected-export closure, transitive import selection, and the
//! parameter facts derived from the resolved program.

use super::*;

#[cfg(test)]
pub(in crate::implementation) fn btree_allocation_upper<K, V>(len: usize) -> usize {
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
pub(super) fn declaration_set_capacity(set: &BTreeSet<DeclarationId>) -> usize {
    btree_allocation_upper::<DeclarationId, ()>(set.len())
        .saturating_add(set.iter().map(|id| id.as_str().len()).sum::<usize>())
}

pub(in crate::implementation) struct SelectedClosureFrame {
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

pub(in crate::implementation) fn selected_closure<'a>(
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

pub(in crate::implementation) fn transitive_imports(
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

pub(in crate::implementation) fn parameter_facts(
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

pub(in crate::implementation) fn import_parameter_facts(
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
