//! Pure deterministic authored-program projection used to prebind builder
//! costs, reconstruct dependency depth, and independently reconstruct the
//! expected cross-file edge set.
use private_layout::registry::LayoutRegistry;

use std::collections::{BTreeMap, BTreeSet};

use crate::ast::{
    Expr, ExprKind, Function, ModuleUse, ModuleUseKind, Program, Type, TypeDeclaration,
    TypeDeclarationKind,
};
use crate::bounded_output::budgeted_format;
use crate::diagnostic::Diagnostic;
use crate::{hir, prelude};

use super::{
    active_builder_limit, checked_usage, graph_error, limit_error, push_edge,
    reserve_builder_structure, resolve_type_id, visit_ast_call_sites, AuthoredDeclaration,
    WorkspaceEdge, HIR_IDENTITY_COPY_FACTOR, HIR_STRING_COPY_FACTOR,
    HIR_STRUCTURE_EXPANSION_FACTOR, MAX_DEPENDENCY_DEPTH,
};

#[path = "expected_projection/borrowed_imports.rs"]
pub(in crate::workspace_graph) mod borrowed_imports;
#[path = "expected_projection/borrowed_leaf.rs"]
pub(in crate::workspace_graph) mod borrowed_leaf;
#[path = "expected_projection/cached_exact.rs"]
pub(super) mod cached_exact;
#[path = "expected_projection/call_carriers.rs"]
mod call_carriers;
#[cfg(test)]
#[path = "expected_projection/call_carriers_original.rs"]
#[allow(dead_code, reason = "complete independent pre-898 forecaster control")]
mod call_carriers_original;
#[path = "expected_projection/call_identity.rs"]
mod call_identity;
#[path = "expected_projection/call_sites.rs"]
mod call_sites;
#[path = "expected_projection/cost.rs"]
pub(super) mod cost;
#[path = "expected_projection/declaration_cost.rs"]
mod declaration_cost;
#[path = "expected_projection/defaults.rs"]
mod defaults;
#[path = "expected_projection/held_projection.rs"]
mod held_projection;
#[path = "expected_projection/identity_slots.rs"]
mod identity_slots;
#[path = "expected_projection/import_stub.rs"]
mod import_stub;
#[path = "expected_projection/prelude_forecast.rs"]
mod prelude_forecast;
#[cfg(test)]
#[path = "expected_projection/prelude_forecast_original.rs"]
#[allow(
    dead_code,
    reason = "complete independent prelude-forecast reference includes its original construction receivers"
)]
mod prelude_forecast_original;
// CORE_PROOF_RECEIVERS_BEGIN
#[cfg(test)]
#[path = "expected_projection/borrowed_imports_original.rs"]
mod borrowed_imports_original;
#[cfg(test)]
#[path = "expected_projection/held_projection_original.rs"]
mod held_projection_original;
#[cfg(test)]
#[path = "expected_projection/import_stub_original.rs"]
mod import_stub_original;
pub(in crate::workspace_graph) use import_stub::core_proofs::CoreProviderProofs;
// CORE_PROOF_RECEIVERS_END
#[path = "expected_projection/local_identity.rs"]
mod local_identity;
#[path = "expected_projection/private_layout.rs"]
pub(super) mod private_layout;
pub(in crate::workspace_graph) use held_projection::{
    synthetic_builder_bytes_scoped_with_retention_with_registry, synthetic_program_with_registry,
};
#[path = "expected_projection/field_unique.rs"]
mod field_unique;
#[path = "expected_projection/record_traversal.rs"]
mod record_traversal;
#[path = "expected_projection/retention.rs"]
mod retention;
#[path = "expected_projection/sort_keys.rs"]
mod sort_keys;
#[path = "expected_projection/statement_segment.rs"]
pub(super) mod statement_segment;
mod string_slots;
#[path = "expected_projection/type_rewrite.rs"]
pub(super) mod type_rewrite;
#[path = "expected_projection/uncached_peak.rs"]
pub(super) mod uncached_peak;
mod vec_constructor;
use cost::{ExpandedDefaultCost, GenericInstanceCost, StructuralCost};
#[cfg(test)]
use retention::retention_prebound;
// A private source-call admission only; scalar/public export gates are separate.
pub(super) fn byte_writer_import_authorized<'a>(
    function: &'a Function,
    module: &str,
    authored: &'a BTreeMap<&'a str, AuthoredDeclaration<'a>>,
    programs: &'a [Program],
) -> Result<bool, Vec<Diagnostic>> {
    import_stub::byte_writer_import_authorized(function, module, authored, programs)
}

use declaration_cost::{
    ast_field_cost, ast_function_contract_cost, ast_function_cost, ast_function_signature_cost,
    ast_program_cost,
};
use identity_slots::{
    ast_function_identity_slots, ast_program_identity_slots, ast_type_declaration_identity_slots,
    ast_type_identity_slots,
};
use import_stub::default_expr;
pub(in crate::workspace_graph) use retention::checked_retention_prebound_with_uncached_peak_with_registry;
#[cfg(test)]
use uncached_peak::uncached_peak_prebound;
use uncached_peak::uncached_peak_prebound_with_registry;
#[cfg(test)]
pub(super) use uncached_peak::{
    initial_core_prebound, next_retention_prebound_with_uncached_peak,
    uncached_output_peak_prebound,
};
pub(in crate::workspace_graph) use uncached_peak::{
    initial_core_prebound_with_registry, next_retention_prebound_with_uncached_peak_with_registry,
    uncached_output_peak_prebound_with_registry,
};
pub(super) struct SyntheticBuilderCosts {
    pub(super) raw_clone_and_hir: usize,
    retained_clone_and_hir: usize,
    transient_import_clone: usize,
    pub(super) runtime: usize,
    retained_hir: usize,
    synthetic_ast: usize,
}
pub(super) fn synthetic_builder_bytes(
    program: &Program,
    authored: &BTreeMap<&str, AuthoredDeclaration<'_>>,
    programs: &[Program],
) -> Result<SyntheticBuilderCosts, Vec<Diagnostic>> {
    synthetic_builder_bytes_with_registry(program, authored, programs, None)
}
pub(in crate::workspace_graph) fn synthetic_builder_bytes_with_registry(
    program: &Program,
    authored: &BTreeMap<&str, AuthoredDeclaration<'_>>,
    programs: &[Program],
    registry: Option<&LayoutRegistry<'_>>,
) -> Result<SyntheticBuilderCosts, Vec<Diagnostic>> {
    synthetic_builder_bytes_scoped_with_registry(
        program, authored, programs, None, 0, true, registry,
    )
}
pub(super) fn synthetic_builder_bytes_scoped(
    program: &Program,
    authored: &BTreeMap<&str, AuthoredDeclaration<'_>>,
    programs: &[Program],
    maximum_identity: Option<usize>,
    layout_mode: u8,
    include_imports: bool,
) -> Result<SyntheticBuilderCosts, Vec<Diagnostic>> {
    synthetic_builder_bytes_scoped_with_registry(
        program,
        authored,
        programs,
        maximum_identity,
        layout_mode,
        include_imports,
        None,
    )
}
pub(in crate::workspace_graph) fn synthetic_builder_bytes_scoped_with_registry(
    program: &Program,
    authored: &BTreeMap<&str, AuthoredDeclaration<'_>>,
    programs: &[Program],
    maximum_identity: Option<usize>,
    layout_mode: u8,
    include_imports: bool,
    registry: Option<&LayoutRegistry<'_>>,
) -> Result<SyntheticBuilderCosts, Vec<Diagnostic>> {
    synthetic_builder_bytes_scoped_with_retention_with_registry(
        program,
        authored,
        programs,
        maximum_identity,
        include_imports,
        (layout_mode, cached_exact::Retention::Parsed),
        registry,
    )
}
fn synthetic_builder_bytes_scoped_with_retention(
    program: &Program,
    authored: &BTreeMap<&str, AuthoredDeclaration<'_>>,
    programs: &[Program],
    maximum_identity: Option<usize>,
    include_imports: bool,
    profile: (u8, cached_exact::Retention),
) -> Result<SyntheticBuilderCosts, Vec<Diagnostic>> {
    held_projection::synthetic_builder_bytes_scoped_with_retention_with_registry(
        program,
        authored,
        programs,
        maximum_identity,
        include_imports,
        profile,
        None,
    )
}

