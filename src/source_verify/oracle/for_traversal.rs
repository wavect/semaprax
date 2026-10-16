//! Recursive source twin of bounded traversal admission and loan expiry.
use super::*;
use crate::ast::program_source::ProgramSource;
#[allow(clippy::too_many_arguments)]
pub(super) fn check(
    program: &dyn ProgramSource,
    current: &Function,
    statement: &Statement,
    scope: &mut HashMap<String, Binding>,
    functions: &crate::source_verify::function_map::FunctionMap<'_>,
    types: &TypeTable<'_>,
    result_type: Option<&Type>,
    allow_moves: bool,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let (item, item_span, values, body, selected, borrowing) = match statement {
        Statement::For {
            item,
            item_span,
            values,
            body,
            ..
        } => (item, item_span, values, body, None, false),
        Statement::ForBorrow {
            item,
            item_span,
            values,
            body,
            index,
            ..
        } => (item, item_span, values, body, index.as_ref(), true),
        _ => unreachable!(),
    };
    let actual = check_expr(
        program,
        current,
        values,
        scope,
        functions,
        types,
        result_type,
        allow_moves,
        diagnostics,
    );
    let source = match &values.kind {
        ExprKind::Var(name) => Some(name.as_str()),
        _ => None,
    };
    let element = actual.as_ref().and_then(|actual| match &actual.ty {
                            ty @ Type::SliceU8 if !borrowing => crate::source_verify::byte_traversal::element(ty),
                            Type::Named { name, arguments }
                                if name == "Vec" && matches!(arguments.as_slice(), [ty] if (!borrowing && crate::vec_ops::ast_element_is_admitted(ty)) || (borrowing && crate::source_verify::record_traversal::admitted(types, ty))) =>
                            {
                                Some(arguments[0].clone())
                            }
                            _ => None,
                        });
    if source.is_none() || element.is_none() {
        diagnostics.push(error(
            program,
            "SPX-T284",
            "for traversal requires a simple immutable binding of exact Vec<T> for a Copy scalar T",
            values.span,
        ));
    }
    if source.is_some_and(|name| scope.get(name).is_some_and(|binding| binding.mutable)) {
        diagnostics.push(
            error(
                program,
                "SPX-T284",
                "for traversal source must be an immutable Vec binding",
                values.span,
            )
            .with_help(crate::source_verify::hints::IMMUTABLE_TRAVERSAL_HELP),
        );
    }
    let source_name = source.map(str::to_owned);
    let _ = reject_while_disallowed_oracle(program, body, functions, types, diagnostics);
    let item_inserted = !scope.contains_key(item);
    if !item_inserted {
        diagnostics.push(error(
            program,
            "SPX-T209",
            format!("loop item `{item}` shadows an existing value"),
            *item_span,
        ));
    }
    if item_inserted {
        if let Some(element) = element {
            scope.insert(
                item.clone(),
                Binding {
                    contract_local: false,
                    traversal_row: None,
                    ty: element,
                    mode: ParamMode::Value,
                    availability: Availability::Available,
                    moved_places: HashMap::new(),
                    definitely_partial: HashSet::new(),
                    native_unit_discard: false,
                    mutable: false,
                    active_loans: BTreeSet::new(),
                    borrow_origin: None,
                },
            );
        }
    }
    let mut index_inserted = false;
    let freeze = crate::source_verify::binding::SourceLoanId {
        borrower: item.clone(),
        start: values.span.start,
        end: body.span.end,
    };
    if borrowing && item_inserted {
        if let Some(root) = source {
            match crate::source_verify::record_traversal::body_binding_conflict(
                body,
                root,
                item,
                selected.map(|index| index.name.as_str()),
            ) {
                Ok(Some(span)) => {
                    diagnostics.push(error(
                        program,
                        "SPX-T284",
                        "record traversal cannot shadow its carrier, row or index binding",
                        span,
                    ));
                }
                Ok(None) => {}
                Err(()) => diagnostics.push(error(
                    program,
                    "SPX-T284",
                    "record traversal binding scan exceeds its source scratch, depth or work budget",
                    body.span,
                )),
            }
            if let Some(binding) = scope.get_mut(item) {
                binding.mode = ParamMode::Borrow;
                binding.traversal_row =
                    Some(Box::new(crate::source_verify::record_traversal::Row {
                        root: root.to_owned(),
                        element: binding.ty.clone(),
                        freeze: freeze.clone(),
                        index_inserted: false,
                    }));
            }
            if let Some(binding) = scope.get_mut(root) {
                binding
                    .active_loans
                    .insert(crate::source_verify::binding::SourceLoan {
                        id: freeze.clone(),
                        projections: Vec::new(),
                    });
            }
        }
        if let Some(index) = selected {
            if index.name == *item || scope.contains_key(&index.name) {
                diagnostics.push(error(
                    program,
                    "SPX-T209",
                    "record traversal index must be a fresh distinct binding",
                    index.span,
                ));
            } else {
                scope.insert(
                    index.name.clone(),
                    Binding {
                        contract_local: false,
                        traversal_row: None,
                        ty: Type::Usize,
                        mode: ParamMode::Value,
                        availability: Availability::Available,
                        moved_places: HashMap::new(),
                        definitely_partial: HashSet::new(),
                        native_unit_discard: false,
                        mutable: false,
                        active_loans: BTreeSet::new(),
                        borrow_origin: None,
                    },
                );
                index_inserted = true;
            }
        }
    }
    let byte_freeze = (!borrowing
        && item_inserted
        && actual
            .as_ref()
            .is_some_and(|actual| actual.ty == Type::SliceU8))
    .then(|| crate::source_verify::byte_traversal::freeze_id(item, values.span, body.span));
    if let (Some(id), Some(root)) = (&byte_freeze, source) {
        crate::source_verify::byte_traversal::freeze(scope, root, id);
    }
    let _ = matching::in_loop(|| {
        check_expr(
            program,
            current,
            body,
            scope,
            functions,
            types,
            result_type,
            allow_moves,
            diagnostics,
        )
    });
    if item_inserted {
        scope.remove(item);
    }
    if let Some(id) = &byte_freeze {
        crate::source_verify::byte_traversal::release(scope, id);
    }
    if borrowing {
        if let Some(root) = source {
            if let Some(binding) = scope.get_mut(root) {
                binding.active_loans.retain(|loan| loan.id != freeze);
            }
        }
        if index_inserted {
            scope.remove(&selected.expect("inserted index").name);
        }
    }
    if let Some(source) = source_name {
        let changed = scope.get(&source).is_none_or(|after| {
            after.availability != Availability::Available
                || !after.moved_places.is_empty()
                || !after.definitely_partial.is_empty()
        });
        if changed {
            diagnostics.push(error(
                                    program,
                                    "SPX-T284",
                                    format!("for traversal source `{source}` cannot be consumed or mutated in the loop body"),
                                    body.span,
                                ));
        }
    }
}
