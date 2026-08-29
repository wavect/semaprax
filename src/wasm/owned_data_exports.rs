//! Descriptor-driven raw Wasm adapters for the closed public owned-data results.

use crate::aggregate_layout::{AggregateFieldValueKind, AggregateLayoutCache, AggregateTarget};
use crate::diagnostic::Diagnostic;
use crate::hir::{DeclarationId, ResolvedProgram};
use crate::project::{PublicApiDescriptor, PublicApiParameterType, PublicApiResultType};
use crate::project::{
    PUBLIC_OPTION_NONE_TAG, PUBLIC_OPTION_SOME_TAG, PUBLIC_RESULT_ERR_TAG, PUBLIC_RESULT_OK_TAG,
};
use crate::variant_layout::{VariantLayoutCache, VariantTarget};

use super::{write_i64, write_u32, I32, I64};

pub(super) const BOUNDARY_STATUS: i32 = 11;

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) enum ResultLayout {
    I64,
    Bool,
    Usize,
    Bytes,
    Utf8,
    OptionBytes {
        payload_offset: u32,
    },
    ResultBytesI64 {
        payload_offset: u32,
    },
    FlatRecord {
        private_size: u32,
        public_size: u32,
        fields: Vec<FlatRecordFieldLayout>,
    },
}

impl ResultLayout {
    fn private_size(&self) -> u32 {
        match self {
            Self::Bool => 4,
            Self::I64 | Self::Usize | Self::Bytes | Self::Utf8 => 8,
            Self::OptionBytes { .. } | Self::ResultBytesI64 { .. } => 16,
            Self::FlatRecord { private_size, .. } => *private_size,
        }
    }