pub(super) fn checked_retention_prebound(
    programs: &[Program],
    authored: &BTreeMap<&str, AuthoredDeclaration<'_>>,
) -> Result<(usize, usize), Vec<Diagnostic>> {
    retention::checked_retention_prebound(programs, authored)
}
pub(super) fn checked_retention_prebound_with_uncached_peak(
    programs: &[Program],
    authored: &BTreeMap<&str, AuthoredDeclaration<'_>>,
    allow_uncached_peak: bool,
) -> Result<(usize, usize), Vec<Diagnostic>> {
    retention::checked_retention_prebound_with_uncached_peak(
        programs,
        authored,
        allow_uncached_peak,
    )
}
pub(super) fn retention_prebound_mode(
    programs: &[Program],
    authored: &BTreeMap<&str, AuthoredDeclaration<'_>>,
    dependency_scoped: bool,
    layout_mode: u8,
) -> Result<(usize, usize), Vec<Diagnostic>> {
    retention_prebound_mode_with_registry(programs, authored, dependency_scoped, layout_mode, None)
}
pub(in crate::workspace_graph) fn retention_prebound_mode_with_registry(
    programs: &[Program],
    authored: &BTreeMap<&str, AuthoredDeclaration<'_>>,
    dependency_scoped: bool,
    layout_mode: u8,
    registry: Option<&LayoutRegistry<'_>>,
) -> Result<(usize, usize), Vec<Diagnostic>> {
    retention::retention_prebound_mode_with_registry(
        programs,
        authored,
        dependency_scoped,
        layout_mode,
        registry,
    )
}
pub(super) fn next_retention_prebound(
    programs: &[Program],
    authored: &BTreeMap<&str, AuthoredDeclaration<'_>>,
    current: usize,
    layout_mode: &mut u8,
) -> Result<(usize, usize), Vec<Diagnostic>> {
    next_retention_prebound_with_registry(programs, authored, current, layout_mode, None)
}
pub(in crate::workspace_graph) fn next_retention_prebound_with_registry(
    programs: &[Program],
    authored: &BTreeMap<&str, AuthoredDeclaration<'_>>,
    current: usize,
    layout_mode: &mut u8,
    registry: Option<&LayoutRegistry<'_>>,
) -> Result<(usize, usize), Vec<Diagnostic>> {
    retention::next_retention_prebound_with_registry(
        programs,
        authored,
        current,
        layout_mode,
        registry,
    )
}

/// A synthetic module retains its own declarations and explicit imported
/// declarations; imported bodies are stubs except checked compiler wrappers.
/// Including *every* declaration in transitive provider modules is therefore
/// a superset of all declaration identities that its resolver can retain.
/// Reverse dependents and unrelated modules cannot become lookup authority.
/// Fixed stack scratch avoids allocating an uncharged traversal collection.
fn dependency_identity_max(
    program: &Program,
    authored: &BTreeMap<&str, AuthoredDeclaration<'_>>,
    programs: &[Program],
) -> Result<usize, Vec<Diagnostic>> {
    if programs.len() > super::MAX_FILES {
        return Err(vec![limit_error("files", super::MAX_FILES)]);
    }
    let mut reachable = [false; super::MAX_FILES];
    let own = programs
        .iter()
        .position(|candidate| candidate.module == program.module)
        .ok_or_else(|| vec![graph_error("SPX-G173", "identity scope module is absent")])?;
    reachable[own] = true;
    loop {
        let mut changed = false;
        for (index, module) in programs.iter().enumerate() {
            if !reachable[index] {
                continue;
            }
            for item in &module.module_uses {
                let target = programs
                    .iter()
                    .position(|candidate| candidate.module == item.target_module)
                    .ok_or_else(|| {
                        vec![graph_error("SPX-G173", "identity scope provider is absent")]
                    })?;
                if !reachable[target] {
                    reachable[target] = true;
                    changed = true;
                }
            }
        }
        if !changed {
            break;
        }
    }
    Ok(authored
        .iter()
        .filter(|(_, declaration)| {
            programs
                .iter()
                .enumerate()
                .any(|(index, module)| reachable[index] && module.module == declaration.module)
        })
        .map(|(id, _)| id.len())
        .chain(prelude::all_ids().into_iter().map(str::len))
        .max()
        .unwrap_or(0))
}

