//! Unit-test-only C control-flow scaffold for the first native cleanup slice.
//!
//! The production resource gate remains closed. This emitter consumes a
//! [`NativeCleanupIndex`] and explicit C identifier bindings; it never derives
//! cleanup from HIR or repairs the attached plan. Missing observations or
//! storage names fail closed with `SPX-B104`.
//! Canonical checked-success continuations are revalidated independently and
//! may only leave empty regions without changing ownership.

#![cfg_attr(
    not(test),
    allow(
        dead_code,
        reason = "the plan scaffold remains unreachable until native conformance is complete"
    )
)]

use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;

use crate::cleanup::LivenessFlagId;
use crate::cleanup_plan::{
    BlockId, CleanupPlace, CleanupResultSource, CleanupTerminator, CleanupTransition,
    EdgeCondition, ExitContinuation, StatusSourceId, StorageId,
};
use crate::diagnostic::Diagnostic;
use crate::hir::ExpressionId;

use super::native_cleanup::{NativeCleanupIndex, NativeCleanupLeaf};

/// Physical C identifiers supplied by the value/status emitter.
///
/// Every value is restricted to one C identifier. This scaffold deliberately
/// does not accept arbitrary snippets that could hide evaluation or ownership
/// changes inside an expression. The surrounding value emitter must allocate
/// every name inside the dedicated `spx_bind_` namespace; arbitrary C/runtime
/// identifiers and object-like macros cannot cross this boundary.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub(crate) struct NativeCleanupBindings {
    pub(crate) storage_values: BTreeMap<StorageId, String>,
    pub(crate) boolean_values: BTreeMap<ExpressionId, String>,
    pub(crate) status_tokens: BTreeMap<StatusSourceId, String>,
    pub(crate) scalar_results: BTreeMap<ExpressionId, String>,
    pub(crate) result_out: Option<String>,
}

/// Emit one deterministic function-body fragment from an already-classified
/// cleanup plan.
///
/// The fragment assumes storage values, boolean observations, status tokens,
/// `spx_status_token`, `SPX_STATUS_SUCCESS`, and
/// `spx_runtime_invariant_failure` are declared by the surrounding emitter.
pub(crate) fn emit(
    index: &NativeCleanupIndex<'_>,
    bindings: &NativeCleanupBindings,
) -> Result<String, Diagnostic> {
    validate_bindings(index, bindings)?;

    let mut output = String::from("/* semaprax.native-cleanup-scaffold.v1 */\n");
    for leaf in &index.leaves {
        writeln!(output, "bool {} = false;", flag_symbol(leaf.flag))
            .expect("writing to a string cannot fail");
    }
    output.push_str("spx_status_token spx_cleanup_selected_status = SPX_STATUS_SUCCESS;\n");
    for place in index.live_owned_parameters {
        let leaf = leaf_for_place(index, place)?;
        writeln!(output, "{} = true;", flag_symbol(leaf.flag))
            .expect("writing to a string cannot fail");
    }
    writeln!(output, "goto {};", block_label(index.entry))
        .expect("writing to a string cannot fail");

    for indexed in &index.blocks {
        writeln!(output, "{}:", block_label(indexed.block.id))
            .expect("writing to a string cannot fail");
        for transition in indexed.transitions {
            emit_transition(&mut output, index, bindings, transition)?;
        }
        emit_terminator(
            &mut output,
            index,
            bindings,
            indexed.block.id,
            &indexed.block.terminator,
        )?;
    }

    for indexed in &index.exits {
        writeln!(output, "{}:", exit_label(indexed.exit.id.0))
            .expect("writing to a string cannot fail");
        for action in indexed.finalizers {
            let leaf = index.leaf(action.guard_flag).ok_or_else(|| {
                cleanup_error(format!(
                    "exit {} references unknown finalizer flag {}",
                    indexed.exit.id.0, action.guard_flag.0
                ))
            })?;
            if leaf.place != action.source || leaf.lifecycle_id != &action.lifecycle_id {
                return Err(cleanup_error(format!(
                    "exit {} finalizer flag {} disagrees with its classified leaf",
                    indexed.exit.id.0, action.guard_flag.0
                )));
            }
            let flag = flag_symbol(action.guard_flag);
            writeln!(output, "if ({flag}) {{").expect("writing to a string cannot fail");
            writeln!(output, "    {flag} = false;").expect("writing to a string cannot fail");
            writeln!(
                output,
                "    (void)\"semaprax.cleanup.trivial-finalizer:{}\";",
                c_string(action.lifecycle_id.as_str())
            )
            .expect("writing to a string cannot fail");
            output.push_str("}\n");
        }
        emit_continuation(&mut output, index, bindings, indexed.exit)?;
    }

    Ok(output)
}

