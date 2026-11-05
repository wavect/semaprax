//! Shared source-held closure consumption for forecast and synthetic construction.
use super::{
    active_builder_limit, ast_function_contract_cost, ast_function_cost,
    ast_function_identity_slots, ast_function_signature_cost, ast_program_cost,
    ast_program_identity_slots, ast_type_declaration_cost, ast_type_declaration_identity_slots,
    ast_type_identity_slots, cached_exact, call_identity, checked_builder_sum, checked_usage, cost,
    default_expr, generic_instance_source_cost, graph_error, import_stub, limit_error,
    private_layout, reserve_builder_structure, synthetic_main_runtime_cost, type_rewrite,
    AuthoredDeclaration, StructuralCost, SyntheticBuilderCosts, HIR_IDENTITY_COPY_FACTOR,
    HIR_STRING_COPY_FACTOR, HIR_STRUCTURE_EXPANSION_FACTOR,
};
use crate::ast::{Expr, ExprKind, Function, ModuleUseKind, Program, Span, Type, TypeDeclaration};
use crate::bounded_output::budgeted_format;
use crate::diagnostic::Diagnostic;
use crate::{hir, prelude};
use std::collections::{BTreeMap, BTreeSet};
pub(in crate::workspace_graph) fn synthetic_builder_bytes_scoped_with_retention_with_registry(
    program: &Program,
    authored: &BTreeMap<&str, AuthoredDeclaration<'_>>,
    programs: &[Program],
    maximum_identity: Option<usize>,
    include_imports: bool,
    profile: (u8, cached_exact::Retention),
    registry: Option<&private_layout::registry::LayoutRegistry<'_>>,
) -> Result<SyntheticBuilderCosts, Vec<Diagnostic>> {
    synthetic_builder_bytes_scoped_with_retention_with_registry_using(
        program,
        authored,
        programs,
        maximum_identity,
        include_imports,
        profile,
        (registry, None),
    )
}

pub(in crate::workspace_graph) fn synthetic_builder_bytes_core<'a>(
    program: &Program,
    authored: &'a BTreeMap<&'a str, AuthoredDeclaration<'a>>,
    programs: &'a [Program],
    maximum_identity: Option<usize>,
    include_imports: bool,
    profile: (u8, cached_exact::Retention),
    proofs: (
        Option<&private_layout::registry::LayoutRegistry<'_>>,
        &mut import_stub::core_proofs::CoreProviderProofs<'a>,
    ),
) -> Result<SyntheticBuilderCosts, Vec<Diagnostic>> {
    synthetic_builder_bytes_scoped_with_retention_with_registry_using(
        program,
        authored,
        programs,
        maximum_identity,
        include_imports,
        profile,
        (proofs.0, Some(proofs.1)),
    )
}

pub(in crate::workspace_graph) fn synthetic_builder_bytes_scoped_with_retention_with_registry_using<
    'a,
