//! Native scalar-snapshot closure carrier and thunk emission.
//!
//! This module is selected only by the resolved closure inventory.  Its fixed
//! eight-cell representation is caller-owned, so a returned closure requires
//! no allocation or cleanup transfer.
use std::fmt::Write as _;

use crate::diagnostic::Diagnostic;
use crate::hir::{self, FunctionExecutionId, ResolvedExprKind, ResolvedFunction, ResolvedType};

use super::{backend_error, c_value_type, COutput, NativeEmissionContext};

pub(super) const CAPTURE_SLOTS: usize = 8;

pub(super) fn enabled(program: &hir::ResolvedProgram) -> bool {
    hir::closure::requires_closures(program)
}

pub(super) fn carrier_type(ty: &ResolvedType) -> Result<String, Diagnostic> {
    if !hir::function_value::is_signature(ty) {
        return Err(backend_error(
            "native closure carrier has an invalid signature",
        ));
    }
    let mut symbol = String::from("spx_closure_");
    for byte in ty.identity_key().bytes() {
        write!(symbol, "{byte:02x}").expect("writing to a string cannot fail");
    }
    Ok(symbol)
}

fn entry_type(ty: &ResolvedType) -> Result<String, Diagnostic> {
    Ok(format!("{}_entry", carrier_type(ty)?))
}

/// Emits profile-local fixed-width cell codecs and one carrier declaration per
/// concrete signature.  `memcpy` keeps float and signed scalar snapshots
/// bit-exact without aliasing or conversion.
pub(super) fn emit_carrier_declarations(
    output: &mut impl COutput,
    program: &hir::ResolvedProgram,
    resource_abi: &super::native_resource::NativeResourceAbi,
) -> Result<(), Diagnostic> {
    output.push_str("static __attribute__((unused)) uint64_t spx_closure_pack_i64(int64_t value) { uint64_t cell = UINT64_C(0); memcpy(&cell, &value, sizeof value); return cell; }\n");
    output.push_str("static __attribute__((unused)) int64_t spx_closure_unpack_i64(uint64_t cell) { int64_t value; memcpy(&value, &cell, sizeof value); return value; }\n");
    output.push_str("static __attribute__((unused)) uint64_t spx_closure_pack_u64(uint64_t value) { return value; }\n");
    output.push_str("static __attribute__((unused)) uint64_t spx_closure_unpack_u64(uint64_t cell) { return cell; }\n");
    output.push_str("static __attribute__((unused)) uint64_t spx_closure_pack_i32(int32_t value) { uint64_t cell = UINT64_C(0); memcpy(&cell, &value, sizeof value); return cell; }\n");
    output.push_str("static __attribute__((unused)) int32_t spx_closure_unpack_i32(uint64_t cell) { int32_t value; memcpy(&value, &cell, sizeof value); return value; }\n");
    output.push_str("static __attribute__((unused)) uint64_t spx_closure_pack_u32(uint32_t value) { uint64_t cell = UINT64_C(0); memcpy(&cell, &value, sizeof value); return cell; }\n");
    output.push_str("static __attribute__((unused)) uint32_t spx_closure_unpack_u32(uint64_t cell) { uint32_t value; memcpy(&value, &cell, sizeof value); return value; }\n");
    output.push_str(
        "static __attribute__((unused)) uint64_t spx_closure_pack_u8(uint8_t value) { return (uint64_t)value; }\n",
    );
    output.push_str(
        "static __attribute__((unused)) uint8_t spx_closure_unpack_u8(uint64_t cell) { return (uint8_t)cell; }\n",
    );
    output.push_str("static __attribute__((unused)) uint64_t spx_closure_pack_bool(bool value) { return value ? UINT64_C(1) : UINT64_C(0); }\n");
    output.push_str(
        "static __attribute__((unused)) bool spx_closure_unpack_bool(uint64_t cell) { return cell != UINT64_C(0); }\n",
    );
    output.push_str("static __attribute__((unused)) uint64_t spx_closure_pack_f32(float value) { uint64_t cell = UINT64_C(0); memcpy(&cell, &value, sizeof value); return cell; }\n");
    output.push_str("static __attribute__((unused)) float spx_closure_unpack_f32(uint64_t cell) { float value; memcpy(&value, &cell, sizeof value); return value; }\n");
    output.push_str("static __attribute__((unused)) uint64_t spx_closure_pack_f64(double value) { uint64_t cell; memcpy(&cell, &value, sizeof value); return cell; }\n");
    output.push_str("static __attribute__((unused)) double spx_closure_unpack_f64(uint64_t cell) { double value; memcpy(&value, &cell, sizeof value); return value; }\n\n");
    let mut signatures = std::collections::BTreeMap::new();
    for function in program
        .functions
        .iter()
        .chain(program.function_instances.iter().map(|item| &item.function))
    {
        for ty in std::iter::once(&function.return_type)
            .chain(function.params.iter().map(|parameter| &parameter.ty))
        {
            if matches!(ty, ResolvedType::Function { .. }) {
                signatures.insert(ty.identity_key(), ty.clone());
            }
        }
        hir::function_value::walk(function, |expression| {
            if matches!(expression.ty, ResolvedType::Function { .. }) {
                signatures.insert(expression.ty.identity_key(), expression.ty.clone());
            }
        });
    }
    for signature in signatures.into_values() {
        let ResolvedType::Function { parameters, result } = &signature else {
            unreachable!()
        };
        let carrier = carrier_type(&signature)?;
        write!(
            output,
            "typedef spx_status_token (*{})(struct spx_context *spx_ctx, const uint64_t *spx_cells",
            entry_type(&signature)?
        )
        .expect("writing to a string cannot fail");
        for parameter in parameters {
            write!(
                output,
                ", {}",
                c_value_type(program, resource_abi, parameter)?
            )
            .expect("writing to a string cannot fail");
        }
        writeln!(
            output,
            ", {} *spx_result_out);",
            c_value_type(program, resource_abi, result)?
        )
        .expect("writing to a string cannot fail");
        writeln!(
            output,
            "typedef struct {{ {} entry; uint64_t cells[{}]; }} {};",
            entry_type(&signature)?,
            CAPTURE_SLOTS,
            carrier
        )
        .expect("writing to a string cannot fail");
    }
    output.push('\n');
    Ok(())
}

