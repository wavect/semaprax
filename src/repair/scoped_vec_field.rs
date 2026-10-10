//! Rebase reads only when exact selected field and fusion metadata agree.
use super::*;
impl StructuralRebase<'_> {
    pub(super) fn compare_vec_field(
        &mut self,
        before: &ResolvedExprKind,
        after: &ResolvedExprKind,
    ) -> Result<(), Vec<Diagnostic>> {
        let (
            ResolvedExprKind::VecFieldRead {
                element: left_element,
                field: left_field,
                bytes: left_bytes,
                args: left_args,
            },
            ResolvedExprKind::VecFieldRead {
                element: right_element,
                field: right_field,
                bytes: right_bytes,
                args: right_args,
            },
        ) = (before, after)
        else {
            return Err(rebase_mismatch());
        };
        if left_element != right_element
            || left_field != right_field
            || left_bytes != right_bytes
            || left_args.len() != 2
            || right_args.len() != 2
        {
            return Err(rebase_mismatch());
        }
        for (left, right) in left_args.iter().zip(right_args) {
            self.compare_expr(left, right)?;
        }
        Ok(())
    }
}
