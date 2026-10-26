use super::super::super::super::{
    alias_clone, ast_function_cost, ast_function_signature_cost, ast_type_declaration_cost,
    budgeted_format, directory, eligible, graph_error, iterative, pay_default_teardown,
    pay_signature, pay_teardown, pay_teardown_minimum, pay_type, refusal,
    reserve_builder_structure, type_rewrite, AuthoredDeclaration, BTreeMap, Diagnostic, Expr,
    ExprKind, Function, GeneratedRoots, LayoutRegistry, ModuleUseKind, Program, StructuralCost,
    SyntheticProgramView, Type, TypeDeclaration,
};
use crate::workspace_graph::expected_projection::import_stub::default_expr_original as default_expr;
use crate::workspace_graph::expected_projection::import_stub_original as import_stub;

pub(in crate::workspace_graph) fn construct<'a>(
    program: &'a Program,
    authored: &BTreeMap<&str, AuthoredDeclaration<'_>>,
    programs: &[Program],
    registry: &LayoutRegistry<'_>,
) -> Result<SyntheticProgramView<'a>, Vec<Diagnostic>> {
    reserve_builder_structure(0)?;
    if !eligible(program, authored, programs) {
        return Err(vec![graph_error(
            "SPX-G173",
            "imported borrowed projection requires the owned route",
        )]);
    }
    let layout = registry.get(program, authored, programs)?;
    let explicit = program
        .module_uses
        .iter()
        .filter(|import| import.kind == ModuleUseKind::Type)
        .count();
    let type_count = explicit
        .checked_add(layout.entries.len())
        .ok_or_else(refusal)?;
    let recursive = iterative(program, authored);
    if recursive {
        pay_teardown_minimum()?;
    }
    let mut roots = if recursive {
        GeneratedRoots::iterative(directory::<TypeDeclaration>(type_count)?, Vec::new())
    } else {
        GeneratedRoots::plain(directory::<TypeDeclaration>(type_count)?, Vec::new())
    };
    let types = &mut roots.types;
    for import in program
        .module_uses
        .iter()
        .filter(|import| import.kind == ModuleUseKind::Type)
    {
        let target = &authored[import.persistent_id.as_str()];
        let declaration = target.ty.expect("validated type target");
        pay_type(declaration)?;
        if recursive {
            let mut cost = StructuralCost::raw_ast(true).with_inline_values(true);
            ast_type_declaration_cost(declaration, &mut cost)?;
            pay_teardown(&cost)?;
        }
        types.push(alias_clone::type_payload(declaration));
        let copy = types.last_mut().expect("inserted type");
        copy.name = crate::bounded_output::budgeted_clone(&import.alias);
        crate::workspace_graph::generic_type_import::rewrite_declaration_with_layout(
            copy,
            target.module,
            program,
            programs,
            Some(&layout),
        )?;
    }
    // Preserve the independent closure's exact order after explicit aliases.
    for entry in &layout.entries {
        pay_type(entry.declaration)?;
        if recursive {
            let mut cost = StructuralCost::raw_ast(true).with_inline_values(true);
            ast_type_declaration_cost(entry.declaration, &mut cost)?;
            pay_teardown(&cost)?;
        }
        types.push(alias_clone::type_payload(entry.declaration));
        let copy = types.last_mut().expect("inserted private type");
        copy.name = crate::bounded_output::budgeted_clone(&entry.alias);
        crate::workspace_graph::generic_type_import::rewrite_declaration_with_layout(
            copy,
            entry.module,
            program,
            programs,
            Some(&layout),
        )?;
    }
    let all_count = program
        .types
        .len()
        .checked_add(types.len())
        .ok_or_else(refusal)?;
    let mut type_index = directory::<(&str, &TypeDeclaration)>(all_count)?;
    type_index.extend(
        program
            .types
            .iter()
            .chain(types.iter())
            .map(|declaration| (declaration.name.as_str(), declaration)),
    );
    type_index.sort_unstable_by(|a, b| a.0.cmp(b.0));
    if type_index.windows(2).any(|items| items[0].0 == items[1].0) {
        return Err(vec![graph_error(
            "SPX-G173",
            "synthetic workspace type-name index is not unique",
        )]);
    }
    let needs_main = !program
        .functions
        .iter()
        .any(|function| function.name == "main")
        && !program
            .module_uses
            .iter()
            .any(|import| import.kind == ModuleUseKind::Function && import.alias == "main");
    let imports = program
        .module_uses
        .iter()
        .filter(|import| import.kind == ModuleUseKind::Function)
        .count();
    roots.functions = directory::<Function>(
        imports
            .checked_add(usize::from(needs_main))
            .ok_or_else(refusal)?,
    )?;
    let functions = &mut roots.functions;
    let mut byte_proofs = import_stub::ByteHelperProofs::new(authored, programs);
    for import in program
        .module_uses
        .iter()
        .filter(|import| import.kind == ModuleUseKind::Function)
    {
        let target = &authored[import.persistent_id.as_str()];
        let original = target.function.expect("validated function target");
        let retain_body = programs
            .iter()
            .find(|provider| provider.module == target.module)
            .is_some_and(|provider| {
                crate::vec_ops::source_wrapper_original(provider, original).is_some()
                    || crate::box_ops::source_wrapper_original(provider, original).is_some()
            });
        if retain_body {
            let mut cost = StructuralCost::raw_ast(true).with_inline_values(true);
            ast_function_cost(original, &mut cost)?;
            reserve_builder_structure(cost.total)?;
            pay_teardown(&cost)?;
        } else {
            pay_signature(original)?;
            if recursive {
                let mut cost = StructuralCost::raw_ast(true).with_inline_values(true);
                ast_function_signature_cost(original, &mut cost)?;
                pay_teardown(&cost)?;
            }
        }
        let function = if retain_body {
            alias_clone::retained_function(original, &import.alias)
        } else {
            alias_clone::signature_function(original, &import.alias)
        };
        functions.push(function);
        if retain_body {
            continue;
        }
        let function = functions.last_mut().expect("inserted signature");
        for param in &mut function.params {
            type_rewrite::rewrite_with_layout(
                &mut param.ty,
                target.module,
                program,
                programs,
                Some(&layout),
            )?;
        }
        type_rewrite::rewrite_with_layout(
            &mut function.return_type,
            target.module,
            program,
            programs,
            Some(&layout),
        )?;
        if recursive {
            pay_default_teardown(&function.return_type, &type_index)?;
        }
        function.body = default_expr(
            function,
            &type_index,
            crate::workspace_graph::owned_function_import::admitted_with_layout(
                program, target, authored, programs, &layout,
            ),
            import_stub::byte_helper_authorized(original, target.module, &mut byte_proofs)?,
        )?;
    }
    // Same old rule also recognizes an imported alias named main.
    if needs_main && !functions.iter().any(|function| function.name == "main") {
        reserve_builder_structure(std::mem::size_of::<Expr>())?;
        functions.push(Function {
            stable_id: budgeted_format(format_args!("workspace.synthetic.main.{}", program.module)),
            explicit_id: true,
            name: crate::bounded_output::budgeted_clone("main"),
            name_span: crate::ast::Span::default(),
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
                span: crate::ast::Span::default(),
            },
            span: crate::ast::Span::default(),
        });
    }
    drop(type_index);
    SyntheticProgramView::with_generated_owner(program, roots).ok_or_else(refusal)
}
