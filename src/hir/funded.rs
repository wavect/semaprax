//! Test-only funded traversal primitives for the explicit renewal receivers.
//! Every helper debits a caller-supplied exact payer before it allocates. A
//! refusal is returned unchanged; it is never turned into a smaller result.
use super::{ExpressionId, ResolvedExpr, ResolvedExprKind};
use crate::cleanup_plan::ReplayClonePayload;
use std::collections::{BTreeMap, BTreeSet};

pub(crate) type Reserve<'r, E> = &'r mut dyn FnMut(usize) -> Result<(), E>;

#[derive(Debug, Eq, PartialEq)]
pub(crate) enum FundedError<E> {
    /// The payer's own refusal, forwarded without translation.
    Refused(E),
    /// Physical failure only: a layout overflow, allocator failure or allocator
    /// disagreement, reported before any authored state is published. Callers
    /// propagate it; it is never read as "no sites".
    Layout(&'static str),
    /// The original checked census' own semantic verdict (admission work or
    /// depth exceeded). The ordinary receivers turn exactly this into an empty
    /// inventory; no payer or physical failure is ever reported through it.
    Census(&'static str),
    /// The original route performs an owning allocation that has no paid twin
    /// yet. The funded route refuses instead of allocating unpaid or reporting
    /// a smaller inventory; it is never a payer refusal or a census verdict.
    #[allow(
        dead_code,
        reason = "retained original receiver or refusal payload; not every legacy entry is selected by current tests"
    )]
    Unfunded(&'static str),
}

pub(crate) fn charge<E>(reserve: Reserve<'_, E>, bytes: usize) -> Result<(), FundedError<E>> {
    reserve(bytes).map_err(FundedError::Refused)
}

/// Replacement backing is reserved in full while the old buffer stays charged;
/// spare capacity never refunds a debit. Target is the replay growth rule.
pub(crate) fn grow<T, E>(
    values: &mut Vec<T>,
    additional: usize,
    reserve: Reserve<'_, E>,
) -> Result<(), FundedError<E>> {
    charge(reserve, 0)?;
    let required = values
        .len()
        .checked_add(additional)
        .ok_or(FundedError::Layout("funded vector length overflowed"))?;
    if required <= values.capacity() {
        return Ok(());
    }
    let doubled = values
        .capacity()
        .checked_mul(2)
        .ok_or(FundedError::Layout("funded vector capacity overflowed"))?;
    let target = required.max(doubled);
    let bytes = target
        .checked_mul(std::mem::size_of::<T>())
        .ok_or(FundedError::Layout("funded vector layout overflowed"))?;
    charge(reserve, bytes)?;
    values
        .try_reserve_exact(target - values.len())
        .map_err(|_| FundedError::Layout("funded vector allocation failed"))?;
    if values.capacity() != target {
        return Err(FundedError::Layout(
            "funded vector capacity disagrees with its reservation",
        ));
    }
    Ok(())
}

pub(crate) fn push<T, E>(
    values: &mut Vec<T>,
    value: T,
    reserve: Reserve<'_, E>,
) -> Result<(), FundedError<E>> {
    grow(values, 1, reserve)?;
    values.push(value);
    Ok(())
}

pub(crate) fn singleton<T, E>(value: T, reserve: Reserve<'_, E>) -> Result<Vec<T>, FundedError<E>> {
    let mut values = Vec::new();
    push(&mut values, value, reserve)?;
    Ok(values)
}

/// Exact number of entries `push_resolved_expression_children_in_authored_order`
/// appends; a parity control below pins it to the original on a corpus.
pub(crate) fn child_count(expression: &ResolvedExpr) -> usize {
    use ResolvedExprKind as K;
    match &expression.kind {
        K::Closure { captures, .. } => captures.len(),
        K::Block { statements, .. } | K::VecConstructor { statements, .. } => {
            statements
                .iter()
                .map(|statement| {
                    (0..statement.child_count())
                        .filter(|index| statement.child(*index).is_some())
                        .count()
                })
                .sum::<usize>()
                + 1
        }
        K::FunctionReference { .. } => 0,
        K::Invoke { args, .. } => args.len() + 1,
        K::Call { args, .. }
        | K::LiteralFormat { args, .. }
        | K::VecFieldRead { args, .. }
        | K::RowLoan { args, .. } => args.len(),
        K::NativeRustImportCall(call) => call.args.len(),
        K::HostCommandCall(call) => call.args.len(),
        K::ByteRange { .. } | K::If { .. } => 3,
        K::LexicalExit { value, .. } => usize::from(value.is_some()),
        K::Unary { .. }
        | K::Try { .. }
        | K::TryOption { .. }
        | K::Project { .. }
        | K::Upcast { .. }
        | K::Yield { .. } => 1,
        K::Binary { .. } => 2,
        K::ConstructRecord { fields, .. } | K::ConstructVariant { fields, .. } => fields.len(),
        K::Match { arms, .. } => {
            arms.iter()
                .map(|arm| 1 + usize::from(arm.guard.is_some()))
                .sum::<usize>()
                + 1
        }
        K::UpdateRecord { fields, .. } => fields.len() + 1,
        K::Int(_)
        | K::Int32(_)
        | K::Char(_)
        | K::Uint8(_)
        | K::Usize(_)
        | K::ArrayU8(_)
        | K::RepeatArrayU8 { .. }
        | K::Float32(_)
        | K::Float64(_)
        | K::Bool(_)
        | K::String(_)
        | K::BorrowedUtf8Literal(_)
        | K::Place(_)
        | K::BorrowPlace { .. } => 0,
    }
}

/// Reserve the exact child slots, then run the original authored-order pusher
/// inside that capacity so it cannot allocate.
pub(crate) fn push_children<'a, E>(
    expression: &'a ResolvedExpr,
    pending: &mut Vec<&'a ResolvedExpr>,
    reserve: Reserve<'_, E>,
) -> Result<(), FundedError<E>> {
    grow(pending, child_count(expression), reserve)?;
    super::push_resolved_expression_children_in_authored_order(expression, pending);
    Ok(())
}

