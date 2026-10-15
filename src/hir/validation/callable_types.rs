//! Executable type and declared ownership validation.
use super::*;
mod inline_frames;
#[cfg(test)]
mod original_receiver;

impl HirValidator<'_> {
    pub(super) fn validate_type(&self, ty: &ResolvedType) -> Result<(), Diagnostic> {
        self.executable_profiles.validate_use(ty)?;
        enum Frame<'a> {
            Enter(&'a ResolvedType),
            Finish(&'a ResolvedType),
        }
        // The first leaf and zero-argument Finish stay inline. Eight generic
        // continuation frames also stay inline; larger worklists spill to Vec.
        let mut frames = inline_frames::Stack::<_, 8>::new();
        let mut inline = Some(Frame::Enter(ty));
        while let Some(frame) = inline.take().or_else(|| frames.pop()) {
            match frame {
                Frame::Enter(
                    ResolvedType::Never
                    | ResolvedType::Unit
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
                    | ResolvedType::SliceU8
                    | ResolvedType::StringMap
                    | ResolvedType::OnceFunction
                    | ResolvedType::OnceFunctionI64
                    | ResolvedType::OnceFunctionI64Pair
                    | ResolvedType::MutFunctionI64,
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
                    if !arguments.is_empty() {
                        let admitted_owned_record =
                            super::type_reachability::is_flat_owned_byte_record(
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
                        let admitted_owned_generic = box_intrinsic::is_type(
                            &self.program.declarations,
                            declaration,
                            arguments,
                        );
                        if !matches!(kind, DeclarationKind::Record | DeclarationKind::Variant)
                            || (!admitted_owned_byte_prelude_instance(declaration, arguments)
                                && !(ResolvedImportResultKind::is_owned_container_type(
                                    ty,
                                    &self.program.declarations,
                                ) && self
                                    .program
                                    .interfaces
                                    .iter()
                                    .flat_map(|i| &i.imports)
                                    .any(|i| {
                                        i.native_rust
                                            && i.result.kind.matches_value_type(&self.program.declarations, ty)
                                    }))
                                && !crate::hir::workspace_link::native_owner::admitted_ri06_regex_result(self.program, ty)
                                && !admitted_owned_record
                                && !admitted_nested_owned_record
                                && !admitted_owned_variant
                                && !super::super::declared_collection::vector(&self.program.declarations,ty)
                                && !admitted_owned_generic
                                && (arguments.as_slice() != [ResolvedType::U8]
                                    || declaration.as_str() != crate::prelude::OPTION_ID)
                                && arguments.iter().any(|argument| {
                                    !matches!(argument, ResolvedType::I64 | ResolvedType::Bool)
                                }))
                    {
                        return Err(hir_error(format!(
                            "nominal type `{declaration}` has unsupported generic arguments"
                        )));
                    }
                    }
                    if arguments.is_empty() {
                        inline = Some(Frame::Finish(ty));
                    } else {
                        frames.push(Frame::Finish(ty));
                        for argument in arguments.iter().rev() {
                            frames.push(Frame::Enter(argument));
                        }
                    }
                }
                Frame::Finish(ty) => {
                    self.program.declarations.type_flags(ty).ok_or_else(|| {
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
        if *ty == ResolvedType::Never {
            return if ownership == OwnershipMode::Value {
                Ok(())
            } else {
                Err(hir_error("lexical exit has invalid ownership mode"))
            };
        }
        if (ty.is_once_function() || ty.is_mut_function()) && ownership != OwnershipMode::Own {
            return Err(hir_error("affine callable requires unique ownership"));
        }
        if ty == &ResolvedType::Str {
            return if ownership == OwnershipMode::Borrow {
                Ok(())
            } else {
                Err(hir_error("borrowed `str` must have borrow ownership"))
            };
        }
        let facts = self.program.declarations.type_flags(ty).ok_or_else(|| {
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

    #[cfg(test)]
    pub(super) fn validate_type_eager_oracle(&self, ty: &ResolvedType) -> Result<(), Diagnostic> {
        crate::hir::owned_text_record::oracle::validate_use(ty, &self.program.declarations)?;
        enum Frame<'a> {
            Enter(&'a ResolvedType),
            Finish(&'a ResolvedType),
        }
        let mut frames = vec![Frame::Enter(ty)];
        while let Some(frame) = frames.pop() {
            match frame {
                Frame::Enter(
                    ResolvedType::Never
                    | ResolvedType::Unit
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
                    | ResolvedType::SliceU8
                    | ResolvedType::StringMap
                    | ResolvedType::OnceFunction
                    | ResolvedType::OnceFunctionI64
                    | ResolvedType::OnceFunctionI64Pair
                    | ResolvedType::MutFunctionI64,
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
                    let admitted_owned_generic =
                        box_intrinsic::is_type(&self.program.declarations, declaration, arguments);
                    if !arguments.is_empty()
                        && (!matches!(kind, DeclarationKind::Record | DeclarationKind::Variant)
                            || (!admitted_owned_byte_prelude_instance(declaration, arguments)
                                && !(ResolvedImportResultKind::is_owned_container_type(
                                    ty,
                                    &self.program.declarations,
                                ) && self
                                    .program
                                    .interfaces
                                    .iter()
                                    .flat_map(|i| &i.imports)
                                    .any(|i| {
                                        i.native_rust
                                            && i.result
                                                .kind
                                                .value_type(&self.program.declarations)
                                                .is_ok_and(|actual| &actual == ty)
                                    }))
                                && !crate::hir::workspace_link::native_owner::admitted_ri06_regex_result(self.program, ty)
                                && !admitted_owned_record
                                && !admitted_nested_owned_record
                                && !admitted_owned_variant
                                && !super::super::declared_collection::vector(&self.program.declarations,ty)
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

    #[cfg(test)]
    pub(super) fn validate_declared_ownership_full_facts_oracle(
        &self,
        ty: &ResolvedType,
        ownership: OwnershipMode,
    ) -> Result<(), Diagnostic> {
        if *ty == ResolvedType::Never {
            return if ownership == OwnershipMode::Value {
                Ok(())
            } else {
                Err(hir_error("lexical exit has invalid ownership mode"))
            };
        }
        if (ty.is_once_function() || ty.is_mut_function()) && ownership != OwnershipMode::Own {
            return Err(hir_error("affine callable requires unique ownership"));
        }
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

impl HirValidator<'_> {
    pub(super) fn finish_expr(
        &self,
        expression: &ResolvedExpr,
        ty: &ResolvedType,
        ownership: OwnershipMode,
    ) -> Result<(), Diagnostic> {
        self.require_type(&expression.ty, ty, "expression")?;
        if expression.ownership != ownership {
            return Err(hir_error(format!(
                "expression `{}` has inconsistent ownership",
                expression.id
            )));
        }
        Ok(())
    }
}

impl HirValidator<'_> {
    /// Float literals must stay finite so canonical source projection and
    /// every backend agree on the exact value; infinities and NaNs cannot be
    /// written as literals and hostile HIR is rejected here.
    pub(super) fn validate_finite_f32(&self, bits: u32) -> Result<(), Diagnostic> {
        if f32::from_bits(bits).is_finite() {
            Ok(())
        } else {
            Err(hir_error(
                "f32 literal bits are not a finite IEEE-754 value",
            ))
        }
    }

    pub(super) fn validate_finite_f64(&self, bits: u64) -> Result<(), Diagnostic> {
        if f64::from_bits(bits).is_finite() {
            Ok(())
        } else {
            Err(hir_error(
                "f64 literal bits are not a finite IEEE-754 value",
            ))
        }
    }
}

#[cfg(test)]
mod type_admission_tests {
    use super::*;
    fn fixture() -> ResolvedProgram {
        let source = crate::parse(
            r#"module admission.frames;
@id("admission.row") record Row { @id("admission.field") field: i64, }
@id("admission.box") record Box<T> { @id("admission.box.field") field: T, }
@id("admission.main") fn main() -> i64 { 0 }
"#,
            std::path::Path::new("type-admission.spx"),
        )
        .unwrap();
        crate::hir::resolve(&source).unwrap()
    }
    fn verdict(result: Result<(), Diagnostic>) -> Result<(), (String, String)> {
        result.map_err(|error| (error.code.to_string(), error.message.into_owned()))
    }
    fn nominal(id: &str, arguments: Vec<ResolvedType>) -> ResolvedType {
        ResolvedType::Nominal {
            declaration: DeclarationId::new(id),
            arguments,
        }
    }
    #[test]
    fn inline_and_lazy_type_admission_preserve_saved_eager_verdicts() {
        let mut program = fixture();
        let mut types = vec![
            ResolvedType::Never,
            ResolvedType::Unit,
            ResolvedType::I64,
            ResolvedType::I32,
            ResolvedType::Char,
            ResolvedType::U8,
            ResolvedType::Usize,
            ResolvedType::ArrayU8(17),
            ResolvedType::F32,
            ResolvedType::F64,
            ResolvedType::Bool,
            ResolvedType::String,
            ResolvedType::Bytes,
            ResolvedType::Str,
            ResolvedType::SliceU8,
            ResolvedType::StringMap,
            ResolvedType::OnceFunction,
            ResolvedType::OnceFunctionI64,
            ResolvedType::OnceFunctionI64Pair,
            ResolvedType::MutFunctionI64,
            ResolvedType::TypeParameter {
                owner: DeclarationId::new("admission.box"),
                index: 0,
            },
            nominal("missing", vec![]),
            nominal("admission.row", vec![]),
            nominal("admission.box", vec![]),
            nominal("admission.row", vec![ResolvedType::Bytes]),
            nominal("admission.box", vec![ResolvedType::Bytes]),
            nominal("admission.box", vec![ResolvedType::String]),
            nominal("admission.box", vec![ResolvedType::I64]),
            nominal("admission.box", vec![ResolvedType::Str]),
        ];
        types.push(ResolvedType::Function {
            parameters: vec![ResolvedType::I64],
            result: Box::new(ResolvedType::Bool),
        });
        types.push(ResolvedType::Function {
            parameters: vec![ResolvedType::Bytes],
            result: Box::new(ResolvedType::Bytes),
        });
        for cached in [false, true] {
            if cached {
                let mut facts = program.declarations.type_facts(&ResolvedType::I64).unwrap();
                facts.copy = false;
                facts.layout_key = "arbitrary hostile cached layout".repeat(32);
                program
                    .declarations
                    .type_facts_by_id
                    .insert("i64".into(), facts.clone());
                program
                    .declarations
                    .type_facts_by_id
                    .insert(nominal("admission.row", vec![]).identity_key(), facts);
            }
            let validator = HirValidator::new(&program).unwrap();
            let missing = validator
                .validate_type(&nominal("missing", vec![]))
                .unwrap_err();
            assert_eq!(missing.code, "SPX-H006");
            assert_eq!(
                missing.message,
                "nominal type `missing` is not a resolved type declaration"
            );
            let arity = validator
                .validate_type(&nominal("admission.box", vec![]))
                .unwrap_err();
            assert_eq!(arity.code, "SPX-H006");
            assert_eq!(
                arity.message,
                "nominal type `admission.box` has incorrect argument arity"
            );
            for ty in &types {
                assert_eq!(
                    verdict(validator.validate_type(ty)),
                    verdict(validator.validate_type_eager_oracle(ty))
                );
                for ownership in [
                    OwnershipMode::Value,
                    OwnershipMode::Own,
                    OwnershipMode::Borrow,
                    OwnershipMode::Shared,
                ] {
                    assert_eq!(
                        verdict(validator.validate_declared_ownership(ty, ownership)),
                        verdict(
                            validator.validate_declared_ownership_full_facts_oracle(ty, ownership)
                        )
                    );
                }
            }
        }
    }
    #[test]
    fn inline_type_admission_allocations_are_literal_and_drop_settles() {
        let program = fixture();
        let validator = HirValidator::new(&program).unwrap();
        let scalar = ResolvedType::I64;
        let (_, scalar_metrics) = crate::test_allocation_observer::measure(|| {
            validator.validate_type(&scalar).unwrap();
            validator
                .validate_declared_ownership(&scalar, OwnershipMode::Value)
                .unwrap();
        });
        assert!(!scalar_metrics.invalid);
        assert_eq!(
            (
                scalar_metrics.allocations,
                scalar_metrics.allocated,
                scalar_metrics.live
            ),
            (0, 0, 0)
        );
        let nominal = nominal("admission.row", vec![]);
        // The memo belongs to its validator. Observe acquisition through its
        // actual Drop on both routes so retained entries cannot escape the
        // settlement measurement.
        let (old, old_metrics) = crate::test_allocation_observer::measure(|| {
            let local = HirValidator::new(&program).unwrap();
            let result = local.validate_type_eager_oracle(&nominal);
            drop(local);
            result
        });
        let (actual, metrics) = crate::test_allocation_observer::measure(|| {
            let local = HirValidator::new(&program).unwrap();
            let result = local.validate_type(&nominal);
            drop(local);
            result
        });
        assert!(old.is_ok() && actual.is_ok());
        for m in [old_metrics, metrics] {
            assert!(!m.invalid);
            assert_eq!(m.live, 0);
            assert_eq!(m.allocated, m.freed);
        }
        assert!(metrics.allocations < old_metrics.allocations);
        assert!(metrics.allocated < old_metrics.allocated);
    }
}
