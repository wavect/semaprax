//! Read-only field access never enters the ordinary String-copy evaluator.
use super::*;

pub(super) fn read(
    environment: &Environment,
    place: &hir::Place,
    bytes: bool,
) -> Result<Value, Flow> {
    let mut value = environment
        .get(&place.root)
        .ok_or(Flow::Guard("unresolved String view root"))?;
    for projection in &place.projections {
        let hir::PlaceProjection::Field(field) = projection else {
            return Err(Flow::Guard("invalid String view projection"));
        };
        let Value::Record(record) = value else {
            return Err(Flow::Guard("String view root is not a live record"));
        };
        value = record
            .fields
            .get(field)
            .ok_or(Flow::Guard("unresolved String view field"))?;
    }
    let Value::String(text) = value else {
        return Err(Flow::Guard("String view field is not live String storage"));
    };
    // This Arc is the evaluator's abstract immutable view, not a semantic
    // String allocation: no fresh owner, UTF-8 materialization charge or drop.
    let contents = Arc::from(text.as_bytes());
    Ok(if bytes {
        Value::BorrowedSlice(BorrowedSliceValue::whole(place.root.clone(), contents))
    } else {
        Value::BorrowedStr(BorrowedStrValue {
            invocation_root: place.root.clone(),
            bytes: contents,
        })
    })
}

#[cfg(test)]
mod tests;
