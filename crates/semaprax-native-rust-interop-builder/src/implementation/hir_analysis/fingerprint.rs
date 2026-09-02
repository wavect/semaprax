//! Domain-separated HIR fingerprinting over the resolved program.

use super::*;

pub(in crate::implementation) fn hir_fingerprint(
    closure: &[&ResolvedFunction],
    imports: &[ImportFact],
    _capacity_baseline: usize,
) -> Result<String, Diagnostic> {
    let mut hasher = Sha256::new();
    hasher.update(HIR_DOMAIN);
    hash_count(&mut hasher, "functions", closure.len());
    for function in closure {
        frame(&mut hasher, b"function");
        frame(&mut hasher, function.id.as_str().as_bytes());
        frame(&mut hasher, function.name.as_bytes());
        frame(&mut hasher, function.result_id.as_str().as_bytes());
        frame(&mut hasher, function.body.id.as_str().as_bytes());
        let return_identity =
            fingerprint_type_identity(&function.return_type, _capacity_baseline, 0)?;
        #[cfg(test)]
        note_post_hir_facts_live(_capacity_baseline, return_identity.capacity());
        frame(&mut hasher, return_identity.as_bytes());
        hash_count(&mut hasher, "effects", function.effects.len());
        for effect in &function.effects {
            frame(&mut hasher, effect.as_bytes());
        }
        hash_count(&mut hasher, "parameters", function.params.len());
        for parameter in &function.params {
            frame(&mut hasher, parameter.id.as_str().as_bytes());
            frame(&mut hasher, parameter.name.as_bytes());
            frame(
                &mut hasher,
                match parameter.ownership {
                    OwnershipMode::Value => b"value",
                    OwnershipMode::Own => b"own",
                    OwnershipMode::Borrow => b"borrow",
                    OwnershipMode::Shared => b"shared",
                },
            );
            let parameter_identity =
                fingerprint_type_identity(&parameter.ty, _capacity_baseline, 0)?;
            #[cfg(test)]
            note_post_hir_facts_live(_capacity_baseline, parameter_identity.capacity());
            frame(&mut hasher, parameter_identity.as_bytes());
        }
        hash_count(&mut hasher, "requires", function.requires.len());
        for requirement in &function.requires {
            hash_expr(&mut hasher, requirement, _capacity_baseline)?;
        }
        frame(&mut hasher, b"body");
        hash_expr(&mut hasher, &function.body, _capacity_baseline)?;
        hash_count(&mut hasher, "ensures", function.ensures.len());
        for guarantee in &function.ensures {
            hash_expr(&mut hasher, guarantee, _capacity_baseline)?;
        }
    }
    hash_count(&mut hasher, "imports", imports.len());
    for import in imports {
        frame(&mut hasher, b"import");
        frame(&mut hasher, import.id.as_bytes());
        frame(&mut hasher, import.interface.as_bytes());
        frame(&mut hasher, import.import_key.as_bytes());
        frame(&mut hasher, scalar_text(import.result).as_bytes());
        hash_count(&mut hasher, "import-parameters", import.parameters.len());
        for parameter in &import.parameters {
            frame(&mut hasher, parameter.name.as_bytes());
            frame(&mut hasher, scalar_text(parameter.ty).as_bytes());
        }
        hash_count(&mut hasher, "import-effects", import.effects.len());
        for effect in &import.effects {
            frame(&mut hasher, effect.as_bytes());
        }
        frame(
            &mut hasher,
            import.failure.as_deref().unwrap_or("infallible").as_bytes(),
        );
        frame(&mut hasher, import.call_contract_digest.as_bytes());
    }
    let digest = format!(
        "sha256:{:x}",
        semaprax::digest_hex::LowerHex(hasher.finalize())
    );
    #[cfg(test)]
    note_post_hir_facts_live(_capacity_baseline, digest.capacity());
    Ok(digest)
}

pub(super) fn hash_count(hasher: &mut Sha256, label: &str, count: usize) {
    frame(hasher, label.as_bytes());
    frame(
        hasher,
        &u64::try_from(count).unwrap_or(u64::MAX).to_be_bytes(),
    );
}

