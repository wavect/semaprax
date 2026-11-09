//! Complete source-to-HIR correspondence of the closed post-construction suffix.
//!
//! Each authored source statement and expression is walked in lockstep with
//! its resolved HIR node. Every HIR expression and binding must carry the
//! canonical function-scoped identity of its source position (including the
//! cached `ValueId` fingerprint) and the source span, and must implement the
//! same operator, literal, call, Option arm, view, field read or owner
//! transfer. Owners are tracked independently: a move, sort or borrowed
//! traversal must name the current live owner, and the source proof's owner
//! events must be exactly the transfers proved here. Equal owner events alone
//! prove nothing. This is proof data; ordinary HIR, ownership, loan, cleanup
//! and status validation under the selected strategy remain mandatory.
use super::{Refusal, Selection};
use crate::ast::{self, BinaryOp, ExprKind as A, Statement as AS, UnaryOp};
use crate::hir::{
    FunctionExecutionId, OwnershipMode as O, ResolvedBinding, ResolvedExpr as E,
    ResolvedExprKind as K, ResolvedMatchPattern as P, ResolvedProgram, ResolvedStatement as S,
    ResolvedType as T,
};
use crate::source_verify::fused_construction::{OwnerEvent, SourceProof, MAX_OWNER_EVENTS};

const SLOTS: usize = 64;

const fn refuse(reason: &'static str) -> Refusal {
    Refusal::Join(reason)
}

fn check(condition: bool, reason: &'static str) -> Result<(), Refusal> {
    if condition {
        Ok(())
    } else {
        Err(refuse(reason))
    }
}

#[derive(Clone, Copy, PartialEq)]
enum Kind {
    Owner,
    Value,
    Row,
}

#[derive(Clone, Copy, PartialEq)]
enum Transfer {
    Move,
    Sort,
    Borrow,
}

#[derive(Clone, Copy)]
struct Slot<'s, 'a> {
    name: &'s str,
    /// Owner or value binding; for a row alias, the traversal's owner binding.
    binding: &'a ResolvedBinding,
    /// Row alias only: the traversal's canonical row index binding.
    index: Option<&'a ResolvedBinding>,
    kind: Kind,
    mutable: bool,
    live: bool,
}

