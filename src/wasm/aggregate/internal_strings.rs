//! Standalone module assembly and String operations. Existing emitters never
//! select this mode; its drop index deliberately agrees with the reused ledger.

#![allow(clippy::items_after_test_module)]

use super::*;
// This standalone profile retains its own diagnostic namespace even when it
// reuses the private owned-data planner.
use crate::wasm::internal_strings::error;
use crate::wasm::internal_strings::Export;
use std::collections::{BTreeMap, BTreeSet, HashMap};

pub(super) const LITERAL_IMPORT: u32 = 0;
pub(super) const CLONE_IMPORT: u32 = 1;
pub(super) const EQ_IMPORT: u32 = 6;
const IMPORT_COUNT: u32 = 10;
const RESULT_OFFSET: u32 = 65_536;
const MAX_MODULE_BYTES: usize = 16 * 1024 * 1024;
const _: () = assert!(BYTE_DROP_IMPORT == 9);

const MAX_CLEANUP_EMISSION_WORK: usize = 262_144;

/// Count every canonical finalizer action that the selected program emits.
/// A CleanupPlan exit is emitted as an independent Wasm cleanup path, so a
/// live place reached by several exits is deliberately charged once per exit.
fn canonical_cleanup_emission_work(
    action_counts: impl IntoIterator<Item = usize>,
) -> Option<usize> {
    action_counts.into_iter().try_fold(0usize, |work, actions| {
        work.checked_add(actions)
            .filter(|total| *total <= MAX_CLEANUP_EMISSION_WORK)
    })
}

