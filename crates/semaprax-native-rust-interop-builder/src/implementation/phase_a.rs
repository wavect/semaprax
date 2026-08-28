use super::*;

/// Pure private phase-A preparation. It performs no filesystem, process, or
/// network operation.
pub(super) fn prepare_native_rust_interop(
    program: &Program,
    spec_bytes: &[u8],
) -> Result<PreparedNativeRustInterop, Vec<Diagnostic>> {
    let result = crate::bounded_output::with_limit(MAX_BUILDER_BYTES, || {
        prepare_native_rust_interop_bounded(program, spec_bytes)
    });
    if result.1 {
        return Err(vec![b109("max_builder_bytes", MAX_BUILDER_BYTES)]);
    }
    result.0.map_err(|error| vec![error])
}

#[cfg(test)]
pub(super) fn prepare_native_rust_interop_with_test_limit(
    program: &Program,
    spec_bytes: &[u8],
    limit: usize,
) -> Result<PreparedNativeRustInterop, Vec<Diagnostic>> {
    assert!(limit <= MAX_BUILDER_BYTES);
    let (result, overflowed) = crate::bounded_output::with_limit(limit, || {
        prepare_native_rust_interop_bounded(program, spec_bytes)
    });
    if overflowed {
        return Err(vec![b109("max_builder_bytes", MAX_BUILDER_BYTES)]);
    }
    result.map_err(|error| vec![error])
}

pub(super) fn prepare_native_rust_interop_bounded(
    program: &Program,
    spec_bytes: &[u8],
) -> Result<PreparedNativeRustInterop, Diagnostic> {
    prepare_native_rust_interop_from_input(Some(program), None, spec_bytes)
}

pub(super) fn prepare_project_native_rust_interop_bounded(
    program: &ResolvedProgram,
    subject_bytes: &[u8],
) -> Result<PreparedNativeRustInterop, Diagnostic> {
    prepare_native_rust_interop_from_input(None, Some(program), subject_bytes)
}

