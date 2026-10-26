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
    // Authenticate the complete live source before selecting the independently
    // reconstructed stage consumed by the unchanged original inverse below.
    const STAGES: &[(&str, &str)] = &[
        ("src/vec_ops/wrappers.rs", "50698a52b7372660a11481ef062ce222aa3a772aa06d2d1f8a8d6e90e637b8c4"),
        ("src/box_ops/wrappers.rs", "62c12326214532a8cdc2d0cf8097c52159ed2bacb19e16af98685a31d9d47492"),
        ("src/workspace_graph/expected_projection/defaults.rs", "e97250b91fc9c8bb8f60e6ea1ef7d90a332dcb9dcf3fffdbd66348a9f3e2a366"),
        ("src/workspace_graph/expected_projection/import_stub.rs", "047c449c4f14513e0288d0cefd71d101aa3fa72794ec3dcb5d9a4a9320737e55"),
        ("src/workspace_graph/expected_projection/borrowed_imports/tests/old.rs", "93bee992f5f49c009e9c40cda5c630fb823dcf109b082c91b7a57b584d798fa1"),
        ("src/workspace_graph/expected_projection/borrowed_imports/tests/full/original_construct.rs", "6c6f059f2a15f756a6c374bbc3cdf89cff93127016bdbe9c438d9520a08d1954"),
        ("src/workspace_graph/expected_projection/borrowed_imports/tests/full/publication.rs", "6b4b920be8abbed49936921dbaedc293e97aee2f6a7d517e95fef3430d2c4229"),
    ];
    let expected = STAGES.iter().find(|(name, _)| *name == path).unwrap().1;
    let mut current =
        crate::source_reflows_merge::main_e177_final_inputs::value_direct(path, expected, source);
    for edit in record["edits"].as_array().unwrap().iter().rev() {
        let after = edit["after"].as_str().unwrap();
        assert_eq!(current.matches(after).count(), 1);
        current = current.replacen(after, edit["before"].as_str().unwrap(), 1);
    }
    current
}