fn generic_instance_source_cost(program: &Program) -> Result<GenericInstanceCost, Vec<Diagnostic>> {
    let mut templates = Vec::with_capacity(program.functions.len());
    for function in &program.functions {
        if function.type_parameters.is_empty() {
            continue;
        }
        let mut cost = StructuralCost::new();
        ast_function_cost(function, &mut cost)?;
        templates.push((
            function.name.as_str(),
            GenericInstanceCost {
                bytes: cost.total,
                string_bytes: cost.string_bytes,
                identity_slots: ast_function_identity_slots(function)?,
                identity_carriers: cost.identity_carriers,
            },
        ));
    }
    templates.sort_by(|left, right| left.0.cmp(right.0));
    let mut total = GenericInstanceCost {
        bytes: 0,
        string_bytes: 0,
        identity_slots: 0,
        identity_carriers: 0,
    };
    for function in program
        .functions
        .iter()
        .filter(|function| function.type_parameters.is_empty())
    {
        for expression in function
            .requires
            .iter()
            .chain(std::iter::once(&function.body))
            .chain(&function.ensures)
        {
            let mut overflowed = false;
            expression.visit_call_instances(&mut |name, arguments, _| {
                if arguments.is_empty() || overflowed {
                    return;
                }
                if let Ok(index) = templates.binary_search_by_key(&name, |(name, _)| *name) {
                    if let (
                        Some(bytes),
                        Some(string_bytes),
                        Some(identity_slots),
                        Some(identity_carriers),
                    ) = (
                        total.bytes.checked_add(templates[index].1.bytes),
                        total
                            .string_bytes
                            .checked_add(templates[index].1.string_bytes),
                        total
                            .identity_slots
                            .checked_add(templates[index].1.identity_slots),
                        total
                            .identity_carriers
                            .checked_add(templates[index].1.identity_carriers),
                    ) {
                        total = GenericInstanceCost {
                            bytes,
                            string_bytes,
                            identity_slots,
                            identity_carriers,
                        };
                    } else {
                        overflowed = true;
                    }
                }
            });
            if overflowed || total.bytes > active_builder_limit() {
                return Err(vec![limit_error("builder_bytes", active_builder_limit())]);
            }
        }
    }
    Ok(total)
}
pub(super) fn checked_builder_sum(left: usize, right: usize) -> Result<usize, Vec<Diagnostic>> {
    left.checked_add(right)
        .filter(|total| *total <= active_builder_limit())
        .ok_or_else(|| vec![limit_error("builder_bytes", active_builder_limit())])
}
pub(super) fn rewrite_type_runtime_cost(
    ty: &Type,
    target_module: &str,
    caller: &Program,
    programs: &[Program],
    cost: &mut StructuralCost,
) -> Result<(), Vec<Diagnostic>> {
    type_rewrite::runtime_cost(ty, target_module, caller, programs, cost)
}
fn synthetic_main_runtime_cost(module: &str) -> Result<StructuralCost, Vec<Diagnostic>> {
    let mut cost = StructuralCost::new();
    cost.add(std::mem::size_of::<Function>())?;
    cost.add(std::mem::size_of::<Expr>())?;
    cost.add_identity_carriers(3)?;
    cost.string("main")?;
    cost.add_split(
        "workspace.synthetic.main.".len(),
        "workspace.synthetic.main.".len(),
    )?;
    cost.add_split(module.len(), module.len())?;
    Ok(cost)
}
fn ast_type_declaration_cost(
    declaration: &TypeDeclaration,
    cost: &mut StructuralCost,
) -> Result<(), Vec<Diagnostic>> {
    cost.value(declaration)?;
    cost.embedded_string(&declaration.stable_id)?;
    cost.embedded_string(&declaration.name)?;
    for parameter in &declaration.type_parameters {
        cost.value(parameter)?;
        cost.embedded_string(&parameter.name)?;
    }
    match &declaration.kind {
        TypeDeclarationKind::Resource { lifecycles } => {
            for lifecycle in lifecycles {
                cost.value(lifecycle)?;
                if let Some(id) = &lifecycle.stable_id {
                    cost.embedded_string(id)?;
                }
                if let crate::ast::ResourceLifecycleKind::Imported { import_key } = &lifecycle.kind
                {
                    cost.embedded_string(import_key)?;
                }
            }
        }
        TypeDeclarationKind::Record { fields } => {
            for field in fields {
                ast_field_cost(field, cost)?;
            }
        }
        TypeDeclarationKind::Class { fields, methods } => {
            for field in fields {
                ast_field_cost(field, cost)?;
            }
            for method in methods {
                ast_function_cost(method, cost)?;
            }
        }
        TypeDeclarationKind::Variant { cases } => {
            for case in cases {
                cost.value(case)?;
                cost.embedded_string(&case.stable_id)?;
                cost.embedded_string(&case.name)?;
                for field in &case.fields {
                    ast_field_cost(field, cost)?;
                }
            }
        }
    }
    Ok(())
}
fn ast_pattern_cost(
    pattern: &crate::ast::MatchPattern,
    cost: &mut StructuralCost,
) -> Result<(), Vec<Diagnostic>> {
    cost.value(pattern)?;
    match pattern {
        crate::ast::MatchPattern::Variant {
            type_name,
            case_name,
            fields,
            ..
        } => {
            cost.embedded_string(type_name)?;
            cost.embedded_string(case_name)?;
            for field in fields {
                cost.value(field)?;
                cost.embedded_string(&field.name)?;
                cost.embedded_string(&field.binding)?;
            }
        }
        crate::ast::MatchPattern::Record {
            type_name, fields, ..
        } => {
            cost.embedded_string(type_name)?;
            for field in fields {
                ast_record_pattern_field_cost(field, cost)?;
            }
        }
        crate::ast::MatchPattern::Wildcard { .. } => {}
        // Refutable Match v1: literal/or/binding structural costs.
        crate::ast::MatchPattern::Literal { value, .. } => {
            cost.string(value.type_text())?;
        }
        crate::ast::MatchPattern::Or { alternatives, .. } => {
            for alternative in alternatives {
                ast_pattern_cost(alternative, cost)?;
            }
        }
        crate::ast::MatchPattern::Binding { name, .. } => {
            cost.embedded_string(name)?;
        }
    }
    Ok(())
}
fn ast_record_pattern_field_cost(
    field: &crate::ast::RecordMatchPatternField,
    cost: &mut StructuralCost,
) -> Result<(), Vec<Diagnostic>> {
    cost.inline_pattern_parent(field, &field.pattern)?;
    cost.embedded_string(&field.name)?;
    cost.value(&field.pattern)?;
    match &field.pattern {
        crate::ast::RecordMatchFieldPattern::Binding { name, .. } => cost.embedded_string(name)?,
        crate::ast::RecordMatchFieldPattern::Record {
            type_name, fields, ..
        } => {
            cost.embedded_string(type_name)?;
            for field in fields {
                ast_record_pattern_field_cost(field, cost)?;
            }
        }
        crate::ast::RecordMatchFieldPattern::Wildcard { .. } => {}
    }
    Ok(())
}
#[allow(clippy::too_many_arguments)]
pub(super) fn validate_dependency_dag(
    programs: &[Program],
) -> Result<BTreeMap<&str, usize>, Vec<Diagnostic>> {
    let dependencies = programs
        .iter()
        .map(|program| {
            (
                program.module.as_str(),
                program
                    .module_uses
                    .iter()
                    .map(|item| item.target_module.as_str())
                    .collect::<BTreeSet<_>>(),
            )
        })
        .collect::<BTreeMap<_, _>>();
    let depths = dependency_depths(&dependencies)?;
    if depths.values().any(|depth| *depth > MAX_DEPENDENCY_DEPTH) {
        return Err(vec![limit_error("dependency_depth", MAX_DEPENDENCY_DEPTH)]);
    }
    Ok(depths)
}
pub(super) fn dependency_depths<'a>(
    dependencies: &BTreeMap<&'a str, BTreeSet<&'a str>>,
) -> Result<BTreeMap<&'a str, usize>, Vec<Diagnostic>> {
    fn visit<'a>(
        module: &'a str,
        dependencies: &BTreeMap<&'a str, BTreeSet<&'a str>>,
        stack: &mut Vec<&'a str>,
        depths: &mut BTreeMap<&'a str, usize>,
    ) -> Result<usize, Vec<Diagnostic>> {
        if let Some(index) = stack.iter().position(|item| *item == module) {
            let mut cycle = stack[index..].to_vec();
            cycle.push(module);
            let witness = canonical_cycle(&cycle);
            let witness = crate::bounded_output::budgeted_join(
                witness
                    .into_iter()
                    .map(crate::bounded_output::budgeted_clone),
                " -> ",
            );
            return Err(vec![graph_error(
                "SPX-G172",
                budgeted_format(format_args!("workspace module dependency cycle: {witness}")),
            )]);
        }
        if let Some(depth) = depths.get(module) {
            return Ok(*depth);
        }
        stack.push(module);
        let mut depth = 1usize;
        for dependency in dependencies.get(module).into_iter().flatten() {
            depth = depth.max(
                1usize
                    .checked_add(visit(dependency, dependencies, stack, depths)?)
                    .ok_or_else(|| vec![limit_error("dependency_depth", MAX_DEPENDENCY_DEPTH)])?,
            );
        }
        stack.pop();
        depths.insert(module, depth);
        Ok(depth)
    }
    let mut depths = BTreeMap::new();
    for module in dependencies.keys() {
        visit(module, dependencies, &mut Vec::new(), &mut depths)?;
    }
    Ok(depths)
}

