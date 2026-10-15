//! Test-only work-metered primitives. Every caller-visible step is announced on
//! the caller's `work` callback before it happens; bytes still go through the
//! existing `Reserve`. Neither channel is converted into the other, and neither
//! reads the production ledger. A refusal is returned unchanged. Receiving release is prepaid before acquisition, including refusal paths.
use super::{charge, clone_bytes, reserve_node, FundedError, Reserve};
use crate::cleanup_plan::ReplayClonePayload;
use crate::hir::{DeclarationId, ExpressionId, ResolvedExpr, ResolvedExprKind};
use std::collections::{btree_map, btree_set, BTreeMap, BTreeSet};

pub(crate) type Work<'w, E> = &'w mut dyn FnMut(usize) -> Result<(), E>;

pub(crate) fn tick<E>(work: Work<'_, E>, units: usize) -> Result<(), FundedError<E>> {
    work(units).map_err(FundedError::Refused)
}

/// Upper bound of one string comparison: bytes inspected before a verdict.
pub(crate) fn str_units(a: &str, b: &str) -> usize {
    a.len().min(b.len()) + 1
}

pub(crate) trait CompareUnits {
    fn compare_units(&self, other: &Self) -> usize;
}
// Assumes `Ord` on both ids is lexicographic over `as_str()`; see section 4.
impl CompareUnits for ExpressionId {
    fn compare_units(&self, other: &Self) -> usize {
        str_units(self.as_str(), other.as_str())
    }
}
impl CompareUnits for DeclarationId {
    fn compare_units(&self, other: &Self) -> usize {
        str_units(self.as_str(), other.as_str())
    }
}

/// Work to drop one value of the type, beyond the visit of its slot.
pub(crate) trait DropUnits {
    const DROP_UNITS: usize;
}
impl DropUnits for ExpressionId {
    const DROP_UNITS: usize = 1;
}
impl DropUnits for () {
    const DROP_UNITS: usize = 0;
}
impl DropUnits for u8 {
    const DROP_UNITS: usize = 0;
}

pub(crate) trait Disposable {
    #[allow(
        dead_code,
        reason = "retained original receiver or refusal payload; not every legacy entry is selected by current tests"
    )]
    fn drop_units(&self) -> usize;
}
impl<T: Copy> Disposable for Vec<T> {
    fn drop_units(&self) -> usize {
        usize::from(self.capacity() > 0)
    }
}
impl<T: Disposable> Disposable for Option<T> {
    fn drop_units(&self) -> usize {
        self.as_ref().map_or(0, Disposable::drop_units)
    }
}
impl<A: Disposable, B: Disposable> Disposable for (A, B) {
    fn drop_units(&self) -> usize {
        self.0.drop_units().saturating_add(self.1.drop_units())
    }
}
#[allow(
    dead_code,
    reason = "retained original receiver or refusal payload; not every legacy entry is selected by current tests"
)]
fn tree_units(len: usize, per: usize) -> usize {
    len.saturating_mul(per).saturating_add(1)
}
impl<K: DropUnits> Disposable for BTreeSet<K> {
    fn drop_units(&self) -> usize {
        tree_units(self.len(), 1 + K::DROP_UNITS)
    }
}
impl<K: DropUnits, V: DropUnits> Disposable for BTreeMap<K, V> {
    fn drop_units(&self) -> usize {
        tree_units(self.len(), 1 + K::DROP_UNITS + V::DROP_UNITS)
    }
}
impl<K: DropUnits> Disposable for btree_set::IntoIter<K> {
    fn drop_units(&self) -> usize {
        tree_units(self.len(), 1 + K::DROP_UNITS)
    }
}
impl<K: DropUnits, V: DropUnits> Disposable for btree_map::IntoIter<K, V> {
    fn drop_units(&self) -> usize {
        tree_units(self.len(), 1 + K::DROP_UNITS + V::DROP_UNITS)
    }
}
impl<K: DropUnits, V: DropUnits> Disposable for btree_map::IntoKeys<K, V> {
    fn drop_units(&self) -> usize {
        tree_units(self.len(), 1 + K::DROP_UNITS + V::DROP_UNITS)
    }
}

// Release is prepaid before acquisition. Refusal performs real Drop without
// another callback and preserves the selected error; no storage is leaked.
pub(crate) fn abandon_units<T, E>(
    held: T,
    _units: usize,
    error: FundedError<E>,
    _work: Work<'_, E>,
) -> FundedError<E> {
    drop(held);
    error
}
pub(crate) fn abandon<D: Disposable, E>(
    held: D,
    error: FundedError<E>,
    _work: Work<'_, E>,
) -> FundedError<E> {
    drop(held);
    error
}
pub(crate) fn retire<D: Disposable, R, E>(
    held: D,
    result: R,
    _work: Work<'_, E>,
) -> Result<R, FundedError<E>> {
    drop(held);
    Ok(result)
}