pub(super) fn pack(ty: &ResolvedType, value: &str) -> Result<String, Diagnostic> {
    let helper = match ty {
        ResolvedType::I64 => "i64",
        ResolvedType::Usize => "u64",
        ResolvedType::I32 => "i32",
        ResolvedType::Char => "u32",
        ResolvedType::U8 => "u8",
        ResolvedType::Bool => "bool",
        ResolvedType::F32 => "f32",
        ResolvedType::F64 => "f64",
        _ => return Err(backend_error("closure capture is not a Copy scalar")),
    };
    Ok(format!("spx_closure_pack_{helper}({value})"))
}

fn unpack(ty: &ResolvedType, cell: usize) -> Result<String, Diagnostic> {
    let helper = match ty {
        ResolvedType::I64 => "i64",
        ResolvedType::Usize => "u64",
        ResolvedType::I32 => "i32",
        ResolvedType::Char => "u32",
        ResolvedType::U8 => "u8",
        ResolvedType::Bool => "bool",
        ResolvedType::F32 => "f32",
        ResolvedType::F64 => "f64",
        _ => return Err(backend_error("closure capture is not a Copy scalar")),
    };
    Ok(format!("spx_closure_unpack_{helper}(spx_cells[{cell}])"))
}

pub(super) fn closure_functions(
    program: &hir::ResolvedProgram,
) -> Result<Vec<ResolvedFunction>, Diagnostic> {
    hir::closure::inventory(program)
        .into_iter()
        .map(|expression| hir::closure::closure_function(program, expression))
        .collect()
}

pub(super) fn thunk_symbol(id: &hir::ExpressionId) -> String {
    let mut symbol = String::from("spx_closure_thunk_");
    for byte in id.as_str().bytes() {
        write!(symbol, "{byte:02x}").expect("writing to a string cannot fail");
    }
    symbol
}

pub(super) fn reference_thunk_symbol(id: &hir::ExpressionId) -> String {
    let mut symbol = String::from("spx_reference_thunk_");
    for byte in id.as_str().bytes() {
        write!(symbol, "{byte:02x}").expect("writing to a string cannot fail");
    }
    symbol
}

fn write_thunk_signature(
    output: &mut impl COutput,
    program: &hir::ResolvedProgram,
    resource_abi: &super::native_resource::NativeResourceAbi,
    symbol: &str,
    ty: &ResolvedType,
    names: bool,
) -> Result<(), Diagnostic> {
    let ResolvedType::Function { parameters, result } = ty else {
        return Err(backend_error("closure thunk has no function signature"));
    };
    write!(
        output,
        "static spx_status_token {symbol}(struct spx_context *spx_ctx, const uint64_t *spx_cells"
    )
    .expect("writing to a string cannot fail");
    for (index, parameter) in parameters.iter().enumerate() {
        write!(
            output,
            ", {}{}",
            c_value_type(program, resource_abi, parameter)?,
            if names {
                format!(" spx_arg_{index}")
            } else {
                String::new()
            }
        )
        .expect("writing to a string cannot fail");
    }
    write!(
        output,
        ", {} *spx_result_out)",
        c_value_type(program, resource_abi, result)?
    )
    .expect("writing to a string cannot fail");
    Ok(())
}

fn emit_thunk_prototypes_for_function(
    output: &mut impl COutput,
    program: &hir::ResolvedProgram,
    resource_abi: &super::native_resource::NativeResourceAbi,
    function: &ResolvedFunction,
) -> Result<(), Diagnostic> {
    let mut error = None;
    hir::function_value::walk(function, |expression| {
        if error.is_some() {
            return;
        }
        let symbol = match &expression.kind {
            ResolvedExprKind::Closure { .. } => thunk_symbol(&expression.id),
            ResolvedExprKind::FunctionReference { .. } => reference_thunk_symbol(&expression.id),
            _ => return,
        };
        error = write_thunk_signature(
            output,
            program,
            resource_abi,
            &symbol,
            &expression.ty,
            false,
        )
        .and_then(|_| {
            output.push_str(";\n");
            Ok(())
        })
        .err();
    });
    error.map_or(Ok(()), Err)
}

