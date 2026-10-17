//! Explicit-payer funded twins of `is_same_owner_concat_hir`,
//! `same_owner_concat_appends` and `same_owner_concat_operands`. Admission and
//! the first-wins/last-wins map semantics are the original's; every owned
//! temporary is debited on the caller's payer before it exists:
//! - the `format!("{:?}")` strings of the "later operand mentions the owner"
//!   test are never built. The owner's Debug text goes into a paid byte vector,
//!   its failure table into a paid vector, and each operand's Debug text is
//!   streamed through a matcher that holds no storage, so the substring verdict
//!   is identical and short-circuits at the first match;
//! - the traversal stack, interpreter child order, map nodes and clone
//!   payloads of keys and values are debited first.
//!
//! A typed-map reopen is admitted by `typed_reopen_admitted`, an allocation-free
//! borrowed projection of `MapOp::resolved_signature` (the original also builds
//! unused AST types, parameter names and `ValueId`s). It compares the argument
//! types and the result type structurally against the fixed shape of each
//! reopen, with no hashing or fingerprint; the original owning signature stays
//! the independent oracle in the parity controls below.
use super::*;
use crate::hir::funded::{self, FundedError, Reserve};
use crate::hir::{
    ExpressionId, ResolvedExpr, ResolvedExprKind, ResolvedFunction, ResolvedStatement,
};
use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;

/// Debit-before-grow byte sink for formatted text.
struct Paid<'p, E> {
    bytes: Vec<u8>,
    reserve: Reserve<'p, E>,
    failure: Option<FundedError<E>>,
}

impl<E> std::fmt::Write for Paid<'_, E> {
    fn write_str(&mut self, text: &str) -> std::fmt::Result {
        match funded::grow(&mut self.bytes, text.len(), &mut *self.reserve) {
            Ok(()) => {
                self.bytes.extend_from_slice(text.as_bytes());
                Ok(())
            }
            Err(failure) => {
                self.failure = Some(failure);
                Err(std::fmt::Error)
            }
        }
    }
}

/// Storage-free streaming substring matcher over a failure table.
struct Find<'n> {
    needle: &'n [u8],
    table: &'n [usize],
    state: usize,
    found: bool,
}

impl std::fmt::Write for Find<'_> {
    fn write_str(&mut self, text: &str) -> std::fmt::Result {
        for &byte in text.as_bytes() {
            while self.state > 0 && self.needle[self.state] != byte {
                self.state = self.table[self.state - 1];
            }
            if self.needle[self.state] == byte {
                self.state += 1;
            }
            if self.state == self.needle.len() {
                self.found = true;
                return Err(std::fmt::Error);
            }
        }
        Ok(())
    }
}

/// Paid needle and failure table for `format!("{owner:?}")` containment.
struct Needle {
    bytes: Vec<u8>,
    table: Vec<usize>,
}

fn needle<E>(owner: &ValueId, reserve: Reserve<'_, E>) -> Result<Needle, FundedError<E>> {
    let mut sink = Paid {
        bytes: Vec::new(),
        reserve: &mut *reserve,
        failure: None,
    };
    if write!(sink, "{owner:?}").is_err() {
        return Err(sink
            .failure
            .unwrap_or(FundedError::Layout("owner debug text failed to format")));
    }
    let bytes = sink.bytes;
    let mut table = Vec::new();
    funded::grow(&mut table, bytes.len(), reserve)?;
    let mut matched = 0usize;
    for at in 0..bytes.len() {
        if at == 0 {
            table.push(0);
            continue;
        }
        while matched > 0 && bytes[at] != bytes[matched] {
            matched = table[matched - 1];
        }
        if bytes[at] == bytes[matched] {
            matched += 1;
        }
        table.push(matched);
    }
    Ok(Needle { bytes, table })
}

fn mentions(argument: &ResolvedExpr, needle: &Needle) -> bool {
    let mut find = Find {
        needle: &needle.bytes,
        table: &needle.table,
        state: 0,
        found: false,
    };
    let _ = write!(find, "{argument:?}");
    find.found
}