fn emit_transition(
    output: &mut String,
    index: &NativeCleanupIndex<'_>,
    bindings: &NativeCleanupBindings,
    transition: &CleanupTransition,
) -> Result<(), Diagnostic> {
    match transition {
        CleanupTransition::Initialize { at, .. } => {
            return Err(cleanup_error(format!(
                "initialize transition `{at}` has no physical payload source in the first native cleanup slice"
            )));
        }
        CleanupTransition::Transfer {
            at,
            source,
            destination,
        } => {
            let source_leaf = leaf_for_place(index, source)?;
            let destination_leaf = leaf_for_place(index, destination)?;
            if source_leaf.lifecycle_id != destination_leaf.lifecycle_id {
                return Err(cleanup_error(format!(
                    "transfer `{at}` changes the classified lifecycle"
                )));
            }
            let source_flag = flag_symbol(source_leaf.flag);
            let destination_flag = flag_symbol(destination_leaf.flag);
            let source_value = storage_binding(bindings, &source.storage)?;
            let destination_value = storage_binding(bindings, &destination.storage)?;
            writeln!(
                output,
                "if (!{source_flag} || {destination_flag}) spx_runtime_invariant_failure(\"cleanup transfer liveness\");"
            )
            .expect("writing to a string cannot fail");
            writeln!(output, "{destination_value} = {source_value};")
                .expect("writing to a string cannot fail");
            writeln!(output, "{source_flag} = false;").expect("writing to a string cannot fail");
            writeln!(output, "{destination_flag} = true;")
                .expect("writing to a string cannot fail");
            writeln!(
                output,
                "(void)\"semaprax.cleanup.transfer:{}\";",
                c_string(at.as_str())
            )
            .expect("writing to a string cannot fail");
        }
        CleanupTransition::CallCommit { call, .. } => {
            return Err(cleanup_error(format!(
                "call-commit transition `{call}` reached the single-frame cleanup scaffold"
            )));
        }
        CleanupTransition::SelectFailure { source } => {
            let status = status_binding(bindings, source)?;
            output.push_str(
                "if (spx_cleanup_selected_status != SPX_STATUS_SUCCESS) \
                 spx_runtime_invariant_failure(\"cleanup failure selection is not write-once\");\n",
            );
            writeln!(
                output,
                "if ({status} == SPX_STATUS_SUCCESS) spx_runtime_invariant_failure(\"cleanup selected a successful status\");"
            )
            .expect("writing to a string cannot fail");
            writeln!(output, "spx_cleanup_selected_status = {status};")
                .expect("writing to a string cannot fail");
        }
    }
    Ok(())
}

fn emit_terminator(
    output: &mut String,
    index: &NativeCleanupIndex<'_>,
    bindings: &NativeCleanupBindings,
    owner: BlockId,
    terminator: &CleanupTerminator,
) -> Result<(), Diagnostic> {
    match terminator {
        CleanupTerminator::Goto(edge) => {
            let edge = index.edge(*edge).ok_or_else(|| {
                cleanup_error(format!(
                    "block {} references unknown edge {}",
                    owner.0, edge.0
                ))
            })?;
            if edge.from != owner || !matches!(edge.condition, EdgeCondition::Always) {
                return Err(cleanup_error(format!(
                    "block {} has a noncanonical goto edge {}",
                    owner.0, edge.id.0
                )));
            }
            writeln!(output, "goto {};", block_label(edge.to))
                .expect("writing to a string cannot fail");
        }
        CleanupTerminator::Branch(edges) => {
            if edges.is_empty() {
                return Err(cleanup_error(format!(
                    "block {} has an empty cleanup branch",
                    owner.0
                )));
            }
            for (position, edge_id) in edges.iter().enumerate() {
                let edge = index.edge(*edge_id).ok_or_else(|| {
                    cleanup_error(format!(
                        "block {} references unknown edge {}",
                        owner.0, edge_id.0
                    ))
                })?;
                if edge.from != owner {
                    return Err(cleanup_error(format!(
                        "edge {} is not owned by block {}",
                        edge.id.0, owner.0
                    )));
                }
                let condition = edge_condition(bindings, &edge.condition)?;
                let keyword = if position == 0 { "if" } else { "else if" };
                writeln!(
                    output,
                    "{keyword} ({condition}) goto {};",
                    block_label(edge.to)
                )
                .expect("writing to a string cannot fail");
            }
            output.push_str(
                "else spx_runtime_invariant_failure(\"cleanup branch selected no edge\");\n",
            );
        }
        CleanupTerminator::Exit(exit) => {
            let indexed = index.exit(*exit).ok_or_else(|| {
                cleanup_error(format!(
                    "block {} references unknown exit {}",
                    owner.0, exit.0
                ))
            })?;
            if indexed.exit.from != owner {
                return Err(cleanup_error(format!(
                    "exit {} is not owned by block {}",
                    exit.0, owner.0
                )));
            }
            writeln!(output, "goto {};", exit_label(exit.0))
                .expect("writing to a string cannot fail");
        }
    }
    Ok(())
}