struct Walk<'s, 'a> {
    program: &'a ResolvedProgram,
    function: FunctionExecutionId,
    selection: &'s Selection<'a>,
    slots: [Option<Slot<'s, 'a>>; SLOTS],
    len: usize,
    work: usize,
    transfers: [Option<(Transfer, &'s ast::Statement)>; MAX_OWNER_EVENTS],
    recorded: usize,
}

fn typed(e: &E, ty: &T, mode: O) -> bool {
    e.ty == *ty && e.ownership == mode
}

fn option_u8(ty: &T) -> bool {
    matches!(ty, T::Nominal { declaration, arguments }
        if declaration.as_str() == crate::prelude::OPTION_ID && arguments.as_slice() == [T::U8])
}

fn scalar(e: &E) -> bool {
    crate::hir::is_scalar_resolved_type(&e.ty) && e.ownership == O::Value
}

fn view(binding: &ResolvedBinding) -> bool {
    matches!(binding.ty, T::Str | T::SliceU8) && binding.ownership == O::Borrow
}

fn call(e: &ast::Expr) -> Option<(&str, &[ast::Type], &[ast::Expr])> {
    match &e.kind {
        A::Call {
            name,
            type_arguments,
            args,
        } => Some((name.as_str(), type_arguments.as_slice(), args.as_slice())),
        _ => None,
    }
}

impl<'s, 'a> Walk<'s, 'a> {
    fn debit(&mut self) -> Result<(), Refusal> {
        self.work = self
            .work
            .checked_sub(1)
            .ok_or(refuse("suffix correspondence work bound"))?;
        Ok(())
    }

    fn expression_id(&self, e: &E, path: &str) -> bool {
        !path.is_empty() && e.id.matches(&self.function, path)
    }

    fn local_id(&self, binding: &ResolvedBinding, path: &str) -> bool {
        !path.is_empty() && binding.id.matches_local(&self.function, path)
    }

    fn lookup(&self, name: &str) -> Result<Slot<'s, 'a>, Refusal> {
        self.slots[..self.len]
            .iter()
            .flatten()
            .find(|slot| slot.name == name && slot.live)
            .copied()
            .ok_or(refuse("suffix name has no live corresponding binding"))
    }

    fn bind(&mut self, slot: Slot<'s, 'a>) -> Result<(), Refusal> {
        self.debit()?;
        // Dead names stay reserved, as in the source census.
        if self.slots[..self.len].iter().flatten().any(|other| {
            other.name == slot.name
                || (other.binding.id == slot.binding.id
                    && slot.kind != Kind::Row
                    && other.kind != Kind::Row)
        }) {
            return Err(refuse("suffix binding reuses a name or value identity"));
        }
        *self
            .slots
            .get_mut(self.len)
            .ok_or(refuse("suffix scope bound"))? = Some(slot);
        self.len += 1;
        Ok(())
    }

    fn kill(&mut self, name: &str) -> Result<(), Refusal> {
        let slot = self.slots[..self.len]
            .iter_mut()
            .flatten()
            .find(|slot| slot.name == name && slot.live)
            .ok_or(refuse("transfer source is not live"))?;
        slot.live = false;
        Ok(())
    }

    fn transfer(&mut self, kind: Transfer, statement: &'s ast::Statement) -> Result<(), Refusal> {
        *self
            .transfers
            .get_mut(self.recorded)
            .ok_or(refuse("owner transfer bound"))? = Some((kind, statement));
        self.recorded += 1;
        Ok(())
    }

    /// The source operand names the current live owner and the HIR operand is
    /// that exact owner place at its canonical position.
    fn owner_operand(
        &self,
        source: &ast::Expr,
        hir: &E,
        path: &str,
        moved: bool,
    ) -> Result<Slot<'s, 'a>, Refusal> {
        let A::Var(name) = &source.kind else {
            return Err(refuse("owner operand is not a name"));
        };
        let slot = self.lookup(name)?;
        check(slot.kind == Kind::Owner, "operand is not the current owner")?;
        check(
            self.expression_id(hir, path)
                && hir.span == source.span
                && hir.ty == slot.binding.ty
                && (hir.ownership == O::Own || !moved && hir.ownership == O::Borrow)
                && matches!(&hir.kind, K::Place(place)
                    if place.root == slot.binding.id && place.projections.is_empty()),
            "owner operand is not the current owner place",
        )?;
        Ok(slot)
    }

    fn owner_binding(&self, binding: &ResolvedBinding) -> bool {
        binding.ty == self.selection.vector().ty && binding.ownership == O::Own
    }

    fn top(&mut self, path: &str, source: &'s ast::Statement, hir: &'a S) -> Result<(), Refusal> {
        self.debit()?;
        let value_path = format!("{path}.value");
        match (source, hir) {
            (
                AS::Let {
                    name,
                    name_span,
                    mutable: false,
                    declared: None,
                    value,
                    ..
                },
                S::Let {
                    binding,
                    mutable: false,
                    value: hir_value,
                    ..
                },
            ) if matches!(&value.kind, A::Var(owner)
                if self.lookup(owner).is_ok_and(|slot| slot.kind == Kind::Owner)) =>
            {
                let A::Var(owner) = &value.kind else {
                    return Err(refuse("owner move differs"));
                };
                self.owner_operand(value, hir_value, &value_path, true)?;
                check(
                    binding.name == *name
                        && binding.span == *name_span
                        && self.local_id(binding, path)
                        && self.owner_binding(binding),
                    "owner move binding differs",
                )?;
                self.kill(owner)?;
                self.bind(Slot {
                    name,
                    binding,
                    index: None,
                    kind: Kind::Owner,
                    mutable: false,
                    live: true,
                })?;
                self.transfer(Transfer::Move, source)
            }
            (
                AS::Let {
                    name,
                    name_span,
                    mutable: false,
                    declared: None,
                    value,
                    ..
                },
                S::Let {
                    binding,
                    mutable: false,
                    value: hir_value,
                    ..
                },
            ) if call(value).is_some_and(|(callee, ..)| callee == "vec_sort_owned") => {
                let Some((_, types, [owner])) = call(value) else {
                    return Err(refuse("owner sort differs"));
                };
                let K::Call {
                    callee,
                    instance: None,
                    type_arguments,
                    args,
                } = &hir_value.kind
                else {
                    return Err(refuse("owner sort differs"));
                };
                let [hir_owner] = args.as_slice() else {
                    return Err(refuse("owner sort differs"));
                };
                check(
                    callee.as_str() == crate::vec_ops::SORT_OWNED_ID
                        && super::join::row_type(types, self.selection.record())
                        && type_arguments.as_slice()
                            == std::slice::from_ref(self.selection.element())
                        && self.expression_id(hir_value, &value_path)
                        && hir_value.span == value.span
                        && typed(hir_value, &self.selection.vector().ty, O::Own),
                    "owner sort differs",
                )?;
                let slot =
                    self.owner_operand(owner, hir_owner, &format!("{value_path}.arg.0"), true)?;
                check(
                    binding.name == *name
                        && binding.span == *name_span
                        && self.local_id(binding, path)
                        && self.owner_binding(binding),
                    "owner sort binding differs",
                )?;
                self.kill(slot.name)?;
                self.bind(Slot {
                    name,
                    binding,
                    index: None,
                    kind: Kind::Owner,
                    mutable: false,
                    live: true,
                })?;
                self.transfer(Transfer::Sort, source)
            }
            (
                AS::ForBorrow {
                    item,
                    item_span,
                    index,
                    values,
                    body,
                    ..
                },
                S::Let {
                    binding,
                    mutable: false,
                    value: region,
                    ..
                },
            ) => {
                let A::Var(owner) = &values.kind else {
                    return Err(refuse("traversal source is not a name"));
                };
                let owner = self.lookup(owner)?;
                check(
                    owner.kind == Kind::Owner
                        && !owner.mutable
                        && self.local_id(binding, path)
                        && self.expression_id(region, &value_path),
                    "traversal identity differs",
                )?;
                // Complete independent freeze replay before any field read.
                let proof = crate::loan_plan::traversal::inspect_borrowed(
                    self.program,
                    region,
                    self.selection.element(),
                    &mut self.work,
                )
                .ok_or(refuse("traversal freeze replay refused"))?
                .ok_or(refuse("traversal freeze region is absent"))?;
                check(
                    proof.source.ty == owner.binding.ty
                        && matches!(&proof.source.kind, K::Place(place)
                            if place.root == owner.binding.id && place.projections.is_empty()),
                    "traversal does not borrow the current owner",
                )?;
                // The replay checks shape and equal roots only; every generated
                // binding and expression must also be at its resolver position.
                self.scaffold(&value_path, region)?;
                let K::Block { statements, .. } = &region.kind else {
                    return Err(refuse("traversal region differs"));
                };
                let Some(
                    S::While {
                        body: iteration, ..
                    }
                    | S::LexicalWhile {
                        body: iteration, ..
                    },
                ) = statements.get(2)
                else {
                    return Err(refuse("traversal region differs"));
                };
                let K::Block {
                    statements: iteration,
                    ..
                } = &iteration.kind
                else {
                    return Err(refuse("traversal iteration differs"));
                };
                let [S::Let {
                    binding: row_index, ..
                }, S::Let {
                    value: authored, ..
                }, _] = iteration.as_slice()
                else {
                    return Err(refuse("traversal iteration differs"));
                };
                check(
                    self.local_id(row_index, &format!("{value_path}.s2.body.s0"))
                        && row_index.ty == T::Usize
                        && row_index.ownership == O::Value
                        // The lowered index keeps the row item's span.
                        && row_index.span == *item_span
                        && match index {
                            Some(index) => row_index.name == index.name,
                            None => row_index.name == "#for-row-index",
                        },
                    "traversal row index differs",
                )?;
                let saved = self.len;
                self.bind(Slot {
                    name: item,
                    binding: owner.binding,
                    index: Some(row_index),
                    kind: Kind::Row,
                    mutable: false,
                    live: true,
                })?;
                if let Some(index) = index {
                    self.bind(Slot {
                        name: &index.name,
                        binding: row_index,
                        index: None,
                        kind: Kind::Value,
                        mutable: false,
                        live: true,
                    })?;
                }
                self.expression(body, authored, &format!("{value_path}.s2.body.s1.value"), 1)?;
                check(
                    typed(authored, &T::I64, O::Value),
                    "traversal body result differs",
                )?;
                self.len = saved;
                self.transfer(Transfer::Borrow, source)
            }
            _ => self.nested(path, source, hir, 0),
        }
    }

    /// Bounded canonical census of the generated borrowed-traversal scaffold
    /// (`resolve_for::lower` as amended by `resolve_for::record::lower`). The
    /// authored body, outer binding and row index are checked by the caller.
    fn scaffold(&mut self, value: &str, region: &'a E) -> Result<(), Refusal> {
        self.debit()?;
        let refused = || refuse("traversal scaffold identity is not its resolver position");
        let K::Block { statements, tail } = &region.kind else {
            return Err(refused());
        };
        let [S::Let {
            binding: length,
            value: length_call,
            ..
        }, S::Let {
            binding: counter,
            value: start,
            ..
        }, iteration_statement] = statements.as_slice()
        else {
            return Err(refused());
        };
        let (condition, iteration) = match iteration_statement {
            S::While {
                condition, body, ..
            } => (condition, body),
            S::LexicalWhile {
                condition,
                body,
                latch,
                ..
            } => {
                let crate::hir::LoopLatchKind::SourceForStep {
                    kind: crate::hir::SourceForKind::BorrowRecord,
                    step,
                } = latch
                else {
                    return Err(refused());
                };
                let step_path = format!("{value}.s2.body.s2.value");
                if step_path.is_empty() || !step.matches(&self.function, &step_path) {
                    return Err(refused());
                }
                (condition, body)
            }
            _ => return Err(refused()),
        };
        let K::Call {
            args: length_args, ..
        } = &length_call.kind
        else {
            return Err(refused());
        };
        let [length_source] = length_args.as_slice() else {
            return Err(refused());
        };
        let K::Binary {
            left: condition_left,
            right: condition_right,
            ..
        } = &condition.kind
        else {
            return Err(refused());
        };
        let K::Block {
            statements: rows,
            tail: iteration_tail,
        } = &iteration.kind
        else {
            return Err(refused());
        };
        let [S::Let {
            value: snapshot, ..
        }, S::Let {
            binding: discarded, ..
        }, S::Assign {
            binding: incremented,
            value: increment,
            ..
        }] = rows.as_slice()
        else {
            return Err(refused());
        };
        let K::Binary {
            left: increment_left,
            right: increment_right,
            ..
        } = &increment.kind
        else {
            return Err(refused());
        };
        let bindings: [(&ResolvedBinding, &str); 4] = [
            (length, ".s0"),
            (counter, ".s1"),
            (discarded, ".s2.body.s1"),
            (incremented, ".s1"),
        ];
        let expressions: [(&E, &str); 14] = [
            (length_call, ".s0.value"),
            (length_source, ".s0.value.arg.0"),
            (start, ".s1.value"),
            (condition.as_ref(), ".s2.condition"),
            (condition_left.as_ref(), ".s2.condition.left"),
            (condition_right.as_ref(), ".s2.condition.right"),
            (iteration.as_ref(), ".s2.body"),
            (snapshot, ".s2.body.s0.value"),
            (increment, ".s2.body.s2.value"),
            (increment_left.as_ref(), ".s2.body.s2.value.left"),
            (increment_right.as_ref(), ".s2.body.s2.value.right"),
            (iteration_tail.as_ref(), ".s2.body.tail"),
            (tail.as_ref(), ".tail"),
            (region, ""),
        ];
        for (binding, suffix) in bindings {
            self.debit()?;
            if !self.local_id(binding, &format!("{value}{suffix}")) {
                return Err(refused());
            }
        }
        for (expression, suffix) in expressions {
            self.debit()?;
            if !self.expression_id(expression, &format!("{value}{suffix}")) {
                return Err(refused());
            }
        }
        Ok(())
    }

    /// Scalar and iteration-local view statements valid at any depth.
    fn nested(
        &mut self,
        path: &str,
        source: &'s ast::Statement,
        hir: &'a S,
        depth: usize,
    ) -> Result<(), Refusal> {
        self.debit()?;
        let value_path = format!("{path}.value");
        match (source, hir) {
            (
                AS::Let {
                    name,
                    name_span,
                    mutable,
                    declared: None,
                    value,
                    ..
                },
                S::Let {
                    binding,
                    mutable: hir_mutable,
                    value: hir_value,
                    ..
                },
            ) => {
                self.expression(value, hir_value, &value_path, depth + 1)?;
                let scalar_binding = crate::hir::is_scalar_resolved_type(&binding.ty)
                    && binding.ownership == O::Value;
                check(
                    mutable == hir_mutable
                        && binding.name == *name
                        && binding.span == *name_span
                        && self.local_id(binding, path)
                        && binding.ty == hir_value.ty
                        && binding.ownership == hir_value.ownership
                        && (scalar_binding || view(binding) && !*mutable),
                    "suffix binding differs",
                )?;
                self.bind(Slot {
                    name,
                    binding,
                    index: None,
                    kind: Kind::Value,
                    mutable: *mutable,
                    live: true,
                })
            }
            (
                AS::Assign {
                    name,
                    field: None,
                    value,
                    ..
                },
                S::Assign {
                    binding,
                    field: None,
                    value: hir_value,
                    ..
                },
            ) => {
                let slot = self.lookup(name)?;
                self.expression(value, hir_value, &value_path, depth + 1)?;
                check(
                    slot.kind == Kind::Value
                        && slot.mutable
                        && binding.id == slot.binding.id
                        && binding.name == slot.binding.name
                        && binding.ty == slot.binding.ty
                        && binding.ownership == slot.binding.ownership
                        && crate::hir::is_scalar_resolved_type(&binding.ty)
                        && hir_value.ty == binding.ty
                        && hir_value.ownership == O::Value,
                    "suffix assignment differs",
                )
            }
            _ => Err(refuse("suffix statement differs")),
        }
    }

    fn expression(
        &mut self,
        source: &'s ast::Expr,
        hir: &'a E,
        path: &str,
        depth: usize,
    ) -> Result<(), Refusal> {
        check(depth <= crate::cache_codec::MAX_DEPTH, "suffix depth bound")?;
        self.debit()?;
        check(
            self.expression_id(hir, path) && hir.span == source.span,
            "suffix expression identity differs",
        )?;
        match (&source.kind, &hir.kind) {
            (A::Int(left), K::Int(right)) => check(
                left == right && typed(hir, &T::I64, O::Value),
                "literal differs",
            ),
            (A::Usize(left), K::Usize(right)) => check(
                left == right && typed(hir, &T::Usize, O::Value),
                "literal differs",
            ),
            (A::Uint8(left), K::Uint8(right)) => check(
                left == right && typed(hir, &T::U8, O::Value),
                "literal differs",
            ),
            (A::Bool(left), K::Bool(right)) => check(
                left == right && typed(hir, &T::Bool, O::Value),
                "literal differs",
            ),
            (A::Var(name), K::Place(place)) => {
                let slot = self.lookup(name)?;
                check(
                    slot.kind == Kind::Value
                        && place.root == slot.binding.id
                        && place.projections.is_empty()
                        && hir.ty == slot.binding.ty
                        && hir.ownership == slot.binding.ownership,
                    "name does not read its corresponding binding",
                )
            }
            (
                A::Project { base, field, .. },
                K::VecFieldRead {
                    origin: crate::hir::VecFieldReadOrigin::Traversal,
                    element,
                    field: field_id,
                    bytes: false,
                    args,
                },
            ) => self.field_read(base, field, hir, element, field_id, args, path),
            (
                A::Unary { op, value },
                K::Unary {
                    op: hir_op,
                    value: hir_value,
                },
            ) => {
                self.expression(value, hir_value, &format!("{path}.value"), depth + 1)?;
                let result = match op {
                    UnaryOp::Not => T::Bool,
                    UnaryOp::Neg => T::I64,
                };
                check(
                    op == hir_op && hir_value.ty == result && typed(hir, &result, O::Value),
                    "unary operator differs",
                )
            }
            (
                A::Binary { op, left, right },
                K::Binary {
                    op: hir_op,
                    left: hir_left,
                    right: hir_right,
                },
            ) => {
                self.expression(left, hir_left, &format!("{path}.left"), depth + 1)?;
                self.expression(right, hir_right, &format!("{path}.right"), depth + 1)?;
                let operand = &hir_left.ty;
                let result = match op {
                    BinaryOp::And | BinaryOp::Or if *operand == T::Bool => T::Bool,
                    BinaryOp::Eq | BinaryOp::Ne => T::Bool,
                    BinaryOp::Lt | BinaryOp::Le | BinaryOp::Gt | BinaryOp::Ge
                        if *operand != T::Bool =>
                    {
                        T::Bool
                    }
                    BinaryOp::Add | BinaryOp::Sub | BinaryOp::Mul
                        if matches!(operand, T::I64 | T::Usize) =>
                    {
                        operand.clone()
                    }
                    _ => return Err(refuse("binary operator outside the closed domain")),
                };
                check(
                    op == hir_op
                        && scalar(hir_left)
                        && scalar(hir_right)
                        && hir_right.ty == *operand
                        && typed(hir, &result, O::Value),
                    "binary operator differs",
                )
            }
            (
                A::If {
                    condition,
                    then_branch,
                    else_branch,
                },
                K::If {
                    condition: hir_condition,
                    then_branch: hir_then,
                    else_branch: hir_else,
                },
            ) => {
                self.expression(
                    condition,
                    hir_condition,
                    &format!("{path}.condition"),
                    depth + 1,
                )?;
                self.expression(then_branch, hir_then, &format!("{path}.then"), depth + 1)?;
                self.expression(else_branch, hir_else, &format!("{path}.else"), depth + 1)?;
                check(
                    typed(hir_condition, &T::Bool, O::Value)
                        && scalar(hir)
                        && hir_then.ty == hir.ty
                        && hir_else.ty == hir.ty,
                    "conditional differs",
                )
            }
            (
                A::Block { statements, tail },
                K::Block {
                    statements: hir_statements,
                    tail: hir_tail,
                },
            ) => {
                check(statements.len() == hir_statements.len(), "block differs")?;
                let saved = self.len;
                for (index, (statement, hir_statement)) in
                    statements.iter().zip(hir_statements).enumerate()
                {
                    self.nested(
                        &format!("{path}.s{index}"),
                        statement,
                        hir_statement,
                        depth + 1,
                    )?;
                }
                self.expression(tail, hir_tail, &format!("{path}.tail"), depth + 1)?;
                self.len = saved;
                check(scalar(hir) && hir_tail.ty == hir.ty, "block result differs")
            }
            (
                A::Match {
                    mode: ast::MatchMode::Value,
                    scrutinee,
                    arms,
                },
                K::Match {
                    mode: crate::hir::ResolvedMatchMode::Value,
                    scrutinee: hir_scrutinee,
                    arms: hir_arms,
                },
            ) => self.option_match(scrutinee, arms, hir, hir_scrutinee, hir_arms, path, depth),
            (A::Call { .. }, _) => self.call(source, hir, path, depth),
            _ => Err(refuse("suffix operation differs")),
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn field_read(
        &self,
        base: &ast::Expr,
        field: &str,
        hir: &E,
        element: &T,
        field_id: &crate::hir::DeclarationId,
        args: &[E],
        path: &str,
    ) -> Result<(), Refusal> {
        let A::Var(row) = &base.kind else {
            return Err(refuse("field read base is not a row alias"));
        };
        let slot = self.lookup(row)?;
        let (Kind::Row, Some(index)) = (slot.kind, slot.index) else {
            return Err(refuse("field read base is not a row alias"));
        };
        let declared = self
            .selection
            .fields()
            .iter()
            .find(|candidate| candidate.name == field)
            .ok_or(refuse("field read names no row field"))?;
        let (result, mode) = match declared.ty {
            T::I64 => (T::I64, O::Value),
            T::String => (T::Str, O::Borrow),
            T::Bytes => (T::SliceU8, O::Borrow),
            _ => return Err(refuse("field read type differs")),
        };
        let [vector, row_index] = args else {
            return Err(refuse("field read arity differs"));
        };
        check(
            element == self.selection.element()
                && *field_id == declared.id
                && typed(hir, &result, mode)
                && self.expression_id(vector, &format!("{path}.arg.0"))
                && self.expression_id(row_index, &format!("{path}.arg.1"))
                && vector.ty == slot.binding.ty
                && matches!(&vector.kind, K::Place(place)
                    if place.root == slot.binding.id && place.projections.is_empty())
                && typed(row_index, &T::Usize, O::Value)
                && matches!(&row_index.kind, K::Place(place)
                    if place.root == index.id && place.projections.is_empty()),
            "field read differs",
        )
    }

    fn call(
        &mut self,
        source: &'s ast::Expr,
        hir: &'a E,
        path: &str,
        depth: usize,
    ) -> Result<(), Refusal> {
        let Some((name, types, args)) = call(source) else {
            return Err(refuse("call differs"));
        };
        if name == crate::byte_ops::STR_AS_BYTES_NAME {
            // A view of a bound String view; a direct row projection may lower
            // to a fused field read and is outside this narrowed profile.
            let [ast::Expr {
                kind: A::Var(text), ..
            }] = args
            else {
                return Err(refuse("view source is not a bound view"));
            };
            let slot = self.lookup(text)?;
            return check(
                types.is_empty()
                    && slot.kind == Kind::Value
                    && slot.binding.ty == T::Str
                    && slot.binding.ownership == O::Borrow
                    && typed(hir, &T::SliceU8, O::Borrow)
                    && matches!(&hir.kind, K::BorrowPlace { operation, place }
                        if operation.as_str() == crate::byte_ops::STR_AS_BYTES_ID
                            && place.root == slot.binding.id
                            && place.projections.is_empty()),
                "view operation differs",
            );
        }
        let K::Call {
            callee,
            instance: None,
            type_arguments,
            args: hir_args,
        } = &hir.kind
        else {
            return Err(refuse("call differs"));
        };
        if name == "vec_len" {
            let ([owner], [hir_owner]) = (args, hir_args.as_slice()) else {
                return Err(refuse("vec_len arity differs"));
            };
            self.owner_operand(owner, hir_owner, &format!("{path}.arg.0"), false)?;
            return check(
                callee.as_str() == crate::vec_ops::LEN_ID
                    && super::join::row_type(types, self.selection.record())
                    && type_arguments.as_slice() == std::slice::from_ref(self.selection.element())
                    && typed(hir, &T::Usize, O::Value),
                "vec_len differs",
            );
        }
        let (target, result) = match name {
            crate::string_ops::I64_FROM_USIZE_NAME => {
                (crate::string_ops::I64_FROM_USIZE_ID, T::I64)
            }
            crate::str_ops::LEN_BYTES_NAME => (crate::str_ops::LEN_BYTES_ID, T::I64),
            crate::byte_ops::LEN_NAME => (crate::byte_ops::LEN_ID, T::Usize),
            crate::byte_ops::GET_NAME => (
                crate::byte_ops::GET_ID,
                T::Nominal {
                    declaration: crate::hir::DeclarationId::compiler(
                        crate::hir::CompilerDeclaration::Option,
                    ),
                    arguments: vec![T::U8],
                },
            ),
            _ => return Err(refuse("call outside the closed builtin domain")),
        };
        check(
            callee.as_str() == target
                && types.is_empty()
                && type_arguments.is_empty()
                && args.len() == hir_args.len()
                && typed(hir, &result, O::Value),
            "builtin call differs",
        )?;
        for (index, (arg, hir_arg)) in args.iter().zip(hir_args).enumerate() {
            self.expression(arg, hir_arg, &format!("{path}.arg.{index}"), depth + 1)?;
        }
        Ok(())
    }

    #[allow(clippy::too_many_arguments)]
    fn option_match(
        &mut self,
        scrutinee: &'s ast::Expr,
        arms: &'s [ast::MatchArm],
        hir: &'a E,
        hir_scrutinee: &'a E,
        hir_arms: &'a [crate::hir::ResolvedMatchArm],
        path: &str,
        depth: usize,
    ) -> Result<(), Refusal> {
        self.expression(
            scrutinee,
            hir_scrutinee,
            &format!("{path}.scrutinee"),
            depth + 1,
        )?;
        check(
            option_u8(&hir_scrutinee.ty)
                && hir_scrutinee.ownership == O::Value
                && scalar(hir)
                && arms.len() == 2
                && hir_arms.len() == 2,
            "Option match differs",
        )?;
        let (mut some, mut none) = (false, false);
        for (index, (arm, hir_arm)) in arms.iter().zip(hir_arms).enumerate() {
            self.debit()?;
            let ast::MatchPattern::Variant {
                type_name,
                case_name,
                fields,
                ..
            } = &arm.pattern
            else {
                return Err(refuse("Option arm differs"));
            };
            let P::Variant {
                variant,
                case,
                fields: hir_fields,
            } = &hir_arm.pattern
            else {
                return Err(refuse("Option arm differs"));
            };
            check(
                arm.guard.is_none()
                    && hir_arm.guard.is_none()
                    && type_name == "Option"
                    && variant.as_str() == crate::prelude::OPTION_ID,
                "Option arm differs",
            )?;
            let saved = self.len;
            match (case_name.as_str(), fields.as_slice(), hir_fields.as_slice()) {
                ("Some", [field], [hir_field]) if !some && field.name == "value" => {
                    let binding = &hir_field.binding;
                    check(
                        case.as_str() == crate::prelude::OPTION_SOME_ID
                            && hir_field.field.as_str() == crate::prelude::OPTION_SOME_VALUE_ID
                            && binding.name == field.binding
                            && binding.span == field.binding_span
                            && binding.ty == T::U8
                            && binding.ownership == O::Value
                            && self.local_id(binding, &format!("{path}.arm.{index}.binding.0")),
                        "Option Some arm differs",
                    )?;
                    some = true;
                    self.bind(Slot {
                        name: &field.binding,
                        binding,
                        index: None,
                        kind: Kind::Value,
                        mutable: false,
                        live: true,
                    })?;
                }
                ("None", [], []) if !none => {
                    check(
                        case.as_str() == crate::prelude::OPTION_NONE_ID,
                        "Option None arm differs",
                    )?;
                    none = true;
                }
                _ => return Err(refuse("Option arm differs")),
            }
            self.expression(
                &arm.value,
                &hir_arm.value,
                &format!("{path}.arm.{index}.value"),
                depth + 1,
            )?;
            self.len = saved;
            check(hir_arm.value.ty == hir.ty, "Option arm result differs")?;
        }
        check(some && none, "Option match is not exhaustive")
    }
}

/// Prove the complete suffix and tail correspondence for one joined pair.
pub(super) fn suffix<'s, 'a>(
    program: &'a ResolvedProgram,
    selection: &'s Selection<'a>,
    proof: &SourceProof<'s>,
    source_suffix: &'s [ast::Statement],
    source_tail: &'s ast::Expr,
) -> Result<(), Refusal> {
    let hir_suffix = selection.suffix();
    check(
        source_suffix.len() == hir_suffix.len(),
        "suffix statement count differs",
    )?;
    if !crate::bounded_output::reserve_active_required(std::mem::size_of::<Walk<'_, '_>>()) {
        return Err(Refusal::Scratch);
    }
    let mut walk = Walk {
        program,
        function: FunctionExecutionId::Monomorphic(selection.function().id.clone()),
        selection,
        slots: [None; SLOTS],
        len: 0,
        work: crate::cache_codec::MAX_NODES,
        transfers: [None; MAX_OWNER_EVENTS],
        recorded: 0,
    };
    walk.bind(Slot {
        name: proof.destination().name,
        binding: selection.vector(),
        index: None,
        kind: Kind::Owner,
        mutable: true,
        live: true,
    })?;
    walk.bind(Slot {
        name: proof.induction().name,
        binding: selection.counter(),
        index: None,
        kind: Kind::Value,
        mutable: true,
        live: true,
    })?;
    // Suffix statements keep their source positions after the loop at s2.
    for (offset, (statement, hir_statement)) in source_suffix.iter().zip(hir_suffix).enumerate() {
        walk.top(&format!("body.s{}", offset + 3), statement, hir_statement)?;
    }
    walk.expression(source_tail, selection.tail(), "body.tail", 1)?;
    check(
        typed(selection.tail(), &T::I64, O::Value),
        "function result differs",
    )?;
    // The source census must have recorded exactly the transfers proved here.
    let mut events = proof.owner_events();
    for recorded in walk.transfers.iter().map_while(|transfer| *transfer) {
        let matched = match (recorded, events.next()) {
            ((Transfer::Move, left), Some(OwnerEvent::Move(right)))
            | ((Transfer::Sort, left), Some(OwnerEvent::Sort(right)))
            | ((Transfer::Borrow, left), Some(OwnerEvent::Borrow(right))) => {
                std::ptr::eq(left, right)
            }
            _ => false,
        };
        check(matched, "source owner events differ from proved transfers")?;
    }
    check(
        events.next().is_none(),
        "source owner events differ from proved transfers",
    )
}
