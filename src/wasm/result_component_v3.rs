//! Closed compiler-generated status/out core for private Component Model v3 evidence.

use std::collections::HashMap;

use crate::ast::{BinaryOp, Program, UnaryOp};
use crate::diagnostic::Diagnostic;
use crate::graph;
use crate::hir::{self, DeclarationId, ResolvedExpr, ResolvedExprKind, ResolvedType, ValueId};

use super::{section, write_bytes, write_i64, write_name, write_u32, I32, I64};

pub(crate) const FUNCTION_ID: &str = "component.evaluate";
pub(crate) const STATUS_OUT_EXPORT: &str = "semaprax_evaluate_status_out";
pub(crate) const CANONICAL_EXPORT: &str = "cabi_evaluate";
pub(crate) const CONTRACT_DOMAIN: &str = "semaprax.contract.v1";
pub(crate) const ARITHMETIC_DOMAIN: &str = "semaprax.arithmetic.v1";
pub(crate) const RESULT_AREA: i32 = 256;
pub(crate) const POISON_AREA: i32 = 128;
pub(crate) const POISON_I64: i64 = 0x5a5a_5a5a_5a5a_5a5a_u64 as i64;

const CONTRACT_REQUIRES: u32 = status_word(1, 1);
const CONTRACT_ENSURES: u32 = status_word(1, 2);
const ARITHMETIC_ADD: u32 = status_word(2, 1);
const ARITHMETIC_SUB: u32 = status_word(2, 2);
const ARITHMETIC_MUL: u32 = status_word(2, 3);
const ARITHMETIC_DIV_ZERO: u32 = status_word(2, 4);
const ARITHMETIC_DIV_OVERFLOW: u32 = status_word(2, 5);
const ARITHMETIC_REM_ZERO: u32 = status_word(2, 6);
const ARITHMETIC_REM_OVERFLOW: u32 = status_word(2, 7);
const ARITHMETIC_NEG: u32 = status_word(2, 8);