pub(in crate::wasm) fn emit(
    program: &ResolvedProgram,
    exports: &[Export],
    closure: &BTreeSet<DeclarationId>,
    owner_limit: Option<u32>,
) -> Result<(Vec<u8>, u32, u32), Diagnostic> {
    // Admission proves every selected type scalar/String. Unselected nominal
    // declarations must not be scanned or affect this closure's artifacts.
    let layouts = VariantLayoutCache::for_scalar_only(VariantTarget::Wasm32);
    let functions = closure
        .iter()
        .map(|id| {
            program
                .functions
                .iter()
                .find(|function| &function.id == id)
                .ok_or_else(|| error("standalone String selected function is absent"))
        })
        .collect::<Result<Vec<_>, _>>()?;
    let calls = crate::call_index::PersistentCallIndex::build(program)?;
    let edges = calls
        .calls_by_owner()
        .iter()
        .filter(|(id, _)| closure.contains(*id))
        .map(|(id, callees)| (id.clone(), callees.clone()))
        .collect();
    let mut frames = BTreeMap::new();
    let mut owners = BTreeMap::new();
    let mut cleanup_action_counts = Vec::new();
    for function in &functions {
        let plan = FunctionPlan::build_profile(program, function, &layouts, true)?;
        cleanup_action_counts.extend(
            function
                .cleanup_plan
                .exits
                .iter()
                .map(|exit| exit.finalize_in_order.len()),
        );
        frames.insert(function.id.clone(), plan.frame_size);
        owners.insert(
            function.id.clone(),
            u32::try_from(plan.cleanup_place_flags.len())
                .map_err(|_| error("standalone String owner count overflows"))?,
        );
    }
    canonical_cleanup_emission_work(cleanup_action_counts)
        .ok_or_else(|| error("standalone String cleanup emission exceeds its work bound"))?;
    let frame_paths = owned_stack::longest_paths(&frames, &edges)
        .map_err(|diagnostic| error(diagnostic.message))?;
    let owner_paths = owned_stack::longest_paths(&owners, &edges)
        .map_err(|diagnostic| error(diagnostic.message))?;
    let stack = exports
        .iter()
        .map(|export| frame_paths[&export.id])
        .max()
        .unwrap_or(0);
    let capacity = exports
        .iter()
        .map(|export| owner_paths[&export.id])
        .max()
        .unwrap_or(0)
        .checked_add(1)
        .filter(|value| *value <= 65_536)
        .ok_or_else(|| error("standalone String owner capacity exceeds 65536"))?;
    if stack > SHADOW_STACK_TOP {
        return Err(error(
            "standalone String selected stack exceeds 65536 bytes",
        ));
    }
    if owner_limit.is_some_and(|limit| limit == 0 || limit > capacity) {
        return Err(error(
            "standalone String owner policy must be within the derived bound",
        ));
    }

    let mut types = Vec::new();
    let mut type_indexes = HashMap::new();
    let imports = [
        ("literal", vec![I32, I32], vec![I64]),
        ("clone", vec![I64], vec![I64]),
        ("concat", vec![I64, I64], vec![I64]),
        ("from_char", vec![I32], vec![I64]),
        ("byte_len", vec![I64], vec![I64]),
        ("char_len", vec![I64], vec![I64]),
        ("eq", vec![I64, I64], vec![I32]),
        ("starts_with", vec![I64, I64], vec![I32]),
        ("contains", vec![I64, I64], vec![I32]),
        ("drop", vec![I64], vec![]),
    ];
    let mut import_types = Vec::new();
    for (_, params, results) in &imports {
        import_types.push(intern_type(
            Signature {
                params: params.clone(),
                results: results.clone(),
            },
            &mut types,
            &mut type_indexes,
        ));
    }
    let mut function_types = Vec::new();
    for function in &functions {
        let mut params = function
            .params
            .iter()
            .map(|parameter| scalar_wasm_type(program, &parameter.ty))
            .collect::<Result<Vec<_>, _>>()?;
        params.push(I32);
        function_types.push(intern_type(
            Signature {
                params,
                results: vec![I32],
            },
            &mut types,
            &mut type_indexes,
        ));
    }
    for export in exports {
        function_types.push(intern_type(
            Signature {
                params: export
                    .parameters
                    .iter()
                    .map(|parameter| scalar_wasm_type(program, parameter))
                    .collect::<Result<Vec<_>, _>>()?,
                results: vec![I32],
            },
            &mut types,
            &mut type_indexes,
        ));
    }
    let function_indexes = functions
        .iter()
        .enumerate()
        .map(|(index, function)| {
            (
                FunctionExecutionId::Monomorphic(function.id.clone()),
                IMPORT_COUNT + index as u32,
            )
        })
        .collect::<HashMap<_, _>>();
    let mut module = b"\0asm\x01\0\0\0".to_vec();
    let mut section_bytes = Vec::new();
    write_u32(&mut section_bytes, types.len() as u32);
    for signature in types {
        section_bytes.push(0x60);
        write_bytes(&mut section_bytes, &signature.params);
        write_bytes(&mut section_bytes, &signature.results);
    }
    section(&mut module, 1, section_bytes);
    let mut section_bytes = Vec::new();
    write_u32(&mut section_bytes, IMPORT_COUNT);
    for ((name, _, _), ty) in imports.iter().zip(import_types) {
        function_import(&mut section_bytes, "semaprax.internal-strings.v1", name, ty);
    }
    section(&mut module, 2, section_bytes);
    let mut section_bytes = Vec::new();
    write_u32(&mut section_bytes, function_types.len() as u32);
    for ty in function_types {
        write_u32(&mut section_bytes, ty);
    }
    section(&mut module, 3, section_bytes);
    section(&mut module, 5, vec![1, 1, 4, 4]);
    let mut globals = vec![1, I32, 1, 0x41];
    write_i64(&mut globals, i64::from(SHADOW_STACK_TOP));
    globals.push(0x0b);
    section(&mut module, 6, globals);
    let mut export_section = Vec::new();
    write_u32(&mut export_section, exports.len() as u32 + 2);
    for (name, kind) in [("memory", 2), ("__spx_stack_pointer", 3)] {
        write_name(&mut export_section, name);
        export_section.extend([kind, 0]);
    }
    for ordinal in 0..exports.len() {
        write_name(&mut export_section, &format!("__spx_call_{ordinal}"));
        export_section.push(0);
        write_u32(
            &mut export_section,
            IMPORT_COUNT + functions.len() as u32 + ordinal as u32,
        );
    }
    section(&mut module, 7, export_section);
    let mut code = Vec::new();
    write_u32(
        &mut code,
        function_types_count(functions.len(), exports.len())?,
    );
    let mut literals = OwnedUtf8Literals::default();
    for function in functions {
        let body = emit_function_profile(
            program,
            function,
            &function_indexes,
            &HashMap::new(),
            &HashMap::new(),
            &layouts,
            None,
            None,
            Some(&mut literals),
            None,
            None,
            true,
        )?;
        append_body(&mut code, body)?;
    }
    for export in exports {
        let target = function_indexes[&FunctionExecutionId::Monomorphic(export.id.clone())];
        append_body(&mut code, wrapper(export, target))?;
    }
    section(&mut module, 10, code);
    let mut data = vec![1, 0, 0x41];
    write_i64(&mut data, i64::from(OWNED_UTF8_LITERAL_BASE));
    data.push(0x0b);
    write_bytes(&mut data, &literals.bytes);
    section(&mut module, 11, data);
    if module.len() > MAX_MODULE_BYTES {
        return Err(error("standalone String module exceeds 16 MiB"));
    }
    wasmparser::Validator::new()
        .validate_all(&module)
        .map_err(|failure| {
            error(format!(
                "standalone String module validation failed: {failure}"
            ))
        })?;
    Ok((module, stack, capacity))
}

