//! Clone only an ordinary import signature, never its discarded implementation.
use crate::ast::{Expr, ExprKind, Function};

pub(super) fn signature(function: &Function) -> Function {
    Function {
        stable_id: function.stable_id.clone(),
        explicit_id: function.explicit_id,
        name: function.name.clone(),
        name_span: function.name_span,
        type_parameters: function.type_parameters.clone(),
        params: function.params.clone(),
        return_type: function.return_type.clone(),
        effects: function.effects.clone(),
        yields: None,
        follows: None,
        requires: Vec::new(),
        ensures: Vec::new(),
        // Replaced by the rewritten return type's checked default before use.
        body: Expr {
            kind: ExprKind::Int(0),
            span: function.body.span,
        },
        span: function.span,
    }
}