const fn status_word(class: u32, code: u32) -> u32 {
    (class << 24) | code
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct PrivateResultCoreArtifactV3 {
    pub(crate) bytes: Vec<u8>,
    pub(crate) source_revision: String,
}

pub(crate) fn emit_private_result_core_v3(
    program: &Program,
) -> Result<PrivateResultCoreArtifactV3, Diagnostic> {
    let resolved = hir::resolve(program).map_err(|diagnostics| {
        diagnostics
            .into_iter()
            .find(|diagnostic| diagnostic.severity.is_error())
            .unwrap_or_else(|| {
                Diagnostic::io("SPX-WIT107", "result component HIR resolution failed")
            })
    })?;
    hir::validate(&resolved)?;
    let function = resolved
        .functions
        .iter()
        .find(|function| function.id == DeclarationId::new(FUNCTION_ID))
        .ok_or_else(|| {
            Diagnostic::io(
                "SPX-WIT107",
                "private result component requires `@id(\"component.evaluate\")`",
            )
        })?;
    if function.params.len() != 2
        || function
            .params
            .iter()
            .any(|parameter| parameter.ty != ResolvedType::I64)
        || function.return_type != ResolvedType::I64
        || !function.effects.is_empty()
        || !resolved.types.is_empty()
        || !resolved.interfaces.is_empty()
    {
        return Err(Diagnostic::io(
            "SPX-WIT107",
            "private result component requires an effect-free `(i64, i64) -> i64` scalar function and no type/interface declarations",
        ));
    }

    let source_revision = graph::revision(program);
    let mut locals = HashMap::from([
        (function.params[0].id.clone(), (0_u32, ResolvedType::I64)),
        (function.params[1].id.clone(), (1_u32, ResolvedType::I64)),
        (function.result_id.clone(), (3_u32, ResolvedType::I64)),
    ]);
    let mut compiler = ExprCompiler {
        locals: &mut locals,
        local_types: vec![I64], // local 3 is the poison-preserving result staging cell
        next_local: 4,
    };
    let mut instructions = Vec::new();
    for contract in &function.requires {
        compiler.emit(contract, &mut instructions)?;
        emit_failure_guard(&mut instructions, CONTRACT_REQUIRES);
    }
    compiler.emit(&function.body, &mut instructions)?;
    instructions.push(0x21); // local.set result
    write_u32(&mut instructions, 3);
    for contract in &function.ensures {
        compiler.emit(contract, &mut instructions)?;
        emit_failure_guard(&mut instructions, CONTRACT_ENSURES);
    }
    instructions.extend([0x20, 0x02, 0x20, 0x03, 0x37, 0x03, 0x00]);
    instructions.push(0x41);
    write_u32(&mut instructions, 0);

    let status_out_body = function_body(&compiler.local_types, instructions);
    let canonical_body = canonical_adapter_body();

    let mut module = b"\0asm\x01\0\0\0".to_vec();
    let mut types = Vec::new();
    write_u32(&mut types, 2);
    types.extend([0x60, 0x03, I64, I64, I32, 0x01, I32]);
    types.extend([0x60, 0x02, I64, I64, 0x01, I32]);
    section(&mut module, 1, types);
    section(&mut module, 3, vec![0x02, 0x00, 0x01]);
    section(&mut module, 5, vec![0x01, 0x00, 0x01]);

    let mut exports = Vec::new();
    write_u32(&mut exports, 3);
    write_name(&mut exports, "memory");
    exports.extend([0x02, 0x00]);
    write_name(&mut exports, STATUS_OUT_EXPORT);
    exports.extend([0x00, 0x00]);
    write_name(&mut exports, CANONICAL_EXPORT);
    exports.extend([0x00, 0x01]);
    section(&mut module, 7, exports);

    let mut code = Vec::new();
    write_u32(&mut code, 2);
    write_bytes(&mut code, &status_out_body);
    write_bytes(&mut code, &canonical_body);
    section(&mut module, 10, code);

    let mut data = Vec::new();
    write_u32(&mut data, 2);
    active_data(&mut data, 0, CONTRACT_DOMAIN.as_bytes());
    active_data(&mut data, 32, ARITHMETIC_DOMAIN.as_bytes());
    section(&mut module, 11, data);

    let mut custom = Vec::new();
    write_name(&mut custom, "semaprax.component-result-v3");
    write_name(&mut custom, &source_revision);
    section(&mut module, 0, custom);

    Ok(PrivateResultCoreArtifactV3 {
        bytes: module,
        source_revision,
    })
}

fn active_data(output: &mut Vec<u8>, offset: i32, bytes: &[u8]) {
    output.push(0x00);
    output.push(0x41);
    write_u32(output, offset as u32);
    output.push(0x0b);
    write_bytes(output, bytes);
}

fn function_body(local_types: &[u8], mut instructions: Vec<u8>) -> Vec<u8> {
    let mut body = Vec::new();
    write_u32(&mut body, local_types.len() as u32);
    for ty in local_types {
        body.extend([0x01, *ty]);
    }
    body.append(&mut instructions);
    body.push(0x0b);
    body
}

fn emit_failure_guard(output: &mut Vec<u8>, status: u32) {
    output.extend([0x45, 0x04, 0x40, 0x41]); // i32.eqz; if void; i32.const
    write_u32(output, status);
    output.extend([0x0f, 0x0b]); // return; end
}

struct ExprCompiler<'a> {
    locals: &'a mut HashMap<ValueId, (u32, ResolvedType)>,
    local_types: Vec<u8>,
    next_local: u32,
}