/// Interpreter `trace_child_expressions` order (it differs from the authored
/// order for some kinds); the slots are reserved first and the original's
/// owned temporary `Vec` is never built. `child_count` is that exact length.
pub(crate) fn push_trace_children<'a, E>(
    expression: &'a ResolvedExpr,
    pending: &mut Vec<&'a ResolvedExpr>,
    reserve: Reserve<'_, E>,
) -> Result<(), FundedError<E>> {
    use ResolvedExprKind as K;
    grow(pending, child_count(expression), reserve)?;
    match &expression.kind {
        K::LexicalExit { value, .. } => pending.extend(value.iter().map(|value| value.as_ref())),
        K::Closure { captures, .. } => {
            pending.extend(captures.iter().map(|capture| &capture.value))
        }
        K::Call { args, .. }
        | K::LiteralFormat { args, .. }
        | K::VecFieldRead { args, .. }
        | K::RowLoan { args, .. } => pending.extend(args.iter()),
        K::Invoke { callable, args } => {
            pending.push(callable.as_ref());
            pending.extend(args.iter());
        }
        K::NativeRustImportCall(call) => pending.extend(call.args.iter()),
        K::HostCommandCall(call) => pending.extend(call.args.iter()),
        K::Unary { value, .. }
        | K::Try { operand: value, .. }
        | K::TryOption { operand: value, .. }
        | K::Project { base: value, .. }
        | K::Upcast { source: value }
        | K::Yield { request: value } => pending.push(value.as_ref()),
        K::Binary { left, right, .. } => {
            pending.push(left.as_ref());
            pending.push(right.as_ref());
        }
        K::ByteRange {
            source, start, end, ..
        } => {
            pending.push(source.as_ref());
            pending.push(start.as_ref());
            pending.push(end.as_ref());
        }
        K::Block { statements, tail }
        | K::VecConstructor {
            statements, tail, ..
        } => {
            for statement in statements {
                for index in 0..statement.child_count() {
                    if let Some(child) = statement.child(index) {
                        pending.push(child);
                    }
                }
            }
            pending.push(tail.as_ref());
        }
        K::If {
            condition,
            then_branch,
            else_branch,
        } => {
            pending.push(condition.as_ref());
            pending.push(then_branch.as_ref());
            pending.push(else_branch.as_ref());
        }
        K::ConstructRecord { fields, .. } | K::ConstructVariant { fields, .. } => {
            pending.extend(fields.iter().map(|field| &field.value))
        }
        K::Match {
            scrutinee, arms, ..
        } => {
            pending.push(scrutinee.as_ref());
            for arm in arms {
                if let Some(guard) = &arm.guard {
                    pending.push(guard.as_ref());
                }
                pending.push(&arm.value);
            }
        }
        K::UpdateRecord { base, fields, .. } => {
            pending.push(base.as_ref());
            pending.extend(fields.iter().map(|field| &field.value));
        }
        K::FunctionReference { .. }
        | K::Int(_)
        | K::Int32(_)
        | K::Char(_)
        | K::Uint8(_)
        | K::Usize(_)
        | K::ArrayU8(_)
        | K::RepeatArrayU8 { .. }
        | K::Float32(_)
        | K::Float64(_)
        | K::Bool(_)
        | K::String(_)
        | K::BorrowedUtf8Literal(_)
        | K::Place(_)
        | K::BorrowPlace { .. } => {}
    }
    Ok(())
}

