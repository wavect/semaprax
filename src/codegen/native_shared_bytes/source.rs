//! Private original-source receiving. This does not open ordinary admission.
//! Compiler proof uses its existing bounded policy; `work` funds the additional
//! current graph and exact target-consumer walk. It is not a runtime account.
use super::{closure, error};
use crate::codegen::COutput;
use crate::diagnostic::Diagnostic;
use crate::hir::{self, ResolvedExpr, ResolvedExprKind, ResolvedType};

pub(in crate::codegen) struct Projection<'owner, 'source> {
    owner: &'owner hir::byte_partition::ValidatedPartitionProgram<'source>,
    selection: closure::Selection,
}
impl<'owner, 'source> Projection<'owner, 'source> {
    pub(in crate::codegen) fn receive(
        owner: &'owner hir::byte_partition::ValidatedPartitionProgram<'source>,
        work: &mut usize,
    ) -> Result<Self, Diagnostic> {
        let result = (|| {
            let mut entry = None;
            for function in &owner.program().functions {
                if equal(
                    function.id.as_str(),
                    owner.program().entrypoint.as_str(),
                    work,
                )? && entry.replace(function).is_some()
                {
                    return Err(error("partition native entry is duplicated"));
                }
            }
            let entry = entry.ok_or_else(|| error("partition native entry is absent"))?;
            owner.simultaneous_view_bound(entry, work)?;
            for function in &owner.program().functions {
                take(work, 1)?;
                admitted_type(&function.return_type, work)?;
                for parameter in &function.params {
                    take(work, 1)?;
                    admitted_type(&parameter.ty, work)?;
                    if parameter.ownership == hir::OwnershipMode::Borrow {
                        return Err(error(
                            "native partition borrowed parameters await their loan receiver",
                        ));
                    }
                }
                for root in function
                    .requires
                    .iter()
                    .chain(std::iter::once(&function.body))
                    .chain(&function.ensures)
                {
                    walk(root, work)?;
                }
            }
            // This first receiving closure has exactly these physical owner
            // consumers. The walk below refuses every other owned operation.
            let selection = closure::closed_over([
                closure::Consumer::Zeroed,
                closure::Consumer::ZeroedBounded,
                closure::Consumer::Partition,
                closure::Consumer::Move,
                closure::Consumer::CleanupFinalizer,
                closure::Consumer::VecLen,
                closure::Consumer::VecCapacity,
                closure::Consumer::VecMove,
                closure::Consumer::VecDrop,
            ])?;
            Ok(Self { owner, selection })
        })();
        if result.is_err() {
            *work = 0;
        }
        result
    }
    pub(in crate::codegen) fn owner(
        &self,
    ) -> &'owner hir::byte_partition::ValidatedPartitionProgram<'source> {
        self.owner
    }
    pub(in crate::codegen) fn emit(&self) -> Result<String, Diagnostic> {
        crate::codegen::native_emit::emit_partition_source(self)
    }
    pub(in crate::codegen) fn emit_bindings(&self, output: &mut impl COutput) {
        super::emit_selected_runtime(output, &self.selection);
        // Deliberate C profile bindings, not a rewrite of generated cleanup.
        // Ordinary runtime definitions above retain their original public ABI.
        output.push_str(include_str!("source/bindings.c"));
    }
    pub(in crate::codegen) fn emit_entry(&self, output: &mut impl COutput, symbol: &str) {
        output.push_str(&include_str!("source/entry.c").replace("$ENTRY", symbol));
    }
}
fn take(work: &mut usize, units: usize) -> Result<(), Diagnostic> {
    *work = work
        .checked_sub(units)
        .ok_or_else(|| error("native partition receiving work exhausted"))?;
    Ok(())
}
fn equal(a: &str, b: &str, work: &mut usize) -> Result<bool, Diagnostic> {
    take(work, 1)?;
    if a.len() != b.len() {
        return Ok(false);
    }
    for (a, b) in a.bytes().zip(b.bytes()) {
        take(work, 1)?;
        if a != b {
            return Ok(false);
        }
    }
    Ok(true)
}
fn admitted_type(ty: &ResolvedType, work: &mut usize) -> Result<(), Diagnostic> {
    take(work, 1)?;
    match ty {
        ResolvedType::Never
        | ResolvedType::Unit
        | ResolvedType::I64
        | ResolvedType::I32
        | ResolvedType::Bool
        | ResolvedType::U8
        | ResolvedType::Usize
        | ResolvedType::Bytes => Ok(()),
        ResolvedType::Nominal {
            declaration,
            arguments,
        } if equal(declaration.as_str(), "core.vec", work)?
            && arguments == &[ResolvedType::Bytes] =>
        {
            Ok(())
        }
        _ => Err(error(
            "native partition type has no complete shared carrier binding",
        )),
    }
}
fn walk(root: &ResolvedExpr, work: &mut usize) -> Result<(), Diagnostic> {
    take(work, 514)?;
    let mut cursors = [None; 514];
    let mut depth = 0;
    cursors[0] = Some(hir::lexical_exit::ChildCursor::new(root));
    check(root, work)?;
    loop {
        take(work, 1)?;
        let child = cursors[depth].as_mut().expect("retained cursor").next();
        if let Some(child) = child {
            if depth + 1 == cursors.len() {
                return Err(error("native partition receiving depth"));
            }
            check(child, work)?;
            depth += 1;
            cursors[depth] = Some(hir::lexical_exit::ChildCursor::new(child));
        } else if depth == 0 {
            break;
        } else {
            cursors[depth] = None;
            depth -= 1;
        }
    }
    Ok(())
}
fn check(expression: &ResolvedExpr, work: &mut usize) -> Result<(), Diagnostic> {
    take(work, 1)?;
    admitted_type(&expression.ty, work)?;
    match &expression.kind {
        ResolvedExprKind::Call { callee, .. } => {
            for op in crate::byte_ops::ByteOp::ALL
                .into_iter()
                .chain([crate::byte_ops::ByteOp::Partition])
            {
                if equal(callee.as_str(), op.id(), work)? {
                    return if matches!(
                        op,
                        crate::byte_ops::ByteOp::Zeroed
                            | crate::byte_ops::ByteOp::ZeroedBounded
                            | crate::byte_ops::ByteOp::Partition
                    ) {
                        Ok(())
                    } else {
                        Err(error(
                            "native partition byte consumer binding is not complete",
                        ))
                    };
                }
            }
            for op in crate::vec_ops::ALL {
                if equal(callee.as_str(), op.id(), work)? {
                    return if matches!(
                        op,
                        crate::vec_ops::VecOp::Len | crate::vec_ops::VecOp::Capacity
                    ) {
                        Ok(())
                    } else {
                        Err(error(
                            "native partition Vec consumer binding is not complete",
                        ))
                    };
                }
            }
            // The independent current graph proof already requires every
            // remaining call to name a current monomorphic source function.
            Ok(())
        }
        ResolvedExprKind::Block { statements, .. } => {
            for statement in statements {
                take(work, 1)?;
                if !matches!(statement, hir::ResolvedStatement::Let { .. }) {
                    return Err(error("native partition statement binding is not complete"));
                }
            }
            Ok(())
        }
        ResolvedExprKind::Int(_)
        | ResolvedExprKind::Int32(_)
        | ResolvedExprKind::Usize(_)
        | ResolvedExprKind::Uint8(_)
        | ResolvedExprKind::Bool(_)
        | ResolvedExprKind::Place(_)
        | ResolvedExprKind::Binary { .. }
        | ResolvedExprKind::Unary { .. }
        | ResolvedExprKind::If { .. } => Ok(()),
        _ => Err(error("native partition expression binding is not complete")),
    }
}
#[cfg(test)]
#[path = "source/tests.rs"]
mod tests;
