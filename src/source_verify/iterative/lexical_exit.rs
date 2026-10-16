//! Ordinary return publishes no intermediate expression value.
use super::*;
use crate::ast::ParamMode;
use crate::source_verify::loans::mark_value_sources_moved;
impl<'a, 'v, 'p> IterativeVerifier<'a, 'v, 'p> {
    pub(super) fn resume_lexical_return(
        &mut self,
        expression: &'p Expr,
        operand: &'p Expr,
        scope: usize,
    ) -> Result<(), Diagnostic> {
        let value = self.values.pop().unwrap_or(None);
        if let Some(value) = value {
            if value.native_unit
                || (value.ty != self.current.payload().return_type
                    && !self
                        .recovery
                        .explains(operand, &self.current.payload().return_type))
            {
                self.diagnostics.push(error(
                    self.program,
                    "SPX-T103",
                    "lexical return operand differs from the function result type",
                    operand.span,
                ));
            }
            if self.types.needs_drop(&value.ty) && value.mode != ParamMode::Own {
                self.diagnostics.push(error(
                    self.program,
                    "SPX-O104",
                    "lexical return must transfer an owned result",
                    operand.span,
                ));
            }
            if !self.allow_moves {
                self.diagnostics.push(error(
                    self.program,
                    "SPX-T312",
                    "return is only admitted in the executable function body",
                    expression.span,
                ));
            } else if self.types.needs_drop(&value.ty) {
                mark_value_sources_moved(
                    self.program,
                    operand,
                    &mut self.scopes[scope].bindings,
                    self.types,
                    self.diagnostics,
                );
            }
            self.values.push(Some(CheckedValue::value(Type::Never)));
        } else {
            self.values.push(None);
        }
        Ok(())
    }
}
