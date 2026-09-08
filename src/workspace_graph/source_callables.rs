//! Replay omitted callable templates through the existing checked source path.
use super::expected_projection::synthetic_builder_bytes;
use super::*;

pub(crate) fn checked_source_callable_closures(
    sources: &[crate::project::ProjectSource],
    retained_templates: &BTreeSet<String>,
    defining_revision: &str,
) -> Result<Vec<serde_json::Value>, Vec<Diagnostic>> {
    if !sources.iter().any(|source| {
        matches!(
            source.source_graph_schema(),
            "semaprax.graph.v36"
                | "semaprax.graph.v37"
                | "semaprax.graph.v38"
                | "semaprax.graph.v39"
                | "semaprax.graph.v40"
                | "semaprax.graph.v41"
                | "semaprax.graph.v42"
        )
    }) {
        return Ok(Vec::new());
    }
    let mut programs = Vec::with_capacity(sources.len());
    let mut candidates = BTreeSet::new();
    for source in sources {
        let parsed = crate::parse(source.source(), source.path()).map_err(|e| vec![e])?;
        if matches!(
            source.source_graph_schema(),
            "semaprax.graph.v36"
                | "semaprax.graph.v37"
                | "semaprax.graph.v38"
                | "semaprax.graph.v39"
                | "semaprax.graph.v40"
                | "semaprax.graph.v41"
                | "semaprax.graph.v42"
        ) && parsed
            .functions
            .iter()
            .any(|f| !f.type_parameters.is_empty() && !retained_templates.contains(&f.stable_id))
        {
            candidates.insert(source.path().to_owned());
        }
        // Canonical reparse removes comments and their span shifts from checked
        // semantic facts. Exact original bytes remain in SourceProjection.
        let normalized = crate::format::canonical(&parsed);
        programs.push(crate::parse(&normalized, source.path()).map_err(|e| vec![e])?);
    }
    if candidates.is_empty() {
        return Ok(Vec::new());
    }
    let authored = index_authored(&programs)?;
    let mut closures = Vec::new();
    let mut output_bytes = 0usize;
    for program in &programs {
        if !candidates.contains(&program.path) {
            continue;
        }
        let prebound = synthetic_builder_bytes(program, &authored, &programs)?;
        let (checked, overflowed) = crate::bounded_output::with_limit(MAX_BUILDER_BYTES, || {
            charge_builder_prebound(prebound.raw_clone_and_hir)?;
            let synthetic = synthetic_program(program, &authored, &programs)?;
            let resolved = crate::vec_ops::with_authenticated_linked_source(|| {
                crate::box_ops::with_authenticated_linked_source(|| hir::resolve(&synthetic))
            })?;
            verify_resolved_call_edges(program, &resolved, &authored)?;
            let omitted = resolved
                .function_templates
                .iter()
                .filter(|template| {
                    !retained_templates.contains(template.id.as_str())
                        && program
                            .functions
                            .iter()
                            .any(|authored| authored.stable_id == template.id.as_str())
                        && hir::function_value::template_uses_value(template)
                })
                .map(|template| template.id.as_str().to_owned())
                .collect::<Vec<_>>();
            if omitted.is_empty() {
                return Ok::<Option<(Vec<String>, String)>, Vec<Diagnostic>>(None);
            }
            let graph = graph::to_hir_json(&resolved, defining_revision).map_err(|e| vec![e])?;
            Ok(Some((omitted, graph)))
        });
        if overflowed {
            return Err(vec![limit_error("builder_bytes", MAX_BUILDER_BYTES)]);
        }
        let Some((mut omitted, graph)) = checked? else {
            continue;
        };
        omitted.sort();
        output_bytes = checked_usage(output_bytes, graph.len(), "output_bytes", MAX_OUTPUT_BYTES)?;
        closures.push(serde_json::json!({
            "path": program.path,
            "defining_revision_kind": "normalized_source_workspace",
            "defining_revision": defining_revision,
            "omitted_callable_templates": omitted,
            "graph": graph,
        }));
    }
    Ok(closures)
}