fn canonical_cycle<'a>(cycle: &[&'a str]) -> Vec<&'a str> {
    let body = &cycle[..cycle.len().saturating_sub(1)];
    let start = body
        .iter()
        .enumerate()
        .min_by_key(|(_, module)| *module)
        .map_or(0, |(index, _)| index);
    let mut result = body[start..]
        .iter()
        .chain(&body[..start])
        .copied()
        .collect::<Vec<_>>();
    if let Some(first) = result.first().copied() {
        result.push(first);
    }
    result
}

pub(super) fn synthetic_program(
    program: &Program,
    authored: &BTreeMap<&str, AuthoredDeclaration<'_>>,
    programs: &[Program],
) -> Result<Program, Vec<Diagnostic>> {
    held_projection::synthetic_program_with_registry(program, authored, programs, None)
}
pub(super) fn rewrite_type(
    ty: &mut Type,
    target_module: &str,
    caller: &Program,
    programs: &[Program],
) -> Result<(), Vec<Diagnostic>> {
    type_rewrite::rewrite(ty, target_module, caller, programs)
}
pub(super) fn collect_expected_edges(
    program: &Program,
    programs: &[Program],
    module_paths: &BTreeMap<&str, &str>,
    authored: &BTreeMap<&str, AuthoredDeclaration<'_>>,
    edges: &mut Vec<WorkspaceEdge>,
) -> Result<(), Vec<Diagnostic>> {
    collect_expected_edges_impl::<false>(program, programs, module_paths, authored, edges)
}

