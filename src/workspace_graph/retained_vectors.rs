//! Output-carrier reservation for the final uncached core retry.

use crate::diagnostic::Diagnostic;

use super::reserve_builder_structure;
use super::{hir, WorkspaceResolvedModule};
use std::collections::BTreeMap;

pub(super) fn reserve_workspace_module_carrier(count: usize) -> Result<(), Vec<Diagnostic>> {
    reserve_builder_structure(
        count
            // Empty private signature and Agent facts must not change frozen
            // scalar graph accounting. Nonempty carriers are charged separately.
            .checked_mul(
                std::mem::size_of::<WorkspaceResolvedModule>()
                    - std::mem::size_of::<BTreeMap<String, (hir::DeclarationKind, hir::TypeFacts)>>(
                    )
                    - std::mem::size_of::<Vec<hir::ResolvedAgentDeclaration>>(),
            )
            .ok_or_else(|| {
                vec![super::limit_error(
                    "builder_bytes",
                    super::active_builder_limit(),
                )]
            })?,
    )
}

fn limit_error() -> Vec<Diagnostic> {
    vec![super::limit_error(
        "builder_bytes",
        super::active_builder_limit(),
    )]
}

fn output_carrier<T>(
    selected: usize,
    fixed_element_bytes: usize,
) -> Result<Vec<T>, Vec<Diagnostic>> {
    let reserved = selected
        .checked_mul(fixed_element_bytes)
        .ok_or_else(limit_error)?;
    reserve_builder_structure(reserved)?;
    let retained = Vec::with_capacity(selected);
    let actual = retained
        .capacity()
        .checked_mul(fixed_element_bytes)
        .ok_or_else(limit_error)?;
    if actual > reserved {
        reserve_builder_structure(actual - reserved)?;
    }
    Ok(retained)
}

/// Move selected entries into a retained output carrier.
///
/// Normal core attempts retain the established full-carrier receipt. The final
/// uncached retry keeps the complete input carrier live and charged until this
/// move finishes, then charges only the new output carrier. `keep` is an `Fn`:
/// the final retry counts and moves the same selection without accepting a
/// stateful predicate as accounting evidence.
pub(super) fn filter_owned_vec<T>(
    items: Vec<T>,
    keep: impl Fn(&T) -> bool,
    retained_output_only: bool,
) -> Result<Vec<T>, Vec<Diagnostic>> {
    if !retained_output_only {
        reserve_builder_structure(
            items
                .len()
                .checked_mul(std::mem::size_of::<T>())
                .ok_or_else(limit_error)?,
        )?;
        return Ok(items.into_iter().filter(|item| keep(item)).collect());
    }

    let selected = items.iter().filter(|item| keep(*item)).count();
    let mut retained = output_carrier::<T>(selected, std::mem::size_of::<T>())?;
    for item in items {
        if keep(&item) {
            retained.push(item);
        }
    }
    debug_assert_eq!(retained.len(), selected);
    Ok(retained)
}

/// Move selected entries and account their loan-plan sidecars.
pub(super) fn filter_owned_vec_accounted<T>(
    items: Vec<T>,
    fixed_element_bytes: usize,
    extra_owned_bytes: impl Fn(&T) -> Result<usize, Vec<Diagnostic>>,
    keep: impl Fn(&T) -> bool,
    retained_output_only: bool,
) -> Result<Vec<T>, Vec<Diagnostic>> {
    if !retained_output_only {
        let fixed = items
            .len()
            .checked_mul(fixed_element_bytes)
            .ok_or_else(limit_error)?;
        let mut bytes = fixed;
        for item in &items {
            bytes = bytes
                .checked_add(extra_owned_bytes(item)?)
                .ok_or_else(limit_error)?;
        }
        reserve_builder_structure(bytes)?;
        return Ok(items.into_iter().filter(|item| keep(item)).collect());
    }

    let mut selected = 0usize;
    let mut sidecars = 0usize;
    for item in &items {
        if keep(item) {
            selected = selected.checked_add(1).ok_or_else(limit_error)?;
            sidecars = sidecars
                .checked_add(extra_owned_bytes(item)?)
                .ok_or_else(limit_error)?;
        }
    }
    reserve_builder_structure(sidecars)?;
    let mut retained = output_carrier::<T>(selected, fixed_element_bytes)?;
    for item in items {
        if keep(&item) {
            retained.push(item);
        }
    }
    debug_assert_eq!(retained.len(), selected);
    Ok(retained)
}

