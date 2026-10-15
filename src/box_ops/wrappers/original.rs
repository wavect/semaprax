//! Complete original wrapper classifier and body receiver.
use super::*;

pub(crate) fn source_wrapper(program: &dyn ProgramSource, function: &Function) -> Option<BoxOp> {
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
    if function.name != wrapper_name(op) {
        return None;
    }
    let parameter = Type::Named {
        name: "T".to_owned(),
        arguments: Vec::new(),
    };
    let expected = super::super::ast_params(op, &parameter);
    if function.params.len() != 1
        || function.params[0].name != "value"
        || function.params[0].mode != expected[0].mode
        || function.params[0].ty != expected[0].ty
        || function.return_type != op.ast_return_type(&parameter)
    {
        return None;
    }
    exact_source_body(&function.body, op, &function.params[0]).then_some(op)
}
fn exact_source_body(body: &Expr, op: BoxOp, param: &crate::ast::Param) -> bool {
    let ExprKind::Block { statements, tail } = &body.kind else {
        return false;
    };
    let ExprKind::Call {
        name,
        type_arguments,
        args,
    } = &tail.kind
    else {
        return false;
    };
    statements.is_empty()
        && name == op.name()
        && type_arguments
            == &[Type::Named {
                name: "T".to_owned(),
                arguments: Vec::new(),
            }]
        && matches!(args.as_slice(),[arg] if matches!(&arg.kind,ExprKind::Var(name) if name==&param.name))
}
