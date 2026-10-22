// Independently retained immediate previous local definition.
#[rustfmt::skip]
macro_rules! before_definition_6 {
    ($Frame:ident, $frame_owned_capacity:ident, $enter_phase_0:ident, $enter_phase_4:ident, $enter_phase_5:ident, $enter_phase_1:ident, $enter_phase_2:ident, $enter_phase_3:ident, $frame_phase_0:ident, $frame_phase_8:ident, $frame_phase_9:ident, $frame_phase_1:ident, $frame_phase_2:ident, $frame_phase_3:ident, $frame_phase_4:ident, $frame_phase_5:ident, $frame_phase_6:ident, $frame_phase_7:ident) => {
        #[inline(never)]
        fn $enter_phase_2<'a: 'e, 'e>(
            validator: &mut HirValidator<'a>,
            expression: &'e ResolvedExpr,
            scope: scope_checkpoint::Scope,
            path: &'e str,
            frames: &mut scope_checkpoint::Storage<$Frame<'e>>,
            allowed_effects: Option<effect_keys::Allowed<'_>>,
        ) -> Result<(), Diagnostic> {
            let program = validator.program;
            match &expression.kind {
                ResolvedExprKind::NativeRustImportCall(call) => {
                    if call.expression != expression.id {
                        return Err(hir_error(
                            "native Rust import call has a non-canonical expression identity",
                        ));
                    }
                    let retained_program = validator.program;
                    let import = retained_program
                        .interfaces
                        .iter()
                        .flat_map(|interface| &interface.imports)
                        .find(|import| import.id == call.import && import.native_rust)
                        .ok_or_else(|| {
                            hir_error("native Rust import call has an unknown target")
                        })?;
                    if import.parameters.len() != call.args.len()
                        || import.result.kind != call.result
                    {
                        return Err(hir_error(
                            "native Rust import call disagrees with its declaration",
                        ));
                    }
                    match allowed_effects {
                        Some(allowed)
                            if import
                                .effects
                                .iter()
                                .any(|effect| !allowed.contains(effect)) =>
                        {
                            return Err(hir_error(
                                "native Rust import call requires an undeclared effect",
                            ));
                        }
                        None if !import.effects.is_empty() => {
                            return Err(hir_error(
                                "contract calls an effectful native Rust import",
                            ));
                        }
                        _ => {}
                    }
                    let result = call.result.value_type(&validator.program.declarations)?;
                    frames.push($Frame::NativeNext {
                        expression,
                        args: &call.args,
                        params: import.parameters.as_slice(),
                        result,
                        index: 0,
                        scope,
                        path,
                    })?;
                }
                ResolvedExprKind::Binary { op, left, right } => {
                    frames.push($Frame::BinaryLeft {
                        expression,
                        op: *op,
                        right,
                        path,
                    })?;
                    frames.push($Frame::Enter {
                        expression: left,
                        scope,
                        path: path_label::Label::child(path, ".left", [0, 0])?,
                    })?;
                }
                ResolvedExprKind::If {
                    condition,
                    then_branch,
                    else_branch,
                } => {
                    frames.push($Frame::IfCondition {
                        expression,
                        then_branch,
                        else_branch,
                        path,
                    })?;
                    frames.push($Frame::Enter {
                        expression: condition,
                        scope,
                        path: path_label::Label::child(path, ".condition", [0, 0])?,
                    })?;
                }
                ResolvedExprKind::Block { statements, tail }
                | ResolvedExprKind::VecConstructor {
                    statements, tail, ..
                } => {
                    crate::hir::vec_constructor::authenticate(expression)?;
                    validator.validate_record_traversal(expression, &scope)?;
                    let outer_ids = scope.ids();
                    let outer = scope.clone();
                    frames.push($Frame::BlockNext {
                        expression,
                        statements,
                        tail,
                        index: 0,
                        scope,
                        outer,
                        outer_ids,
                        path,
                    })?;
                }
                ResolvedExprKind::ConstructRecord { record, fields } => {
                    if crate::map_ops::is_declaration(record.as_str())
                        || record.as_str() == crate::iterator_ops::ITER_ID
                        || record.as_str() == crate::list_ops::LIST_ID
                    {
                        return Err(hir_error("iterator owner cannot be authored as a record"));
                    }
                    let declaration = validator
                        .program
                        .declarations
                        .declaration(record)
                        .ok_or_else(|| {
                            scope_checkpoint::paths::diagnostic(format_args!(
                                "record `{record}` is not indexed"
                            ))
                        })?;
                    if !matches!(
                        declaration.kind,
                        DeclarationKind::Record | DeclarationKind::Class
                    ) {
                        return Err(scope_checkpoint::paths::diagnostic(format_args!(
                            "constructor target `{record}` is not a record or class"
                        )));
                    }
                    let expected =
                        aggregate_metadata::record_fields(program, record).ok_or_else(|| {
                            scope_checkpoint::paths::diagnostic(format_args!(
                                "record `{record}` has no fields"
                            ))
                        })?;
                    let ResolvedType::Nominal {
                        declaration: instance,
                        arguments,
                    } = &expression.ty
                    else {
                        return Err(hir_error("record constructor result is not nominal"));
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
                            "constructor for `{record}` has an invalid concrete instance"
                        )));
                    }
                    frames.push($Frame::RecordNext {
                        expression,
                        fields,
                        expected,
                        record,
                        arguments,
                        seen: BTreeSet::new(),
                        index: 0,
                        scope,
                        path,
                    })?;
                }
                ResolvedExprKind::ConstructVariant {
                    variant,
                    case,
                    fields,
                } => {
                    if expression.ty.is_compiler_byte_option() {
                        return Err(hir_error(
                            "Option<u8> may be produced only by compiler-owned byte_get",
                        ));
                    }
                    let ResolvedType::Nominal {
                        declaration: instance,
                        arguments,
                    } = &expression.ty
                    else {
                        return Err(hir_error("variant constructor has a non-nominal result"));
                    };
                    if instance != variant {
                        return Err(hir_error(
                            "variant constructor result disagrees with its declaration",
                        ));
                    }
                    let declaration = validator
                        .program
                        .declarations
                        .declaration(variant)
                        .ok_or_else(|| {
                            scope_checkpoint::paths::diagnostic(format_args!(
                                "variant `{variant}` is not indexed"
                            ))
                        })?;
                    if declaration.kind != DeclarationKind::Variant {
                        return Err(scope_checkpoint::paths::diagnostic(format_args!(
                            "constructor target `{variant}` is not a variant"
                        )));
                    }
                    let expected = aggregate_metadata::variant_cases(program, variant)
                        .and_then(|cases| cases.iter().find(|item| item.id == *case))
                        .ok_or_else(|| {
                            scope_checkpoint::paths::diagnostic(format_args!(
                                "constructor for `{variant}` contains foreign case `{case}`"
                            ))
                        })?
                        .fields
                        .as_slice();
                    frames.push($Frame::VariantNext {
                        expression,
                        fields,
                        expected,
                        variant,
                        case,
                        arguments,
                        seen: BTreeSet::new(),
                        index: 0,
                        scope,
                        path,
                    })?;
                }
                ResolvedExprKind::UpdateRecord {
                    base,
                    record,
                    fields,
                } => {
                    frames.push($Frame::UpdateBase {
                        expression,
                        record,
                        fields,
                        path,
                    })?;
                    frames.push($Frame::Enter {
                        expression: base,
                        scope,
                        path: path_label::Label::child(path, ".base", [0, 0])?,
                    })?;
                }
                ResolvedExprKind::Match {
                    scrutinee, arms, ..
                } => {
                    frames.push($Frame::MatchScrutinee {
                        expression,
                        arms,
                        path,
                    })?;
                    frames.push($Frame::Enter {
                        expression: scrutinee,
                        scope,
                        path: path_label::Label::child(path, ".scrutinee", [0, 0])?,
                    })?;
                }
                ResolvedExprKind::Project { base, field } => {
                    if matches!(&base.kind, ResolvedExprKind::Place(_)) {
                        return Err(hir_error(
                            "place field projections must use a resolved place path",
                        ));
                    }
                    frames.push($Frame::Project { expression, field })?;
                    frames.push($Frame::Enter {
                        expression: base,
                        scope,
                        path: path_label::Label::child(path, ".base", [0, 0])?,
                    })?;
                }
                // Class Inheritance v1: independently re-derive the
                // upcast contract from the resolved source before the
                // ancestor-typed result is accepted.
                ResolvedExprKind::Upcast { source } => {
                    frames.push($Frame::Upcast { expression })?;
                    frames.push($Frame::Enter {
                        expression: source,
                        scope,
                        path: path_label::Label::child(path, ".source", [0, 0])?,
                    })?;
                }
                // Resumable Effects v1 (issue #204): the request
                // resolves like any other child expression; its
                // own type and the whole node's ownership are
                // independently re-derived on the way back up,
                // matching every other node here.
                ResolvedExprKind::Yield { request } => {
                    frames.push($Frame::Yield { expression })?;
                    frames.push($Frame::Enter {
                        expression: request,
                        scope,
                        path: path_label::Label::child(path, ".request", [0, 0])?,
                    })?;
                }
                _ => unreachable!("expression was routed to its matching phase"),
            }
            Ok(())
        }
    };
}
