//! Conservative table demand for the import-free Bytes iterator runtime.
//! Count every reachable construction, including repeated helper calls.
//! Iterator conversion, push and next reuse a row and never add a live row.
use super::error;
use crate::diagnostic::Diagnostic;
use crate::hir::{
    DeclarationId, ResolvedExpr, ResolvedExprKind as K, ResolvedProgram, ResolvedStatement as S,
    ResolvedType,
};
use crate::vec_ops::VecOp;

const ROWS: u32 = 32;
const CAPACITY: u64 = 32;
const WORK: usize = 100_000;

#[derive(Clone, Copy)]
enum Count {
    Unseen,
    Active,
    Complete(u32),
}

pub(super) fn prove(program: &ResolvedProgram, entry: &DeclarationId) -> Result<u32, Diagnostic> {
    prove_demand(program, entry)
        .ok_or_else(|| error("public-generic Wasm cannot prove bounded Bytes Vec table demand"))
}

fn prove_demand(program: &ResolvedProgram, entry: &DeclarationId) -> Option<u32> {
    let size = program
        .functions
        .len()
        .checked_add(program.function_instances.len())?;
    let bytes = std::alloc::Layout::array::<Count>(size).ok()?.size();
    let mut counts = Vec::new();
    if size >= WORK || !crate::bounded_output::reserve_active_required(bytes) {
        return None;
    }
    if counts.try_reserve_exact(size).is_err() || counts.capacity() != size {
        crate::bounded_output::refuse_active_required(bytes);
        return None;
    }
    counts.resize(size, Count::Unseen);
    let mut demand = Demand {
        program,
        remaining: WORK,
        counts,
    };
    demand.pay(size)?;
    let entry = program
        .functions
        .iter()
        .position(|function| function.id == *entry)?;
    demand.execution(entry, 0)
}

struct Demand<'a> {
    program: &'a ResolvedProgram,
    remaining: usize,
    // Monomorphic functions precede materialized instances, in HIR order.
    counts: Vec<Count>,
}

impl Demand<'_> {
    fn pay(&mut self, amount: usize) -> Option<()> {
        self.remaining = self.remaining.checked_sub(amount)?;
        Some(())
    }

    fn execution(&mut self, index: usize, depth: usize) -> Option<u32> {
        self.pay(1)?;
        match self.counts[index] {
            Count::Active => return None,
            Count::Complete(count) => return Some(count),
            Count::Unseen => {}
        }
        if depth >= crate::cache_codec::MAX_DEPTH {
            return None;
        }
        let function = if index < self.program.functions.len() {
            &self.program.functions[index]
        } else {
            &self.program.function_instances[index - self.program.functions.len()].function
        };
        self.counts[index] = Count::Active;
        let mut count = 0;
        for root in function
            .requires
            .iter()
            .chain([&function.body])
            .chain(&function.ensures)
        {
            count = add(count, self.expression(root, depth + 1)?)?;
        }
        self.counts[index] = Count::Complete(count);
        Some(count)
    }

    fn expression(&mut self, expression: &ResolvedExpr, depth: usize) -> Option<u32> {
        self.pay(1)?;
        if depth >= crate::cache_codec::MAX_DEPTH {
            return None;
        }
        let depth = depth + 1;
        match &expression.kind {
            K::Closure { .. }
            | K::FunctionReference { .. }
            | K::Invoke { .. }
            | K::HostCommandCall(_)
            | K::NativeRustImportCall(_)
            | K::VecFieldRead { .. }
            | K::RowLoan { .. }
            | K::Yield { .. } => return None,
            K::Block { statements, tail }
            | K::VecConstructor {
                statements, tail, ..
            } => {
                let mut count = 0;
                for statement in statements {
                    let next = match statement {
                        S::While {
                            condition, body, ..
                        }
                        | S::LexicalWhile {
                            condition, body, ..
                        } => {
                            if self.expression(condition, depth)? != 0
                                || self.expression(body, depth)? != 0
                            {
                                return None;
                            }
                            0
                        }
                        _ => self.expression(statement.value(), depth)?,
                    };
                    count = add(count, next)?;
                }
                return add(count, self.expression(tail, depth)?);
            }
            _ => {}
        }

        let mut count = 0;
        for child in crate::hir::lexical_exit::ChildCursor::new(expression) {
            count = add(count, self.expression(child, depth)?)?;
        }
        if let K::Call {
            callee,
            instance,
            type_arguments,
            args,
        } = &expression.kind
        {
            if instance.is_none() {
                if let Some(op) = crate::vec_ops::by_id(callee.as_str()) {
                    if type_arguments.as_slice() != [ResolvedType::Bytes] {
                        return None;
                    }
                    return match op {
                        VecOp::WithCapacity
                            if matches!(args.as_slice(), [arg]
                            if matches!(arg.kind, K::Usize(n) if n <= CAPACITY)) =>
                        {
                            add(count, 1)
                        }
                        VecOp::Push | VecOp::Len | VecOp::Capacity => Some(count),
                        _ => None,
                    };
                }
                if crate::iterator_ops::by_id(callee.as_str()).is_some() {
                    return (type_arguments.as_slice() == [ResolvedType::Bytes]).then_some(count);
                }
                if let Some(op) = crate::byte_ops::by_id(callee.as_str()) {
                    return (!matches!(
                        op,
                        crate::byte_ops::ByteOp::Partition
                            | crate::byte_ops::ByteOp::Set5
                            | crate::byte_ops::ByteOp::Set1Or5
                            | crate::byte_ops::ByteOp::Set1Or6Or48
                    ))
                    .then_some(count);
                }
            }
            self.pay(self.counts.len())?;
            let index = match instance {
                None => self
                    .program
                    .functions
                    .iter()
                    .position(|function| function.id == *callee),
                Some(instance) => self
                    .program
                    .function_instances
                    .iter()
                    .position(|candidate| {
                        candidate.id == *instance && candidate.template == *callee
                    })
                    .map(|index| self.program.functions.len() + index),
            };
            if let Some(index) = index {
                return add(count, self.execution(index, depth)?);
            }
            if instance.is_none()
                && crate::string_ops::by_id(callee.as_str())
                    .is_some_and(|op| op.is_integer_conversion())
            {
                return Some(count);
            }
            return None;
        }
        Some(count)
    }
}

fn add(left: u32, right: u32) -> Option<u32> {
    left.checked_add(right).filter(|total| *total <= ROWS)
}

#[cfg(test)]
mod tests;