fn prepare_native_rust_interop_from_input<'a>(
    source_program: Option<&'a Program>,
    project_program: Option<&'a ResolvedProgram>,
    input_bytes: &[u8],
) -> Result<PreparedNativeRustInterop, Diagnostic> {
    let is_project = project_program.is_some();
    debit(input_bytes.len())?;
    let (
        spec,
        spec_authority,
        canonical_spec,
        project_subject_digest,
        resolved_owner,
        hir_budget,
        canonical_source_len,
    ) = if let Some(program) = source_program {
        validate_native_rust_source_expression_budget(program)?;
        let canonical_source = canonical_source_bounded(program)?;
        let (spec, spec_authority) =
            parse_spec_with_source_authority(program, input_bytes, &canonical_source)?;
        identifier_audit(program, &spec)?;
        let canonical_spec_budget = reserve_temporary_exact(MAX_SPEC_BYTES)?;
        let canonical_spec = render_spec(&spec);
        canonical_spec_budget.retain(canonical_spec.capacity())?;
        let mut hir_scan_stack = [None; MAX_SEMANTIC_EXPRESSION_DEPTH + 1];
        let hir_capacity =
            hir_pre_resolve_capacity(program, canonical_source.len(), &mut hir_scan_stack)?;
        let mut source_hir_budget = reserve_temporary_exact(
            hir_capacity
                .complete()
                .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?,
        )?;
        let dispose_frames = Vec::with_capacity(hir_capacity.disposal_frames);
        note_hir_resolve_pass();
        let owner = ResolvedProgramOwner::new(
            hir::resolve(program).map_err(|_| b107("selected identity missing"))?,
            dispose_frames,
            hir_capacity.disposal_frames,
        );
        let actual_hir_retained = hir_owned_capacity(owner.program())?
            .checked_add(hir_capacity.declaration_index_upper)
            .and_then(|bytes| {
                bytes.checked_add(
                    hir_capacity
                        .disposal_frames
                        .checked_mul(std::mem::size_of::<ResolvedDisposeFrame>())?,
                )
            })
            .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
        if actual_hir_retained > hir_capacity.retained_upper {
            return Err(b109("max_builder_bytes", MAX_BUILDER_BYTES));
        }
        source_hir_budget.shrink_held(actual_hir_retained)?;
        (
            spec,
            spec_authority,
            canonical_spec,
            None,
            PhaseAResolved::Source(owner),
            Some(source_hir_budget),
            canonical_source.len(),
        )
    } else {
        let program = project_program.ok_or_else(b106)?;
        let (subject, subject_authority) = parse_project_subject(input_bytes)?;
        hir::validate(program).map_err(|_| b107("invalid resolved Project program"))?;
        if program.module != subject.entry_module {
            return Err(b107("Project scalar closure required"));
        }
        identifier_gate(&program.module)?;
        let exports = subject
            .exports
            .iter()
            .map(|export| export.stable_id.clone())
            .collect::<Vec<_>>();
        let project_subject_digest = domain_digest(PROJECT_SUBJECT_DOMAIN, input_bytes);
        let spec = Spec {
            module: subject.entry_module.clone(),
            source_revision: None,
            target: current_target().ok_or_else(|| b107("unsupported target"))?,
            exports,
            imports: Vec::new(),
            capabilities: Vec::new(),
        };
        let spec_authority = reserve_temporary_exact(
            checked_spec_owned_capacity(&spec)
                .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?,
        )?;
        let canonical_spec_budget = reserve_temporary_exact(input_bytes.len())?;
        let canonical_spec = render_project_subject(&subject);
        if canonical_spec.as_bytes() != input_bytes {
            return Err(b106());
        }
        canonical_spec_budget.retain(canonical_spec.capacity())?;
        drop(subject);
        drop(subject_authority);
        (
            spec,
            spec_authority,
            canonical_spec,
            Some(project_subject_digest),
            PhaseAResolved::Project(program),
            None,
            input_bytes.len(),
        )
    };
    #[cfg(test)]
    let spec_transfer_allocations = (
        spec.source_revision
            .as_ref()
            .map_or(std::ptr::null(), |value| value.as_ptr()),
        spec.target.triple.as_ptr(),
        spec.target.endian.as_ptr(),
        spec.target.panic_strategy.as_ptr(),
        spec.target.thread_policy.as_ptr(),
    );
    let resolved = resolved_owner.program();
    let (closure, reached_imports) = selected_closure(resolved, &spec.exports)?;
    validate_native_rust_expression_budget_for_closure(&closure, true)?;
    validate_selected_scalar_closure(&closure)?;
    validate_native_unit_discard_bindings(&closure)?;
    #[cfg(test)]
    inject_prepare_failure(PrepareFailurePoint::Closure)?;
    // Keep the complete reservation through the post-resolution closure and
    // validation phases: their maps, DFS stacks, and pending vectors are part
    // of `scratch_upper`. Only after every such phase settles may the shared
    // sequential scratch be released while the conservative retained HIR and
    // selected-function clone ceiling remain authorized.
    // `DeclarationIndex` is intentionally opaque across the crate boundary.
    // Its maps contain only declaration identities/type facts derived from
    // canonical source; charge a separate source-derived upper while every
    // public ResolvedProgram field and selected clone is exact-censused.
    let facts_capacity = post_hir_facts_capacity(
        canonical_source_len,
        canonical_spec.len(),
        resolved,
        &closure,
        &spec,
    )?;
    let spec_transfer_capacity = prepared_spec_transfer_capacity(&spec)
        .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
    // The source revision and target strings are still owned by `spec` and
    // therefore already covered by `spec_authority`. Reserve only the new
    // facts topology here; the existing authority is narrowed and retained
    // when those exact allocations move into Prepared below.
    let facts_complete_without_spec_transfer = facts_capacity
        .complete()
        .and_then(|complete| complete.checked_sub(spec_transfer_capacity))
        .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
    let facts_budget = reserve_temporary_exact(facts_complete_without_spec_transfer)?;
    #[cfg(test)]
    note_post_hir_facts_entry();
    if reached_imports != spec.imports.iter().cloned().collect() {
        return Err(b107("unselected import reached"));
    }
    let mut selected_effects = BTreeSet::new();
    for function in &closure {
        identifier_gate(function.id.as_str())?;
        identifier_gate(&function.name)?;
        if function.effects.len() > MAX_EFFECTS {
            return Err(b109("max_effects", MAX_EFFECTS));
        }
        for effect in &function.effects {
            identifier_gate(effect)?;
            selected_effects.insert(effect.as_str());
        }
        if is_project
            && (!function.effects.is_empty()
                || resolved
                    .declarations
                    .declaration(&function.id)
                    .map(|declaration| declaration.identity_origin)
                    != Some(crate::hir::IdentityOrigin::Explicit))
        {
            return Err(b107(
                "Project closure requires explicit effect-free declarations",
            ));
        }
    }
    let source_functions = resolved
        .functions
        .iter()
        .map(|function| (function.id.as_str(), function))
        .collect::<BTreeMap<_, _>>();
    for id in &spec.exports {
        let function = source_functions
            .get(id.as_str())
            .ok_or_else(|| b107("selected identity missing"))?;
        let explicit_id = resolved
            .declarations
            .declaration(&function.id)
            .map(|declaration| declaration.identity_origin)
            == Some(crate::hir::IdentityOrigin::Explicit);
        if !explicit_id
            || function.name == "main"
            || function.params.len() > MAX_PARAMETERS
            || function.params.iter().any(|parameter| {
                parameter.ownership != OwnershipMode::Value || scalar_type(&parameter.ty).is_none()
            })
            || scalar_type(&function.return_type).is_none()
        {
            return Err(b107(if !explicit_id {
                "explicit persistent ID required"
            } else {
                "scalar value signature required"
            }));
        }
    }

    let resolved_import_count = resolved
        .interfaces
        .iter()
        .try_fold(0usize, |count, interface| {
            count.checked_add(interface.imports.len())
        })
        .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
    let mut resolved_imports = Vec::with_capacity(resolved_import_count);
    resolved_imports.extend(resolved.interfaces.iter().flat_map(|interface| {
        interface
            .imports
            .iter()
            .map(move |import| (interface.id.as_str(), import))
    }));
    #[cfg(test)]
    note_post_hir_facts_capacity(post_hir_selection_scratch_capacity(
        &selected_effects,
        &source_functions,
        &resolved_imports,
    ));
    #[cfg(test)]
    note_post_hir_facts_scratch(post_hir_selection_scratch_capacity(
        &selected_effects,
        &source_functions,
        &resolved_imports,
    ));
    let mut import_facts = Vec::with_capacity(spec.imports.len());
    for id in &spec.imports {
        let (interface, import) = resolved_imports
            .iter()
            .find(|(_, import)| import.id.as_str() == id)
            .copied()
            .ok_or_else(|| b107("selected identity missing"))?;
        if !import.native_rust {
            return Err(b107("selected identity missing"));
        }
        identifier_gate(interface)?;
        identifier_gate(import.id.as_str())?;
        identifier_gate(&import.name)?;
        if import.effects.len() > MAX_EFFECTS {
            return Err(b109("max_effects", MAX_EFFECTS));
        }
        for effect in &import.effects {
            identifier_gate(effect)?;
            selected_effects.insert(effect.as_str());
        }
        let parameters = import_parameter_facts(import)?;
        let result = match import.result.kind {
            ResolvedImportResultKind::Unit => ScalarType::Unit,
            ResolvedImportResultKind::I64 => ScalarType::I64,
            ResolvedImportResultKind::Bool => ScalarType::Bool,
        };
        let failure = match &import.failure {
            ResolvedImportFailure::Infallible => None,
            ResolvedImportFailure::Status { domain_id, .. } => Some(domain_id.clone()),
        };
        let hash = full_hash(id);
        let effect_set = import.effects.iter().cloned().collect::<BTreeSet<_>>();
        #[cfg(test)]
        let import_effect_baseline = post_hir_facts_owned_capacity(&Vec::new(), &import_facts);
        #[cfg(test)]
        let import_effect_outer_scratch = post_hir_selection_scratch_capacity(
            &selected_effects,
            &source_functions,
            &resolved_imports,
        );
        #[cfg(test)]
        let import_effect_locals =
            parameter_facts_owned_capacity(&parameters, parameters.capacity())
                .saturating_add(failure.as_ref().map_or(0, String::capacity))
                .saturating_add(hash.capacity());
        #[cfg(test)]
        note_post_hir_facts_live(
            import_effect_baseline,
            import_effect_outer_scratch
                .saturating_add(import_effect_locals)
                .saturating_add(
                    checked_owned_string_set(&effect_set)
                        .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?,
                ),
        );
        let mut effects = Vec::with_capacity(effect_set.len());
        let mut remaining_effects = effect_set;
        while let Some(effect) = remaining_effects.pop_first() {
            effects.push(effect);
            #[cfg(test)]
            note_post_hir_facts_live(
                import_effect_baseline,
                import_effect_outer_scratch
                    .saturating_add(import_effect_locals)
                    .saturating_add(
                        checked_owned_string_set(&remaining_effects)
                            .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?,
                    )
                    .saturating_add(
                        checked_owned_string_vec(&effects, effects.capacity())
                            .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?,
                    ),
            );
        }
        import_facts.push(ImportFact {
            id: id.clone(),
            interface: interface.to_owned(),
            import_key: import.import_key.clone(),
            rust_method: format!("import_{hash}"),
            c_field: format!("spxnr1_i_{hash}"),
            parameters,
            result,
            effects: effects.clone(),
            capabilities: effects,
            failure,
            call_contract_digest: String::new(),
        });
        #[cfg(test)]
        note_post_hir_facts_live(
            post_hir_facts_owned_capacity(&Vec::new(), &import_facts),
            post_hir_selection_scratch_capacity(
                &selected_effects,
                &source_functions,
                &resolved_imports,
            )
            .saturating_add(hash.capacity()),
        );
    }
    import_facts.sort_by(|left, right| left.id.cmp(&right.id));
    if selected_effects.len() > MAX_EFFECTS {
        return Err(b109("max_effects", MAX_EFFECTS));
    }
    let selected_capability_set = import_facts
        .iter()
        .flat_map(|import| import.capabilities.iter().cloned())
        .collect::<BTreeSet<_>>();
    #[cfg(test)]
    let selected_capability_baseline = post_hir_facts_owned_capacity(&Vec::new(), &import_facts)
        .saturating_add(post_hir_selection_scratch_capacity(
            &selected_effects,
            &source_functions,
            &resolved_imports,
        ));
    #[cfg(test)]
    let selected_capability_set_owned = checked_owned_string_set(&selected_capability_set)
        .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
    #[cfg(test)]
    note_post_hir_facts_live(selected_capability_baseline, selected_capability_set_owned);
    let mut selected_capabilities = Vec::with_capacity(selected_capability_set.len());
    for capability in selected_capability_set {
        selected_capabilities.push(capability);
        #[cfg(test)]
        note_post_hir_facts_live(
            selected_capability_baseline,
            selected_capability_set_owned.saturating_add(
                checked_owned_string_vec(&selected_capabilities, selected_capabilities.capacity())
                    .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?,
            ),
        );
    }
    if selected_capabilities != spec.capabilities {
        return Err(b107("effect or capability mismatch"));
    }

    let status_domain_set = import_facts
        .iter()
        .filter_map(|import| import.failure.clone())
        .collect::<BTreeSet<_>>();
    #[cfg(test)]
    let status_domain_set_owned = checked_owned_string_set(&status_domain_set)
        .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
    #[cfg(test)]
    let status_phase_outer_scratch = post_hir_selection_scratch_capacity(
        &selected_effects,
        &source_functions,
        &resolved_imports,
    )
    .checked_add(
        checked_owned_string_vec(&selected_capabilities, selected_capabilities.capacity())
            .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?,
    )
    .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
    #[cfg(test)]
    let status_domain_conversion_baseline =
        post_hir_facts_owned_capacity(&Vec::new(), &import_facts)
            .checked_add(status_phase_outer_scratch)
            .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
    #[cfg(test)]
    note_post_hir_facts_capacity(
        status_domain_conversion_baseline
            .checked_add(status_domain_set_owned)
            .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?,
    );
    #[cfg(test)]
    note_post_hir_facts_scratch(
        status_phase_outer_scratch
            .checked_add(status_domain_set_owned)
            .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?,
    );
    let mut status_domains = Vec::with_capacity(status_domain_set.len());
    for domain in status_domain_set {
        status_domains.push(domain);
        #[cfg(test)]
        {
            let status_domain_conversion_scratch = status_domain_set_owned
                .checked_add(
                    checked_owned_string_vec(&status_domains, status_domains.capacity())
                        .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?,
                )
                .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
            note_post_hir_facts_scratch(
                status_phase_outer_scratch
                    .checked_add(status_domain_conversion_scratch)
                    .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?,
            );
            note_post_hir_facts_capacity(
                status_domain_conversion_baseline
                    .checked_add(status_domain_conversion_scratch)
                    .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?,
            );
        }
    }
    if status_domains
        .len()
        .checked_add(4)
        .is_none_or(|count| count > MAX_STATUS_DOMAINS)
    {
        return Err(b109("max_status_domains", MAX_STATUS_DOMAINS));
    }
    let ordinals = status_domains
        .iter()
        .enumerate()
        .map(|(index, domain)| {
            (
                domain.as_str(),
                u16::try_from(index + 1).unwrap_or(u16::MAX),
            )
        })
        .collect::<BTreeMap<_, _>>();
    #[cfg(test)]
    note_post_hir_facts_capacity(
        post_hir_facts_owned_capacity(&Vec::new(), &import_facts)
            + post_hir_selection_scratch_capacity(
                &selected_effects,
                &source_functions,
                &resolved_imports,
            )
            + checked_owned_string_vec(&selected_capabilities, selected_capabilities.capacity())
                .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?
            + checked_owned_string_vec(&status_domains, status_domains.capacity())
                .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?
            + borrowed_map_owned_capacity::<&str, u16>(ordinals.len()),
    );
    #[allow(clippy::needless_range_loop)]
    for index in 0..import_facts.len() {
        #[cfg(test)]
        let import_digest_baseline = post_hir_facts_owned_capacity(&Vec::new(), &import_facts)
            + post_hir_selection_scratch_capacity(
                &selected_effects,
                &source_functions,
                &resolved_imports,
            )
            + checked_owned_string_vec(&selected_capabilities, selected_capabilities.capacity())
                .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?
            + checked_owned_string_vec(&status_domains, status_domains.capacity())
                .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?
            + borrowed_map_owned_capacity::<&str, u16>(ordinals.len())
            + owned_string_set_owned_capacity(&reached_imports);
        #[cfg(not(test))]
        let import_digest_baseline = 0;
        let import = &mut import_facts[index];
        let failure = import.failure.as_ref().map_or_else(
            || "infallible".to_owned(),
            |domain| {
                format!(
                    "{}:{domain}",
                    ordinals.get(domain.as_str()).copied().unwrap_or(u16::MAX)
                )
            },
        );
        import.call_contract_digest = call_digest(
            "import",
            &import.id,
            &import.parameters,
            import.result,
            &import.effects,
            &import.capabilities,
            &[],
            &[],
            &failure,
            import_digest_baseline + failure.capacity(),
            &spec.target,
        )?;
    }

    let by_function = closure
        .iter()
        .map(|function| (function.id.as_str(), *function))
        .collect::<BTreeMap<_, _>>();
    for function in &closure {
        #[cfg(test)]
        let traversal_baseline = post_hir_live_facts_capacity(
            &Vec::new(),
            &import_facts,
            &selected_effects,
            &source_functions,
            &resolved_imports,
            &selected_capabilities,
            &status_domains,
            &ordinals,
            &by_function,
        );
        #[cfg(not(test))]
        let traversal_baseline = 0;
        let reachable = transitive_imports(
            function,
            &by_function,
            facts_capacity.traversal_pending_capacity,
            traversal_baseline,
        )?;
        let reachable_effects = reachable
            .iter()
            .filter_map(|id| import_facts.iter().find(|import| import.id == id.as_str()))
            .flat_map(|import| import.effects.iter().cloned())
            .collect::<BTreeSet<_>>();
        let declared = function.effects.iter().cloned().collect::<BTreeSet<_>>();
        #[cfg(test)]
        note_post_hir_facts_live(
            traversal_baseline,
            owned_string_set_owned_capacity(&reachable)
                .saturating_add(owned_string_set_owned_capacity(&reachable_effects))
                .saturating_add(owned_string_set_owned_capacity(&declared)),
        );
        if declared != reachable_effects {
            return Err(b107("effect or capability mismatch"));
        }
    }
    let mut export_facts = Vec::with_capacity(spec.exports.len());
    for id in &spec.exports {
        let function = by_function
            .get(id.as_str())
            .ok_or_else(|| b107("selected identity missing"))?;
        let parameters = parameter_facts(function)?;
        let result = scalar_type(&function.return_type)
            .filter(|ty| *ty != ScalarType::Unit)
            .ok_or_else(|| b107("scalar value signature required"))?;
        #[cfg(test)]
        let traversal_baseline = post_hir_live_facts_capacity(
            &export_facts,
            &import_facts,
            &selected_effects,
            &source_functions,
            &resolved_imports,
            &selected_capabilities,
            &status_domains,
            &ordinals,
            &by_function,
        );
        #[cfg(not(test))]
        let traversal_baseline = 0;
        let reachable_imports = transitive_imports(
            function,
            &by_function,
            facts_capacity.traversal_pending_capacity,
            traversal_baseline,
        )?;
        let capabilities = spec.capabilities.clone();
        let mut required_imports = Vec::with_capacity(import_facts.len());
        required_imports.extend(import_facts.iter().map(|import| import.id.clone()));
        let mut required_import_contracts = Vec::with_capacity(import_facts.len());
        required_import_contracts.extend(
            import_facts
                .iter()
                .map(|import| (import.id.clone(), import.call_contract_digest.clone())),
        );
        #[cfg(test)]
        let export_prefix_baseline = traversal_baseline
            .saturating_add(parameter_facts_owned_capacity(
                &parameters,
                parameters.capacity(),
            ))
            .saturating_add(owned_string_set_owned_capacity(&reachable_imports))
            .saturating_add(
                checked_owned_string_vec(&capabilities, capabilities.capacity())
                    .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?,
            )
            .saturating_add(
                checked_owned_string_vec(&required_imports, required_imports.capacity())
                    .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?,
            )
            .saturating_add(
                checked_owned_string_pairs(&required_import_contracts)
                    .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?,
            );
        let effect_set = function.effects.iter().cloned().collect::<BTreeSet<_>>();
        #[cfg(test)]
        let effect_set_owned = checked_owned_string_set(&effect_set)
            .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
        #[cfg(test)]
        note_post_hir_facts_live(export_prefix_baseline, effect_set_owned);
        let mut effects = Vec::with_capacity(effect_set.len());
        for effect in effect_set {
            effects.push(effect);
            #[cfg(test)]
            note_post_hir_facts_live(
                export_prefix_baseline,
                effect_set_owned.saturating_add(
                    checked_owned_string_vec(&effects, effects.capacity())
                        .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?,
                ),
            );
        }
        let status_domain_ordinal_set = reachable_imports
            .iter()
            .filter_map(|id| import_facts.iter().find(|import| import.id == id.as_str()))
            .filter_map(|import| import.failure.as_deref())
            .filter_map(|domain| ordinals.get(domain).copied())
            .collect::<BTreeSet<_>>();
        #[cfg(test)]
        let status_domain_ordinal_set_owned =
            btree_allocation_upper::<u16, ()>(status_domain_ordinal_set.len());
        #[cfg(test)]
        let status_ordinal_baseline = export_prefix_baseline.saturating_add(
            checked_owned_string_vec(&effects, effects.capacity())
                .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?,
        );
        #[cfg(test)]
        note_post_hir_facts_live(status_ordinal_baseline, status_domain_ordinal_set_owned);
        let mut status_domain_ordinals = Vec::with_capacity(
            status_domain_ordinal_set
                .len()
                .checked_add(3)
                .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?,
        );
        for ordinal in status_domain_ordinal_set {
            status_domain_ordinals.push(ordinal);
            #[cfg(test)]
            note_post_hir_facts_live(
                status_ordinal_baseline,
                status_domain_ordinal_set_owned.saturating_add(
                    checked_u16_vec(&status_domain_ordinals)
                        .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?,
                ),
            );
        }
        status_domain_ordinals.extend([65_533, 65_534, 65_535]);
        status_domain_ordinals.sort_unstable();
        let status_contract_values = status_domain_ordinals
            .iter()
            .map(|ordinal| match *ordinal {
                65_533 => Ok::<_, Diagnostic>("65533:semaprax.native-rust-semantics.v1".to_owned()),
                65_534 => Ok("65534:semaprax.native-rust-host.v1".to_owned()),
                65_535 => Ok("65535:semaprax.native-rust-adapter.v1".to_owned()),
                _ => {
                    let domain = status_domains
                        .get(usize::from(*ordinal).saturating_sub(1))
                        .ok_or_else(b111)?;
                    Ok(format!("{ordinal}:{domain}"))
                }
            })
            .collect::<Result<Vec<_>, _>>()?;
        #[cfg(test)]
        let status_contract_baseline = status_ordinal_baseline.saturating_add(
            checked_u16_vec(&status_domain_ordinals)
                .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?,
        );
        #[cfg(test)]
        note_post_hir_facts_live(
            status_contract_baseline,
            checked_owned_string_vec(&status_contract_values, status_contract_values.capacity())
                .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?,
        );
        let status_contract = status_contract_values.join(";");
        #[cfg(test)]
        note_post_hir_facts_live(
            status_contract_baseline,
            checked_owned_string_vec(&status_contract_values, status_contract_values.capacity())
                .and_then(|bytes| bytes.checked_add(status_contract.capacity()))
                .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?,
        );
        let hash = full_hash(id);
        #[cfg(test)]
        let export_digest_baseline = status_contract_baseline
            .saturating_add(
                checked_owned_string_vec(
                    &status_contract_values,
                    status_contract_values.capacity(),
                )
                .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?,
            )
            .saturating_add(status_contract.capacity())
            .saturating_add(hash.capacity());
        #[cfg(not(test))]
        let export_digest_baseline = 0;
        let call_contract_digest = call_digest(
            "export",
            id,
            &parameters,
            result,
            &effects,
            &capabilities,
            &required_imports,
            &required_import_contracts,
            &status_contract,
            export_digest_baseline,
            &spec.target,
        )?;
        export_facts.push(ExportFact {
            id: id.clone(),
            rust_method: format!("export_{hash}"),
            c_symbol: format!("spxnr1_e_{hash}"),
            parameters: parameters.clone(),
            result,
            effects: effects.clone(),
            capabilities: capabilities.clone(),
            required_imports: required_imports.clone(),
            status_domain_ordinals,
            call_contract_digest,
        });
        #[cfg(test)]
        {
            let export_clone_overlap_scratch =
                parameter_facts_owned_capacity(&parameters, parameters.capacity())
                    .saturating_add(owned_string_set_owned_capacity(&reachable_imports))
                    .saturating_add(
                        checked_owned_string_vec(&capabilities, capabilities.capacity())
                            .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?,
                    )
                    .saturating_add(
                        checked_owned_string_vec(&required_imports, required_imports.capacity())
                            .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?,
                    )
                    .saturating_add(
                        checked_owned_string_pairs(&required_import_contracts)
                            .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?,
                    )
                    .saturating_add(
                        checked_owned_string_vec(&effects, effects.capacity())
                            .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?,
                    )
                    .saturating_add(
                        checked_owned_string_vec(
                            &status_contract_values,
                            status_contract_values.capacity(),
                        )
                        .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?,
                    )
                    .saturating_add(status_contract.capacity())
                    .saturating_add(hash.capacity());
            note_post_hir_facts_live(
                post_hir_live_facts_capacity(
                    &export_facts,
                    &import_facts,
                    &selected_effects,
                    &source_functions,
                    &resolved_imports,
                    &selected_capabilities,
                    &status_domains,
                    &ordinals,
                    &by_function,
                ),
                export_clone_overlap_scratch,
            );
        }
    }
    export_facts.sort_by(|left, right| left.id.cmp(&right.id));
    #[cfg(test)]
    note_post_hir_facts_capacity(post_hir_facts_owned_capacity(&export_facts, &import_facts));
    drop(by_function);
    drop(ordinals);
    drop(selected_capabilities);
    drop(resolved_imports);
    drop(source_functions);
    drop(selected_effects);
    drop(reached_imports);
    status_domains.shrink_to_fit();
    #[cfg(test)]
    let fingerprint_baseline = post_hir_facts_owned_capacity(&export_facts, &import_facts)
        + string_vec_owned_capacity(&status_domains, status_domains.capacity());
    #[cfg(not(test))]
    let fingerprint_baseline = 0;
    let hir_digest = hir_fingerprint(&closure, &import_facts, fingerprint_baseline)?;
    #[cfg(test)]
    inject_prepare_failure(PrepareFailurePoint::Facts)?;
    let descriptor_budget = reserve_temporary_exact(MAX_DESCRIPTOR_BYTES)?;
    let descriptor = if is_project {
        render_descriptor_for_subject(
            &spec,
            DescriptorSubject::ProjectSubjectDigest(
                project_subject_digest.as_deref().ok_or_else(b108)?,
            ),
            &hir_digest,
            &status_domains,
            &export_facts,
            &import_facts,
        )?
    } else {
        render_descriptor(
            &spec,
            &hir_digest,
            &status_domains,
            &export_facts,
            &import_facts,
        )?
    };
    descriptor_budget.retain(descriptor.capacity())?;
    if descriptor.len() > MAX_DESCRIPTOR_BYTES {
        return Err(b109("max_descriptor_bytes", MAX_DESCRIPTOR_BYTES));
    }
    if is_project {
        replay_descriptor_for_subject(
            &descriptor,
            &spec,
            DescriptorSubject::ProjectSubjectDigest(
                project_subject_digest.as_deref().ok_or_else(b108)?,
            ),
            &hir_digest,
            &export_facts,
            &import_facts,
        )?;
    } else {
        replay_descriptor(
            &descriptor,
            &spec,
            &hir_digest,
            &export_facts,
            &import_facts,
        )?;
    }
    let header_budget = reserve_temporary_exact(MAX_GENERATED_HEADER_BYTES)?;
    let generated_header = generate_header(&export_facts, &import_facts)?;
    header_budget.retain(generated_header.capacity())?;
    let c_budget = reserve_temporary_exact(MAX_GENERATED_C_BYTES)?;
    let generated_c = generate_c(&spec, &closure, &export_facts, &import_facts)?;
    c_budget.retain(generated_c.capacity())?;
    let rust_budget = reserve_temporary_exact(MAX_GENERATED_RUST_BYTES)?;
    let (generated_rust, private_ffi_source) =
        generate_rust_artifacts(&spec, &export_facts, &import_facts)?;
    let rust_capacity = generated_rust
        .capacity()
        .checked_add(private_ffi_source.capacity())
        .ok_or_else(|| b109("max_generated_rust_bytes", MAX_GENERATED_RUST_BYTES))?;
    rust_budget.retain(rust_capacity)?;
    #[cfg(test)]
    inject_prepare_failure(PrepareFailurePoint::Render)?;
    for (field, bytes, maximum) in [
        (
            "max_generated_c_bytes",
            generated_c.len(),
            MAX_GENERATED_C_BYTES,
        ),
        (
            "max_generated_header_bytes",
            generated_header.len(),
            MAX_GENERATED_HEADER_BYTES,
        ),
    ] {
        if bytes > maximum {
            return Err(b109(field, maximum));
        }
    }
    replay_generated_exact(
        &spec,
        &closure,
        &export_facts,
        &import_facts,
        &generated_header,
        &generated_c,
        &generated_rust,
        &private_ffi_source,
    )?;
    #[cfg(test)]
    inject_prepare_failure(PrepareFailurePoint::Replay)?;
    drop(status_domains);
    let mut closure_ids = Vec::with_capacity(closure.len());
    closure_ids.extend(
        closure
            .iter()
            .map(|function| function.id.as_str().to_owned()),
    );
    let spec_digest = domain_digest(
        if is_project {
            PROJECT_SUBJECT_DOMAIN
        } else {
            SPEC_DIGEST_DOMAIN
        },
        canonical_spec.as_bytes(),
    );
    let descriptor_digest = domain_digest(
        if is_project {
            PROJECT_DESCRIPTOR_DIGEST_DOMAIN
        } else {
            DESCRIPTOR_DIGEST_DOMAIN
        },
        descriptor.as_bytes(),
    );
    let spec_authority_bytes = checked_spec_owned_capacity(&spec)
        .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
    if spec_authority.maximum() != spec_authority_bytes {
        return Err(b109("max_builder_bytes", MAX_BUILDER_BYTES));
    }
    let closure_id_bytes = closure_ids
        .iter()
        .try_fold(0usize, |bytes, id| bytes.checked_add(id.capacity()))
        .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
    let persistent_without_spec_transfer =
        post_hir_facts_owned_capacity_checked(&export_facts, &import_facts)
            .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?
            .checked_add(
                closure_ids
                    .capacity()
                    .checked_mul(std::mem::size_of::<String>())
                    .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?,
            )
            .and_then(|bytes| bytes.checked_add(closure_id_bytes))
            .and_then(|bytes| bytes.checked_add(hir_digest.capacity()))
            .and_then(|bytes| bytes.checked_add(spec_digest.capacity()))
            .and_then(|bytes| bytes.checked_add(descriptor_digest.capacity()))
            .and_then(|bytes| {
                bytes.checked_add(project_subject_digest.as_ref().map_or(0, String::capacity))
            })
            .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
    let persistent_facts = persistent_without_spec_transfer
        .checked_add(spec_transfer_capacity)
        .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
    #[cfg(test)]
    POST_HIR_AUTHORITY_TRANSFER_TERMS.with(|terms| {
        terms.set([
            facts_capacity.complete().expect("checked facts capacity"),
            spec_transfer_capacity,
            facts_complete_without_spec_transfer,
            persistent_without_spec_transfer,
            persistent_facts,
        ]);
    });
    if persistent_facts > facts_capacity.retained_upper {
        return Err(b109("max_builder_bytes", MAX_BUILDER_BYTES));
    }
    if spec_transfer_capacity > spec_authority_bytes {
        return Err(b109("max_builder_bytes", MAX_BUILDER_BYTES));
    }
    let retained_without_spec_transfer_upper = facts_capacity
        .retained_upper
        .checked_sub(spec_transfer_capacity)
        .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
    if persistent_without_spec_transfer > retained_without_spec_transfer_upper {
        return Err(b109("max_builder_bytes", MAX_BUILDER_BYTES));
    }
    #[cfg(test)]
    let ledger_before_transfer = crate::bounded_output::remaining_active()
        .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
    #[cfg(test)]
    let facts_reserved_before_transfer = facts_budget.maximum();
    let Spec {
        module,
        source_revision,
        target,
        exports: spec_exports,
        imports: spec_imports,
        capabilities: spec_capabilities,
    } = spec;
    #[cfg(test)]
    assert_eq!(
        spec_transfer_allocations,
        (
            source_revision
                .as_ref()
                .map_or(std::ptr::null(), |value| value.as_ptr()),
            target.triple.as_ptr(),
            target.endian.as_ptr(),
            target.panic_strategy.as_ptr(),
            target.thread_policy.as_ptr(),
        ),
        "Spec source/target allocations must move into Prepared without clones",
    );
    let moved_transfer_capacity = source_revision
        .as_ref()
        .map_or(0, String::capacity)
        .checked_add(target.triple.capacity())
        .and_then(|bytes| bytes.checked_add(target.endian.capacity()))
        .and_then(|bytes| bytes.checked_add(target.panic_strategy.capacity()))
        .and_then(|bytes| bytes.checked_add(target.thread_policy.capacity()))
        .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
    if moved_transfer_capacity != spec_transfer_capacity {
        return Err(b109("max_builder_bytes", MAX_BUILDER_BYTES));
    }
    // Destroy every non-transferred Spec allocation before narrowing its
    // authority; the five moved allocations remain continuously covered.
    drop((module, spec_exports, spec_imports, spec_capabilities));
    #[cfg(test)]
    let expected_remaining_after_transfer = ledger_before_transfer
        .checked_add(
            spec_authority_bytes
                .checked_sub(spec_transfer_capacity)
                .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?,
        )
        .and_then(|bytes| {
            bytes.checked_add(
                facts_reserved_before_transfer.checked_sub(persistent_without_spec_transfer)?,
            )
        })
        .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
    spec_authority.retain(spec_transfer_capacity)?;
    facts_budget.retain(persistent_without_spec_transfer)?;
    #[cfg(test)]
    assert_eq!(
        crate::bounded_output::remaining_active(),
        Some(expected_remaining_after_transfer),
        "Spec authority must release exactly once before Prepared facts retain",
    );
    drop(closure);
    let _ = resolved;
    drop(resolved_owner);
    drop(hir_budget);
    Ok(PreparedNativeRustInterop {
        spec_digest,
        canonical_spec,
        descriptor_digest,
        descriptor,
        source_revision,
        project_subject_digest,
        hir_digest,
        target,
        exports: export_facts,
        imports: import_facts,
        closure: closure_ids,
        generated_c,
        generated_header,
        generated_rust,
        private_ffi_source,
    })
}