pub(crate) fn clone_units<K: ReplayClonePayload + DropUnits, E>(
    _key: &K,
) -> Result<usize, FundedError<E>> {
    Ok(1)
}

// ---- metered tree search -------------------------------------------------

/// Pays every comparison any descent could make, one iterator step and one
/// key comparison per key, before the real search runs.
fn pay_search<'a, K: CompareUnits + 'a, E>(
    mut keys: impl Iterator<Item = &'a K>,
    query: &K,
    work: Work<'_, E>,
) -> Result<(), FundedError<E>> {
    loop {
        tick(work, 1)?;
        let Some(key) = keys.next() else {
            return Ok(());
        };
        tick(work, key.compare_units(query))?;
    }
}

pub(crate) fn search_set<K: CompareUnits, E>(
    set: &BTreeSet<K>,
    query: &K,
    work: Work<'_, E>,
) -> Result<(), FundedError<E>> {
    tick(work, 1)?;
    pay_search(set.iter(), query, work)
}

pub(crate) fn search_map<K: CompareUnits, V, E>(
    map: &BTreeMap<K, V>,
    query: &K,
    work: Work<'_, E>,
) -> Result<(), FundedError<E>> {
    tick(work, 1)?;
    pay_search(map.keys(), query, work)
}

pub(crate) fn contains_with_work<K: Ord + CompareUnits, E>(
    set: &BTreeSet<K>,
    key: &K,
    work: Work<'_, E>,
) -> Result<bool, FundedError<E>> {
    search_set(set, key, work)?;
    Ok(set.contains(key))
}

pub(crate) fn contains_key_with_work<K: Ord + CompareUnits, V, E>(
    map: &BTreeMap<K, V>,
    key: &K,
    work: Work<'_, E>,
) -> Result<bool, FundedError<E>> {
    search_map(map, key, work)?;
    Ok(map.contains_key(key))
}

// ---- metered insertion ----------------------------------------------------

pub(crate) fn insert_cloned_key_with_work<K, E>(
    set: &mut BTreeSet<K>,
    key: &K,
    work: Work<'_, E>,
    reserve: Reserve<'_, E>,
) -> Result<(), FundedError<E>>
where
    K: Ord + Clone + ReplayClonePayload + CompareUnits + DropUnits,
{
    let present = contains_with_work(set, key, work)?;
    if !present {
        tick(work, 1 + K::DROP_UNITS)?;
        reserve_node::<K, (), E>(reserve)?;
    }
    charge(reserve, clone_bytes::<K, E>(key)?)?;
    search_set(set, key, work)?; // the insertion's own descent
    if present {
        tick(work, K::DROP_UNITS)?; // insert drops the rejected duplicate
    }
    tick(work, clone_units::<K, E>(key)?)?;
    set.insert(key.clone());
    Ok(())
}

pub(crate) fn insert_cloned_with_work<K, V, E>(
    map: &mut BTreeMap<K, V>,
    key: &K,
    value: V,
    work: Work<'_, E>,
    reserve: Reserve<'_, E>,
) -> Result<(), FundedError<E>>
where
    K: Ord + Clone + ReplayClonePayload + CompareUnits + DropUnits,
    V: DropUnits,
{
    let paid = (|| -> Result<(), FundedError<E>> {
        let present = contains_key_with_work(map, key, work)?;
        if !present {
            tick(work, 1 + K::DROP_UNITS + V::DROP_UNITS)?;
            reserve_node::<K, V, E>(reserve)?;
        }
        charge(reserve, clone_bytes::<K, E>(key)?)?;
        search_map(map, key, work)?;
        if present {
            tick(work, K::DROP_UNITS + V::DROP_UNITS)?; // new key + replaced value
        }
        tick(work, clone_units::<K, E>(key)?)
    })();
    match paid {
        Ok(()) => {
            map.insert(key.clone(), value);
            Ok(())
        }
        Err(error) => Err(abandon_units(value, V::DROP_UNITS, error, work)),
    }
}

fn pay_insert_set<K: Ord + CompareUnits + DropUnits, E>(
    set: &BTreeSet<K>,
    key: &K,
    work: Work<'_, E>,
    reserve: Reserve<'_, E>,
) -> Result<(), FundedError<E>> {
    let present = contains_with_work(set, key, work)?;
    if !present {
        tick(work, 1 + K::DROP_UNITS)?;
        reserve_node::<K, (), E>(reserve)?;
    }
    search_set(set, key, work)?;
    if present {
        tick(work, K::DROP_UNITS)?;
    }
    Ok(())
}

