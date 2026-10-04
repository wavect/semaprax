//! Self-contained Core Wasm carrier for the narrow immutable `List<i64>` lane.
//!
//! Zero is Nil. Every Cons occupies one 16-byte cell above the private shadow
//! stack: head:i64, length:u32, tail:i32. A root entry resets the bump cursor
//! to page one; memory grows as needed, and cells are never modified after
//! construction. Thus a copied list and an exposed tail safely share nodes.

use super::*;
use crate::list_ops::{self, ListOp};

const HEAP_START: i64 = 65_536;
const CELL_BYTES: i64 = 16;

pub(in crate::wasm) fn emit_closed_list(
    program: &ResolvedProgram,
    has_public_adapter: bool,
) -> Result<Vec<u8>, Diagnostic> {
    crate::hir::validate(program)?;
    let closed = !has_public_adapter
        && program.permits.is_empty()
        && program.agents.is_empty()
        && program.interfaces.is_empty()
        && program.function_templates.is_empty()
        && program.function_instances.is_empty()
        && program
            .functions
            .iter()
            .all(|function| function.effects.is_empty())
        && program.types.iter().all(|item| {
            program
                .declarations
                .declaration(&item.id)
                .is_some_and(|indexed| {
                    indexed.identity_origin == crate::hir::IdentityOrigin::CompilerOwned
                })
        })
        && !crate::wasm::program_uses_byte_data(program)
        && !crate::wasm::program_uses_vec(program)
        && !crate::wasm::program_uses_box(program)
        && !crate::wasm::program_uses_strings(program)
        && !crate::hir::closure::requires_runtime_closures(program);
    if !closed {
        return Err(Diagnostic::io(
            "SPX-W130",
            "immutable List requires the closed pure Core Wasm profile",
        ));
    }
    super::emit(program)
}

pub(super) fn append_heap_global(globals: &mut Vec<u8>, enabled: bool, base: u32) -> Option<u32> {
    if !enabled {
        return None;
    }
    globals.extend([I32, 0x01, 0x41]);
    write_i64(globals, HEAP_START);
    globals.push(0x0b);
    Some(base + call_admission::GLOBAL_COUNT)
}

pub(super) fn emit_heap_reset(body: &mut Vec<u8>, global: Option<u32>) {
    let Some(global) = global else { return };
    body.push(0x41);
    write_i64(body, HEAP_START);
    body.push(0x24);
    write_u32(body, global);
}

