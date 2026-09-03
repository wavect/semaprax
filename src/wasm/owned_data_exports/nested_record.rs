//! Project-v11 nested record layout authentication and opaque carrier lowering.

use crate::aggregate_layout::{AggregateFieldValueKind, AggregateLayoutCache, AggregateTarget};
use crate::diagnostic::Diagnostic;
use crate::hir::{ResolvedProgram, ResolvedType};
use crate::project::{
    NestedOwnedRecordApiDescriptor, NestedOwnedRecordLeafType, PublicApiParameterType,
};

use super::{
    error, load_i32, load_i64, local_get, local_set, poison_temporary, raw_symbol, store_i32,
    store_i64, FlatRecordFieldKind, OwnedDataExportPlan, ParameterType, ResultLayout,
};

const MAX_DEPTH: usize = 64;
const MAX_LEAVES: usize = 4_096;
const MAX_OWNED_LEAVES: usize = 256;

#[derive(Clone, Debug, Eq, PartialEq)]
pub(in crate::wasm) struct NestedRecordLayout {
    pub(super) private_size: u32,
    pub(super) public_size: u32,
    pub(super) leaves: Vec<NestedLeafLayout>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(in crate::wasm) struct NestedLeafLayout {
    source_offset: u32,
    public_offset: u32,
    kind: FlatRecordFieldKind,
}

pub(super) fn prepare(
    program: &ResolvedProgram,
    descriptor: &NestedOwnedRecordApiDescriptor,
) -> Result<Vec<OwnedDataExportPlan>, Diagnostic> {
    crate::hir::validate(program)?;
    let layouts = AggregateLayoutCache::build(program, AggregateTarget::Wasm32)?;
    descriptor
        .exports()
        .iter()
        .map(|export| {
            let function = program
                .functions
                .iter()
                .find(|function| function.id == *export.stable_id())
                .ok_or_else(|| error("nested record descriptor target is absent from held HIR"))?;
            let root = layouts.layout(&function.return_type)?;
            root.validate(program)?;
            if root.record != *export.result_record_id() || root.align != 8 {
                return Err(error("nested record Wasm32 root layout disagrees"));
            }
            let declared = independently_flatten(program, &layouts, &function.return_type)?;
            if declared.len() != export.leaves().len() || declared.len() > MAX_LEAVES {
                return Err(error("nested record leaf inventory disagrees"));
            }
            let mut leaves = Vec::with_capacity(declared.len());
            for ((path, source_offset, kind), described) in
                declared.into_iter().zip(export.leaves())
            {
                let described_kind = leaf_kind(described.ty());
                if path != described.field_path()
                    || kind != described_kind
                    || described.ordinal() as usize != leaves.len()
                {
                    return Err(error("nested record descriptor leaf is not canonical"));
                }
                leaves.push(NestedLeafLayout {
                    source_offset,
                    public_offset: described
                        .ordinal()
                        .checked_mul(8)
                        .ok_or_else(|| error("nested record public offset overflows"))?,
                    kind,
                });
            }
            if !leaves
                .iter()
                .any(|leaf| leaf.kind == FlatRecordFieldKind::OwnedBytes)
            {
                return Err(error("nested record carrier has no owned leaf"));
            }
            if leaves
                .iter()
                .filter(|leaf| leaf.kind == FlatRecordFieldKind::OwnedBytes)
                .count()
                > MAX_OWNED_LEAVES
            {
                return Err(error(
                    "nested record owned-leaf inventory exceeds its bound",
                ));
            }
            let public_size = u32::try_from(leaves.len())
                .ok()
                .and_then(|count| count.checked_mul(8))
                .ok_or_else(|| error("nested record public carrier size overflows"))?;
            let parameters = export
                .parameters()
                .iter()
                .map(|(_, _, parameter)| match parameter {
                    PublicApiParameterType::I64 => ParameterType::I64,
                    PublicApiParameterType::Bool => ParameterType::Bool,
                    PublicApiParameterType::BorrowStr => ParameterType::BorrowStr,
                    PublicApiParameterType::BorrowSliceU8 => ParameterType::BorrowSliceU8,
                })
                .collect();
            Ok(OwnedDataExportPlan {
                stable_id: export.stable_id().as_str().to_owned(),
                wasm_export: raw_symbol(export.stable_id().as_str()),
                function_id: export.stable_id().clone(),
                parameters,
                result: ResultLayout::NestedRecord(NestedRecordLayout {
                    private_size: root.size,
                    public_size,
                    leaves,
                }),
            })
        })
        .collect()
}

type DerivedLeaf = (Vec<crate::hir::DeclarationId>, u32, FlatRecordFieldKind);

fn independently_flatten(
    program: &ResolvedProgram,
    layouts: &AggregateLayoutCache,
    root: &ResolvedType,
) -> Result<Vec<DerivedLeaf>, Diagnostic> {
    let mut output = Vec::new();
    let mut work = vec![(root.clone(), 0_u32, Vec::new(), 1_usize, 0_usize)];
    let mut visited_fields = 0_usize;
    while !work.is_empty() {
        let index = work.len() - 1;
        let (ty, base, prefix, depth, field_index) = &mut work[index];
        if *depth > MAX_DEPTH {
            return Err(error("nested record layout exceeds its depth bound"));
        }
        let layout = layouts.layout(ty)?;
        layout.validate(program)?;
        if *field_index == layout.fields.len() {
            work.pop();
            continue;
        }
        let field = layout.fields[*field_index].clone();
        *field_index += 1;
        visited_fields = visited_fields
            .checked_add(1)
            .filter(|count| *count <= MAX_LEAVES)
            .ok_or_else(|| error("nested record physical inventory exceeds its bound"))?;
        let offset = base
            .checked_add(field.offset)
            .ok_or_else(|| error("nested record source offset overflows"))?;
        let mut path = prefix.clone();
        path.push(field.field.clone());
        match field.value_kind {
            AggregateFieldValueKind::Aggregate => {
                let child_depth = *depth + 1;
                work.push((field.ty, offset, path, child_depth, 0));
            }
            AggregateFieldValueKind::OwnedBytes if field.ty == ResolvedType::Bytes => {
                output.push((path, offset, FlatRecordFieldKind::OwnedBytes));
            }
            AggregateFieldValueKind::Copy => {
                let kind = match field.ty {
                    ResolvedType::I64 => FlatRecordFieldKind::I64,
                    ResolvedType::Bool => FlatRecordFieldKind::Bool,
                    ResolvedType::Usize => FlatRecordFieldKind::Usize,
                    _ => {
                        return Err(error(
                            "nested record Copy leaf representation is unsupported",
                        ))
                    }
                };
                output.push((path, offset, kind));
            }
            _ => {
                return Err(error(
                    "nested record physical leaf representation is unsupported",
                ))
            }
        }
    }
    Ok(output)
}

fn leaf_kind(kind: NestedOwnedRecordLeafType) -> FlatRecordFieldKind {
    match kind {
        NestedOwnedRecordLeafType::I64 => FlatRecordFieldKind::I64,
        NestedOwnedRecordLeafType::Bool => FlatRecordFieldKind::Bool,
        NestedOwnedRecordLeafType::Usize => FlatRecordFieldKind::Usize,
        NestedOwnedRecordLeafType::OwnedBytes => FlatRecordFieldKind::OwnedBytes,
    }
}

#[allow(clippy::too_many_arguments)]
pub(super) fn emit_publication(
    body: &mut Vec<u8>,
    layout: &NestedRecordLayout,
    temporary_out: u32,
    result_out: u32,
    private_size: u32,
    charged: u32,
    scalar: u32,
    old_stack: u32,
    bytes_drop_index: u32,
) {
    for leaf in layout
        .leaves
        .iter()
        .filter(|leaf| leaf.kind == FlatRecordFieldKind::Bool)
    {
        load_i32(body, temporary_out, leaf.source_offset);
        super::i32_const(body, 1);
        body.push(0x4b);
        body.extend([0x04, 0x40, 0x00, 0x0b]);
    }
    // Authenticate the cumulative output bound before publishing any carrier.
    // On rejection, settle every semantic owner before returning a failure.
    super::i32_const(body, 0);
    local_set(body, charged);
    for leaf in layout
        .leaves
        .iter()
        .filter(|leaf| leaf.kind == FlatRecordFieldKind::OwnedBytes)
    {
        local_get(body, charged);
        load_i64(body, temporary_out, leaf.source_offset);
        body.push(0xa7); // i32.wrap_i64: packed carrier length
        body.push(0x6a); // i32.add
        local_set(body, charged);
    }
    local_get(body, charged);
    super::i32_const(body, 65_536);
    body.push(0x4b); // i32.gt_u
    body.extend([0x04, 0x40]);
    for leaf in layout
        .leaves
        .iter()
        .filter(|leaf| leaf.kind == FlatRecordFieldKind::OwnedBytes)
    {
        load_i64(body, temporary_out, leaf.source_offset);
        body.push(0x10);
        super::write_u32(body, bytes_drop_index);
    }
    poison_temporary(body, temporary_out, private_size);
    local_get(body, old_stack);
    body.push(0x24);
    super::write_u32(body, 0);
    super::i32_const(body, 2);
    body.push(0x0f);
    body.push(0x0b);
    // The synchronous adapter's status return is the publication boundary.
    // All public slots may be filled before poisoning without partial JS
    // observability; an intervening trap poisons the enclosing invocation.
    for leaf in &layout.leaves {
        match leaf.kind {
            FlatRecordFieldKind::Bool => {
                super::i64_const(body, 0);
                local_set(body, scalar);
                store_i64(body, result_out, leaf.public_offset, scalar);
                load_i32(body, temporary_out, leaf.source_offset);
                local_set(body, charged);
                store_i32(body, result_out, leaf.public_offset, charged);
            }
            FlatRecordFieldKind::I64
            | FlatRecordFieldKind::Usize
            | FlatRecordFieldKind::OwnedBytes => {
                load_i64(body, temporary_out, leaf.source_offset);
                local_set(body, scalar);
                store_i64(body, result_out, leaf.public_offset, scalar);
            }
        }
    }
    poison_temporary(body, temporary_out, private_size);
}
