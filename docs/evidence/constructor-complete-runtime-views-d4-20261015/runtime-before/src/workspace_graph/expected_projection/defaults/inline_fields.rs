//! Complete field evaluation and paid lazy Vec growth, with inline errors.
use super::{active_builder_limit, limit_error, FieldInitializer, Span, TypeDeclaration};
use crate::ast::FieldDeclaration;
use crate::workspace_graph::constructor_failure::{reserve, Failure};
fn physical() -> Failure {
    crate::bounded_output::refuse_active_required(usize::MAX);
    Failure::one(limit_error("builder_bytes", active_builder_limit()))
}
fn make(
    field: &FieldDeclaration,
    declarations: &[(&str, &TypeDeclaration)],
    allow_owned: bool,
) -> Result<FieldInitializer, Failure> {
    let span = Span::default();
    reserve(std::mem::size_of::<Result<FieldInitializer, Failure>>())?;
    reserve(std::mem::size_of::<FieldInitializer>())?;
    Ok(FieldInitializer {
        name: crate::bounded_output::budgeted_clone(&field.name),
        name_span: span,
        value: super::inline::default_expr(&field.ty, declarations, allow_owned)?,
        span,
    })
}
fn append(output: &mut Vec<FieldInitializer>, field: FieldInitializer) -> Result<(), Failure> {
    reserve(std::mem::size_of::<Result<(), Failure>>())?;
    reserve(0)?;
    if output.len() == output.capacity() {
        // Same non-byte minimum4 and geometric growth as the old Result<Vec>
        // collection. Acquire only after this full field succeeds, before push.
        let capacity = output.capacity().checked_mul(2).ok_or_else(physical)?.max(
            if std::mem::size_of::<FieldInitializer>() <= 1024 {
                4
            } else {
                1
            },
        );
        let layout =
            std::alloc::Layout::array::<FieldInitializer>(capacity).map_err(|_| physical())?;
        reserve(layout.size())?;
        output
            .try_reserve_exact(capacity - output.len())
            .map_err(|_| physical())?;
        if output.capacity() != capacity {
            return Err(physical());
        }
    }
    output.push(field);
    Ok(())
}
pub(in crate::workspace_graph) fn build(
    fields: &[FieldDeclaration],
    declarations: &[(&str, &TypeDeclaration)],
    allow_owned: bool,
) -> Result<Vec<FieldInitializer>, Failure> {
    reserve(std::mem::size_of::<Result<Vec<FieldInitializer>, Failure>>())?;
    reserve(0)?;
    let Some((first, rest)) = fields.split_first() else {
        reserve(std::mem::size_of::<Vec<FieldInitializer>>())?;
        return Ok(Vec::new());
    };
    // The old fallible iterator builds the complete first item before creating
    // its Vec. Preserve that semantic/error frontier before the modeled carrier.
    let first = make(first, declarations, allow_owned)?;
    reserve(std::mem::size_of::<Vec<FieldInitializer>>())?;
    let mut output = Vec::new();
    append(&mut output, first)?;
    for field in rest {
        append(&mut output, make(field, declarations, allow_owned)?)?;
    }
    reserve(0)?;
    Ok(output)
}
