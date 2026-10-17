//! Exact-source imported overlays, including ordinary classified retained
//! wrappers and invariant/class roots. Views confer no import authority.
#[path = "borrowed_imports/alias_clone.rs"]
#[allow(
    clippy::duplicate_mod,
    reason = "Run the same controls against independent receivers."
)]
mod alias_clone;
#[path = "borrowed_imports/receiving.rs"]
#[allow(
    clippy::duplicate_mod,
    reason = "Run the same controls against independent receivers."
)]
mod receiving;
use super::{
    active_builder_limit, ast_function_cost, ast_function_signature_cost,
    ast_type_declaration_cost, budgeted_format, default_expr, graph_error, import_stub,
    limit_error, reserve_builder_structure, type_rewrite, AuthoredDeclaration, LayoutRegistry,
    StructuralCost,
};
use crate::ast::program_source::{generated_roots::GeneratedRoots, SyntheticProgramView};
use crate::ast::{
    Expr, ExprKind, Function, ModuleUseKind, Program, Type, TypeDeclaration, TypeDeclarationKind,
};
use crate::diagnostic::Diagnostic;
pub(in crate::workspace_graph) use receiving::construct_receiving;
use std::collections::BTreeMap;

pub(in crate::workspace_graph) fn eligible(
    program: &Program,
    authored: &BTreeMap<&str, AuthoredDeclaration<'_>>,
    _programs: &[Program],
) -> bool {
    !program.module_uses.is_empty()
        && program
            .module_uses
            .iter()
            .all(|import| authored.contains_key(import.persistent_id.as_str()))
}
fn refusal() -> Vec<Diagnostic> {
    vec![limit_error("builder_bytes", active_builder_limit())]
}
fn directory<T>(count: usize) -> Result<Vec<T>, Vec<Diagnostic>> {
    let bytes = count
        .checked_mul(std::mem::size_of::<T>())
        .ok_or_else(refusal)?;
    reserve_builder_structure(bytes)?;
    let values = Vec::with_capacity(count);
    if values.capacity() != count {
        return Err(refusal());
    }
    Ok(values)
}
fn pay_type(declaration: &TypeDeclaration) -> Result<(), Vec<Diagnostic>> {
    let mut cost = StructuralCost::raw_ast(true).with_inline_values(true);
    ast_type_declaration_cost(declaration, &mut cost)?;
    reserve_builder_structure(cost.total)
}
fn pay_signature(function: &Function) -> Result<(), Vec<Diagnostic>> {
    let mut cost = StructuralCost::raw_ast(true).with_inline_values(true);
    ast_function_signature_cost(function, &mut cost)?;
    reserve_builder_structure(cost.total)
}

// A geometric Vec worklist's cumulative requests are below four times its
// element census; its initial nonbyte capacity is four. Raw inline AST cost
// counts every node/header without a clone discount. Charge the full fourfold
// census before cloning each recursive root, plus each worklist's minimum once.
fn pay_teardown(cost: &StructuralCost) -> Result<(), Vec<Diagnostic>> {
    reserve_builder_structure(cost.total.checked_mul(4).ok_or_else(refusal)?)
}
fn iterative(program: &Program, authored: &BTreeMap<&str, AuthoredDeclaration<'_>>) -> bool {
    program.module_uses.iter().any(|import| {
        let target = &authored[import.persistent_id.as_str()];
        target
            .function
            .is_some_and(|function| !function.type_parameters.is_empty())
            || target.ty.is_some_and(|ty| {
                !ty.invariants().is_empty() || matches!(ty.kind, TypeDeclarationKind::Class { .. })
            })
    })
}
fn pay_teardown_minimum() -> Result<(), Vec<Diagnostic>> {
    let bytes = [
        std::mem::size_of::<Function>(),
        std::mem::size_of::<Expr>(),
        std::mem::size_of::<Type>(),
        std::mem::size_of::<crate::ast::MatchPattern>(),
        std::mem::size_of::<crate::ast::RecordMatchPatternField>(),
    ]
    .into_iter()
    .try_fold(0usize, |sum, size| sum.checked_add(size))
    .ok_or_else(refusal)?;
    reserve_builder_structure(bytes.checked_mul(4).ok_or_else(refusal)?)
}

