//! Source-scoped computed signature defaults; no independent type admission.

use super::*;

pub(super) fn charge(nodes: &mut usize, additional: usize) -> Result<()> {
    *nodes = nodes
        .checked_add(additional)
        .ok_or_else(|| capacity("computed signature argument inventory overflow"))?;
    if *nodes > MAX_WALK_NODES {
        return Err(capacity(
            "computed signature argument inventory exceeds its bound",
        ));
    }
    Ok(())
}

fn carrier(body: Expr, original: &[Param]) -> Function {
    Function {
        stable_id: String::new(),
        explicit_id: false,
        name: String::new(),
        name_span: Span::default(),
        type_parameters: Vec::new(),
        params: original.to_vec(),
        return_type: Type::I64,
        effects: Vec::new(),
        requires: Vec::new(),
        ensures: Vec::new(),
        body,
        span: Span::default(),
    }
}

/// Construct against actual module bindings and reserve every local in the
/// lowered template before selecting argument staging names. The carrier is
/// solely input to the existing bounded AST visitors, never source admission.
pub(super) fn prepare(
    revision: Option<&ProjectRevision>,
    program: &Program,
    original: &[Param],
    template: &Value,
    occupied: &mut BTreeSet<String>,
    total_nodes: &mut usize,
) -> Result<(Expr, usize)> {
    let revision = revision.ok_or_else(|| {
        grammar("computed signature arguments require a retained checked Project revision")
    })?;
    let scope = original.iter().map(|param| param.name.clone()).collect();
    let body =
        super::super::construct_expression_with_revision(revision, program, &scope, template)?;
    let mut function = carrier(body, original);
    let mut expressions = 0usize;
    let mut bindings = 0usize;
    let mut patterns = 0usize;
    super::super::walk_function(&mut function, &mut expressions, &mut |expression| {
        match &expression.kind {
            ExprKind::Var(name) | ExprKind::Call { name, .. } => {
                occupied.insert(name.clone());
            }
            ExprKind::Block { statements, .. } => {
                charge(&mut bindings, statements.len())?;
                for statement in statements {
                    if let Statement::Let { name, .. } | Statement::Assign { name, .. } = statement
                    {
                        occupied.insert(name.clone());
                    }
                }
            }
            ExprKind::Match { arms, .. } => {
                for arm in arms {
                    reserve_pattern(&arm.pattern, occupied, 0, &mut patterns)?;
                    if let MatchPattern::Variant { fields, .. } = &arm.pattern {
                        charge(&mut bindings, fields.len())?;
                    }
                }
            }
            _ => {}
        }
        Ok(())
    })?;
    let count = expressions
        .checked_add(bindings)
        .and_then(|count| count.checked_add(patterns))
        .ok_or_else(|| capacity("computed signature template inventory overflow"))?;
    charge(total_nodes, count)?;
    Ok((function.body, count))
}

pub(super) fn substitute(
    body: Expr,
    original: &[Param],
    stages: &[String],
    occupied: &mut BTreeSet<String>,
) -> Result<Expr> {
    let renames = original
        .iter()
        .zip(stages)
        .map(|(param, stage)| (param.name.clone(), stage.clone()))
        .collect();
    let destinations = stages.iter().cloned().collect();
    let mut function = carrier(body, original);
    rename::apply(&mut function, original, &renames, &destinations, occupied)?;
    Ok(function.body)
}
