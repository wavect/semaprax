//! Executable type and declared ownership validation.
use super::*;

impl HirValidator<'_> {
    pub(super) fn validate_type(&self, ty: &ResolvedType) -> Result<(), Diagnostic> {
        enum Frame<'a> {
            Enter(&'a ResolvedType),
            Finish(&'a ResolvedType),
        }
        let mut frames = vec![Frame::Enter(ty)];
        while let Some(frame) = frames.pop() {
            match frame {
                Frame::Enter(
                    ResolvedType::Unit
                    | ResolvedType::I64
                    | ResolvedType::I32
                    | ResolvedType::Char
                    | ResolvedType::U8
                    | ResolvedType::Usize
                    | ResolvedType::ArrayU8(_)
                    | ResolvedType::F32
                    | ResolvedType::F64
                    | ResolvedType::Bool
                    | ResolvedType::String
                    | ResolvedType::Bytes
                    | ResolvedType::Str
                    | ResolvedType::SliceU8,
                ) => {}
                Frame::Enter(ty @ ResolvedType::Function { .. }) => {
                    if !super::function_value::is_signature(ty) {
                        return Err(hir_error("invalid function value signature"));
                    }
                }
                Frame::Enter(ResolvedType::TypeParameter { .. }) => {
                    return Err(hir_error(
                        "uninstantiated type parameters are not valid in executable HIR",
                    ));
                }
                Frame::Enter(
                    ty @ ResolvedType::Nominal {
                        declaration,
                        arguments,
                    },
                ) => {
                    let kind = self
                        .program
                        .declarations
                        .declaration(declaration)
                        .map(|item| item.kind)
                        .filter(|kind| {
                            matches!(
                                kind,
                                DeclarationKind::Resource
                                    | DeclarationKind::Record
                                    | DeclarationKind::Class
                                    | DeclarationKind::Variant
                            )
                        })
                        .ok_or_else(|| {
                            hir_error(format!(
                                "nominal type `{declaration}` is not a resolved type declaration"
                            ))
                        })?;
                    let parameters = self
                        .program
                        .declarations
                        .type_parameters(declaration)
                        .ok_or_else(|| {
                            hir_error(format!("nominal type `{declaration}` has no parameters"))
                        })?;
                    if arguments.len() != parameters.len() {
                        return Err(hir_error(format!(
                            "nominal type `{declaration}` has incorrect argument arity"
                        )));
                    }
                    let admitted_owned_record = super::type_reachability::is_flat_owned_byte_record(
                        &self.program.declarations,
                        ty,
                    );
                    let admitted_nested_owned_record =
                        super::type_reachability::is_admitted_nested_owned_byte_record(
                            &self.program.declarations,
                            ty,
                        );
                    let admitted_owned_variant =
                        super::type_reachability::is_admitted_concrete_owned_byte_variant(
                            &self.program.declarations,
                            ty,
                        );
                    let admitted_owned_generic = box_intrinsic::is_type(declaration, arguments);
                    if !arguments.is_empty()
                        && (!matches!(kind, DeclarationKind::Record | DeclarationKind::Variant)
                            || (!admitted_owned_byte_prelude_instance(declaration, arguments)
                                && !admitted_owned_record
                                && !admitted_nested_owned_record
                                && !admitted_owned_variant
                                && !admitted_owned_generic
                                && (arguments.as_slice() != [ResolvedType::U8]
                                    || declaration.as_str() != crate::prelude::OPTION_ID)
                                && arguments.iter().any(|argument| {
                                    !matches!(argument, ResolvedType::I64 | ResolvedType::Bool)
                                })))
                    {
                        return Err(hir_error(format!(
                            "nominal type `{declaration}` has unsupported generic arguments"
                        )));
                    }
                    frames.push(Frame::Finish(ty));
                    for argument in arguments.iter().rev() {
                        frames.push(Frame::Enter(argument));
                    }
                }
                Frame::Finish(ty) => {
                    self.program.declarations.type_facts(ty).ok_or_else(|| {
                        hir_error(format!(
                            "type `{}` has no semantic facts",
                            ty.identity_key()
                        ))
                    })?;
                }
            }
        }
        Ok(())
    }

    pub(super) fn validate_declared_ownership(
        &self,
        ty: &ResolvedType,
        ownership: OwnershipMode,
    ) -> Result<(), Diagnostic> {
        if ty == &ResolvedType::Str {
            return if ownership == OwnershipMode::Borrow {
                Ok(())
            } else {
                Err(hir_error("borrowed `str` must have borrow ownership"))
            };
        }
        let facts = self.program.declarations.type_facts(ty).ok_or_else(|| {
            hir_error(format!(
                "type `{}` has no semantic facts",
                ty.identity_key()
            ))
        })?;
        if (facts.copy && ownership != OwnershipMode::Value)
            || (!facts.copy && ownership == OwnershipMode::Value)
        {
            return Err(hir_error(format!(
                "type `{}` has an invalid ownership mode",
                ty.identity_key()
            )));
        }
        Ok(())
    }
}
