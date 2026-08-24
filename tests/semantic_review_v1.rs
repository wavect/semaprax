use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicUsize, Ordering};

use semaprax::hir::{self, ResolvedExpr, ResolvedExprKind};
use semaprax::{graph, impact, parse, repair, review};
use sha2::{Digest, Sha256};

static NEXT_FIXTURE: AtomicUsize = AtomicUsize::new(0);

struct Fixture {
    directory: PathBuf,
    source: PathBuf,
    patch: PathBuf,
}

impl Fixture {
    fn new(label: &str, source: &str, patch: &str) -> Self {
        let sequence = NEXT_FIXTURE.fetch_add(1, Ordering::Relaxed);
        let directory = std::env::temp_dir().join(format!(
            "semaprax-review-{}-{label}-{sequence}",
            std::process::id()
        ));
        std::fs::create_dir_all(&directory).unwrap();
        let source_path = directory.join("module.spx");
        let patch_path = directory.join("change.spatch");
        std::fs::write(&source_path, source).unwrap();
        std::fs::write(&patch_path, patch).unwrap();
        Self {
            directory,
            source: source_path,
            patch: patch_path,
        }
    }

    fn inventory(&self) -> BTreeSet<String> {
        std::fs::read_dir(&self.directory)
            .unwrap()
            .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
            .collect()
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        std::fs::remove_dir_all(&self.directory).unwrap();
    }
}

fn revision(source: &str) -> String {
    graph::revision(&parse(source, Path::new("review.spx")).unwrap())
}

fn first_call<'a>(expression: &'a ResolvedExpr, template: &str) -> Option<&'a ResolvedExpr> {
    if matches!(&expression.kind, ResolvedExprKind::Call { callee, .. } if callee.as_str() == template)
    {
        return Some(expression);
    }
    match &expression.kind {
        ResolvedExprKind::Int(_)
        | ResolvedExprKind::Int32(_)
        | ResolvedExprKind::Char(_)
        | ResolvedExprKind::Uint8(_)
        | ResolvedExprKind::Float32(_)
        | ResolvedExprKind::Float64(_)
        | ResolvedExprKind::Bool(_)
        | ResolvedExprKind::String(_)
        | ResolvedExprKind::Place(_) => None,
        ResolvedExprKind::Call { args, .. } => args
            .iter()
            .find_map(|argument| first_call(argument, template)),
        ResolvedExprKind::NativeRustImportCall(call) => call
            .args
            .iter()
            .find_map(|argument| first_call(argument, template)),
        ResolvedExprKind::Unary { value, .. }
        | ResolvedExprKind::Project { base: value, .. }
        | ResolvedExprKind::Upcast { source: value }
        | ResolvedExprKind::Try { operand: value, .. }
        | ResolvedExprKind::TryOption { operand: value, .. } => first_call(value, template),
        ResolvedExprKind::Binary { left, right, .. } => {
            first_call(left, template).or_else(|| first_call(right, template))
        }
        ResolvedExprKind::Block { statements, tail } => statements
            .iter()
            .find_map(|statement| first_call(statement.value(), template))
            .or_else(|| first_call(tail, template)),
        ResolvedExprKind::If {
            condition,
            then_branch,
            else_branch,
        } => first_call(condition, template)
            .or_else(|| first_call(then_branch, template))
            .or_else(|| first_call(else_branch, template)),
        ResolvedExprKind::ConstructRecord { fields, .. }
        | ResolvedExprKind::ConstructVariant { fields, .. } => fields
            .iter()
            .find_map(|field| first_call(&field.value, template)),
        ResolvedExprKind::Match { scrutinee, arms } => first_call(scrutinee, template)
            .or_else(|| arms.iter().find_map(|arm| first_call(&arm.value, template))),
        ResolvedExprKind::UpdateRecord { base, fields, .. } => {
            first_call(base, template).or_else(|| {
                fields
                    .iter()
                    .find_map(|field| first_call(&field.value, template))
            })
        }
    }
}