fn collect_expected_edges_impl<const VISIT_EMPTY_CALL_IMPORTS: bool>(
    program: &Program,
    programs: &[Program],
    module_paths: &BTreeMap<&str, &str>,
    authored: &BTreeMap<&str, AuthoredDeclaration<'_>>,
    edges: &mut Vec<WorkspaceEdge>,
) -> Result<(), Vec<Diagnostic>> {
    let function_uses = program
        .module_uses
        .iter()
        .filter(|item| item.kind == ModuleUseKind::Function)
        .map(|item| (item.alias.as_str(), item))
        .collect::<BTreeMap<_, _>>();
    let type_uses = program
        .module_uses
        .iter()
        .filter(|item| item.kind == ModuleUseKind::Type)
        .map(|item| (item.alias.as_str(), item))
        .collect::<BTreeMap<_, _>>();
    for (index, permit) in program.permits.iter().enumerate() {
        let path = budgeted_format(format_args!("permit.{index}"));
        push_edge(
            edges,
            WorkspaceEdge {
                caller_path: crate::bounded_output::budgeted_clone(&program.path),
                caller: crate::bounded_output::budgeted_clone(&program.module),
                target_path: crate::bounded_output::budgeted_clone(&program.path),
                target: crate::bounded_output::budgeted_clone(permit),
                kind: "capability_authority",
                site: "module",
                expression: crate::bounded_output::budgeted_clone(&path),
                ast_path: path,
                alias: String::new(),
                ordinal: index,
            },
        )?;
    }
    // All remaining edges require an authored import; capability edges above
    // still participate in the complete independently checked projection.
    if function_uses.is_empty() && type_uses.is_empty() {
        edges.sort();
        return Ok(());
    }
    for declaration in &program.types {
        let declaration_type_uses = ScopedTypeUses {
            uses: &type_uses,
            shadowed: &declaration.type_parameters,
        };
        match &declaration.kind {
            TypeDeclarationKind::Resource { .. } => {}
            TypeDeclarationKind::Record { fields } => {
                for (index, field) in fields.iter().enumerate() {
                    collect_type_reference_edge(
                        program,
                        &declaration.stable_id,
                        &field.ty,
                        &budgeted_format(format_args!(
                            "type.{}.field.{index}",
                            declaration.stable_id
                        )),
                        declaration_type_uses,
                        module_paths,
                        authored,
                        edges,
                    )?;
                }
            }
            TypeDeclarationKind::Class { fields, methods } => {
                for (index, field) in fields.iter().enumerate() {
                    collect_type_reference_edge(
                        program,
                        &declaration.stable_id,
                        &field.ty,
                        &budgeted_format(format_args!(
                            "type.{}.field.{index}",
                            declaration.stable_id
                        )),
                        declaration_type_uses,
                        module_paths,
                        authored,
                        edges,
                    )?;
                }
                for method in methods {
                    for (param_index, param) in method.params.iter().enumerate() {
                        collect_type_reference_edge(
                            program,
                            &method.stable_id,
                            &param.ty,
                            &budgeted_format(format_args!(
                                "fn.{}.param.{param_index}",
                                method.stable_id
                            )),
                            declaration_type_uses,
                            module_paths,
                            authored,
                            edges,
                        )?;
                    }
                    collect_type_reference_edge(
                        program,
                        &method.stable_id,
                        &method.return_type,
                        &budgeted_format(format_args!("fn.{}.return", method.stable_id)),
                        declaration_type_uses,
                        module_paths,
                        authored,
                        edges,
                    )?;
                }
            }
            TypeDeclarationKind::Variant { cases } => {
                for (case_index, case) in cases.iter().enumerate() {
                    for (field_index, field) in case.fields.iter().enumerate() {
                        collect_type_reference_edge(
                            program,
                            &declaration.stable_id,
                            &field.ty,
                            &budgeted_format(format_args!(
                                "type.{}.case.{case_index}.field.{field_index}",
                                declaration.stable_id
                            )),
                            declaration_type_uses,
                            module_paths,
                            authored,
                            edges,
                        )?;
                    }
                }
            }
        }
    }
    for function in &program.functions {
        let function_type_uses = ScopedTypeUses {
            uses: &type_uses,
            shadowed: &function.type_parameters,
        };
        for (index, param) in function.params.iter().enumerate() {
            collect_type_reference_edge(
                program,
                &function.stable_id,
                &param.ty,
                &budgeted_format(format_args!(
                    "function.{}.param.{index}",
                    function.stable_id
                )),
                function_type_uses,
                module_paths,
                authored,
                edges,
            )?;
        }
        collect_type_reference_edge(
            program,
            &function.stable_id,
            &function.return_type,
            &budgeted_format(format_args!("function.{}.return", function.stable_id)),
            function_type_uses,
            module_paths,
            authored,
            edges,
        )?;
        for (site, expressions) in [
            ("requires", function.requires.as_slice()),
            ("body", std::slice::from_ref(&function.body)),
            ("ensures", function.ensures.as_slice()),
        ] {
            for (root_index, expression) in expressions.iter().enumerate() {
                let root = match site {
                    "requires" => budgeted_format(format_args!("requires.{root_index}")),
                    "body" => crate::bounded_output::budgeted_clone("body"),
                    "ensures" => budgeted_format(format_args!("ensures.{root_index}")),
                    _ => unreachable!(),
                };
                // No call edge is possible without an authored function import.
                // Type references and all later edge reconstruction still run.
                if VISIT_EMPTY_CALL_IMPORTS || !function_uses.is_empty() {
                    let mut call_ordinal = 0usize;
                    visit_ast_call_sites(expression, &root, &mut |name, path| {
                        let ordinal = call_ordinal;
                        call_ordinal += 1;
                        if let Some(module_use) = function_uses.get(name) {
                            let target = &authored[module_use.persistent_id.as_str()];
                            let edge = WorkspaceEdge {
                                caller_path: crate::bounded_output::budgeted_clone(&program.path),
                                caller: crate::bounded_output::budgeted_clone(&function.stable_id),
                                target_path: crate::bounded_output::budgeted_clone(
                                    module_paths[target.module],
                                ),
                                target: crate::bounded_output::budgeted_clone(
                                    &module_use.persistent_id,
                                ),
                                kind: "call",
                                site,
                                expression: hir::workspace_expression_identity(
                                    &hir::DeclarationId::new(
                                        crate::bounded_output::budgeted_clone(&function.stable_id),
                                    ),
                                    path,
                                ),
                                ast_path: crate::bounded_output::budgeted_clone(path),
                                alias: crate::bounded_output::budgeted_clone(&module_use.alias),
                                ordinal,
                            };
                            push_edge(edges, edge)?;
                            if let Some(target_function) = target.function {
                                for effect in &target_function.effects {
                                    push_edge(
                                        edges,
                                        WorkspaceEdge {
                                            caller_path: crate::bounded_output::budgeted_clone(
                                                &program.path,
                                            ),
                                            caller: crate::bounded_output::budgeted_clone(
                                                &function.stable_id,
                                            ),
                                            target_path: crate::bounded_output::budgeted_clone(
                                                target.path,
                                            ),
                                            target: crate::bounded_output::budgeted_clone(effect),
                                            kind: "effect_requirement",
                                            site,
                                            expression: hir::workspace_expression_identity(
                                                &hir::DeclarationId::new(
                                                    crate::bounded_output::budgeted_clone(
                                                        &function.stable_id,
                                                    ),
                                                ),
                                                path,
                                            ),
                                            ast_path: crate::bounded_output::budgeted_clone(path),
                                            alias: crate::bounded_output::budgeted_clone(
                                                &module_use.alias,
                                            ),
                                            ordinal,
                                        },
                                    )?;
                                }
                            }
                        }
                        Ok(())
                    })?;
                }
                collect_expression_type_edges(
                    program,
                    &function.stable_id,
                    expression,
                    &root,
                    function_type_uses,
                    module_paths,
                    authored,
                    edges,
                )?;
            }
        }
    }
    record_traversal::collect_edges(program, programs, module_paths, authored, edges)?;
    for (ordinal, module_use) in program.module_uses.iter().enumerate() {
        if module_use.kind == ModuleUseKind::Protocol {
            continue;
        }
        let target = &authored[module_use.persistent_id.as_str()];
        push_edge(
            edges,
            WorkspaceEdge {
                caller_path: crate::bounded_output::budgeted_clone(&program.path),
                caller: crate::bounded_output::budgeted_clone(&program.module),
                target_path: crate::bounded_output::budgeted_clone(module_paths[target.module]),
                target: crate::bounded_output::budgeted_clone(&module_use.persistent_id),
                kind: match module_use.kind {
                    ModuleUseKind::Function => "function_import",
                    ModuleUseKind::Type => "type_import",
                    ModuleUseKind::Protocol => unreachable!(),
                },
                site: "module",
                expression: budgeted_format(format_args!("use.{ordinal}")),
                ast_path: budgeted_format(format_args!("use.{ordinal}")),
                alias: crate::bounded_output::budgeted_clone(&module_use.alias),
                ordinal,
            },
        )?;
    }
    edges.sort();
    Ok(())
}

