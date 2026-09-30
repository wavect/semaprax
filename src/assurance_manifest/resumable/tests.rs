use super::super::{
    derive, generate, render, verify_envelope, verify_envelope_against_source,
    AssuranceManifestOptions,
};
use super::*;
use std::sync::atomic::{AtomicUsize, Ordering};

static COUNTER: AtomicUsize = AtomicUsize::new(0);

const SOURCE: &str = r#"
module test.resumable_assurance;
@id("app.ask")
fn ask(seed: i64) -> i64
    yields i64 -> i64
    requires seed >= 0
    ensures result >= 0
{
    let first = yield seed;
    let second = yield first + 1;
    second
}
@id("app.main")
fn main() -> i64 { 0 }
"#;

const CONTROL_SOURCE: &str = r#"
module test.resumable_assurance_control;
@id("app.ask_control")
fn ask_control(seed: i64) -> i64
    yields i64 -> i64
    requires seed >= 0
    ensures result >= 0
{
    let first = if seed > 0 {
        let a = yield seed;
        a
    } else {
        let b = yield 0;
        b
    };
    first + 1
}
@id("app.main")
fn main() -> i64 { 0 }
"#;

fn program(source: &str) -> (crate::ast::Program, ResolvedProgram) {
    let parsed = crate::parse(source, Path::new("resumable-assurance.spx")).unwrap();
    let resolved = crate::hir::resolve(&parsed).unwrap();
    (parsed, resolved)
}

fn write_source(source: &str) -> std::path::PathBuf {
    let path = std::env::temp_dir().join(format!(
        "semaprax-resumable-assurance-{}-{}.spx",
        std::process::id(),
        COUNTER.fetch_add(1, Ordering::SeqCst)
    ));
    std::fs::write(&path, source).unwrap();
    path
}

fn envelope(source: &str) -> String {
    let path = write_source(source);
    let result = generate(&path, &AssuranceManifestOptions::default()).unwrap();
    verify_envelope_against_source(&result, &path).unwrap();
    std::fs::remove_file(path).unwrap();
    result
}

fn rewrap(value: &serde_json::Value) -> String {
    let payload = serde_json::to_string(&value["payload"]).unwrap();
    format!(
        "{{\"schema\":\"{}\",\"digest\":\"{}\",\"bytes\":{},\"payload\":{}}}",
        render::SCHEMA,
        render::payload_digest(payload.as_bytes()),
        payload.len(),
        payload
    )
}

#[test]
fn source_contracts_bind_entry_and_final_resume_without_new_obligation_ids() {
    let (parsed, resolved) = program(SOURCE);
    let mut obligations = derive::derive_obligations(&parsed);
    let original = obligations.clone();
    attach(&resolved, &mut obligations).unwrap();
    assert_eq!(
        original.iter().map(|o| &o.id).collect::<Vec<_>>(),
        obligations.iter().map(|o| &o.id).collect::<Vec<_>>()
    );
    let plan = lower_sequential(&resolved, &resolved.functions[0]).unwrap();
    for (before, after) in original.iter().zip(&obligations) {
        assert_eq!(before.methods[0], after.methods[0]);
        if matches!(
            after.kind,
            ObligationKind::Precondition | ObligationKind::Postcondition
        ) {
            assert_eq!(after.methods.len(), 2);
            assert_eq!(after.methods[1].class, AssuranceClass::RuntimeGuarded);
        }
    }
    let pre = obligations
        .iter()
        .find(|o| o.kind == ObligationKind::Precondition)
        .unwrap();
    let post = obligations
        .iter()
        .find(|o| o.kind == ObligationKind::Postcondition)
        .unwrap();
    assert_eq!(pre.methods[1].inputs[3], plan.entry.id.as_str());
    assert_eq!(
        pre.methods[1].inputs[4],
        plan.suspensions[0].state.id.as_str()
    );
    assert_eq!(
        post.methods[1].inputs[3],
        plan.suspensions[1].state.id.as_str()
    );
    assert_eq!(post.methods[1].inputs[4], plan.complete.id.as_str());
    let report = envelope(SOURCE);
    verify_envelope(&report).unwrap();
}

