//! Descriptor-driven raw Wasm adapters for direct owned `Bytes` results.

use crate::diagnostic::Diagnostic;
use crate::hir::{DeclarationId, ResolvedProgram};
use crate::project::{PublicApiDescriptor, PublicApiParameterType, PublicApiResultType};

use super::{write_i64, write_u32, I32, I64};

pub(super) const RESULT_SIZE: u32 = 8;
pub(super) const BOUNDARY_STATUS: i32 = 11;

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
    ) -> Result<Vec<u8>, Diagnostic> {
        let raw_count = u32::try_from(self.raw_params().len())
            .map_err(|_| error("owned-data wrapper parameter count overflows"))?;
        let result_out = raw_count - 1;
        let charged = raw_count;
        let old_stack = raw_count + 1;
        let temporary_out = raw_count + 2;
        let status = raw_count + 3;
        let carrier = raw_count + 4;
        let mut body = Vec::new();
        write_u32(&mut body, 2);
        write_u32(&mut body, 4);
        body.push(I32);
        write_u32(&mut body, 1);
        body.push(I64);

        // Authenticate alignment and the complete fixed-memory range before
        // evaluating or calling any semantic function.
        local_get(&mut body, result_out);
        i32_const(&mut body, 7);
        body.push(0x71); // i32.and
        boundary_return(&mut body);
        local_get(&mut body, result_out);
        i32_const(&mut body, 131_072 - RESULT_SIZE as i32);
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
        i32_const(&mut body, 8);
        body.push(0x49); // i32.lt_u
        body.extend([0x04, 0x40, 0x00, 0x0b]); // invariant trap
        local_get(&mut body, result_out);
        local_get(&mut body, old_stack);
        i32_const(&mut body, 8);
        body.push(0x6b);
        body.push(0x46); // i32.eq: public out must not alias private temp
        boundary_return(&mut body);
        local_get(&mut body, old_stack);
        i32_const(&mut body, 8);
        body.push(0x6b);
        body.push(0x22);
        write_u32(&mut body, temporary_out);
        body.push(0x24); // global.set
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
        poison_temporary(&mut body, temporary_out);
        local_get(&mut body, old_stack);
        body.push(0x24);
        write_u32(&mut body, 0);
        local_get(&mut body, status);
        body.push(0x0f);
        body.push(0x0b);

        local_get(&mut body, temporary_out);
        body.extend([0x29, 0x03, 0x00]); // i64.load align=8
        body.push(0x21);
        write_u32(&mut body, carrier);
        poison_temporary(&mut body, temporary_out);
        local_get(&mut body, result_out);
        local_get(&mut body, carrier);
        body.extend([0x37, 0x03, 0x00]); // final i64.store align=8
        local_get(&mut body, old_stack);
        body.push(0x24);
        write_u32(&mut body, 0);
        i32_const(&mut body, 0);
        body.push(0x0b);
        Ok(body)
    }
}

fn poison_temporary(body: &mut Vec<u8>, pointer: u32) {
    local_get(body, pointer);
    i64_const(body, -6_510_615_555_426_900_571_i64); // 0xa5 repeated
    body.extend([0x37, 0x03, 0x00]);
}

pub(super) fn prepare(
    program: &ResolvedProgram,
    descriptor: &PublicApiDescriptor,
) -> Result<Vec<OwnedDataExportPlan>, Diagnostic> {
    crate::hir::validate(program)?;
    descriptor
        .exports()
        .iter()
        .map(|export| {
            if export.result() != PublicApiResultType::OwnedBytes {
                return Err(error("WP-10 admits only direct owned-bytes results"));
            }
            if !program
                .functions
                .iter()
                .any(|function| function.id == *export.stable_id())
            {
                return Err(error(
                    "owned-data descriptor target is absent from held HIR",
                ));
            }
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
            })
        })
        .collect()
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