fn emit_continuation(
    output: &mut String,
    index: &NativeCleanupIndex<'_>,
    bindings: &NativeCleanupBindings,
    exit: &crate::cleanup_plan::ExitTarget,
) -> Result<(), Diagnostic> {
    match &exit.continuation {
        ExitContinuation::Continue(edge) => {
            let edge = bounded_continue_edge(index, exit, *edge)?;
            writeln!(output, "goto {};", block_label(edge.to))
                .expect("writing to a string cannot fail");
        }
        ExitContinuation::CommitResult { source } => {
            let result_out = result_out(bindings)?;
            output.push_str(
                "if (spx_cleanup_selected_status != SPX_STATUS_SUCCESS) \
                 spx_runtime_invariant_failure(\"cleanup result commit selected failure\");\n",
            );
            match source {
                CleanupResultSource::Scalar { expression } => {
                    emit_assert_all_dead(
                        output,
                        index,
                        "cleanup scalar result commit retains a live resource",
                    );
                    let value = scalar_binding(bindings, expression)?;
                    writeln!(output, "*{result_out} = {value};")
                        .expect("writing to a string cannot fail");
                }
                CleanupResultSource::Owned { storage } => {
                    if storage.storage != StorageId::ProvisionalResult {
                        return Err(cleanup_error(format!(
                            "exit {} publishes owned non-provisional storage",
                            exit.id.0
                        )));
                    }
                    let leaf = leaf_for_place(index, storage)?;
                    let flag = flag_symbol(leaf.flag);
                    let value = storage_binding(bindings, &storage.storage)?;
                    emit_assert_only_leaf_live(output, index, leaf.flag);
                    writeln!(output, "*{result_out} = {value};")
                        .expect("writing to a string cannot fail");
                    writeln!(output, "{flag} = false;").expect("writing to a string cannot fail");
                }
            }
            output.push_str("return SPX_STATUS_SUCCESS;\n");
        }
        ExitContinuation::ReturnFailure { source } => {
            let status = status_binding(bindings, source)?;
            writeln!(
                output,
                "if (spx_cleanup_selected_status == SPX_STATUS_SUCCESS || spx_cleanup_selected_status != {status}) spx_runtime_invariant_failure(\"cleanup failure return changed status\");"
            )
            .expect("writing to a string cannot fail");
            emit_assert_all_dead(
                output,
                index,
                "cleanup failure return retains a live resource",
            );
            output.push_str("return spx_cleanup_selected_status;\n");
        }
        ExitContinuation::ReturnUnit => {
            return Err(cleanup_error(format!(
                "exit {} uses unsupported unit return",
                exit.id.0
            )));
        }
    }
    Ok(())
}

fn bounded_continue_edge<'a>(
    index: &'a NativeCleanupIndex<'a>,
    exit: &crate::cleanup_plan::ExitTarget,
    edge_id: crate::cleanup_plan::EdgeId,
) -> Result<&'a crate::cleanup_plan::CleanupEdge, Diagnostic> {
    let reject = |detail: &str| {
        cleanup_error(format!(
            "exit {} continuation {detail}; only the canonical empty-region success continuation is supported",
            exit.id.0
        ))
    };
    if !exit.finalize_in_order.is_empty() || exit.leaves_regions.is_empty() {
        return Err(reject("performs cleanup or leaves no region"));
    }
    let source = index
        .block(exit.from)
        .ok_or_else(|| reject("has an unknown source block"))?;
    if !source.transitions.is_empty() || source.block.terminator != CleanupTerminator::Exit(exit.id)
    {
        return Err(reject("changes state before continuing"));
    }
    let edge = index
        .edge(edge_id)
        .ok_or_else(|| reject("references an unknown edge"))?;
    if edge.from != exit.from || !matches!(edge.condition, EdgeCondition::Always) {
        return Err(reject("does not own one unconditional edge"));
    }
    let incoming = index
        .edges
        .iter()
        .filter(|candidate| candidate.to == source.block.id)
        .collect::<Vec<_>>();
    if incoming.len() != 1
        || !matches!(
            incoming[0].condition,
            EdgeCondition::BooleanResult(_, true) | EdgeCondition::StatusZero(_)
        )
    {
        return Err(reject("is not reached by one successful checked branch"));
    }

    let mut expected_region = Some(source.block.region);
    for region_id in &exit.leaves_regions {
        if expected_region != Some(*region_id) {
            return Err(reject("does not leave one contiguous region chain"));
        }
        let region = index
            .regions
            .iter()
            .find(|region| region.id == *region_id)
            .ok_or_else(|| reject("references an unknown region"))?;
        if !region.slots.is_empty() || region.normal_scope_end != exit.id {
            return Err(reject("leaves a resource-owning or non-normal region"));
        }
        expected_region = region.parent;
    }
    let Some(parent_region) = expected_region else {
        return Err(reject("escapes the root region"));
    };
    let target = index
        .block(edge.to)
        .ok_or_else(|| reject("targets an unknown block"))?;
    if target.block.region != parent_region
        || index
            .edges
            .iter()
            .filter(|candidate| candidate.to == target.block.id)
            .count()
            != 1
        || index
            .exits
            .iter()
            .filter(|candidate| {
                matches!(candidate.exit.continuation, ExitContinuation::Continue(id) if id == edge_id)
            })
            .count()
            != 1
    {
        return Err(reject("does not have one target in the surviving region"));
    }
    Ok(edge)
}

fn emit_assert_all_dead(output: &mut String, index: &NativeCleanupIndex<'_>, message: &str) {
    for leaf in &index.leaves {
        writeln!(
            output,
            "if ({}) spx_runtime_invariant_failure(\"{message}\");",
            flag_symbol(leaf.flag)
        )
        .expect("writing to a string cannot fail");
    }
}