/// Reserve actual retained Agent clone carriers and payloads before linking.
/// Legacy source bytes do not change, but the successor cache accounts its
/// actual Rust metadata footprint even when optional helper metadata is absent.
pub(super) fn reserve_agent_execution_metadata<'a>(
    agents: impl Iterator<Item = &'a hir::ResolvedAgentDeclaration>,
) -> Result<(), Vec<Diagnostic>> {
    let mut bytes = 0usize;
    let mut add = |amount: usize| -> Result<(), Vec<Diagnostic>> {
        bytes = bytes.checked_add(amount).ok_or_else(limit_error)?;
        Ok(())
    };
    for agent in agents {
        add(std::mem::size_of_val(agent))?;
        for value in [
            agent.stable_id.as_str(),
            &agent.name,
            &agent.runtime_v1_json,
        ] {
            add(value.len())?;
        }
        for role in &agent.types {
            add(std::mem::size_of_val(role))?;
            add(role.stable_id.as_str().len())?;
        }
        for operation in &agent.operations {
            add(std::mem::size_of_val(operation))?;
            add(operation.stable_id.as_str().len())?;
        }
        if let Some(binding) = &agent.model_wait {
            add(std::mem::size_of_val(binding.as_ref()))?;
            add(binding.helper_id.as_str().len())?;
        }
    }
    reserve_builder_structure(bytes)
}

#[cfg(test)]
mod agent_execution_tests {
    use super::*;

    #[test]
    fn cloned_agent_carriers_charge_actual_fixed_and_optional_bytes_at_exact_limits() {
        let text = format!(
            "{}\n@id(\"helper\") fn wait(value:i64)->i64 {{value}}",
            crate::parser::agent_embedded_tests::source().replace(
                "runtime_v1",
                "model_wait_v1 { propose = \"helper\"; } runtime_v1"
            )
        );
        let ast = crate::check(&text, "agent-charge.spx").unwrap();
        let resolved = crate::hir::resolve(&ast).unwrap();
        let original = resolved.agents[0].clone();
        for with_helper in [false, true] {
            let mut agent = original.clone();
            if !with_helper {
                agent.model_wait = None;
            }
            let mut expected = std::mem::size_of_val(&agent)
                + agent.stable_id.as_str().len()
                + agent.name.len()
                + agent.runtime_v1_json.len();
            for role in &agent.types {
                expected += std::mem::size_of_val(role) + role.stable_id.as_str().len();
            }
            for operation in &agent.operations {
                expected += std::mem::size_of_val(operation) + operation.stable_id.as_str().len();
            }
            if with_helper {
                expected +=
                    std::mem::size_of::<hir::ResolvedAgentModelWaitBinding>() + "helper".len();
            }
            let (result, overflow, used) =
                crate::bounded_output::with_limit_usage(expected, || {
                    reserve_agent_execution_metadata(std::iter::once(&agent))
                });
            result.unwrap();
            assert!(!overflow);
            assert_eq!(used, expected);
            let (result, overflow, _) =
                crate::bounded_output::with_limit_usage(expected - 1, || {
                    reserve_agent_execution_metadata(std::iter::once(&agent))
                });
            assert_eq!(result.unwrap_err()[0].code, "SPX-G171");
            assert!(overflow);
        }
    }
}
