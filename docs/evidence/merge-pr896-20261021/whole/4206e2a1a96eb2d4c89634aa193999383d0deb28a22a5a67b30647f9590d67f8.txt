//! Final pre-bound for a core that does not retain staged synthetic ASTs.
use super::private_layout::registry::LayoutRegistry;
use std::collections::BTreeMap;

use crate::ast::Program;
use crate::diagnostic::Diagnostic;

use super::{
    active_builder_limit, checked_retention_prebound_with_uncached_peak_with_registry,
    checked_usage, cost, dependency_identity_max, limit_error,
    retention_prebound_mode_with_registry, synthetic_builder_bytes_scoped_with_registry,
    AuthoredDeclaration,
};

// Order temporary HIR overhead from largest to smallest. For two adjacent
// modules with retained sizes a,b and peaks A,B, A-a >= B-b makes resolving
// A first no worse than B first. A path tie-break makes this canonical.
// The fixed arrays allocate no heap. Compute this with the other admission
// forecasts, before entering the bounded construction phase.
pub(in crate::workspace_graph) struct UncachedOutputLayout {
    pub order: [usize; super::super::MAX_FILES],
    pub forecast: usize,
}

pub(in crate::workspace_graph) fn uncached_output_layout(
    programs: &[Program],
    authored: &BTreeMap<&str, AuthoredDeclaration<'_>>,
) -> Result<UncachedOutputLayout, Vec<Diagnostic>> {
    uncached_output_layout_with_registry(programs, authored, None)
}
pub(in crate::workspace_graph) fn uncached_output_layout_with_registry(
    programs: &[Program],
    authored: &BTreeMap<&str, AuthoredDeclaration<'_>>,
    registry: Option<&LayoutRegistry<'_>>,
) -> Result<UncachedOutputLayout, Vec<Diagnostic>> {
    #[cfg(test)]
    let mut observation = super::super::forecast_observation::begin_pass(6);
    let _precharge = super::super::diagnostics::precharge_scope();
    // This pass is the refusing bound whenever it refuses. Never let an
    // earlier summed fallback's partial module sum describe it.
    super::super::diagnostics::begin_precharge_pass();
    let mut order = [0; super::super::MAX_FILES];
    let mut full_costs = [0usize; super::super::MAX_FILES];
    let mut retained_costs = [0usize; super::super::MAX_FILES];
    for (index, program) in programs.iter().enumerate() {
        order[index] = index;
        #[cfg(test)]
        observation.module(index);
        super::super::diagnostics::begin_precharge_module(&program.path);
        let maximum = Some(dependency_identity_max(program, authored, programs)?);
        full_costs[index] = synthetic_builder_bytes_scoped_with_registry(
            program, authored, programs, maximum, 5, true, registry,
        )?
        .retained_hir;
        retained_costs[index] = synthetic_builder_bytes_scoped_with_registry(
            program, authored, programs, maximum, 5, false, registry,
        )?
        .retained_hir;
        super::super::diagnostics::record_precharge_module(&program.path, full_costs[index]);
    }
    order[..programs.len()].sort_unstable_by(|left, right| {
        let overhead = |index: usize| full_costs[index].saturating_sub(retained_costs[index]);
        overhead(*right)
            .cmp(&overhead(*left))
            .then_with(|| programs[*left].path.cmp(&programs[*right].path))
    });
    let mut retained = 0usize;
    let mut peak = 0usize;
    for (earlier, index) in order[..programs.len()].iter().enumerate() {
        let path = programs[*index].path.as_str();
        #[cfg(test)]
        let retained_before = retained;
        stage_sequential_refusal(path, earlier, retained, full_costs[*index], true);
        peak = peak.max(checked_usage(
            retained,
            full_costs[*index],
            "builder_bytes",
            active_builder_limit(),
        )?);
        stage_sequential_refusal(path, earlier, retained, retained_costs[*index], false);
        retained = checked_usage(
            retained,
            retained_costs[*index],
            "builder_bytes",
            active_builder_limit(),
        )?;
        #[cfg(test)]
        observation.step(
            path,
            *index,
            earlier,
            (retained_before, full_costs[*index], retained_costs[*index]),
            (peak, retained),
        );
    }
    super::super::diagnostics::complete_sequential_precharge(peak.max(retained));
    #[cfg(test)]
    observation.complete(peak.max(retained), retained, 0);
    Ok(UncachedOutputLayout {
        order,
        forecast: peak.max(retained),
    })
}

/// Name a sequential step under exactly the condition on which the following
/// `checked_usage` refuses it. The step itself is never altered or skipped.
fn stage_sequential_refusal(
    path: &str,
    earlier: usize,
    retained: usize,
    additional: usize,
    resolving: bool,
) {
    if retained
        .checked_add(additional)
        .is_none_or(|total| total > active_builder_limit())
    {
        super::super::diagnostics::record_sequential_precharge_refusal(
            path, earlier, retained, additional, resolving,
        );
    }
}

