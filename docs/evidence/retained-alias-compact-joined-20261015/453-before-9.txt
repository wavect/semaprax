//! Exact preclone backing for the selected expression-fact owner.
use super::*;
use crate::cleanup_plan::replay::physical_storage::{btree_node_upper, PhysicalReservation};
use std::mem::size_of;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct Failure {
    pub(super) detail: &'static str,
    pub(super) physical: bool,
}

pub(super) enum Payer<'p> {
    Selected(&'p mut PhysicalReservation),
    ActiveBuilder,
    #[cfg(test)]
    Oracle(crate::cleanup_plan::replay::allocation::OracleLease),
}
impl Payer<'_> {
    fn reserve(&mut self, bytes: usize) -> Result<(), ()> {
        match self {
            Self::Selected(physical) => physical.reserve(bytes).map_err(|_| ()),
            #[cfg(test)]
            Self::Oracle(lease) => {
                if lease.refused() {
                    Err(())
                } else {
                    lease.charge(bytes).map_err(|_| ())
                }
            }
            Self::ActiveBuilder => {
                if crate::bounded_output::active_refused()
                    || !crate::bounded_output::reserve_active_required(bytes)
                {
                    Err(())
                } else {
                    Ok(())
                }
            }
        }
    }
}
type TypeCursor<'a> = [Option<(&'a ResolvedType, usize)>; 514];
pub(super) struct FactCapacity<'a, 'p> {
    _function: &'a ResolvedFunction,
    physical: Payer<'p>,
    types: TypeCursor<'a>,
    work: usize,
}
impl<'a, 'p> FactCapacity<'a, 'p> {
    pub(super) fn with_payer(
        function: &'a ResolvedFunction,
        mut physical: Payer<'p>,
    ) -> Result<Self, Failure> {
        // One reused type cursor and the collector's one active expression cursor.
        physical
            .reserve(size_of::<Self>() + size_of::<[FactFrame<'_>; 514]>())
            .map_err(|_| Failure {
                detail: "physical expression-fact cursor reservation refused",
                physical: true,
            })?;
        Ok(Self {
            _function: function,
            physical,
            types: [None; 514],
            work: MAX_REPLAY_WORK_UNITS,
        })
    }
    fn reserve(&mut self, bytes: usize) -> Result<(), Failure> {
        self.physical.reserve(bytes).map_err(|_| Failure {
            detail: "physical expression-fact reservation refused",
            physical: true,
        })
    }
    fn array<T>(&mut self, count: usize) -> Result<(), Failure> {
        let bytes = count.checked_mul(size_of::<T>()).ok_or(Failure {
            detail: "physical expression-fact array reservation refused",
            physical: true,
        })?;
        self.physical.reserve(bytes).map_err(|_| Failure {
            detail: "physical expression-fact array reservation refused",
            physical: true,
        })
    }
    fn types(&mut self, root: &'a ResolvedType) -> Result<(), Failure> {
        self.types[0] = Some((root, 0));
        let mut len = 1;
        while len != 0 {
            self.work = self.work.checked_sub(1).ok_or_else(|| Failure {
                detail: "expression-fact type work exceeds limit",
                physical: false,
            })?;
            let (ty, next) = self.types[len - 1].expect("type cursor frame");
            if next == 0 {
                match ty {
                    ResolvedType::Function { parameters, .. } => {
                        self.array::<ResolvedType>(parameters.len())?;
                        self.reserve(size_of::<ResolvedType>())?;
                    }
                    ResolvedType::Nominal {
                        declaration,
                        arguments,
                    } => {
                        self.reserve(declaration.as_str().len())?;
                        self.array::<ResolvedType>(arguments.len())?;
                    }
                    ResolvedType::TypeParameter { owner, .. } => {
                        self.reserve(owner.as_str().len())?
                    }
                    ResolvedType::Never
                    | ResolvedType::OnceFunction
                    | ResolvedType::OnceFunctionI64
                    | ResolvedType::OnceFunctionI64Pair
                    | ResolvedType::MutFunctionI64
                    | ResolvedType::Unit
                    | ResolvedType::I64
                    | ResolvedType::I32
                    | ResolvedType::Char
                    | ResolvedType::U8
                    | ResolvedType::Usize
                    | ResolvedType::ArrayU8(_)
                    | ResolvedType::F32
                    | ResolvedType::F64
                    | ResolvedType::Bool
                    | ResolvedType::String
                    | ResolvedType::Bytes
                    | ResolvedType::Str
                    | ResolvedType::SliceU8
                    | ResolvedType::StringMap => {}
                }
            }
            let child = match ty {
                ResolvedType::Function { parameters, result } => {
                    if next == 0 {
                        Some(result.as_ref())
                    } else {
                        parameters.get(next - 1)
                    }
                }
                ResolvedType::Nominal { arguments, .. } => arguments.get(next),
                _ => None,
            };
            if let Some(child) = child {
                if len == self.types.len() {
                    return Err(Failure {
                        detail: "expression-fact type depth exceeds limit",
                        physical: false,
                    });
                }
                self.types[len - 1] = Some((ty, next + 1));
                self.types[len] = Some((child, 0));
                len += 1;
            } else {
                self.types[len - 1] = None;
                len -= 1;
            }
        }
        Ok(())
    }
    pub(super) fn expression(&mut self, expression: &'a ResolvedExpr) -> Result<(), Failure> {
        self.work = self.work.checked_sub(1).ok_or_else(|| Failure {
            detail: "expression-fact inventory exceeds work limit",
            physical: false,
        })?;
        self.reserve(
            btree_node_upper::<ExpressionId, Option<CallFact<'_>>>().ok_or_else(|| Failure {
                detail: "expression-fact tree layout overflow",
                physical: false,
            })?,
        )?;
        // ExpressionId clones retain existing Arc backing; only the new map and
        // argument slots historically allocated. Keep their clone reservations
        // conservatively while payloads now borrow the current HIR.
        if let Some((callee, args)) = crate::hir::closure::once::call(expression) {
            self.reserve(callee.as_str().len())?;
            return self.array::<ExpressionId>(args.len());
        }
        match &expression.kind {
            ResolvedExprKind::Invoke { callable, args } => {
                self.reserve("core.function.invoke".len())?;
                self.array::<ExpressionId>(args.len())?;
                self.array::<ResolvedType>(1)?;
                self.types(&callable.ty)?;
            }
            ResolvedExprKind::VecFieldRead { args, .. } => {
                self.reserve(crate::vec_field::ID.len())?;
                self.array::<ExpressionId>(args.len())?;
            }
            ResolvedExprKind::LiteralFormat { args, .. } => {
                self.reserve(crate::literal_format::ID.len())?;
                self.array::<ExpressionId>(args.len())?;
            }
            ResolvedExprKind::Call {
                callee,
                instance,
                args,
                type_arguments,
            } => {
                self.reserve(callee.as_str().len())?;
                if let Some(instance) = instance {
                    self.reserve(instance.as_str().len())?;
                }
                self.array::<ExpressionId>(args.len())?;
                self.array::<ResolvedType>(type_arguments.len())?;
                for ty in type_arguments {
                    self.types(ty)?;
                }
            }
            ResolvedExprKind::NativeRustImportCall(call)
                if crate::cleanup_plan::native_rust::owns(expression) =>
            {
                self.reserve(call.import.as_str().len())?;
                self.array::<ExpressionId>(call.args.len())?;
            }
            ResolvedExprKind::HostCommandCall(call) => {
                // The callee is a fixed compiler identity and allocates nothing;
                // its original text term stays reserved, conservatively.
                self.reserve(crate::command_io_ops::id(call.operation).len())?;
                self.array::<ExpressionId>(call.args.len())?;
            }
            ResolvedExprKind::ByteRange { operation, .. } => {
                self.reserve(operation.as_str().len())?;
                self.array::<ExpressionId>(3)?;
            }
            _ => {}
        }
        Ok(())
    }
    pub(super) fn arena_expression(&mut self, expression: &'a ResolvedExpr) -> Result<(), Failure> {
        self.work = self.work.checked_sub(1).ok_or_else(|| Failure {
            detail: "expression-fact inventory exceeds work limit",
            physical: false,
        })?;
        // ExpressionId clones retain existing Arc backing; only the new map and
        // argument slots historically allocated. Keep their clone reservations
        // conservatively while payloads now borrow the current HIR.
        if let Some((callee, args)) = crate::hir::closure::once::call(expression) {
            self.reserve(callee.as_str().len())?;
            return self.array::<ExpressionId>(args.len());
        }
        match &expression.kind {
            ResolvedExprKind::Invoke { callable, args } => {
                self.reserve("core.function.invoke".len())?;
                self.array::<ExpressionId>(args.len())?;
                self.array::<ResolvedType>(1)?;
                self.types(&callable.ty)?;
            }
            ResolvedExprKind::VecFieldRead { args, .. } => {
                self.reserve(crate::vec_field::ID.len())?;
                self.array::<ExpressionId>(args.len())?;
            }
            ResolvedExprKind::LiteralFormat { args, .. } => {
                self.reserve(crate::literal_format::ID.len())?;
                self.array::<ExpressionId>(args.len())?;
            }
            ResolvedExprKind::Call {
                callee,
                instance,
                args,
                type_arguments,
            } => {
                self.reserve(callee.as_str().len())?;
                if let Some(instance) = instance {
                    self.reserve(instance.as_str().len())?;
                }
                self.array::<ExpressionId>(args.len())?;
                self.array::<ResolvedType>(type_arguments.len())?;
                for ty in type_arguments {
                    self.types(ty)?;
                }
            }
            ResolvedExprKind::NativeRustImportCall(call)
                if crate::cleanup_plan::native_rust::owns(expression) =>
            {
                self.reserve(call.import.as_str().len())?;
                self.array::<ExpressionId>(call.args.len())?;
            }
            ResolvedExprKind::HostCommandCall(call) => {
                // The callee is a fixed compiler identity and allocates nothing;
                // its original text term stays reserved, conservatively.
                self.reserve(crate::command_io_ops::id(call.operation).len())?;
                self.array::<ExpressionId>(call.args.len())?;
            }
            ResolvedExprKind::ByteRange { operation, .. } => {
                self.reserve(operation.as_str().len())?;
                self.array::<ExpressionId>(3)?;
            }
            _ => {}
        }
        Ok(())
    }
    pub(super) fn arena_reserve_array<T>(&mut self, count: usize) -> Result<(), Failure> {
        self.array::<T>(count)
    }
    pub(super) fn arena_work(&mut self, units: usize) -> Result<(), Failure> {
        let remaining = self.work.checked_sub(units).ok_or(Failure {
            detail: "expression-fact arena work exceeds limit",
            physical: false,
        })?;
        self.work = remaining;
        Ok(())
    }
}
