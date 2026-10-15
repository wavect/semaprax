//! Complete field evaluation and paid lazy Vec growth, with inline errors.
use super::{FieldInitializer, Span, TypeDeclaration};
use crate::ast::FieldDeclaration;
use crate::workspace_graph::constructor_failure::{receiving::Receiver, typed::Failure};
fn make(
    field: &FieldDeclaration,
    declarations: &[(&str, &TypeDeclaration)],
    allow_owned: bool,
    owner: &mut Receiver,
) -> Result<FieldInitializer, Failure> {
    let span = Span::default();
    owner.entry::<FieldInitializer>()?;
    owner.reserve(std::mem::size_of::<FieldInitializer>())?;
    Ok(FieldInitializer {
        name: owner.text(&field.name)?,
        name_span: span,
        value: super::typed::default_expr(&field.ty, declarations, allow_owned, owner)?,
        span,
    })
}
fn append(
    output: &mut Vec<FieldInitializer>,
    field: FieldInitializer,
    owner: &mut Receiver,
) -> Result<(), Failure> {
    owner.entry::<()>()?;
    owner.reserve(
        std::mem::size_of::<&mut Vec<FieldInitializer>>() + std::mem::size_of::<FieldInitializer>(),
    )?;
    owner.reserve(0)?;
    if output.len() == output.capacity() {
        // Same non-byte minimum4 and geometric growth as the old Result<Vec>
        // collection. Acquire only after this full field succeeds, before push.
        let capacity = output
            .capacity()
            .checked_mul(2)
            .ok_or_else(|| owner.physical())?
            .max(if std::mem::size_of::<FieldInitializer>() <= 1024 {
                4
            } else {
                1
            });
        let layout = std::alloc::Layout::array::<FieldInitializer>(capacity)
            .map_err(|_| owner.physical())?;
        owner.reserve(layout.size())?;
        output
            .try_reserve_exact(capacity - output.len())
            .map_err(|_| owner.physical())?;
        if output.capacity() != capacity {
            return Err(owner.physical().into());
        }
    }
    output.push(field);
    Ok(())
}
pub(in crate::workspace_graph) fn build(
    fields: &[FieldDeclaration],
    declarations: &[(&str, &TypeDeclaration)],
    allow_owned: bool,
    owner: &mut Receiver,
) -> Result<Vec<FieldInitializer>, Failure> {
    owner.entry::<Vec<FieldInitializer>>()?;
    owner.reserve(0)?;
    let Some((first, rest)) = fields.split_first() else {
        owner.reserve(std::mem::size_of::<Vec<FieldInitializer>>())?;
        return Ok(Vec::new());
    };
    // The old fallible iterator builds the complete first item before creating
    // its Vec. Preserve that semantic/error frontier before the modeled carrier.
    let first = make(first, declarations, allow_owned, owner)?;
    owner.reserve(std::mem::size_of::<Vec<FieldInitializer>>())?;
    let mut output = Vec::new();
    append(&mut output, first, owner)?;
    for field in rest {
        let field = make(field, declarations, allow_owned, owner)?;
        append(&mut output, field, owner)?;
    }
    owner.reserve(0)?;
    Ok(output)
}
