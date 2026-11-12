//! Complete paired replay through the current invocation-owned receivers.
use super::*;

pub(in crate::cleanup_plan::replay) fn replay(
    program: &ResolvedProgram,
    indexed: bool,
) -> Result<(usize, usize), Diagnostic> {
    let full = program;
    let mut budget = ReplayBudget::new();
    let helpers = if indexed {
        Some(ordinary_renewals::index(full, &mut budget)?)
    } else {
        None
    };
    let functions = || {
        full.functions.iter().chain(
            full.function_instances
                .iter()
                .map(|instance| &instance.function),
        )
    };
    with_program_status_backing!(program, full, budget,
        functions(), fact_backing, first_status_function; {
        budget.reserve_bytes(
            first_status_function,
            expression_facts::receiving::StatusBacking::ACQUISITION_BYTES,
            "invocation status backing acquisition and receiving",
        )?;
        let mut status_backing =
            expression_facts::receiving::StatusBacking::acquire(full, first_status_function, &mut budget)?;
        let mut continuations = continuation_storage::Storage::for_program(full, &mut budget)?;
        for function in functions() {
            expression_facts::receiving::validate_structure_with_status_backing(
                program,
                function,
                &mut budget,
                helpers.as_ref(),
                Some(&mut continuations),
                &mut fact_backing,
                &mut status_backing,
            )?;
            if !status_backing.complete_into(program, function, &mut budget) {
                return Err(status_backing.take_error());
            }
        }
        Ok::<(), Diagnostic>(())
    })?;
    Ok((
        MAX_REPLAY_WORK_UNITS - budget.remaining,
        budget.skeleton_spent,
    ))
}