pub(in crate::workspace_graph) fn charge_uncached_synthetic_ast(
    program: &Program,
    authored: &BTreeMap<&str, AuthoredDeclaration<'_>>,
    programs: &[Program],
    retained_peak: &mut usize,
) -> Result<(), Vec<Diagnostic>> {
    charge_uncached_synthetic_ast_with_registry(program, authored, programs, retained_peak, None)
}
pub(in crate::workspace_graph) fn charge_uncached_synthetic_ast_with_registry(
    program: &Program,
    authored: &BTreeMap<&str, AuthoredDeclaration<'_>>,
    programs: &[Program],
    retained_peak: &mut usize,
    registry: Option<&LayoutRegistry<'_>>,
) -> Result<(), Vec<Diagnostic>> {
    let bytes = if super::borrowed_leaf::eligible(program) {
        super::borrowed_leaf::ast_peak(program)?
    } else {
        checked_usage(
            synthetic_builder_bytes_scoped_with_registry(
                program, authored, programs, None, 4, true, registry,
            )?
            .synthetic_ast,
            super::borrowed_leaf::input_carrier_delta()?,
            "builder_bytes",
            active_builder_limit(),
        )?
    };
    // The uncached path drops this AST before constructing the next one.
    // Preserve a monotonic peak reservation; never refund earlier charges.
    if bytes > *retained_peak {
        super::reserve_builder_structure(bytes - *retained_peak)?;
        *retained_peak = bytes;
    }
    Ok(())
}

// CORE_PEAK_BEGIN
pub(in crate::workspace_graph) fn charge_uncached_synthetic_ast_core<'a>(
    program: &Program,
    authored: &'a BTreeMap<&'a str, AuthoredDeclaration<'a>>,
    programs: &'a [Program],
    retained_peak: &mut usize,
    registry: Option<&LayoutRegistry<'_>>,
    core: &mut super::CoreProviderProofs<'a>,
) -> Result<(), Vec<Diagnostic>> {
    let bytes = if super::borrowed_leaf::eligible(program) {
        super::borrowed_leaf::ast_peak(program)?
    } else {
        checked_usage(
            super::held_projection::synthetic_builder_bytes_core(
                program,
                authored,
                programs,
                None,
                true,
                (4, super::cached_exact::Retention::Parsed),
                (registry, core),
            )?
            .synthetic_ast,
            super::borrowed_leaf::input_carrier_delta()?,
            "builder_bytes",
            active_builder_limit(),
        )?
    };
    // The uncached path drops this AST before constructing the next one.
    // Preserve a monotonic peak reservation; never refund earlier charges.
    if bytes > *retained_peak {
        super::reserve_builder_structure(bytes - *retained_peak)?;
        *retained_peak = bytes;
    }
    Ok(())
}
// CORE_PEAK_END

/// Mode six retains a filtered module and compact cross-module proof after
/// each resolution.  Its peak is the already-retained output/proof plus one
/// complete synthetic AST/HIR, never every complete resolved program.
pub(in crate::workspace_graph) fn uncached_output_peak_prebound(
    programs: &[Program],
    authored: &BTreeMap<&str, AuthoredDeclaration<'_>>,
) -> Result<(usize, usize), Vec<Diagnostic>> {
    uncached_output_peak_prebound_with_registry(programs, authored, None)
}
pub(in crate::workspace_graph) fn uncached_output_peak_prebound_with_registry(
    programs: &[Program],
    authored: &BTreeMap<&str, AuthoredDeclaration<'_>>,
    registry: Option<&LayoutRegistry<'_>>,
) -> Result<(usize, usize), Vec<Diagnostic>> {
    let layout = uncached_output_layout_with_registry(programs, authored, registry)?;
    Ok((layout.forecast, layout.forecast))
}

/// Select the only receipt a core without a retained frontend may use.
pub(in crate::workspace_graph) fn initial_core_prebound(
    programs: &[Program],
    authored: &BTreeMap<&str, AuthoredDeclaration<'_>>,
    frontend_is_absent: bool,
) -> Result<(usize, usize, bool, u8), Vec<Diagnostic>> {
    initial_core_prebound_with_registry(programs, authored, frontend_is_absent, None)
}
pub(in crate::workspace_graph) fn initial_core_prebound_with_registry(
    programs: &[Program],
    authored: &BTreeMap<&str, AuthoredDeclaration<'_>>,
    frontend_is_absent: bool,
    registry: Option<&LayoutRegistry<'_>>,
) -> Result<(usize, usize, bool, u8), Vec<Diagnostic>> {
    let _precharge = super::super::diagnostics::precharge_scope();
    let (receipt, initial_mode) = match checked_retention_prebound_with_uncached_peak_with_registry(
        programs,
        authored,
        frontend_is_absent,
        registry,
    ) {
        Ok(receipt) => (receipt, 1),
        Err(errors) if frontend_is_absent && cost::is_builder_refusal(&errors) => (
            uncached_output_peak_prebound_with_registry(programs, authored, registry)?,
            6,
        ),
        Err(errors) => return Err(errors),
    };
    Ok((receipt.0, receipt.1, frontend_is_absent, initial_mode))
}