pub(super) fn validate_selected_scalar_closure(
    functions: &[&ResolvedFunction],
) -> Result<(), Diagnostic> {
    note_hir_post_resolve_phase(2);
    let mut pending = Vec::new();
    for function in functions {
        if function.params.len() > MAX_PARAMETERS
            || function.params.iter().any(|parameter| {
                parameter.ownership != hir::OwnershipMode::Value
                    || scalar_type(&parameter.ty).is_none()
            })
            || scalar_type(&function.return_type).is_none()
            || !function.cleanup.slots.is_empty()
            || !function.cleanup.flags.is_empty()
            || !function.cleanup_plan.slots.is_empty()
        {
            return Err(b107("scalar value signature required"));
        }
        pending.extend(function.requires.iter());
        pending.push(&function.body);
        pending.extend(function.ensures.iter());
    }
    while let Some(expression) = pending.pop() {
        note_hir_post_resolve_capacity(
            1,
            pending.capacity() * std::mem::size_of::<&ResolvedExpr>(),
        );
        let direct_unit_import = expression.ty == ResolvedType::Unit
            && matches!(expression.kind, ResolvedExprKind::NativeRustImportCall(_));
        if scalar_type(&expression.ty).is_none() && !direct_unit_import {
            return Err(b107("scalar value signature required"));
        }
        match &expression.kind {
            ResolvedExprKind::Int(_)
            | ResolvedExprKind::Int32(_)
            | ResolvedExprKind::Char(_)
            | ResolvedExprKind::Uint8(_)
            | ResolvedExprKind::Usize(_)
            | ResolvedExprKind::Float32(_)
            | ResolvedExprKind::Float64(_)
            | ResolvedExprKind::Bool(_)
            | ResolvedExprKind::String(_) => {}
            ResolvedExprKind::Place(place)
                if place.projections.is_empty()
                    && expression.ownership == hir::OwnershipMode::Value => {}
            ResolvedExprKind::Call {
                type_arguments,
                instance,
                args,
                ..
            } if type_arguments.is_empty() && instance.is_none() => pending.extend(args),
            ResolvedExprKind::NativeRustImportCall(call) => pending.extend(&call.args),
            ResolvedExprKind::HostCommandCall(_) => {
                // Keep command I/O outside the promoted scalar Rust boundary.
                return Err(b107("scalar value signature required"));
            }
            ResolvedExprKind::Unary { value, .. } => pending.push(value),
            ResolvedExprKind::Binary { left, right, .. } => {
                pending.push(left);
                pending.push(right);
            }
            ResolvedExprKind::Block { statements, tail } => {
                for statement in statements {
                    let (binding, value) = match statement {
                        ResolvedStatement::Let { binding, value, .. }
                        | ResolvedStatement::Assign { binding, value, .. } => (binding, value),
                        ResolvedStatement::Unsafe { .. } | ResolvedStatement::While { .. } => {
                            return Err(b107("scalar value signature required"));
                        }
                    };
                    let unit_discard = binding.ty == ResolvedType::Unit
                        && value.ty == ResolvedType::Unit
                        && matches!(value.kind, ResolvedExprKind::NativeRustImportCall(_));
                    if binding.ownership != hir::OwnershipMode::Value
                        || (scalar_type(&binding.ty).is_none() && !unit_discard)
                    {
                        return Err(b107("scalar value signature required"));
                    }
                    pending.push(value);
                }
                pending.push(tail);
            }
            ResolvedExprKind::If {
                condition,
                then_branch,
                else_branch,
            } => {
                pending.push(condition);
                pending.push(then_branch);
                pending.push(else_branch);
            }
            ResolvedExprKind::ConstructRecord { .. }
            | ResolvedExprKind::ArrayU8(_)
            | ResolvedExprKind::RepeatArrayU8 { .. }
            | ResolvedExprKind::BorrowPlace { .. }
            | ResolvedExprKind::ByteRange { .. }
            | ResolvedExprKind::Call { .. }
            | ResolvedExprKind::ConstructVariant { .. }
            | ResolvedExprKind::Match { .. }
            | ResolvedExprKind::Try { .. }
            | ResolvedExprKind::TryOption { .. }
            | ResolvedExprKind::UpdateRecord { .. }
            | ResolvedExprKind::Project { .. }
            | ResolvedExprKind::Upcast { .. }
            | ResolvedExprKind::Place(_) => {
                return Err(b107("scalar value signature required"));
            }
        }
    }
    Ok(())
}

