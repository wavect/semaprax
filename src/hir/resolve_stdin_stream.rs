//! Resolution of the sealed streaming reader inspections and owned host calls.
use super::*;
use crate::ast::{Expr, ExprKind, Span};

impl Resolver<'_> {
    pub(super) fn resolve_stdin_inspection(
        &self,
        execution: &FunctionExecutionId,
        expression: &Expr,
        bindings: &BTreeMap<String, Binding>,
        path: &str,
    ) -> Result<Option<ResolvedExpr>, Diagnostic> {
        let ExprKind::Call {
            name,
            type_arguments,
            args,
        } = &expression.kind
        else {
            return Ok(None);
        };
        let Some(op) = crate::stdin_stream_ops::pure_by_name(name) else {
            return Ok(None);
        };
        let [argument] = args.as_slice() else {
            return Err(self.error(
                "SPX-T270",
                "streaming stdin inspection requires one reader",
                expression.span,
            ));
        };
        let ExprKind::Var(name) = &argument.kind else {
            return Err(self.error(
                "SPX-T270",
                "streaming stdin inspection requires an exact named reader",
                argument.span,
            ));
        };
        let binding = bindings.get(name).ok_or_else(|| {
            self.error(
                "SPX-H002",
                "streaming stdin reader is out of scope",
                argument.span,
            )
        })?;
        if !type_arguments.is_empty()
            || !crate::stdin_stream_ops::is_reader(&binding.ty)
            || !matches!(
                binding.ownership,
                OwnershipMode::Own | OwnershipMode::Borrow
            )
        {
            return Err(self.error(
                "SPX-T270",
                "invalid streaming stdin inspection operand",
                argument.span,
            ));
        }
        let place = Place {
            root: binding.id.clone(),
            projections: Vec::new(),
        };
        let (kind, ownership) = match op {
            crate::stdin_stream_ops::PureOp::Chunk => (
                ResolvedExprKind::BorrowPlace {
                    operation: DeclarationId::new(op.id()),
                    place,
                },
                OwnershipMode::Borrow,
            ),
            crate::stdin_stream_ops::PureOp::Eof => (
                ResolvedExprKind::Call {
                    callee: DeclarationId::new(op.id()),
                    type_arguments: Vec::new(),
                    instance: None,
                    args: vec![ResolvedExpr {
                        id: ExpressionId::new(execution, &format!("{path}.arg.0")),
                        ty: binding.ty.clone(),
                        ownership: binding.ownership,
                        kind: ResolvedExprKind::Place(place),
                        span: argument.span,
                    }],
                },
                OwnershipMode::Value,
            ),
        };
        Ok(Some(ResolvedExpr {
            id: ExpressionId::new(execution, path),
            ty: op.result(),
            ownership,
            kind,
            span: expression.span,
        }))
    }
    pub(super) fn finish_host_command(
        &self,
        execution: &FunctionExecutionId,
        path: &str,
        span: Span,
        op: ResolvedHostCommandOperation,
        args: Vec<ResolvedExpr>,
    ) -> Result<ResolvedExpr, Diagnostic> {
        for (index, argument) in args.iter().enumerate() {
            if !crate::command_io_ops::accepts_resolved(op, index, &argument.ty) {
                return Err(self.error(
                    "SPX-T270",
                    format!(
                        "command I/O operation `{}` argument {index} has the wrong type",
                        crate::command_io_ops::name(op)
                    ),
                    argument.span,
                ));
            }
        }
        let expression = ExpressionId::new(execution, path);
        Ok(ResolvedExpr {
            id: expression.clone(),
            ty: crate::command_io_ops::return_type(op),
            ownership: crate::command_io_ops::result_ownership(op),
            kind: ResolvedExprKind::HostCommandCall(ResolvedHostCommandCall {
                expression,
                operation: op,
                args,
            }),
            span,
        })
    }
}