fn assert_closed_sections(report: &serde_json::Value, operation_count: usize) {
    let expected = [
        "behavior",
        "api_identity",
        "security_authority",
        "memory_ownership",
        "target_artifact",
        "migration",
        "unsafe",
    ];
    assert_eq!(
        report["sections"]
            .as_object()
            .unwrap()
            .keys()
            .map(String::as_str)
            .collect::<BTreeSet<_>>(),
        expected.into_iter().collect::<BTreeSet<_>>()
    );
    for section in expected {
        let findings = report["sections"][section]["findings"].as_array().unwrap();
        assert_eq!(findings.len(), operation_count, "{section}");
        for (index, finding) in findings.iter().enumerate() {
            assert_eq!(finding["operation_indices"], serde_json::json!([index]));
            assert_eq!(finding["evidence_ids"], serde_json::json!(["evidence:0"]));
        }
    }
}

#[test]
fn v1_review_is_complete_canonical_read_only_and_digest_bound() {
    let source = r#"module review.rename;
@id("review.helper") fn helper()->i64{41}
@id("app.main") fn main()->i64{helper()+1}
"#;
    let patch = format!(
        "# review exact bytes\nbase {}\nrename review.helper to answer\nrequire no-new-effects\n",
        revision(source)
    );
    let fixture = Fixture::new("v1", source, &patch);
    let inventory = fixture.inventory();
    let output = review::preview(&fixture.source, &fixture.patch).unwrap();
    assert_eq!(
        review::preview(&fixture.source, &fixture.patch).unwrap(),
        output
    );
    assert_eq!(fixture.inventory(), inventory);
    assert_eq!(std::fs::read_to_string(&fixture.source).unwrap(), source);
    let report: serde_json::Value = serde_json::from_str(&output).unwrap();
    assert_eq!(
        format!(
            "{:x}",
            semaprax::digest_hex::LowerHex(Sha256::digest(output.as_bytes()))
        ),
        "054c12822e9984b3f9cab06056f311f35af3b06a438af7ade0b452a823443946"
    );
    let section_offsets = [
        "\"behavior\":",
        "\"api_identity\":",
        "\"security_authority\":",
        "\"memory_ownership\":",
        "\"target_artifact\":",
        "\"migration\":",
        "\"unsafe\":",
    ]
    .map(|key| output.find(key).unwrap());
    assert!(section_offsets.windows(2).all(|pair| pair[0] < pair[1]));
    assert_eq!(report["schema"], "semaprax.semantic-review.v1");
    assert_eq!(report["patch"]["schema"], "semaprax.semantic-patch.v1");
    assert_eq!(report["budget"]["used_output_bytes"], output.len());
    assert_eq!(report["budget"]["used_operations"], 2);
    assert_eq!(report["evidence"]["kind"], "semantic_impact_v1");
    assert_eq!(
        report["evidence"]["report"]["truncation"]["truncated"],
        false
    );
    assert_eq!(
        report["evidence"]["report"]["frontier"],
        serde_json::json!([])
    );
    assert_closed_sections(&report, 2);
    assert_eq!(
        report["nonclaims"],
        serde_json::json!([
            "not_proof_carrying_patch",
            "no_authenticated_provenance_or_signature",
            "no_human_approval_ui_or_policy",
            "no_public_verify_api_or_proof_artifact",
            "no_lock_stage_apply_or_commit_authority",
            "no_repository_or_multi_file_analysis",
            "no_agent_context_generation_or_embedding",
            "no_test_or_target_execution",
            "no_general_capability_security_unsafe_or_abi_analysis",
            "no_semantic_impact_v3",
            "no_persistence_or_incrementality",
            "no_external_consumer_compatibility"
        ])
    );
    assert_eq!(
        report["sections"]["behavior"]["findings"][0]["disposition"],
        "bounded_no_change"
    );
    assert_eq!(
        report["sections"]["api_identity"]["findings"][0]["disposition"],
        "change"
    );

    let mut source_hasher = Sha256::new();
    source_hasher.update(b"semaprax.semantic-review.source-digest.v1\0");
    source_hasher.update((source.len() as u64).to_le_bytes());
    source_hasher.update(source.as_bytes());
    assert_eq!(
        report["source"]["digest"],
        format!(
            "sha256:{:x}",
            semaprax::digest_hex::LowerHex(source_hasher.finalize())
        )
    );
    let mut patch_hasher = Sha256::new();
    patch_hasher.update(b"semaprax.semantic-review.patch-digest.v1\0");
    patch_hasher.update((patch.len() as u64).to_le_bytes());
    patch_hasher.update(patch.as_bytes());
    assert_eq!(
        report["patch"]["digest"],
        format!(
            "sha256:{:x}",
            semaprax::digest_hex::LowerHex(patch_hasher.finalize())
        )
    );
    let impact_bytes = impact::preview(
        &fixture.source,
        &fixture.patch,
        &impact::SemanticImpactOptions::new(1024, 16 * 1024 * 1024, 1024).unwrap(),
    )
    .unwrap();
    assert_eq!(
        report["evidence"]["report"],
        serde_json::from_str::<serde_json::Value>(&impact_bytes).unwrap()
    );
    let mut impact_hasher = Sha256::new();
    impact_hasher.update(b"semaprax.semantic-review.impact-digest.v1\0");
    impact_hasher.update((impact_bytes.len() as u64).to_le_bytes());
    impact_hasher.update(impact_bytes.as_bytes());
    assert_eq!(
        report["evidence"]["digest"],
        format!(
            "sha256:{:x}",
            semaprax::digest_hex::LowerHex(impact_hasher.finalize())
        )
    );
}