pub(super) fn visit_ast_call_sites(
    expression: &Expr,
    path: &str,
    visit: &mut impl FnMut(&str, &str) -> Result<(), Vec<Diagnostic>>,
) -> Result<(), Vec<Diagnostic>> {
    match &expression.kind {
        ExprKind::Closure { body, .. } => {
            visit_ast_call_sites(body, &format!("{path}.closure.body"), visit)?
        }
        ExprKind::Call { name, args, .. } => {
            visit(name, path)?;
            for (index, argument) in args.iter().enumerate() {
                visit_ast_call_sites(
                    argument,
                    &crate::bounded_output::budgeted_format(format_args!("{path}.arg.{index}")),
                    visit,
                )?;
            }
        }
        ExprKind::ArrayU8(_) | ExprKind::RepeatArrayU8 { .. } => {}
        ExprKind::Unary { value, .. } => {
            visit_ast_call_sites(
                value,
                &crate::bounded_output::budgeted_format(format_args!("{path}.value")),
                visit,
            )?;
        }
        ExprKind::Binary { left, right, .. } => {
            visit_ast_call_sites(
                left,
                &crate::bounded_output::budgeted_format(format_args!("{path}.left")),
                visit,
            )?;
            visit_ast_call_sites(
                right,
                &crate::bounded_output::budgeted_format(format_args!("{path}.right")),
                visit,
            )?;
        }
        ExprKind::Block { statements, tail } => {
            for (index, statement) in statements.iter().enumerate() {
                match statement {
                    crate::ast::Statement::Let { value, .. }
                    | crate::ast::Statement::Assign { value, .. } => visit_ast_call_sites(
                        value,
                        &crate::bounded_output::budgeted_format(format_args!(
                            "{path}.s{index}.value"
                        )),
                        visit,
                    )?,
                    crate::ast::Statement::Unsafe { body, .. } => visit_ast_call_sites(
                        body,
                        &crate::bounded_output::budgeted_format(format_args!(
                            "{path}.s{index}.value"
                        )),
                        visit,
                    )?,
                    crate::ast::Statement::While {
                        condition, body, ..
                    } => {
                        visit_ast_call_sites(
                            condition,
                            &crate::bounded_output::budgeted_format(format_args!(
                                "{path}.s{index}.condition"
                            )),
                            visit,
                        )?;
                        visit_ast_call_sites(
                            body,
                            &crate::bounded_output::budgeted_format(format_args!(
                                "{path}.s{index}.body"
                            )),
                            visit,
                        )?;
                    }
                    crate::ast::Statement::For { values, body, .. } => {
                        // Bounded `for` traversal is lowered, so its authored
                        // children do not sit at `.values` and `.body`.
                        // `hir::resolve_for::lower` desugars the statement at
                        // `s{index}` into `s{index}.value`: `.s0` binds the
                        // length, `.s1` the index, `.s2` is the `while`, and
                        // the authored body is resolved at
                        // `.value.s2.body.s1.value` behind the item binding at
                        // `.value.s2.body.s0`. The source is admitted only as
                        // an immutable binding (`SPX-T284`), so it reaches the
                        // lowering as the place argument of the `vec_len`
                        // call. Naming the authored paths is what keeps this
                        // reconstruction independent of the HIR while still
                        // describing the same program.
                        visit_ast_call_sites(
                            values,
                            &crate::bounded_output::budgeted_format(format_args!(
                                "{path}.s{index}.value.s0.value.arg.0"
                            )),
                            visit,
                        )?;
                        visit_ast_call_sites(
                            body,
                            &crate::bounded_output::budgeted_format(format_args!(
                                "{path}.s{index}.value.s2.body.s1.value"
                            )),
                            visit,
                        )?;
                    }
                    crate::ast::Statement::ForOwn { values, body, .. } => {
                        // Consuming iterator traversal has a distinct HIR
                        // lowering. Its iterator source is staged at `.s0`,
                        // and the yielded-item binding is at
                        // `.s1.body.s0.value.arm.1.binding.0`; its authored
                        // body follows that binding at `.arm.1.value.s0.value`.
                        visit_ast_call_sites(
                            values,
                            &crate::bounded_output::budgeted_format(format_args!(
                                "{path}.s{index}.value.s0.value.arg.0"
                            )),
                            visit,
                        )?;
                        visit_ast_call_sites(
                            body,
                            &crate::bounded_output::budgeted_format(format_args!(
                                "{path}.s{index}.value.s1.body.s0.value.arm.1.value.s0.value"
                            )),
                            visit,
                        )?;
                    }
                }
            }
            visit_ast_call_sites(
                tail,
                &crate::bounded_output::budgeted_format(format_args!("{path}.tail")),
                visit,
            )?;
        }
        ExprKind::If {
            condition,
            then_branch,
            else_branch,
        } => {
            visit_ast_call_sites(
                condition,
                &crate::bounded_output::budgeted_format(format_args!("{path}.condition")),
                visit,
            )?;
            visit_ast_call_sites(
                then_branch,
                &crate::bounded_output::budgeted_format(format_args!("{path}.then")),
                visit,
            )?;
            visit_ast_call_sites(
                else_branch,
                &crate::bounded_output::budgeted_format(format_args!("{path}.else")),
                visit,
            )?;
        }
        ExprKind::ConstructRecord { fields, .. } | ExprKind::ConstructVariant { fields, .. } => {
            for (index, field) in fields.iter().enumerate() {
                visit_ast_call_sites(
                    &field.value,
                    &crate::bounded_output::budgeted_format(format_args!(
                        "{path}.field.{index}.value"
                    )),
                    visit,
                )?;
            }
        }
        ExprKind::Match {
            scrutinee, arms, ..
        } => {
            visit_ast_call_sites(
                scrutinee,
                &crate::bounded_output::budgeted_format(format_args!("{path}.scrutinee")),
                visit,
            )?;
            for (index, arm) in arms.iter().enumerate() {
                if let Some(guard) = &arm.guard {
                    visit_ast_call_sites(
                        guard.as_ref(),
                        &crate::bounded_output::budgeted_format(format_args!(
                            "{path}.arm.{index}.guard"
                        )),
                        visit,
                    )?;
                }
                visit_ast_call_sites(
                    &arm.value,
                    &crate::bounded_output::budgeted_format(format_args!(
                        "{path}.arm.{index}.value"
                    )),
                    visit,
                )?;
            }
        }
        ExprKind::Try { operand } => {
            visit_ast_call_sites(
                operand,
                &crate::bounded_output::budgeted_format(format_args!("{path}.operand")),
                visit,
            )?;
        }
        ExprKind::UpdateRecord { base, fields } => {
            visit_ast_call_sites(
                base,
                &crate::bounded_output::budgeted_format(format_args!("{path}.base")),
                visit,
            )?;
            for (index, field) in fields.iter().enumerate() {
                visit_ast_call_sites(
                    &field.value,
                    &crate::bounded_output::budgeted_format(format_args!(
                        "{path}.field.{index}.value"
                    )),
                    visit,
                )?;
            }
        }
        ExprKind::Project { base, .. } => {
            visit_ast_call_sites(
                base,
                &crate::bounded_output::budgeted_format(format_args!("{path}.base")),
                visit,
            )?;
        }
        ExprKind::Int(_)
        | ExprKind::Int32(_)
        | ExprKind::Char(_)
        | ExprKind::Uint8(_)
        | ExprKind::Usize(_)
        | ExprKind::Float32(_)
        | ExprKind::Float64(_)
        | ExprKind::Bool(_)
        | ExprKind::String(_)
        | ExprKind::Var(_) => {}
        // Method calls resolve to hoisted functions in HIR; the AST-level
        // call-site walk sees them through the resolved Call edge instead.
        ExprKind::MethodCall { .. } => {}
        // `super.method(...)` also resolves to a hoisted parent method in HIR.
        ExprKind::SuperMethod { .. } => {}
    }
    Ok(())
}