pub(super) fn emit_thunk_prototypes(
    output: &mut impl COutput,
    program: &hir::ResolvedProgram,
    resource_abi: &super::native_resource::NativeResourceAbi,
) -> Result<(), Diagnostic> {
    for function in program
        .functions
        .iter()
        .chain(program.function_instances.iter().map(|item| &item.function))
    {
        emit_thunk_prototypes_for_function(output, program, resource_abi, function)?;
    }
    for function in closure_functions(program)? {
        emit_thunk_prototypes_for_function(output, program, resource_abi, &function)?;
    }
    output.push('\n');
    Ok(())
}

pub(super) fn emit_thunks(
    output: &mut impl COutput,
    program: &hir::ResolvedProgram,
    emission: &NativeEmissionContext<'_>,
) -> Result<(), Diagnostic> {
    for expression in hir::closure::inventory(program) {
        let ResolvedType::Function {
            parameters,
            result: _,
        } = &expression.ty
        else {
            return Err(backend_error("closure has no function signature"));
        };
        let ResolvedExprKind::Closure { captures, .. } = &expression.kind else {
            unreachable!()
        };
        let derived_id = hir::closure::closure_id(&expression.id);
        let target = emission
            .functions
            .get(&FunctionExecutionId::Monomorphic(derived_id))
            .ok_or_else(|| backend_error("closure body is not indexed for native emission"))?;
        write_thunk_signature(
            output,
            program,
            emission.resource_abi,
            &thunk_symbol(&expression.id),
            &expression.ty,
            true,
        )?;
        output.push_str(" {\n");
        if captures.is_empty() {
            output.push_str("    (void)spx_cells;\n");
        }
        output.push_str("    return ");
        output.push_str(&target.symbol);
        output.push_str("(spx_ctx");
        for (slot, capture) in captures.iter().enumerate() {
            write!(output, ", {}", unpack(&capture.binding.ty, slot)?)
                .expect("writing to a string cannot fail");
        }
        for index in 0..parameters.len() {
            write!(output, ", spx_arg_{index}").expect("writing to a string cannot fail");
        }
        output.push_str(", spx_result_out);\n}\n\n");
    }
    for function in program
        .functions
        .iter()
        .chain(program.function_instances.iter().map(|item| &item.function))
    {
        let mut error = None;
        hir::function_value::walk(function, |expression| {
            if error.is_some() {
                return;
            }
            let ResolvedExprKind::FunctionReference { target } = &expression.kind else {
                return;
            };
            let result = (|| -> Result<(), Diagnostic> {
                write_thunk_signature(
                    output,
                    program,
                    emission.resource_abi,
                    &reference_thunk_symbol(&expression.id),
                    &expression.ty,
                    true,
                )?;
                output.push_str(" {\n    (void)spx_cells;\n    return ");
                let target = emission
                    .functions
                    .get(&FunctionExecutionId::Monomorphic(target.clone()))
                    .ok_or_else(|| {
                        backend_error(
                            "function reference target is not indexed for native closure emission",
                        )
                    })?;
                output.push_str(&target.symbol);
                output.push_str("(spx_ctx");
                let ResolvedType::Function { parameters, .. } = &expression.ty else {
                    unreachable!()
                };
                for index in 0..parameters.len() {
                    write!(output, ", spx_arg_{index}").expect("writing to a string cannot fail");
                }
                output.push_str(", spx_result_out);\n}\n\n");
                Ok(())
            })();
            error = result.err();
        });
        if let Some(error) = error {
            return Err(error);
        }
    }
    for function in closure_functions(program)? {
        let mut error = None;
        hir::function_value::walk(&function, |expression| {
            if error.is_some() {
                return;
            }
            let ResolvedExprKind::FunctionReference { target } = &expression.kind else {
                return;
            };
            let result = (|| -> Result<(), Diagnostic> {
                write_thunk_signature(
                    output,
                    program,
                    emission.resource_abi,
                    &reference_thunk_symbol(&expression.id),
                    &expression.ty,
                    true,
                )?;
                output.push_str(" {\n    (void)spx_cells;\n    return ");
                let target = emission
                    .functions
                    .get(&FunctionExecutionId::Monomorphic(target.clone()))
                    .ok_or_else(|| {
                        backend_error(
                            "function reference target is not indexed for native closure emission",
                        )
                    })?;
                output.push_str(&target.symbol);
                output.push_str("(spx_ctx");
                let ResolvedType::Function { parameters, .. } = &expression.ty else {
                    unreachable!()
                };
                for index in 0..parameters.len() {
                    write!(output, ", spx_arg_{index}").expect("writing to a string cannot fail");
                }
                output.push_str(", spx_result_out);\n}\n\n");
                Ok(())
            })();
            error = result.err();
        });
        if let Some(error) = error {
            return Err(error);
        }
    }
    Ok(())
}
