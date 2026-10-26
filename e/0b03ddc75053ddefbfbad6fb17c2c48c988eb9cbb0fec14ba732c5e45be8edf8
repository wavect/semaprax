//! Actual lazy field Vec backing at the complete old successful-item frontier.
//! Error Vec/text remain the unchanged diagnostic API and an explicit residual.
use super::{
    active_builder_limit, limit_error, reserve_builder_structure, Diagnostic, FieldInitializer,
    Span, TypeDeclaration,
};
use crate::ast::FieldDeclaration;
fn physical() -> Vec<Diagnostic> {
    crate::bounded_output::refuse_active_required(usize::MAX);
    vec![limit_error("builder_bytes", active_builder_limit())]
}
fn make(
    field: &FieldDeclaration,
    declarations: &[(&str, &TypeDeclaration)],
    allow_owned: bool,
) -> Result<FieldInitializer, Vec<Diagnostic>> {
    let span = Span::default();
    reserve_builder_structure(std::mem::size_of::<FieldInitializer>())?;
    Ok(FieldInitializer {
        name: crate::bounded_output::budgeted_clone(&field.name),
        name_span: span,
        value: super::default_expr(&field.ty, declarations, allow_owned)?,
        span,
    })
}
fn append(
    output: &mut Vec<FieldInitializer>,
    field: FieldInitializer,
) -> Result<(), Vec<Diagnostic>> {
    reserve_builder_structure(0)?;
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
        reserve_builder_structure(layout.size())?;
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
pub(super) fn build(
    fields: &[FieldDeclaration],
    declarations: &[(&str, &TypeDeclaration)],
    allow_owned: bool,
) -> Result<Vec<FieldInitializer>, Vec<Diagnostic>> {
    reserve_builder_structure(0)?;
    let Some((first, rest)) = fields.split_first() else {
        reserve_builder_structure(std::mem::size_of::<Vec<FieldInitializer>>())?;
        return Ok(Vec::new());
    };
    // The old fallible iterator builds the complete first item before creating
    // its Vec. Preserve that semantic/error frontier before the modeled carrier.
    let first = make(first, declarations, allow_owned)?;
    reserve_builder_structure(std::mem::size_of::<Vec<FieldInitializer>>())?;
    let mut output = Vec::new();
    append(&mut output, first)?;
    for field in rest {
        append(&mut output, make(field, declarations, allow_owned)?)?;
    }
    reserve_builder_structure(0)?;
    Ok(output)
}
#[cfg(test)]
#[path = "field_directory/tests.rs"]
mod tests;

#[cfg(test)]
pub(in crate::workspace_graph::expected_projection) fn project_original_receiver(
    path: &str,
    source: &str,
) -> String {
    let records: serde_json::Value = serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/docs/evidence/constructor-value-owners-824/exact-source-inverses.json"
    )))
    .unwrap();
    let record = records
        .as_array()
        .unwrap()
        .iter()
        .find(|r| r["source"].as_str() == Some(path))
        .unwrap();
    let mut current = source.to_owned();
    for edit in record["edits"].as_array().unwrap().iter().rev() {
        let after = edit["after"].as_str().unwrap();
        assert_eq!(current.matches(after).count(), 1);
        current = current.replacen(after, edit["before"].as_str().unwrap(), 1);
    }
    current
}
