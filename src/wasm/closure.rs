//! Closure-only aggregate carrier and environment adapters. Legacy function
//! values retain their original table-index ABI when no closure is retained.
use super::*;

pub(super) fn adapter_types(
    plan: &function_value::TablePlan,
    types: &mut Vec<Signature>,
    indexes: &mut HashMap<Signature, u32>,
) -> Result<Vec<u32>, Diagnostic> {
    if !plan.closure_profile {
        return Ok(Vec::new());
    }
    plan.targets
        .iter()
        .map(|target| {
            let signature = crate::hir::function_value::signature(target)
                .ok_or_else(|| error("closure adapter target signature is invalid"))?;
            let mut abi = function_value::abi_signature(&signature)?;
            abi.params.insert(0, I32);
            Ok(intern_type(abi, types, indexes))
        })
        .collect()
}

/// These wrappers have `(environment, authored arguments, result out)->status`.
/// Each environment slot has its target-authenticated scalar type; no runtime
/// tag or ambient memory allocation participates in capture interpretation.
pub(super) fn append_adapters(
    code: &mut Vec<u8>,
    plan: &function_value::TablePlan,
    indexes: &HashMap<FunctionExecutionId, u32>,
) -> Result<(), Diagnostic> {
    if !plan.closure_profile {
        return Ok(());
    }
    for target in &plan.targets {
        let mut body = vec![0]; // no locals
        if let Some(captures) = plan.captures.get(&target.id) {
            for (ordinal, ty) in captures.iter().enumerate() {
                body.extend([0x20, 0x00]);
                let (opcode, align) = load(ty)?;
                body.push(opcode);
                write_u32(&mut body, align);
                write_u32(&mut body, 8 + ordinal as u32 * 8);
            }
        }
        for index in 1..=target.params.len() + 1 {
            body.push(0x20);
            write_u32(&mut body, index as u32);
        }
        body.push(0x10);
        write_u32(
            &mut body,
            *indexes
                .get(&FunctionExecutionId::Monomorphic(target.id.clone()))
                .ok_or_else(|| error("closure adapter body is not indexed"))?,
        );
        body.push(0x0b);
        write_u32(code, body.len() as u32);
        code.extend(body);
    }
    Ok(())
}

fn load(ty: &ResolvedType) -> Result<(u8, u32), Diagnostic> {
    match scalar_wasm_type(ty)? {
        I64 => Ok((0x29, 3)),
        F32 => Ok((0x2a, 2)),
        F64 => Ok((0x2b, 3)),
        I32 => Ok((0x28, 2)),
        _ => Err(error("closure capture scalar type is invalid")),
    }
}

impl Emitter<'_> {
    pub(super) fn closure_profile(&self) -> bool {
        crate::hir::closure::requires_closures(self.program)
    }

    fn closure_destination(&self, expression: &ResolvedExpr) -> Result<Pointer, Diagnostic> {
        self.plan.expr_pointer(expression)
    }

    pub(super) fn emit_closure_reference(
        &mut self,
        expression: &ResolvedExpr,
        target: &DeclarationId,
    ) -> Result<Value, Diagnostic> {
        let pointer = self.closure_destination(expression)?;
        self.emit_pointer(pointer);
        self.output.extend([0x41, 0x00, 0x41]);
        write_i64(self.output, 80);
        self.output.extend([0xfc, 0x0b, 0x00]);
        self.emit_pointer(pointer);
        self.output.push(0x41);
        write_i64(
            self.output,
            i64::from(
                *self
                    .function_tables
                    .get(target)
                    .ok_or_else(|| error("closure carrier target is absent"))?,
            ),
        );
        self.output.extend([0x36, 0x02, 0x00]);
        Ok(Value::Aggregate {
            pointer,
            ty: expression.ty.clone(),
        })
    }

    pub(super) fn emit_closure(&mut self, expression: &ResolvedExpr) -> Result<Value, Diagnostic> {
        let ResolvedExprKind::Closure { captures, .. } = &expression.kind else {
            return Err(error("closure lowering received another expression"));
        };
        let value = self
            .emit_closure_reference(expression, &crate::hir::closure::closure_id(&expression.id))?;
        let Value::Aggregate { pointer, .. } = &value else {
            unreachable!()
        };
        for (ordinal, capture) in captures.iter().enumerate() {
            let snapshot = self.emit_expr(&capture.value)?;
            self.require_scalar(&snapshot, &capture.binding.ty, "closure snapshot")?;
            self.emit_pointer(Pointer {
                local: pointer.local,
                offset: pointer.offset + 8 + ordinal as u32 * 8,
            });
            self.get_scalar(&snapshot);
            self.store_scalar(&capture.binding.ty);
        }
        Ok(value)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::process::Command;
    #[test]
    fn wasm_closures_failure_retains_status_output_sentinel_and_restores_frames() {
        let source = r#"
module test.wasm_closure_failure;
@id("closure.probe") fn probe(divisor:i64)->i64 {
    let numerator=42;
    let callback=fn(value:i64)->i64 { numerator/value };
    callback(divisor)
}
@id("app.main") fn main()->i64 { probe(2) }
"#;
        let checked = crate::check(source, "closure-failure.spx").unwrap();
        let program = crate::hir::resolve(&checked).unwrap();
        let bytes = emit_profile(&program, true, false).unwrap();
        assert_eq!(bytes, emit_profile(&program, true, false).unwrap());
        let available = Command::new("node").arg("--version").output().is_ok();
        assert!(available || std::env::var_os("SPX_REQUIRE_NODE").is_none());
        if !available {
            return;
        }
        let root =
            std::env::temp_dir().join(format!("semaprax-closure-failure-{}", std::process::id()));
        std::fs::create_dir_all(&root).unwrap();
        std::fs::write(root.join("module.wasm"), bytes).unwrap();
        let name = function_value::hex_identity(&DeclarationId::new("closure.probe"));
        let script = format!(
            r#"
import {{readFile}} from 'node:fs/promises';
const bytes=await readFile(new URL('./module.wasm',import.meta.url));
const fail=()=>{{throw Error('unexpected host operation')}};
const {{instance}}=await WebAssembly.instantiate(bytes,{{env:{{spx_add:fail,spx_sub:fail,spx_mul:fail,spx_div:fail,spx_rem:fail,spx_neg:fail,spx_contract_fail:fail}}}});
const output=2048,memory=instance.exports.__spx_test_memory,view=new DataView(memory.buffer);
const top=instance.exports.__spx_test_shadow_stack.value;
for(let run=0;run<3;run++){{
  new Uint8Array(memory.buffer,output,8).fill(0xa5);
  const status=instance.exports.__spx_test_{name}(0n,output);
  if(status!=={status})throw Error(`wrong closure failure status ${{status}}`);
  for(const byte of new Uint8Array(memory.buffer,output,8))if(byte!==0xa5)throw Error('failure published provisional result');
  if(instance.exports.__spx_test_shadow_stack.value!==top)throw Error('failure leaked closure frame');
  if(instance.exports.__spx_test_{name}(2n,output)!==0||view.getBigInt64(output,true)!==21n)throw Error('success after failure changed');
  if(instance.exports.__spx_test_shadow_stack.value!==top)throw Error('success leaked closure frame');
}}
"#,
            status = STATUS_DIV_ZERO
        );
        std::fs::write(root.join("probe.mjs"), script).unwrap();
        let output = Command::new("node")
            .arg(root.join("probe.mjs"))
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        std::fs::remove_dir_all(root).unwrap();
    }
}
