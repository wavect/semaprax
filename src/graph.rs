//! Deterministic semantic graph serialization and bounded context queries.
//!
//! Human source supplies the revision. Resolved HIR supplies every semantic
//! identity and fact in graph v3; spans and display names are metadata only.

use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::fmt::Write;

use sha2::{Digest, Sha256};

use crate::ast::{BinaryOp, Program, UnaryOp};
use crate::diagnostic::{quote_json, Diagnostic};
use crate::format;
use crate::hir::{
    self, DeclarationId, IdentityOrigin, OwnershipMode, Place, PlaceProjection, ResolvedExpr,
    ResolvedExprKind, ResolvedFunction, ResolvedProgram, ResolvedStatement, ResolvedType,
    ResolvedTypeDeclarationKind, TypeFacts,
};

/// Hash the canonical human-readable source projection.
///
/// This revision intentionally does not depend on HIR spans, display metadata,
/// or the graph wire format. Semantic transactions therefore remain bound to
/// the exact canonical source meaning that a human can review in Git.
pub fn revision(program: &Program) -> String {
    let source = format::canonical(program);
    let mut hasher = Sha256::new();
    hasher.update(b"semaprax.graph-revision.v1\0");
    hasher.update(source.as_bytes());
    format!("sha256:{:x}", hasher.finalize())
}

/// Resolve and serialize a parsed program as `semaprax.graph.v3`.
///
/// Resolution is deliberately part of this public boundary. Invalid source
/// cannot be mistaken for a checked semantic graph by library callers.
pub fn to_json(program: &Program) -> Result<String, Vec<Diagnostic>> {
    let revision = revision(program);
    let resolved = hir::resolve(program)?;
    to_hir_json(&resolved, &revision).map_err(|diagnostic| vec![diagnostic])
}

/// Resolve and return a bounded call-dependency slice.
///
/// `symbol` may be either a function's display name or its persistent
/// declaration ID. `Ok(None)` means that no function matched the symbol;
/// resolution or graph validation failures are returned as diagnostics.
pub fn context_json(
    program: &Program,
    symbol: &str,
    depth: usize,
) -> Result<Option<String>, Vec<Diagnostic>> {
    let revision = revision(program);
    let resolved = hir::resolve(program)?;
    context_hir_json(&resolved, &revision, symbol, depth).map_err(|diagnostic| vec![diagnostic])
}

fn to_hir_json(program: &ResolvedProgram, source_revision: &str) -> Result<String, Diagnostic> {
    hir::validate(program)?;
    let selected_functions = program
        .functions
        .iter()
        .map(|function| function.id.clone())
        .collect();
    let selected_types = program
        .types
        .iter()
        .map(|declaration| declaration.id.clone())
        .collect();
    graph_json(
        program,
        source_revision,
        &selected_functions,
        &selected_types,
        &GraphView::Module,
    )
}

fn context_hir_json(
    program: &ResolvedProgram,
    source_revision: &str,
    symbol: &str,
    depth: usize,
) -> Result<Option<String>, Diagnostic> {
    hir::validate(program)?;

    // Exact declaration identity is authoritative if another function's
    // display name happens to contain the same text.
    let root = program
        .functions
        .iter()
        .find(|function| function.id.as_str() == symbol)
        .map(|function| function.id.clone())
        .or_else(|| program.declarations.function_id(symbol).cloned());
    let Some(root) = root else {
        return Ok(None);
    };

    let functions = program
        .functions
        .iter()
        .map(|function| (function.id.clone(), function))
        .collect::<BTreeMap<_, _>>();
    let mut selected = BTreeSet::from([root.clone()]);
    let mut queue = VecDeque::from([(root.clone(), 0_usize)]);
    while let Some((function_id, current_depth)) = queue.pop_front() {
        if current_depth >= depth {
            continue;
        }
        if let Some(function) = functions.get(&function_id) {
            visit_function_calls(function, &mut |callee| {
                if functions.contains_key(callee) && selected.insert(callee.clone()) {
                    queue.push_back((callee.clone(), current_depth + 1));
                }
            });
        }
    }

    let mut selected_types = BTreeSet::new();
    for function in &program.functions {
        if selected.contains(&function.id) {
            collect_function_type_declarations(function, &mut selected_types);
        }
    }

    let mut frontier = BTreeSet::new();
    for function in &program.functions {
        if !selected.contains(&function.id) {
            continue;
        }
        visit_function_calls(function, &mut |callee| {
            if functions.contains_key(callee) && !selected.contains(callee) {
                frontier.insert(callee.clone());
            }
        });
    }

    graph_json(
        program,
        source_revision,
        &selected,
        &selected_types,
        &GraphView::Context {
            root: &root,
            depth,
            frontier: &frontier,
        },
    )
    .map(Some)
}

