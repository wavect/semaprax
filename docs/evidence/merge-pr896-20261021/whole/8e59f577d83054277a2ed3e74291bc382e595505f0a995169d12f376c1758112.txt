//! Complete original cached/ordinary core receiver, independently compiled.
use super::*;

// Keep the source-authority inputs explicit at this audited construction boundary.
#[allow(clippy::too_many_arguments)]
pub(super) fn build_resolved_core(
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
    let mut synthetic_ast_peak = 0usize;
    let mut selected_static_modules = static_text::SelectedModules::default();
    for index in 0..programs.len() {
        let program = &programs[compact_order.map_or(index, |order| order[index])];
        if retained_output_only {
            staged!(
                UncachedSyntheticPeak,
                expected_projection::uncached_peak::charge_uncached_synthetic_ast_with_registry(
                    program,
                    authored,
                    programs,
                    &mut synthetic_ast_peak,
                    Some(layouts),
                )
            )?;
        }
        let synthetic = staged!(
            SyntheticModule,
            if frontend.is_none() {
                expected_projection::borrowed_leaf::Input::construct_transient(
                    program, authored, programs, layouts,
                )
            } else {
                expected_projection::borrowed_leaf::Input::construct_receiving(
                    program,
                    authored,
                    programs,
                    layouts,
                    frontend
                        .as_deref()
                        .is_none_or(|cache| cache.permits_borrowed_cold_source(&program.path)),
                )
            }
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
                identity_resolution::resolve_module(
                    frontend.as_deref(),
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
                    cache.resolved_module_receiving(
                        &program.path,
                        || synthetic.into_receiving_key(),
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

const MAX_CHECKED_VALUE_VISITS: usize = 1_048_576;
const MAX_CHECKED_VALUE_DEPTH: usize = 256;

#[derive(Clone, Copy)]
enum CheckedValueNode<'a> {
    Type(&'a hir::ResolvedType),
    Expression(&'a hir::ResolvedExpr),
    Statement(&'a hir::ResolvedStatement),
    Binding(&'a hir::ResolvedBinding),
    Field(&'a hir::ResolvedFieldInitializer),
    Arm(&'a hir::ResolvedMatchArm),
    Pattern(&'a hir::ResolvedMatchPattern),
    RecordField(&'a hir::ResolvedRecordMatchPatternField),
}

impl<'a> CheckedValueNode<'a> {
    fn ty(self) -> Option<&'a hir::ResolvedType> {
        match self {
            Self::Type(ty) => Some(ty),
            Self::Expression(expression) => Some(&expression.ty),
            Self::Binding(binding) => Some(&binding.ty),
            Self::Pattern(hir::ResolvedMatchPattern::Record { instance, .. }) => Some(instance),
            Self::RecordField(hir::ResolvedRecordMatchPatternField {
                pattern: hir::ResolvedRecordMatchFieldPattern::Record { instance, .. },
                ..
            }) => Some(instance),
            _ => None,
        }
    }

    fn child(self, index: usize) -> Option<Self> {
        use hir::ResolvedExprKind as E;
        use hir::ResolvedMatchPattern as P;
        use hir::ResolvedRecordMatchFieldPattern as F;
        use hir::ResolvedStatement as S;
        match self {
            Self::Type(_) | Self::Binding(_) => None,
            Self::Field(field) => (index == 0).then_some(Self::Expression(&field.value)),
            Self::Statement(statement) => match statement {
                S::Let { binding, value, .. } | S::Assign { binding, value, .. } => {
                    [Self::Binding(binding), Self::Expression(value)]
                        .get(index)
                        .copied()
                }
                S::CopyDiscard { .. }
                | S::Unsafe { .. }
                | S::While { .. }
                | S::LexicalWhile { .. } => statement.child(index).map(Self::Expression),
            },
            Self::Arm(arm) => match index {
                0 => Some(Self::Pattern(&arm.pattern)),
                1 => Some(Self::Expression(arm.guard.as_deref().unwrap_or(&arm.value))),
                2 if arm.guard.is_some() => Some(Self::Expression(&arm.value)),
                _ => None,
            },
            Self::Pattern(pattern) => match pattern {
                P::Variant { fields, .. } => {
                    fields.get(index).map(|field| Self::Binding(&field.binding))
                }
                P::Record { fields, .. } => fields.get(index).map(Self::RecordField),
                P::Binding(binding) => (index == 0).then_some(Self::Binding(binding)),
                P::Or(alternatives) => alternatives.get(index).map(Self::Pattern),
                P::Wildcard | P::Literal(_) => None,
            },
            Self::RecordField(field) => match &field.pattern {
                F::Binding(binding) => (index == 0).then_some(Self::Binding(binding)),
                F::Record { fields, .. } => fields.get(index).map(Self::RecordField),
                F::Wildcard => None,
            },
            Self::Expression(expression) => match &expression.kind {
                E::LexicalExit { value, .. } => (index == 0)
                    .then_some(value.as_deref())
                    .flatten()
                    .map(Self::Expression),
                E::Closure {
                    parameters,
                    captures,
                    body,
                } => {
                    if index < parameters.len() {
                        Some(Self::Binding(&parameters[index]))
                    } else if index < parameters.len() + captures.len() * 2 {
                        let offset = index - parameters.len();
                        if offset.is_multiple_of(2) {
                            Some(Self::Binding(&captures[offset / 2].binding))
                        } else {
                            Some(Self::Expression(&captures[offset / 2].value))
                        }
                    } else {
                        (index == parameters.len() + captures.len() * 2)
                            .then_some(Self::Expression(body))
                    }
                }
                E::ByteRange {
                    source, start, end, ..
                } => [source.as_ref(), start.as_ref(), end.as_ref()]
                    .get(index)
                    .copied()
                    .map(Self::Expression),
                E::Call { args, .. }
                | E::LiteralFormat { args, .. }
                | E::VecFieldRead { args, .. }
                | E::RowLoan { args, .. } => args.get(index).map(Self::Expression),
                E::Invoke { callable, args } => {
                    if index == 0 {
                        Some(Self::Expression(callable))
                    } else {
                        args.get(index - 1).map(Self::Expression)
                    }
                }
                E::NativeRustImportCall(call) => call.args.get(index).map(Self::Expression),
                E::HostCommandCall(call) => call.args.get(index).map(Self::Expression),
                E::Unary { value, .. }
                | E::Project { base: value, .. }
                | E::Upcast { source: value }
                | E::Yield { request: value } => (index == 0).then_some(Self::Expression(value)),
                E::Try {
                    operand,
                    residual_type,
                    ..
                }
                | E::TryOption {
                    operand,
                    residual_type,
                    ..
                } => [Self::Expression(operand), Self::Type(residual_type)]
                    .get(index)
                    .copied(),
                E::Binary { left, right, .. } => [left.as_ref(), right.as_ref()]
                    .get(index)
                    .copied()
                    .map(Self::Expression),
                E::Block { statements, tail }
                | E::VecConstructor {
                    statements, tail, ..
                } => {
                    if index < statements.len() {
                        statements.get(index).map(Self::Statement)
                    } else {
                        (index == statements.len()).then_some(Self::Expression(tail))
                    }
                }
                E::If {
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
                .map(Self::Expression),
                E::ConstructRecord { fields, .. } | E::ConstructVariant { fields, .. } => {
                    fields.get(index).map(Self::Field)
                }
                E::Match {
                    scrutinee, arms, ..
                } => {
                    if index == 0 {
                        Some(Self::Expression(scrutinee))
                    } else {
                        arms.get(index - 1).map(Self::Arm)
                    }
                }
                E::UpdateRecord { base, fields, .. } => {
                    if index == 0 {
                        Some(Self::Expression(base))
                    } else {
                        fields.get(index - 1).map(Self::Field)
                    }
                }
                E::Int(_)
                | E::Int32(_)
                | E::Char(_)
                | E::Uint8(_)
                | E::Usize(_)
                | E::Float32(_)
                | E::Float64(_)
                | E::Bool(_)
                | E::String(_)
                | E::BorrowedUtf8Literal(_)
                | E::ArrayU8(_)
                | E::RepeatArrayU8 { .. }
                | E::BorrowPlace { .. }
                | E::Place(_)
                | E::FunctionReference { .. } => None,
            },
        }
    }
}

use crate::workspace_graph::before_private_roots::reconstruct_workspace_declaration_facts_with_authority;
use crate::workspace_graph::validation::before_roots::{
    validate_stub_signatures_authored, WorkspaceValidationIndex,
};
