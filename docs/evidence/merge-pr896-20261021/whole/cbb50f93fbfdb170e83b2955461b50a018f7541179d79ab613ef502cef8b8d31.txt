//! Same stub decision and provider proof, with explicit original receiver.
use super::{forward_parameter, record_shape, Expr, ExprKind, Function, ParamMode, Type};
use crate::workspace_graph::constructor_failure::{receiving::Receiver, typed::Failure};
pub(in crate::workspace_graph) fn default_expr(
    function: &Function,
    declarations: &[(&str, &crate::ast::TypeDeclaration)],
    allow_owned: bool,
    allow_byte_forward: bool,
    owner: &mut Receiver,
) -> Result<Expr, Failure> {
    owner.entry::<Expr>()?;
    let byte_owner = function.params.iter().find(|parameter| {
        allow_byte_forward && parameter.mode == ParamMode::Own && parameter.ty == Type::Bytes
    });
    if let Some(parameter) = byte_owner.or_else(|| {
        forward_parameter(function, allow_owned, |ty| {
            let Type::Named { name, arguments } = ty else {
                return false;
            };
            arguments.is_empty()
                && declarations
                    .iter()
                    .any(|(candidate, declaration)| *candidate == name && record_shape(declaration))
        })
    }) {
        owner.reserve(std::mem::size_of::<Expr>())?;
        return Ok(Expr {
            kind: ExprKind::Var(owner.text(&parameter.name)?),
            span: crate::ast::Span::default(),
        });
    }
    super::super::defaults::typed::default_expr(
        &function.return_type,
        declarations,
        allow_owned,
        owner,
    )
}
