//! Payload-free variant equality: both operands are Copy variant values of
//! one declaration, so `==` and `!=` compare their stored case tags.

use super::*;

impl<'a, O: COutput> CEmitter<'a, O> {
    pub(super) fn emit_case_equality(
        &mut self,
        op: BinaryOp,
        left: CValue,
        right: &ResolvedExpr,
        result_type: &ResolvedType,
    ) -> Result<CValue, Diagnostic> {
        let right = self.emit_expr(right)?;
        self.require_type(&right.ty, &left.ty, "variant equality right operand")?;
        self.require_type(result_type, &ResolvedType::Bool, "variant equality result")?;
        let temporary = self.temporary(&ResolvedType::Bool)?;
        let operator = if op == BinaryOp::Eq { "==" } else { "!=" };
        self.line(&format!(
            "{temporary} = ({}.spx_tag {operator} {}.spx_tag);",
            left.code, right.code
        ));
        Ok(CValue {
            code: temporary,
            ty: ResolvedType::Bool,
        })
    }
}
