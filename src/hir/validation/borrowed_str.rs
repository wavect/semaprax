//! Root authentication for immutable borrowed-string local bindings.

use super::*;

impl HirValidator<'_> {
    pub(super) fn borrowed_str_let_origin(
        &self,
        value: &ResolvedExpr,
    ) -> Result<(Place, bool), Diagnostic> {
        match &value.kind {
            ResolvedExprKind::VecFieldRead { args, .. } => {
                let Some(ResolvedExpr {
                    kind: ResolvedExprKind::Place(place),
                    ..
                }) = args.first()
                else {
                    return Err(hir_error("vector view lacks a named carrier origin"));
                };
                Ok((place.clone(), true))
            }
            ResolvedExprKind::Place(place) if place.projections.is_empty() => self
                .borrowed_str_aliases
                .get(&place.root)
                .cloned()
                .map(|origin| (origin, false))
                .ok_or_else(|| {
                    hir_error("borrowed-str local alias lacks authenticated root provenance")
                }),
            ResolvedExprKind::BorrowPlace { operation, place }
                if operation.as_str() == crate::byte_ops::STRING_AS_STR_ID =>
            {
                Ok((place.clone(), true))
            }
            ResolvedExprKind::NativeRustImportCall(call)
                if matches!(call.result, ResolvedImportResultKind::BorrowedStr { .. }) =>
            {
                let import = self
                    .program
                    .interfaces
                    .iter()
                    .flat_map(|interface| &interface.imports)
                    .find(|import| import.id == call.import)
                    .filter(|import| {
                        native_borrow::admitted(self.program, import)
                            && import.result.kind == call.result
                    })
                    .ok_or_else(|| {
                        hir_error("native string view lacks its authenticated receiver relation")
                    })?;
                let [receiver] = call.args.as_slice() else {
                    return Err(hir_error("native string view must have one receiver"));
                };
                let ResolvedExprKind::Place(place) = &receiver.kind else {
                    return Err(hir_error("native string view requires a named receiver"));
                };
                if !place.projections.is_empty() || receiver.ty != import.parameters[0].ty {
                    return Err(hir_error("native string view receiver identity mismatch"));
                }
                Ok((place.clone(), true))
            }
            ResolvedExprKind::HostCommandCall(call)
                if call.operation == ResolvedHostCommandOperation::ArgUtf8
                    || crate::environment_ops::is_lookup(call.operation) =>
            {
                Ok((
                    Place {
                        root: ValueId::intrinsic_parameter(
                            if crate::environment_ops::is_lookup(call.operation) {
                                crate::environment_ops::ARENA_ID
                            } else {
                                crate::command_io_ops::ARG_UTF8_ID
                            },
                            usize::MAX,
                        ),
                        projections: Vec::new(),
                    },
                    false,
                ))
            }
            _ => Err(hir_error(
                "borrowed-str local must be an exact alias or authenticated owning String view",
            )),
        }
    }
}