/// Borrowed twin of `op.reopens() && op.resolved_signature(type_arguments)
/// .is_some_and(|(params, ty)| args.len() == params.len() && result == ty &&
/// every argument type equals its parameter type)`. Scalar conversion, the
/// `ast_type` key/value/`Add` rules and the closed AST round trip are applied
/// to the borrowed types; the collection type is `StringMap` for a non-set
/// `[string, i64]` and otherwise the compiler `Map`/`Set` nominal over exactly
/// the type arguments (identity is its text, as in `DeclarationId` equality).
fn typed_reopen_admitted(
    op: crate::map_ops::MapOp,
    type_arguments: &[crate::hir::ResolvedType],
    args: &[ResolvedExpr],
    result: &crate::hir::ResolvedType,
) -> bool {
    use crate::hir::ResolvedType as T;
    use crate::map_ops::{ast_key, ast_value, resolved_scalar_ast, MapOp, MAP_ID, SET_ID};
    if !op.reopens()
        || type_arguments
            .iter()
            .any(|ty| resolved_scalar_ast(ty).is_none())
        || type_arguments.len() != op.type_arity()
    {
        return false;
    }
    if !resolved_scalar_ast(&type_arguments[0]).is_some_and(|key| ast_key(&key)) {
        return false;
    }
    if !op.is_set() {
        let value = resolved_scalar_ast(&type_arguments[1]);
        if !value.as_ref().is_some_and(ast_value)
            || (op == MapOp::Add && type_arguments[1] != T::I64)
        {
            return false;
        }
    }
    let collection = |ty: &T| {
        if !op.is_set() && matches!(type_arguments, [T::String, T::I64]) {
            return matches!(ty, T::StringMap);
        }
        matches!(ty, T::Nominal { declaration, arguments }
            if declaration.as_str() == if op.is_set() { SET_ID } else { MAP_ID }
                && arguments.as_slice() == type_arguments)
    };
    // Parameters: collection, key, and the value for `Add`/`Set`.
    let expected = 2 + usize::from(matches!(op, MapOp::Add | MapOp::Set));
    args.len() == expected
        && collection(result)
        && collection(&args[0].ty)
        && args[1].ty == type_arguments[0]
        && (expected == 2 || args[2].ty == type_arguments[1])
}

pub(crate) fn is_same_owner_concat_hir_funded<E>(
    value: &ResolvedExpr,
    owner: &ValueId,
    reserve: Reserve<'_, E>,
) -> Result<bool, FundedError<E>> {
    funded::charge(reserve, 0)?;
    let ResolvedExprKind::Call {
        callee,
        type_arguments,
        instance: None,
        args,
    } = &value.kind
    else {
        return Ok(false);
    };
    let typed = crate::map_ops::by_id(callee.as_str())
        .is_some_and(|op| typed_reopen_admitted(op, type_arguments, args, &value.ty));
    let admitted = typed
        || (type_arguments.is_empty()
            && by_id(callee.as_str()).is_some_and(|op| {
                op.is_same_owner_reopen()
                    && args.len() == op.arity()
                    && value.ty == op.return_type()
            }));
    if !admitted
        || !matches!(
            args.first().map(|argument| &argument.kind),
            Some(ResolvedExprKind::Place(place))
                if &place.root == owner && place.projections.is_empty()
        )
    {
        return Ok(false);
    }
    if args.len() > 1 {
        let needle = needle(owner, reserve)?;
        if args[1..].iter().any(|argument| mentions(argument, &needle)) {
            return Ok(false);
        }
    }
    Ok(true)
}

pub(crate) fn same_owner_concat_appends_funded<E>(
    function: &ResolvedFunction,
    reserve: Reserve<'_, E>,
) -> Result<BTreeMap<ExpressionId, ExpressionId>, FundedError<E>> {
    funded::charge(reserve, 0)?;
    let mut appends = BTreeMap::new();
    let mut pending = funded::singleton(&function.body, reserve)?;
    while let Some(expression) = pending.pop() {
        if let ResolvedExprKind::Block { statements, .. }
        | ResolvedExprKind::VecConstructor { statements, .. } = &expression.kind
        {
            for statement in statements {
                if let ResolvedStatement::Assign {
                    binding,
                    field: None,
                    value,
                    ..
                } = statement
                {
                    if is_same_owner_concat_hir_funded(value, &binding.id, reserve)? {
                        if let ResolvedExprKind::Call { args, .. } = &value.kind {
                            funded::charge_clone(&value.id, reserve)?;
                            funded::insert_cloned(
                                &mut appends,
                                &args[0].id,
                                value.id.clone(),
                                reserve,
                            )?;
                        }
                    }
                }
            }
        }
        funded::push_trace_children(expression, &mut pending, reserve)?;
    }
    Ok(appends)
}

