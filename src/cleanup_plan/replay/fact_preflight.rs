//! Lexical current preflight; complete old caller/preflight remain independent.
use super::*;
#[cfg(test)]
macro_rules! with_program_backing {
    ($program:ident, $full:ident, $budget:ident, $functions:expr, $owner:ident; $continuation:block) => {{
        let mut total = 0usize;
        let mut first = None;
        for function in $functions {
            first.get_or_insert(function);
            // Actual lexical HIR selects a deferred resource plan before the
            // capped enumerating census. This is no proof or zero-work estimate.
            if factored::strategy::provisional(function)
                == factored::strategy::ResourceStrategy::DeferredLexical
            {
                continue;
            }
            let function_upper = skeleton_work_upper($program, function)?;
            total = total.checked_add(function_upper).ok_or_else(|| {
                replay_error(
                    function,
                    "cleanup replay program-wide skeleton-work preflight overflowed",
                )
            })?;
            if total > MAX_REPLAY_WORK_UNITS {
                return Err(replay_error(
                    function,
                    "cleanup replay program-wide skeleton-work preflight exceeds the global budget",
                ));
            }
        }
        if let Some(function) = first {
            $budget.reserve_skeleton(function, total)?;
            $budget.reserve_bytes(
                function,
                expression_facts::arena::invocation::Backing::ACQUISITION_BYTES,
                "invocation fact backing acquisition and returned receiver",
            )?;
            let mut $owner =
                expression_facts::arena::invocation::Backing::acquire($full, function, $budget)?;
            $continuation
        } else {
            // No owner-shaped Option/Result/return packet exists on this path.
            // The caller publishes its same original Result<(),Diagnostic>.
            Ok(())
        }
    }};
}

macro_rules! with_program_status_backing {
    ($program:ident, $full:ident, $budget:ident, $functions:expr, $owner:ident, $first:ident; $continuation:block) => {{
        let mut total = 0usize;
        let mut first = None;
        for function in $functions {
            first.get_or_insert(function);
            // Actual lexical HIR selects a deferred resource plan before the
            // capped enumerating census. This is no proof or zero-work estimate.
            if factored::strategy::provisional(function)
                == factored::strategy::ResourceStrategy::DeferredLexical
            {
                continue;
            }
            let function_upper = skeleton_work_upper($program, function)?;
            total = total.checked_add(function_upper).ok_or_else(|| {
                replay_error(
                    function,
                    "cleanup replay program-wide skeleton-work preflight overflowed",
                )
            })?;
            if total > MAX_REPLAY_WORK_UNITS {
                return Err(replay_error(
                    function,
                    "cleanup replay program-wide skeleton-work preflight exceeds the global budget",
                ));
            }
        }
        if let Some($first) = first {
            $budget.reserve_skeleton($first, total)?;
            $budget.reserve_bytes(
                $first,
                expression_facts::arena::invocation::Backing::ACQUISITION_BYTES,
                "invocation fact backing acquisition and returned receiver",
            )?;
            let mut $owner =
                expression_facts::arena::invocation::Backing::acquire($full, $first, $budget)?;
            $continuation
        } else {
            // No owner-shaped Option/Result/return packet exists on this path.
            // The caller publishes its same original Result<(),Diagnostic>.
            Ok(())
        }
    }};
}

pub(in crate::cleanup_plan) fn validate_program_context(
    program: &dyn crate::cleanup::ReplayContext,
) -> Result<(), Diagnostic> {
    let full = program.full_program().ok_or_else(|| {
        Diagnostic::io(
            "SPX-H006",
            "allocation replay requires the held complete program",
        )
    })?;
    let mut budget = ReplayBudget::new();
    let helpers = ordinary_renewals::index(full, &mut budget)?;
    with_program_status_backing!(program, full, budget,
        program.functions().iter().chain(program.function_instances().iter().map(|instance| &instance.function)),
        fact_backing, first_status_function; {
    budget.reserve_bytes(first_status_function, expression_facts::receiving::StatusBacking::ACQUISITION_BYTES,
        "invocation status backing acquisition and receiving")?;
    let mut status_backing = expression_facts::receiving::StatusBacking::acquire(full, first_status_function, &mut budget)?;
    let mut continuations = continuation_storage::Storage::for_program(full, &mut budget)?;
    #[cfg(test)]
    let mut diagnostic_ordinal = 0usize;
    for function in program.functions() {
        #[cfg(test)]
        let _debit_function = {
            let scope = debit_routes::function(diagnostic_ordinal, false, function.id.as_str());
            diagnostic_ordinal = diagnostic_ordinal.saturating_add(1);
            scope
        };
        #[cfg(feature = "unstable-workflow-profiling")]
        let _paid_structure = budget.paid_phase(PaidPhase::ReplayStructure);
        expression_facts::receiving::validate_structure_with_status_backing(
            program,
            function,
            &mut budget,
            Some(&helpers),
            Some(&mut continuations),
            &mut fact_backing,
            &mut status_backing,
        )?;
        status_backing.complete(program, function, &mut budget)?;
    }
    #[cfg(test)]
    let mut diagnostic_instance = 0usize;
    for instance in program.function_instances() {
        #[cfg(test)]
        let _debit_function = {
            let scope =
                debit_routes::function(diagnostic_instance, true, instance.function.id.as_str());
            diagnostic_instance = diagnostic_instance.saturating_add(1);
            scope
        };
        #[cfg(feature = "unstable-workflow-profiling")]
        let _paid_structure = budget.paid_phase(PaidPhase::ReplayStructure);
        expression_facts::receiving::validate_structure_with_status_backing(
            program,
            &instance.function,
            &mut budget,
            Some(&helpers),
            Some(&mut continuations),
            &mut fact_backing,
            &mut status_backing,
        )?;
        status_backing.complete(program, &instance.function, &mut budget)?;
    }
    Ok(())
    })
}