fn emit_assert_only_leaf_live(
    output: &mut String,
    index: &NativeCleanupIndex<'_>,
    live_flag: LivenessFlagId,
) {
    for leaf in &index.leaves {
        let flag = flag_symbol(leaf.flag);
        if leaf.flag == live_flag {
            writeln!(
                output,
                "if (!{flag}) spx_runtime_invariant_failure(\"cleanup publishes a dead owned result\");"
            )
            .expect("writing to a string cannot fail");
        } else {
            writeln!(
                output,
                "if ({flag}) spx_runtime_invariant_failure(\"cleanup owned result commit retains another live resource\");"
            )
            .expect("writing to a string cannot fail");
        }
    }
}

fn edge_condition(
    bindings: &NativeCleanupBindings,
    condition: &EdgeCondition,
) -> Result<String, Diagnostic> {
    match condition {
        EdgeCondition::Always => Err(cleanup_error(
            "unconditional edge reached cleanup branch emission",
        )),
        EdgeCondition::BooleanResult(expression, expected) => {
            let value = bindings.boolean_values.get(expression).ok_or_else(|| {
                cleanup_error(format!(
                    "missing boolean binding for expression `{expression}`"
                ))
            })?;
            Ok(if *expected {
                value.clone()
            } else {
                format!("!{value}")
            })
        }
        EdgeCondition::StatusZero(source) => Ok(format!(
            "{} == SPX_STATUS_SUCCESS",
            status_binding(bindings, source)?
        )),
        EdgeCondition::StatusNonzero(source) => Ok(format!(
            "{} != SPX_STATUS_SUCCESS",
            status_binding(bindings, source)?
        )),
    }
}

fn validate_bindings(
    index: &NativeCleanupIndex<'_>,
    bindings: &NativeCleanupBindings,
) -> Result<(), Diagnostic> {
    let expected_storage = index
        .slots
        .iter()
        .map(|slot| slot.slot.storage.clone())
        .collect::<BTreeSet<_>>();
    require_exact_keys(
        &expected_storage,
        bindings.storage_values.keys().cloned().collect(),
        "storage",
    )?;

    let mut expected_booleans = BTreeSet::new();
    let mut expected_statuses = BTreeSet::new();
    for edge in index.edges {
        match &edge.condition {
            EdgeCondition::BooleanResult(expression, _) => {
                expected_booleans.insert(expression.clone());
            }
            EdgeCondition::StatusZero(source) | EdgeCondition::StatusNonzero(source) => {
                expected_statuses.insert(source.clone());
            }
            EdgeCondition::Always => {}
        }
    }
    let mut expected_scalars = BTreeSet::new();
    let mut publishes_result = false;
    for block in &index.blocks {
        for transition in block.transitions {
            match transition {
                CleanupTransition::SelectFailure { source } => {
                    expected_statuses.insert(source.clone());
                }
                CleanupTransition::CallCommit { call, .. } => {
                    return Err(cleanup_error(format!(
                        "call-commit transition `{call}` reached binding preflight"
                    )));
                }
                CleanupTransition::Initialize { .. } | CleanupTransition::Transfer { .. } => {}
            }
        }
    }
    for indexed in &index.exits {
        match &indexed.exit.continuation {
            ExitContinuation::Continue(edge) => {
                bounded_continue_edge(index, indexed.exit, *edge)?;
            }
            ExitContinuation::CommitResult { source } => {
                publishes_result = true;
                if let CleanupResultSource::Scalar { expression } = source {
                    expected_scalars.insert(expression.clone());
                }
            }
            ExitContinuation::ReturnFailure { source } => {
                expected_statuses.insert(source.clone());
            }
            ExitContinuation::ReturnUnit => {
                return Err(cleanup_error(format!(
                    "exit {} uses unsupported unit return",
                    indexed.exit.id.0
                )));
            }
        }
    }
    require_exact_keys(
        &expected_booleans,
        bindings.boolean_values.keys().cloned().collect(),
        "boolean",
    )?;
    require_exact_keys(
        &expected_statuses,
        bindings.status_tokens.keys().cloned().collect(),
        "status",
    )?;
    require_exact_keys(
        &expected_scalars,
        bindings.scalar_results.keys().cloned().collect(),
        "scalar result",
    )?;
    if publishes_result != bindings.result_out.is_some() {
        return Err(cleanup_error(if publishes_result {
            "missing caller result-out binding"
        } else {
            "unexpected caller result-out binding"
        }));
    }

    let mut physical_identifiers = BTreeSet::new();
    for identifier in all_binding_identifiers(bindings) {
        if !is_c_identifier(identifier) {
            return Err(cleanup_error(format!(
                "binding `{identifier}` is not one C identifier"
            )));
        }
        if is_c_keyword(identifier) {
            return Err(cleanup_error(format!(
                "binding `{identifier}` is a reserved C keyword"
            )));
        }
        if is_reserved_binding_identifier(identifier) {
            return Err(cleanup_error(format!(
                "binding `{identifier}` is reserved by C or the SEMAPRAX compiler/runtime"
            )));
        }
        if identifier
            .strip_prefix("spx_bind_")
            .is_none_or(str::is_empty)
        {
            return Err(cleanup_error(format!(
                "binding `{identifier}` is outside the dedicated `spx_bind_` namespace"
            )));
        }
        if !physical_identifiers.insert(identifier) {
            return Err(cleanup_error(format!(
                "binding `{identifier}` aliases two cleanup scaffold inputs"
            )));
        }
    }
    Ok(())
}

