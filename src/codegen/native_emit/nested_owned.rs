//! Result-shell publication for validated nested owned-byte records.

use crate::aggregate_layout::AggregateLayoutCache;
use crate::diagnostic::Diagnostic;
use crate::hir::{DeclarationId, DeclarationKind, ResolvedProgram, ResolvedType};
use std::collections::BTreeSet;

use crate::variant_layout::{VariantFieldValueKind, VariantLayout, VariantLayoutCache};

use super::{backend_error, c_case_symbol, c_field_symbol, variant_declaration_id, COutput};

pub(super) fn borrowed_aggregate_byte_paths(
    program: &ResolvedProgram,
    record_layouts: &AggregateLayoutCache,
    variant_layouts: &VariantLayoutCache,
    ty: &ResolvedType,
) -> Result<Vec<Vec<DeclarationId>>, Diagnostic> {
    if is_exact_record(program, ty)? {
        let mut output = Vec::new();
        collect_record_paths(program, record_layouts, ty, &mut output)?;
        return Ok(output);
    }
    if variant_declaration_id(program, ty)?.is_some() {
        let layout = variant_layouts.layout(ty)?;
        layout.validate(program)?;
        return Ok(layout
            .cases
            .iter()
            .flat_map(|case| {
                case.fields
                    .iter()
                    .filter(|field| matches!(field.ty, ResolvedType::Bytes))
                    .map(|field| vec![case.case.clone(), field.field.clone()])
            })
            .collect());
    }
    Ok(Vec::new())
}

pub(super) fn borrowed_aggregate_path_suffix(path: &[DeclarationId]) -> Result<String, Diagnostic> {
    if path.is_empty() {
        return Err(backend_error("borrowed aggregate byte path is empty"));
    }
    if let [case, field] = path {
        return Ok(format!("{}_{}", c_case_symbol(case), c_field_symbol(field)));
    }
    Ok(path
        .iter()
        .map(c_field_symbol)
        .collect::<Vec<_>>()
        .join("_"))
}

fn collect_record_paths(
    program: &ResolvedProgram,
    layouts: &AggregateLayoutCache,
    root: &ResolvedType,
    output: &mut Vec<Vec<DeclarationId>>,
) -> Result<(), Diagnostic> {
    enum Frame {
        Record(ResolvedType, Vec<DeclarationId>, usize),
        Bytes(Vec<DeclarationId>),
        Leave(DeclarationId),
    }
    let mut pending = vec![Frame::Record(root.clone(), Vec::new(), 1)];
    let mut active = BTreeSet::new();
    let mut fields = 0usize;
    while let Some(frame) = pending.pop() {
        match frame {
            Frame::Record(ty, prefix, depth) => {
                if depth > crate::cleanup::MAX_CLEANUP_SHAPE_DEPTH
                    || !is_exact_record(program, &ty)?
                {
                    return Err(backend_error(
                        "closed or over-depth record reached borrowed aggregate byte paths",
                    ));
                }
                let layout = layouts.layout(&ty)?;
                layout.validate(program)?;
                if !active.insert(layout.record.clone()) {
                    return Err(backend_error(
                        "cyclic record reached borrowed aggregate byte paths",
                    ));
                }
                fields = fields
                    .checked_add(layout.fields.len())
                    .ok_or_else(|| backend_error("borrowed aggregate field count overflowed"))?;
                if fields > crate::cleanup::MAX_CLEANUP_VISITED_FIELDS {
                    return Err(backend_error(
                        "borrowed aggregate byte paths exceed the field limit",
                    ));
                }
                pending.push(Frame::Leave(layout.record.clone()));
                for field in layout.fields.iter().rev() {
                    let mut path = prefix.clone();
                    path.push(field.field.clone());
                    if field.ty == ResolvedType::Bytes {
                        pending.push(Frame::Bytes(path));
                    } else if is_exact_record(program, &field.ty)? {
                        pending.push(Frame::Record(field.ty.clone(), path, depth + 1));
                    } else if matches!(field.ty, ResolvedType::Nominal { .. }) {
                        return Err(backend_error(
                            "non-record nominal reached nested borrowed byte paths",
                        ));
                    }
                }
            }
            Frame::Bytes(path) => {
                if output.len() >= crate::cleanup::MAX_CLEANUP_OWNED_LEAVES {
                    return Err(backend_error(
                        "borrowed aggregate byte paths exceed the owned-leaf limit",
                    ));
                }
                output.push(path);
            }
            Frame::Leave(record) => {
                active.remove(&record);
            }
        }
    }
    Ok(())
}

