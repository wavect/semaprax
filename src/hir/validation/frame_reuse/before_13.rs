// Independently retained immediate previous local definition.
#[rustfmt::skip]
macro_rules! before_definition_13 {
    ($Frame:ident, $frame_owned_capacity:ident, $enter_phase_0:ident, $enter_phase_4:ident, $enter_phase_5:ident, $enter_phase_1:ident, $enter_phase_2:ident, $enter_phase_3:ident, $frame_phase_0:ident, $frame_phase_8:ident, $frame_phase_9:ident, $frame_phase_1:ident, $frame_phase_2:ident, $frame_phase_3:ident, $frame_phase_4:ident, $frame_phase_5:ident, $frame_phase_6:ident, $frame_phase_7:ident) => {
        #[inline(never)]
        fn $frame_phase_3<'a: 'e, 'e>(
            validator: &mut HirValidator<'a>,
            frame: $Frame<'e>,
            frames: &mut scope_checkpoint::Storage<$Frame<'e>>,
            scopes: &mut scope_checkpoint::Storage<scope_checkpoint::Scope>,
            publication: &mut scope_checkpoint::Publication<'_>,
            allow_moves: bool,
        ) -> Result<(), Diagnostic> {
            let program = validator.program;
            match frame {
                $Frame::RecordNext {
                    expression,
                    fields,
                    expected,
                    record,
                    arguments,
                    seen,
                    index,
                    scope,
                    path,
                } => {
                    if index == fields.len() {
                        if seen.len() != expected.len() {
                            return Err(scope_checkpoint::paths::diagnostic(format_args!(
                                "constructor for `{record}` is missing required fields"
                            )));
                        }
                        let ownership =
                            validator.expected_ownership(&expression.ty, OwnershipMode::Own)?;
                        validator.finish_expr(expression, &expression.ty, ownership)?;
                        scopes.push(scope)?;
                    } else {
                        let initializer = &fields[index];
                        let mut seen = seen;
                        if !expected.iter().any(|field| field.id == initializer.field) {
                            return Err(scope_checkpoint::paths::diagnostic(format_args!(
                                "constructor for `{record}` contains foreign field `{}`",
                                initializer.field
                            )));
                        }
                        if !seen.insert(initializer.field.clone()) {
                            return Err(scope_checkpoint::paths::diagnostic(format_args!(
                                "constructor for `{record}` repeats field `{}`",
                                initializer.field
                            )));
                        }
                        frames.push($Frame::RecordAfterField {
                            expression,
                            fields,
                            expected,
                            record,
                            arguments,
                            seen,
                            index,
                            path,
                        })?;
                        frames.push($Frame::Enter {
                            expression: &initializer.value,
                            scope,
                            path: path_label::Label::child(path, ".field.{}.value", [index, 0])?,
                        })?;
                    }
                }
                $Frame::RecordAfterField {
                    expression,
                    fields,
                    expected,
                    record,
                    arguments,
                    seen,
                    index,
                    path,
                } => {
                    let mut scope = scopes.pop().expect("record field scope retained");
                    publication.publish(&scope)?;
                    let initializer = &fields[index];
                    let declared = expected
                        .iter()
                        .find(|field| field.id == initializer.field)
                        .expect("field authenticated before child");
                    let field_ty = substitute_type(&declared.ty, record, arguments)?;
                    validator.require_type(&initializer.value.ty, &field_ty, "record field")?;
                    let ownership = validator.expected_ownership(&field_ty, OwnershipMode::Own)?;
                    if initializer.value.ownership != ownership {
                        return Err(scope_checkpoint::paths::diagnostic(format_args!(
                            "field `{}` has incompatible ownership",
                            initializer.field
                        )));
                    }
                    if ownership == OwnershipMode::Own {
                        if !allow_moves {
                            return Err(hir_error(
                                "contract cannot transfer ownership into a record",
                            ));
                        }
                        validator.mark_value_sources_moved(&initializer.value, &mut scope)?;
                        publication.publish(&scope)?;
                    }
                    frames.push($Frame::RecordNext {
                        expression,
                        fields,
                        expected,
                        record,
                        arguments,
                        seen,
                        index: index + 1,
                        scope,
                        path,
                    })?;
                }
                $Frame::VariantNext {
                    expression,
                    fields,
                    expected,
                    variant,
                    case,
                    arguments,
                    seen,
                    index,
                    scope,
                    path,
                } => {
                    if index == fields.len() {
                        if seen.len() != expected.len() {
                            return Err(scope_checkpoint::paths::diagnostic(format_args!(
                                "constructor for `{case}` is missing required payload fields"
                            )));
                        }
                        let ownership =
                            validator.expected_ownership(&expression.ty, OwnershipMode::Own)?;
                        validator.finish_expr(expression, &expression.ty, ownership)?;
                        scopes.push(scope)?;
                    } else {
                        let initializer = &fields[index];
                        let mut seen = seen;
                        if !expected.iter().any(|field| field.id == initializer.field) {
                            return Err(scope_checkpoint::paths::diagnostic(format_args!(
                                "constructor for `{case}` contains foreign field `{}`",
                                initializer.field
                            )));
                        }
                        if !seen.insert(initializer.field.clone()) {
                            return Err(scope_checkpoint::paths::diagnostic(format_args!(
                                "constructor for `{case}` repeats field `{}`",
                                initializer.field
                            )));
                        }
                        frames.push($Frame::VariantAfterField {
                            expression,
                            fields,
                            expected,
                            variant,
                            case,
                            arguments,
                            seen,
                            index,
                            path,
                        })?;
                        frames.push($Frame::Enter {
                            expression: &initializer.value,
                            scope,
                            path: path_label::Label::child(path, ".field.{}.value", [index, 0])?,
                        })?;
                    }
                }
                $Frame::VariantAfterField {
                    expression,
                    fields,
                    expected,
                    variant,
                    case,
                    arguments,
                    seen,
                    index,
                    path,
                } => {
                    let mut scope = scopes.pop().expect("variant field scope retained");
                    publication.publish(&scope)?;
                    let initializer = &fields[index];
                    let declared = expected
                        .iter()
                        .find(|field| field.id == initializer.field)
                        .expect("variant field authenticated before child");
                    let field_ty = substitute_type(&declared.ty, variant, arguments)?;
                    validator.require_type(
                        &initializer.value.ty,
                        &field_ty,
                        "variant payload field",
                    )?;
                    let ownership = validator.expected_ownership(&field_ty, OwnershipMode::Own)?;
                    if initializer.value.ownership != ownership {
                        return Err(scope_checkpoint::paths::diagnostic(format_args!(
                            "variant payload field `{}` has incompatible ownership",
                            initializer.field
                        )));
                    }
                    if ownership == OwnershipMode::Own {
                        if !allow_moves {
                            return Err(hir_error(
                                "contract cannot transfer ownership into a variant",
                            ));
                        }
                        validator.mark_value_sources_moved(&initializer.value, &mut scope)?;
                        publication.publish(&scope)?;
                    }
                    frames.push($Frame::VariantNext {
                        expression,
                        fields,
                        expected,
                        variant,
                        case,
                        arguments,
                        seen,
                        index: index + 1,
                        scope,
                        path,
                    })?;
                }
                $Frame::UpdateBase {
                    expression,
                    record,
                    fields,
                    path,
                } => {
                    let mut scope = scopes.pop().expect("update base scope retained");
                    publication.publish(&scope)?;
                    let ResolvedExprKind::UpdateRecord { base, .. } = &expression.kind else {
                        unreachable!()
                    };
                    let declaration = validator
                        .program
                        .declarations
                        .declaration(record)
                        .ok_or_else(|| {
                            scope_checkpoint::paths::diagnostic(format_args!(
                                "record `{record}` is not indexed"
                            ))
                        })?;
                    if declaration.kind != DeclarationKind::Record {
                        return Err(scope_checkpoint::paths::diagnostic(format_args!(
                            "record update target `{record}` is not a record"
                        )));
                    }
                    let ResolvedType::Nominal {
                        declaration: instance,
                        arguments,
                    } = &base.ty
                    else {
                        return Err(hir_error("record update base is not nominal"));
                    };
                    let parameters = validator
                        .program
                        .declarations
                        .type_parameters(record)
                        .ok_or_else(|| {
                            scope_checkpoint::paths::diagnostic(format_args!(
                                "record `{record}` has no parameters"
                            ))
                        })?;
                    if instance != record
                        || arguments.len() != parameters.len()
                        || !crate::hir::type_reachability::record_args_ok(
                            &validator.program.declarations,
                            record,
                            arguments,
                        )
                    {
                        return Err(scope_checkpoint::paths::diagnostic(format_args!(
                            "record update for `{record}` has an invalid concrete instance"
                        )));
                    }
                    validate_nested_update_base_shape(validator.program, base, expression.span)?;
                    let ownership = validator.expected_ownership(&base.ty, OwnershipMode::Own)?;
                    if base.ownership != ownership {
                        return Err(scope_checkpoint::paths::diagnostic(format_args!(
                            "record update base for `{record}` has incompatible ownership"
                        )));
                    }
                    if ownership == OwnershipMode::Own {
                        if !allow_moves {
                            return Err(hir_error(
                                "contract cannot transfer ownership from a record update base",
                            ));
                        }
                        validator.mark_value_sources_moved(base, &mut scope)?;
                        publication.publish(&scope)?;
                    }
                    let expected =
                        aggregate_metadata::record_fields(program, record).ok_or_else(|| {
                            scope_checkpoint::paths::diagnostic(format_args!(
                                "record `{record}` has no fields"
                            ))
                        })?;
                    frames.push($Frame::UpdateNext {
                        expression,
                        fields,
                        expected,
                        record,
                        arguments,
                        seen: BTreeSet::new(),
                        index: 0,
                        scope,
                        path,
                        ownership,
                    })?;
                }
                $Frame::UpdateNext {
                    expression,
                    fields,
                    expected,
                    record,
                    arguments,
                    seen,
                    index,
                    scope,
                    path,
                    ownership,
                } => {
                    if index == fields.len() {
                        let ResolvedExprKind::UpdateRecord { base, .. } = &expression.kind else {
                            unreachable!()
                        };
                        validator.finish_expr(expression, &base.ty, ownership)?;
                        scopes.push(scope)?;
                    } else {
                        let initializer = &fields[index];
                        let mut seen = seen;
                        if !expected.iter().any(|field| field.id == initializer.field) {
                            return Err(scope_checkpoint::paths::diagnostic(format_args!(
                                "update for `{record}` contains foreign field `{}`",
                                initializer.field
                            )));
                        }
                        if !seen.insert(initializer.field.clone()) {
                            return Err(scope_checkpoint::paths::diagnostic(format_args!(
                                "update for `{record}` repeats field `{}`",
                                initializer.field
                            )));
                        }
                        frames.push($Frame::UpdateAfterField {
                            expression,
                            fields,
                            expected,
                            record,
                            arguments,
                            seen,
                            index,
                            path,
                            ownership,
                        })?;
                        frames.push($Frame::Enter {
                            expression: &initializer.value,
                            scope,
                            path: path_label::Label::child(path, ".field.{}.value", [index, 0])?,
                        })?;
                    }
                }
                $Frame::UpdateAfterField {
                    expression,
                    fields,
                    expected,
                    record,
                    arguments,
                    seen,
                    index,
                    path,
                    ownership,
                } => {
                    let mut scope = scopes.pop().expect("update field scope retained");
                    publication.publish(&scope)?;
                    let initializer = &fields[index];
                    let declared = expected
                        .iter()
                        .find(|field| field.id == initializer.field)
                        .expect("update field authenticated before child");
                    let field_ty = substitute_type(&declared.ty, record, arguments)?;
                    validator.require_type(
                        &initializer.value.ty,
                        &field_ty,
                        "record replacement",
                    )?;
                    let expected_ownership =
                        validator.expected_ownership(&field_ty, OwnershipMode::Own)?;
                    if initializer.value.ownership != expected_ownership {
                        return Err(scope_checkpoint::paths::diagnostic(format_args!(
                            "replacement field `{}` has incompatible ownership",
                            initializer.field
                        )));
                    }
                    if expected_ownership == OwnershipMode::Own {
                        if !allow_moves {
                            return Err(hir_error(
                                "contract cannot transfer ownership into a record replacement",
                            ));
                        }
                        validator.mark_value_sources_moved(&initializer.value, &mut scope)?;
                        publication.publish(&scope)?;
                    }
                    frames.push($Frame::UpdateNext {
                        expression,
                        fields,
                        expected,
                        record,
                        arguments,
                        seen,
                        index: index + 1,
                        scope,
                        path,
                        ownership,
                    })?;
                }
                _ => unreachable!("frame was routed to its matching phase"),
            }
            Ok(())
        }
    };
}