/// `BTreeSet::extend` from an owned set: keys move, only new keys need a node.
pub(crate) fn extend_set<K: Ord, E>(
    target: &mut BTreeSet<K>,
    source: BTreeSet<K>,
    reserve: Reserve<'_, E>,
) -> Result<(), FundedError<E>> {
    for key in source {
        if !target.contains(&key) {
            reserve_node::<K, (), E>(reserve)?;
        }
        target.insert(key);
    }
    Ok(())
}

/// The original `children` temporary and the tagged `extend`, both reserved.
pub(crate) fn extend_children<'a, T, E>(
    expression: &'a ResolvedExpr,
    pending: &mut Vec<T>,
    tag: impl FnMut(&'a ResolvedExpr) -> T,
    reserve: Reserve<'_, E>,
) -> Result<(), FundedError<E>> {
    let mut children = Vec::new();
    grow(&mut children, child_count(expression), reserve)?;
    super::push_resolved_expression_children_in_authored_order(expression, &mut children);
    grow(pending, children.len(), reserve)?;
    pending.extend(children.into_iter().map(tag));
    Ok(())
}

pub(crate) fn reserve_node<K, V, E>(reserve: Reserve<'_, E>) -> Result<(), FundedError<E>> {
    let bytes = crate::cleanup_plan::replay_btree_node_upper::<K, V>()
        .ok_or(FundedError::Layout("funded tree layout overflowed"))?;
    charge(reserve, bytes)
}

fn clone_bytes<K: ReplayClonePayload, E>(key: &K) -> Result<usize, FundedError<E>> {
    key.clone_payload_bytes()
        .ok_or(FundedError::Layout("funded clone payload overflowed"))
}

/// Node (when the key is new) and clone payload are charged before the clone
/// and the insertion, matching the replay identity-tree order.
pub(crate) fn insert_cloned<K: Ord + Clone + ReplayClonePayload, V, E>(
    map: &mut BTreeMap<K, V>,
    key: &K,
    value: V,
    reserve: Reserve<'_, E>,
) -> Result<(), FundedError<E>> {
    if !map.contains_key(key) {
        reserve_node::<K, V, E>(reserve)?;
    }
    charge(reserve, clone_bytes(key)?)?;
    map.insert(key.clone(), value);
    Ok(())
}

pub(crate) fn insert_cloned_key<K: Ord + Clone + ReplayClonePayload, E>(
    set: &mut BTreeSet<K>,
    key: &K,
    reserve: Reserve<'_, E>,
) -> Result<(), FundedError<E>> {
    if !set.contains(key) {
        reserve_node::<K, (), E>(reserve)?;
    }
    charge(reserve, clone_bytes(key)?)?;
    set.insert(key.clone());
    Ok(())
}

/// `BTreeMap::extend` with an owned source: later entries replace earlier
/// values, keys move, and only genuinely new keys need a node.
pub(crate) fn extend_map<V, E>(
    target: &mut BTreeMap<ExpressionId, V>,
    source: BTreeMap<ExpressionId, V>,
    reserve: Reserve<'_, E>,
) -> Result<(), FundedError<E>> {
    for (key, value) in source {
        insert_moved(target, key, value, reserve)?;
    }
    Ok(())
}

/// Insertion of an already-owned key: only a genuinely new key needs a node.
pub(crate) fn insert_moved<K: Ord, V, E>(
    map: &mut BTreeMap<K, V>,
    key: K,
    value: V,
    reserve: Reserve<'_, E>,
) -> Result<(), FundedError<E>> {
    if !map.contains_key(&key) {
        reserve_node::<K, V, E>(reserve)?;
    }
    map.insert(key, value);
    Ok(())
}

/// Charge the backing a `key.clone()` will allocate, before cloning it.
pub(crate) fn charge_clone<K: ReplayClonePayload, E>(
    key: &K,
    reserve: Reserve<'_, E>,
) -> Result<(), FundedError<E>> {
    charge(reserve, clone_bytes(key)?)
}

/// Deterministic test payer shared by the receiver controls.
#[cfg(test)]
pub(crate) mod payer {
    use super::*;

    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    pub(crate) enum PayerRefusal {
        /// The scripted ordinal was refused.
        Ordinal(usize),
        Exhausted {
            requested: usize,
        },
        Sticky,
    }

