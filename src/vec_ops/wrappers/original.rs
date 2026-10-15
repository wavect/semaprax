//! Complete original wrapper classifier and body receiver.
use super::*;

pub(crate) fn source_wrapper(program: &dyn ProgramSource, function: &Function) -> Option<VecOp> {
    if !source_module_is_authenticated(program)
        || !function.explicit_id
        || function.type_parameters.len() != 1
        || function.type_parameters[0].name != "T"
        || !function.effects.is_empty()
        || !function.requires.is_empty()
        || !function.ensures.is_empty()
    {
        return None;
    }
    let op = wrapper_by_id(&function.stable_id)?;
    if function.name != wrapper_name(op)? {
        return None;
    }
    let parameter = Type::Named {
        name: "T".to_owned(),
        arguments: Vec::new(),
    };
    let expected = super::super::ast_params(op, &parameter);
    let expected_names: &[&str] = match op {
        VecOp::WithCapacity => &["capacity"],
        VecOp::Push => &["values", "value"],
        VecOp::Len | VecOp::Capacity => &["values"],
        VecOp::Get => &["values", "index"],
        VecOp::ReserveExact => &["values", "additional"],
        VecOp::Set => &["values", "index", "value"],
        VecOp::Clear | VecOp::Sort => &["values"],
        VecOp::CloneAt
        | VecOp::Replace
        | VecOp::ReserveOwned
        | VecOp::SortOwned
        | VecOp::SortBy
        | VecOp::StrAt
        | VecOp::UniqueBy
        | VecOp::UpdateField => return None,
    };
    if function.params.len() != expected.len()
        || function
            .params
            .iter()
            .zip(expected.iter().zip(expected_names))
            .any(|(actual, (expected, name))| {
                actual.name != *name || actual.mode != expected.mode || actual.ty != expected.ty
            })
        || function.return_type != op.ast_return_type(&parameter)
    {
        return None;
    }
    exact_source_body(&function.body, op, &function.params).then_some(op)
}

fn exact_source_body(body: &Expr, op: VecOp, params: &[crate::ast::Param]) -> bool {
    let ExprKind::Block { statements, tail } = &body.kind else {
        return false;
    };
    if !statements.is_empty() {
        return false;
    }
    let ExprKind::Call {
        name,
        type_arguments,
        args,
    } = &tail.kind
    else {
        return false;
    };
    name == op.name()
        && type_arguments
            == &[Type::Named {
                name: "T".to_owned(),
                arguments: Vec::new(),
            }]
        && args.len() == params.len()
        && args.iter().zip(params).all(
            |(argument, parameter)| matches!(&argument.kind, ExprKind::Var(name) if name == &parameter.name),
        )
}