enum GraphView<'a> {
    Module,
    Context {
        root: &'a DeclarationId,
        depth: usize,
        frontier: &'a BTreeSet<DeclarationId>,
    },
}

fn graph_json(
    program: &ResolvedProgram,
    source_revision: &str,
    selected_functions: &BTreeSet<DeclarationId>,
    selected_types: &BTreeSet<DeclarationId>,
    view: &GraphView<'_>,
) -> Result<String, Diagnostic> {
    let mut output = String::new();
    write!(
        output,
        "{{\"schema\":\"semaprax.graph.v3\",\"revision\":{},\"view\":{},\"identity\":{{\"declarations\":\"explicit-persistent-or-automatic-unstable\",\"values\":\"revision-scoped-structural\",\"expressions\":\"revision-scoped-structural\"}},\"module\":{},\"permits\":{},\"entrypoint\":{},\"type_facts\":[{}],\"nodes\":[",
        quote_json(source_revision),
        view_json(view),
        quote_json(&program.module),
        string_array(&program.permits),
        quote_json(program.entrypoint.as_str()),
        type_facts_array(program, selected_functions, selected_types)?
    )
    .expect("writing to a string cannot fail");

    let mut first = true;
    for declaration in &program.types {
        if !selected_types.contains(&declaration.id) {
            continue;
        }
        if !first {
            output.push(',');
        }
        first = false;
        let ty = ResolvedType::Nominal {
            declaration: declaration.id.clone(),
            arguments: Vec::new(),
        };
        let identity_origin = identity_origin(program, &declaration.id)?;
        let memory = match declaration.kind {
            ResolvedTypeDeclarationKind::Resource => "unique",
        };
        write!(
            output,
            "{{\"id\":{},\"kind\":\"resource\",\"name\":{},\"identity_origin\":{},\"persistent\":{},\"memory\":{},\"type_id\":{}}}",
            quote_json(declaration.id.as_str()),
            quote_json(&declaration.name),
            quote_json(identity_origin.text()),
            identity_origin.is_persistent(),
            quote_json(memory),
            quote_json(&ty.identity_key())
        )
        .expect("writing to a string cannot fail");
    }

    for function in &program.functions {
        if !selected_functions.contains(&function.id) {
            continue;
        }
        if !first {
            output.push(',');
        }
        first = false;

        let mut calls = BTreeSet::new();
        visit_function_calls(function, &mut |callee| {
            calls.insert(callee.as_str().to_owned());
        });
        let params = function
            .params
            .iter()
            .map(|param| {
                Ok(format!(
                    "{{\"id\":{},\"name\":{},\"type_id\":{},\"ownership_mode\":{}}}",
                    quote_json(param.id.as_str()),
                    quote_json(&param.name),
                    quote_json(&param.ty.identity_key()),
                    quote_json(ownership_text(param.ownership))
                ))
            })
            .collect::<Result<Vec<_>, Diagnostic>>()?
            .join(",");
        let requires = function
            .requires
            .iter()
            .map(|expression| expr_json(program, expression))
            .collect::<Result<Vec<_>, _>>()?
            .join(",");
        let ensures = function
            .ensures
            .iter()
            .map(|expression| expr_json(program, expression))
            .collect::<Result<Vec<_>, _>>()?
            .join(",");
        let body = expr_json(program, &function.body)?;
        let identity_origin = identity_origin(program, &function.id)?;
        let result_ownership = result_ownership(program, &function.return_type)?;
        let calls = calls.into_iter().collect::<Vec<_>>();

        write!(
            output,
            "{{\"id\":{},\"kind\":\"function\",\"name\":{},\"identity_origin\":{},\"persistent\":{},\"params\":[{}],\"result_id\":{},\"result\":{{\"id\":{},\"type_id\":{},\"ownership_mode\":{}}},\"return_type_id\":{},\"effects\":{},\"requires_graph\":[{}],\"ensures_graph\":[{}],\"calls\":{},\"body\":{}}}",
            quote_json(function.id.as_str()),
            quote_json(&function.name),
            quote_json(identity_origin.text()),
            identity_origin.is_persistent(),
            params,
            quote_json(function.result_id.as_str()),
            quote_json(function.result_id.as_str()),
            quote_json(&function.return_type.identity_key()),
            quote_json(ownership_text(result_ownership)),
            quote_json(&function.return_type.identity_key()),
            string_array(&function.effects),
            requires,
            ensures,
            string_array(&calls),
            body
        )
        .expect("writing to a string cannot fail");
    }
    output.push_str("]}");
    Ok(output)
}

