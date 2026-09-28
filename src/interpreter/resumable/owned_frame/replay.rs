//! Shared borrowed phase evaluator: no owning Arc enters the Copy environment.
use super::*;

#[cfg(test)]
thread_local! {static EVALUATIONS:std::cell::Cell<usize>=const{std::cell::Cell::new(0)};}
#[cfg(test)]
pub(super) fn evaluation_count() -> usize {
    EVALUATIONS.with(std::cell::Cell::get)
}
#[cfg(test)]
pub(super) fn reset_evaluations() {
    EVALUATIONS.with(|count| count.set(0));
}

pub(super) type PhaseResult = Result<Option<(ArgumentValue, usize)>, Flow>;
#[allow(clippy::too_many_arguments)]
pub(super) fn evaluate(
    plan: &CheckedOwnedFramePlan,
    root: &Value,
    mut environment: Environment,
    next: usize,
    answer: Option<Value>,
    start: bool,
    budget: &mut OwnedFrameBudget,
) -> (PhaseResult, Environment, bool) {
    #[cfg(test)]
    EVALUATIONS.with(|count| count.set(count.get() + 1));
    let entry = plan.function();
    let admitted = BTreeMap::new();
    let mut evaluator = Evaluator::new_prepared(
        FunctionLookup::Borrowed(&admitted),
        BTreeMap::new(),
        &plan.program().declarations,
        budget.remaining,
        0,
        PreparedCancellation::Never,
    );
    let mut provisional = false;
    let evaluated: Result<Option<(ArgumentValue, usize)>, Flow> = (|| {
        if start {
            evaluator.charge()?; // frame entry, charged in the consuming lane
            check_contracts(&mut evaluator, entry, root, &mut environment, true)?;
        }
        let ResolvedExprKind::Block { statements, .. } = &entry.body.kind else {
            return Err(Flow::Guard("checked body changed"));
        };
        let mut answer = answer;
        for (index, statement) in statements.iter().enumerate().skip(next) {
            let ResolvedStatement::Let { binding, value, .. } = statement else {
                return Err(Flow::Guard("checked statement changed"));
            };
            if let ResolvedExprKind::Yield { request } = &value.kind {
                if start {
                    evaluator.charge()?; // yield node
                    let produced =
                        evaluate_copy(&mut evaluator, request, entry, root, &mut environment)?;
                    let request = super::super::argument_of(&produced)
                        .ok_or(Flow::Guard("non-scalar request"))?;
                    return Ok(Some((request, index)));
                }
                evaluator.charge()?; // resumed yield node
                environment.push((
                    binding.id.clone(),
                    answer.take().ok_or(Flow::Guard("missing answer"))?,
                ));
            } else {
                let value = evaluate_copy(&mut evaluator, value, entry, root, &mut environment)?;
                environment.push((binding.id.clone(), value));
            }
        }
        evaluator.charge()?; // whole identity tail transfers the retained root
        provisional = true;
        check_contracts(&mut evaluator, entry, root, &mut environment, false)?;
        Ok(None)
    })();
    let steps = evaluator.steps;
    drop(evaluator); // no owning root was ever installed in its environment
    budget.remaining -= steps;
    budget.consumed += steps;
    (evaluated, environment, provisional)
}