impl ExprCompiler<'_> {
    fn scratch(&mut self, ty: &ResolvedType) -> Result<u32, Diagnostic> {
        let wasm_ty = match ty {
            ResolvedType::I64 => I64,
            ResolvedType::Bool => I32,
            _ => return Err(profile_error()),
        };
        let local = self.next_local;
        self.next_local += 1;
        self.local_types.push(wasm_ty);
        Ok(local)
    }

    fn emit(&mut self, expr: &ResolvedExpr, output: &mut Vec<u8>) -> Result<(), Diagnostic> {
        match &expr.kind {
            ResolvedExprKind::Int(value) => {
                output.push(0x42);
                write_i64(output, *value);
            }
            ResolvedExprKind::Bool(value) => output.extend([0x41, u8::from(*value)]),
            ResolvedExprKind::Place(place) if place.projections.is_empty() => {
                let (local, ty) = self.locals.get(&place.root).ok_or_else(profile_error)?;
                if ty != &expr.ty {
                    return Err(profile_error());
                }
                output.push(0x20);
                write_u32(output, *local);
            }
            ResolvedExprKind::Unary { op, value } => match op {
                UnaryOp::Not => {
                    self.emit(value, output)?;
                    output.push(0x45);
                }
                UnaryOp::Neg => {
                    let operand = self.scratch(&ResolvedType::I64)?;
                    self.emit(value, output)?;
                    output.push(0x21);
                    write_u32(output, operand);
                    output.push(0x20);
                    write_u32(output, operand);
                    output.push(0x42);
                    write_i64(output, i64::MIN);
                    output.extend([0x51, 0x04, 0x40, 0x41]);
                    write_u32(output, ARITHMETIC_NEG);
                    output.extend([0x0f, 0x0b, 0x42, 0x00, 0x20]);
                    write_u32(output, operand);
                    output.push(0x7d);
                }
            },
            ResolvedExprKind::Binary { op, left, right } => {
                self.emit_binary(*op, left, right, output)?;
            }
            ResolvedExprKind::Block { statements, tail } if statements.is_empty() => {
                self.emit(tail, output)?;
            }
            _ => return Err(profile_error()),
        }
        Ok(())
    }

    fn emit_binary(
        &mut self,
        op: BinaryOp,
        left: &ResolvedExpr,
        right: &ResolvedExpr,
        output: &mut Vec<u8>,
    ) -> Result<(), Diagnostic> {
        if matches!(op, BinaryOp::And | BinaryOp::Or) {
            self.emit(left, output)?;
            output.extend([0x04, 0x7f]);
            if op == BinaryOp::And {
                self.emit(right, output)?;
            } else {
                output.extend([0x41, 0x01]);
            }
            output.push(0x05);
            if op == BinaryOp::And {
                output.extend([0x41, 0x00]);
            } else {
                self.emit(right, output)?;
            }
            output.push(0x0b);
            return Ok(());
        }
        if matches!(
            op,
            BinaryOp::Eq | BinaryOp::Ne | BinaryOp::Lt | BinaryOp::Le | BinaryOp::Gt | BinaryOp::Ge
        ) {
            self.emit(left, output)?;
            self.emit(right, output)?;
            let opcode = match (&left.ty, op) {
                (ResolvedType::I64, BinaryOp::Eq) => 0x51,
                (ResolvedType::I64, BinaryOp::Ne) => 0x52,
                (ResolvedType::I64, BinaryOp::Lt) => 0x53,
                (ResolvedType::I64, BinaryOp::Gt) => 0x55,
                (ResolvedType::I64, BinaryOp::Le) => 0x57,
                (ResolvedType::I64, BinaryOp::Ge) => 0x59,
                (ResolvedType::Bool, BinaryOp::Eq) => 0x46,
                (ResolvedType::Bool, BinaryOp::Ne) => 0x47,
                _ => return Err(profile_error()),
            };
            output.push(opcode);
            return Ok(());
        }

        let lhs = self.scratch(&ResolvedType::I64)?;
        let rhs = self.scratch(&ResolvedType::I64)?;
        let result = self.scratch(&ResolvedType::I64)?;
        self.emit(left, output)?;
        output.push(0x21);
        write_u32(output, lhs);
        self.emit(right, output)?;
        output.push(0x21);
        write_u32(output, rhs);

        if matches!(op, BinaryOp::Div | BinaryOp::Rem) {
            output.extend([0x20]);
            write_u32(output, rhs);
            output.extend([0x50, 0x04, 0x40, 0x41]);
            write_u32(
                output,
                if op == BinaryOp::Div {
                    ARITHMETIC_DIV_ZERO
                } else {
                    ARITHMETIC_REM_ZERO
                },
            );
            output.extend([0x0f, 0x0b, 0x20]);
            write_u32(output, lhs);
            output.push(0x42);
            write_i64(output, i64::MIN);
            output.extend([0x51, 0x20]);
            write_u32(output, rhs);
            output.push(0x42);
            write_i64(output, -1);
            output.extend([0x51, 0x71, 0x04, 0x40, 0x41]);
            write_u32(
                output,
                if op == BinaryOp::Div {
                    ARITHMETIC_DIV_OVERFLOW
                } else {
                    ARITHMETIC_REM_OVERFLOW
                },
            );
            output.extend([0x0f, 0x0b]);
        }

        output.push(0x20);
        write_u32(output, lhs);
        output.push(0x20);
        write_u32(output, rhs);
        output.push(match op {
            BinaryOp::Add => 0x7c,
            BinaryOp::Sub => 0x7d,
            BinaryOp::Mul => 0x7e,
            BinaryOp::Div => 0x7f,
            BinaryOp::Rem => 0x81,
            _ => return Err(profile_error()),
        });
        output.push(0x21);
        write_u32(output, result);

        match op {
            BinaryOp::Add => {
                output.extend([0x20]);
                write_u32(output, lhs);
                output.extend([0x20]);
                write_u32(output, result);
                output.extend([0x85, 0x20]);
                write_u32(output, rhs);
                output.extend([0x20]);
                write_u32(output, result);
                output.extend([0x85, 0x83, 0x42, 0x00, 0x53]);
                emit_overflow_guard(output, ARITHMETIC_ADD);
            }
            BinaryOp::Sub => {
                output.extend([0x20]);
                write_u32(output, lhs);
                output.extend([0x20]);
                write_u32(output, rhs);
                output.extend([0x85, 0x20]);
                write_u32(output, lhs);
                output.extend([0x20]);
                write_u32(output, result);
                output.extend([0x85, 0x83, 0x42, 0x00, 0x53]);
                emit_overflow_guard(output, ARITHMETIC_SUB);
            }
            BinaryOp::Mul => {
                output.extend([0x20]);
                write_u32(output, rhs);
                output.extend([0x50, 0x04, 0x40, 0x05, 0x20]);
                write_u32(output, result);
                output.push(0x42);
                write_i64(output, i64::MIN);
                output.extend([0x51, 0x20]);
                write_u32(output, rhs);
                output.push(0x42);
                write_i64(output, -1);
                output.extend([0x51, 0x71, 0x04, 0x40, 0x41]);
                write_u32(output, ARITHMETIC_MUL);
                output.extend([0x0f, 0x0b, 0x20]);
                write_u32(output, result);
                output.extend([0x20]);
                write_u32(output, rhs);
                output.extend([0x7f, 0x20]);
                write_u32(output, lhs);
                output.push(0x52);
                output.extend([0x04, 0x40, 0x41]);
                write_u32(output, ARITHMETIC_MUL);
                output.extend([0x0f, 0x0b, 0x0b]);
            }
            BinaryOp::Div | BinaryOp::Rem => {}
            _ => return Err(profile_error()),
        }
        output.push(0x20);
        write_u32(output, result);
        Ok(())
    }
}

