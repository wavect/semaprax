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

// A sealed Reader has no default constructor. Its non-executable import
// prototype forwards its one existing owner; the provider's real body is
// checked independently in the defining module and retained at link time.
pub(super) fn default_expr(
    function: &Function,
    declarations: &[(&str, &crate::ast::TypeDeclaration)],
    allow_owned: bool,
) -> Result<Expr, Vec<crate::diagnostic::Diagnostic>> {
    if crate::stdin_stream_ops::ast_forward_signature(function) {
        super::reserve_builder_structure(std::mem::size_of::<Expr>())?;
        return Ok(Expr {
            kind: ExprKind::Var(crate::bounded_output::budgeted_clone(
                &function.params[0].name,
            )),
            span: crate::ast::Span::default(),
        });
    }
    super::defaults::default_expr(&function.return_type, declarations, allow_owned)
}

#[allow(clippy::too_many_arguments)]
pub(super) fn default_expr_expanded_cost(
    function: &Function,
    module: &str,
    caller: &crate::ast::Program,
    authored: &std::collections::BTreeMap<&str, super::AuthoredDeclaration<'_>>,
    programs: &[crate::ast::Program],
    memo: &mut [std::collections::BTreeMap<String, super::ExpandedDefaultCost>; 2],
    visiting: &mut std::collections::BTreeSet<String>,
    allow_owned: bool,
) -> Result<super::ExpandedDefaultCost, Vec<crate::diagnostic::Diagnostic>> {
    if crate::stdin_stream_ops::ast_forward_signature(function) {
        let string_bytes = function.params[0].name.len();
        return Ok(super::ExpandedDefaultCost {
            bytes: super::checked_builder_sum(std::mem::size_of::<Expr>(), string_bytes)?,
            string_bytes,
            // Ordinary Var HIR carries the expression/value/type identities;
            // signature nominal identities are charged by the caller already.
            identity_slots: 3,
        });
    }
    super::defaults::default_expr_expanded_cost(
        &function.return_type,
        module,
        caller,
        authored,
        programs,
        memo,
        visiting,
        allow_owned,
    )
}