#[test]
fn v2_review_covers_grouped_behavioral_change_and_policy_in_every_section() {
    let source = r#"module review.generic;
@id("review.marker") fn marker<T,U>()->bool{true}
@id("review.caller") fn caller()->bool{marker<i64,bool>()}
@id("app.main") fn main()->i64{if caller(){1}else{0}}
"#;
    let program = parse(source, Path::new("review-generic.spx")).unwrap();
    let resolved = hir::resolve(&program).unwrap();
    let caller = resolved
        .functions
        .iter()
        .find(|function| function.id.as_str() == "review.caller")
        .unwrap();
    let call = first_call(&caller.body, "review.marker").unwrap();
    let ResolvedExprKind::Call {
        instance: Some(instance),
        ..
    } = &call.kind
    else {
        panic!("generic call has an instance")
    };
    let patch = format!(
        "schema semaprax.semantic-patch.v2\nbase {}\nreplace-call-type-argument expression {} template review.marker old-instance {} index 0 from i64 to bool\nrequire no-new-effects\n",
        graph::revision(&program), call.id, instance
    );
    let fixture = Fixture::new("v2", source, &patch);
    let output = review::preview(&fixture.source, &fixture.patch).unwrap();
    assert_eq!(
        review::preview(&fixture.source, &fixture.patch).unwrap(),
        output
    );
    assert_eq!(
        format!(
            "{:x}",
            semaprax::digest_hex::LowerHex(Sha256::digest(output.as_bytes()))
        ),
        "37fe056f519366fcaf6c13586e3b78afd64d51483490a1120e3e0fdc1b04c421"
    );
    let report: serde_json::Value = serde_json::from_str(&output).unwrap();
    assert_eq!(report["patch"]["schema"], "semaprax.semantic-patch.v2");
    assert_eq!(report["evidence"]["kind"], "semantic_impact_v1");
    assert_eq!(
        report["evidence"]["report"]["affected_functions"]
            .as_array()
            .unwrap()
            .len(),
        2
    );
    assert_closed_sections(&report, 2);
    assert_eq!(
        report["sections"]["behavior"]["findings"][0]["disposition"],
        "change"
    );
    assert_eq!(
        report["sections"]["memory_ownership"]["findings"][0]["disposition"],
        "bounded_no_change"
    );
    assert_eq!(std::fs::read_to_string(&fixture.source).unwrap(), source);
}

#[test]
fn v2_function_member_and_case_renames_share_the_exact_normalized_graph_proof() {
    let source = r#"module review.rename_domains;
@id("review.box") record Box { @id("review.box.value") value: i64, }
@id("review.outcome") variant Outcome { @id("review.outcome.ok") Ok { @id("review.outcome.ok.value") value: i64, }, @id("review.outcome.err") Err, }
@id("review.read") fn read(input:Box)->i64{match input {Box { value }=>value,}}
@id("review.unwrap") fn unwrap(input:Outcome)->i64{match input {Outcome::Ok { value }=>value,Outcome::Err {}=>0,}}
@id("app.main") fn main()->i64{read(Box { value: unwrap(Outcome::Ok { value: 42 }) })}
"#;
    let patch = format!(
        "schema semaprax.semantic-patch.v2\nbase {}\nrename review.read to consume\nrename-member owner review.box member review.box.value to payload\nrename-case owner review.outcome case review.outcome.ok to Success\nrename-member owner review.outcome.ok member review.outcome.ok.value to payload\n",
        revision(source)
    );
    let fixture = Fixture::new("rename-domains", source, &patch);
    let output = review::preview(&fixture.source, &fixture.patch).unwrap();
    let report: serde_json::Value = serde_json::from_str(&output).unwrap();
    assert_closed_sections(&report, 4);
    assert_eq!(
        report["sections"]["behavior"]["assessment"],
        "unchanged_within_admitted_domain"
    );
    for finding in report["sections"]["behavior"]["findings"]
        .as_array()
        .unwrap()
    {
        assert_eq!(finding["disposition"], "bounded_no_change");
        assert!(finding["statement"]
            .as_str()
            .unwrap()
            .contains("name-normalized Graph"));
    }
    assert_eq!(
        report["evidence"]["report"]["affected_functions"],
        serde_json::json!([])
    );
}