#[allow(
    dead_code,
    reason = "retained original receiver or refusal payload; not every legacy entry is selected by current tests"
)]
fn pay_insert_map<K: Ord + CompareUnits + DropUnits, V: DropUnits, E>(
    map: &BTreeMap<K, V>,
    key: &K,
    work: Work<'_, E>,
    reserve: Reserve<'_, E>,
) -> Result<(), FundedError<E>> {
    let present = contains_key_with_work(map, key, work)?;
    if !present {
        tick(work, 1 + K::DROP_UNITS + V::DROP_UNITS)?;
        reserve_node::<K, V, E>(reserve)?;
    }
    search_map(map, key, work)?;
    if present {
        tick(work, K::DROP_UNITS + V::DROP_UNITS)?;
    }
    Ok(())
}

#[allow(
    dead_code,
    reason = "retained original receiver or refusal payload; not every legacy entry is selected by current tests"
)]
pub(crate) fn insert_moved_with_work<K, V, E>(
    map: &mut BTreeMap<K, V>,
    key: K,
    value: V,
    work: Work<'_, E>,
    reserve: Reserve<'_, E>,
) -> Result<(), FundedError<E>>
where
    K: Ord + CompareUnits + DropUnits,
    V: DropUnits,
{
    match pay_insert_map(map, &key, work, reserve) {
        Ok(()) => {
            map.insert(key, value);
            Ok(())
        }
        Err(error) => {
            let error = abandon_units(key, K::DROP_UNITS, error, work);
            Err(abandon_units(value, V::DROP_UNITS, error, work))
        }
    }
}

pub(crate) fn extend_set_with_work<K, E>(
    target: &mut BTreeSet<K>,
    source: BTreeSet<K>,
    work: Work<'_, E>,
    reserve: Reserve<'_, E>,
) -> Result<(), FundedError<E>>
where
    K: Ord + CompareUnits + DropUnits,
{
    if let Err(error) = tick(work, 1) {
        return Err(abandon(source, error, work));
    }
    let mut items = source.into_iter();
    loop {
        if let Err(error) = tick(work, 1) {
            return Err(abandon(items, error, work));
        }
        let Some(key) = items.next() else { break };
        match pay_insert_set(target, &key, work, reserve) {
            Ok(()) => {
                target.insert(key);
            }
            Err(error) => {
                let error = abandon_units(key, K::DROP_UNITS, error, work);
                return Err(abandon(items, error, work));
            }
        }
    }
    retire(items, (), work)
}

#[allow(
    dead_code,
    reason = "retained original receiver or refusal payload; not every legacy entry is selected by current tests"
)]
pub(crate) fn extend_map_with_work<K, V, E>(
    target: &mut BTreeMap<K, V>,
    source: BTreeMap<K, V>,
    work: Work<'_, E>,
    reserve: Reserve<'_, E>,
) -> Result<(), FundedError<E>>
where
    K: Ord + CompareUnits + DropUnits,
    V: DropUnits,
{
    if let Err(error) = tick(work, 1) {
        return Err(abandon(source, error, work));
    }
    let mut items = source.into_iter();
    loop {
        if let Err(error) = tick(work, 1) {
            return Err(abandon(items, error, work));
        }
        let Some((key, value)) = items.next() else {
            break;
        };
        match pay_insert_map(target, &key, work, reserve) {
            Ok(()) => {
                target.insert(key, value);
            }
            Err(error) => {
                let error = abandon_units(key, K::DROP_UNITS, error, work);
                let error = abandon_units(value, V::DROP_UNITS, error, work);
                return Err(abandon(items, error, work));
            }
        }
    }
    retire(items, (), work)
}

// ---- metered child pushes -------------------------------------------------

/// `(children, slot probes)`. Block-like nodes probe every statement slot, a
/// match reads every arm; both reads are paid before they happen.
pub(crate) fn child_count_with_work<E>(
    expression: &ResolvedExpr,
    work: Work<'_, E>,
) -> Result<(usize, usize), FundedError<E>> {
    use ResolvedExprKind as K;
    match &expression.kind {
        K::Block { statements, .. } | K::VecConstructor { statements, .. } => {
            let mut count = 1usize;
            let mut probes = statements.len();
            for statement in statements {
                tick(work, 1)?;
                let slots = statement.child_count();
                tick(work, slots)?;
                probes = probes
                    .checked_add(slots)
                    .ok_or(FundedError::Layout("funded probe count overflowed"))?;
                for index in 0..slots {
                    if statement.child(index).is_some() {
                        count += 1;
                    }
                }
            }
            Ok((count, probes))
        }
        K::Match { arms, .. } => {
            tick(work, arms.len())?;
            Ok((super::child_count(expression), arms.len()))
        }
        _ => {
            tick(work, 1)?;
            Ok((super::child_count(expression), 0))
        }
    }
}

