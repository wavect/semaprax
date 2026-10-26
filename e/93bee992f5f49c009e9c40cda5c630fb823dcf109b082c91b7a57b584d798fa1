//! Saved complete owned projection producer, before imported borrowing.
use super::{
    active_builder_limit, budgeted_format, graph_error, import_stub, limit_error, private_layout,
    reserve_builder_structure, type_rewrite, AuthoredDeclaration, BTreeMap, Diagnostic, Expr,
    ExprKind, Function, ModuleUseKind, Program, Type, TypeDeclaration,
};
use crate::ast::Span;
use crate::workspace_graph::expected_projection::import_stub::default_expr_original as default_expr;

pub(super) fn construct(
    program: &Program,
    authored: &BTreeMap<&str, AuthoredDeclaration<'_>>,
    programs: &[Program],
    registry: Option<&private_layout::registry::LayoutRegistry<'_>>,
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
    let mut byte_proofs = import_stub::ByteHelperProofs::new(authored, programs);
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
        crate::workspace_graph::generic_type_import::rewrite_declaration_with_layout(
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
                crate::vec_ops::source_wrapper_original(provider, target_function).is_some()
                    || crate::box_ops::source_wrapper_original(provider, target_function).is_some()
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
            crate::workspace_graph::owned_function_import::admitted_with_layout(
                program, target, authored, programs, layout,
            ),
            import_stub::byte_helper_authorized(target_function, target.module, &mut byte_proofs)?,
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
    Ok(synthetic)
}