fn all_binding_identifiers(bindings: &NativeCleanupBindings) -> impl Iterator<Item = &str> {
    bindings
        .storage_values
        .values()
        .chain(bindings.boolean_values.values())
        .chain(bindings.status_tokens.values())
        .chain(bindings.scalar_results.values())
        .chain(bindings.result_out.iter())
        .map(String::as_str)
}

fn require_exact_keys<T: Ord + std::fmt::Debug>(
    expected: &BTreeSet<T>,
    actual: BTreeSet<T>,
    kind: &str,
) -> Result<(), Diagnostic> {
    if let Some(missing) = expected.difference(&actual).next() {
        return Err(cleanup_error(format!(
            "missing {kind} binding for `{missing:?}`"
        )));
    }
    if let Some(extra) = actual.difference(expected).next() {
        return Err(cleanup_error(format!(
            "unexpected {kind} binding for `{extra:?}`"
        )));
    }
    Ok(())
}

fn leaf_for_place<'a>(
    index: &'a NativeCleanupIndex<'a>,
    place: &CleanupPlace,
) -> Result<&'a NativeCleanupLeaf<'a>, Diagnostic> {
    if !place.projections.is_empty() {
        return Err(cleanup_error(
            "projected cleanup place reached direct-resource emission",
        ));
    }
    let slot = index.slot(&place.storage).ok_or_else(|| {
        cleanup_error(format!(
            "cleanup place references unknown storage `{:?}`",
            place.storage
        ))
    })?;
    if slot.leaf.place != *place {
        return Err(cleanup_error(
            "cleanup place disagrees with its classified slot",
        ));
    }
    Ok(&slot.leaf)
}

fn storage_binding<'a>(
    bindings: &'a NativeCleanupBindings,
    storage: &StorageId,
) -> Result<&'a str, Diagnostic> {
    bindings
        .storage_values
        .get(storage)
        .map(String::as_str)
        .ok_or_else(|| cleanup_error(format!("missing storage binding for `{storage:?}`")))
}

fn status_binding<'a>(
    bindings: &'a NativeCleanupBindings,
    source: &StatusSourceId,
) -> Result<&'a str, Diagnostic> {
    bindings
        .status_tokens
        .get(source)
        .map(String::as_str)
        .ok_or_else(|| cleanup_error(format!("missing status binding for `{source:?}`")))
}

fn scalar_binding<'a>(
    bindings: &'a NativeCleanupBindings,
    expression: &ExpressionId,
) -> Result<&'a str, Diagnostic> {
    bindings
        .scalar_results
        .get(expression)
        .map(String::as_str)
        .ok_or_else(|| cleanup_error(format!("missing scalar result binding for `{expression}`")))
}

fn result_out(bindings: &NativeCleanupBindings) -> Result<&str, Diagnostic> {
    bindings
        .result_out
        .as_deref()
        .ok_or_else(|| cleanup_error("missing caller result-out binding"))
}

fn flag_symbol(flag: LivenessFlagId) -> String {
    format!("spx_cleanup_flag_{}", flag.0)
}

fn block_label(block: BlockId) -> String {
    format!("spx_cleanup_block_{}", block.0)
}

fn exit_label(exit: u32) -> String {
    format!("spx_cleanup_exit_{exit}")
}

fn is_c_identifier(value: &str) -> bool {
    let mut bytes = value.bytes();
    let Some(first) = bytes.next() else {
        return false;
    };
    (first == b'_' || first.is_ascii_alphabetic())
        && bytes.all(|byte| byte == b'_' || byte.is_ascii_alphanumeric())
}

fn is_c_keyword(value: &str) -> bool {
    matches!(
        value,
        "_Alignas"
            | "_Alignof"
            | "_Atomic"
            | "_Bool"
            | "_Complex"
            | "_Generic"
            | "_Imaginary"
            | "_Noreturn"
            | "_Static_assert"
            | "_Thread_local"
            | "auto"
            | "break"
            | "case"
            | "char"
            | "const"
            | "continue"
            | "default"
            | "do"
            | "double"
            | "else"
            | "enum"
            | "extern"
            | "float"
            | "for"
            | "goto"
            | "if"
            | "inline"
            | "int"
            | "long"
            | "register"
            | "restrict"
            | "return"
            | "short"
            | "signed"
            | "sizeof"
            | "static"
            | "struct"
            | "switch"
            | "typedef"
            | "union"
            | "unsigned"
            | "void"
            | "volatile"
            | "while"
    )
}

fn is_reserved_binding_identifier(value: &str) -> bool {
    matches!(
        value,
        "bool" | "true" | "false" | "NULL" | "SPX_STATUS_SUCCESS"
    ) || value.starts_with("__")
        || value
            .strip_prefix('_')
            .and_then(|rest| rest.bytes().next())
            .is_some_and(|byte| byte.is_ascii_uppercase())
        || (value.starts_with("spx_") && !value.starts_with("spx_bind_"))
        || value.starts_with("SPX_")
}