#[derive(Clone, Copy)]
struct ScopedTypeUses<'a> {
    uses: &'a BTreeMap<&'a str, &'a ModuleUse>,
    shadowed: &'a [crate::ast::TypeParameterDeclaration],
}

impl<'a> ScopedTypeUses<'a> {
    fn get(self, name: &str) -> Option<&'a ModuleUse> {
        if self.shadowed.iter().any(|parameter| parameter.name == name) {
            None
        } else {
            self.uses.get(name).copied()
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn collect_type_reference_edge(
    program: &Program,
    owner: &str,
    ty: &Type,
    path: &str,
    type_uses: ScopedTypeUses<'_>,
    module_paths: &BTreeMap<&str, &str>,
    authored: &BTreeMap<&str, AuthoredDeclaration<'_>>,
    edges: &mut Vec<WorkspaceEdge>,
) -> Result<(), Vec<Diagnostic>> {
    collect_type_reference_edge_at(
        program,
        owner,
        ty,
        path,
        None,
        type_uses,
        module_paths,
        authored,
        edges,
    )
}

#[allow(clippy::too_many_arguments)]
fn collect_type_reference_edge_at(
    program: &Program,
    owner: &str,
    ty: &Type,
    path: &str,
    expression: Option<&str>,
    type_uses: ScopedTypeUses<'_>,
    module_paths: &BTreeMap<&str, &str>,
    authored: &BTreeMap<&str, AuthoredDeclaration<'_>>,
    edges: &mut Vec<WorkspaceEdge>,
) -> Result<(), Vec<Diagnostic>> {
    let Type::Named { name, arguments } = ty else {
        return Ok(());
    };
    collect_named_type_reference_edge_at(
        program,
        owner,
        name,
        arguments,
        path,
        expression,
        type_uses,
        module_paths,
        authored,
        edges,
    )
}

#[allow(clippy::too_many_arguments)]
fn collect_named_type_reference_edge_at(
    program: &Program,
    owner: &str,
    name: &str,
    arguments: &[Type],
    path: &str,
    expression: Option<&str>,
    type_uses: ScopedTypeUses<'_>,
    module_paths: &BTreeMap<&str, &str>,
    authored: &BTreeMap<&str, AuthoredDeclaration<'_>>,
    edges: &mut Vec<WorkspaceEdge>,
) -> Result<(), Vec<Diagnostic>> {
    if let Some(module_use) = type_uses.get(name) {
        let target = &authored[module_use.persistent_id.as_str()];
        push_edge(
            edges,
            WorkspaceEdge {
                caller_path: crate::bounded_output::budgeted_clone(&program.path),
                caller: crate::bounded_output::budgeted_clone(owner),
                target_path: crate::bounded_output::budgeted_clone(module_paths[target.module]),
                target: crate::bounded_output::budgeted_clone(&module_use.persistent_id),
                kind: "type_reference",
                site: "type",
                expression: crate::bounded_output::budgeted_clone(expression.unwrap_or(path)),
                ast_path: crate::bounded_output::budgeted_clone(path),
                alias: crate::bounded_output::budgeted_clone(&module_use.alias),
                ordinal: edges.len(),
            },
        )?;
    }
    for (index, argument) in arguments.iter().enumerate() {
        let argument_path = budgeted_format(format_args!("{path}.argument.{index}"));
        collect_type_reference_edge_at(
            program,
            owner,
            argument,
            &argument_path,
            expression,
            type_uses,
            module_paths,
            authored,
            edges,
        )?;
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn collect_expression_type_edges(
    program: &Program,
    owner: &str,
    expression: &Expr,
    path: &str,
    type_uses: ScopedTypeUses<'_>,
    module_paths: &BTreeMap<&str, &str>,
    authored: &BTreeMap<&str, AuthoredDeclaration<'_>>,
    edges: &mut Vec<WorkspaceEdge>,
) -> Result<(), Vec<Diagnostic>> {
    if type_uses.uses.is_empty() {
        return Ok(());
    }
    // Scalar nodes only recurse. Derive their identity only if a type site
    // actually needs it, preserving its exact encoding and normal charge.
    let expression_id = std::cell::LazyCell::new(|| {
        hir::workspace_expression_identity(
            &hir::DeclarationId::new(crate::bounded_output::budgeted_clone(owner)),
            path,
        )
    });
    match &expression.kind {
        ExprKind::VecConstructor { .. } => {
            let expansion = crate::ast::vec_constructor::inspect(expression).map_err(|_| {
                vec![graph_error(
                    "SPX-G173",
                    "Vec constructor source provenance disagrees with retained HIR",
                )]
            })?;
            // The marker replaces its ordinary Block at the same identity.
            collect_expression_type_edges(
                program,
                owner,
                expansion.value,
                path,
                type_uses,
                module_paths,
                authored,
                edges,
            )?;
        }
        ExprKind::Closure {
            params,
            return_type,
            body,
            ..
        } => {
            for (index, parameter) in params.iter().enumerate() {
                collect_type_reference_edge_at(
                    program,
                    owner,
                    &parameter.ty,
                    &format!("{path}.closure.param.{index}"),
                    Some(&expression_id),
                    type_uses,
                    module_paths,
                    authored,
                    edges,
                )?;
            }
            collect_type_reference_edge_at(
                program,
                owner,
                return_type,
                &format!("{path}.closure.result"),
                Some(&expression_id),
                type_uses,
                module_paths,
                authored,
                edges,
            )?;
            collect_expression_type_edges(
                program,
                owner,
                body,
                &format!("{path}.closure.body"),
                type_uses,
                module_paths,
                authored,
                edges,
            )?;
        }
        ExprKind::Call {
            type_arguments,
            args,
            ..
        } => {
            for (index, argument) in type_arguments.iter().enumerate() {
                let type_path = budgeted_format(format_args!("{path}.type_argument.{index}"));
                collect_type_reference_edge_at(
                    program,
                    owner,
                    argument,
                    &type_path,
                    Some(&expression_id),
                    type_uses,
                    module_paths,
                    authored,
                    edges,
                )?;
            }
            for (index, argument) in args.iter().enumerate() {
                let child = budgeted_format(format_args!("{path}.arg.{index}"));
                collect_expression_type_edges(
                    program,
                    owner,
                    argument,
                    &child,
                    type_uses,
                    module_paths,
                    authored,
                    edges,
                )?;
            }
        }
        ExprKind::MethodCall {
            receiver,
            type_arguments,
            args,
            ..
        } => {
            for (index, argument) in type_arguments.iter().enumerate() {
                let type_path = budgeted_format(format_args!("{path}.type_argument.{index}"));
                collect_type_reference_edge_at(
                    program,
                    owner,
                    argument,
                    &type_path,
                    Some(&expression_id),
                    type_uses,
                    module_paths,
                    authored,
                    edges,
                )?;
            }
            collect_expression_type_edges(
                program,
                owner,
                receiver,
                &budgeted_format(format_args!("{path}.receiver")),
                type_uses,
                module_paths,
                authored,
                edges,
            )?;
            for (index, argument) in args.iter().enumerate() {
                let child = budgeted_format(format_args!("{path}.arg.{index}"));
                collect_expression_type_edges(
                    program,
                    owner,
                    argument,
                    &child,
                    type_uses,
                    module_paths,
                    authored,
                    edges,
                )?;
            }
        }
        ExprKind::SuperMethod { args, .. } => {
            for (index, argument) in args.iter().enumerate() {
                let child = budgeted_format(format_args!("{path}.arg.{index}"));
                collect_expression_type_edges(
                    program,
                    owner,
                    argument,
                    &child,
                    type_uses,
                    module_paths,
                    authored,
                    edges,
                )?;
            }
        }
        ExprKind::LexicalExit { value: None, .. } => {}
        ExprKind::LexicalExit {
            value: Some(value), ..
        }
        | ExprKind::Unary { value, .. } => collect_expression_type_edges(
            program,
            owner,
            value,
            &budgeted_format(format_args!("{path}.value")),
            type_uses,
            module_paths,
            authored,
            edges,
        )?,
        ExprKind::Binary { left, right, .. } => {
            collect_expression_type_edges(
                program,
                owner,
                left,
                &budgeted_format(format_args!("{path}.left")),
                type_uses,
                module_paths,
                authored,
                edges,
            )?;
            collect_expression_type_edges(
                program,
                owner,
                right,
                &budgeted_format(format_args!("{path}.right")),
                type_uses,
                module_paths,
                authored,
                edges,
            )?;
        }
        ExprKind::Block { statements, tail } => {
            for (index, statement) in statements.iter().enumerate() {
                let child_count = statement.child_count();
                for child_index in 0..child_count {
                    let Some(child) = statement.child(child_index) else {
                        continue;
                    };
                    // Bounded `for` traversal is lowered, so its authored
                    // children keep the paths `hir::resolve_for::lower` gives
                    // them rather than `.values`/`.body`; `visit_ast_call_sites` names the same.
                    let segment = statement_segment::child(statement, child_index);
                    collect_expression_type_edges(
                        program,
                        owner,
                        child,
                        &budgeted_format(format_args!("{path}.s{index}.{segment}")),
                        type_uses,
                        module_paths,
                        authored,
                        edges,
                    )?;
                }
            }
            collect_expression_type_edges(
                program,
                owner,
                tail,
                &budgeted_format(format_args!("{path}.tail")),
                type_uses,
                module_paths,
                authored,
                edges,
            )?;
        }
        ExprKind::If {
            condition,
            then_branch,
            else_branch,
        } => {
            for (suffix, child) in [
                ("condition", condition.as_ref()),
                ("then", then_branch.as_ref()),
                ("else", else_branch.as_ref()),
            ] {
                collect_expression_type_edges(
                    program,
                    owner,
                    child,
                    &budgeted_format(format_args!("{path}.{suffix}")),
                    type_uses,
                    module_paths,
                    authored,
                    edges,
                )?;
            }
        }
        ExprKind::ConstructRecord {
            type_name,
            type_arguments,
            fields,
            ..
        }
        | ExprKind::ConstructVariant {
            type_name,
            type_arguments,
            fields,
            ..
        } => {
            collect_named_type_reference_edge_at(
                program,
                owner,
                type_name,
                type_arguments,
                &budgeted_format(format_args!("{path}.type")),
                Some(&expression_id),
                type_uses,
                module_paths,
                authored,
                edges,
            )?;
            for (index, field) in fields.iter().enumerate() {
                collect_expression_type_edges(
                    program,
                    owner,
                    &field.value,
                    &budgeted_format(format_args!("{path}.field.{index}.value")),
                    type_uses,
                    module_paths,
                    authored,
                    edges,
                )?;
            }
        }
        ExprKind::Match {
            scrutinee, arms, ..
        } => {
            collect_expression_type_edges(
                program,
                owner,
                scrutinee,
                &budgeted_format(format_args!("{path}.scrutinee")),
                type_uses,
                module_paths,
                authored,
                edges,
            )?;
            for (index, arm) in arms.iter().enumerate() {
                let pattern_path = budgeted_format(format_args!("{path}.arm.{index}.pattern"));
                collect_match_pattern_type_edges(
                    program,
                    owner,
                    &arm.pattern,
                    &pattern_path,
                    &expression_id,
                    type_uses,
                    module_paths,
                    authored,
                    edges,
                )?;
                collect_expression_type_edges(
                    program,
                    owner,
                    &arm.value,
                    &budgeted_format(format_args!("{path}.arm.{index}.value")),
                    type_uses,
                    module_paths,
                    authored,
                    edges,
                )?;
            }
        }
        ExprKind::Try { operand } | ExprKind::Yield { request: operand } => {
            collect_expression_type_edges(
                program,
                owner,
                operand,
                &budgeted_format(format_args!("{path}.operand")),
                type_uses,
                module_paths,
                authored,
                edges,
            )?
        }
        ExprKind::UpdateRecord { base, fields } => {
            collect_expression_type_edges(
                program,
                owner,
                base,
                &budgeted_format(format_args!("{path}.base")),
                type_uses,
                module_paths,
                authored,
                edges,
            )?;
            for (index, field) in fields.iter().enumerate() {
                collect_expression_type_edges(
                    program,
                    owner,
                    &field.value,
                    &budgeted_format(format_args!("{path}.field.{index}.value")),
                    type_uses,
                    module_paths,
                    authored,
                    edges,
                )?;
            }
        }
        ExprKind::Project { base, .. } => collect_expression_type_edges(
            program,
            owner,
            base,
            &budgeted_format(format_args!("{path}.base")),
            type_uses,
            module_paths,
            authored,
            edges,
        )?,
        ExprKind::Int(_)
        | ExprKind::Int32(_)
        | ExprKind::Char(_)
        | ExprKind::Uint8(_)
        | ExprKind::Usize(_)
        | ExprKind::Float32(_)
        | ExprKind::Float64(_)
        | ExprKind::Bool(_)
        | ExprKind::String(_)
        | ExprKind::BorrowedUtf8Literal(_)
        | ExprKind::ArrayU8(_)
        | ExprKind::RepeatArrayU8 { .. }
        | ExprKind::Var(_) => {}
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn collect_match_pattern_type_edges(
    program: &Program,
    owner: &str,
    pattern: &crate::ast::MatchPattern,
    path: &str,
    expression: &str,
    type_uses: ScopedTypeUses<'_>,
    module_paths: &BTreeMap<&str, &str>,
    authored: &BTreeMap<&str, AuthoredDeclaration<'_>>,
    edges: &mut Vec<WorkspaceEdge>,
) -> Result<(), Vec<Diagnostic>> {
    match pattern {
        crate::ast::MatchPattern::Variant { type_name, .. }
        | crate::ast::MatchPattern::Record { type_name, .. } => {
            collect_named_type_reference_edge_at(
                program,
                owner,
                type_name,
                &[],
                path,
                Some(expression),
                type_uses,
                module_paths,
                authored,
                edges,
            )?;
        }
        crate::ast::MatchPattern::Wildcard { .. } => {}
        // Refutable Match v1: literal/binding patterns reference no named
        // types; or-patterns recurse into their literal alternatives.
        crate::ast::MatchPattern::Literal { .. } | crate::ast::MatchPattern::Binding { .. } => {}
        crate::ast::MatchPattern::Or { alternatives, .. } => {
            for (index, alternative) in alternatives.iter().enumerate() {
                collect_match_pattern_type_edges(
                    program,
                    owner,
                    alternative,
                    &budgeted_format(format_args!("{path}.alternative.{index}")),
                    expression,
                    type_uses,
                    module_paths,
                    authored,
                    edges,
                )?;
            }
        }
    }
    if let crate::ast::MatchPattern::Record { fields, .. } = pattern {
        for (index, field) in fields.iter().enumerate() {
            collect_record_pattern_type_edges(
                program,
                owner,
                &field.pattern,
                &budgeted_format(format_args!("{path}.field.{index}.pattern")),
                expression,
                type_uses,
                module_paths,
                authored,
                edges,
            )?;
        }
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn collect_record_pattern_type_edges(
    program: &Program,
    owner: &str,
    pattern: &crate::ast::RecordMatchFieldPattern,
    path: &str,
    expression: &str,
    type_uses: ScopedTypeUses<'_>,
    module_paths: &BTreeMap<&str, &str>,
    authored: &BTreeMap<&str, AuthoredDeclaration<'_>>,
    edges: &mut Vec<WorkspaceEdge>,
) -> Result<(), Vec<Diagnostic>> {
    let crate::ast::RecordMatchFieldPattern::Record {
        type_name, fields, ..
    } = pattern
    else {
        return Ok(());
    };
    collect_named_type_reference_edge_at(
        program,
        owner,
        type_name,
        &[],
        path,
        Some(expression),
        type_uses,
        module_paths,
        authored,
        edges,
    )?;
    for (index, field) in fields.iter().enumerate() {
        collect_record_pattern_type_edges(
            program,
            owner,
            &field.pattern,
            &budgeted_format(format_args!("{path}.field.{index}.pattern")),
            expression,
            type_uses,
            module_paths,
            authored,
            edges,
        )?;
    }
    Ok(())
}

pub(super) fn verify_resolved_call_edges(
    program: &Program,
    resolved: &hir::ResolvedProgram,
    authored: &BTreeMap<&str, AuthoredDeclaration<'_>>,
) -> Result<(), Vec<Diagnostic>> {
    sort_keys::verify(program, resolved, authored)?;
    field_unique::verify(program, resolved, authored)?;
    string_slots::verify(program, resolved)?;
    vec_constructor::verify(program, resolved)?;
    let aliases = program
        .module_uses
        .iter()
        .filter(|item| item.kind == ModuleUseKind::Function)
        .map(|item| (item.alias.as_str(), item.persistent_id.as_str()))
        .collect::<BTreeMap<_, _>>();
    if aliases.is_empty() {
        return Ok(());
    }
    let mut expected = Vec::new();
    for function in &program.functions {
        let owner =
            hir::DeclarationId::new(crate::bounded_output::budgeted_clone(&function.stable_id));
        for (root, expression) in function
            .requires
            .iter()
            .enumerate()
            .map(|(index, expression)| {
                (
                    budgeted_format(format_args!("requires.{index}")),
                    expression,
                )
            })
            .chain(std::iter::once((
                crate::bounded_output::budgeted_clone("body"),
                &function.body,
            )))
            .chain(
                function
                    .ensures
                    .iter()
                    .enumerate()
                    .map(|(index, expression)| {
                        (budgeted_format(format_args!("ensures.{index}")), expression)
                    }),
            )
        {
            visit_ast_call_sites(expression, &root, &mut |name, path| {
                if let Some(target) = aliases.get(name) {
                    reserve_builder_structure(std::mem::size_of::<(&str, String, &str)>())?;
                    expected.push((
                        function.stable_id.as_str(),
                        hir::workspace_expression_identity(&owner, path),
                        *target,
                    ));
                }
                Ok(())
            })?;
        }
    }
    let target_ids = program
        .module_uses
        .iter()
        .filter(|item| item.kind == ModuleUseKind::Function)
        .map(|item| item.persistent_id.as_str())
        .collect::<BTreeSet<_>>();
    let mut actual = call_sites::imported_call_sites(resolved, &target_ids)?;
    expected.sort();
    actual.sort();
    if expected
        .iter()
        .map(|(owner, expression, target)| (*owner, expression.as_str(), *target))
        .ne(actual
            .iter()
            .map(|(owner, expression, target)| (owner.as_str(), *expression, *target)))
        || expected
            .iter()
            .any(|(_, _, target)| !authored.contains_key(target))
    {
        return Err(vec![graph_error(
            "SPX-G173",
            "independent workspace call-edge reconstruction disagrees with HIR",
        )]);
    }
    Ok(())
}

#[cfg(test)]
#[path = "expected_projection/identity_prebound_tests.rs"]
mod identity_prebound_tests;

#[cfg(test)]
#[path = "expected_projection/call_absence_tests.rs"]
mod call_absence_tests;