fn result_ownership(
    program: &ResolvedProgram,
    ty: &ResolvedType,
) -> Result<OwnershipMode, Diagnostic> {
    program
        .declarations
        .type_facts(ty)
        .map(|facts| {
            if facts.copy {
                OwnershipMode::Value
            } else {
                OwnershipMode::Own
            }
        })
        .ok_or_else(|| {
            Diagnostic::io(
                "SPX-G001",
                format!(
                    "semantic graph has no facts for resolved type `{}`",
                    ty.identity_key()
                ),
            )
        })
}

fn view_json(view: &GraphView<'_>) -> String {
    match view {
        GraphView::Module => "{\"kind\":\"module\"}".to_owned(),
        GraphView::Context {
            root,
            depth,
            frontier,
        } => format!(
            "{{\"kind\":\"context\",\"root\":{},\"depth\":{depth},\"truncated\":{},\"frontier\":[{}]}}",
            quote_json(root.as_str()),
            !frontier.is_empty(),
            frontier
                .iter()
                .map(|id| quote_json(id.as_str()))
                .collect::<Vec<_>>()
                .join(",")
        ),
    }
}

fn identity_origin(
    program: &ResolvedProgram,
    id: &DeclarationId,
) -> Result<IdentityOrigin, Diagnostic> {
    program
        .declarations
        .declaration(id)
        .map(|declaration| declaration.identity_origin)
        .ok_or_else(|| {
            Diagnostic::io(
                "SPX-G002",
                format!("semantic graph has no declaration metadata for `{id}`"),
            )
        })
}

fn visit_function_calls(function: &ResolvedFunction, visit: &mut impl FnMut(&DeclarationId)) {
    for contract in &function.requires {
        visit_expr_calls(contract, visit);
    }
    visit_expr_calls(&function.body, visit);
    for contract in &function.ensures {
        visit_expr_calls(contract, visit);
    }
}

fn visit_expr_calls(expression: &ResolvedExpr, visit: &mut impl FnMut(&DeclarationId)) {
    match &expression.kind {
        ResolvedExprKind::Int(_) | ResolvedExprKind::Bool(_) | ResolvedExprKind::Place(_) => {}
        ResolvedExprKind::Call { callee, args } => {
            visit(callee);
            for argument in args {
                visit_expr_calls(argument, visit);
            }
        }
        ResolvedExprKind::Unary { value, .. } => visit_expr_calls(value, visit),
        ResolvedExprKind::Binary { left, right, .. } => {
            visit_expr_calls(left, visit);
            visit_expr_calls(right, visit);
        }
        ResolvedExprKind::Block { statements, tail } => {
            for statement in statements {
                let ResolvedStatement::Let { value, .. } = statement;
                visit_expr_calls(value, visit);
            }
            visit_expr_calls(tail, visit);
        }
        ResolvedExprKind::If {
            condition,
            then_branch,
            else_branch,
        } => {
            visit_expr_calls(condition, visit);
            visit_expr_calls(then_branch, visit);
            visit_expr_calls(else_branch, visit);
        }
    }
}