// The builder limit scopes the `with_limit_usage` core attempt, after
// `build_owned_inner` has parsed and retained the authored `programs`; that
// pre-existing AST is deliberately outside this phase's receipt. This receipt
// starts at `build_resolved_core`: every uncached synthetic AST is dropped
// after its one HIR resolution, while the resolved HIR remains in
// `synthetic_modules` until filtering completes.
pub(super) fn uncached_peak_prebound(
    programs: &[Program],
    authored: &BTreeMap<&str, AuthoredDeclaration<'_>>,
) -> Result<(usize, usize), Vec<Diagnostic>> {
    uncached_peak_prebound_with_registry(programs, authored, None)
}
pub(in crate::workspace_graph) fn uncached_peak_prebound_with_registry(
    programs: &[Program],
    authored: &BTreeMap<&str, AuthoredDeclaration<'_>>,
    registry: Option<&LayoutRegistry<'_>>,
) -> Result<(usize, usize), Vec<Diagnostic>> {
    #[cfg(test)]
    let mut observation = super::super::forecast_observation::begin_pass(5);
    let _precharge = super::super::diagnostics::precharge_scope();
    super::super::diagnostics::begin_precharge_pass();
    let mut retained_hir = 0usize;
    let mut synthetic_ast_peak = 0usize;
    for program in programs {
        super::super::diagnostics::begin_precharge_module(&program.path);
        let maximum = Some(dependency_identity_max(program, authored, programs)?);
        let costs = synthetic_builder_bytes_scoped_with_registry(
            program, authored, programs, maximum, 4, true, registry,
        )?;
        super::super::diagnostics::record_precharge_module(&program.path, costs.retained_hir);
        retained_hir = checked_usage(
            retained_hir,
            costs.retained_hir,
            "builder_bytes",
            active_builder_limit(),
        )?;
        synthetic_ast_peak = synthetic_ast_peak.max(if super::borrowed_leaf::eligible(program) {
            super::borrowed_leaf::ast_peak(program)?
        } else {
            checked_usage(
                costs.synthetic_ast,
                super::borrowed_leaf::input_carrier_delta()?,
                "builder_bytes",
                active_builder_limit(),
            )?
        });
    }
    let total = checked_usage(
        retained_hir,
        synthetic_ast_peak,
        "builder_bytes",
        active_builder_limit(),
    )?;
    #[cfg(test)]
    observation.complete(total, retained_hir, synthetic_ast_peak);
    Ok((total, total))
}

/// Mode five is an execution-phase peak, never a checked-cache receipt.
/// Callers may select it only when no frontend can retain synthetic programs.
pub(in crate::workspace_graph) fn next_retention_prebound_with_uncached_peak(
    programs: &[Program],
    authored: &BTreeMap<&str, AuthoredDeclaration<'_>>,
    current: usize,
    layout_mode: &mut u8,
    allow_uncached_peak: bool,
) -> Result<(usize, usize), Vec<Diagnostic>> {
    next_retention_prebound_with_uncached_peak_with_registry(
        programs,
        authored,
        current,
        layout_mode,
        allow_uncached_peak,
        None,
    )
}
pub(in crate::workspace_graph) fn next_retention_prebound_with_uncached_peak_with_registry(
    programs: &[Program],
    authored: &BTreeMap<&str, AuthoredDeclaration<'_>>,
    current: usize,
    layout_mode: &mut u8,
    allow_uncached_peak: bool,
    registry: Option<&LayoutRegistry<'_>>,
) -> Result<(usize, usize), Vec<Diagnostic>> {
    let maximum_mode = if allow_uncached_peak { 6 } else { 4 };
    while *layout_mode < maximum_mode {
        *layout_mode += 1;
        if *layout_mode == 5 {
            match uncached_peak_prebound_with_registry(programs, authored, registry) {
                Ok((resolve, total)) if resolve < current => return Ok((resolve, total)),
                Ok(_) => continue,
                Err(errors) if cost::is_builder_refusal(&errors) => continue,
                Err(errors) => return Err(errors),
            }
        }
        if *layout_mode == 6 {
            match uncached_output_peak_prebound_with_registry(programs, authored, registry) {
                Ok((resolve, total)) => return Ok((resolve, total)),
                Err(errors) if cost::is_builder_refusal(&errors) => continue,
                Err(errors) => return Err(errors),
            }
        }
        match retention_prebound_mode_with_registry(
            programs,
            authored,
            true,
            *layout_mode,
            registry,
        ) {
            Ok((resolve, total)) if resolve < current => return Ok((resolve, total)),
            Ok(_) => continue,
            Err(errors) if cost::is_builder_refusal(&errors) => continue,
            Err(errors) => return Err(errors),
        }
    }
    Err(vec![limit_error("builder_bytes", active_builder_limit())])
}