pub(super) fn emit_owned_record_shell(
    output: &mut impl COutput,
    program: &ResolvedProgram,
    layouts: &AggregateLayoutCache,
    destination: &str,
    source: &str,
    ty: &ResolvedType,
    _active: &mut BTreeSet<DeclarationId>,
) -> Result<(), Diagnostic> {
    enum Frame {
        Record(String, String, ResolvedType, usize),
        Bytes(String),
        Scalar(String, String),
        Leave(DeclarationId),
    }
    let mut pending = vec![Frame::Record(
        destination.to_owned(),
        source.to_owned(),
        ty.clone(),
        1,
    )];
    let mut active = BTreeSet::new();
    let mut fields = 0usize;
    let mut leaves = 0usize;
    while let Some(frame) = pending.pop() {
        match frame {
            Frame::Record(destination, source, ty, depth) => {
                if depth > crate::cleanup::MAX_CLEANUP_SHAPE_DEPTH
                    || !is_exact_record(program, &ty)?
                {
                    return Err(backend_error(
                        "closed or over-depth record reached owned result publication",
                    ));
                }
                let layout = layouts.layout(&ty)?;
                layout.validate(program)?;
                if !active.insert(layout.record.clone()) {
                    return Err(backend_error(
                        "cyclic record reached owned result publication",
                    ));
                }
                fields = fields
                    .checked_add(layout.fields.len())
                    .ok_or_else(|| backend_error("owned result field count overflowed"))?;
                if fields > crate::cleanup::MAX_CLEANUP_VISITED_FIELDS {
                    return Err(backend_error(
                        "owned result publication exceeds its field limit",
                    ));
                }
                pending.push(Frame::Leave(layout.record.clone()));
                for field in layout.fields.iter().rev() {
                    if field.size == 0 {
                        continue;
                    }
                    let symbol = c_field_symbol(&field.field);
                    let destination = format!("({destination}).{symbol}");
                    let source = format!("({source}).{symbol}");
                    if field.ty == ResolvedType::Bytes {
                        pending.push(Frame::Bytes(destination));
                    } else if is_exact_record(program, &field.ty)? {
                        pending.push(Frame::Record(
                            destination,
                            source,
                            field.ty.clone(),
                            depth + 1,
                        ));
                    } else if matches!(
                        field.ty,
                        ResolvedType::Nominal { .. } | ResolvedType::String
                    ) {
                        return Err(backend_error(
                            "closed field kind reached nested owned result publication",
                        ));
                    } else {
                        pending.push(Frame::Scalar(destination, source));
                    }
                }
            }
            Frame::Bytes(destination) => {
                leaves = leaves
                    .checked_add(1)
                    .ok_or_else(|| backend_error("owned result leaf count overflowed"))?;
                if leaves > crate::cleanup::MAX_CLEANUP_OWNED_LEAVES {
                    return Err(backend_error(
                        "owned result publication exceeds its owned-leaf limit",
                    ));
                }
                writeln!(output, "    {destination} = (spx_bytes_v1) {{0}};")
                    .expect("writing to a string cannot fail");
            }
            Frame::Scalar(destination, source) => {
                writeln!(output, "    {destination} = {source};")
                    .expect("writing to a string cannot fail");
            }
            Frame::Leave(record) => {
                active.remove(&record);
            }
        }
    }
    Ok(())
}

pub(super) fn emit_owned_variant_shell(
    output: &mut impl COutput,
    program: &ResolvedProgram,
    layout: &VariantLayout,
    destination: &str,
    source: &str,
) -> Result<(), Diagnostic> {
    layout.validate(program)?;
    writeln!(
        output,
        "    if (({source}).spx_tag >= UINT32_C({})) spx_runtime_invariant_failure(\"invalid owned variant result tag\");",
        layout.cases.len()
    )
    .expect("writing to a string cannot fail");
    writeln!(
        output,
        "    memset(&({destination}), 0, sizeof({destination}));"
    )
    .expect("writing to a string cannot fail");
    writeln!(output, "    ({destination}).spx_tag = ({source}).spx_tag;")
        .expect("writing to a string cannot fail");
    for case in &layout.cases {
        writeln!(
            output,
            "    if (({source}).spx_tag == UINT32_C({})) {{",
            case.tag
        )
        .expect("writing to a string cannot fail");
        for field in &case.fields {
            if field.size == 0 || field.value_kind == VariantFieldValueKind::OwnedBytes {
                continue;
            }
            let case_symbol = c_case_symbol(&case.case);
            let field_symbol = c_field_symbol(&field.field);
            writeln!(
                output,
                "        ({destination}).spx_payload.{case_symbol}.{field_symbol} = ({source}).spx_payload.{case_symbol}.{field_symbol};"
            )
            .expect("writing to a string cannot fail");
        }
        output.push_str("    }\n");
    }
    Ok(())
}

fn is_exact_record(program: &ResolvedProgram, ty: &ResolvedType) -> Result<bool, Diagnostic> {
    let ResolvedType::Nominal {
        declaration,
        arguments,
    } = ty
    else {
        return Ok(false);
    };
    let item = program
        .declarations
        .declaration(declaration)
        .ok_or_else(|| backend_error(format!("unknown native type `{declaration}`")))?;
    Ok(arguments.is_empty()
        && program
            .declarations
            .type_parameters(declaration)
            .is_some_and(|parameters| parameters.is_empty())
        && item.kind == DeclarationKind::Record)
}