#[cfg(test)]
mod tests {
    use super::canonical_cleanup_emission_work;

    #[test]
    fn canonical_string_cleanup_counts_each_exit_finalizer() {
        // These are actual `finalize_in_order` lengths for four generated
        // exit paths. Each path emits its own 65_536-action finalizer
        // sequence, so repeated owners are intentionally charged again.
        assert_eq!(
            canonical_cleanup_emission_work([65_536, 65_536, 65_536, 65_536]),
            Some(262_144)
        );
        assert_eq!(
            canonical_cleanup_emission_work([65_536, 65_536, 65_536, 65_536, 1]),
            None
        );
    }
}

fn function_types_count(functions: usize, exports: usize) -> Result<u32, Diagnostic> {
    u32::try_from(functions + exports)
        .map_err(|_| error("standalone String function count overflows"))
}

fn append_body(code: &mut Vec<u8>, body: Vec<u8>) -> Result<(), Diagnostic> {
    if code
        .len()
        .checked_add(body.len())
        .and_then(|size| size.checked_add(5))
        .is_none_or(|size| size > MAX_MODULE_BYTES)
    {
        return Err(error("standalone String code exceeds 16 MiB"));
    }
    write_u32(code, body.len() as u32);
    code.extend(body);
    Ok(())
}

fn wrapper(export: &Export, target: u32) -> Vec<u8> {
    let mut body = vec![0, 0x41];
    write_i64(&mut body, i64::from(RESULT_OFFSET));
    body.extend([0x42, 0, 0x37, 3, 0]); // zero all eight public result bytes
    for (index, parameter) in export.parameters.iter().enumerate() {
        if *parameter == ResolvedType::Bool {
            body.push(0x20);
            write_u32(&mut body, index as u32);
            body.extend([0x41, 1, 0x4b, 0x04, 0x40, 0, 0x0b]); // unsigned >1 traps
        }
    }
    for index in 0..export.parameters.len() {
        body.push(0x20);
        write_u32(&mut body, index as u32);
    }
    body.push(0x41);
    write_i64(&mut body, i64::from(RESULT_OFFSET));
    body.push(0x10);
    write_u32(&mut body, target);
    body.push(0x0b);
    body
}

