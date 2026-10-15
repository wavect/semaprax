//! Full ancestor freezes over every subtree, including nested lowered code.
//! Only borrowed lookup rows are sorted. No traversal skips an inner body.
use super::{
    DeclarationIndex, Event, Failure, ResolvedExpr, ResolvedFunction, Row, Storage, Walker, index,
};
use super::super::{
    ResolvedExprKind, ResolvedMatchPattern, ResolvedStatement, ValueId, bare_place, receiver_shape,
};
use super::intervals::{self, Freeze, Kind};
#[derive(Clone, Copy)]
struct Receiver<'a> {
    expression: &'a ResolvedExpr,
}
pub(super) fn validate<'a>(
    function: &'a ResolvedFunction,
    rows: &[Row<'a>],
    index: &index::Index<'a>,
    declarations: Option<&DeclarationIndex>,
    storage: &Storage,
    walker: &mut Walker<'a>,
) -> Result<(), Failure> {
    let mut count = 0usize;
    for row in rows {
        let owner = owner(row, declarations)?;
        count = count
            .checked_add(7 + usize::from(owner.is_some()))
            .ok_or_else(|| Failure::semantic("borrowed byte traversal freeze count overflows"))?;
    }
    let mut freezes = storage.vector(count)?;
    let mut receivers = storage.vector(rows.len())?;
    let mut ancestors: Vec<usize> = storage.vector(rows.len())?;
    for (position, row) in rows.iter().enumerate() {
        while ancestors
            .last()
            .is_some_and(|parent| row.start >= rows[*parent].body_end)
        {
            ancestors.pop();
        }
        if ancestors.last().is_some_and(|parent| {
            row.start < rows[*parent].body_start || row.end > rows[*parent].body_end
        }) {
            return Err(Failure::semantic(
                "borrowed byte traversal region crosses its enclosing authored body",
            ));
        }
        let entry = ancestors
            .first()
            .map_or(row.start, |parent| rows[*parent].start);
        index.available(row.shape.view, entry)?;
        for id in row.shape.frozen {
            storage.push(
                &mut freezes,
                Freeze {
                    id,
                    kind: Kind::Rebind,
                    start: row.body_start,
                    end: row.body_end,
                    prefix_end: 0,
                },
            )?;
        }
        if let Some(id) = owner(row, declarations)? {
            storage.push(
                &mut freezes,
                Freeze {
                    id,
                    kind: Kind::Rebind,
                    start: row.body_start,
                    end: row.body_end,
                    prefix_end: 0,
                },
            )?;
        }
        storage.push(
            &mut freezes,
            Freeze {
                id: &row.shape.payload.id,
                kind: Kind::HiddenPayloadRead,
                start: row.body_start,
                end: row.body_end,
                prefix_end: 0,
            },
        )?;
        storage.push(
            &mut receivers,
            Receiver {
                expression: row.shape.receiver,
            },
        )?;
        storage.push(&mut ancestors, position)?;
    }
    intervals::prepare(&mut freezes);
    receivers.sort_unstable_by(|a, b| a.expression.id.cmp(&b.expression.id));
    let mut unauthorized = false;
    walker.function(function, |event| {
        let Event::Enter { expression, position } = event else { return Ok(()); };
        if matches!(&expression.kind, ResolvedExprKind::Place(place) | ResolvedExprKind::BorrowPlace { place, .. } if intervals::active(&freezes, &place.root, Kind::HiddenPayloadRead, position)) {
            return Err(Failure::semantic("borrowed byte receiver payload escapes its receiver"));
        }
        if let ResolvedExprKind::Block { statements, .. } | ResolvedExprKind::VecConstructor { statements, .. } = &expression.kind {
            for statement in statements {
                if let ResolvedStatement::Let { binding, .. } | ResolvedStatement::Assign { binding, .. } = statement {
                    if intervals::active(&freezes, &binding.id, Kind::Rebind, position) {
                        return Err(Failure::semantic("borrowed byte traversal view, owner, counter or item is mutated or rebound"));
                    }
                }
            }
        }
        if let ResolvedExprKind::Match { arms, .. } = &expression.kind {
            for arm in arms {
                if let ResolvedMatchPattern::Variant { fields, .. } = &arm.pattern {
                    if fields.iter().any(|field| intervals::active(&freezes, &field.binding.id, Kind::Rebind, position)) {
                        return Err(Failure::semantic("borrowed byte traversal binding is shadowed"));
                    }
                }
            }
        }
        if receiver_shape(expression).is_some() {
            let admitted = receivers.binary_search_by(|row| row.expression.id.cmp(&expression.id)).ok().is_some_and(|index| std::ptr::eq(receivers[index].expression, expression));
            unauthorized |= !admitted;
        }
        Ok(())
    })?;
    if unauthorized {
        return Err(Failure::semantic(
            "borrowed byte receiver is outside an authenticated Slice<u8> traversal",
        ));
    }
    Ok(())
}
fn owner<'a>(
    row: &Row<'_>,
    declarations: Option<&'a DeclarationIndex>,
) -> Result<Option<&'a ValueId>, Failure> {
    let Some(declarations) = declarations else {
        return Ok(None);
    };
    let view = bare_place(row.shape.view)
        .ok_or_else(|| Failure::semantic("borrowed byte traversal view is not a bare place"))?;
    let provenance = declarations
        .byte_slice_provenance(&view.root)
        .ok_or_else(|| {
            Failure::semantic(
                "borrowed byte traversal view lacks authenticated byte-slice provenance",
            )
        })?;
    Ok((provenance.root != view.root).then_some(&provenance.root))
}