// Defaults can expand a shared record type repeatedly. Count that expanded
// expression tree through borrowed declarations, rather than charging only the
// unique type directory. This is storage payment, not default admission; the
// original constructor still chooses forwarding, classifiers and first errors.
fn pay_default_teardown(
    ty: &Type,
    declarations: &[(&str, &TypeDeclaration)],
) -> Result<(), Vec<Diagnostic>> {
    fn census(
        ty: &Type,
        declarations: &[(&str, &TypeDeclaration)],
    ) -> Result<usize, Vec<Diagnostic>> {
        let mut bytes = std::mem::size_of::<Expr>();
        if let Type::Named { name, arguments } = ty {
            if arguments.is_empty() {
                if let Ok(index) =
                    declarations.binary_search_by_key(&name.as_str(), |(name, _)| *name)
                {
                    let fields: &[crate::ast::FieldDeclaration] = match &declarations[index].1.kind
                    {
                        TypeDeclarationKind::Record { fields }
                        | TypeDeclarationKind::Class { fields, .. } => fields,
                        TypeDeclarationKind::Variant { cases } => {
                            cases.first().map_or(&[], |case| &case.fields)
                        }
                        TypeDeclarationKind::Resource { .. } => &[],
                    };
                    for field in fields {
                        bytes = bytes
                            .checked_add(census(&field.ty, declarations)?)
                            .ok_or_else(refusal)?;
                    }
                }
            } else {
                // Accepted compiler collection defaults have a root call,
                // one scalar size operand and copied type arguments. Generic
                // authored defaults still receive their old closed refusal.
                bytes = bytes
                    .checked_add(std::mem::size_of::<Expr>())
                    .ok_or_else(refusal)?;
                let mut cost = StructuralCost::raw_ast(true);
                super::declaration_cost::ast_type_cost(ty, &mut cost)?;
                bytes = bytes.checked_add(cost.total).ok_or_else(refusal)?;
            }
        } else if *ty == Type::Bytes || *ty == Type::StringMap {
            bytes = bytes
                .checked_add(std::mem::size_of::<Expr>())
                .ok_or_else(refusal)?;
        }
        Ok(bytes)
    }
    reserve_builder_structure(
        census(ty, declarations)?
            .checked_mul(4)
            .ok_or_else(refusal)?,
    )
}

pub(in crate::workspace_graph) fn construct<'a>(
    program: &'a Program,
    authored: &BTreeMap<&str, AuthoredDeclaration<'_>>,
    programs: &[Program],
    registry: &LayoutRegistry<'_>,
) -> Result<SyntheticProgramView<'a>, Vec<Diagnostic>> {
    construct_using(program, authored, programs, registry, None)
}

pub(in crate::workspace_graph) fn construct_core<'a>(
    program: &'a Program,
    authored: &'a BTreeMap<&'a str, AuthoredDeclaration<'a>>,
    programs: &'a [Program],
    registry: &LayoutRegistry<'_>,
    core: &mut import_stub::core_proofs::CoreProviderProofs<'a>,
) -> Result<SyntheticProgramView<'a>, Vec<Diagnostic>> {
    construct_using(program, authored, programs, registry, Some(core))
}

pub(in crate::workspace_graph) fn construct_using<'p, 'a>(
    program: &'p Program,
    authored: &'a BTreeMap<&'a str, AuthoredDeclaration<'a>>,
    programs: &'a [Program],
    registry: &LayoutRegistry<'_>,
    core: Option<&mut import_stub::core_proofs::CoreProviderProofs<'a>>,
) -> Result<SyntheticProgramView<'p>, Vec<Diagnostic>> {
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
        reserve_builder_structure(0)?;
        super::super::generic_type_import::rewrite_declaration_with_layout(
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
        reserve_builder_structure(0)?;
        super::super::generic_type_import::rewrite_declaration_with_layout(
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
    let mut byte_proofs = import_stub::core_proofs::Phase::new(authored, programs, core)
        .map_err(|error| vec![error])?;
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
                crate::vec_ops::source_wrapper(provider, original).is_some()
                    || crate::box_ops::source_wrapper(provider, original).is_some()
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
        reserve_builder_structure(0)?;
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
            super::super::owned_function_import::admitted_with_layout(
                program, target, authored, programs, &layout,
            ),
            import_stub::byte_helper_authorized(
                original,
                target.module,
                byte_proofs.get().map_err(|error| vec![error])?,
            )?,
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
        reserve_builder_structure(0)?;
    }
    drop(type_index);
    // Infallible bounded string helpers record refusal and return empty text.
    // A generated view must never publish that partial construction as success.
    reserve_builder_structure(0)?;
    let result = SyntheticProgramView::with_generated_owner(program, roots).ok_or_else(refusal)?;
    byte_proofs.complete().map_err(|error| vec![error])?;
    Ok(result)
}

#[cfg(test)]
#[path = "borrowed_imports/tests.rs"]
#[allow(
    clippy::duplicate_mod,
    reason = "Run the same controls against independent receivers."
)]
mod tests;

#[cfg(test)]
#[path = "borrowed_imports/inline.rs"]
pub(in crate::workspace_graph) mod inline;

#[path = "borrowed_imports/typed.rs"]
pub(in crate::workspace_graph) mod typed;

// Test-only access to complete prior/current helpers from the affine receiver
// tests. Production module visibility and the complete old bodies stay exact.
#[cfg(test)]
pub(in crate::workspace_graph) use super::defaults::{
    inline as constructor_test_inline_defaults, typed as constructor_test_typed_defaults,
};
#[cfg(test)]
pub(in crate::workspace_graph) use super::import_stub::typed as constructor_test_typed_import_stub;
#[cfg(test)]
pub(in crate::workspace_graph) fn constructor_test_default_expr_inline(
    function: &crate::ast::Function,
    declarations: &[(&str, &crate::ast::TypeDeclaration)],
    allow_owned: bool,
    allow_byte_forward: bool,
) -> Result<crate::ast::Expr, crate::workspace_graph::constructor_failure::Failure> {
    super::import_stub::default_expr_inline(
        function,
        declarations,
        allow_owned,
        allow_byte_forward,
    )
}