#[test]
fn nonyield_obligation_bytes_are_unchanged() {
    let source = SOURCE
        .replace("    yields i64 -> i64\n", "")
        .replace("yield ", "");
    let (parsed, resolved) = program(&source);
    let mut actual = derive::derive_obligations(&parsed);
    let expected = actual.clone();
    attach(&resolved, &mut actual).unwrap();
    assert_eq!(actual, expected);
    let report = envelope(&source);
    assert!(!report.contains(TOOL));
    let value: serde_json::Value = serde_json::from_str(&report).unwrap();
    let mut legacy_obligations = expected;
    legacy_obligations.extend(derive::derive_resolved_obligations(&resolved).unwrap());
    let options = AssuranceManifestOptions::default();
    let legacy = render::render(&render::RenderInput {
        source_path_text: value["payload"]["source"]["path"].as_str().unwrap(),
        revision: &crate::graph::revision(&parsed),
        source_sha256: &render::source_digest(&source),
        obligations: &legacy_obligations,
        assumptions: &[],
        max_bytes: options.max_bytes,
        max_obligations: options.max_obligations,
    });
    assert_eq!(
        report, legacy,
        "non-yielding manifest bytes must not change"
    );
}

#[test]
fn source_replay_rejects_rehashed_state_plan_tool_and_missing_binding() {
    let path = write_source(SOURCE);
    let report = generate(&path, &AssuranceManifestOptions::default()).unwrap();
    verify_envelope_against_source(&report, &path).unwrap();
    let value: serde_json::Value = serde_json::from_str(&report).unwrap();
    let index = value["payload"]["obligations"]
        .as_array()
        .unwrap()
        .iter()
        .position(|o| o["kind"] == "precondition")
        .unwrap();
    for field in [
        "state",
        "plan",
        "tool",
        "missing",
        "order",
        "declaration_id",
        "kind",
    ] {
        let mut changed = value.clone();
        let methods = &mut changed["payload"]["obligations"][index]["methods"];
        match field {
            "state" => methods[1]["inputs"][3] = "forged-state".into(),
            "plan" => methods[1]["inputs"][1] = "forged-plan".into(),
            "tool" => methods[1]["tool"] = "renamed-tool".into(),
            "missing" => {
                methods.as_array_mut().unwrap().remove(1);
            }
            "order" => methods.as_array_mut().unwrap().swap(0, 1),
            "declaration_id" => {
                changed["payload"]["obligations"][index]["declaration_id"] = "app.other".into()
            }
            "kind" => changed["payload"]["obligations"][index]["kind"] = "postcondition".into(),
            _ => unreachable!(),
        }
        let forged = rewrap(&changed);
        verify_envelope(&forged).unwrap();
        assert_eq!(
            verify_envelope_against_source(&forged, &path)
                .unwrap_err()
                .code,
            "SPX-Z104",
            "{field}"
        );
    }
    std::fs::remove_file(path).unwrap();
}

#[test]
fn source_drift_changes_plan_even_with_unchanged_contract_ids() {
    let report = envelope(SOURCE);
    let changed = SOURCE.replace("first + 1", "first + 2");
    assert_eq!(
        verify_source(&report, &changed, Path::new("test.spx"))
            .unwrap_err()
            .code,
        "SPX-Z104"
    );
    let (_, first) = program(SOURCE);
    let (_, second) = program(&changed);
    let first = methods(&first).unwrap();
    let second = methods(&second).unwrap();
    assert_eq!(
        first.keys().collect::<Vec<_>>(),
        second.keys().collect::<Vec<_>>()
    );
    assert_ne!(first, second);
}

#[test]
fn one_site_contract_mapping_is_deterministic_and_unsupported_projection_refuses() {
    let source = SOURCE.replace("    let second = yield first + 1;\n    second", "    first");
    let (_, resolved) = program(&source);
    assert_eq!(methods(&resolved).unwrap(), methods(&resolved).unwrap());
    let map = methods(&resolved).unwrap();
    assert!(map
        .values()
        .all(|m| m.bounds.as_deref() == Some("sequential_copy_scalar_yields:1")));
    let unsupported = SOURCE.replace("fn main() -> i64 { 0 }", "fn main() -> i64 { ask(0) }");
    let (_, resolved) = program(&unsupported);
    assert_eq!(methods(&resolved).unwrap_err().code, "SPX-H006");
}