>(
    program: &Program,
    authored: &'a BTreeMap<&'a str, AuthoredDeclaration<'a>>,
    programs: &'a [Program],
    maximum_identity: Option<usize>,
    include_imports: bool,
    profile: (u8, cached_exact::Retention),
    proofs: (
        Option<&private_layout::registry::LayoutRegistry<'_>>,
        Option<&mut import_stub::core_proofs::CoreProviderProofs<'a>>,
    ),
) -> Result<SyntheticBuilderCosts, Vec<Diagnostic>> {
    let (registry, core) = proofs;
    let (layout_mode, retention) = profile;
    let mut raw = StructuralCost::raw_ast(layout_mode >= 1).with_inline_values(layout_mode >= 2);
    ast_program_cost(program, &mut raw, include_imports)?;
    let fresh;
    let held;
    let layout = if let Some(registry) = registry {
        held = registry.get(program, authored, programs)?;
        &*held
    } else {
        fresh = private_layout::LayoutClosure::derive(program, authored, programs)?;
        &fresh
    };
    let mut identity_slots = ast_program_identity_slots(program)?;
    let mut runtime = StructuralCost::new();
    let mut default_memo = [BTreeMap::new(), BTreeMap::new()];
    let mut byte_proofs = import_stub::core_proofs::Phase::new(authored, programs, core)
        .map_err(|error| vec![error])?;
    let mut transient_import_clone = 0usize;
    layout.add_costs(
        program,
        programs,
        &mut raw,
        &mut runtime,
        &mut identity_slots,
    )?;
    for module_use in program.module_uses.iter().filter(|_| include_imports) {
        if module_use.kind == ModuleUseKind::Protocol {
            continue;
        }
        let target = &authored[module_use.persistent_id.as_str()];
        runtime.string(&module_use.alias)?;
        if let Some(function) = target.function {
            // `synthetic_program` retains an imported function as a stub: the
            // signature it rewrites, with `requires` and `ensures` cleared and
            // the body replaced by the return type's default expression. Only
            // the retained signature is charged here; the default body is
            // charged as runtime cost below. The provider's own contract and
            // body reach the resolver exactly once, through the defining
            // module's own `ast_program_cost`.
            ast_function_signature_cost(function, &mut raw)?;
            // Preserve old fitting receipts even though ordinary stubs now
            // clone only their signature. The historical transient-body debit
            // remains conservative; only mode 4 removes its peak after all
            // older estimates refuse. Intrinsic wrapper bodies are separately
            // charged as retained runtime structures below.
            transient_import_clone =
                transient_import_clone.max(ast_function_contract_cost(function)?.total);
            if programs
                .iter()
                .find(|provider| provider.module == target.module)
                .is_some_and(|provider| {
                    crate::vec_ops::source_wrapper(provider, function).is_some()
                        || crate::box_ops::source_wrapper(provider, function).is_some()
                })
            {
                ast_function_cost(function, &mut runtime)?;
                identity_slots =
                    checked_builder_sum(identity_slots, ast_function_identity_slots(function)?)?;
                continue;
            }
            identity_slots = checked_builder_sum(
                identity_slots,
                ast_type_identity_slots(&function.return_type)?,
            )?;
            for param in &function.params {
                identity_slots =
                    checked_builder_sum(identity_slots, ast_type_identity_slots(&param.ty)?)?;
                type_rewrite::runtime_cost_with_layout(
                    &param.ty,
                    target.module,
                    program,
                    programs,
                    &mut runtime,
                    Some(layout),
                )?;
            }
            type_rewrite::runtime_cost_with_layout(
                &function.return_type,
                target.module,
                program,
                programs,
                &mut runtime,
                Some(layout),
            )?;
            let cost = import_stub::default_expr_expanded_cost_with_layout(
                function,
                target.module,
                program,
                authored,
                programs,
                &mut default_memo,
                &mut BTreeSet::new(),
                super::super::owned_function_import::admitted_with_layout(
                    program, target, authored, programs, layout,
                ),
                import_stub::byte_helper_authorized(
                    function,
                    target.module,
                    byte_proofs.get().map_err(|error| vec![error])?,
                )?,
                Some(layout),
            )?;
            runtime.add_split(cost.bytes, cost.string_bytes)?;
            runtime.add_identity_carriers(cost.identity_carriers)?;
            identity_slots = checked_builder_sum(identity_slots, cost.identity_slots)?;
        } else {
            let declaration = target.ty.expect("validated type target");
            ast_type_declaration_cost(declaration, &mut raw)?;
            identity_slots = checked_builder_sum(
                identity_slots,
                ast_type_declaration_identity_slots(declaration)?,
            )?;
            super::super::generic_type_import::rewrite_declaration_runtime_cost_with_layout(
                declaration,
                target.module,
                program,
                programs,
                &mut runtime,
                Some(layout),
            )?;
        }
    }
    if !program
        .functions
        .iter()
        .any(|function| function.name == "main")
    {
        let synthetic_main = synthetic_main_runtime_cost(&program.module)?;
        runtime.add_split(synthetic_main.total, synthetic_main.string_bytes)?;
        runtime.add_identity_carriers(synthetic_main.identity_carriers)?;
    }
    if layout_mode >= 2 {
        identity_slots = identity_slots
            .checked_sub(call_identity::discount_with_imports(
                program,
                authored,
                include_imports,
            ))
            .expect("scalar calls retain their expression and callee identity slots");
    }
    identity_slots = identity_slots
        .checked_sub(raw.scalar_identity_discount)
        .expect("raw scalar discount retains each expression identity slot");
    let generic_instances = generic_instance_source_cost(program)?;
    identity_slots = checked_builder_sum(identity_slots, generic_instances.identity_slots)?;
    let hir_input = checked_usage(
        raw.total,
        runtime.total,
        "builder_bytes",
        active_builder_limit(),
    )?;
    let hir_input = checked_usage(
        hir_input,
        generic_instances.bytes,
        "builder_bytes",
        active_builder_limit(),
    )?;
    let string_input = checked_builder_sum(raw.string_bytes, runtime.string_bytes)?;
    let string_input = checked_builder_sum(string_input, generic_instances.string_bytes)?;
    // String contents are part of `hir_input`; only the remaining structural
    // bytes expand by the structural factor.
    let structure_input = hir_input.saturating_sub(string_input);
    let fixed_hir_upper = structure_input
        .checked_mul(HIR_STRUCTURE_EXPANSION_FACTOR)
        .and_then(|structure| {
            string_input
                .checked_mul(HIR_STRING_COPY_FACTOR)
                .and_then(|strings| structure.checked_add(strings))
        })
        .ok_or_else(|| vec![limit_error("builder_bytes", active_builder_limit())])?;
    let maximum_identity_bytes = maximum_identity.unwrap_or_else(|| {
        authored
            .keys()
            .map(|id| id.len())
            .chain(prelude::all_ids().into_iter().map(str::len))
            .max()
            .unwrap_or(0)
    });
    #[cfg(test)]
    let prior_fixed_hir_upper = fixed_hir_upper;
    let fixed_hir_upper = if layout_mode == cached_exact::MODE {
        cached_exact::fixed_upper(fixed_hir_upper, &raw, &runtime, retention)?
    } else {
        fixed_hir_upper
            .checked_sub(cost::fixed_hir_discount(layout_mode, &raw, &runtime)?)
            .expect("literal storage discount retains the complete reduced fixed bundle")
    };
    let identity_occurrence_upper = identity_slots
        .checked_mul(maximum_identity_bytes)
        .and_then(|bytes| bytes.checked_mul(HIR_IDENTITY_COPY_FACTOR))
        .ok_or_else(|| vec![limit_error("builder_bytes", active_builder_limit())])?;
    // Heap carriers are prebound independently of inline layouts and shared clones.
    let identity_carriers = cost::identity_carriers(layout_mode, &raw, &runtime)?;
    let identity_carriers =
        checked_builder_sum(identity_carriers, generic_instances.identity_carriers)?;
    let identity_carrier_bytes = identity_carriers
        .checked_mul(hir::ExpressionId::SHARED_ALLOCATION_CARRIER_BYTES)
        .ok_or_else(|| vec![limit_error("builder_bytes", active_builder_limit())])?;
    // The resolver's source chain also retains the compiler-owned base
    // variants. They are absent from synthetic AST and cannot spend an authored
    // expression discount. Borrow their producer metadata, without warming or
    // constructing the process prelude table during estimation.
    let prelude_hir_upper = super::prelude_forecast::retained_upper(
        &prelude::base_schema::BASE,
        maximum_identity_bytes,
    )?;
    let hir_upper = fixed_hir_upper
        .checked_add(prelude_hir_upper)
        .and_then(|bytes| bytes.checked_add(identity_carrier_bytes))
        .and_then(|bytes| bytes.checked_add(identity_occurrence_upper))
        .ok_or_else(|| vec![limit_error("builder_bytes", active_builder_limit())])?;
    let retained_clone_and_hir = checked_usage(
        raw.total,
        hir_upper,
        "builder_bytes",
        active_builder_limit(),
    )?;
    #[cfg(test)]
    let original_mode7_fixed_upper = raw
        .exact_expressions
        .checked_add(runtime.exact_expressions)
        .and_then(|count| {
            count
                .checked_mul(super::super::HIR_FIXED_EXPANSION_FACTOR)
                .and_then(|n| n.checked_mul(std::mem::size_of::<crate::ast::Expr>()))
                .and_then(|old| prior_fixed_hir_upper.checked_sub(old))
                .and_then(|base| {
                    count
                        .checked_mul(if retention == cached_exact::Retention::Semantic {
                            2
                        } else {
                            1
                        })
                        .and_then(|n| n.checked_mul(super::super::HIR_EXPR_FIXED_BUNDLE))
                        .and_then(|exact| base.checked_add(exact))
                })
        })
        .unwrap_or(usize::MAX);
    #[cfg(test)]
    super::super::forecast_observation::record_components(
        &program.path,
        layout_mode,
        retention == cached_exact::Retention::Semantic,
        include_imports,
        [
            raw.total,
            raw.string_bytes,
            raw.exact_expressions,
            raw.literal_fixed_discount,
            runtime.total,
            runtime.string_bytes,
            runtime.exact_expressions,
            structure_input,
            string_input,
            prior_fixed_hir_upper,
            fixed_hir_upper,
            identity_occurrence_upper,
            identity_carrier_bytes,
            retained_clone_and_hir,
            generic_instances.bytes,
            transient_import_clone,
            original_mode7_fixed_upper,
            prelude_hir_upper,
            hir_upper,
        ],
    );
    let result = SyntheticBuilderCosts {
        raw_clone_and_hir: checked_usage(
            retained_clone_and_hir,
            transient_import_clone,
            "builder_bytes",
            active_builder_limit(),
        )?,
        retained_clone_and_hir,
        transient_import_clone,
        runtime: runtime.total,
        retained_hir: hir_upper,
        synthetic_ast: checked_usage(
            raw.total,
            runtime.total,
            "builder_bytes",
            active_builder_limit(),
        )?,
    };
    byte_proofs.complete().map_err(|error| vec![error])?;
    Ok(result)
}

