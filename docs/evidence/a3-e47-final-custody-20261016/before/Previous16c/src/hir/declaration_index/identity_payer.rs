//! Explicit identity payer for initial declaration-index construction.
//! Semantic diagnostics retain their original owned compatibility contract.
//! Identity refusal returns the exact caller error, without rendering a new
//! Diagnostic, allocating a report, or consulting an ambient ledger.
use super::nominal_construction::Payer;
use super::*;

#[derive(Debug)]
pub(in crate::hir) enum IdentityIndexError<E> {
    Semantic(Diagnostic),
    Identity(E),
}
impl<E> From<Diagnostic> for IdentityIndexError<E> {
    fn from(error: Diagnostic) -> Self {
        Self::Semantic(error)
    }
}

impl DeclarationIndex {
    /// The caller owns this reservation callback and its refusal carrier.
    /// Only bytes are requested from it; exact source text is constructed
    /// internally, so callback presence cannot substitute declaration IDs.
    #[cfg_attr(
        not(test),
        allow(
            dead_code,
            reason = "explicit index payer seam awaits caller-owned outward report migration"
        )
    )]
    pub(in crate::hir) fn from_verified_with_identity_reservation<E>(
        program: &dyn ProgramSource,
        prelude: &[crate::ast::TypeDeclaration],
        mut reserve: impl FnMut(usize) -> Result<(), E>,
    ) -> Result<Self, IdentityIndexError<E>> {
        Self::from_verified_with_identity_payer(program, prelude, &mut |text| {
            DeclarationId::paid_borrowed(text, &mut reserve)
        })
    }

    /// Same explicit owner, now also acquiring the canonical nominal owners.
    #[cfg(test)]
    pub(in crate::hir) fn from_verified_with_nominal_reservation<E>(
        program: &dyn ProgramSource,
        prelude: &[crate::ast::TypeDeclaration],
        reserve: impl FnMut(usize) -> Result<(), E>,
    ) -> Result<Self, IdentityIndexError<E>> {
        Self::from_verified_with_nominal_base(program, prelude, None, reserve)
    }

    #[cfg(test)]
    pub(in crate::hir) fn from_verified_with_nominal_base<E>(
        program: &dyn ProgramSource,
        prelude: &[crate::ast::TypeDeclaration],
        base: Option<&super::super::prelude_bundle::BasePreludeBundle>,
        reserve: impl FnMut(usize) -> Result<(), E>,
    ) -> Result<Self, IdentityIndexError<E>> {
        Self::from_verified_with_nominal_extensions(program, prelude, base, None, reserve)
    }

    pub(in crate::hir) fn from_verified_with_nominal_extensions<E>(
        program: &dyn ProgramSource,
        prelude: &[crate::ast::TypeDeclaration],
        base: Option<&super::super::prelude_bundle::BasePreludeBundle>,
        vector: Option<&super::super::prelude_bundle::VectorExtension>,
        mut reserve: impl FnMut(usize) -> Result<(), E>,
    ) -> Result<Self, IdentityIndexError<E>> {
        if let Some(base) = base {
            Self::reserve_base_receivers(base, &mut reserve)
                .map_err(IdentityIndexError::Identity)?;
        }
        let workspace = super::nominal_rollback::Workspace::acquire(program, prelude, &mut reserve)
            .map_err(IdentityIndexError::Identity)?;
        reserve(
            2 * std::mem::size_of::<Payer<'_, E>>()
                + 2 * std::mem::size_of::<Self>()
                + std::mem::size_of::<Result<Self, IdentityIndexError<E>>>(),
        )
        .map_err(IdentityIndexError::Identity)?;
        let mut index = Self::build_nominal_index(
            program,
            prelude,
            &mut Payer::Funded(&mut reserve),
            true,
            workspace,
            base,
            vector,
        )?;
        index
            .share_nominal_payloads(&mut reserve)
            .map_err(IdentityIndexError::Identity)?;
        index.nominal_drop_workspace.finish();
        Ok(index)
    }

    // Borrow the callback on the stack so this complete constructor specializes
    // by its refusal carrier, rather than every caller closure type. No callback
    // escapes and reservation order and owned identity construction stay exact.
    pub(super) fn from_verified_with_identity_payer<E>(
        program: &dyn ProgramSource,
        prelude: &[crate::ast::TypeDeclaration],
        identity: &mut dyn FnMut(&str) -> Result<DeclarationId, E>,
    ) -> Result<Self, IdentityIndexError<E>> {
        let mut index = Self::build_nominal_index(
            program,
            prelude,
            &mut Payer::Legacy(identity),
            false,
            super::nominal_rollback::Workspace::default(),
            None,
            None,
        )?;
        index.share_legacy_nominal_payloads_unfunded();
        Ok(index)
    }

    fn build_nominal_index<E>(
        program: &dyn ProgramSource,
        prelude: &[crate::ast::TypeDeclaration],
        payer: &mut Payer<'_, E>,
        canonical_nominals: bool,
        workspace: super::nominal_rollback::Workspace,
        base: Option<&super::super::prelude_bundle::BasePreludeBundle>,
        vector: Option<&super::super::prelude_bundle::VectorExtension>,
    ) -> Result<Self, IdentityIndexError<E>> {
        let mut index = Self::default();
        index.nominal_drop_workspace = workspace;
        for declaration in program.types().iter().chain(prelude) {
            if let Some(ty) = Self::compiler_type_for_source(base, vector, prelude, declaration) {
                let kind = match &ty.kind {
                    ResolvedTypeDeclarationKind::Variant { .. } => DeclarationKind::Variant,
                    ResolvedTypeDeclarationKind::Record { .. } => DeclarationKind::Record,
                    _ => unreachable!("closed compiler payload kind"),
                };
                if super::super::prelude_bundle::VectorExtension::for_source(
                    vector,
                    prelude,
                    declaration,
                )
                .is_some()
                {
                    payer
                        .node::<DeclarationId, NominalPayload<ResolvedTypeParameterDeclaration>>()
                        .map_err(IdentityIndexError::Identity)?;
                    payer.bytes(2 * std::mem::size_of::<NominalPayload<ResolvedTypeParameterDeclaration>>()
                        + std::mem::size_of::<Option<NominalPayload<ResolvedTypeParameterDeclaration>>>())
                        .map_err(IdentityIndexError::Identity)?;
                }
                payer
                    .top(&ty.name, kind)
                    .map_err(IdentityIndexError::Identity)?;
                index.insert_top_level(
                    payer.text(&ty.name).map_err(IdentityIndexError::Identity)?,
                    ty.id.clone(),
                    kind,
                    IdentityOrigin::CompilerOwned,
                );
                index
                    .type_parameters
                    .insert(ty.id.clone(), ty.type_parameters.clone());
                continue;
            }
            let kind = match declaration.kind {
                TypeDeclarationKind::Resource { .. } => DeclarationKind::Resource,
                TypeDeclarationKind::Record { .. } => DeclarationKind::Record,
                TypeDeclarationKind::Class { .. } => DeclarationKind::Class,
                TypeDeclarationKind::Variant { .. } => DeclarationKind::Variant,
            };
            payer
                .top(&declaration.name, kind)
                .map_err(IdentityIndexError::Identity)?;
            index.insert_top_level(
                payer
                    .text(&declaration.name)
                    .map_err(IdentityIndexError::Identity)?,
                payer
                    .identity(&declaration.stable_id)
                    .map_err(IdentityIndexError::Identity)?,
                kind,
                if crate::prelude::is_compiler_owned_id(&declaration.stable_id) {
                    IdentityOrigin::CompilerOwned
                } else if declaration.explicit_id {
                    IdentityOrigin::Explicit
                } else {
                    IdentityOrigin::Automatic
                },
            );
            let owner = index.inserted_identity(&declaration.stable_id);
            let mut parameters = payer
                .vec::<ResolvedTypeParameterDeclaration>(declaration.type_parameters.len())
                .map_err(IdentityIndexError::Identity)?;
            for (ordinal, parameter) in declaration.type_parameters.iter().enumerate() {
                parameters.push(ResolvedTypeParameterDeclaration {
                    name: payer
                        .text(&parameter.name)
                        .map_err(IdentityIndexError::Identity)?,
                    index: u32::try_from(ordinal).map_err(|_| {
                        payer.semantic(
                            "SPX-H006",
                            format_args!("type `{}` has too many parameters", declaration.name),
                            declaration.span,
                            program.path(),
                            true,
                        )
                    })?,
                    span: parameter.span,
                });
            }
            index.type_parameters.insert(owner, parameters);
        }
        for interface in program.interfaces() {
            let interface_id = payer
                .identity(&interface.stable_id)
                .map_err(IdentityIndexError::Identity)?;
            payer
                .top(&interface.name, DeclarationKind::Interface)
                .map_err(IdentityIndexError::Identity)?;
            index.insert_top_level(
                payer
                    .text(&interface.name)
                    .map_err(IdentityIndexError::Identity)?,
                interface_id.clone(),
                DeclarationKind::Interface,
                IdentityOrigin::Explicit,
            );
            for import in &interface.imports {
                let import_id = payer
                    .identity(&import.stable_id)
                    .map_err(IdentityIndexError::Identity)?;
                payer
                    .node::<String, DeclarationId>()
                    .map_err(IdentityIndexError::Identity)?;
                index.imports_by_key.insert(
                    payer
                        .text(&import.stable_id)
                        .map_err(IdentityIndexError::Identity)?,
                    import_id.clone(),
                );
                if import.native_rust {
                    payer
                        .node::<String, DeclarationId>()
                        .map_err(IdentityIndexError::Identity)?;
                    index.native_rust_imports_by_name.insert(
                        payer
                            .text(&import.name)
                            .map_err(IdentityIndexError::Identity)?,
                        import_id.clone(),
                    );
                }
                payer.owned().map_err(IdentityIndexError::Identity)?;
                index.insert_owned_declaration(
                    interface_id.clone(),
                    payer
                        .text(&import.name)
                        .map_err(IdentityIndexError::Identity)?,
                    import_id,
                    DeclarationKind::Import,
                    IdentityOrigin::Explicit,
                );
            }
        }
        for declaration in program.types().iter().chain(prelude) {
            let TypeDeclarationKind::Resource { lifecycles } = &declaration.kind else {
                continue;
            };
            if let [lifecycle] = lifecycles.as_slice() {
                let lifecycle_id = lifecycle
                    .stable_id
                    .as_ref()
                    .expect("verified lifecycle has an explicit identity");
                payer.owned().map_err(IdentityIndexError::Identity)?;
                index.insert_owned_declaration(
                    index.inserted_identity(&declaration.stable_id),
                    payer.text("drop").map_err(IdentityIndexError::Identity)?,
                    payer
                        .identity(lifecycle_id)
                        .map_err(IdentityIndexError::Identity)?,
                    DeclarationKind::ResourceDrop,
                    IdentityOrigin::Explicit,
                );
            }
        }
        for function in program.functions() {
            let owner = payer
                .identity(&function.stable_id)
                .map_err(IdentityIndexError::Identity)?;
            payer
                .top(&function.name, DeclarationKind::Function)
                .map_err(IdentityIndexError::Identity)?;
            index.insert_top_level(
                payer
                    .text(&function.name)
                    .map_err(IdentityIndexError::Identity)?,
                owner.clone(),
                DeclarationKind::Function,
                if super::record_invariants::is_synthesized(&function.name) {
                    IdentityOrigin::CompilerOwned
                } else if function.explicit_id {
                    IdentityOrigin::Explicit
                } else {
                    IdentityOrigin::Automatic
                },
            );
            let mut parameters = payer
                .vec::<ResolvedTypeParameterDeclaration>(function.type_parameters.len())
                .map_err(IdentityIndexError::Identity)?;
            for (ordinal, parameter) in function.type_parameters.iter().enumerate() {
                parameters.push(ResolvedTypeParameterDeclaration {
                    name: payer
                        .text(&parameter.name)
                        .map_err(IdentityIndexError::Identity)?,
                    index: u32::try_from(ordinal).map_err(|_| {
                        payer.semantic(
                            "SPX-H006",
                            format_args!("function `{}` has too many parameters", function.name),
                            function.span,
                            program.path(),
                            true,
                        )
                    })?,
                    span: parameter.span,
                });
            }
            index.type_parameters.insert(owner, parameters);
        }
        if program
            .interfaces()
            .iter()
            .flat_map(|interface| &interface.imports)
            .filter(|import| import.native_rust)
            .any(|import| index.functions_by_name.contains_key(&import.name))
        {
            return Err(payer.semantic(
                "SPX-B107",
                format_args!(
                    "{}",
                    "Native Rust Interop declaration set is unsupported: symbol collision"
                ),
                Span::default(),
                program.path(),
                false,
            ));
        }
        for declaration in program.types().iter().chain(prelude) {
            let (TypeDeclarationKind::Record { fields }
            | TypeDeclarationKind::Class { fields, .. }) = &declaration.kind
            else {
                continue;
            };
            let owner = index.inserted_identity(&declaration.stable_id);
            if let Some(ty) = Self::compiler_type_for_source(base, vector, prelude, declaration) {
                if let ResolvedTypeDeclarationKind::Record { fields } = &ty.kind {
                    payer
                        .node::<DeclarationId, NominalPayload<ResolvedFieldDeclaration>>()
                        .map_err(IdentityIndexError::Identity)?;
                    payer
                        .bytes(
                            2 * std::mem::size_of::<NominalPayload<ResolvedFieldDeclaration>>()
                                + std::mem::size_of::<
                                    Option<NominalPayload<ResolvedFieldDeclaration>>,
                                >(),
                        )
                        .map_err(IdentityIndexError::Identity)?;
                    index.record_fields.insert(owner, fields.clone());
                    continue;
                }
            }
            payer
                .vector::<ResolvedFieldDeclaration>(fields.len())
                .map_err(IdentityIndexError::Identity)?;
            let mut resolved_fields = index
                .nominal_drop_workspace
                .fields::<ResolvedFieldDeclaration>(fields.len());
            for (ordinal, field) in fields.iter().enumerate() {
                let ty = payer
                    .resolve_type(&index, &field.ty, Some(&owner))
                    .map_err(IdentityIndexError::Identity)?
                    .ok_or_else(|| {
                        payer.semantic(
                            "SPX-H001",
                            format_args!("unresolved field type `{}`", field.ty),
                            field.span,
                            program.path(),
                            true,
                        )
                    })?;
                let field_index = u32::try_from(ordinal).map_err(|_| {
                    payer.semantic(
                        "SPX-H006",
                        format_args!("record `{}` has too many fields", declaration.name),
                        declaration.span,
                        program.path(),
                        true,
                    )
                })?;
                let ty = index.nominal_drop_workspace.resolved_type(ty);
                let resolved = ResolvedFieldDeclaration {
                    id: payer
                        .identity(&field.stable_id)
                        .map_err(IdentityIndexError::Identity)?,
                    name: payer
                        .text(&field.name)
                        .map_err(IdentityIndexError::Identity)?,
                    index: field_index,
                    ty: ty.into_type(),
                    span: field.span,
                };
                resolved_fields.push(resolved);
                let resolved = resolved_fields.last().expect("guarded constructed field");
                payer
                    .field(&resolved.name)
                    .map_err(IdentityIndexError::Identity)?;
                index.insert_field_view(
                    owner.clone(),
                    resolved,
                    if field.explicit_id {
                        IdentityOrigin::Explicit
                    } else {
                        IdentityOrigin::Automatic
                    },
                );
            }
            index
                .record_fields
                .insert(owner, resolved_fields.into_vec());
        }
        for declaration in program.types().iter() {
            let TypeDeclarationKind::Class { fields, methods } = &declaration.kind else {
                continue;
            };
            let owner = index.inserted_identity(&declaration.stable_id);
            payer
                .vector::<ResolvedFieldDeclaration>(fields.len())
                .map_err(IdentityIndexError::Identity)?;
            let mut resolved_fields = index
                .nominal_drop_workspace
                .fields::<ResolvedFieldDeclaration>(fields.len());
            for (ordinal, field) in fields.iter().enumerate() {
                let ty = payer
                    .resolve_type(&index, &field.ty, Some(&owner))
                    .map_err(IdentityIndexError::Identity)?
                    .ok_or_else(|| {
                        payer.semantic(
                            "SPX-H001",
                            format_args!("unresolved class field type `{}`", field.ty),
                            field.span,
                            program.path(),
                            true,
                        )
                    })?;
                let field_index = u32::try_from(ordinal).map_err(|_| {
                    payer.semantic(
                        "SPX-H006",
                        format_args!("class `{}` has too many fields", declaration.name),
                        declaration.span,
                        program.path(),
                        true,
                    )
                })?;
                let ty = index.nominal_drop_workspace.resolved_type(ty);
                let resolved = ResolvedFieldDeclaration {
                    id: payer
                        .identity(&field.stable_id)
                        .map_err(IdentityIndexError::Identity)?,
                    name: payer
                        .text(&field.name)
                        .map_err(IdentityIndexError::Identity)?,
                    index: field_index,
                    ty: ty.into_type(),
                    span: field.span,
                };
                resolved_fields.push(resolved);
                let resolved = resolved_fields.last().expect("guarded constructed field");
                payer
                    .field(&resolved.name)
                    .map_err(IdentityIndexError::Identity)?;
                index.insert_field_view(
                    owner.clone(),
                    resolved,
                    if field.explicit_id {
                        IdentityOrigin::Explicit
                    } else {
                        IdentityOrigin::Automatic
                    },
                );
            }
            index
                .record_fields
                .insert(owner.clone(), resolved_fields.into_vec());
            for method in methods {
                let method_id = payer
                    .identity(&method.stable_id)
                    .map_err(IdentityIndexError::Identity)?;
                if index.declarations.contains_key(&method_id) {
                    return Err(payer.semantic(
                        "SPX-S102",
                        format_args!("duplicate stable id `{}`", method.stable_id),
                        method.span,
                        program.path(),
                        true,
                    ));
                }
                payer.owned().map_err(IdentityIndexError::Identity)?;
                index.insert_owned_declaration(
                    owner.clone(),
                    payer
                        .text(&method.name)
                        .map_err(IdentityIndexError::Identity)?,
                    method_id,
                    DeclarationKind::Function,
                    if method.explicit_id {
                        IdentityOrigin::Explicit
                    } else {
                        IdentityOrigin::Automatic
                    },
                );
                if !method.type_parameters.is_empty() {
                    return Err(payer.semantic(
                        "SPX-T224",
                        format_args!(
                            "class method `{}` cannot declare generic parameters in this slice",
                            method.name
                        ),
                        method.span,
                        program.path(),
                        true,
                    ));
                }
                index
                    .type_parameters
                    .insert(index.inserted_identity(&method.stable_id), Vec::new());
            }
        }
        for declaration in program.types().iter().chain(prelude) {
            let TypeDeclarationKind::Variant { cases } = &declaration.kind else {
                continue;
            };
            if let Some(ty) = Self::compiler_type_for_source(base, vector, prelude, declaration) {
                if super::super::prelude_bundle::VectorExtension::for_source(
                    vector,
                    prelude,
                    declaration,
                )
                .is_some()
                {
                    let ResolvedTypeDeclarationKind::Variant { cases } = &ty.kind else {
                        unreachable!("compiler variant")
                    };
                    payer
                        .node::<DeclarationId, NominalPayload<ResolvedVariantCaseDeclaration>>()
                        .map_err(IdentityIndexError::Identity)?;
                    payer
                        .bytes(
                            2 * std::mem::size_of::<NominalPayload<ResolvedVariantCaseDeclaration>>(
                            ) + std::mem::size_of::<
                                Option<NominalPayload<ResolvedVariantCaseDeclaration>>,
                            >(),
                        )
                        .map_err(IdentityIndexError::Identity)?;
                    for case in cases {
                        payer
                            .case(&case.name)
                            .map_err(IdentityIndexError::Identity)?;
                        let name = payer
                            .text(&case.name)
                            .map_err(IdentityIndexError::Identity)?;
                        index.insert_case(
                            ty.id.clone(),
                            name,
                            case.id.clone(),
                            IdentityOrigin::CompilerOwned,
                        );
                        for field in &case.fields {
                            payer
                                .field(&field.name)
                                .map_err(IdentityIndexError::Identity)?;
                            index.insert_case_field_view(
                                case.id.clone(),
                                field,
                                IdentityOrigin::CompilerOwned,
                            );
                        }
                        // share_nominal_payloads pays the coordinate initializer;
                        // this acquisition owns its new receiving map backing.
                        payer
                            .node::<DeclarationId, CaseFields>()
                            .map_err(IdentityIndexError::Identity)?;
                    }
                    index.variant_cases.insert(ty.id.clone(), cases.clone());
                } else {
                    index.insert_base_variant(ty);
                }
                continue;
            }
            let owner = index.inserted_identity(&declaration.stable_id);
            payer
                .vector::<ResolvedVariantCaseDeclaration>(cases.len())
                .map_err(IdentityIndexError::Identity)?;
            let mut resolved_cases = index
                .nominal_drop_workspace
                .fields::<ResolvedVariantCaseDeclaration>(cases.len());
            for (case_ordinal, case) in cases.iter().enumerate() {
                let case_id = payer
                    .identity(&case.stable_id)
                    .map_err(IdentityIndexError::Identity)?;
                let case_index = u32::try_from(case_ordinal).map_err(|_| {
                    payer.semantic(
                        "SPX-H006",
                        format_args!("variant `{}` has too many cases", declaration.name),
                        declaration.span,
                        program.path(),
                        true,
                    )
                })?;
                payer
                    .case(&case.name)
                    .map_err(IdentityIndexError::Identity)?;
                index.insert_case(
                    owner.clone(),
                    payer
                        .text(&case.name)
                        .map_err(IdentityIndexError::Identity)?,
                    case_id.clone(),
                    if crate::prelude::is_compiler_owned_id(&case.stable_id) {
                        IdentityOrigin::CompilerOwned
                    } else if case.explicit_id {
                        IdentityOrigin::Explicit
                    } else {
                        IdentityOrigin::Automatic
                    },
                );
                payer
                    .vector::<ResolvedFieldDeclaration>(case.fields.len())
                    .map_err(IdentityIndexError::Identity)?;
                let mut resolved_fields = index
                    .nominal_drop_workspace
                    .fields::<ResolvedFieldDeclaration>(case.fields.len());
                for (field_ordinal, field) in case.fields.iter().enumerate() {
                    let ty = payer
                        .resolve_type(&index, &field.ty, Some(&owner))
                        .map_err(IdentityIndexError::Identity)?
                        .ok_or_else(|| {
                            payer.semantic(
                                "SPX-H001",
                                format_args!("unresolved case field type `{}`", field.ty),
                                field.span,
                                program.path(),
                                true,
                            )
                        })?;
                    let field_index = u32::try_from(field_ordinal).map_err(|_| {
                        payer.semantic(
                            "SPX-H006",
                            format_args!(
                                "variant case `{}::{}` has too many fields",
                                declaration.name, case.name
                            ),
                            case.span,
                            program.path(),
                            true,
                        )
                    })?;
                    let ty = index.nominal_drop_workspace.resolved_type(ty);
                    let resolved = ResolvedFieldDeclaration {
                        id: payer
                            .identity(&field.stable_id)
                            .map_err(IdentityIndexError::Identity)?,
                        name: payer
                            .text(&field.name)
                            .map_err(IdentityIndexError::Identity)?,
                        index: field_index,
                        ty: ty.into_type(),
                        span: field.span,
                    };
                    resolved_fields.push(resolved);
                    let resolved = resolved_fields.last().expect("guarded constructed field");
                    payer
                        .field(&resolved.name)
                        .map_err(IdentityIndexError::Identity)?;
                    index.insert_case_field_view(
                        case_id.clone(),
                        resolved,
                        if crate::prelude::is_compiler_owned_id(&field.stable_id) {
                            IdentityOrigin::CompilerOwned
                        } else if field.explicit_id {
                            IdentityOrigin::Explicit
                        } else {
                            IdentityOrigin::Automatic
                        },
                    );
                }
                if !canonical_nominals {
                    index
                        .case_fields
                        .insert(case_id.clone(), resolved_fields.clone());
                }
                resolved_cases.push(ResolvedVariantCaseDeclaration {
                    id: case_id,
                    name: payer
                        .text(&case.name)
                        .map_err(IdentityIndexError::Identity)?,
                    index: case_index,
                    fields: resolved_fields.into_vec(),
                    span: case.span,
                });
            }
            index.variant_cases.insert(owner, resolved_cases.into_vec());
        }
        if canonical_nominals {
            index.materialize_class_inheritance_paid(program, &mut |n| payer.bytes(n))?;
        } else {
            index.materialize_class_inheritance(program)?;
        }
        let facts = if canonical_nominals {
            index.populate_type_facts_paid(&mut |n| payer.bytes(n))?
        } else {
            index.populate_type_facts()
        };
        if !facts {
            if canonical_nominals {
                return Err(super::nominal_paid_values::diagnostic(
                    "SPX-T217",
                    format_args!(
                        "record declarations contain an illegal by-value recursive layout"
                    ),
                    Span::default(),
                    program.path(),
                    &mut |n| payer.bytes(n),
                )?
                .into());
            }
            return Err(IdentityIndexError::Semantic(
                Diagnostic::error(
                    "SPX-T217",
                    "record declarations contain an illegal by-value recursive layout",
                    Span::default(),
                )
                .at_path(program.path()),
            ));
        }
        Ok(index)
    }
}