#[test]
fn control_dependent_source_contracts_bind_entry_and_final_resume() {
    let (parsed, resolved) = program(CONTROL_SOURCE);
    let mut obligations = derive::derive_obligations(&parsed);
    let original = obligations.clone();
    attach(&resolved, &mut obligations).unwrap();
    assert_eq!(
        original.iter().map(|o| &o.id).collect::<Vec<_>>(),
        obligations.iter().map(|o| &o.id).collect::<Vec<_>>(),
        "attaching a control-dependent method must never mint a new obligation id"
    );
    let plan = lower_control(&resolved, &resolved.functions[0]).unwrap();
    assert!(!plan.carries_owned_bytes);
    assert_eq!(plan.sites.len(), 2);
    let pre = obligations
        .iter()
        .find(|o| o.kind == ObligationKind::Precondition)
        .unwrap();
    let post = obligations
        .iter()
        .find(|o| o.kind == ObligationKind::Postcondition)
        .unwrap();
    assert_eq!(pre.methods.len(), 2);
    assert_eq!(post.methods.len(), 2);
    assert_eq!(pre.methods[1].class, AssuranceClass::RuntimeGuarded);
    assert_eq!(pre.methods[1].inputs[3], plan.entry.id.as_str());
    assert_eq!(pre.methods[1].inputs[4], plan.sites[0].state.id.as_str());
    assert_eq!(
        post.methods[1].inputs[3],
        plan.sites.last().unwrap().state.id.as_str()
    );
    assert_eq!(post.methods[1].inputs[4], plan.complete.id.as_str());
    assert_eq!(
        pre.methods[1].bounds.as_deref(),
        Some("control_dependent_copy_scalar_yields:2")
    );
    assert_eq!(
        post.methods[1].bounds.as_deref(),
        Some("control_dependent_copy_scalar_yields:2")
    );
    // No yield-free backend projection exists for the control-dependent plan
    // (`SPX-H006`), unlike the sequential lane's `resumable_yield_free_projection`.
    assert_eq!(pre.methods[1].target, None);
    assert_eq!(post.methods[1].target, None);
    let report = envelope(CONTROL_SOURCE);
    verify_envelope(&report).unwrap();
}

/// Bug #296 (R20): a purely scalar control-dependent function (no owned
/// `Bytes` local, an empty cleanup plan) reaching a top-level `yield` only
/// past a preceding statement that itself branches (`first`'s own
/// `if`/`else`, each arm holding its own `yield`) must still let source
/// contract obligation generation succeed -- `methods` calls `lower_control`
/// unconditionally for every control-dependent `yields` function that
/// carries a `requires`/`ensures` clause, and `lower_control` itself calls
/// `cleanup_plan::carried_locals_at` for every site, which is exactly where
/// this bug's branching-predecessor refusal used to surface.
const JOIN_SOURCE: &str = r#"
module test.resumable_assurance_control_join;
@id("app.ask_join")
fn ask_join(seed: i64) -> i64
    yields i64 -> i64
    requires seed >= 0
    ensures result >= 0
{
    let first = if seed > 0 {
        let a = yield seed;
        a
    } else {
        let b = yield 0;
        b
    };
    let second = yield first + 1;
    second
}
@id("app.main")
fn main() -> i64 { 0 }
"#;

#[test]
fn control_dependent_source_contracts_bind_a_top_level_site_past_a_branching_predecessor() {
    let (parsed, resolved) = program(JOIN_SOURCE);
    let mut obligations = derive::derive_obligations(&parsed);
    let original = obligations.clone();
    attach(&resolved, &mut obligations).unwrap();
    assert_eq!(
        original.iter().map(|o| &o.id).collect::<Vec<_>>(),
        obligations.iter().map(|o| &o.id).collect::<Vec<_>>(),
        "attaching a control-dependent method must never mint a new obligation id"
    );
    let plan = lower_control(&resolved, &resolved.functions[0]).unwrap();
    assert!(!plan.carries_owned_bytes);
    assert_eq!(plan.sites.len(), 3);
    let pre = obligations
        .iter()
        .find(|o| o.kind == ObligationKind::Precondition)
        .unwrap();
    assert_eq!(
        pre.methods[1].bounds.as_deref(),
        Some("control_dependent_copy_scalar_yields:3")
    );
    let report = envelope(JOIN_SOURCE);
    verify_envelope(&report).unwrap();
}

#[test]
fn control_dependent_source_drift_changes_plan_even_with_unchanged_contract_ids() {
    let report = envelope(CONTROL_SOURCE);
    let changed = CONTROL_SOURCE.replace("first + 1", "first + 2");
    assert_eq!(
        verify_source(&report, &changed, Path::new("test.spx"))
            .unwrap_err()
            .code,
        "SPX-Z104"
    );
    let (_, first) = program(CONTROL_SOURCE);
    let (_, second) = program(&changed);
    let first = methods(&first).unwrap();
    let second = methods(&second).unwrap();
    assert_eq!(
        first.keys().collect::<Vec<_>>(),
        second.keys().collect::<Vec<_>>()
    );
    assert_ne!(first, second);
}