#[cfg(test)]
pub(in crate::cleanup_plan) fn before_status_backing_validate_program_context(
    program: &dyn crate::cleanup::ReplayContext,
) -> Result<(), Diagnostic> {
    let full = program.full_program().ok_or_else(|| {
        Diagnostic::io(
            "SPX-H006",
            "allocation replay requires the held complete program",
        )
    })?;
    let mut budget = ReplayBudget::new();
    let helpers = ordinary_renewals::index(full, &mut budget)?;
    with_program_backing!(program, full, budget,
        program.functions().iter().chain(program.function_instances().iter().map(|instance| &instance.function)),
        fact_backing; {
    let mut continuations = continuation_storage::Storage::for_program(full, &mut budget)?;
    #[cfg(test)]
    let mut diagnostic_ordinal = 0usize;
    for function in program.functions() {
        #[cfg(test)]
        let _debit_function = {
            let scope = debit_routes::function(diagnostic_ordinal, false, function.id.as_str());
            diagnostic_ordinal = diagnostic_ordinal.saturating_add(1);
            scope
        };
        #[cfg(feature = "unstable-workflow-profiling")]
        let _paid_structure = budget.paid_phase(PaidPhase::ReplayStructure);
        expression_facts::receiving::validate_structure_with_fact_backing(
            program,
            function,
            &mut budget,
            Some(&helpers),
            Some(&mut continuations),
            &mut fact_backing,
        )?;
    }
    #[cfg(test)]
    let mut diagnostic_instance = 0usize;
    for instance in program.function_instances() {
        #[cfg(test)]
        let _debit_function = {
            let scope =
                debit_routes::function(diagnostic_instance, true, instance.function.id.as_str());
            diagnostic_instance = diagnostic_instance.saturating_add(1);
            scope
        };
        #[cfg(feature = "unstable-workflow-profiling")]
        let _paid_structure = budget.paid_phase(PaidPhase::ReplayStructure);
        expression_facts::receiving::validate_structure_with_fact_backing(
            program,
            &instance.function,
            &mut budget,
            Some(&helpers),
            Some(&mut continuations),
            &mut fact_backing,
        )?;
    }
    Ok(())
    })
}

#[cfg(test)]
pub(in crate::cleanup_plan) fn before_invocation_validate_program_context(
    program: &dyn crate::cleanup::ReplayContext,
) -> Result<(), Diagnostic> {
    let full = program.full_program().ok_or_else(|| {
        Diagnostic::io(
            "SPX-H006",
            "allocation replay requires the held complete program",
        )
    })?;
    let mut budget = ReplayBudget::new();
    let helpers = ordinary_renewals::index(full, &mut budget)?;
    reserve_program_skeleton_work(
        program,
        program.functions().iter().chain(
            program
                .function_instances()
                .iter()
                .map(|instance| &instance.function),
        ),
        &mut budget,
    )?;
    let mut continuations = continuation_storage::Storage::for_program(full, &mut budget)?;
    #[cfg(test)]
    let mut diagnostic_ordinal = 0usize;
    for function in program.functions() {
        #[cfg(test)]
        let _debit_function = {
            let scope = debit_routes::function(diagnostic_ordinal, false, function.id.as_str());
            diagnostic_ordinal = diagnostic_ordinal.saturating_add(1);
            scope
        };
        #[cfg(feature = "unstable-workflow-profiling")]
        let _paid_structure = budget.paid_phase(PaidPhase::ReplayStructure);
        validate_structure_with_budget_storage(
            program,
            function,
            &mut budget,
            Some(&helpers),
            Some(&mut continuations),
        )?;
    }
    #[cfg(test)]
    let mut diagnostic_instance = 0usize;
    for instance in program.function_instances() {
        #[cfg(test)]
        let _debit_function = {
            let scope =
                debit_routes::function(diagnostic_instance, true, instance.function.id.as_str());
            diagnostic_instance = diagnostic_instance.saturating_add(1);
            scope
        };
        #[cfg(feature = "unstable-workflow-profiling")]
        let _paid_structure = budget.paid_phase(PaidPhase::ReplayStructure);
        validate_structure_with_budget_storage(
            program,
            &instance.function,
            &mut budget,
            Some(&helpers),
            Some(&mut continuations),
        )?;
    }
    Ok(())
}