fn c_string(value: &str) -> String {
    let mut escaped = String::with_capacity(value.len());
    for character in value.chars() {
        match character {
            '\\' => escaped.push_str("\\\\"),
            '"' => escaped.push_str("\\\""),
            '\n' => escaped.push_str("\\n"),
            '\r' => escaped.push_str("\\r"),
            '\t' => escaped.push_str("\\t"),
            value if value.is_control() => {
                let mut bytes = [0; 4];
                for byte in value.encode_utf8(&mut bytes).bytes() {
                    write!(escaped, "\\{byte:03o}").expect("writing to a string cannot fail");
                }
            }
            value => escaped.push(value),
        }
    }
    escaped
}

fn cleanup_error(message: impl Into<String>) -> Diagnostic {
    Diagnostic::io(
        "SPX-B104",
        format!("native cleanup scaffold: {}", message.into()),
    )
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use crate::cleanup_plan::{CleanupResultSource, ExitContinuation};
    use crate::hir::{self, ResolvedFunction, ResolvedProgram};
    use crate::parse;

    use super::super::native_cleanup::classify;
    use super::*;

    const SOURCE: &str = r#"module test.native_cleanup_emit;

@id("token.type")
resource Token {
    @id("token.drop")
    drop trivial;
}

@id("token.discard")
fn discard(value: own Token) -> i64 { 0 }

@id("token.discard-two")
fn discard_two(first: own Token, second: own Token) -> i64 { 0 }

@id("token.contract-failure")
fn contract_failure(value: own Token) -> i64 requires false { 0 }

@id("token.identity")
fn identity(value: own Token) -> Token { value }

@id("token.checked")
fn checked(value: own Token, number: i64) -> i64 requires number >= 0 { number + 1 }

@id("app.main")
fn main() -> i64 { 0 }
"#;

    fn program() -> ResolvedProgram {
        let parsed = parse(SOURCE, Path::new("native-cleanup-emit.spx")).unwrap();
        hir::resolve(&parsed).unwrap()
    }

    fn function<'a>(program: &'a ResolvedProgram, id: &str) -> &'a ResolvedFunction {
        program
            .functions
            .iter()
            .find(|function| function.id.as_str() == id)
            .unwrap()
    }

    fn complete_bindings(index: &NativeCleanupIndex<'_>) -> NativeCleanupBindings {
        let mut bindings = NativeCleanupBindings::default();
        for slot in &index.slots {
            bindings.storage_values.insert(
                slot.slot.storage.clone(),
                format!("spx_bind_slot_{}", slot.slot.id.0),
            );
        }
        for edge in index.edges {
            match &edge.condition {
                EdgeCondition::BooleanResult(expression, _) => {
                    let next = bindings.boolean_values.len();
                    bindings
                        .boolean_values
                        .entry(expression.clone())
                        .or_insert_with(|| format!("spx_bind_bool_{next}"));
                }
                EdgeCondition::StatusZero(source) | EdgeCondition::StatusNonzero(source) => {
                    let next = bindings.status_tokens.len();
                    bindings
                        .status_tokens
                        .entry(source.clone())
                        .or_insert_with(|| format!("spx_bind_status_{next}"));
                }
                EdgeCondition::Always => {}
            }
        }
        for indexed in &index.exits {
            match &indexed.exit.continuation {
                ExitContinuation::CommitResult { source } => {
                    bindings.result_out = Some("spx_bind_result_out".to_owned());
                    if let CleanupResultSource::Scalar { expression } = source {
                        bindings
                            .scalar_results
                            .insert(expression.clone(), "spx_bind_scalar_result".to_owned());
                    }
                }
                ExitContinuation::ReturnFailure { source } => {
                    let next = bindings.status_tokens.len();
                    bindings
                        .status_tokens
                        .entry(source.clone())
                        .or_insert_with(|| format!("spx_bind_status_{next}"));
                }
                ExitContinuation::Continue(_) | ExitContinuation::ReturnUnit => {}
            }
        }
        bindings
    }

    #[test]
    fn discard_emits_exact_plan_driven_c() {
        let program = program();
        let index = classify(&program, function(&program, "token.discard")).unwrap();
        let emitted = emit(&index, &complete_bindings(&index)).unwrap();
        let expected = concat!(
            "/* semaprax.native-cleanup-scaffold.v1 */\n",
            "bool spx_cleanup_flag_0 = false;\n",
            "spx_status_token spx_cleanup_selected_status = SPX_STATUS_SUCCESS;\n",
            "spx_cleanup_flag_0 = true;\n",
            "goto spx_cleanup_block_0;\n",
            "spx_cleanup_block_0:\n",
            "goto spx_cleanup_exit_0;\n",
            "spx_cleanup_exit_0:\n",
            "if (spx_cleanup_flag_0) {\n",
            "    spx_cleanup_flag_0 = false;\n",
            "    (void)\"semaprax.cleanup.trivial-finalizer:token.drop\";\n",
            "}\n",
            "if (spx_cleanup_selected_status != SPX_STATUS_SUCCESS) spx_runtime_invariant_failure(\"cleanup result commit selected failure\");\n",
            "if (spx_cleanup_flag_0) spx_runtime_invariant_failure(\"cleanup scalar result commit retains a live resource\");\n",
            "*spx_bind_result_out = spx_bind_scalar_result;\n",
            "return SPX_STATUS_SUCCESS;\n",
        );
        assert_eq!(emitted, expected);
    }

    #[test]
    fn reverse_finalizers_clear_each_guard_before_the_marker() {
        let program = program();
        let index = classify(&program, function(&program, "token.discard-two")).unwrap();
        let emitted = emit(&index, &complete_bindings(&index)).unwrap();
        let second_clear = emitted.find("spx_cleanup_flag_1 = false;").unwrap();
        let second_finalize = emitted[second_clear..]
            .find("semaprax.cleanup.trivial-finalizer:token.drop")
            .map(|offset| second_clear + offset)
            .unwrap();
        let first_clear = emitted[second_finalize + 1..]
            .find("spx_cleanup_flag_0 = false;")
            .map(|offset| second_finalize + 1 + offset)
            .unwrap();
        let first_finalize = emitted[first_clear..]
            .find("semaprax.cleanup.trivial-finalizer:token.drop")
            .map(|offset| first_clear + offset)
            .unwrap();
        assert!(second_clear < second_finalize);
        assert!(second_finalize < first_clear);
        assert!(first_clear < first_finalize);
    }

    #[test]
    fn contract_failure_scaffold_is_deterministic_and_sticky() {
        let program = program();
        let index = classify(&program, function(&program, "token.contract-failure")).unwrap();
        let bindings = complete_bindings(&index);
        let first = emit(&index, &bindings).unwrap();
        let second = emit(&index, &bindings).unwrap();
        assert_eq!(first, second);
        assert!(first.contains("cleanup failure selection is not write-once"));
        let terminal_status = first
            .rfind("cleanup failure return changed status")
            .expect("exact failure status assertion");
        let terminal_liveness = first
            .rfind("cleanup failure return retains a live resource")
            .expect("failure liveness assertion");
        let failure_return = first
            .rfind("return spx_cleanup_selected_status;")
            .expect("sticky failure return");
        assert!(terminal_status < terminal_liveness);
        assert!(terminal_liveness < failure_return);
        assert!(first.contains("spx_cleanup_exit_"));
    }

    #[test]
    fn real_contract_and_checked_arithmetic_branches_emit_canonical_continue() {
        let program = program();
        let index = classify(&program, function(&program, "token.checked")).unwrap();
        let emitted = emit(&index, &complete_bindings(&index)).unwrap();

        emitted
            .find("if (spx_bind_bool_0) goto spx_cleanup_block_")
            .expect("contract success branch");
        emitted
            .find("else if (!spx_bind_bool_0) goto spx_cleanup_block_")
            .expect("contract failure branch");
        emitted
            .find("spx_cleanup_exit_1:\ngoto spx_cleanup_block_4;")
            .expect("empty-region continuation");
        emitted
            .find("== SPX_STATUS_SUCCESS) goto spx_cleanup_block_")
            .expect("checked arithmetic success branch");
        emitted
            .find("!= SPX_STATUS_SUCCESS) goto spx_cleanup_block_")
            .expect("checked arithmetic failure branch");
        emitted
            .find("spx_cleanup_selected_status = spx_bind_status_")
            .expect("sticky failure selection");
        let result_assertion = emitted
            .rfind("cleanup result commit selected failure")
            .expect("success terminal assertion");
        let result_write = emitted
            .rfind("*spx_bind_result_out = spx_bind_scalar_result;")
            .expect("scalar result publication");

        assert!(result_assertion < result_write);
    }

    #[test]
    fn owned_result_requires_exact_provisional_liveness_before_publication() {
        let program = program();
        let index = classify(&program, function(&program, "token.identity")).unwrap();
        let bindings = complete_bindings(&index);
        let provisional = bindings
            .storage_values
            .get(&StorageId::ProvisionalResult)
            .expect("provisional result binding");
        let provisional_leaf = index
            .slot(&StorageId::ProvisionalResult)
            .expect("provisional cleanup slot")
            .leaf
            .flag;
        let emitted = emit(&index, &bindings).unwrap();

        let status = emitted
            .rfind("cleanup result commit selected failure")
            .expect("success status assertion");
        let other_dead = emitted
            .rfind("cleanup owned result commit retains another live resource")
            .expect("non-result liveness assertion");
        let result_live = emitted
            .rfind("cleanup publishes a dead owned result")
            .expect("result liveness assertion");
        let publication = emitted
            .rfind(&format!("*spx_bind_result_out = {provisional};"))
            .expect("owned publication");
        let clear = emitted[publication..]
            .find(&format!("spx_cleanup_flag_{} = false;", provisional_leaf.0))
            .map(|offset| publication + offset)
            .expect("post-publication liveness clear");
        let success_return = emitted[clear..]
            .find("return SPX_STATUS_SUCCESS;")
            .map(|offset| clear + offset)
            .expect("success return");

        assert!(status < other_dead);
        assert!(other_dead < result_live);
        assert!(result_live < publication);
        assert!(publication < clear);
        assert!(clear < success_return);
    }

    #[test]
    fn emitter_rejects_cleanup_bearing_continue_independently() {
        let program = program();
        let index = classify(&program, function(&program, "token.contract-failure")).unwrap();
        let mut exit = index
            .exits
            .iter()
            .find(|indexed| matches!(indexed.exit.continuation, ExitContinuation::Continue(_)))
            .expect("compiler contract continuation")
            .exit
            .clone();
        let finalizer = index
            .exits
            .iter()
            .flat_map(|indexed| indexed.finalizers)
            .next()
            .expect("terminal cleanup")
            .clone();
        exit.finalize_in_order.push(finalizer);
        let mut output = String::new();
        let diagnostic =
            emit_continuation(&mut output, &index, &complete_bindings(&index), &exit).unwrap_err();
        assert_eq!(diagnostic.code, "SPX-B104");
        assert!(diagnostic.message.contains("performs cleanup"));
        assert!(output.is_empty());

        let mut conditional = index
            .exits
            .iter()
            .find(|indexed| matches!(indexed.exit.continuation, ExitContinuation::Continue(_)))
            .expect("compiler contract continuation")
            .exit
            .clone();
        let conditional_edge = index
            .edges
            .iter()
            .find(|edge| !matches!(edge.condition, EdgeCondition::Always))
            .expect("contract branch")
            .id;
        conditional.continuation = ExitContinuation::Continue(conditional_edge);
        let diagnostic = emit_continuation(
            &mut output,
            &index,
            &complete_bindings(&index),
            &conditional,
        )
        .unwrap_err();
        assert_eq!(diagnostic.code, "SPX-B104");
        assert!(diagnostic.message.contains("unconditional edge"));
        assert!(output.is_empty());
    }

    #[test]
    fn missing_and_extra_observation_bindings_fail_closed() {
        let program = program();
        let index = classify(&program, function(&program, "token.contract-failure")).unwrap();
        let mut missing = complete_bindings(&index);
        missing.boolean_values.clear();
        let diagnostic = emit(&index, &missing).unwrap_err();
        assert_eq!(diagnostic.code, "SPX-B104");
        assert!(diagnostic.message.contains("missing boolean binding"));

        let mut extra = complete_bindings(&index);
        extra
            .storage_values
            .insert(StorageId::ProvisionalResult, "spx_bind_extra".to_owned());
        let diagnostic = emit(&index, &extra).unwrap_err();
        assert_eq!(diagnostic.code, "SPX-B104");
        assert!(diagnostic.message.contains("unexpected storage binding"));
    }

    #[test]
    fn arbitrary_c_expressions_are_not_accepted_as_bindings() {
        let program = program();
        let index = classify(&program, function(&program, "token.discard")).unwrap();
        let mut bindings = complete_bindings(&index);
        bindings.result_out = Some("spx_bind_result_out + 1".to_owned());
        let diagnostic = emit(&index, &bindings).unwrap_err();
        assert_eq!(diagnostic.code, "SPX-B104");
        assert!(diagnostic.message.contains("is not one C identifier"));
    }

    #[test]
    fn keywords_aliases_and_scaffold_names_are_rejected() {
        let program = program();
        let index = classify(&program, function(&program, "token.discard")).unwrap();

        let mut keyword = complete_bindings(&index);
        keyword.result_out = Some("return".to_owned());
        let diagnostic = emit(&index, &keyword).unwrap_err();
        assert_eq!(diagnostic.code, "SPX-B104");
        assert!(diagnostic.message.contains("reserved C keyword"));

        let mut alias = complete_bindings(&index);
        alias.result_out = alias.storage_values.values().next().cloned();
        let diagnostic = emit(&index, &alias).unwrap_err();
        assert_eq!(diagnostic.code, "SPX-B104");
        assert!(diagnostic.message.contains("aliases two"));

        for identifier in [
            "true",
            "false",
            "bool",
            "NULL",
            "SPX_STATUS_SUCCESS",
            "__implementation",
            "_Reserved",
            "spx_runtime_invariant_failure",
            "SPX_PRIVATE",
        ] {
            let mut reserved = complete_bindings(&index);
            reserved.result_out = Some(identifier.to_owned());
            let diagnostic = emit(&index, &reserved).unwrap_err();
            assert_eq!(diagnostic.code, "SPX-B104");
            assert!(diagnostic.message.contains("is reserved"));
        }

        for identifier in [
            "UINT32_MAX",
            "SIZE_MAX",
            "INT64_MAX",
            "PTRDIFF_MAX",
            "stderr",
            "spx_bind_",
        ] {
            let mut outside_allocator = complete_bindings(&index);
            outside_allocator.result_out = Some(identifier.to_owned());
            let diagnostic = emit(&index, &outside_allocator).unwrap_err();
            assert_eq!(diagnostic.code, "SPX-B104");
            assert!(diagnostic
                .message
                .contains("dedicated `spx_bind_` namespace"));
        }
    }

    #[test]
    fn initialize_without_a_physical_payload_source_fails_closed() {
        let program = program();
        let index = classify(&program, function(&program, "token.discard")).unwrap();
        let destination = index.slots[0].leaf.place.clone();
        let at = match &index.exits[0].exit.continuation {
            ExitContinuation::CommitResult {
                source: CleanupResultSource::Scalar { expression },
            } => expression.clone(),
            continuation => panic!("unexpected continuation: {continuation:?}"),
        };
        let transition = CleanupTransition::Initialize { at, destination };
        let mut output = String::new();
        let diagnostic =
            emit_transition(&mut output, &index, &complete_bindings(&index), &transition)
                .unwrap_err();
        assert_eq!(diagnostic.code, "SPX-B104");
        assert!(diagnostic.message.contains("no physical payload source"));
        assert!(output.is_empty());
    }
}
