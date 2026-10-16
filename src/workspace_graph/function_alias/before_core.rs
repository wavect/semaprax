//! Complete independent immediate-old core constructor, source adapted only for visibility.
use crate::workspace_graph::*;
// Keep the source-authority inputs explicit at this audited construction boundary.
#[allow(clippy::too_many_arguments)]
pub(in crate::workspace_graph) fn build_resolved_core_current(
    programs: &[Program],
    module_paths: &BTreeMap<&str, &str>,
    dependency_depths: &BTreeMap<&str, usize>,
    authored: &BTreeMap<&str, AuthoredDeclaration<'_>>,
    authenticated_sources: Option<&[WorkspaceSource]>,
    mut frontend: Option<&mut crate::project::incremental::FrontendPass>,
    layouts: &expected_projection::private_layout::registry::LayoutRegistry<'_>,
    compact_order: Option<&[usize]>,
) -> Result<ResolvedCore, constructor_failure::receiving::CoreFailure> {
    #[cfg(test)]
    if constructor_failure::receiving::before::active() {
        return constructor_failure::receiving::before::build_resolved_core_current(
            programs,
            module_paths,
            dependency_depths,
            authored,
            authenticated_sources,
            frontend,
            layouts,
            compact_order,
        )
        .map_err(constructor_failure::receiving::CoreFailure::from);
    }
    let retained_output_only = compact_order.is_some();
    let mut loan_scratch = loan_retention::RetentionScratch::default();
    let mut expected_edges = Vec::new();
    for program in programs {
        staged!(
            ExpectedEdges,
            collect_expected_edges(
                program,
                programs,
                module_paths,
                authored,
                &mut expected_edges,
            )
        )?;
    }
    checked_retention::reserve_input_carriers(programs.len())?;
    // The final uncached attempt uses a compact index and sequential HIR
    // filtering; earlier receipts retain the complete synthetic HIR vector.
    let mut validation = retained_output_only
        .then(|| staged!(CompactValidation, WorkspaceValidationIndex::new(programs)))
        .transpose()?;
    let authority = expected_projection::private_layout::authority::invocation::Invocation::new(
        programs, authored,
    );
    let mut synthetic_modules = if retained_output_only {
        Vec::new()
    } else {
        Vec::with_capacity(programs.len())
    };
    let mut modules = Vec::new();
    let mut imported_vec_instances = BTreeMap::new();
    if retained_output_only {
        reserve_workspace_module_carrier(programs.len())?;
        modules = Vec::with_capacity(programs.len());
    }
    // Earlier/cached forecasts retain the old inline Program carrier. The
    // new tagged input adds one reused enclosing header, not one per module.
    // Sequential uncached retention prices that same carrier in its peak.
    if !retained_output_only {
        reserve_builder_structure(expected_projection::borrowed_leaf::input_carrier_delta()?)?;
    }
    let mut core_proofs = expected_projection::CoreProviderProofs::new(authored, programs)
        .map_err(|error| vec![error])?;
    let mut receiver = Some(constructor_failure::receiving::Receiver::acquire()?);
    let mut synthetic_ast_peak = 0usize;
    let mut selected_static_modules = static_text::SelectedModules::default();
    for index in 0..programs.len() {
        let program = &programs[compact_order.map_or(index, |order| order[index])];
        if retained_output_only {
            staged!(
                UncachedSyntheticPeak,
                expected_projection::uncached_peak::charge_uncached_synthetic_ast_core(
                    program,
                    authored,
                    programs,
                    &mut synthetic_ast_peak,
                    Some(layouts),
                    &mut core_proofs,
                )
            )?;
        }
        let synthetic = staged!(
            SyntheticModule,
            constructor_failure::typed::construct_transient_core(
                program,
                authored,
                programs,
                layouts,
                &mut core_proofs,
                receiver.as_mut().expect("live constructor receiver"),
            )
            .map_err(|failure| receiver
                .take()
                .expect("first constructor error exits core")
                .receive_typed(failure))
        )?;
        // Exact stubs/spans and the unconditional prebound govern hits.
        let cached = staged!(
            FrontendLookup,
            frontend.as_deref_mut().and_then(|cache| match &synthetic {
                expected_projection::borrowed_leaf::Input::Owned(input) =>
                    cache.checked_module(&program.path, input),
                expected_projection::borrowed_leaf::Input::Borrowed(input) =>
                    cache.checked_module_source(&program.path, input),
                expected_projection::borrowed_leaf::Input::Transient(_) =>
                    unreachable!("complete old constructor cannot produce transient function view"),
            })
        );
        let shared_before = core_proofs.base_prelude.selected_bytes();
        let (resolved, static_text) = if let Some((resolved, resolver_bytes, original_loan_bytes)) =
            cached
        {
            let before = crate::bounded_output::active_remaining()
                .expect("resolved core has a builder budget");
            let static_text = staged!(CachedValidation, hir::validate_with_static_text(&resolved))
                .map_err(|error| vec![error])?;
            identity_resolution::replay_cached_prelude(&mut core_proofs, &synthetic)
                .map_err(identity_resolution::ModuleFailure::into_legacy)?;
            let validation_bytes =
                identity_resolution::local_resolution_bytes(before, shared_before, &core_proofs)?;
            let residual = resolver_bytes
                .checked_sub(validation_bytes)
                .ok_or_else(|| {
                    vec![graph_error(
                        "SPX-G257",
                        "checked module replay accounting exceeds its original resolver work",
                    )]
                })?;
            // Charge only the residual resolver cost after real revalidation.
            charge_builder_prebound(residual)?;
            // Preserve proof-capacity charging and the cold-equivalent residual.
            let loan_residual = original_loan_bytes
                .checked_sub(resolved_loan_bytes(&resolved)?)
                .ok_or_else(|| {
                    vec![graph_error(
                        "SPX-G257",
                        "checked module clone exceeds its original loan-plan capacity accounting",
                    )]
                })?;
            charge_builder_prebound(loan_residual)?;
            (ResolvedModuleInput::Checked(resolved), static_text)
        } else {
            let before = crate::bounded_output::active_remaining()
                .expect("resolved core has a builder budget");
            // Diagnostics name private layout copies by provider provenance
            // through a borrowed view of the already held closure; binding it
            // derives, debits and allocates nothing.
            let held = layouts.held(program);
            let private = held
                .as_deref()
                .map(|layout| layout.presentation(program, synthetic.source()))
                .transpose()?
                .flatten();
            // Typed receiver: one identity report is acquired on this same
            // resolved-core ledger before either route, and Setup stays Copy
            // until this legacy outer adapter.
            let (resolved, function_costs, reused_functions, static_text) =
                identity_resolution::resolve_module_with_core(
                    &mut core_proofs,
                    frontend.as_deref(),
                    &program.path,
                    &synthetic,
                    private,
                )
                .map_err(identity_resolution::ModuleFailure::into_legacy)?;
            drop(held);
            let resolver_bytes =
                identity_resolution::local_resolution_bytes(before, shared_before, &core_proofs)?;
            let resolved = if let Some(cache) = frontend.as_deref_mut() {
                staged!(FrontendRetention, {
                    let loan_bytes = resolved_loan_bytes(&resolved)?;
                    cache.resolved_module_split(
                        &program.path,
                        || synthetic.into_split_key(index),
                        resolved,
                        resolver_bytes,
                        loan_bytes,
                        function_costs,
                        reused_functions,
                    )
                })?
            } else {
                ResolvedModuleInput::Owned(resolved)
            };
            (resolved, static_text)
        };
        staged!(
            ResolvedCallEdges,
            verify_resolved_call_edges(program, &resolved, authored)
        )?;
        if let Some(validation) = validation.as_mut() {
            staged!(
                AuthoredModuleValidation,
                validation.record_module_authored(
                    &program.module,
                    &resolved,
                    programs,
                    authored,
                    Some(&authority),
                )
            )?;
            let (module, imported_instances) = staged!(
                RetainedModule,
                retain_workspace_module(
                    program,
                    crate::bounded_output::budgeted_clone(&program.module),
                    resolved,
                    programs,
                    authored,
                    Some(&mut loan_scratch),
                    static_text,
                    authenticated_sources,
                )
            )?;
            staged!(
                ImportedVecInstances,
                owned_generics::merge_imported_vec_instances(
                    &mut imported_vec_instances,
                    imported_instances,
                )
            )?;
            modules.push(module);
        } else {
            selected_static_modules.record(synthetic_modules.len(), programs.len(), static_text)?;
            synthetic_modules.push((
                crate::bounded_output::budgeted_clone(&program.module),
                resolved,
            ));
        }
    }
    if retained_output_only {
        modules.sort_unstable_by(|left, right| left.path.cmp(&right.path));
    }
    let declarations = if let Some(validation) = validation {
        staged!(
            StubSignatures,
            validation.validate_stub_signatures_authored(programs, authored, Some(&authority))
        )?;
        staged!(DeclarationFacts, validation.finish(programs))?
    } else {
        staged!(
            StubSignatures,
            validate_stub_signatures_authored(
                programs,
                &synthetic_modules,
                authored,
                Some(&authority)
            )
        )?;
        let declarations = staged!(
            DeclarationFacts,
            reconstruct_workspace_declaration_facts_with_authority(
                &synthetic_modules,
                programs,
                authored,
                Some(&authority)
            )
        )?;
        let programs_by_module = programs
            .iter()
            .map(|program| (program.module.as_str(), program))
            .collect::<BTreeMap<_, _>>();
        reserve_workspace_module_carrier(synthetic_modules.len())?;
        modules = Vec::with_capacity(synthetic_modules.len());
        for (module_index, (module, resolved)) in synthetic_modules.into_iter().enumerate() {
            let program = programs_by_module
                .get(module.as_str())
                .expect("every resolved module has authenticated source");
            let (module, imported_instances) = staged!(
                RetainedModule,
                retain_workspace_module(
                    program,
                    module,
                    resolved,
                    programs,
                    authored,
                    None,
                    selected_static_modules.contains(module_index),
                    authenticated_sources,
                )
            )?;
            staged!(
                ImportedVecInstances,
                owned_generics::merge_imported_vec_instances(
                    &mut imported_vec_instances,
                    imported_instances,
                )
            )?;
            modules.push(module);
        }
        declarations
    };
    staged!(
        ImportedVecInstances,
        owned_generics::attach_imported_vec_instances(&mut modules, imported_vec_instances)
    )?;
    staged!(
        RetainedFacts,
        retained_validation::validate_core_facts(
            programs,
            &modules,
            &expected_edges,
            compact_order.is_some(),
        )
    )?;
    staged!(
        RetainedFacts,
        validate_retained_declaration_shapes(&modules, &declarations)
    )?;
    let owned_module_paths = staged!(
        CoreDirectories,
        module_paths
            .iter()
            .map(|(module, path)| {
                (
                    crate::bounded_output::budgeted_clone(module),
                    crate::bounded_output::budgeted_clone(path),
                )
            })
            .collect()
    );
    if dependency_depths.len() != module_paths.len()
        || dependency_depths
            .keys()
            .any(|module| !module_paths.contains_key(module))
    {
        return Err(vec![graph_error(
            "SPX-G173",
            "workspace dependency-depth keys disagree with authenticated modules",
        )]
        .into());
    }
    let owned_dependency_depths = staged!(
        CoreDirectories,
        dependency_depths
            .iter()
            .map(|(module, depth)| (crate::bounded_output::budgeted_clone(module), *depth))
            .collect()
    );
    Ok((
        modules,
        owned_module_paths,
        owned_dependency_depths,
        declarations,
        expected_edges,
    ))
}

#[cfg(test)]
#[path = "before_core/tests.rs"]
mod tests;

// Test-only selection preserves the complete old constructor above. It has no
// cache, quota mutation, warmup or production configuration surface.
thread_local! {
    static PARITY: std::cell::Cell<Option<(bool, usize)>> = const { std::cell::Cell::new(None) };
}
pub(in crate::workspace_graph) fn selected() -> bool {
    PARITY.with(|state| state.get().is_some_and(|(old, _)| old))
}
pub(in crate::workspace_graph) fn borrowed_entry() {
    PARITY.with(|state| {
        if let Some((old, count)) = state.get() {
            assert!(
                !old,
                "the independent old constructor must never borrow alias payloads"
            );
            state.set(Some((old, count + 1)));
        }
    });
}
fn parity_run<T>(old: bool, run: impl FnOnce() -> T) -> (T, usize) {
    struct Restore(Option<(bool, usize)>);
    impl Drop for Restore {
        fn drop(&mut self) {
            PARITY.with(|state| state.set(self.0));
        }
    }
    let _restore = Restore(PARITY.with(|state| state.replace(Some((old, 0)))));
    let result = run();
    let count = PARITY.with(|state| state.get().unwrap().1);
    (result, count)
}
