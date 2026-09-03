//! Source places and their availability: resolving an expression to a rooted
//! projection path, and the partial-move lattice over those paths.

use super::binding::{Availability, Binding};
use super::diagnostics::error;
use super::type_table::TypeTable;
use crate::ast::{Expr, ExprKind, ParamMode, Program, Span, Type};
use crate::diagnostic::Diagnostic;
use std::collections::{HashMap, HashSet};

#[derive(Clone)]
pub(super) struct SourcePlace {
    pub(super) root: String,
    pub(super) root_span: Span,
    pub(super) projections: Vec<String>,
    pub(super) ty: Type,
    pub(super) mode: ParamMode,
}

pub(super) fn source_place(
    expr: &Expr,
    variables: &HashMap<String, Binding>,
    types: &TypeTable<'_>,
) -> Option<SourcePlace> {
    let mut current = expr;
    let mut projected = Vec::new();
    while let ExprKind::Project { base, field, .. } = &current.kind {
        projected.push(field.as_str());
        current = base;
    }
    let ExprKind::Var(name) = &current.kind else {
        return None;
    };
    let binding = variables.get(name)?;
    let mut place = SourcePlace {
        root: name.clone(),
        root_span: current.span,
        projections: Vec::with_capacity(projected.len()),
        ty: binding.ty.clone(),
        mode: binding.mode,
    };
    for field in projected.into_iter().rev() {
        let declared = types
            .record_fields(&place.ty)?
            .iter()
            .find(|candidate| candidate.name == field)?;
        place.ty = types.record_field_type(&place.ty, declared)?;
        if !types.needs_drop(&place.ty) {
            place.mode = ParamMode::Value;
        }
        place.projections.push(field.to_owned());
    }
    Some(place)
}

pub(super) fn check_source_place_availability(
    program: &Program,
    place: &SourcePlace,
    variables: &HashMap<String, Binding>,
    span: Span,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let Some(binding) = variables.get(&place.root) else {
        return;
    };
    match binding.availability {
        Availability::Moved => {
            diagnostics.push(
                error(
                    program,
                    "SPX-O101",
                    format!("use of resource `{}` after ownership was moved", place.root),
                    place.root_span,
                )
                .with_help("borrow the resource if the callee does not need ownership"),
            );
            return;
        }
        Availability::MaybeMoved => {
            diagnostics.push(
                error(
                    program,
                    "SPX-O107",
                    format!(
                        "resource `{}` may have been moved on another control-flow path",
                        place.root
                    ),
                    place.root_span,
                )
                .with_help("move the resource on every path or keep it borrowed"),
            );
            return;
        }
        Availability::Available => {}
    }
    let state = overlapping_place_state(binding, &place.projections);
    let display = format!("{}.{}", place.root, place.projections.join("."));
    match state {
        Availability::Available => {}
        Availability::Moved => diagnostics.push(
            error(
                program,
                "SPX-O109",
                format!("use of partially moved place `{display}`"),
                span,
            )
            .with_help("use an available sibling field or avoid moving this place earlier"),
        ),
        Availability::MaybeMoved => diagnostics.push(
            error(
                program,
                "SPX-O110",
                format!("place `{display}` may have been moved on another control-flow path"),
                span,
            )
            .with_help("move the field on every path or keep it borrowed"),
        ),
    }
}

pub(super) fn overlapping_place_state(binding: &Binding, requested: &[String]) -> Availability {
    if binding.availability != Availability::Available {
        return binding.availability;
    }
    let mut maybe_moved = false;
    for (moved, state) in &binding.moved_places {
        if path_is_prefix(moved, requested) || path_is_prefix(requested, moved) {
            if *state == Availability::Moved {
                return Availability::Moved;
            }
            maybe_moved = true;
        }
    }
    if binding
        .definitely_partial
        .iter()
        .any(|partial| path_is_prefix(requested, partial))
    {
        return Availability::Moved;
    }
    if maybe_moved {
        Availability::MaybeMoved
    } else {
        Availability::Available
    }
}

pub(super) fn path_is_prefix<T: PartialEq>(prefix: &[T], path: &[T]) -> bool {
    prefix.len() <= path.len() && prefix.iter().zip(path).all(|(left, right)| left == right)
}

pub(super) fn join_moved_places(
    left: &Binding,
    right: &Binding,
) -> HashMap<Vec<String>, Availability> {
    left.moved_places
        .keys()
        .chain(right.moved_places.keys())
        .cloned()
        .collect::<HashSet<_>>()
        .into_iter()
        .filter_map(|path| {
            let left = left
                .moved_places
                .get(&path)
                .copied()
                .unwrap_or(Availability::Available);
            let right = right
                .moved_places
                .get(&path)
                .copied()
                .unwrap_or(Availability::Available);
            let state = left.join(right);
            (state != Availability::Available).then_some((path, state))
        })
        .collect()
}

pub(super) fn join_definitely_partial(left: &Binding, right: &Binding) -> HashSet<Vec<String>> {
    let mut candidates = HashSet::new();
    for path in left
        .moved_places
        .keys()
        .chain(right.moved_places.keys())
        .chain(left.definitely_partial.iter())
        .chain(right.definitely_partial.iter())
    {
        for length in 0..=path.len() {
            candidates.insert(path[..length].to_vec());
        }
    }
    candidates
        .into_iter()
        .filter(|path| {
            overlapping_place_state(left, path) == Availability::Moved
                && overlapping_place_state(right, path) == Availability::Moved
        })
        .collect()
}
