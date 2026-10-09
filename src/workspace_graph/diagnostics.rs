//! Shared bounded workspace-graph diagnostic constructors.

use std::cell::{Cell, RefCell};

use crate::ast::{ModuleUse, Program, Span};
use crate::diagnostic::Diagnostic;

use super::{
    WorkspaceResolvedModule, MAX_BUILDER_BYTES, MAX_CALLABLES, MAX_CALLS, MAX_CROSS_FILE_EDGES,
    MAX_DECLARATIONS, MAX_DEPENDENCY_DEPTH, MAX_ENTRY_MODULE_BYTES, MAX_FILES, MAX_OUTPUT_BYTES,
    MAX_TOTAL_SOURCE_BYTES, MAX_USES,
};

pub(super) const PROVIDER_MAIN_HELP: &str = "`entry` in semaprax.toml must name the module that declares `main`; every other listed source is a provider module and declares no `main`";
pub(super) const PROJECT_SIGNATURE_HELP: &str = "Project v1 function boundaries admit only Copy scalar values; keep records, classes, variants, Option, and Result inside functions, or select a project profile that explicitly admits the required public carrier";

/// Canonical shared wire object for every artifact that embeds Workspace
/// Semantic Graph limits. The values derive from the limits enforced by the
/// graph builder, rather than being separately restated by each consumer.
pub(crate) fn push_workspace_graph_limits(output: &mut crate::bounded_output::CappedString) {
    use std::fmt::Write as _;

    write!(
        output,
        "{{\"max_managed_files\":{MAX_FILES},\"max_reachable_modules\":{MAX_FILES},\"max_entry_module_bytes\":{MAX_ENTRY_MODULE_BYTES},\"max_total_source_bytes\":{MAX_TOTAL_SOURCE_BYTES},\"max_declarations\":{MAX_DECLARATIONS},\"max_callables\":{MAX_CALLABLES},\"max_call_sites\":{MAX_CALLS},\"max_uses\":{MAX_USES},\"max_resolved_cross_file_edges\":{MAX_CROSS_FILE_EDGES},\"max_dependency_depth\":{MAX_DEPENDENCY_DEPTH},\"max_builder_bytes\":{MAX_BUILDER_BYTES},\"max_manifest_bytes\":1048576,\"max_output_bytes\":{MAX_OUTPUT_BYTES},\"max_retained_generations\":32,\"max_staging_attempts\":32,\"max_unexpected_inventory_entries\":0}}"
    )
    .expect("writing to a string cannot fail");
}

pub(super) fn use_error(program: &Program, module_use: &ModuleUse, message: &str) -> Diagnostic {
    Diagnostic::error("SPX-G172", message, module_use.span).at_path(&program.path)
}

/// Advisory lookup from the already validated provider inventory; no guessed
/// replacement, source read, or repair authority. At most 128 bytes of each
/// label are escaped, beside a portable provider path of at most 240 bytes.
pub(super) fn unknown_import_error(
    program: &Program,
    module_use: &ModuleUse,
    modules: &std::collections::BTreeMap<&str, &str>,
) -> Diagnostic {
    fn preview(value: &str) -> (&str, &str) {
        let mut end = value.len().min(128);
        while !value.is_char_boundary(end) {
            end -= 1;
        }
        (&value[..end], if end == value.len() { "" } else { "…" })
    }
    let diagnostic = use_error(program, module_use, "persistent target identity is unknown");
    let Some(provider_path) = modules.get(module_use.target_module.as_str()) else {
        return diagnostic;
    };
    let (identity, identity_suffix) = preview(&module_use.persistent_id);
    let (module, module_suffix) = preview(&module_use.target_module);
    // Inventory paths contain only portable alphanumeric/._-/ bytes and
    // cannot start with an option prefix. Single quotes keep the query operand
    // literal; debug-escaped labels cannot inject additional help lines.
    diagnostic.with_help(crate::bounded_output::budgeted_format(format_args!(
        "imported identity {identity:?}{identity_suffix} was not found in provider module {module:?}{module_suffix}; run `semaprax query '{provider_path}'` to list its declarations, then use the exact explicit @id and declaration kind"
    )))
}