pub(crate) fn same_owner_concat_operands_funded<E>(
    function: &ResolvedFunction,
    reserve: Reserve<'_, E>,
) -> Result<BTreeSet<ExpressionId>, FundedError<E>> {
    let appends = same_owner_concat_appends_funded(function, reserve)?;
    let mut operands = BTreeSet::new();
    for key in appends.into_keys() {
        funded::reserve_node::<ExpressionId, (), E>(reserve)?;
        operands.insert(key);
    }
    Ok(operands)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::hir::funded::payer::{assert_every_boundary, Payer};

    const SOURCE: &str = r#"module concat.funded;
@id("c.main") fn main()->i64 {
 let mut text=string_from_i64(1);
 let mut count=0;
 while count<3 {
  text=string_concat(text,"x");
  count=count+1;
  0
 }
 string_len(text)
}"#;

    fn program() -> crate::hir::ResolvedProgram {
        crate::hir::resolve(&crate::check(SOURCE, "funded-concat.spx").unwrap()).unwrap()
    }

    fn pairs(map: &BTreeMap<ExpressionId, ExpressionId>) -> Vec<(String, String)> {
        map.iter()
            .map(|(k, v)| (k.as_str().to_owned(), v.as_str().to_owned()))
            .collect()
    }

    /// Rewrite the append's second operand to read the consumed owner again.
    fn forwarded(program: &mut crate::hir::ResolvedProgram) {
        let function = &mut program.functions[0];
        let ResolvedExprKind::Block { statements, .. } = &mut function.body.kind else {
            panic!("main block")
        };
        let ResolvedStatement::While { body, .. } = statements
            .iter_mut()
            .find(|s| matches!(s, ResolvedStatement::While { .. }))
            .unwrap()
        else {
            unreachable!()
        };
        let ResolvedExprKind::Block { statements, .. } = &mut body.kind else {
            panic!("loop block")
        };
        let ResolvedStatement::Assign { value, .. } = &mut statements[0] else {
            panic!("append")
        };
        let ResolvedExprKind::Call { args, .. } = &mut value.kind else {
            panic!("concat call")
        };
        args[1] = args[0].clone();
    }

    #[test]
    fn funded_appends_and_operands_match_the_original_at_every_boundary() {
        let program = program();
        let function = &program.functions[0];
        let old = same_owner_concat_appends(function);
        assert_eq!(old.len(), 1, "the loop append is the corpus");
        let (value, calls, charged) = assert_every_boundary(|reserve| {
            Ok(pairs(&same_owner_concat_appends_funded(function, reserve)?))
        });
        assert_eq!(value, pairs(&old));
        assert!(calls > 0 && charged > 0);
        let old_operands = same_owner_concat_operands(function);
        let (operands, _, _) = assert_every_boundary(|reserve| {
            Ok(same_owner_concat_operands_funded(function, reserve)?
                .into_iter()
                .map(|id| id.as_str().to_owned())
                .collect::<Vec<_>>())
        });
        assert_eq!(
            operands,
            old_operands
                .iter()
                .map(|id| id.as_str().to_owned())
                .collect::<Vec<_>>()
        );
    }

    #[test]
    fn forwarded_owner_operand_is_rejected_by_both_and_charged_in_full() {
        let mut program = program();
        forwarded(&mut program);
        let function = &program.functions[0];
        let old = same_owner_concat_appends(function);
        assert!(old.is_empty(), "the original rejects the forwarded owner");
        let (value, calls, _) = assert_every_boundary(|reserve| {
            Ok(pairs(&same_owner_concat_appends_funded(function, reserve)?))
        });
        assert!(value.is_empty());
        assert!(calls > 0);
        // The mention test itself is exercised with its needle and table paid.
        let owner = match &function.body.kind {
            ResolvedExprKind::Block { statements, .. } => statements
                .iter()
                .find_map(|s| match s {
                    ResolvedStatement::Let { binding, .. } => Some(binding.id.clone()),
                    _ => None,
                })
                .unwrap(),
            _ => panic!("main block"),
        };
        let mut payer = Payer::unlimited();
        let needle = needle(&owner, &mut |b| payer.pay(b)).unwrap();
        assert_eq!(needle.bytes, format!("{owner:?}").into_bytes());
        assert!(payer.charged >= needle.bytes.len() + needle.table.len() * 8);
    }

    #[test]
    fn streaming_matcher_agrees_with_the_formatted_substring_search() {
        let program = program();
        let function = &program.functions[0];
        let ResolvedExprKind::Block { statements, tail } = &function.body.kind else {
            panic!("main block")
        };
        let owners = statements.iter().filter_map(|s| match s {
            ResolvedStatement::Let { binding, .. } => Some(binding.id.clone()),
            _ => None,
        });
        let mut checked = 0;
        for owner in owners {
            let mut payer = Payer::unlimited();
            let needle = needle(&owner, &mut |b| payer.pay(b)).unwrap();
            for expression in [&function.body, tail.as_ref()] {
                assert_eq!(
                    mentions(expression, &needle),
                    format!("{expression:?}").contains(&format!("{owner:?}"))
                );
                checked += 1;
            }
        }
        assert!(checked >= 4);
    }

    use crate::hir::{DeclarationId, ResolvedType as T};
    use crate::map_ops::{MapOp, MAP_ID, SET_ID};

    fn append_call(program: &crate::hir::ResolvedProgram) -> &ResolvedExpr {
        let ResolvedExprKind::Block { statements, .. } = &program.functions[0].body.kind else {
            panic!("main block")
        };
        let ResolvedStatement::While { body, .. } = statements
            .iter()
            .find(|s| matches!(s, ResolvedStatement::While { .. }))
            .unwrap()
        else {
            unreachable!()
        };
        let ResolvedExprKind::Block { statements, .. } = &body.kind else {
            panic!("loop block")
        };
        let ResolvedStatement::Assign { value, .. } = &statements[0] else {
            panic!("append")
        };
        value
    }

    /// The owner and a different binding of the fixture's `main`.
    fn owners(program: &crate::hir::ResolvedProgram) -> (ValueId, ValueId) {
        let ResolvedExprKind::Block { statements, .. } = &program.functions[0].body.kind else {
            panic!("main block")
        };
        let mut lets = statements.iter().filter_map(|s| match s {
            ResolvedStatement::Let { binding, .. } => Some(binding.id.clone()),
            _ => None,
        });
        (lets.next().unwrap(), lets.next().unwrap())
    }

    /// A call of the fixture's shape with the given callee, type arguments,
    /// argument types and result type; arguments after the first are copies of
    /// the append's second operand.
    fn variant(base: &ResolvedExpr, callee: &str, targs: &[T], tys: &[T], ty: &T) -> ResolvedExpr {
        let mut call = base.clone();
        call.ty = ty.clone();
        let ResolvedExprKind::Call {
            callee: name,
            type_arguments,
            args,
            ..
        } = &mut call.kind
        else {
            unreachable!()
        };
        *name = DeclarationId::new(callee);
        *type_arguments = targs.to_vec();
        let (first, rest) = (args[0].clone(), args[1].clone());
        *args = tys
            .iter()
            .enumerate()
            .map(|(at, ty)| {
                let mut arg = if at == 0 { first.clone() } else { rest.clone() };
                arg.ty = ty.clone();
                arg
            })
            .collect();
        call
    }

    /// The original owning signature, read independently of the projection.
    fn old_signature(op: MapOp, targs: &[T], call: &ResolvedExpr) -> bool {
        let ResolvedExprKind::Call { args, .. } = &call.kind else {
            unreachable!()
        };
        op.reopens()
            && op.resolved_signature(targs).is_some_and(|(params, ty)| {
                args.len() == params.len()
                    && call.ty == ty
                    && args
                        .iter()
                        .zip(params)
                        .all(|(arg, param)| arg.ty == param.ty)
            })
    }

    fn type_argument_lists() -> Vec<Vec<T>> {
        let mut domain = vec![
            T::String,
            T::I64,
            T::I32,
            T::U8,
            T::Usize,
            T::Char,
            T::F32,
            T::F64,
            T::Bool,
        ];
        domain.extend([T::Unit, T::StringMap]);
        let mut lists = vec![vec![]];
        for a in &domain {
            lists.push(vec![a.clone()]);
            lists.extend(domain.iter().map(|b| vec![a.clone(), b.clone()]));
        }
        for a in [T::String, T::I64, T::Bool] {
            for b in [T::String, T::I64] {
                for c in [T::I64, T::Unit] {
                    lists.push(vec![a.clone(), b.clone(), c]);
                }
            }
        }
        lists
    }

    #[test]
    fn typed_map_reopen_projection_matches_the_original_signature_exhaustively() {
        let program = program();
        let base = append_call(&program);
        let (owner, other) = owners(&program);
        let (mut admitted, mut refused) = (0usize, 0usize);
        let mut ops = BTreeSet::new();
        let mut string_map = false;
        for op in MapOp::ALL {
            for targs in type_argument_lists() {
                let canonical = op.resolved_signature(&targs).map(|(params, result)| {
                    (params.into_iter().map(|p| p.ty).collect::<Vec<_>>(), result)
                });
                let (tys, result) = canonical.unwrap_or((vec![T::I64, T::I64], T::I64));
                let nominal = T::Nominal {
                    declaration: DeclarationId::new(if op.is_set() { SET_ID } else { MAP_ID }),
                    arguments: targs.clone(),
                };
                let mut cases = vec![(tys.clone(), result.clone())];
                for at in 0..tys.len() {
                    let mut changed = tys.clone();
                    changed[at] = T::Unit;
                    cases.push((changed, result.clone()));
                }
                cases.push((tys.clone(), T::Unit));
                cases.push((tys[..tys.len() - 1].to_vec(), result.clone()));
                let mut extra = tys.clone();
                extra.push(T::I64);
                cases.push((extra, result.clone()));
                let mut swapped = tys.clone();
                swapped[0] = nominal.clone();
                cases.push((swapped.clone(), nominal.clone()));
                cases.push((swapped, result.clone()));
                cases.push((tys.clone(), nominal.clone()));
                for id in [MAP_ID, SET_ID, "forged.collection"] {
                    let forge = |ty: &T| match ty {
                        T::Nominal { arguments, .. } => T::Nominal {
                            declaration: DeclarationId::new(id),
                            arguments: arguments.clone(),
                        },
                        other => other.clone(),
                    };
                    cases.push((tys.iter().map(forge).collect(), forge(&result)));
                }
                for (case_tys, case_result) in cases {
                    let call = variant(base, op.id(), &targs, &case_tys, &case_result);
                    let ResolvedExprKind::Call { args, .. } = &call.kind else {
                        unreachable!()
                    };
                    let expected = old_signature(op, &targs, &call);
                    assert_eq!(
                        typed_reopen_admitted(op, &targs, args, &call.ty),
                        expected,
                        "{op:?} {targs:?} {case_tys:?} -> {case_result:?}"
                    );
                    let mut payer = Payer::unlimited();
                    let old = is_same_owner_concat_hir(&call, &owner);
                    let new = is_same_owner_concat_hir_funded(&call, &owner, &mut |b| payer.pay(b));
                    assert_eq!(new, Ok(old), "{op:?} {targs:?} {case_tys:?}");
                    assert_eq!(
                        is_same_owner_concat_hir_funded(&call, &other, &mut |b| payer.pay(b)),
                        Ok(is_same_owner_concat_hir(&call, &other))
                    );
                    if expected {
                        assert!(old, "the fixture owner is admitted when the signature is");
                        admitted += 1;
                        ops.insert(op.name());
                        string_map |= targs == [T::String, T::I64] && case_result == T::StringMap;
                        if args.len() > 1 {
                            // A later operand that reads the consumed owner is refused.
                            let mut mention = call.clone();
                            let ResolvedExprKind::Call { args, .. } = &mut mention.kind else {
                                unreachable!()
                            };
                            let last = args.len() - 1;
                            args[last].kind = args[0].kind.clone();
                            assert!(!is_same_owner_concat_hir(&mention, &owner));
                            assert_eq!(
                                is_same_owner_concat_hir_funded(&mention, &owner, &mut |b| {
                                    payer.pay(b)
                                }),
                                Ok(false)
                            );
                        }
                    } else {
                        refused += 1;
                    }
                }
            }
        }
        assert!(admitted > 40 && refused > 5000, "{admitted} {refused}");
        assert_eq!(ops.len(), 5, "every typed reopen is admitted somewhere");
        assert!(string_map, "the StringMap carrier shape is exercised");
        let unknown = variant(
            base,
            "not.a.map.op",
            &[],
            &[T::String, T::String],
            &T::String,
        );
        let mut payer = Payer::unlimited();
        assert_eq!(
            is_same_owner_concat_hir_funded(&unknown, &owner, &mut |b| payer.pay(b)),
            Ok(is_same_owner_concat_hir(&unknown, &owner))
        );
    }

    #[test]
    fn typed_map_reopen_is_decided_without_unfunded_and_refuses_at_every_boundary() {
        let program = program();
        let base = append_call(&program);
        let (owner, _) = owners(&program);
        let targs = [T::I64, T::I64];
        let (tys, result) = {
            let (params, result) = MapOp::Set.resolved_signature(&targs).unwrap();
            (params.into_iter().map(|p| p.ty).collect::<Vec<_>>(), result)
        };
        let call = variant(base, MapOp::Set.id(), &targs, &tys, &result);
        assert!(is_same_owner_concat_hir(&call, &owner));
        let (value, calls, charged) = assert_every_boundary(|reserve| {
            is_same_owner_concat_hir_funded(&call, &owner, reserve)
        });
        // Admitted with two later operands: the owner needle and table are paid.
        assert!(value && calls > 1 && charged > 0);
    }
}

#[cfg(test)]
pub(crate) mod work;
