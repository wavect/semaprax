//! Uncached coupled census; the complete immediate-old estimate stays live.
//! Only local-callee acquisition changes. Fixed/nominal/expression/writer and
//! all sixteen identity copies retain their original independent equations.
use super::{
    active_builder_limit, ast_function_contract_cost, ast_function_cost,
    ast_function_identity_slots, ast_function_signature_cost, ast_program_cost,
    ast_program_identity_slots, ast_type_declaration_cost, ast_type_declaration_identity_slots,
    ast_type_identity_slots, cached_exact, call_identity, checked_builder_sum, checked_usage, cost,
    generic_instance_source_cost, import_stub, limit_error, private_layout,
    synthetic_main_runtime_cost, type_rewrite, AuthoredDeclaration, StructuralCost,
    SyntheticBuilderCosts, HIR_IDENTITY_COPY_FACTOR, HIR_STRING_COPY_FACTOR,
    HIR_STRUCTURE_EXPANSION_FACTOR,
};
use crate::ast::{ModuleUseKind, Program};
use crate::diagnostic::Diagnostic;
use crate::{hir, prelude};
use std::collections::{BTreeMap, BTreeSet};

#[allow(
    clippy::too_many_arguments,
    reason = "complete original forecast inputs plus same-invocation census loan"
)]
pub(in crate::workspace_graph::expected_projection) fn estimate_with_census<'p, 'a>(
    program: &'p Program,
    authored: &'a BTreeMap<&'a str, AuthoredDeclaration<'a>>,
    programs: &'a [Program],
    maximum_identity: Option<usize>,
    layout_mode: u8,
    include_imports: bool,
    registry: Option<&private_layout::registry::LayoutRegistry<'_>>,
    census: &mut super::super::shared_local_callees::modular_pair::Census<'p, 'a>,
) -> Result<SyntheticBuilderCosts, Vec<Diagnostic>> {
    if layout_mode != 6 {
        return Err(vec![limit_error("builder_bytes", active_builder_limit())]);
    }
    let core = None;
    let retention = cached_exact::Retention::Parsed;
    let mut raw = StructuralCost::raw_ast(layout_mode >= 1).with_inline_values(layout_mode >= 2);
    ast_program_cost(program, &mut raw, include_imports)?;
    let (call_data, shared_callees) =
        super::super::shared_local_callees::modular_pair::read_or_count(
            Some(census),
            super::super::shared_local_callees::Context {
                program,
                authored,
                programs,
            },
            include_imports,
        )?;
    raw.exact_carrier_free_calls = call_data.roots;
    let call_proof = call_data.bytes;
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
                super::super::super::owned_function_import::admitted_with_layout(
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
            super::super::super::generic_type_import::rewrite_declaration_runtime_cost_with_layout(
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
    #[cfg(any())]
    let prior_fixed_hir_upper = fixed_hir_upper;
    let fixed_hir_upper = if layout_mode == cached_exact::MODE {
        cached_exact::fixed_upper(fixed_hir_upper, &raw, &runtime, retention)?
    } else {
        fixed_hir_upper
            .checked_sub(cost::fixed_hir_discount(layout_mode, &raw, &runtime)?)
            .expect("literal storage discount retains the complete reduced fixed bundle")
    };
    // Only the final uncached route has no semantic-cache serialization owner.
    // This independently paid source certificate concerns local callee backing,
    // not the auxiliary-carrier absence counter, BASE, or an expression ID.
    let identity_slots = identity_slots
        .checked_sub(shared_callees.slots)
        .expect("a certified raw call retains its callee slot until this separate proof");
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
    let prelude_hir_upper = super::super::prelude_forecast::retained_upper(
        &prelude::base_schema::BASE,
        maximum_identity_bytes,
    )?;
    let hir_upper = fixed_hir_upper
        .checked_add(call_proof)
        .and_then(|bytes| bytes.checked_add(shared_callees.bytes))
        .and_then(|bytes| bytes.checked_add(prelude_hir_upper))
        .and_then(|bytes| bytes.checked_add(identity_carrier_bytes))
        .and_then(|bytes| bytes.checked_add(identity_occurrence_upper))
        .ok_or_else(|| vec![limit_error("builder_bytes", active_builder_limit())])?;
    let retained_clone_and_hir = checked_usage(
        raw.total,
        hir_upper,
        "builder_bytes",
        active_builder_limit(),
    )?;
    #[cfg(any())]
    let original_mode7_fixed_upper = raw
        .exact_expressions
        .checked_add(runtime.exact_expressions)
        .and_then(|count| {
            count
                .checked_mul(crate::workspace_graph::HIR_FIXED_EXPANSION_FACTOR)
                .and_then(|n| n.checked_mul(std::mem::size_of::<crate::ast::Expr>()))
                .and_then(|old| prior_fixed_hir_upper.checked_sub(old))
                .and_then(|base| {
                    count
                        .checked_mul(if retention == cached_exact::Retention::Semantic {
                            2
                        } else {
                            1
                        })
                        .and_then(|n| n.checked_mul(crate::workspace_graph::HIR_EXPR_FIXED_BUNDLE))
                        .and_then(|exact| base.checked_add(exact))
                })
        })
        .unwrap_or(usize::MAX);
    #[cfg(any())]
    super::super::super::forecast_observation::record_components(
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
