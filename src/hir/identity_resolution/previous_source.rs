//! Complete source-checked receiver with a compiler-held previous ProgramSource.
use super::super::function_reuse_source::environment_is_exact as function_reuse_environment_is_exact;
use super::*;

#[allow(
    dead_code,
    reason = "retained original receiver or refusal payload; not every legacy entry is selected by current tests"
)]
pub(crate) fn resolve_source_checked_previous<'p, E>(
    program: &'p dyn ProgramSource,
    previous_program: Option<&dyn ProgramSource>,
    previous_resolved: Option<&ResolvedProgram>,
    previous_costs: Option<&BTreeMap<String, usize>>,
    private: Option<source_verify::private_names::PrivateNames<'p>>,
    mut reserve: impl FnMut(usize) -> Result<(), E>,
) -> Result<(ResolvedProgram, FunctionResolutionWork), Failure<E>> {
    #[cfg(feature = "unstable-workflow-profiling")]
    let _workflow_span = crate::workflow_profile::span(crate::workflow_profile::Stage::Resolve);
    let diagnostics = source_verify::verify_presenting(program, private);
    if diagnostics
        .iter()
        .any(|diagnostic| diagnostic.severity.is_error())
    {
        return Err(Failure::Source(diagnostics));
    }
    let reuse = previous_program
        .zip(previous_resolved)
        .zip(previous_costs)
        .filter(|((previous, _), _)| function_reuse_environment_is_exact(program, *previous))
        .map(|((previous, resolved), costs)| FunctionReuse {
            current: program,
            program: previous,
            resolved,
            costs,
        });
    let clause_program = record_invariants::source::clause_program(program);
    let inventory: &dyn ProgramSource = clause_program.as_ref().map_or(program, |owned| owned);
    let resolved = source_pass(
        inventory,
        if clause_program.is_some() {
            None
        } else {
            reuse
        },
        &mut reserve,
    )?;
    let Some(desugared) = record_invariants::source::desugar(program, &resolved.0) else {
        return Ok(resolved);
    };
    pass(&desugared, None, &mut reserve)
}

#[cfg(test)]
mod tests {
    #[test]
    fn full_source_checked_previous_receiver_is_exact_original_with_named_input_hooks() {
        let old = include_str!("../../../docs/evidence/immutable-cache-source-split/src__hir__identity_resolution.rs.before.txt");
        let current = include_str!("previous_source.rs");
        let extract = |source: &str, marker: &str| {
            let start = source.find(marker).unwrap();
            let end = start + source[start..].find("\n}\n").unwrap() + 2;
            source[start..end].to_owned()
        };
        let old = extract(old, "pub(crate) fn resolve_source_checked<");
        let current = extract(current, "pub(crate) fn resolve_source_checked_previous<")
            .replace("resolve_source_checked_previous", "resolve_source_checked")
            .replace("Option<&dyn ProgramSource>", "Option<&Program>")
            .replace(
                "function_reuse_environment_is_exact(program, *previous)",
                "function_reuse_environment_is_exact(program, previous)",
            );
        assert_eq!(old, current);
    }
}