/// Our count is paid inside `child_count_with_work`. Before delegating we also
/// pay the delegate's own recount and the original pusher's slot reads
/// (`2 * probes`), plus the `count` pushes themselves.
fn pre_push_units<E>(count: usize, probes: usize) -> Result<usize, FundedError<E>> {
    probes
        .checked_mul(2)
        .and_then(|p| p.checked_add(count))
        .ok_or(FundedError::Layout("funded push units overflowed"))
}

pub(crate) fn push_children_with_work<'a, E>(
    expression: &'a ResolvedExpr,
    pending: &mut Vec<&'a ResolvedExpr>,
    work: Work<'_, E>,
    reserve: Reserve<'_, E>,
) -> Result<(), FundedError<E>> {
    let (count, probes) = child_count_with_work(expression, work)?;
    tick(work, pre_push_units(count, probes)?)?;
    if pending
        .len()
        .checked_add(count)
        .is_none_or(|needed| needed > pending.capacity())
    {
        tick(work, pending.len())?;
        if pending.capacity() != 0 {
            tick(work, 1)?;
        }
    }
    super::push_children(expression, pending, reserve)
}

pub(crate) fn push_trace_children_with_work<'a, E>(
    expression: &'a ResolvedExpr,
    pending: &mut Vec<&'a ResolvedExpr>,
    work: Work<'_, E>,
    reserve: Reserve<'_, E>,
) -> Result<(), FundedError<E>> {
    let (count, probes) = child_count_with_work(expression, work)?;
    tick(work, pre_push_units(count, probes)?)?;
    if pending
        .len()
        .checked_add(count)
        .is_none_or(|needed| needed > pending.capacity())
    {
        tick(work, pending.len())?;
        if pending.capacity() != 0 {
            tick(work, 1)?;
        }
    }
    super::push_trace_children(expression, pending, reserve)
}

// Complete structural equality: no missing variant silently falls back to
// an unpaid derived comparison. These admission shapes retain their original
// source depth rule and every child is paid before it is inspected.
pub(crate) fn type_eq_with_work<E>(
    a: &crate::hir::ResolvedType,
    b: &crate::hir::ResolvedType,
    work: Work<'_, E>,
) -> Result<bool, FundedError<E>> {
    use crate::hir::ResolvedType as T;
    tick(work, 1)?;
    if std::mem::discriminant(a) != std::mem::discriminant(b) {
        return Ok(false);
    }
    match (a, b) {
        (
            T::Nominal {
                declaration: x,
                arguments: xs,
            },
            T::Nominal {
                declaration: y,
                arguments: ys,
            },
        ) => {
            tick(work, str_units(x.as_str(), y.as_str()))?;
            if x != y {
                return Ok(false);
            }
            slice_eq_with_work(xs, ys, work)
        }
        (
            T::Function {
                parameters: xs,
                result: x,
            },
            T::Function {
                parameters: ys,
                result: y,
            },
        ) => {
            if !slice_eq_with_work(xs, ys, work)? {
                return Ok(false);
            }
            type_eq_with_work(x, y, work)
        }
        (
            T::TypeParameter {
                owner: x,
                index: xi,
            },
            T::TypeParameter {
                owner: y,
                index: yi,
            },
        ) => {
            tick(work, str_units(x.as_str(), y.as_str()))?;
            if x != y {
                return Ok(false);
            }
            tick(work, 1)?;
            Ok(xi == yi)
        }
        (T::ArrayU8(x), T::ArrayU8(y)) => {
            tick(work, 1)?;
            Ok(x == y)
        }
        (T::Never, T::Never)
        | (T::OnceFunction, T::OnceFunction)
        | (T::OnceFunctionI64, T::OnceFunctionI64)
        | (T::OnceFunctionI64Pair, T::OnceFunctionI64Pair)
        | (T::MutFunctionI64, T::MutFunctionI64)
        | (T::Unit, T::Unit)
        | (T::I64, T::I64)
        | (T::I32, T::I32)
        | (T::Char, T::Char)
        | (T::U8, T::U8)
        | (T::Usize, T::Usize)
        | (T::F32, T::F32)
        | (T::F64, T::F64)
        | (T::Bool, T::Bool)
        | (T::String, T::String)
        | (T::Bytes, T::Bytes)
        | (T::Str, T::Str)
        | (T::SliceU8, T::SliceU8)
        | (T::StringMap, T::StringMap) => Ok(true),
        _ => Ok(false),
    }
}
pub(crate) fn slice_eq_with_work<E>(
    a: &[crate::hir::ResolvedType],
    b: &[crate::hir::ResolvedType],
    work: Work<'_, E>,
) -> Result<bool, FundedError<E>> {
    tick(work, 1)?;
    if a.len() != b.len() {
        return Ok(false);
    }
    for (x, y) in a.iter().zip(b) {
        if !type_eq_with_work(x, y, work)? {
            return Ok(false);
        }
    }
    Ok(true)
}