    pub(crate) struct Payer {
        pub(crate) calls: usize,
        pub(crate) charged: usize,
        allowance: usize,
        refuse_at: Option<usize>,
        refused: bool,
    }

    impl Payer {
        pub(crate) fn unlimited() -> Self {
            Self::with(usize::MAX, None)
        }
        pub(crate) fn allowance(bytes: usize) -> Self {
            Self::with(bytes, None)
        }
        pub(crate) fn refusing_at(ordinal: usize) -> Self {
            Self::with(usize::MAX, Some(ordinal))
        }
        fn with(allowance: usize, refuse_at: Option<usize>) -> Self {
            Self {
                calls: 0,
                charged: 0,
                allowance,
                refuse_at,
                refused: false,
            }
        }
        pub(crate) fn pay(&mut self, bytes: usize) -> Result<(), PayerRefusal> {
            let ordinal = self.calls;
            self.calls += 1;
            if self.refused {
                return Err(PayerRefusal::Sticky);
            }
            if self.refuse_at == Some(ordinal) {
                self.refused = true;
                return Err(PayerRefusal::Ordinal(ordinal));
            }
            match self.charged.checked_add(bytes) {
                Some(total) if total <= self.allowance => {
                    self.charged = total;
                    Ok(())
                }
                _ => {
                    self.refused = true;
                    Err(PayerRefusal::Exhausted { requested: bytes })
                }
            }
        }
    }

    /// Shared receiver control. `run` normalizes its result into an owned
    /// value. Checks: the unlimited result, the exact allowance, one byte
    /// short, and a scripted refusal at every charge ordinal which must come
    /// back unchanged with no later charge attempted (so it was neither skipped
    /// nor cleared). Returns `(old_value, calls, charged)` of the unlimited run.
    pub(crate) fn assert_every_boundary<R: std::fmt::Debug + PartialEq>(
        run: impl Fn(Reserve<'_, PayerRefusal>) -> Result<R, FundedError<PayerRefusal>>,
    ) -> (R, usize, usize) {
        let mut unlimited = Payer::unlimited();
        let value = run(&mut |bytes| unlimited.pay(bytes)).expect("unlimited payer");
        let (calls, charged) = (unlimited.calls, unlimited.charged);
        assert!(calls > 0, "a funded receiver must authenticate its payer");
        let mut exact = Payer::allowance(charged);
        assert_eq!(
            run(&mut |bytes| exact.pay(bytes)).expect("exact allowance"),
            value
        );
        assert_eq!(exact.charged, charged);
        if charged > 0 {
            let mut short = Payer::allowance(charged - 1);
            let refusal = run(&mut |bytes| short.pay(bytes)).expect_err("one byte short");
            assert!(matches!(
                refusal,
                FundedError::Refused(PayerRefusal::Exhausted { .. })
            ));
            assert!(short.charged < charged);
        }
        for ordinal in 0..calls {
            let mut scripted = Payer::refusing_at(ordinal);
            let refusal = run(&mut |bytes| scripted.pay(bytes)).expect_err("scripted refusal");
            assert_eq!(
                refusal,
                FundedError::Refused(PayerRefusal::Ordinal(ordinal)),
                "refusal at charge {ordinal} must propagate unchanged"
            );
            assert_eq!(
                scripted.calls,
                ordinal + 1,
                "no charge may follow the first refusal at {ordinal}"
            );
        }
        (value, calls, charged)
    }
}

#[cfg(test)]
mod tests {
    use super::payer::*;
    use super::*;

    const SOURCE: &str = include_str!("../../tests/owned_data/byte_helper_renewal.spx");

    #[test]
    fn child_count_and_funded_push_match_the_original_pusher_on_every_node() {
        let checked = crate::check(SOURCE, "funded-children.spx").unwrap();
        let program = crate::hir::resolve(&checked).unwrap();
        let mut visited = 0usize;
        for function in &program.functions {
            let mut pending = vec![&function.body];
            while let Some(expression) = pending.pop() {
                let mut original = Vec::new();
                super::super::push_resolved_expression_children_in_authored_order(
                    expression,
                    &mut original,
                );
                assert_eq!(child_count(expression), original.len());
                let mut payer = Payer::unlimited();
                let mut funded = Vec::new();
                push_children(expression, &mut funded, &mut |bytes| payer.pay(bytes)).unwrap();
                assert_eq!(funded.len(), original.len());
                assert!(funded
                    .iter()
                    .zip(&original)
                    .all(|(a, b)| std::ptr::eq(*a, *b)));
                let slots = std::mem::size_of::<&ResolvedExpr>();
                assert_eq!(
                    payer.charged,
                    funded.capacity() * slots * usize::from(!funded.is_empty())
                );
                pending.extend(original);
                visited += 1;
            }
        }
        assert!(visited > 20);
    }