#[test]
fn v3_review_embeds_the_exact_shared_repair_identity_rebase_without_impact() {
    let source = r#"module review.rebase;
fn helper(value:i64)->i64{value+1}
@id("review.caller") fn caller(value:i64)->i64{helper(value)}
@id("app.main") fn main()->i64{caller(41)}
"#;
    let target = "auto:review.rebase.helper";
    let query = repair::DiagnosticRepairQuery::assign_function_id(target).unwrap();
    let source_fixture = Fixture::new("v3-source", source, "");
    let repairs: serde_json::Value =
        serde_json::from_str(&repair::query(&source_fixture.source, &query).unwrap()).unwrap();
    let repair_preview: serde_json::Value = serde_json::from_str(
        &repair::instantiate(
            &source_fixture.source,
            repairs["repair"]["id"].as_str().unwrap(),
            &repair::PersistentDeclarationId::new("review.rebase.helper").unwrap(),
        )
        .unwrap(),
    )
    .unwrap();
    std::fs::write(
        &source_fixture.patch,
        repair_preview["patch"]["source"].as_str().unwrap(),
    )
    .unwrap();
    let inventory = source_fixture.inventory();
    let output = review::preview(&source_fixture.source, &source_fixture.patch).unwrap();
    assert_eq!(
        review::preview(&source_fixture.source, &source_fixture.patch).unwrap(),
        output
    );
    assert_eq!(
        format!(
            "{:x}",
            semaprax::digest_hex::LowerHex(Sha256::digest(output.as_bytes()))
        ),
        "081bcb20aca2e74f724f5bc0cd2cf03770a499e11aa090d92b59650209165544"
    );
    let report: serde_json::Value = serde_json::from_str(&output).unwrap();
    assert_eq!(source_fixture.inventory(), inventory);
    assert_eq!(report["patch"]["schema"], "semaprax.semantic-patch.v3");
    assert_eq!(report["evidence"]["kind"], "identity_rebase_v1");
    assert_eq!(
        report["evidence"]["identity_rebase"],
        repair_preview["identity_rebase"]
    );
    let identity = &report["evidence"]["identity_rebase"];
    let direct_callers = serde_json::to_string(&identity["direct_callers"]).unwrap();
    let identity_bytes = format!(
        "{{\"before_id\":{},\"after_id\":{},\"name\":{},\"direct_callers\":{},\"derived_id_count\":{},\"derived_id_digest\":{}}}",
        serde_json::to_string(&identity["before_id"]).unwrap(),
        serde_json::to_string(&identity["after_id"]).unwrap(),
        serde_json::to_string(&identity["name"]).unwrap(),
        direct_callers,
        identity["derived_id_count"],
        serde_json::to_string(&identity["derived_id_digest"]).unwrap(),
    );
    let mut identity_hasher = Sha256::new();
    identity_hasher.update(b"semaprax.semantic-review.identity-rebase-digest.v1\0");
    identity_hasher.update((identity_bytes.len() as u64).to_le_bytes());
    identity_hasher.update(identity_bytes.as_bytes());
    assert_eq!(
        report["evidence"]["digest"],
        format!(
            "sha256:{:x}",
            semaprax::digest_hex::LowerHex(identity_hasher.finalize())
        )
    );
    assert!(report["evidence"].get("report").is_none());
    assert_eq!(report["budget"]["used_impact_bytes"], 0);
    assert_closed_sections(&report, 1);
    assert_eq!(
        report["sections"]["migration"]["findings"][0]["disposition"],
        "migration_required"
    );
    assert_eq!(
        std::fs::read_to_string(&source_fixture.source).unwrap(),
        source
    );
}