impl Emitter<'_> {
    pub(super) fn emit_list_op(
        &mut self,
        expr: &ResolvedExpr,
        op: ListOp,
        type_arguments: &[ResolvedType],
        args: &[ResolvedExpr],
    ) -> Result<Value, Diagnostic> {
        if !type_arguments.is_empty() || args.len() != op.argument_count() {
            return Err(error("immutable List operation has invalid resolved shape"));
        }
        require_type(
            &expr.ty,
            &op.resolved_return_type(),
            "immutable List result",
        )?;
        let heap = self
            .list_heap_global
            .ok_or_else(|| Diagnostic::io("SPX-W130", "immutable List heap is unavailable"))?;
        match op {
            ListOp::Nil => {
                let local = self.plan.expr_scalar(expr)?;
                self.output.extend([0x41, 0x00, 0x21]);
                write_u32(self.output, local);
                Ok(Value::Scalar {
                    local,
                    ty: expr.ty.clone(),
                })
            }
            ListOp::Cons => self.emit_list_cons(expr, args, heap),
            ListOp::Uncons => self.emit_list_uncons(expr, args),
        }
    }

    fn emit_list_cons(
        &mut self,
        expr: &ResolvedExpr,
        args: &[ResolvedExpr],
        heap: u32,
    ) -> Result<Value, Diagnostic> {
        let head = self.emit_expr(&args[0])?;
        self.require_scalar(&head, &ResolvedType::I64, "immutable List head")?;
        let tail = self.emit_expr(&args[1])?;
        self.require_scalar(&tail, &list_ops::resolved_list(), "immutable List tail")?;
        let local = self.plan.expr_scalar(expr)?;

        // Address zero is valid linear memory. Its load is discarded for Nil,
        // while a nonzero tail always points at a previously initialized cell.
        self.output.extend([0x41, 0x00]);
        self.get_scalar(&tail);
        self.output.extend([0x28, 0x02, 0x08]);
        self.get_scalar(&tail);
        self.output.extend([0x45, 0x1b, 0x41]);
        write_i64(self.output, crate::immutable_list::MAX_LENGTH as i64);
        self.output.push(0x4f); // i32.ge_u
        self.emit_vec_failure_if(expr, STATUS_LIST_LENGTH_LIMIT)?;

        // The final 16-byte address is reserved so cursor arithmetic never
        // wraps u32, even on an engine whose maximum is the full 4 GiB.
        self.output.push(0x23);
        write_u32(self.output, heap);
        self.output.push(0x41);
        write_i64(self.output, -16);
        self.output.push(0x4f); // i32.ge_u
        self.emit_vec_failure_if(expr, STATUS_LIST_MEMORY_LIMIT)?;

        self.output.push(0x23); // heap + cell bytes > memory.size * 65536
        write_u32(self.output, heap);
        self.output.push(0x41);
        write_i64(self.output, CELL_BYTES);
        self.output.push(0x6a);
        self.output.extend([0x3f, 0x00, 0x41, 0x10, 0x74, 0x4b]);
        self.output
            .extend([0x04, I32, 0x41, 0x01, 0x40, 0x00, 0x41]);
        write_i64(self.output, -1);
        self.output.extend([0x46, 0x05, 0x41, 0x00, 0x0b]);
        self.emit_vec_failure_if(expr, STATUS_LIST_MEMORY_LIMIT)?;

        self.output.push(0x23);
        write_u32(self.output, heap);
        self.get_scalar(&head);
        self.output.extend([0x37, 0x03, 0x00]); // head at +0

        self.output.push(0x23);
        write_u32(self.output, heap);
        self.output.extend([0x41, 0x00]);
        self.get_scalar(&tail);
        self.output.extend([0x28, 0x02, 0x08]);
        self.get_scalar(&tail);
        self.output
            .extend([0x45, 0x1b, 0x41, 0x01, 0x6a, 0x36, 0x02, 0x08]);

        self.output.push(0x23);
        write_u32(self.output, heap);
        self.get_scalar(&tail);
        self.output.extend([0x36, 0x02, 0x0c]); // tail at +12

        self.output.push(0x23);
        write_u32(self.output, heap);
        self.output.push(0x21);
        write_u32(self.output, local);
        self.output.push(0x23);
        write_u32(self.output, heap);
        self.output.push(0x41);
        write_i64(self.output, CELL_BYTES);
        self.output.push(0x6a);
        self.output.push(0x24);
        write_u32(self.output, heap);
        Ok(Value::Scalar {
            local,
            ty: expr.ty.clone(),
        })
    }

    fn emit_list_uncons(
        &mut self,
        expr: &ResolvedExpr,
        args: &[ResolvedExpr],
    ) -> Result<Value, Diagnostic> {
        let list = self.emit_expr(&args[0])?;
        self.require_scalar(&list, &list_ops::resolved_list(), "immutable List operand")?;
        let layout = variant_layout(self.variant_layouts, &expr.ty)?;
        if layout.variant.as_str() != list_ops::STEP_ID
            || layout.cases.len() != 2
            || layout.cases[0].case.as_str() != list_ops::NIL_CASE_ID
            || layout.cases[1].case.as_str() != list_ops::CONS_CASE_ID
        {
            return Err(error("immutable ListStep layout is not canonical"));
        }
        let fields = &layout.cases[1].fields;
        if fields.len() != 2
            || fields[0].field.as_str() != list_ops::HEAD_ID
            || fields[1].field.as_str() != list_ops::TAIL_ID
        {
            return Err(error("immutable ListStep payload is not canonical"));
        }
        let pointer = self.plan.expr_pointer(expr)?;
        self.emit_pointer(pointer);
        self.output.extend([0x41, 0x00, 0x41]);
        write_i64(self.output, i64::from(layout.size));
        self.output.extend([0xfc, 0x0b, 0x00]); // memory.fill
        self.get_scalar(&list);
        self.output.extend([0x04, 0x40]);
        self.emit_pointer(pointer);
        self.output.extend([0x41, 0x01, 0x36, 0x02, 0x00]);
        self.emit_pointer(pointer);
        self.get_scalar(&list);
        self.output.extend([0x29, 0x03, 0x00, 0x37, 0x03]);
        write_u32(self.output, layout.payload_offset + fields[0].offset);
        self.emit_pointer(pointer);
        self.get_scalar(&list);
        self.output.extend([0x28, 0x02, 0x0c, 0x36, 0x02]);
        write_u32(self.output, layout.payload_offset + fields[1].offset);
        self.output.push(0x0b);
        Ok(Value::Aggregate {
            pointer,
            ty: expr.ty.clone(),
        })
    }
}
