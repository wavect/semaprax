//! Explicit-payer funded twins of `condition_reads`, `function_reads` and
//! `needs_cleanup`. Same pop-from-the-end discipline, same `While`-condition
//! only statement rule inside condition blocks, and the interpreter
//! `trace_child_expressions` order for every other node. The pending stacks,
//! the identity sets (node plus clone payload) and the merge of a condition's
//! set into the function's are all debited on the caller's payer first, and its
//! refusal is returned unchanged. No owned temporary `Vec` or set is built
//! before its debit, and nothing reads the production ACTIVE ledger.
use super::*;
use crate::hir::funded::{self, FundedError, Reserve};

pub(crate) fn condition_reads_funded<E>(
    condition: &ResolvedExpr,
    reserve: Reserve<'_, E>,
) -> Result<BTreeSet<ExpressionId>, FundedError<E>> {
    funded::charge(reserve, 0)?;
    let mut reads = BTreeSet::new();
    let mut pending = funded::singleton(condition, reserve)?;
    while let Some(expression) = pending.pop() {
        if let Some(arguments) = resolved_operands(expression) {
            for argument in arguments {
                funded::insert_cloned_key(&mut reads, &argument.id, reserve)?;
            }
            continue;
        }
        if let ResolvedExprKind::Block { statements, tail } = &expression.kind {
            let mut needed = 1usize;
            for statement in statements {
                needed += if matches!(statement, ResolvedStatement::While { .. }) {
                    1
                } else {
                    (0..statement.child_count())
                        .filter(|index| statement.child(*index).is_some())
                        .count()
                };
            }
            funded::grow(&mut pending, needed, reserve)?;
            pending.push(tail);
            for statement in statements {
                if let ResolvedStatement::While { condition, .. } = statement {
                    pending.push(condition);
                } else {
                    for index in 0..statement.child_count() {
                        if let Some(child) = statement.child(index) {
                            pending.push(child);
                        }
                    }
                }
            }
        } else {
            funded::push_trace_children(expression, &mut pending, reserve)?;
        }
    }
    Ok(reads)
}

pub(crate) fn function_reads_funded<E>(
    function: &ResolvedFunction,
    reserve: Reserve<'_, E>,
) -> Result<BTreeSet<ExpressionId>, FundedError<E>> {
    funded::charge(reserve, 0)?;
    let mut reads = BTreeSet::new();
    let mut pending = funded::singleton(&function.body, reserve)?;
    while let Some(expression) = pending.pop() {
        if let ResolvedExprKind::Block { statements, .. }
        | ResolvedExprKind::VecConstructor { statements, .. } = &expression.kind
        {
            for statement in statements {
                if let ResolvedStatement::While { condition, .. } = statement {
                    let found = condition_reads_funded(condition, reserve)?;
                    funded::extend_set(&mut reads, found, reserve)?;
                }
            }
        }
        funded::push_trace_children(expression, &mut pending, reserve)?;
    }
    Ok(reads)
}

#[cfg(test)]
pub(crate) fn needs_cleanup_funded<E>(
    condition: &ResolvedExpr,
    reserve: Reserve<'_, E>,
) -> Result<bool, FundedError<E>> {
    let reads = condition_reads_funded(condition, reserve)?;
    let mut pending = funded::singleton(condition, reserve)?;
    while let Some(expression) = pending.pop() {
        if expression.ty == ResolvedType::String && !reads.contains(&expression.id) {
            return Ok(true);
        }
        funded::push_children(expression, &mut pending, reserve)?;
    }
    Ok(false)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::hir::funded::payer::assert_every_boundary;

    /// Closure-free fixtures: a body-only condition read, a `while` inside a
    /// block whose condition reads a String owner, and a non-reading use.
    const SOURCE: &str = r#"module cond.funded;
@id("c.reads") fn reads(text: own string)->i64 {
 let mut owner=text;
 let mut count=0;
 while string_len(owner)>0 && string_len(owner)<4 {
  owner=string_concat(owner,"x");
  count=count+1;
  0
 }
 let other=string_from_i64(count);
 while string_len(other)==0 { count=count+1; 0 }
 if string_len(other)>0 {count}else{0}
}
@id("app.main") fn main()->i64 {0}
"#;

    fn ids(set: &BTreeSet<ExpressionId>) -> Vec<String> {
        set.iter().map(|id| id.as_str().to_owned()).collect()
    }

    #[test]
    fn original_explicit_owned_string_fixture_retains_its_source_refusal() {
        let diagnostics = crate::check(SOURCE, "funded-conditions.spx").unwrap_err();
        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].code, "SPX-O002");
        assert_eq!(diagnostics[0].message, "string parameters own their input implicitly; an explicit ownership mode is not admitted");
    }

    fn program() -> crate::hir::ResolvedProgram {
        let admitted = SOURCE.replace("text: own string", "text: string");
        crate::hir::resolve(&crate::check(&admitted, "funded-conditions.spx").unwrap()).unwrap()
    }

    #[test]
    fn funded_function_and_condition_reads_match_the_original_at_every_boundary() {
        let program = program();
        let mut total = 0;
        for function in &program.functions {
            let old = function_reads(function);
            total += old.len();
            let (value, calls, charged) = assert_every_boundary(|reserve| {
                Ok(ids(&function_reads_funded(function, reserve)?))
            });
            assert_eq!(value, ids(&old), "{}", function.id.as_str());
            assert!(calls > 0 && (old.is_empty() || charged > 0));
            // Every `while` condition, and every expression as a body-only
            // condition (nested while/closure shapes included).
            let mut pending = vec![&function.body];
            while let Some(expression) = pending.pop() {
                let old = condition_reads(expression);
                let (value, _, _) = assert_every_boundary(|reserve| {
                    Ok(ids(&condition_reads_funded(expression, reserve)?))
                });
                assert_eq!(value, ids(&old));
                let (cleanup, _, _) =
                    assert_every_boundary(|reserve| needs_cleanup_funded(expression, reserve));
                assert_eq!(cleanup, needs_cleanup(expression));
                pending.extend(crate::interpreter::trace_child_expressions(expression));
            }
        }
        assert!(total >= 3, "the corpus must exercise condition reads");
    }

    #[test]
    fn funded_reads_never_touch_the_production_ledger() {
        let program = program();
        let function = &program.functions[0];
        let (calls, refused, active) = crate::bounded_output::with_limit_usage(0, || {
            let mut payer = funded::payer::Payer::unlimited();
            function_reads_funded(function, &mut |b| payer.pay(b)).unwrap();
            payer.calls
        });
        assert!(calls > 0 && !refused);
        assert_eq!(active, 0);
    }
}

#[cfg(test)]
pub(crate) mod work;