pub(super) fn graph_error(code: &'static str, message: impl Into<String>) -> Diagnostic {
    Diagnostic::io(code, message)
}

pub(super) fn project_function_error(
    module: &WorkspaceResolvedModule,
    message: impl Into<String>,
    span: Option<Span>,
) -> Diagnostic {
    Diagnostic::error(
        "SPX-G172",
        message,
        span.unwrap_or(Span {
            start: 0,
            end: 0,
            line: 1,
            column: 1,
        }),
    )
    .at_path(&module.path)
}

pub(super) fn limit_error(field: &'static str, maximum: usize) -> Diagnostic {
    let error = graph_error(
        "SPX-G171",
        crate::bounded_output::budgeted_format(format_args!(
            "Workspace Semantic Graph `{field}` exceeds {maximum}"
        )),
    );
    if field != "builder_bytes" {
        return error;
    }
    let help = builder_bytes_help(maximum);
    if help.is_empty() {
        return error;
    }
    error.with_help(help)
}

/// Which of the two `builder_bytes` bounds refused.
///
/// The static admission pre-charge forecasts the whole build before the graph
/// builder allocates anything; live accumulation charges the structures the
/// builder really allocates. Both refuse with `SPX-G171` and the same budget,
/// so without this the diagnostic cannot tell an author whether factoring the
/// source differently could possibly help.
#[derive(Clone, Copy, PartialEq, Eq)]
enum BuilderPhase {
    Precharge,
    Live,
}

thread_local! {
    static BUILDER_PHASE: Cell<BuilderPhase> = const { Cell::new(BuilderPhase::Live) };
    static PRECHARGE_MODULES: Cell<usize> = const { Cell::new(0) };
    static PRECHARGE_BYTES: Cell<usize> = const { Cell::new(0) };
    static PRECHARGE_DOMINANT: RefCell<Option<(String, usize)>> = const { RefCell::new(None) };
    static PRECHARGE_PENDING: RefCell<Option<String>> = const { RefCell::new(None) };
}

/// Restores the previous phase on drop, so a pre-charge nested inside another
/// one cannot leave live accumulation mislabelled.
pub(super) struct PrechargeScope(BuilderPhase);

impl Drop for PrechargeScope {
    fn drop(&mut self) {
        BUILDER_PHASE.with(|phase| phase.set(self.0));
    }
}

pub(super) fn precharge_scope() -> PrechargeScope {
    PrechargeScope(BUILDER_PHASE.with(|phase| phase.replace(BuilderPhase::Precharge)))
}

/// Start one forecast pass. Every fallback receipt re-costs each module, so
/// attribution keeps only the pass that is currently running.
pub(super) fn begin_precharge_pass() {
    PRECHARGE_MODULES.with(|modules| modules.set(0));
    PRECHARGE_BYTES.with(|bytes| bytes.set(0));
    PRECHARGE_DOMINANT.with(|dominant| *dominant.borrow_mut() = None);
    PRECHARGE_PENDING.with(|pending| *pending.borrow_mut() = None);
}

/// Name the module whose own forecast is being computed. One module's
/// forecast can exceed the budget on its own, which is a different author
/// remedy from a sum over modules that each fit.
pub(super) fn begin_precharge_module(path: &str) {
    PRECHARGE_PENDING.with(|pending| *pending.borrow_mut() = Some(path.to_owned()));
}

/// Record one module's forecast. Modules are costed in canonical path order
/// and only a strictly larger forecast replaces the dominant module, so the
/// retained attribution is deterministic.
pub(super) fn record_precharge_module(path: &str, bytes: usize) {
    PRECHARGE_PENDING.with(|pending| *pending.borrow_mut() = None);
    PRECHARGE_MODULES.with(|modules| modules.set(modules.get().saturating_add(1)));
    PRECHARGE_BYTES.with(|total| total.set(total.get().saturating_add(bytes)));
    PRECHARGE_DOMINANT.with(|dominant| {
        let mut dominant = dominant.borrow_mut();
        if dominant.as_ref().is_none_or(|(_, seen)| bytes > *seen) {
            *dominant = Some((path.to_owned(), bytes));
        }
    });
}