fn collect_function_type_declarations(
    function: &ResolvedFunction,
    declarations: &mut BTreeSet<DeclarationId>,
) {
    for param in &function.params {
        collect_nominal_declarations(&param.ty, declarations);
    }
    collect_nominal_declarations(&function.return_type, declarations);
    for expression in &function.requires {
        collect_expr_type_declarations(expression, declarations);
    }
    collect_expr_type_declarations(&function.body, declarations);
    for expression in &function.ensures {
        collect_expr_type_declarations(expression, declarations);
    }
}

fn collect_expr_type_declarations(
    expression: &ResolvedExpr,
    declarations: &mut BTreeSet<DeclarationId>,
) {
    collect_nominal_declarations(&expression.ty, declarations);
    match &expression.kind {
        ResolvedExprKind::Int(_) | ResolvedExprKind::Bool(_) | ResolvedExprKind::Place(_) => {}
        ResolvedExprKind::Call { args, .. } => {
            for argument in args {
                collect_expr_type_declarations(argument, declarations);
            }
        }
        ResolvedExprKind::Unary { value, .. } => {
            collect_expr_type_declarations(value, declarations);
        }
        ResolvedExprKind::Binary { left, right, .. } => {
            collect_expr_type_declarations(left, declarations);
            collect_expr_type_declarations(right, declarations);
        }
        ResolvedExprKind::Block { statements, tail } => {
            for statement in statements {
                let ResolvedStatement::Let { binding, value, .. } = statement;
                collect_nominal_declarations(&binding.ty, declarations);
                collect_expr_type_declarations(value, declarations);
            }
            collect_expr_type_declarations(tail, declarations);
        }
        ResolvedExprKind::If {
            condition,
            then_branch,
            else_branch,
        } => {
            collect_expr_type_declarations(condition, declarations);
            collect_expr_type_declarations(then_branch, declarations);
            collect_expr_type_declarations(else_branch, declarations);
        }
    }
}

fn collect_nominal_declarations(ty: &ResolvedType, declarations: &mut BTreeSet<DeclarationId>) {
    if let ResolvedType::Nominal {
        declaration,
        arguments,
    } = ty
    {
        declarations.insert(declaration.clone());
        for argument in arguments {
            collect_nominal_declarations(argument, declarations);
        }
    }
}

fn expr_json(program: &ResolvedProgram, expression: &ResolvedExpr) -> Result<String, Diagnostic> {
    let header = format!(
        "\"id\":{},\"type_id\":{},\"ownership_mode\":{}",
        quote_json(expression.id.as_str()),
        quote_json(&expression.ty.identity_key()),
        quote_json(ownership_text(expression.ownership))
    );
    let output = match &expression.kind {
        ResolvedExprKind::Int(value) => {
            format!(
                "{{{header},\"kind\":\"int\",\"value\":{}}}",
                quote_json(&value.to_string())
            )
        }
        ResolvedExprKind::Bool(value) => {
            format!("{{{header},\"kind\":\"bool\",\"value\":{value}}}")
        }
        ResolvedExprKind::Place(place) => format!(
            "{{{header},\"kind\":\"place\",\"place\":{}}}",
            place_json(place)
        ),
        ResolvedExprKind::Call { callee, args } => format!(
            "{{{header},\"kind\":\"call\",\"callee\":{},\"args\":[{}]}}",
            quote_json(callee.as_str()),
            args.iter()
                .map(|argument| expr_json(program, argument))
                .collect::<Result<Vec<_>, _>>()?
                .join(",")
        ),
        ResolvedExprKind::Unary { op, value } => format!(
            "{{{header},\"kind\":\"unary\",\"op\":{},\"value\":{}}}",
            quote_json(unary_text(*op)),
            expr_json(program, value)?
        ),
        ResolvedExprKind::Binary { op, left, right } => format!(
            "{{{header},\"kind\":\"binary\",\"op\":{},\"left\":{},\"right\":{}}}",
            quote_json(binary_text(*op)),
            expr_json(program, left)?,
            expr_json(program, right)?
        ),
        ResolvedExprKind::Block { statements, tail } => format!(
            "{{{header},\"kind\":\"block\",\"statements\":[{}],\"tail\":{}}}",
            statements
                .iter()
                .map(|statement| statement_json(program, statement))
                .collect::<Result<Vec<_>, _>>()?
                .join(","),
            expr_json(program, tail)?
        ),
        ResolvedExprKind::If {
            condition,
            then_branch,
            else_branch,
        } => format!(
            "{{{header},\"kind\":\"if\",\"condition\":{},\"then\":{},\"else\":{}}}",
            expr_json(program, condition)?,
            expr_json(program, then_branch)?,
            expr_json(program, else_branch)?
        ),
    };
    Ok(output)
}

