//! Closed repair selection; all proposed semantics come from fresh derivation.
use super::{diagnostics::ProjectCandidateAttempt, intent, wire, ProjectCandidate};
use crate::ast::Program;
use crate::diagnostic::Diagnostic;
use serde_json::Value;

pub(super) fn apply(
    base: &ProjectCandidate,
    programs: &mut [Program],
    request: &Value,
) -> Result<intent::IntentSummary, Vec<Diagnostic>> {
    exact(request, &["kind", "target", "rejected_intent", "repair_id"])?;
    let target = text(request, "target")?;
    let rejected = &request["rejected_intent"];
    exact(rejected, &["kind", "target", "body"])?;
    if rejected["kind"] != "replace_function_body" || rejected["target"] != target {
        return Err(grammar(
            "repair intention must select a body rejection on exactly the same target",
        ));
    }
    let body = &rejected["body"];
    if !body.is_object() {
        return Err(grammar(
            "repair intention body requires a typed constructor",
        ));
    }
    if matches!(text(body, "kind")?, "i64" | "i32" | "u8" | "usize") {
        // Preserve the original literal wire grammar and diagnostics exactly.
        exact(body, &["kind", "value"])?;
        if !(body["value"].is_i64() || body["value"].is_u64()) {
            return Err(grammar(
                "repair intention supports only an explicit integer-literal body rejection",
            ));
        }
    }
    let repair_id = text(request, "repair_id")?;
    wire::validate_digest(repair_id)
        .map_err(|_| grammar("repair selector must be an exact SHA-256 digest"))?;
    // This closed rejected kind makes recursion impossible: neither the failed
    // apply nor the compiler-derived successful apply can be another repair.
    let derived = ProjectCandidateAttempt::derive_wire_repair(base, rejected, repair_id)?;
    let mut summary = intent::apply_with_revision(base.revision(), programs, &derived.intent)?;
    summary.kind = "repair_diagnostic".into();
    Ok(summary)
}

fn exact(value: &Value, fields: &[&str]) -> Result<(), Vec<Diagnostic>> {
    let object = value
        .as_object()
        .ok_or_else(|| grammar("repair intention requires a closed object"))?;
    if object.len() != fields.len() || fields.iter().any(|field| !object.contains_key(*field)) {
        return Err(grammar("repair intention has missing or unknown fields"));
    }
    Ok(())
}
fn text<'a>(value: &'a Value, field: &str) -> Result<&'a str, Vec<Diagnostic>> {
    value[field]
        .as_str()
        .ok_or_else(|| grammar("repair intention field must be text"))
}
fn grammar(message: &'static str) -> Vec<Diagnostic> {
    vec![Diagnostic::io("SPX-G268", message)]
}
pub(super) fn stale(message: &'static str) -> Vec<Diagnostic> {
    vec![Diagnostic::io("SPX-G270", message)]
}
pub(super) fn rebase_conflict() -> Vec<Diagnostic> {
    vec![Diagnostic::io("SPX-G271", "repair intention is bound to its exact candidate predecessor; rediscover the repair instead of rebasing its selector")]
}