fn emit_overflow_guard(output: &mut Vec<u8>, status: u32) {
    output.extend([0x04, 0x40, 0x41]);
    write_u32(output, status);
    output.extend([0x0f, 0x0b]);
}

fn profile_error() -> Diagnostic {
    Diagnostic::io(
        "SPX-WIT107",
        "private result component admits only direct scalar expressions without calls, blocks, records, or branches",
    )
}

fn canonical_adapter_body() -> Vec<u8> {
    let mut code = Vec::new();
    // One i32 local holds the private status word.
    write_u32(&mut code, 1);
    code.extend([0x01, I32]);
    // Canonical result memory is reset before every call.
    for offset in [RESULT_AREA, RESULT_AREA + 8, RESULT_AREA + 16] {
        code.push(0x41);
        write_u32(&mut code, offset as u32);
        code.extend([0x42, 0x00, 0x37, 0x03, 0x00]);
    }
    // Preserve poison unless status/out reports success.
    code.push(0x41);
    write_u32(&mut code, POISON_AREA as u32);
    code.push(0x42);
    write_i64(&mut code, POISON_I64);
    code.extend([0x37, 0x03, 0x00, 0x20, 0x00, 0x20, 0x01, 0x41]);
    write_u32(&mut code, POISON_AREA as u32);
    code.extend([0x10, 0x00, 0x22, 0x02, 0x45, 0x04, 0x40]);
    // ok tag and payload
    code.push(0x41);
    write_u32(&mut code, RESULT_AREA as u32);
    code.extend([0x41, 0x00, 0x3a, 0x00, 0x00, 0x41]);
    write_u32(&mut code, (RESULT_AREA + 8) as u32);
    code.push(0x41);
    write_u32(&mut code, POISON_AREA as u32);
    code.extend([0x29, 0x03, 0x00, 0x37, 0x03, 0x00, 0x05]);
    // err tag
    code.push(0x41);
    write_u32(&mut code, RESULT_AREA as u32);
    code.extend([0x41, 0x01, 0x3a, 0x00, 0x00]);
    // domain pointer selected from class
    code.push(0x41);
    write_u32(&mut code, (RESULT_AREA + 8) as u32);
    code.extend([0x20, 0x02, 0x41]);
    write_u32(&mut code, 24);
    code.extend([0x76, 0x41, 0x01, 0x46, 0x04, 0x7f, 0x41, 0x00, 0x05, 0x41]);
    write_u32(&mut code, 32);
    code.extend([0x0b, 0x36, 0x02, 0x00]);
    // domain length selected from class
    code.push(0x41);
    write_u32(&mut code, (RESULT_AREA + 12) as u32);
    code.extend([0x20, 0x02, 0x41]);
    write_u32(&mut code, 24);
    code.extend([0x76, 0x41, 0x01, 0x46, 0x04, 0x7f, 0x41]);
    write_u32(&mut code, CONTRACT_DOMAIN.len() as u32);
    code.extend([0x05, 0x41]);
    write_u32(&mut code, ARITHMETIC_DOMAIN.len() as u32);
    code.extend([0x0b, 0x36, 0x02, 0x00]);
    // status code
    code.push(0x41);
    write_u32(&mut code, (RESULT_AREA + 16) as u32);
    code.extend([0x20, 0x02, 0x41]);
    write_u32(&mut code, 0x00ff_ffff);
    code.extend([0x71, 0x36, 0x02, 0x00]);
    // class, option-some tag, retryable=false
    code.push(0x41);
    write_u32(&mut code, (RESULT_AREA + 20) as u32);
    code.extend([0x20, 0x02, 0x41]);
    write_u32(&mut code, 24);
    code.extend([0x76, 0x3a, 0x00, 0x00]);
    code.push(0x41);
    write_u32(&mut code, (RESULT_AREA + 21) as u32);
    code.extend([0x41, 0x01, 0x3a, 0x00, 0x00]);
    code.push(0x41);
    write_u32(&mut code, (RESULT_AREA + 22) as u32);
    code.extend([0x41, 0x00, 0x3a, 0x00, 0x00, 0x0b]);
    code.push(0x41);
    write_u32(&mut code, RESULT_AREA as u32);
    code.push(0x0b);
    code
}
