//! Complete original core constructor, independently compiled; body unchanged.
use crate::bounded_output::staged;
use crate::workspace_graph::{
    AuthoredDeclaration, ResolvedCore, ResolvedModuleInput, WorkspaceSource,
    charge_builder_prebound, checked_retention, collect_expected_edges,
    constructor_failure, expected_projection, graph_error, identity_resolution, loan_retention,
    owned_generics,
    reserve_builder_structure, reserve_workspace_module_carrier, resolved_loan_bytes,
    retain_workspace_module, retained_validation, static_text,
    validate_retained_declaration_shapes,
    verify_resolved_call_edges,
};
use crate::{ast::Program, diagnostic::Diagnostic, hir};
use std::cell::Cell;
use std::collections::BTreeMap;
thread_local! { static ACTIVE: Cell<bool> = const { Cell::new(false) }; }
pub(in crate::workspace_graph) fn active() -> bool {
    ACTIVE.with(Cell::get)
}
pub(super) fn with_before<T>(operation: impl FnOnce() -> T) -> T {
    struct Restore(bool);
    impl Drop for Restore {
        fn drop(&mut self) {
            ACTIVE.with(|slot| slot.set(self.0));
        }
    }
    let _restore = Restore(ACTIVE.with(|slot| slot.replace(true)));
    operation()
}
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
) -> Result<ResolvedCore, Vec<Diagnostic>> {
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
            constructor_failure::construct_transient_core(
                program,
                authored,
                programs,
                layouts,
                &mut core_proofs,
            )
            .map_err(constructor_failure::Failure::into_legacy_unfunded)
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
        let (resolved, static_text) = if let Some((resolved, resolver_bytes, original_loan_bytes)) =
            cached
        {
            let before = crate::bounded_output::active_remaining()
                .expect("resolved core has a builder budget");
            let static_text = staged!(CachedValidation, hir::validate_with_static_text(&resolved))
                .map_err(|error| vec![error])?;
            let validation_bytes = before.saturating_sub(
                crate::bounded_output::active_remaining()
                    .expect("resolved core has a builder budget"),
            );
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
                    &mut core_proofs,frontend.as_deref(),
                    &program.path,
                    &synthetic,
                    private,
                )
                .map_err(identity_resolution::ModuleFailure::into_legacy)?;
            drop(held);
            let resolver_bytes = before.saturating_sub(
                crate::bounded_output::active_remaining()
                    .expect("resolved core has a builder budget"),
            );
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
        )]);
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

use crate::workspace_graph::validation::before_roots::{WorkspaceValidationIndex, validate_stub_signatures_authored};
use crate::workspace_graph::before_private_roots::reconstruct_workspace_declaration_facts_with_authority;
