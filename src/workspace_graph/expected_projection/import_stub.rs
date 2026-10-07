//! Clone only an ordinary import signature, never its discarded implementation.
use crate::ast::{
    Expr, ExprKind, Function, Param, ParamMode, Type, TypeDeclaration, TypeDeclarationKind,
};

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

// A non-executable renewal prototype forwards existing storage. Constructing
// a default here would invent an allocation in the caller's loop; the real
// provider body is retained and its capacity/effects replayed before linking.
fn record_shape(declaration: &TypeDeclaration) -> bool {
    let TypeDeclarationKind::Record { fields } = &declaration.kind else {
        return false;
    };
    declaration.explicit_id
        && declaration.type_parameters.is_empty()
        && fields.len() == 2
        && fields.iter().all(|field| field.explicit_id)
        && fields
            .iter()
            .filter(|field| field.ty == Type::Bytes)
            .count()
            == 1
        && fields
            .iter()
            .filter(|field| field.ty == Type::Usize)
            .count()
            == 1
}

fn forward_parameter<'a>(
    function: &'a Function,
    allow_owned: bool,
    is_record: impl Fn(&Type) -> bool,
) -> Option<&'a Param> {
    if crate::stdin_stream_ops::ast_forward_signature(function) {
        return function.params.first();
    }
    if !allow_owned
        || !function.effects.is_empty()
        || !function.type_parameters.is_empty()
        || !is_record(&function.return_type)
    {
        return None;
    }
    let mut owner = None;
    for parameter in &function.params {
        match parameter.mode {
            ParamMode::Own if parameter.ty == function.return_type && owner.is_none() => {
                owner = Some(parameter)
            }
            ParamMode::Borrow if is_record(&parameter.ty) => {}
            ParamMode::Value if crate::vec_ops::ast_element_is_admitted(&parameter.ty) => {}
            _ => return None,
        }
    }
    owner
}

// A sealed Reader has no default constructor. Its non-executable import
// prototype forwards its one existing owner; the provider's real body is
// checked independently in the defining module and retained at link time.
pub(super) fn default_expr(
    function: &Function,
    declarations: &[(&str, &crate::ast::TypeDeclaration)],
    allow_owned: bool,
) -> Result<Expr, Vec<crate::diagnostic::Diagnostic>> {
    if let Some(parameter) = forward_parameter(function, allow_owned, |ty| {
        let Type::Named { name, arguments } = ty else {
            return false;
        };
        arguments.is_empty()
            && declarations
                .iter()
                .any(|(candidate, declaration)| *candidate == name && record_shape(declaration))
    }) {
        super::reserve_builder_structure(std::mem::size_of::<Expr>())?;
        return Ok(Expr {
            kind: ExprKind::Var(crate::bounded_output::budgeted_clone(&parameter.name)),
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
    if let Some(parameter) = forward_parameter(function, allow_owned, |ty| {
        let Type::Named { name, arguments } = ty else {
            return false;
        };
        arguments.is_empty()
            && super::super::resolve_type_id(module, name, programs)
                .and_then(|id| authored.get(id.as_str()).and_then(|target| target.ty))
                .is_some_and(record_shape)
    }) {
        let string_bytes = parameter.name.len();
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
