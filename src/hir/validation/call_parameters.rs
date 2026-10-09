//! Validation-only views of compiler-owned byte-operation signatures.
//!
//! Owned signatures retain their original storage and capacity census. Byte
//! signatures borrow static type metadata; their synthetic identities are
//! formatted only if an existing ownership diagnostic needs the exact text.
use super::*;
use crate::byte_ops::ByteOp;
use std::fmt;

pub(super) enum CallParameters {
    Owned(Vec<ResolvedParam>),
    Byte(ByteOp),
}

pub(super) struct ParameterView<'a> {
    pub(super) ty: &'a ResolvedType,
    pub(super) ownership: OwnershipMode,
    identity: ParameterIdentity<'a>,
}

enum ParameterIdentity<'a> {
    Owned(&'a ValueId),
    Byte(ByteOp, usize),
}

impl fmt::Display for ParameterIdentity<'_> {
    fn fmt(&self, output: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Owned(identity) => fmt::Display::fmt(identity, output),
            Self::Byte(operation, index) => write!(output, "{}.param.{index}", operation.id()),
        }
    }
}

impl<'a> ParameterView<'a> {
    fn owned(parameter: &'a ResolvedParam) -> Self {
        Self {
            ty: &parameter.ty,
            ownership: parameter.ownership,
            identity: ParameterIdentity::Owned(&parameter.id),
        }
    }
}

impl CallParameters {
    pub(super) fn parameter(&self, index: usize) -> ParameterView<'_> {
        match self {
            Self::Owned(parameters) => ParameterView::owned(&parameters[index]),
            Self::Byte(operation) => ParameterView {
                ty: &operation.param_types()[index],
                ownership: operation.param_ownership(index),
                identity: ParameterIdentity::Byte(*operation, index),
            },
        }
    }

    pub(super) fn owned_capacity(&self) -> usize {
        match self {
            Self::Owned(parameters) => {
                parameters.capacity() * std::mem::size_of::<ResolvedParam>()
                    + parameters
                        .iter()
                        .map(|parameter| {
                            parameter.id.as_str().len()
                                + parameter.name.capacity()
                                + resolved_type_owned_capacity(&parameter.ty)
                        })
                        .sum::<usize>()
            }
            // The enum and descriptor are inline in the charged frame; every
            // referenced type is immutable static data, with no heap payload.
            Self::Byte(_) => 0,
        }
    }
}

impl HirValidator<'_> {
    #[cfg(test)]
    pub(super) fn validate_argument_ownership(
        &self,
        argument: &ResolvedExpr,
        param: &ResolvedParam,
    ) -> Result<(), Diagnostic> {
        self.validate_argument_ownership_view(argument, ParameterView::owned(param))
    }

    pub(super) fn validate_argument_ownership_view(
        &self,
        argument: &ResolvedExpr,
        param: ParameterView<'_>,
    ) -> Result<(), Diagnostic> {
        let actual = argument.ownership;
        let facts = self.borrowed_type_facts(param.ty)?.ok_or_else(|| {
            hir_error_at_span(
                argument.span,
                format!("type `{}` has no semantic facts", param.ty.identity_key()),
            )
        })?;
        let valid = if facts.copy {
            actual == OwnershipMode::Value && param.ownership == OwnershipMode::Value
        } else {
            match param.ownership {
                OwnershipMode::Own => actual == OwnershipMode::Own,
                OwnershipMode::Borrow => {
                    let exact_place = matches!(&argument.kind, ResolvedExprKind::Place(_));
                    if crate::stdin_stream_ops::is_reader(param.ty) {
                        matches!(actual, OwnershipMode::Own | OwnershipMode::Borrow)
                            && matches!(&argument.kind, ResolvedExprKind::Place(place) if place.projections.is_empty())
                    } else if *param.ty == ResolvedType::Bytes {
                        matches!(actual, OwnershipMode::Own | OwnershipMode::Borrow) && exact_place
                    } else if crate::map_ops::is_collection(param.ty) {
                        // Compiler-owned nominal collections are leaves, not
                        // authored aggregate records; projected and temporary
                        // readers borrow their carrier without transferring it.
                        matches!(actual, OwnershipMode::Own | OwnershipMode::Borrow)
                    } else if resolved_type_contains_owned_bytes(self.program, param.ty) {
                        (vec_intrinsic::is_owned_vec_carrier(self.program, param.ty)
                            || crate::hir::type_reachability::is_admitted_nested_owned_byte_record(
                                &self.program.declarations,
                                param.ty,
                            )
                            || crate::hir::owned_text_record::admitted(
                                param.ty,
                                &self.program.declarations,
                            )
                            || resolved_type_is_flat_owned_byte_variant(self.program, param.ty))
                            && matches!(actual, OwnershipMode::Own | OwnershipMode::Borrow)
                            && matches!(
                                &argument.kind,
                                ResolvedExprKind::Place(place) if place.projections.is_empty()
                            )
                    } else if crate::hir::owned_text_record::admitted(
                        param.ty,
                        &self.program.declarations,
                    ) || resolved_type_is_flat_owned_string_variant(
                        self.program,
                        param.ty,
                    ) {
                        matches!(actual, OwnershipMode::Own | OwnershipMode::Borrow)
                            && matches!(
                                &argument.kind,
                                ResolvedExprKind::Place(place) if place.projections.is_empty()
                            )
                    } else {
                        true
                    }
                }
                OwnershipMode::Shared => actual == OwnershipMode::Shared,
                OwnershipMode::Value => false,
            }
        };
        if valid {
            Ok(())
        } else {
            Err(hir_error_at_span(
                argument.span,
                format!(
                    "argument ownership is incompatible with parameter `{}`",
                    param.identity
                ),
            ))
        }
    }
}

#[cfg(test)]
mod tests;