pub(super) fn validate_native_unit_discard_bindings(
    functions: &[&ResolvedFunction],
) -> Result<(), Diagnostic> {
    note_hir_post_resolve_phase(3);
    for function in functions {
        let mut discarded = BTreeSet::<hir::ValueId>::new();
        let mut pending = vec![(&function.body, false)];
        while let Some((expression, direct_let_rhs)) = pending.pop() {
            note_hir_post_resolve_capacity(
                2,
                pending.capacity() * std::mem::size_of::<(&ResolvedExpr, bool)>()
                    + discarded.len()
                        * (std::mem::size_of::<hir::ValueId>()
                            + std::mem::size_of::<BTreeSet<hir::ValueId>>())
                    + discarded.iter().map(|id| id.as_str().len()).sum::<usize>(),
            );
            if expression.ty == ResolvedType::Unit && !direct_let_rhs {
                return Err(b107("scalar value signature required"));
            }
            match &expression.kind {
                ResolvedExprKind::Block { statements, tail } => {
                    for statement in statements {
                        let (binding, value) = match statement {
                            ResolvedStatement::Let { binding, value, .. }
                            | ResolvedStatement::Assign { binding, value, .. } => (binding, value),
                            ResolvedStatement::Unsafe { .. } | ResolvedStatement::While { .. } => {
                                return Err(b107("scalar value signature required"));
                            }
                        };
                        if value.ty == ResolvedType::Unit {
                            if !matches!(value.kind, ResolvedExprKind::NativeRustImportCall(_))
                                || binding.ty != ResolvedType::Unit
                                || !discarded.insert(binding.id.clone())
                            {
                                return Err(b107("scalar value signature required"));
                            }
                            pending.push((value, true));
                        } else {
                            pending.push((value, false));
                        }
                    }
                    pending.push((tail, false));
                }
                ResolvedExprKind::Place(place) if discarded.contains(&place.root) => {
                    return Err(b107("scalar value signature required"));
                }
                ResolvedExprKind::Call { args, .. } => {
                    pending.extend(args.iter().map(|child| (child, false)));
                }
                ResolvedExprKind::NativeRustImportCall(call) => {
                    pending.extend(call.args.iter().map(|child| (child, false)));
                }
                ResolvedExprKind::Unary { value, .. }
                | ResolvedExprKind::Try { operand: value, .. }
                | ResolvedExprKind::TryOption { operand: value, .. }
                | ResolvedExprKind::Project { base: value, .. } => pending.push((value, false)),
                ResolvedExprKind::Binary { left, right, .. } => {
                    pending.push((left, false));
                    pending.push((right, false));
                }
                ResolvedExprKind::If {
                    condition,
                    then_branch,
                    else_branch,
                } => {
                    pending.push((condition, false));
                    pending.push((then_branch, false));
                    pending.push((else_branch, false));
                }
                ResolvedExprKind::ConstructRecord { fields, .. }
                | ResolvedExprKind::ConstructVariant { fields, .. } => {
                    pending.extend(fields.iter().map(|field| (&field.value, false)));
                }
                ResolvedExprKind::Match { scrutinee, arms } => {
                    pending.push((scrutinee, false));
                    pending.extend(arms.iter().map(|arm| (&arm.value, false)));
                }
                ResolvedExprKind::UpdateRecord { base, fields, .. } => {
                    pending.push((base, false));
                    pending.extend(fields.iter().map(|field| (&field.value, false)));
                }
                _ => {}
            }
        }
    }
    Ok(())
}