pub(in crate::implementation) enum HirFingerprintAction<'a> {
    Expr(&'a ResolvedExpr, usize),
    Exprs(&'a [ResolvedExpr], usize, usize),
    Statement(&'a ResolvedStatement, usize),
    Statements(&'a [ResolvedStatement], usize, usize),
    Field(&'a crate::hir::ResolvedFieldInitializer, usize),
    Fields(&'a [crate::hir::ResolvedFieldInitializer], usize, usize),
    Pattern(&'a crate::hir::ResolvedMatchPattern),
    Patterns(&'a [crate::hir::ResolvedMatchPattern], usize),
    RecordPatternField(&'a crate::hir::ResolvedRecordMatchPatternField),
    RecordPatternFields(&'a [crate::hir::ResolvedRecordMatchPatternField], usize),
    Arms(&'a [crate::hir::ResolvedMatchArm], usize, usize),
    Arm(&'a crate::hir::ResolvedMatchArm, usize),
    TryIds([&'a DeclarationId; 5], usize),
    OptionIds([&'a DeclarationId; 4], usize),
    Bytes(&'a [u8]),
    Type(&'a ResolvedType),
}

pub(in crate::implementation) fn hash_expr(
    hasher: &mut Sha256,
    expression: &ResolvedExpr,
    _capacity_baseline: usize,
) -> Result<(), Diagnostic> {
    let ownership = |ownership| match ownership {
        OwnershipMode::Value => b"value".as_slice(),
        OwnershipMode::Own => b"own".as_slice(),
        OwnershipMode::Borrow => b"borrow".as_slice(),
        OwnershipMode::Shared => b"shared".as_slice(),
    };
    let mut actions = Vec::with_capacity(MAX_SEMANTIC_EXPRESSION_DEPTH * 4 + 8);
    actions.push(HirFingerprintAction::Expr(expression, 1));
    while let Some(action) = actions.pop() {
        if actions.len() + 4 > actions.capacity() {
            return Err(b109(
                "max_semantic_expression_depth",
                MAX_SEMANTIC_EXPRESSION_DEPTH,
            ));
        }
        match action {
            HirFingerprintAction::Bytes(value) => frame(hasher, value),
            HirFingerprintAction::Type(ty) => {
                let action_bytes = actions
                    .capacity()
                    .saturating_mul(std::mem::size_of::<HirFingerprintAction<'_>>());
                let identity = fingerprint_type_identity(ty, _capacity_baseline, action_bytes)?;
                #[cfg(test)]
                note_post_hir_facts_live(
                    _capacity_baseline,
                    actions
                        .capacity()
                        .saturating_mul(std::mem::size_of::<HirFingerprintAction<'_>>())
                        .saturating_add(identity.capacity()),
                );
                frame(hasher, identity.as_bytes());
            }
            HirFingerprintAction::Statement(statement, depth) => {
                match statement {
                    ResolvedStatement::Let { binding, value, .. }
                    | ResolvedStatement::Assign { binding, value, .. } => {
                        // These tags and the following byte sequence preserve
                        // the pre-While fingerprint contract exactly.
                        frame(
                            hasher,
                            if matches!(statement, ResolvedStatement::Assign { .. }) {
                                b"assign"
                            } else {
                                b"let"
                            },
                        );
                        frame(hasher, binding.id.as_str().as_bytes());
                        frame(hasher, binding.name.as_bytes());
                        let action_bytes = actions
                            .capacity()
                            .saturating_mul(std::mem::size_of::<HirFingerprintAction<'_>>());
                        let binding_identity = fingerprint_type_identity(
                            &binding.ty,
                            _capacity_baseline,
                            action_bytes,
                        )?;
                        #[cfg(test)]
                        note_post_hir_facts_live(
                            _capacity_baseline,
                            actions
                                .capacity()
                                .saturating_mul(std::mem::size_of::<HirFingerprintAction<'_>>())
                                .saturating_add(binding_identity.capacity()),
                        );
                        frame(hasher, binding_identity.as_bytes());
                        frame(hasher, ownership(binding.ownership));
                        actions.push(HirFingerprintAction::Expr(value, depth));
                    }
                    ResolvedStatement::Unsafe { audit, body, .. } => {
                        frame(hasher, b"unsafe");
                        frame(hasher, audit.as_bytes());
                        actions.push(HirFingerprintAction::Expr(body, depth));
                    }
                    ResolvedStatement::While {
                        condition, body, ..
                    } => {
                        frame(hasher, b"while");
                        actions.push(HirFingerprintAction::Expr(body, depth));
                        actions.push(HirFingerprintAction::Expr(condition, depth));
                    }
                }
            }
            HirFingerprintAction::Statements(statements, index, depth) => {
                if let Some(statement) = statements.get(index) {
                    actions.push(HirFingerprintAction::Statements(
                        statements,
                        index + 1,
                        depth,
                    ));
                    actions.push(HirFingerprintAction::Statement(statement, depth));
                }
            }
            HirFingerprintAction::Exprs(expressions, index, depth) => {
                if let Some(expression) = expressions.get(index) {
                    actions.push(HirFingerprintAction::Exprs(expressions, index + 1, depth));
                    actions.push(HirFingerprintAction::Expr(expression, depth));
                }
            }
            HirFingerprintAction::TryIds(ids, index) => {
                if let Some(id) = ids.get(index) {
                    actions.push(HirFingerprintAction::TryIds(ids, index + 1));
                    actions.push(HirFingerprintAction::Bytes(id.as_str().as_bytes()));
                }
            }
            HirFingerprintAction::OptionIds(ids, index) => {
                if let Some(id) = ids.get(index) {
                    actions.push(HirFingerprintAction::OptionIds(ids, index + 1));
                    actions.push(HirFingerprintAction::Bytes(id.as_str().as_bytes()));
                }
            }
            HirFingerprintAction::Field(field, depth) => {
                frame(hasher, field.field.as_str().as_bytes());
                actions.push(HirFingerprintAction::Expr(&field.value, depth));
            }
            HirFingerprintAction::Fields(fields, index, depth) => {
                if index == 0 {
                    hash_count(hasher, "fields", fields.len());
                }
                if let Some(field) = fields.get(index) {
                    actions.push(HirFingerprintAction::Fields(fields, index + 1, depth));
                    actions.push(HirFingerprintAction::Field(field, depth));
                }
            }
            HirFingerprintAction::Arms(arms, index, depth) => {
                if index == 0 {
                    hash_count(hasher, "arms", arms.len());
                }
                if let Some(arm) = arms.get(index) {
                    actions.push(HirFingerprintAction::Arms(arms, index + 1, depth));
                    actions.push(HirFingerprintAction::Arm(arm, depth));
                }
            }
            HirFingerprintAction::Arm(arm, depth) => {
                actions.push(HirFingerprintAction::Expr(&arm.value, depth));
                if let Some(guard) = &arm.guard {
                    actions.push(HirFingerprintAction::Expr(guard, depth));
                    actions.push(HirFingerprintAction::Bytes(b"guard"));
                }
                actions.push(HirFingerprintAction::Pattern(&arm.pattern));
            }
            HirFingerprintAction::RecordPatternFields(fields, index) => {
                if let Some(field) = fields.get(index) {
                    actions.push(HirFingerprintAction::RecordPatternFields(fields, index + 1));
                    actions.push(HirFingerprintAction::RecordPatternField(field));
                }
            }
            HirFingerprintAction::RecordPatternField(field) => {
                frame(hasher, field.field.as_str().as_bytes());
                match &field.pattern {
                    crate::hir::ResolvedRecordMatchFieldPattern::Binding(binding) => {
                        frame(hasher, b"binding");
                        hash_binding(
                            hasher,
                            binding,
                            _capacity_baseline,
                            actions
                                .capacity()
                                .saturating_mul(std::mem::size_of::<HirFingerprintAction<'_>>()),
                        )?;
                    }
                    crate::hir::ResolvedRecordMatchFieldPattern::Wildcard => {
                        frame(hasher, b"wildcard");
                    }
                    crate::hir::ResolvedRecordMatchFieldPattern::Record {
                        record,
                        instance,
                        fields,
                    } => {
                        frame(hasher, b"record");
                        frame(hasher, record.as_str().as_bytes());
                        let action_bytes = actions
                            .capacity()
                            .saturating_mul(std::mem::size_of::<HirFingerprintAction<'_>>());
                        let instance_identity =
                            fingerprint_type_identity(instance, _capacity_baseline, action_bytes)?;
                        #[cfg(test)]
                        note_post_hir_facts_live(
                            _capacity_baseline,
                            actions
                                .capacity()
                                .saturating_mul(std::mem::size_of::<HirFingerprintAction<'_>>())
                                .saturating_add(instance_identity.capacity()),
                        );
                        frame(hasher, instance_identity.as_bytes());
                        hash_count(hasher, "record-pattern-fields", fields.len());
                        actions.push(HirFingerprintAction::RecordPatternFields(fields, 0));
                    }
                }
            }
            HirFingerprintAction::Pattern(pattern) => match pattern {
                crate::hir::ResolvedMatchPattern::Wildcard => frame(hasher, b"wildcard"),
                // Refutable Match v1: deterministic fingerprints for the
                // additive pattern spellings.
                crate::hir::ResolvedMatchPattern::Literal(value) => {
                    frame(hasher, b"literal");
                    match value {
                        crate::hir::PatternValue::Int(inner) => {
                            frame(hasher, b"int");
                            frame(hasher, inner.to_le_bytes().as_slice());
                        }
                        crate::hir::PatternValue::Int32(inner) => {
                            frame(hasher, b"int32");
                            frame(hasher, inner.to_le_bytes().as_slice());
                        }
                        crate::hir::PatternValue::Uint8(inner) => {
                            frame(hasher, b"uint8");
                            frame(hasher, [*inner].as_slice());
                        }
                        crate::hir::PatternValue::Usize(inner) => {
                            frame(hasher, b"usize");
                            frame(hasher, inner.to_le_bytes().as_slice());
                        }
                        crate::hir::PatternValue::Char(inner) => {
                            frame(hasher, b"char");
                            frame(hasher, inner.to_le_bytes().as_slice());
                        }
                        crate::hir::PatternValue::Bool(inner) => {
                            frame(hasher, b"bool");
                            frame(hasher, [*inner as u8].as_slice());
                        }
                    }
                }
                crate::hir::ResolvedMatchPattern::Or(alternatives) => {
                    frame(hasher, b"or");
                    hash_count(hasher, "or-alternatives", alternatives.len());
                    actions.push(HirFingerprintAction::Patterns(alternatives, 0));
                }
                crate::hir::ResolvedMatchPattern::Binding(binding) => {
                    frame(hasher, b"binding");
                    hash_binding(
                        hasher,
                        binding,
                        _capacity_baseline,
                        actions
                            .capacity()
                            .saturating_mul(std::mem::size_of::<HirFingerprintAction<'_>>()),
                    )?;
                }
                crate::hir::ResolvedMatchPattern::Variant {
                    variant,
                    case,
                    fields,
                } => {
                    frame(hasher, b"variant");
                    frame(hasher, variant.as_str().as_bytes());
                    frame(hasher, case.as_str().as_bytes());
                    hash_count(hasher, "variant-pattern-fields", fields.len());
                    for field in fields {
                        frame(hasher, field.field.as_str().as_bytes());
                        hash_binding(
                            hasher,
                            &field.binding,
                            _capacity_baseline,
                            actions
                                .capacity()
                                .saturating_mul(std::mem::size_of::<HirFingerprintAction<'_>>()),
                        )?;
                    }
                }
                crate::hir::ResolvedMatchPattern::Record {
                    record,
                    instance,
                    fields,
                } => {
                    frame(hasher, b"record");
                    frame(hasher, record.as_str().as_bytes());
                    let action_bytes = actions
                        .capacity()
                        .saturating_mul(std::mem::size_of::<HirFingerprintAction<'_>>());
                    let instance_identity =
                        fingerprint_type_identity(instance, _capacity_baseline, action_bytes)?;
                    #[cfg(test)]
                    note_post_hir_facts_live(
                        _capacity_baseline,
                        actions
                            .capacity()
                            .saturating_mul(std::mem::size_of::<HirFingerprintAction<'_>>())
                            .saturating_add(instance_identity.capacity()),
                    );
                    frame(hasher, instance_identity.as_bytes());
                    hash_count(hasher, "record-pattern-fields", fields.len());
                    actions.push(HirFingerprintAction::RecordPatternFields(fields, 0));
                }
            },
            HirFingerprintAction::Patterns(patterns, index) => {
                if let Some(pattern) = patterns.get(index) {
                    actions.push(HirFingerprintAction::Patterns(patterns, index + 1));
                    actions.push(HirFingerprintAction::Pattern(pattern));
                }
            }
            HirFingerprintAction::Expr(expression, depth) => {
                if depth > MAX_SEMANTIC_EXPRESSION_DEPTH {
                    return Err(b109(
                        "max_semantic_expression_depth",
                        MAX_SEMANTIC_EXPRESSION_DEPTH,
                    ));
                }
                let child_depth = depth.checked_add(1).ok_or_else(|| {
                    b109(
                        "max_semantic_expression_depth",
                        MAX_SEMANTIC_EXPRESSION_DEPTH,
                    )
                })?;
                frame(hasher, b"expression");
                frame(hasher, expression.id.as_str().as_bytes());
                let action_bytes = actions
                    .capacity()
                    .saturating_mul(std::mem::size_of::<HirFingerprintAction<'_>>());
                let identity =
                    fingerprint_type_identity(&expression.ty, _capacity_baseline, action_bytes)?;
                #[cfg(test)]
                note_post_hir_facts_live(
                    _capacity_baseline,
                    actions
                        .capacity()
                        .saturating_mul(std::mem::size_of::<HirFingerprintAction<'_>>())
                        .saturating_add(identity.capacity()),
                );
                frame(hasher, identity.as_bytes());
                frame(hasher, ownership(expression.ownership));
                match &expression.kind {
                    ResolvedExprKind::Int32(_)
                    | ResolvedExprKind::Char(_)
                    | ResolvedExprKind::Uint8(_)
                    | ResolvedExprKind::Usize(_)
                    | ResolvedExprKind::ArrayU8(_)
                    | ResolvedExprKind::RepeatArrayU8 { .. }
                    | ResolvedExprKind::Float32(_)
                    | ResolvedExprKind::Float64(_)
                    | ResolvedExprKind::String(_)
                    | ResolvedExprKind::BorrowPlace { .. }
                    | ResolvedExprKind::ByteRange { .. } => {
                        // Non-i64 scalar signatures are outside the scalar
                        // native boundary; admission rejects them first.
                        return Err(b107("scalar value signature required"));
                    }
                    ResolvedExprKind::Int(value) => {
                        frame(hasher, b"int");
                        frame(hasher, &value.to_be_bytes());
                    }
                    ResolvedExprKind::Bool(value) => {
                        frame(hasher, b"bool");
                        frame(hasher, &[*value as u8]);
                    }
                    ResolvedExprKind::Place(place) => {
                        frame(hasher, b"place");
                        frame(hasher, place.root.as_str().as_bytes());
                        hash_count(hasher, "projections", place.projections.len());
                        for projection in &place.projections {
                            match projection {
                                crate::hir::PlaceProjection::Field(field) => {
                                    frame(hasher, b"field");
                                    frame(hasher, field.as_str().as_bytes());
                                }
                                crate::hir::PlaceProjection::VariantField { case, field } => {
                                    frame(hasher, b"variant-field");
                                    frame(hasher, case.as_str().as_bytes());
                                    frame(hasher, field.as_str().as_bytes());
                                }
                            }
                        }
                    }
                    ResolvedExprKind::Call {
                        callee,
                        type_arguments,
                        instance,
                        args,
                    } => {
                        frame(hasher, b"call");
                        frame(hasher, callee.as_str().as_bytes());
                        hash_count(hasher, "type-arguments", type_arguments.len());
                        for argument in type_arguments {
                            let action_bytes = actions
                                .capacity()
                                .saturating_mul(std::mem::size_of::<HirFingerprintAction<'_>>());
                            let argument_identity = fingerprint_type_identity(
                                argument,
                                _capacity_baseline,
                                action_bytes,
                            )?;
                            #[cfg(test)]
                            note_post_hir_facts_live(
                                _capacity_baseline,
                                actions
                                    .capacity()
                                    .saturating_mul(std::mem::size_of::<HirFingerprintAction<'_>>())
                                    .saturating_add(argument_identity.capacity()),
                            );
                            frame(hasher, argument_identity.as_bytes());
                        }
                        frame(
                            hasher,
                            instance
                                .as_ref()
                                .map_or(b"".as_slice(), |value| value.as_str().as_bytes()),
                        );
                        hash_count(hasher, "arguments", args.len());
                        actions.push(HirFingerprintAction::Exprs(args, 0, child_depth));
                    }
                    ResolvedExprKind::NativeRustImportCall(call) => {
                        frame(hasher, b"native-rust-import");
                        frame(hasher, call.expression.as_str().as_bytes());
                        frame(hasher, call.import.as_str().as_bytes());
                        frame(
                            hasher,
                            match call.result {
                                ResolvedImportResultKind::Unit => b"unit",
                                ResolvedImportResultKind::I64 => b"i64",
                                ResolvedImportResultKind::Bool => b"bool",
                            },
                        );
                        hash_count(hasher, "arguments", call.args.len());
                        actions.push(HirFingerprintAction::Exprs(&call.args, 0, child_depth));
                    }
                    ResolvedExprKind::HostCommandCall(_) => {
                        // Public Native Rust SDKs do not carry invocation
                        // command-input/output authority.
                        return Err(b107("scalar value signature required"));
                    }
                    ResolvedExprKind::Unary { op, value } => {
                        frame(
                            hasher,
                            match op {
                                crate::ast::UnaryOp::Neg => b"unary-neg",
                                crate::ast::UnaryOp::Not => b"unary-not",
                            },
                        );
                        actions.push(HirFingerprintAction::Expr(value, child_depth));
                    }
                    ResolvedExprKind::Binary { op, left, right } => {
                        frame(
                            hasher,
                            match op {
                                crate::ast::BinaryOp::Add => b"binary-add",
                                crate::ast::BinaryOp::Sub => b"binary-sub",
                                crate::ast::BinaryOp::Mul => b"binary-mul",
                                crate::ast::BinaryOp::Div => b"binary-div",
                                crate::ast::BinaryOp::Rem => b"binary-rem",
                                crate::ast::BinaryOp::Eq => b"binary-eq",
                                crate::ast::BinaryOp::Ne => b"binary-ne",
                                crate::ast::BinaryOp::Lt => b"binary-lt",
                                crate::ast::BinaryOp::Le => b"binary-le",
                                crate::ast::BinaryOp::Gt => b"binary-gt",
                                crate::ast::BinaryOp::Ge => b"binary-ge",
                                crate::ast::BinaryOp::And => b"binary-and",
                                crate::ast::BinaryOp::Or => b"binary-or",
                            },
                        );
                        actions.push(HirFingerprintAction::Expr(right, child_depth));
                        actions.push(HirFingerprintAction::Expr(left, child_depth));
                    }
                    ResolvedExprKind::Block { statements, tail } => {
                        frame(hasher, b"block");
                        hash_count(hasher, "statements", statements.len());
                        actions.push(HirFingerprintAction::Expr(tail, child_depth));
                        actions.push(HirFingerprintAction::Statements(statements, 0, child_depth));
                    }
                    ResolvedExprKind::If {
                        condition,
                        then_branch,
                        else_branch,
                    } => {
                        frame(hasher, b"if");
                        actions.push(HirFingerprintAction::Expr(else_branch, child_depth));
                        actions.push(HirFingerprintAction::Expr(then_branch, child_depth));
                        actions.push(HirFingerprintAction::Expr(condition, child_depth));
                    }
                    ResolvedExprKind::ConstructRecord { record, fields } => {
                        frame(hasher, b"construct-record");
                        frame(hasher, record.as_str().as_bytes());
                        actions.push(HirFingerprintAction::Fields(fields, 0, child_depth));
                    }
                    ResolvedExprKind::ConstructVariant {
                        variant,
                        case,
                        fields,
                    } => {
                        frame(hasher, b"construct-variant");
                        frame(hasher, variant.as_str().as_bytes());
                        frame(hasher, case.as_str().as_bytes());
                        actions.push(HirFingerprintAction::Fields(fields, 0, child_depth));
                    }
                    ResolvedExprKind::Match {
                        scrutinee, arms, ..
                    } => {
                        frame(hasher, b"match");
                        actions.push(HirFingerprintAction::Arms(arms, 0, child_depth));
                        actions.push(HirFingerprintAction::Expr(scrutinee, child_depth));
                    }
                    ResolvedExprKind::Try {
                        operand,
                        result,
                        ok_case,
                        ok_field,
                        err_case,
                        err_field,
                        residual_type,
                    } => {
                        frame(hasher, b"try");
                        let ids = [result, ok_case, ok_field, err_case, err_field];
                        actions.push(HirFingerprintAction::Type(residual_type));
                        actions.push(HirFingerprintAction::TryIds(ids, 0));
                        actions.push(HirFingerprintAction::Expr(operand, child_depth));
                    }
                    ResolvedExprKind::TryOption {
                        operand,
                        option,
                        some_case,
                        some_field,
                        none_case,
                        residual_type,
                    } => {
                        frame(hasher, b"try-option");
                        let ids = [option, some_case, some_field, none_case];
                        actions.push(HirFingerprintAction::Type(residual_type));
                        actions.push(HirFingerprintAction::OptionIds(ids, 0));
                        actions.push(HirFingerprintAction::Expr(operand, child_depth));
                    }
                    ResolvedExprKind::UpdateRecord {
                        base,
                        record,
                        fields,
                    } => {
                        frame(hasher, b"update-record");
                        actions.push(HirFingerprintAction::Fields(fields, 0, child_depth));
                        actions.push(HirFingerprintAction::Bytes(record.as_str().as_bytes()));
                        actions.push(HirFingerprintAction::Expr(base, child_depth));
                    }
                    ResolvedExprKind::Project { base, field } => {
                        frame(hasher, b"project");
                        actions.push(HirFingerprintAction::Bytes(field.as_str().as_bytes()));
                        actions.push(HirFingerprintAction::Expr(base, child_depth));
                    }
                    ResolvedExprKind::Upcast { source } => {
                        frame(hasher, b"upcast");
                        actions.push(HirFingerprintAction::Expr(source, child_depth));
                    }
                }
            }
        }
        #[cfg(test)]
        note_post_hir_facts_live(
            _capacity_baseline,
            actions
                .capacity()
                .saturating_mul(std::mem::size_of::<HirFingerprintAction<'_>>()),
        );
    }
    Ok(())
}

fn hash_binding(
    hasher: &mut Sha256,
    binding: &crate::hir::ResolvedBinding,
    _capacity_baseline: usize,
    _action_bytes: usize,
) -> Result<(), Diagnostic> {
    frame(hasher, binding.id.as_str().as_bytes());
    frame(hasher, binding.name.as_bytes());
    let binding_identity =
        fingerprint_type_identity(&binding.ty, _capacity_baseline, _action_bytes)?;
    #[cfg(test)]
    note_post_hir_facts_live(
        _capacity_baseline,
        _action_bytes.saturating_add(binding_identity.capacity()),
    );
    frame(hasher, binding_identity.as_bytes());
    frame(
        hasher,
        match binding.ownership {
            OwnershipMode::Value => b"value",
            OwnershipMode::Own => b"own",
            OwnershipMode::Borrow => b"borrow",
            OwnershipMode::Shared => b"shared",
        },
    );
    Ok(())
}
