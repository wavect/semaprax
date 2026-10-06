//! Case-tag tests over Copy variants: payload-free variant equality, which
//! compares two stored case tags, and or-pattern arms over payload-free cases,
//! which test the staged scrutinee's tag against each alternative.

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

    /// Opens the guarded block of an or-pattern arm in a plain value variant
    /// match; the shared arm epilogue emits the value and closes it.
    pub(super) fn open_case_or_arm(
        &mut self,
        mode: hir::ResolvedMatchMode,
        layout: &VariantLayout,
        staged: &str,
        matched: &str,
        alternatives: &[hir::ResolvedMatchPattern],
    ) -> Result<(), Diagnostic> {
        if mode != hir::ResolvedMatchMode::Value {
            return Err(backend_error(
                "or-pattern arm requires a plain value variant match",
            ));
        }
        let mut tests = Vec::with_capacity(alternatives.len());
        for alternative in alternatives {
            let hir::ResolvedMatchPattern::Variant {
                variant,
                case,
                fields,
            } = alternative
            else {
                return Err(backend_error(
                    "or-pattern alternative is not a case pattern",
                ));
            };
            let case_layout = layout
                .case(case)
                .filter(|_| *variant == layout.variant && fields.is_empty())
                .ok_or_else(|| {
                    backend_error(format!("or-pattern references foreign case `{case}`"))
                })?;
            tests.push(format!("{staged}.spx_tag == UINT32_C({})", case_layout.tag));
        }
        self.line(&format!("if (!{matched} && ({})) {{", tests.join(" || ")));
        self.indent += 1;
        self.line(&format!("{matched} = true;"));
        Ok(())
    }
}
