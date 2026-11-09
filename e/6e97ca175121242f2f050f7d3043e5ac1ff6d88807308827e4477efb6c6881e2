use super::*;
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
    budget.reserve_bytes(first_status_function, expression_facts::receiving::Before941StatusBacking::ACQUISITION_BYTES,
        "invocation status backing acquisition and receiving")?;
    let mut status_backing = expression_facts::receiving::Before941StatusBacking::acquire(full, first_status_function, &mut budget)?;
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
        expression_facts::receiving::before_941_status_structure(
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
        expression_facts::receiving::before_941_status_structure(
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