    fn public_size(&self) -> u32 {
        match self {
            Self::FlatRecord { public_size, .. } => *public_size,
            _ => self.private_size(),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum FlatRecordFieldKind {
    I64,
    Bool,
    Usize,
    OwnedBytes,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct FlatRecordFieldLayout {
    source_offset: u32,
    public_offset: u32,
    kind: FlatRecordFieldKind,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum ParameterType {
    I64,
    Bool,
    BorrowStr,
    BorrowSliceU8,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct OwnedDataExportPlan {
    pub(super) stable_id: String,
    pub(super) wasm_export: String,
    pub(super) function_id: DeclarationId,
    pub(super) parameters: Vec<ParameterType>,
    pub(super) result: ResultLayout,
}

impl OwnedDataExportPlan {
    pub(super) fn raw_params(&self) -> Vec<u8> {
        let mut result = Vec::new();
        for parameter in &self.parameters {
            match parameter {
                ParameterType::I64 => result.push(I64),
                ParameterType::Bool => result.push(I32),
                ParameterType::BorrowStr | ParameterType::BorrowSliceU8 => {
                    result.extend([I32, I32])
                }
            }
        }
        result.push(I32); // caller-owned result_out
        result
    }

    pub(super) fn emit_wrapper_body(
        &self,
        target_index: u32,
        utf8_validate_index: u32,
        bytes_drop_index: u32,
    ) -> Result<Vec<u8>, Diagnostic> {
        let raw_count = u32::try_from(self.raw_params().len())
            .map_err(|_| error("owned-data wrapper parameter count overflows"))?;
        let result_out = raw_count - 1;
        let charged = raw_count;
        let old_stack = raw_count + 1;
        let temporary_out = raw_count + 2;
        let status = raw_count + 3;
        let carrier = raw_count + 4;
        let scalar = raw_count + 5;
        let mut body = Vec::new();
        write_u32(&mut body, 2);
        write_u32(&mut body, 4);
        body.push(I32);
        write_u32(&mut body, 2);
        body.push(I64);

        let private_size = self.result.private_size();
        let public_size = self.result.public_size();

        // Authenticate alignment and the complete fixed-memory range before
        // evaluating or calling any semantic function.
        local_get(&mut body, result_out);
        i32_const(&mut body, 7);
        body.push(0x71); // i32.and
        boundary_return(&mut body);
        local_get(&mut body, result_out);
        i32_const(&mut body, 131_072 - public_size as i32);
        body.push(0x4b); // i32.gt_u
        boundary_return(&mut body);

        i32_const(&mut body, 0);
        local_set(&mut body, charged);
        let mut raw = 0_u32;
        for parameter in &self.parameters {
            match parameter {
                ParameterType::I64 => raw += 1,
                ParameterType::Bool => {
                    local_get(&mut body, raw);
                    i32_const(&mut body, 1);
                    body.push(0x4b);
                    boundary_return(&mut body);
                    raw += 1;
                }
                ParameterType::BorrowStr | ParameterType::BorrowSliceU8 => {
                    let offset = raw;
                    let length = raw + 1;
                    local_get(&mut body, length);
                    i32_const(&mut body, 65_536);
                    body.push(0x4b);
                    boundary_return(&mut body);
                    local_get(&mut body, offset);
                    i32_const(&mut body, 65_536);
                    local_get(&mut body, length);
                    body.push(0x6b);
                    body.push(0x4b);
                    boundary_return(&mut body);
                    local_get(&mut body, charged);
                    i32_const(&mut body, 65_536);
                    local_get(&mut body, length);
                    body.push(0x6b);
                    body.push(0x4b);
                    boundary_return(&mut body);
                    local_get(&mut body, charged);
                    local_get(&mut body, length);
                    body.push(0x6a);
                    local_set(&mut body, charged);
                    if *parameter == ParameterType::BorrowStr {
                        local_get(&mut body, offset);
                        local_get(&mut body, length);
                        body.push(0x10);
                        write_u32(&mut body, utf8_validate_index);
                        body.push(0x45);
                        boundary_return(&mut body);
                    }
                    raw += 2;
                }
            }
        }

        // The semantic target owns its ordinary internal result parameter.
        // Keep that private until its sticky status confirms publication.
        body.push(0x23); // global.get private shadow stack
        write_u32(&mut body, 0);
        body.push(0x22); // local.tee
        write_u32(&mut body, old_stack);
        i32_const(&mut body, private_size as i32);
        body.push(0x49); // i32.lt_u
        body.extend([0x04, 0x40, 0x00, 0x0b]); // invariant trap
        local_get(&mut body, old_stack);
        i32_const(&mut body, private_size as i32);
        body.push(0x6b);
        body.push(0x21); // local.set; the pointer is reloaded for each use below
        write_u32(&mut body, temporary_out);
        // Authenticate complete half-open range disjointness. Equality alone
        // is insufficient when the private aggregate and public carrier have
        // different sizes and would admit aligned partial overlap.
        local_get(&mut body, result_out);
        local_get(&mut body, temporary_out);
        i32_const(&mut body, private_size as i32);
        body.push(0x6a);
        body.push(0x49); // public_start < private_end
        local_get(&mut body, temporary_out);
        local_get(&mut body, result_out);
        i32_const(&mut body, public_size as i32);
        body.push(0x6a);
        body.push(0x49); // private_start < public_end
        body.push(0x71); // i32.and
        boundary_return(&mut body);
        local_get(&mut body, temporary_out);
        body.push(0x24); // global.set private shadow stack
        write_u32(&mut body, 0);

        raw = 0;
        for parameter in &self.parameters {
            match parameter {
                ParameterType::I64 | ParameterType::Bool => {
                    local_get(&mut body, raw);
                    raw += 1;
                }
                ParameterType::BorrowStr | ParameterType::BorrowSliceU8 => {
                    local_get(&mut body, raw);
                    body.push(0xad); // i64.extend_i32_u
                    i64_const(&mut body, 32);
                    body.push(0x86);
                    local_get(&mut body, raw + 1);
                    body.push(0xad);
                    body.push(0x84);
                    raw += 2;
                }
            }
        }
        local_get(&mut body, temporary_out);
        body.push(0x10);
        write_u32(&mut body, target_index);
        local_set(&mut body, status);

        local_get(&mut body, status);
        body.extend([0x04, 0x40]);
        poison_temporary(&mut body, temporary_out, private_size);
        local_get(&mut body, old_stack);
        body.push(0x24);
        write_u32(&mut body, 0);
        local_get(&mut body, status);
        body.push(0x0f);
        body.push(0x0b);

        match &self.result {
            ResultLayout::I64 | ResultLayout::Usize => {
                load_i64(&mut body, temporary_out, 0);
                local_set(&mut body, carrier);
                poison_temporary(&mut body, temporary_out, private_size);
                store_i64(&mut body, result_out, 0, carrier);
            }
            ResultLayout::Bool => {
                load_i32(&mut body, temporary_out, 0);
                local_set(&mut body, charged);
                local_get(&mut body, charged);
                i32_const(&mut body, 1);
                body.push(0x4b); // i32.gt_u
                body.extend([0x04, 0x40, 0x00, 0x0b]); // invalid HIR result traps
                poison_temporary(&mut body, temporary_out, private_size);
                store_i32(&mut body, result_out, 0, charged);
            }
            ResultLayout::Bytes => {
                load_i64(&mut body, temporary_out, 0);
                local_set(&mut body, carrier);
                poison_temporary(&mut body, temporary_out, 8);
                store_i64(&mut body, result_out, 0, carrier);
            }
            ResultLayout::Utf8 => {
                load_i64(&mut body, temporary_out, 0);
                local_set(&mut body, carrier);
                local_get(&mut body, carrier);
                i64_const(&mut body, 32);
                body.push(0x88); // i64.shr_u: packed carrier root
                body.push(0xa7); // i32.wrap_i64
                local_get(&mut body, carrier);
                body.push(0xa7); // i32.wrap_i64: packed carrier length
                body.push(0x10);
                write_u32(&mut body, utf8_validate_index);
                body.push(0x45); // i32.eqz
                body.extend([0x04, 0x40]);
                local_get(&mut body, carrier);
                body.push(0x10);
                write_u32(&mut body, bytes_drop_index);
                poison_temporary(&mut body, temporary_out, 8);
                local_get(&mut body, old_stack);
                body.push(0x24);
                write_u32(&mut body, 0);
                i32_const(&mut body, 2); // adapter rejection after settlement
                body.push(0x0f);
                body.push(0x0b);
                poison_temporary(&mut body, temporary_out, 8);
                store_i64(&mut body, result_out, 0, carrier);
            }
            ResultLayout::OptionBytes { payload_offset } => {
                authenticate_tag(&mut body, temporary_out, charged);
                local_get(&mut body, charged);
                i32_const(&mut body, PUBLIC_OPTION_NONE_TAG as i32);
                body.push(0x46);
                body.extend([0x04, 0x40]);
                poison_temporary(&mut body, temporary_out, private_size);
                store_i32(&mut body, result_out, 0, charged);
                body.push(0x05);
                load_i64(&mut body, temporary_out, *payload_offset);
                local_set(&mut body, carrier);
                poison_temporary(&mut body, temporary_out, private_size);
                store_i64(&mut body, result_out, *payload_offset, carrier);
                store_i32(&mut body, result_out, 0, charged);
                body.push(0x0b);
            }
            ResultLayout::ResultBytesI64 { payload_offset } => {
                authenticate_tag(&mut body, temporary_out, charged);
                load_i64(&mut body, temporary_out, *payload_offset);
                local_set(&mut body, carrier);
                poison_temporary(&mut body, temporary_out, private_size);
                store_i64(&mut body, result_out, *payload_offset, carrier);
                store_i32(&mut body, result_out, 0, charged);
            }
            ResultLayout::FlatRecord { fields, .. } => {
                // Authenticate every bool before exposing any scalar field.
                for field in fields
                    .iter()
                    .filter(|field| field.kind == FlatRecordFieldKind::Bool)
                {
                    load_i32(&mut body, temporary_out, field.source_offset);
                    i32_const(&mut body, 1);
                    body.push(0x4b);
                    body.extend([0x04, 0x40, 0x00, 0x0b]);
                }
                let owned = fields
                    .iter()
                    .find(|field| field.kind == FlatRecordFieldKind::OwnedBytes)
                    .ok_or_else(|| error("flat record carrier lost its owned field"))?;
                load_i64(&mut body, temporary_out, owned.source_offset);
                local_set(&mut body, carrier);
                for field in fields
                    .iter()
                    .filter(|field| field.kind != FlatRecordFieldKind::OwnedBytes)
                {
                    match field.kind {
                        FlatRecordFieldKind::Bool => {
                            i64_const(&mut body, 0);
                            local_set(&mut body, scalar);
                            store_i64(&mut body, result_out, field.public_offset, scalar);
                            load_i32(&mut body, temporary_out, field.source_offset);
                            local_set(&mut body, charged);
                            store_i32(&mut body, result_out, field.public_offset, charged);
                        }
                        FlatRecordFieldKind::I64 | FlatRecordFieldKind::Usize => {
                            load_i64(&mut body, temporary_out, field.source_offset);
                            local_set(&mut body, scalar);
                            store_i64(&mut body, result_out, field.public_offset, scalar);
                        }
                        FlatRecordFieldKind::OwnedBytes => unreachable!("filtered above"),
                    }
                }
                poison_temporary(&mut body, temporary_out, private_size);
                // The sole ownership-bearing field is the carrier commit and
                // is therefore always the final public write.
                store_i64(&mut body, result_out, owned.public_offset, carrier);
            }
        }
        local_get(&mut body, old_stack);
        body.push(0x24);
        write_u32(&mut body, 0);
        i32_const(&mut body, 0);
        body.push(0x0b);
        Ok(body)
    }
}

fn poison_temporary(body: &mut Vec<u8>, pointer: u32, size: u32) {
    let full_words = size / 8;
    for word in 0..full_words {
        let offset = word * 8;
        local_get(body, pointer);
        i64_const(body, -6_510_615_555_426_900_571_i64); // 0xa5 repeated
        body.extend([0x37, 0x03]);
        write_u32(body, offset);
    }
    if size % 8 == 4 {
        local_get(body, pointer);
        i32_const(body, -1_515_870_811_i32); // 0xa5 repeated
        body.extend([0x36, 0x02]);
        write_u32(body, full_words * 8);
    }
}

pub(super) fn prepare(
    program: &ResolvedProgram,
    descriptor: &PublicApiDescriptor,
) -> Result<Vec<OwnedDataExportPlan>, Diagnostic> {
    crate::hir::validate(program)?;
    let variant_layouts = VariantLayoutCache::build(program, VariantTarget::Wasm32)?;
    descriptor
        .exports()
        .iter()
        .map(|export| {
            let function = program
                .functions
                .iter()
                .find(|function| function.id == *export.stable_id())
                .ok_or_else(|| error("owned-data descriptor target is absent from held HIR"))?;
            let result = result_layout(
                program,
                &variant_layouts,
                export.result(),
                &function.return_type,
            )?;
            let parameters = export
                .parameters()
                .iter()
                .map(|parameter| match parameter.ty() {
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
                result,
            })
        })
        .collect()
}

pub(super) fn prepare_flat_records(
    program: &ResolvedProgram,
    descriptor: &crate::project::FlatOwnedRecordApiDescriptor,
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
                .ok_or_else(|| error("flat record descriptor target is absent from held HIR"))?;
            let layout = layouts.layout(&function.return_type)?;
            layout.validate(program)?;
            if layout.record != *export.record_id()
                || layout.fields.len() != export.fields().len()
                || layout.align != 8
            {
                return Err(error("flat record Wasm32 layout disagrees with descriptor"));
            }
            let public_size = u32::try_from(export.fields().len())
                .ok()
                .and_then(|count| count.checked_mul(8))
                .ok_or_else(|| error("flat record public carrier size overflows"))?;
            let fields = export
                .fields()
                .iter()
                .zip(&layout.fields)
                .map(|(field, physical)| {
                    if field.stable_id() != &physical.field {
                        return Err(error(
                            "flat record field order disagrees with Wasm32 layout",
                        ));
                    }
                    let kind = match field.ty() {
                        crate::project::FlatOwnedRecordFieldType::I64 => FlatRecordFieldKind::I64,
                        crate::project::FlatOwnedRecordFieldType::Bool => FlatRecordFieldKind::Bool,
                        crate::project::FlatOwnedRecordFieldType::Usize => {
                            FlatRecordFieldKind::Usize
                        }
                        crate::project::FlatOwnedRecordFieldType::OwnedBytes => {
                            FlatRecordFieldKind::OwnedBytes
                        }
                    };
                    let expected_value_kind = if kind == FlatRecordFieldKind::OwnedBytes {
                        AggregateFieldValueKind::OwnedBytes
                    } else {
                        AggregateFieldValueKind::Copy
                    };
                    if physical.value_kind != expected_value_kind
                        || physical.size
                            != if kind == FlatRecordFieldKind::Bool {
                                4
                            } else {
                                8
                            }
                    {
                        return Err(error("flat record field representation is not exact"));
                    }
                    Ok(FlatRecordFieldLayout {
                        source_offset: physical.offset,
                        public_offset: field
                            .ordinal()
                            .checked_mul(8)
                            .ok_or_else(|| error("flat record public field offset overflows"))?,
                        kind,
                    })
                })
                .collect::<Result<Vec<_>, _>>()?;
            if fields
                .iter()
                .filter(|field| field.kind == FlatRecordFieldKind::OwnedBytes)
                .count()
                != 1
            {
                return Err(error("flat record carrier requires one owned field"));
            }
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
                result: ResultLayout::FlatRecord {
                    private_size: layout.size,
                    public_size,
                    fields,
                },
            })
        })
        .collect()
}

fn result_layout(
    program: &ResolvedProgram,
    layouts: &VariantLayoutCache,
    result: PublicApiResultType,
    ty: &crate::hir::ResolvedType,
) -> Result<ResultLayout, Diagnostic> {
    match result {
        PublicApiResultType::I64 => return Ok(ResultLayout::I64),
        PublicApiResultType::Bool => return Ok(ResultLayout::Bool),
        PublicApiResultType::Usize => return Ok(ResultLayout::Usize),
        PublicApiResultType::OwnedBytes => return Ok(ResultLayout::Bytes),
        PublicApiResultType::OwnedUtf8 if *ty == crate::hir::ResolvedType::String => {
            return Ok(ResultLayout::Utf8)
        }
        PublicApiResultType::OwnedUtf8 => {
            return Err(error(
                "owned UTF-8 descriptor result disagrees with held HIR",
            ))
        }
        PublicApiResultType::OptionOwnedBytes | PublicApiResultType::ResultOwnedBytesI64 => {}
    }
    let layout = layouts.layout(ty)?;
    layout.validate(program)?;
    if layout.size != 16 || layout.align != 8 || layout.tag_size != 4 || layout.payload_offset != 8
    {
        return Err(error(
            "owned-data variant target layout is not the fixed Wasm32 carrier",
        ));
    }
    let expected = match result {
        PublicApiResultType::OptionOwnedBytes => [
            (crate::prelude::OPTION_NONE_ID, PUBLIC_OPTION_NONE_TAG, None),
            (
                crate::prelude::OPTION_SOME_ID,
                PUBLIC_OPTION_SOME_TAG,
                Some(crate::prelude::OPTION_SOME_VALUE_ID),
            ),
        ],
        PublicApiResultType::ResultOwnedBytesI64 => [
            (
                crate::prelude::RESULT_OK_ID,
                PUBLIC_RESULT_OK_TAG,
                Some(crate::prelude::RESULT_OK_VALUE_ID),
            ),
            (
                crate::prelude::RESULT_ERR_ID,
                PUBLIC_RESULT_ERR_TAG,
                Some(crate::prelude::RESULT_ERR_ERROR_ID),
            ),
        ],
        _ => return Err(error("WP-11 admits only owned byte result families")),
    };
    if layout.cases.len() != expected.len() {
        return Err(error("owned-data variant case inventory is not exact"));
    }
    for (case, (id, tag, field)) in layout.cases.iter().zip(expected) {
        if case.case.as_str() != id || case.tag != tag {
            return Err(error("owned-data variant discriminant disagrees"));
        }
        match field {
            None if case.fields.is_empty() => {}
            Some(field_id)
                if case.fields.len() == 1
                    && case.fields[0].field.as_str() == field_id
                    && case.fields[0].offset == 0
                    && case.fields[0].size == 8
                    && case.fields[0].align == 8 => {}
            _ => return Err(error("owned-data variant payload layout disagrees")),
        }
    }
    Ok(match result {
        PublicApiResultType::OptionOwnedBytes => ResultLayout::OptionBytes {
            payload_offset: layout.payload_offset,
        },
        PublicApiResultType::ResultOwnedBytesI64 => ResultLayout::ResultBytesI64 {
            payload_offset: layout.payload_offset,
        },
        _ => unreachable!("closed above"),
    })
}

fn authenticate_tag(body: &mut Vec<u8>, pointer: u32, tag_local: u32) {
    local_get(body, pointer);
    body.extend([0x28, 0x02, 0x00]);
    body.push(0x22);
    write_u32(body, tag_local);
    i32_const(body, 1);
    body.push(0x4b);
    body.extend([0x04, 0x40, 0x00, 0x0b]);
}

fn load_i64(body: &mut Vec<u8>, pointer: u32, offset: u32) {
    local_get(body, pointer);
    body.extend([0x29, 0x03]);
    write_u32(body, offset);
}

fn load_i32(body: &mut Vec<u8>, pointer: u32, offset: u32) {
    local_get(body, pointer);
    body.extend([0x28, 0x02]);
    write_u32(body, offset);
}

fn store_i64(body: &mut Vec<u8>, pointer: u32, offset: u32, value: u32) {
    local_get(body, pointer);
    local_get(body, value);
    body.extend([0x37, 0x03]);
    write_u32(body, offset);
}

fn store_i32(body: &mut Vec<u8>, pointer: u32, offset: u32, value: u32) {
    local_get(body, pointer);
    local_get(body, value);
    body.extend([0x36, 0x02]);
    write_u32(body, offset);
}

fn raw_symbol(stable_id: &str) -> String {
    let mut result = String::from("spx_owned_v1_");
    for byte in stable_id.bytes() {
        use std::fmt::Write as _;
        let _ = write!(result, "{byte:02x}");
    }
    result
}

fn boundary_return(body: &mut Vec<u8>) {
    body.extend([0x04, 0x40]);
    i32_const(body, BOUNDARY_STATUS);
    body.push(0x0f);
    body.push(0x0b);
}

fn local_get(body: &mut Vec<u8>, local: u32) {
    body.push(0x20);
    write_u32(body, local);
}
fn local_set(body: &mut Vec<u8>, local: u32) {
    body.push(0x21);
    write_u32(body, local);
}
fn i32_const(body: &mut Vec<u8>, value: i32) {
    body.push(0x41);
    write_i64(body, i64::from(value));
}
fn i64_const(body: &mut Vec<u8>, value: i64) {
    body.push(0x42);
    write_i64(body, value);
}
fn error(message: impl Into<String>) -> Diagnostic {
    Diagnostic::io("SPX-W124", message)
}