impl Emitter<'_> {
    pub(super) fn string_capacity_guard(&mut self, local: u32) -> Result<(), Diagnostic> {
        self.output.push(0x20);
        write_u32(self.output, local);
        self.output.push(0x50);
        // Allocation policy is not a source CleanupPlan failure source. There
        // are no resources in this profile; the common String sweep owns it.
        let source = self.failure_expression.take();
        let result = self.fail_if(11);
        self.failure_expression = source;
        result
    }

    pub(super) fn drop_internal_string(&mut self, value: &Value) -> Result<(), Diagnostic> {
        if let Value::Scalar {
            local,
            ty: ResolvedType::String,
        } = value
        {
            owned_strings::emit_drop(self.output, *local);
            Ok(())
        } else {
            Err(error("standalone String finalizer requires a local owner"))
        }
    }

    pub(super) fn emit_internal_string_operation(
        &mut self,
        expr: &ResolvedExpr,
        operation: crate::string_ops::StringOp,
        args: &[ResolvedExpr],
    ) -> Result<Value, Diagnostic> {
        use crate::string_ops::StringOp;
        if operation.is_wasm_refused() {
            return Err(crate::string_ops::text_toolkit_wasm_refusal(operation));
        }
        if args.len() != operation.arity() {
            return Err(error("standalone String operation arity disagrees"));
        }
        require_type(
            &expr.ty,
            &operation.return_type(),
            "String operation result",
        )?;
        let mut values = Vec::with_capacity(args.len());
        for (parameter_index, (argument, expected)) in
            args.iter().zip(operation.param_types()).enumerate()
        {
            let value = self.emit_expr(argument)?;
            require_type(value_type(&value), expected, "String operation argument")?;
            if operation.consumes_arguments() {
                let epoch = crate::cleanup_plan::StorageId::CallArgument {
                    call: expr.id.clone(),
                    parameter_index: u32::try_from(parameter_index)
                        .map_err(|_| error("String operation argument index overflows u32"))?,
                    value_expression: argument.id.clone(),
                };
                let carrier = self
                    .plan
                    .cleanup_call_argument_carriers
                    .get(&epoch)
                    .copied()
                    .ok_or_else(|| {
                        error("consuming String operation has no canonical call epoch carrier")
                    })?;
                values.push(Value::Scalar {
                    local: carrier,
                    ty: expected.clone(),
                });
            } else {
                values.push(value);
            }
        }
        // `concat` owns its arguments. The child expression transitions have
        // already moved their carriers into these epochs; commit before the
        // host operation so a later finalizer cannot see a consumed source.
        self.apply_call_commit(&expr.id)?;
        let destination = self.plan.expr_scalar(expr)?;
        if expr.ty == ResolvedType::String {
            owned_strings::emit_empty_guard(self.output, destination);
        }
        for value in &values {
            self.get_scalar(value);
        }
        let index = match operation {
            StringOp::Concat => 2,
            StringOp::FromChar => 3,
            StringOp::Len | StringOp::IsEmpty => 4,
            StringOp::LenChars => 5,
            StringOp::StartsWith => 7,
            StringOp::Contains => 8,
            StringOp::FromI64
            | StringOp::FromUsize
            | StringOp::Slice
            | StringOp::Find
            | StringOp::ToI64
            | StringOp::Trim
            | StringOp::ByteAt
            | StringOp::FileReadText => {
                return Err(Diagnostic::io(
                    "SPX-W116",
                    "numeric-to-text operations require the scalar Core-Wasm string lane",
                ));
            }
            _ => return Err(crate::string_ops::text_toolkit_wasm_refusal(operation)),
        };
        self.output.push(0x10);
        write_u32(self.output, index);
        if operation == StringOp::IsEmpty {
            self.output.push(0x50);
        }
        self.output.push(0x21);
        write_u32(self.output, destination);
        if operation == StringOp::Concat {
            // The host concat reads both staged carriers and returns a fresh
            // carrier. These committed call epochs are then physically
            // consumed once; their canonical flags were cleared above.
            for value in &values {
                self.drop_internal_string(value)?;
            }
        }
        if expr.ty == ResolvedType::String {
            self.string_capacity_guard(destination)?;
        }
        Ok(Value::Scalar {
            local: destination,
            ty: expr.ty.clone(),
        })
    }
}
