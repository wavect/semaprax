//! Host-command admission helpers shared by both HIR validation walks and the
//! while-loop admission re-check.
//!
//! The effect and while-body rules are read from the closed operation tables
//! (`command_io_ops`, `network_io_ops`) so the source verifier, the admission
//! oracle, and this trust boundary cannot disagree about one operation.

use super::*;

/// Every effect a host-command call needs must be declared by the enclosing
/// function; contracts (`allowed_effects == None`) admit no host command.
pub(super) fn require_effects(
    operation: ResolvedHostCommandOperation,
    allowed_effects: Option<&BTreeSet<String>>,
) -> Result<(), Diagnostic> {
    let Some(allowed) = allowed_effects else {
        return Err(hir_error("contract calls effectful host-command operation"));
    };
    if let Some(effect) =
        crate::command_io_ops::required_effects(operation).find(|effect| !allowed.contains(*effect))
    {
        return Err(hir_error(format!(
            "host-command operation requires undeclared effect `{effect}`"
        )));
    }
    Ok(())
}

impl HirValidator<'_> {
    /// Admit one host-command call inside a `while` condition or body and
    /// return the scalar arguments the caller must keep validating. Only the
    /// operations `command_io_ops::admitted_in_while` names qualify; each
    /// borrowed byte-slice argument must be an existing authenticated alias
    /// and is consumed here because a slice place is not a Copy scalar.
    pub(super) fn while_host_command_scalar_arguments<'e>(
        &self,
        expression: &'e ResolvedExpr,
        call: &'e ResolvedHostCommandCall,
    ) -> Result<Vec<&'e ResolvedExpr>, Diagnostic> {
        let operation = call.operation;
        if !crate::command_io_ops::admitted_in_while(operation) {
            return Err(hir_error(
                "while loops cannot contain owned-result or single-write command I/O",
            ));
        }
        let params = crate::command_io_ops::resolved_params(operation);
        if call.args.len() != params.len()
            || expression.ty != crate::command_io_ops::return_type(operation)
            || expression.ownership != OwnershipMode::Value
        {
            return Err(hir_error(
                "while loop command I/O call has a non-canonical shape",
            ));
        }
        let mut scalars = Vec::with_capacity(call.args.len());
        for (argument, param) in call.args.iter().zip(&params) {
            if param.ty != ResolvedType::SliceU8 {
                scalars.push(argument);
                continue;
            }
            let ResolvedExprKind::Place(place) = &argument.kind else {
                return Err(hir_error(
                    "while loop command I/O requires an existing byte-slice alias",
                ));
            };
            if argument.ty != ResolvedType::SliceU8
                || !place.projections.is_empty()
                || (!self.byte_slice_aliases.contains_key(&place.root)
                    && self
                        .program
                        .declarations
                        .byte_slice_provenance(&place.root)
                        .is_none())
            {
                return Err(hir_error(
                    "while loop command I/O lacks authenticated slice provenance",
                ));
            }
        }
        Ok(scalars)
    }
}