fn statement_json(
    program: &ResolvedProgram,
    statement: &ResolvedStatement,
) -> Result<String, Diagnostic> {
    match statement {
        ResolvedStatement::Let { binding, value, .. } => Ok(format!(
            "{{\"kind\":\"let\",\"binding\":{{\"id\":{},\"name\":{},\"type_id\":{},\"ownership_mode\":{}}},\"value\":{}}}",
            quote_json(binding.id.as_str()),
            quote_json(&binding.name),
            quote_json(&binding.ty.identity_key()),
            quote_json(ownership_text(binding.ownership)),
            expr_json(program, value)?
        )),
    }
}

fn place_json(place: &Place) -> String {
    format!(
        "{{\"root\":{},\"projections\":[{}]}}",
        quote_json(place.root.as_str()),
        place
            .projections
            .iter()
            .map(projection_json)
            .collect::<Vec<_>>()
            .join(",")
    )
}

fn projection_json(projection: &PlaceProjection) -> String {
    match projection {
        PlaceProjection::Field(field) => format!(
            "{{\"kind\":\"field\",\"field\":{}}}",
            quote_json(field.as_str())
        ),
        PlaceProjection::VariantField { case, field } => format!(
            "{{\"kind\":\"variant_field\",\"case\":{},\"field\":{}}}",
            quote_json(case.as_str()),
            quote_json(field.as_str())
        ),
    }
}

fn type_facts_array(
    program: &ResolvedProgram,
    selected_functions: &BTreeSet<DeclarationId>,
    selected_types: &BTreeSet<DeclarationId>,
) -> Result<String, Diagnostic> {
    let mut types = BTreeMap::new();
    for declaration in &program.types {
        if !selected_types.contains(&declaration.id) {
            continue;
        }
        collect_type(
            &ResolvedType::Nominal {
                declaration: declaration.id.clone(),
                arguments: Vec::new(),
            },
            &mut types,
        );
    }
    for function in &program.functions {
        if !selected_functions.contains(&function.id) {
            continue;
        }
        for param in &function.params {
            collect_type(&param.ty, &mut types);
        }
        collect_type(&function.return_type, &mut types);
        for expression in &function.requires {
            collect_expr_types(expression, &mut types);
        }
        collect_expr_types(&function.body, &mut types);
        for expression in &function.ensures {
            collect_expr_types(expression, &mut types);
        }
    }
    types
        .values()
        .map(|ty| {
            Ok(format!(
                "{{\"id\":{},\"type\":{},\"facts\":{}}}",
                quote_json(&ty.identity_key()),
                type_json(ty),
                facts_json(program, ty)?
            ))
        })
        .collect::<Result<Vec<_>, Diagnostic>>()
        .map(|items| items.join(","))
}

fn collect_expr_types(expression: &ResolvedExpr, types: &mut BTreeMap<String, ResolvedType>) {
    collect_type(&expression.ty, types);
    match &expression.kind {
        ResolvedExprKind::Int(_) | ResolvedExprKind::Bool(_) | ResolvedExprKind::Place(_) => {}
        ResolvedExprKind::Call { args, .. } => {
            for argument in args {
                collect_expr_types(argument, types);
            }
        }
        ResolvedExprKind::Unary { value, .. } => collect_expr_types(value, types),
        ResolvedExprKind::Binary { left, right, .. } => {
            collect_expr_types(left, types);
            collect_expr_types(right, types);
        }
        ResolvedExprKind::Block { statements, tail } => {
            for statement in statements {
                let ResolvedStatement::Let { binding, value, .. } = statement;
                collect_type(&binding.ty, types);
                collect_expr_types(value, types);
            }
            collect_expr_types(tail, types);
        }
        ResolvedExprKind::If {
            condition,
            then_branch,
            else_branch,
        } => {
            collect_expr_types(condition, types);
            collect_expr_types(then_branch, types);
            collect_expr_types(else_branch, types);
        }
    }
}