    #[test]
    fn funded_trace_children_match_the_interpreter_order_on_every_node() {
        let checked = crate::check(SOURCE, "funded-trace-children.spx").unwrap();
        let program = crate::hir::resolve(&checked).unwrap();
        let mut visited = 0usize;
        for function in &program.functions {
            let mut pending = vec![&function.body];
            while let Some(expression) = pending.pop() {
                let original = crate::interpreter::trace_child_expressions(expression);
                assert_eq!(child_count(expression), original.len());
                let mut unlimited = Payer::unlimited();
                let mut funded = Vec::new();
                push_trace_children(expression, &mut funded, &mut |b| unlimited.pay(b)).unwrap();
                assert!(
                    funded.len() == original.len()
                        && funded
                            .iter()
                            .zip(&original)
                            .all(|(a, b)| std::ptr::eq(*a, *b))
                );
                assert!(unlimited.calls > 0);
                // A refusal at the reservation leaves the output untouched.
                let mut refusing = Payer::refusing_at(1);
                let mut untouched = Vec::new();
                let refused =
                    push_trace_children(expression, &mut untouched, &mut |b| refusing.pay(b));
                if original.is_empty() {
                    assert!(refused.is_ok());
                } else {
                    assert_eq!(refused, Err(FundedError::Refused(PayerRefusal::Ordinal(1))));
                    assert!(untouched.is_empty());
                }
                pending.extend(original);
                visited += 1;
            }
        }
        assert!(visited > 20);
    }

    #[test]
    fn vector_growth_charges_the_replacement_before_allocating_and_is_sticky() {
        let (value, calls, charged) = assert_every_boundary(|reserve| {
            let mut values: Vec<u64> = Vec::new();
            for next in 0..5u64 {
                push(&mut values, next, reserve)?;
            }
            Ok(values)
        });
        assert_eq!(value, [0, 1, 2, 3, 4]);
        // Capacities 1, 2, 4, 8 are exact replacement layouts of 8-byte slots.
        assert_eq!(charged, (1 + 2 + 4 + 8) * 8);
        assert_eq!(calls, 5 + 4);
    }

    #[test]
    fn vector_layout_overflow_is_reported_inline_without_charging_the_payer() {
        let mut payer = Payer::unlimited();
        let mut values: Vec<u64> = Vec::new();
        let error = grow(&mut values, usize::MAX, &mut |bytes| payer.pay(bytes)).unwrap_err();
        assert!(matches!(error, FundedError::Layout(_)));
        assert_eq!(payer.charged, 0);
        assert!(values.capacity() == 0);
    }

    #[test]
    fn zero_byte_authentication_surfaces_a_sticky_payer() {
        let mut payer = Payer::refusing_at(0);
        assert_eq!(
            charge(&mut |bytes| payer.pay(bytes), 0),
            Err(FundedError::Refused(PayerRefusal::Ordinal(0)))
        );
        assert_eq!(
            charge(&mut |bytes| payer.pay(bytes), 0),
            Err(FundedError::Refused(PayerRefusal::Sticky))
        );
    }

    #[test]
    fn map_insertion_charges_a_node_only_for_new_keys_and_never_clones_unfunded() {
        let checked = crate::check(SOURCE, "funded-map.spx").unwrap();
        let program = crate::hir::resolve(&checked).unwrap();
        let id = program.functions[0].body.id.clone();
        let node = crate::cleanup_plan::replay_btree_node_upper::<ExpressionId, u8>().unwrap();
        let mut map = BTreeMap::new();
        let mut payer = Payer::unlimited();
        insert_cloned(&mut map, &id, 1u8, &mut |bytes| payer.pay(bytes)).unwrap();
        assert_eq!(payer.charged, node);
        insert_cloned(&mut map, &id, 2u8, &mut |bytes| payer.pay(bytes)).unwrap();
        assert_eq!(payer.charged, node, "a replacement allocates no new node");
        assert_eq!(map.get(&id), Some(&2));
        let mut short = Payer::allowance(node - 1);
        let mut fresh = BTreeMap::new();
        assert!(insert_cloned(&mut fresh, &id, 1u8, &mut |bytes| short.pay(bytes)).is_err());
        assert!(fresh.is_empty(), "a refused charge publishes nothing");
    }
}

#[cfg(test)]
pub(crate) mod work;
