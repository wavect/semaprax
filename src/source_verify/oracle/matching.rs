//! Test-only recursive checking of `match` expressions: exhaustiveness,
//! per-arm binding state, and the joins that produce the match's value.

use crate::ast::{Expr, Function, MatchMode, MatchPattern, ParamMode, Program, Type};
use crate::diagnostic::Diagnostic;
use crate::source_verify::binding::{Availability, Binding, CheckedValue};
use crate::source_verify::declared_type::check_record_pattern;
use crate::source_verify::diagnostics::{error, reject_native_unit_value, source_identifier};
use crate::source_verify::loans::{activate_match_loan, mark_value_sources_moved, merge_moved};
use crate::source_verify::oracle::check_expr;
use crate::source_verify::place::{join_definitely_partial, join_moved_places, source_place};
use crate::source_verify::scope::pattern_literal_type;
use crate::source_verify::type_table::TypeTable;
use std::collections::{BTreeSet, HashMap, HashSet};

#[allow(clippy::too_many_arguments, clippy::borrowed_box)]
pub(super) fn oracle_match(
    mode: &MatchMode,
    scrutinee: &Box<Expr>,
    arms: &Vec<crate::ast::MatchArm>,
    program: &Program,
    current: &Function,
    expr: &Expr,
    variables: &mut HashMap<String, Binding>,
    functions: &HashMap<&str, &Function>,
    types: &TypeTable<'_>,
    result_type: Option<&Type>,
    allow_moves: bool,
    diagnostics: &mut Vec<Diagnostic>,
) -> Option<CheckedValue> {
    let scrutinee_value = check_expr(
        program,
        current,
        scrutinee,
        variables,
        functions,
        types,
        result_type,
        allow_moves,
        diagnostics,
    );
    if let Some(value) = &scrutinee_value {
        reject_native_unit_value(program, scrutinee, value, diagnostics);
    }
    // Refutable Match v1: Copy-scalar decision chain in the
    // recursive oracle twin.
    let scalar_scrutinee = *mode == MatchMode::Value
        && scrutinee_value.as_ref().is_some_and(|value| {
            matches!(
                value.ty,
                Type::I64 | Type::I32 | Type::Char | Type::U8 | Type::Usize | Type::Bool
            ) && value.mode == ParamMode::Value
        });
    if scalar_scrutinee {
        let scrutinee_value = scrutinee_value.expect("scalar checked above");
        let mut result = None::<CheckedValue>;
        for arm in arms {
            match &arm.pattern {
                MatchPattern::Wildcard { .. } => {}
                MatchPattern::Binding { name, span } => {
                    if !source_identifier(name) || variables.contains_key(name) {
                        diagnostics.push(error(
                            program,
                            "SPX-M104",
                            format!("invalid or duplicate pattern binding `{name}`"),
                            *span,
                        ));
                    } else {
                        variables.insert(
                            name.clone(),
                            Binding {
                                ty: scrutinee_value.ty.clone(),
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
                MatchPattern::Literal { value, span } => {
                    if pattern_literal_type(*value) != scrutinee_value.ty {
                        diagnostics.push(error(
                            program,
                            "SPX-T255",
                            format!(
                                "literal pattern of type `{}` cannot match a `{}` \
                                 scrutinee; pattern literals compare against exactly \
                                 their own type",
                                value.type_text(),
                                scrutinee_value.ty
                            ),
                            *span,
                        ));
                    }
                }
                MatchPattern::Or { alternatives, span } => {
                    let mut seen_type: Option<Type> = None;
                    for alternative in alternatives {
                        let MatchPattern::Literal { value, span } = alternative else {
                            diagnostics.push(error(
                                program,
                                "SPX-M105",
                                "or-patterns admit only literal alternatives in v1",
                                alternative.span(),
                            ));
                            continue;
                        };
                        if pattern_literal_type(*value) != scrutinee_value.ty {
                            diagnostics.push(error(
                                program,
                                "SPX-T255",
                                format!(
                                    "literal pattern of type `{}` cannot match a `{}` \
                                     scrutinee; pattern literals compare against \
                                     exactly their own type",
                                    value.type_text(),
                                    scrutinee_value.ty
                                ),
                                *span,
                            ));
                        }
                        match (&seen_type, pattern_literal_type(*value)) {
                            (None, ty) => seen_type = Some(ty),
                            (Some(seen), ty) if *seen == ty => {}
                            (Some(seen), ty) => {
                                diagnostics.push(error(
                                    program,
                                    "SPX-M105",
                                    format!(
                                        "or-pattern mixes `{seen}` and `{ty}` literal \
                                         alternatives"
                                    ),
                                    *span,
                                ));
                            }
                        }
                    }
                    if alternatives.is_empty() {
                        diagnostics.push(error(
                            program,
                            "SPX-M105",
                            "or-pattern needs at least one literal alternative",
                            *span,
                        ));
                    }
                }
                MatchPattern::Variant { span, .. } | MatchPattern::Record { span, .. } => {
                    diagnostics.push(error(
                        program,
                        "SPX-M103",
                        "aggregate pattern is incompatible with a Copy-scalar scrutinee",
                        *span,
                    ));
                }
            }
            if let Some(guard) = &arm.guard {
                let guard_value = check_expr(
                    program,
                    current,
                    guard.as_ref(),
                    variables,
                    functions,
                    types,
                    result_type,
                    allow_moves,
                    diagnostics,
                );
                if let Some(value) = &guard_value {
                    reject_native_unit_value(program, guard.as_ref(), value, diagnostics);
                }
                if guard_value
                    .as_ref()
                    .is_none_or(|value| value.ty != Type::Bool || value.mode != ParamMode::Value)
                {
                    diagnostics.push(error(
                        program,
                        "SPX-T256",
                        format!(
                            "match guard must be bool; received {}",
                            guard_value.as_ref().map_or_else(
                                || "an invalid value".to_owned(),
                                |value| value.ty.to_string()
                            )
                        ),
                        guard.span,
                    ));
                }
            }
            let arm_value = check_expr(
                program,
                current,
                &arm.value,
                variables,
                functions,
                types,
                result_type,
                allow_moves,
                diagnostics,
            );
            if let Some(value) = &arm_value {
                reject_native_unit_value(program, &arm.value, value, diagnostics);
            }
            if let Some(arm_value) = arm_value {
                if let Some(expected) = &result {
                    if expected.ty != arm_value.ty || expected.mode != arm_value.mode {
                        diagnostics.push(error(
                            program,
                            "SPX-T259",
                            format!(
                                "match arms return incompatible values: {} and {}",
                                expected.ty, arm_value.ty
                            ),
                            arm.value.span,
                        ));
                    }
                } else {
                    result = Some(arm_value);
                }
            }
        }
        let last = arms.last().expect("match always has an arm list");
        let catch_all = matches!(
            &last.pattern,
            MatchPattern::Wildcard { .. } | MatchPattern::Binding { .. }
        );
        if !catch_all || last.guard.is_some() {
            diagnostics.push(error(
                program,
                "SPX-T257",
                "refutable match requires a trailing irrefutable catch-all arm (`_` or \
                 a binding) without a guard",
                last.span,
            ));
        }
        return result;
    }
    if scrutinee_value
        .as_ref()
        .is_some_and(|value| types.record_fields(&value.ty).is_some())
    {
        let Some(scrutinee_value) = scrutinee_value else {
            unreachable!("record instance was checked above");
        };
        let needs_drop = types.needs_drop(&scrutinee_value.ty);
        match mode {
            MatchMode::Value => {
                if needs_drop || scrutinee_value.mode != ParamMode::Value {
                    diagnostics.push(error(
                        program,
                        "SPX-O111",
                        "plain record match requires a Copy scrutinee",
                        scrutinee.span,
                    ));
                }
            }
            MatchMode::Own => {
                if !needs_drop || scrutinee_value.mode != ParamMode::Own {
                    diagnostics.push(error(
                        program,
                        "SPX-O117",
                        "`match own` requires an owned non-Copy record scrutinee",
                        scrutinee.span,
                    ));
                } else if allow_moves {
                    mark_value_sources_moved(program, scrutinee, variables, types, diagnostics);
                } else {
                    diagnostics.push(error(
                        program,
                        "SPX-O105",
                        "contract expression cannot consume a match scrutinee",
                        scrutinee.span,
                    ));
                }
            }
            MatchMode::Borrow => {
                if !needs_drop
                    || !matches!(scrutinee_value.mode, ParamMode::Own | ParamMode::Borrow)
                    || source_place(scrutinee, variables, types)
                        .is_none_or(|place| !place.projections.is_empty())
                {
                    diagnostics.push(error(
                        program,
                        "SPX-O117",
                        "`match borrow` requires a named owned or borrowed non-Copy record place",
                        scrutinee.span,
                    ));
                }
            }
        }
        let Some((first, rest)) = arms.split_first() else {
            diagnostics.push(error(
                program,
                "SPX-M101",
                format!(
                    "non-exhaustive match; missing record pattern for `{}`",
                    scrutinee_value.ty
                ),
                expr.span,
            ));
            return None;
        };
        for arm in rest {
            diagnostics.push(error(
                program,
                "SPX-M102",
                "unreachable arm after an irrefutable record pattern",
                arm.pattern.span(),
            ));
        }
        let outer_names = variables.keys().cloned().collect::<Vec<_>>();
        let mut arm_variables = variables.clone();
        if *mode == MatchMode::Borrow {
            if let Some(place) = source_place(scrutinee, variables, types) {
                activate_match_loan(&mut arm_variables, &place, expr.span);
            }
        }
        match &first.pattern {
            MatchPattern::Wildcard { span } if *mode != MatchMode::Value => {
                diagnostics.push(error(
                    program,
                    "SPX-O117",
                    "explicit ownership match requires an exact record pattern",
                    *span,
                ));
            }
            MatchPattern::Wildcard { .. } => {}
            MatchPattern::Record {
                type_name,
                fields,
                span,
                ..
            } => check_record_pattern(
                program,
                type_name,
                fields,
                &scrutinee_value.ty,
                &mut arm_variables,
                types,
                diagnostics,
                *span,
                *mode,
            ),
            MatchPattern::Variant { .. } => diagnostics.push(error(
                program,
                "SPX-M103",
                "variant pattern is incompatible with a record scrutinee",
                first.pattern.span(),
            )),
            MatchPattern::Literal { .. }
            | MatchPattern::Or { .. }
            | MatchPattern::Binding { .. } => {
                diagnostics.push(error(
                    program,
                    "SPX-T254",
                    "refutable patterns are incompatible with an aggregate record \
                     scrutinee",
                    first.pattern.span(),
                ));
            }
        }
        let result = check_expr(
            program,
            current,
            &first.value,
            &mut arm_variables,
            functions,
            types,
            result_type,
            allow_moves,
            diagnostics,
        );
        if let Some(value) = &result {
            reject_native_unit_value(program, &first.value, value, diagnostics);
        }
        merge_moved(variables, &arm_variables, &outer_names);
        if result.as_ref().is_some_and(|value| {
            !matches!(value.ty, Type::I64 | Type::Bool) || value.mode != ParamMode::Value
        }) {
            diagnostics.push(error(
                program,
                "SPX-T216",
                "record match arm must return a Copy i64 or bool value",
                first.value.span,
            ));
            return None;
        }
        return result;
    }
    let variant_instance = scrutinee_value.as_ref().and_then(|value| match &value.ty {
        Type::Named { name, arguments } if types.variant_cases(&value.ty).is_some() => {
            Some((name.clone(), arguments.clone()))
        }
        Type::I64
        | Type::I32
        | Type::Char
        | Type::U8
        | Type::Usize
        | Type::ArrayU8(_)
        | Type::F32
        | Type::F64
        | Type::Bool
        | Type::String
        | Type::Bytes
        | Type::Str
        | Type::SliceU8
        | Type::Named { .. } => None,
    });
    let variant_name = variant_instance.as_ref().map(|(name, _)| name.clone());
    let declared_cases = scrutinee_value
        .as_ref()
        .and_then(|value| types.variant_cases(&value.ty));
    if declared_cases.is_none() {
        diagnostics.push(error(
            program,
            "SPX-M103",
            format!(
                "match scrutinee must be a Copy variant, received {}",
                scrutinee_value.as_ref().map_or_else(
                    || "an invalid value".to_owned(),
                    |value| value.ty.to_string()
                )
            ),
            scrutinee.span,
        ));
    }

    let variant_needs_drop = scrutinee_value
        .as_ref()
        .is_some_and(|value| types.needs_drop(&value.ty));
    if let Some(scrutinee_value) = &scrutinee_value {
        match mode {
            MatchMode::Value if variant_needs_drop => diagnostics.push(error(
                program,
                "SPX-O111",
                "plain variant match requires a Copy scrutinee",
                scrutinee.span,
            )),
            MatchMode::Own
                if !variant_needs_drop
                    || !types.is_flat_owned_byte_variant(&scrutinee_value.ty)
                    || scrutinee_value.mode != ParamMode::Own =>
            {
                diagnostics.push(error(
                    program,
                    "SPX-O117",
                    "`match own` requires an owned admitted non-Copy variant scrutinee",
                    scrutinee.span,
                ));
            }
            MatchMode::Own if allow_moves => {
                mark_value_sources_moved(program, scrutinee, variables, types, diagnostics);
            }
            MatchMode::Own => diagnostics.push(error(
                program,
                "SPX-O105",
                "contract expression cannot consume a match scrutinee",
                scrutinee.span,
            )),
            MatchMode::Borrow
                if !variant_needs_drop
                    || !types.is_flat_owned_byte_variant(&scrutinee_value.ty)
                    || !matches!(scrutinee_value.mode, ParamMode::Own | ParamMode::Borrow)
                    || source_place(scrutinee, variables, types)
                        .is_none_or(|place| !place.projections.is_empty()) =>
            {
                diagnostics.push(error(
                    program,
                    "SPX-O117",
                    "`match borrow` requires an unprojected named owned or borrowed admitted non-Copy variant place",
                    scrutinee.span,
                ));
            }
            MatchMode::Borrow | MatchMode::Value => {}
        }
    }

    let outer_names = variables.keys().cloned().collect::<Vec<_>>();
    let mut arm_states = Vec::new();
    let mut covered = HashSet::new();
    let mut wildcard_seen = false;
    let mut result = None::<CheckedValue>;
    for arm in arms {
        let mut arm_variables = variables.clone();
        if *mode == MatchMode::Borrow {
            if let Some(place) = source_place(scrutinee, variables, types) {
                if place.projections.is_empty() {
                    activate_match_loan(&mut arm_variables, &place, expr.span);
                }
            }
        }
        match &arm.pattern {
            MatchPattern::Wildcard { span } => {
                if *mode != MatchMode::Value {
                    diagnostics.push(error(
                        program,
                        "SPX-O117",
                        "explicit ownership variant match requires every case pattern",
                        *span,
                    ));
                }
                if wildcard_seen || declared_cases.is_some_and(|cases| covered.len() == cases.len())
                {
                    diagnostics.push(error(
                        program,
                        "SPX-M102",
                        "unreachable wildcard match arm",
                        *span,
                    ));
                }
                wildcard_seen = true;
            }
            MatchPattern::Variant {
                type_name,
                case_name,
                fields,
                span,
                ..
            } => {
                let compatible = variant_name.as_deref() == Some(type_name.as_str());
                let declared_case = compatible
                    .then_some(declared_cases)
                    .flatten()
                    .and_then(|cases| cases.iter().find(|case| case.name == *case_name));
                if declared_case.is_none() {
                    diagnostics.push(error(
                        program,
                        "SPX-M103",
                        format!(
                            "pattern `{type_name}::{case_name}` is incompatible with the match scrutinee"
                        ),
                        *span,
                    ));
                } else if wildcard_seen || !covered.insert(case_name.as_str()) {
                    diagnostics.push(error(
                        program,
                        "SPX-M102",
                        format!("unreachable duplicate case `{type_name}::{case_name}`"),
                        *span,
                    ));
                }
                let mut supplied = HashSet::new();
                let mut bindings = HashSet::new();
                for field in fields {
                    let declared_field = declared_case.and_then(|case| {
                        case.fields
                            .iter()
                            .find(|candidate| candidate.name == field.name)
                    });
                    if !supplied.insert(field.name.as_str()) || declared_field.is_none() {
                        diagnostics.push(error(
                            program,
                            "SPX-M104",
                            format!(
                                "unknown or duplicate pattern field `{}` in `{type_name}::{case_name}`",
                                field.name
                            ),
                            field.span,
                        ));
                    }
                    if !source_identifier(&field.binding)
                        || !bindings.insert(field.binding.as_str())
                        || arm_variables.contains_key(&field.binding)
                    {
                        diagnostics.push(error(
                            program,
                            "SPX-M104",
                            format!("invalid or duplicate pattern binding `{}`", field.binding),
                            field.binding_span,
                        ));
                        continue;
                    }
                    if let Some(declared_field) = declared_field {
                        let binding_ty = variant_instance
                            .as_ref()
                            .and_then(|(name, arguments)| {
                                types.declaration(name).and_then(|declaration| {
                                    TypeTable::substitute_variant_type(
                                        declaration,
                                        arguments,
                                        &declared_field.ty,
                                    )
                                })
                            })
                            .unwrap_or_else(|| declared_field.ty.clone());
                        let binding_mode = if types.needs_drop(&binding_ty) {
                            match mode {
                                MatchMode::Own => ParamMode::Own,
                                MatchMode::Borrow => ParamMode::Borrow,
                                MatchMode::Value => ParamMode::Value,
                            }
                        } else {
                            ParamMode::Value
                        };
                        arm_variables.insert(
                            field.binding.clone(),
                            Binding {
                                ty: binding_ty,
                                mode: binding_mode,
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
                if let Some(declared_case) = declared_case {
                    for field in &declared_case.fields {
                        if !supplied.contains(field.name.as_str()) {
                            diagnostics.push(error(
                                program,
                                "SPX-M104",
                                format!(
                                    "pattern `{type_name}::{case_name}` is missing payload field `{}`",
                                    field.name
                                ),
                                *span,
                            ));
                        }
                    }
                }
            }
            MatchPattern::Record { span, .. } => diagnostics.push(error(
                program,
                "SPX-M103",
                "record pattern is incompatible with a variant scrutinee",
                *span,
            )),
            MatchPattern::Literal { span, .. }
            | MatchPattern::Or { span, .. }
            | MatchPattern::Binding { span, .. } => {
                diagnostics.push(error(
                    program,
                    "SPX-T254",
                    "refutable patterns are incompatible with an aggregate variant \
                     scrutinee",
                    *span,
                ));
            }
        }
        let arm_value = check_expr(
            program,
            current,
            &arm.value,
            &mut arm_variables,
            functions,
            types,
            result_type,
            allow_moves,
            diagnostics,
        );
        if let Some(value) = &arm_value {
            reject_native_unit_value(program, &arm.value, value, diagnostics);
        }
        if let Some(arm_value) = arm_value {
            if variant_needs_drop
                && (*mode == MatchMode::Value
                    || !matches!(arm_value.ty, Type::I64 | Type::Bool)
                    || arm_value.mode != ParamMode::Value)
            {
                diagnostics.push(error(
                    program,
                    "SPX-T216",
                    "owned variant match arms must return a Copy i64 or bool value",
                    arm.value.span,
                ));
            }
            if let Some(expected) = &result {
                if expected.ty != arm_value.ty || expected.mode != arm_value.mode {
                    diagnostics.push(error(
                        program,
                        "SPX-T216",
                        format!(
                            "match arms return incompatible values: {} and {}",
                            expected.ty, arm_value.ty
                        ),
                        arm.value.span,
                    ));
                }
            } else {
                result = Some(arm_value);
            }
        }
        arm_states.push(arm_variables);
    }
    if !wildcard_seen {
        if let (Some(variant_name), Some(cases)) = (&variant_name, declared_cases) {
            if let Some(missing) = cases
                .iter()
                .find(|case| !covered.contains(case.name.as_str()))
            {
                let witness = if missing.fields.is_empty() {
                    format!("{variant_name}::{} {{}}", missing.name)
                } else {
                    format!("{variant_name}::{} {{ .. }}", missing.name)
                };
                diagnostics.push(error(
                    program,
                    "SPX-M101",
                    format!("non-exhaustive match; missing case `{witness}`"),
                    expr.span,
                ));
            }
        }
    }
    if let Some((first, rest)) = arm_states.split_first() {
        let mut joined = first.clone();
        for state in rest {
            for name in &outer_names {
                if let (Some(joined_binding), Some(state_binding)) =
                    (joined.get_mut(name), state.get(name))
                {
                    joined_binding.availability =
                        joined_binding.availability.join(state_binding.availability);
                    joined_binding.moved_places = join_moved_places(joined_binding, state_binding);
                    joined_binding.definitely_partial =
                        join_definitely_partial(joined_binding, state_binding);
                }
            }
        }
        merge_moved(variables, &joined, &outer_names);
    }
    result
}