fn collect_type(ty: &ResolvedType, types: &mut BTreeMap<String, ResolvedType>) {
    types.entry(ty.identity_key()).or_insert_with(|| ty.clone());
    if let ResolvedType::Nominal { arguments, .. } = ty {
        for argument in arguments {
            collect_type(argument, types);
        }
    }
}

fn type_json(ty: &ResolvedType) -> String {
    match ty {
        ResolvedType::I64 => "{\"kind\":\"primitive\",\"name\":\"i64\"}".to_owned(),
        ResolvedType::Bool => "{\"kind\":\"primitive\",\"name\":\"bool\"}".to_owned(),
        ResolvedType::TypeParameter { owner, index } => format!(
            "{{\"kind\":\"type_parameter\",\"owner\":{},\"index\":{index}}}",
            quote_json(owner.as_str())
        ),
        ResolvedType::Nominal {
            declaration,
            arguments,
        } => format!(
            "{{\"kind\":\"nominal\",\"declaration\":{},\"arguments\":[{}]}}",
            quote_json(declaration.as_str()),
            arguments
                .iter()
                .map(type_json)
                .collect::<Vec<_>>()
                .join(",")
        ),
    }
}

fn facts_json(program: &ResolvedProgram, ty: &ResolvedType) -> Result<String, Diagnostic> {
    program
        .declarations
        .type_facts(ty)
        .map(|facts| facts_object(&facts))
        .ok_or_else(|| {
            Diagnostic::io(
                "SPX-G001",
                format!(
                    "semantic graph has no facts for resolved type `{}`",
                    ty.identity_key()
                ),
            )
        })
}

fn facts_object(facts: &TypeFacts) -> String {
    format!(
        "{{\"copy\":{},\"contains_resource\":{},\"sized\":{},\"needs_drop\":{},\"layout_key\":{}}}",
        facts.copy,
        facts.contains_resource,
        facts.sized,
        facts.needs_drop,
        quote_json(&facts.layout_key)
    )
}

fn ownership_text(ownership: OwnershipMode) -> &'static str {
    match ownership {
        OwnershipMode::Value => "value",
        OwnershipMode::Own => "own",
        OwnershipMode::Borrow => "borrow",
        OwnershipMode::Shared => "shared",
    }
}

fn unary_text(op: UnaryOp) -> &'static str {
    match op {
        UnaryOp::Neg => "-",
        UnaryOp::Not => "!",
    }
}

fn binary_text(op: BinaryOp) -> &'static str {
    op.text()
}

fn string_array(values: &[String]) -> String {
    format!(
        "[{}]",
        values
            .iter()
            .map(|value| quote_json(value))
            .collect::<Vec<_>>()
            .join(",")
    )
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::{to_hir_json, ResolvedProgram};
    use crate::{hir, parse};

    fn resolved_program() -> ResolvedProgram {
        let source = r#"
module test.graph_hir;
@id("app.main")
fn main() -> i64 { 42 }
"#;
        hir::resolve(&parse(source, Path::new("graph-hir.spx")).unwrap()).unwrap()
    }

    #[test]
    fn internal_hir_renderer_revalidates_before_serializing() {
        let mut program = resolved_program();
        program.entrypoint = hir::DeclarationId::new("missing.entrypoint");
        assert_eq!(
            to_hir_json(&program, "trusted-source-revision")
                .unwrap_err()
                .code,
            "SPX-H006"
        );
    }

    #[test]
    fn internal_hir_renderer_preserves_its_trusted_source_revision() {
        let graph = to_hir_json(&resolved_program(), "trusted-source-revision").unwrap();
        assert!(graph.contains("\"revision\":\"trusted-source-revision\""));
    }
}
