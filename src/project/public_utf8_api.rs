//! Closed Project-v10 UTF-8 descriptor facts and the deliberately narrow
//! string-carrier admission shared by descriptor derivation and replay.

use crate::hir::{ResolvedExpr, ResolvedExprKind, ResolvedFunction, ResolvedType};

pub const PUBLIC_OWNED_UTF8_API_SCHEMA: &str = "semaprax.public-owned-utf8-api.v1";
pub const PUBLIC_OWNED_UTF8_PROJECT_SCHEMA: &str = "semaprax.project.v10";
pub(super) const UTF8_DESCRIPTOR_DIGEST_DOMAIN: &[u8] =
    b"semaprax.public-owned-utf8-api.digest.v1\0";

pub(super) fn validate_closure_shape(function: &ResolvedFunction) -> Result<(), String> {
    if expression_reaches_string_intrinsic(&function.body) {
        return Err(format!(
            "owned UTF-8 closure function `{}` may not call a compiler-owned string intrinsic",
            function.id
        ));
    }
    let reaches_owned_string = expression_reaches_owned_string(&function.body);
    if function.return_type == ResolvedType::String {
        if !is_direct_string_carrier(&function.body) {
            return Err(format!(
                "owned UTF-8 closure function `{}` must return one literal or direct retained-function call carrier",
                function.id
            ));
        }
    } else if reaches_owned_string {
        return Err(format!(
            "owned UTF-8 closure function `{}` may not stage a non-result string",
            function.id
        ));
    }
    Ok(())
}

fn is_direct_string_carrier(expression: &ResolvedExpr) -> bool {
    match &expression.kind {
        ResolvedExprKind::String(_) | ResolvedExprKind::Call { .. } => true,
        ResolvedExprKind::Block { statements, tail } => {
            statements.is_empty() && is_direct_string_carrier(tail)
        }
        _ => false,
    }
}

fn expression_reaches_string_intrinsic(root: &ResolvedExpr) -> bool {
    let mut pending = vec![root];
    while let Some(expression) = pending.pop() {
        if matches!(expression.kind, ResolvedExprKind::LiteralFormat { .. }) {
            return true;
        }
        if let ResolvedExprKind::Call { callee, .. } = &expression.kind {
            if crate::string_ops::by_id(callee.as_str()).is_some() {
                return true;
            }
        }
        crate::hir::push_resolved_expression_children_in_authored_order(expression, &mut pending);
    }
    false
}

fn expression_reaches_owned_string(root: &ResolvedExpr) -> bool {
    let mut pending = vec![root];
    while let Some(expression) = pending.pop() {
        if expression.ty == ResolvedType::String {
            return true;
        }
        crate::hir::push_resolved_expression_children_in_authored_order(expression, &mut pending);
    }
    false
}

#[cfg(test)]
mod tests {
    #[test]
    fn literal_format_does_not_widen_the_closed_v10_string_carrier() {
        let ast = crate::check(
            r#"module utf8.format;
@id("take") fn take(text:own string)->string {text}
@id("render") fn render()->string {take(string_format("{}",1))}
@id("main") fn main()->i64 {0}
"#,
            "utf8-format.spx",
        )
        .unwrap();
        let program = crate::hir::resolve(&ast).unwrap();
        let function = program
            .functions
            .iter()
            .find(|f| f.id.as_str() == "render")
            .unwrap();
        assert!(super::validate_closure_shape(function)
            .unwrap_err()
            .contains("compiler-owned string intrinsic"));
    }
}