pub(in crate::workspace_graph) fn synthetic_program_with_registry(
    program: &Program,
    authored: &BTreeMap<&str, AuthoredDeclaration<'_>>,
    programs: &[Program],
    registry: Option<&private_layout::registry::LayoutRegistry<'_>>,
) -> Result<Program, Vec<Diagnostic>> {
    synthetic_program_with_registry_using(program, authored, programs, registry, None)
}

pub(in crate::workspace_graph) fn synthetic_program_core<'a>(
    program: &Program,
    authored: &'a BTreeMap<&'a str, AuthoredDeclaration<'a>>,
    programs: &'a [Program],
    registry: Option<&private_layout::registry::LayoutRegistry<'_>>,
    core: &mut import_stub::core_proofs::CoreProviderProofs<'a>,
) -> Result<Program, Vec<Diagnostic>> {
    synthetic_program_with_registry_using(program, authored, programs, registry, Some(core))
}

pub(in crate::workspace_graph) fn synthetic_program_with_registry_using<'a>(
    program: &Program,
    authored: &'a BTreeMap<&'a str, AuthoredDeclaration<'a>>,
    programs: &'a [Program],
    registry: Option<&private_layout::registry::LayoutRegistry<'_>>,
    core: Option<&mut import_stub::core_proofs::CoreProviderProofs<'a>>,
) -> Result<Program, Vec<Diagnostic>> {
    let fresh;
    let held;
    let layout = if let Some(registry) = registry {
        held = registry.get(program, authored, programs)?;
        &*held
    } else {
        fresh = private_layout::LayoutClosure::derive(program, authored, programs)?;
        &fresh
    };
    let mut byte_proofs = import_stub::core_proofs::Phase::new(authored, programs, core)
        .map_err(|error| vec![error])?;
    let mut synthetic = program.clone();
    synthetic.module_uses.clear();
    // Project validation already authenticated static implementation sidecars.
    // Runtime HIR has no witness or dispatch representation for them.
    synthetic.implementations.clear();
    for module_use in program
        .module_uses
        .iter()
        .filter(|item| item.kind == ModuleUseKind::Type)
    {
        let target = &authored[module_use.persistent_id.as_str()];
        let mut ty = target.ty.expect("validated type target").clone();
        ty.name = crate::bounded_output::budgeted_clone(&module_use.alias);
        super::super::generic_type_import::rewrite_declaration_with_layout(
            &mut ty,
            target.module,
            program,
            programs,
            Some(layout),
        )?;
        synthetic.types.push(ty);
    }
    layout.append_types(&mut synthetic, program, programs)?;
    let type_index_bytes = synthetic
        .types
        .len()
        .checked_mul(std::mem::size_of::<(&str, &TypeDeclaration)>())
        .ok_or_else(|| vec![limit_error("builder_bytes", active_builder_limit())])?;
    reserve_builder_structure(type_index_bytes)?;
    let mut type_declarations = Vec::with_capacity(synthetic.types.len());
    for declaration in &synthetic.types {
        type_declarations.push((declaration.name.as_str(), declaration));
    }
    type_declarations.sort_by(|left, right| left.0.cmp(right.0));
    if type_declarations
        .windows(2)
        .any(|items| items[0].0 == items[1].0)
    {
        return Err(vec![graph_error(
            "SPX-G173",
            "synthetic workspace type-name index is not unique",
        )]);
    }
    for module_use in program
        .module_uses
        .iter()
        .filter(|item| item.kind == ModuleUseKind::Function)
    {
        let target = &authored[module_use.persistent_id.as_str()];
        let target_function = target.function.expect("validated function target");
        let retain_body = programs
            .iter()
            .find(|provider| provider.module == target.module)
            .is_some_and(|provider| {
                crate::vec_ops::source_wrapper(provider, target_function).is_some()
                    || crate::box_ops::source_wrapper(provider, target_function).is_some()
            });
        let mut function = if retain_body {
            target_function.clone()
        } else {
            import_stub::signature(target_function)
        };
        function.name = crate::bounded_output::budgeted_clone(&module_use.alias);
        if retain_body {
            synthetic.functions.push(function);
            continue;
        }
        for param in &mut function.params {
            type_rewrite::rewrite_with_layout(
                &mut param.ty,
                target.module,
                program,
                programs,
                Some(layout),
            )?;
        }
        type_rewrite::rewrite_with_layout(
            &mut function.return_type,
            target.module,
            program,
            programs,
            Some(layout),
        )?;
        // Clearing would retain the cloned contracts' Vec capacities in every
        // finished stub. Replace the vectors so the full imported contracts
        // remain transient, matching the final peak-only pre-bound.
        function.requires = Vec::new();
        function.ensures = Vec::new();
        function.body = default_expr(
            &function,
            &type_declarations,
            super::super::owned_function_import::admitted_with_layout(
                program, target, authored, programs, layout,
            ),
            import_stub::byte_helper_authorized(
                target_function,
                target.module,
                byte_proofs.get().map_err(|error| vec![error])?,
            )?,
        )?;
        synthetic.functions.push(function);
    }
    if !synthetic
        .functions
        .iter()
        .any(|function| function.name == "main")
    {
        reserve_builder_structure(std::mem::size_of::<Function>())?;
        reserve_builder_structure(std::mem::size_of::<Expr>())?;
        synthetic.functions.push(Function {
            stable_id: budgeted_format(format_args!(
                "workspace.synthetic.main.{}",
                synthetic.module
            )),
            explicit_id: true,
            name: crate::bounded_output::budgeted_clone("main"),
            name_span: Span::default(),
            type_parameters: Vec::new(),
            params: Vec::new(),
            return_type: Type::I64,
            effects: Vec::new(),
            yields: None,
            follows: None,
            requires: Vec::new(),
            ensures: Vec::new(),
            body: Expr {
                kind: ExprKind::Int(0),
                span: Span::default(),
            },
            span: Span::default(),
        });
    }
    byte_proofs.complete().map_err(|error| vec![error])?;
    Ok(synthetic)
}