/// Name the bound that refused and the module that drove it. The message
/// itself stays byte-identical, because nested workspace routes remap a
/// `builder_bytes` refusal by comparing it exactly.
fn builder_bytes_help(maximum: usize) -> String {
    let precharge = BUILDER_PHASE.with(Cell::get) == BuilderPhase::Precharge;
    let modules = PRECHARGE_MODULES.with(Cell::get);
    let forecast = PRECHARGE_BYTES.with(Cell::get);
    let dominant = PRECHARGE_DOMINANT.with(|dominant| dominant.borrow().clone());
    let pending = PRECHARGE_PENDING.with(|pending| pending.borrow().clone());
    if let (true, Some(path)) = (precharge, pending) {
        return crate::bounded_output::budgeted_format(format_args!(
            "the static admission pre-charge refused before the graph builder ran, while forecasting module `{path}` on its own: that one module does not fit the {maximum}-byte budget, which already carries {forecast} bytes for {modules} earlier module(s). Splitting `{path}` into several modules cannot reduce the forecast, because the pre-charge sums every reachable module; reducing its admitted source can."
        ));
    }
    match (precharge, dominant) {
        (true, Some((path, bytes))) => crate::bounded_output::budgeted_format(format_args!(
            "the static admission pre-charge refused before the graph builder ran: forecasting {modules} reachable module(s) already reaches {forecast} bytes of the {maximum}-byte budget, and `{path}` alone forecasts {bytes} bytes of retained resolver state. The pre-charge sums every reachable module, so splitting one module into several cannot reduce it; removing a dependency or reducing total reachable source can."
        )),
        (true, None) => crate::bounded_output::budgeted_format(format_args!(
            "the static admission pre-charge refused before the graph builder ran, and before any single module was fully costed against the {maximum}-byte budget."
        )),
        (false, Some((path, bytes))) => crate::bounded_output::budgeted_format(format_args!(
            "live graph construction, not the static admission pre-charge, exceeded the budget: the pre-charge forecast {forecast} bytes for {modules} module(s) and fit within {maximum}, with `{path}` the largest at {bytes} bytes. Reducing reachable source is the only lever; the forecast itself was not the refusing bound."
        )),
        (false, None) => crate::bounded_output::budgeted_format(format_args!(
            "live graph construction, not the static admission pre-charge, exceeded the {maximum}-byte budget while allocating workspace graph structure."
        )),
    }
}

/// Evidence belongs to the final resolved-core attempt, captured before its
/// budget is restored. Neither earlier retries nor diagnostic rendering replace it.
pub(super) fn live_builder_refusal(
    maximum: usize,
    refusal: Option<crate::bounded_output::ReservationRefusal>,
) -> Diagnostic {
    let diagnostic = limit_error("builder_bytes", maximum);
    let evidence = match refusal {
        Some(refusal) => crate::bounded_output::budgeted_format(format_args!(
            "resolved-core phase first sticky ledger refusal: requested {} bytes with {} remaining and {} reserved floor. These are cumulative reservation bytes, not a retained-memory forecast or process RSS; the exact inner operation is unknown.",
            refusal.requested, refusal.remaining, refusal.floor,
        )),
        None => crate::bounded_output::budgeted_format(format_args!(
            "resolved-core phase refused; the first reservation and exact inner operation are unknown."
        )),
    };
    if evidence.is_empty() {
        return diagnostic;
    }
    let help = match diagnostic.help.as_deref() {
        Some(help) => crate::bounded_output::budgeted_format(format_args!("{help} {evidence}")),
        None => evidence,
    };
    if help.is_empty() {
        diagnostic
    } else {
        diagnostic.with_help(help)
    }
}

#[cfg(test)]
mod refusal_tests;