#[test]
fn cli_is_fixed_arity_and_emits_exact_api_bytes_plus_one_lf() {
    let source = "module review.cli;\n@id(\"review.helper\") fn helper()->i64{1}\n@id(\"app.main\") fn main()->i64{helper()}\n";
    let patch = format!(
        "base {}\nrename review.helper to renamed\n",
        revision(source)
    );
    let fixture = Fixture::new("cli", source, &patch);
    let api = review::preview(&fixture.source, &fixture.patch).unwrap();
    let binary = env!("CARGO_BIN_EXE_semaprax");
    let output = Command::new(binary)
        .arg("review")
        .arg(&fixture.source)
        .arg(&fixture.patch)
        .output()
        .unwrap();
    assert!(output.status.success());
    assert_eq!(output.stdout, format!("{api}\n").as_bytes());
    let rejected = Command::new(binary)
        .arg("review")
        .arg(&fixture.source)
        .arg(&fixture.patch)
        .arg("--depth")
        .arg("1")
        .output()
        .unwrap();
    assert_eq!(rejected.status.code(), Some(2));
}

#[test]
fn stale_or_malformed_patch_never_creates_a0_artifacts() {
    let source = "module review.hostile;\n@id(\"review.helper\") fn helper()->i64{1}\n@id(\"app.main\") fn main()->i64{helper()}\n";
    for (label, patch, code) in [
        (
            "stale",
            "base sha256:stale\nrename review.helper to renamed\n".to_owned(),
            "SPX-G409",
        ),
        ("malformed", "rename\n".to_owned(), "SPX-G101"),
    ] {
        let fixture = Fixture::new(label, source, &patch);
        let inventory = fixture.inventory();
        let error = review::preview(&fixture.source, &fixture.patch).unwrap_err();
        assert_eq!(error[0].code, code);
        assert_eq!(fixture.inventory(), inventory);
        assert_eq!(std::fs::read_to_string(&fixture.source).unwrap(), source);
    }
}

#[test]
fn operation_limit_is_rejected_before_unresolved_hir_is_constructed() {
    let source = "module review.limit;\n@id(\"review.helper\") fn helper()->i64{missing()}\n@id(\"app.main\") fn main()->i64{helper()}\n";
    let mut patch = format!("base {}\n", revision(source));
    for _ in 0..=4096 {
        patch.push_str("rename review.helper to renamed\n");
    }
    let fixture = Fixture::new("operation-limit", source, &patch);
    let error = review::preview(&fixture.source, &fixture.patch).unwrap_err();
    assert_eq!(error[0].code, "SPX-G120");
}

#[test]
fn declaration_limit_is_rejected_before_hir_construction() {
    let mut source =
        String::from("module review.declaration_limit;\n@id(\"review.large\") record Large {\n");
    for index in 0..4095 {
        source.push_str(&format!(
            "@id(\"review.large.field{index}\") field{index}: i64,\n"
        ));
    }
    source.push_str("}\n@id(\"app.main\") fn main()->i64{missing()}\n");
    let fixture = Fixture::new("declaration-limit", &source, "base irrelevant\n");
    let error = review::preview(&fixture.source, &fixture.patch).unwrap_err();
    assert_eq!(error[0].code, "SPX-G120");
}

#[test]
fn callable_limit_is_rejected_before_hir_construction() {
    let mut source = String::from("module review.callable_limit;\n");
    for index in 0..1025 {
        source.push_str(&format!(
            "@id(\"review.function{index}\") fn function{index}()->i64{{missing()}}\n"
        ));
    }
    let fixture = Fixture::new("callable-limit", &source, "base irrelevant\n");
    let error = review::preview(&fixture.source, &fixture.patch).unwrap_err();
    assert_eq!(error[0].code, "SPX-G120");
}

#[test]
fn call_site_limit_is_rejected_before_hir_construction() {
    let mut source = String::from("module review.call_limit;\n@id(\"app.main\") fn main()->i64{\n");
    for index in 0..=65_536 {
        source.push_str(&format!("let value{index}=missing();\n"));
    }
    source.push_str("0}\n");
    let fixture = Fixture::new("call-limit", &source, "base irrelevant\n");
    let error = review::preview(&fixture.source, &fixture.patch).unwrap_err();
    assert_eq!(error[0].code, "SPX-G120");
}
