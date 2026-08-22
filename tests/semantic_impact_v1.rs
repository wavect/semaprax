use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicUsize, Ordering};

use semaprax::hir::{self, ResolvedExpr, ResolvedExprKind, ResolvedStatement};
use semaprax::{graph, impact, parse, patch};
use sha2::{Digest, Sha256};

static NEXT_FIXTURE: AtomicUsize = AtomicUsize::new(0);

struct Fixture {
    directory: PathBuf,
    source: PathBuf,
    patch: PathBuf,
}

impl Fixture {
    fn new(label: &str, source: &str) -> (Self, String) {
        let sequence = NEXT_FIXTURE.fetch_add(1, Ordering::Relaxed);
        let directory = std::env::temp_dir().join(format!(
            "semaprax-impact-{}-{label}-{sequence}",
            std::process::id()
        ));
        std::fs::create_dir_all(&directory).unwrap();
        let source_path = directory.join("module.spx");
        let patch_path = directory.join("change.spatch");
        std::fs::write(&source_path, source).unwrap();
        let revision = graph::revision(&parse(source, &source_path).unwrap());
        (
            Self {
                directory,
                source: source_path,
                patch: patch_path,
            },
            revision,
        )
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

fn first_call<'a>(expression: &'a ResolvedExpr, template: &str) -> Option<&'a ResolvedExpr> {
    if matches!(&expression.kind, ResolvedExprKind::Call { callee, .. } if callee.as_str() == template)
    {
        return Some(expression);
    }
    match &expression.kind {
        ResolvedExprKind::Int(_)
        | ResolvedExprKind::Float32(_)
        | ResolvedExprKind::Float64(_)
        | ResolvedExprKind::Bool(_)
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
        | ResolvedExprKind::Try { operand: value, .. }
        | ResolvedExprKind::TryOption { operand: value, .. } => first_call(value, template),
        ResolvedExprKind::Binary { left, right, .. } => {
            first_call(left, template).or_else(|| first_call(right, template))
        }
        ResolvedExprKind::Block { statements, tail } => statements
            .iter()
            .find_map(|statement| {
                let ResolvedStatement::Let { value, .. } = statement;
                first_call(value, template)
            })
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

#[test]
fn rename_preview_is_canonical_read_only_and_digest_bound_to_exact_patch_bytes() {
    let source = r#"module impact.rename;
@id("helper.answer") fn answer()->i64{42}
@id("app.main") fn main()->i64{answer()}
"#;
    let (fixture, revision) = Fixture::new("rename", source);
    let patch_source = format!(
        "# exact bytes matter\nbase {revision}\nrename helper.answer to computed\nrequire no-new-effects\n"
    );
    std::fs::write(&fixture.patch, &patch_source).unwrap();
    let before_inventory = fixture.inventory();
    let output = impact::preview(
        &fixture.source,
        &fixture.patch,
        &impact::SemanticImpactOptions::default(),
    )
    .unwrap();
    assert_eq!(fixture.inventory(), before_inventory);
    assert_eq!(std::fs::read_to_string(&fixture.source).unwrap(), source);

    let parsed: serde_json::Value = serde_json::from_str(&output).unwrap();
    assert_eq!(
        format!(
            "{:x}",
            semaprax::digest_hex::LowerHex(Sha256::digest(output.as_bytes()))
        ),
        "94bbe5dcfe02f4b80b12ba5c8faf0889ddf11a96598072e539490c71a09518e9"
    );
    assert_eq!(parsed["schema"], "semaprax.semantic-impact.v1");
    assert_eq!(parsed["patch"]["schema"], "semaprax.semantic-patch.v1");
    assert_eq!(parsed["budget"]["used_bytes"], output.len());
    assert_eq!(parsed["affected_functions"], serde_json::json!([]));
    assert_eq!(parsed["changes"][0]["classification"], "source_projection");
    assert_eq!(
        parsed["changes"][0]["source_consumers"]
            .as_array()
            .unwrap()
            .len(),
        2
    );
    assert!(parsed["changes"][0]["source_consumers"]
        .as_array()
        .unwrap()
        .iter()
        .all(|consumer| consumer["identity_origin"] == "explicit"));

    let mut hasher = Sha256::new();
    hasher.update(b"semaprax.semantic-impact.patch-digest.v1\0");
    hasher.update((patch_source.len() as u64).to_le_bytes());
    hasher.update(patch_source.as_bytes());
    assert_eq!(
        parsed["patch"]["digest"],
        format!(
            "sha256:{:x}",
            semaprax::digest_hex::LowerHex(hasher.finalize())
        )
    );

    let first_digest = parsed["patch"]["digest"].clone();
    std::fs::write(
        &fixture.patch,
        format!("{patch_source}# trailing comment\n"),
    )
    .unwrap();
    let changed: serde_json::Value = serde_json::from_str(
        &impact::preview(
            &fixture.source,
            &fixture.patch,
            &impact::SemanticImpactOptions::default(),
        )
        .unwrap(),
    )
    .unwrap();
    assert_ne!(changed["patch"]["digest"], first_digest);
    assert_eq!(changed["operations"], parsed["operations"]);
    let envelope_error = impact::preview(
        &fixture.source,
        &fixture.patch,
        &impact::SemanticImpactOptions::new(1, 1024, 256).unwrap(),
    )
    .unwrap_err();
    assert_eq!(envelope_error[0].code, "SPX-G109");
}

#[test]
fn grouped_call_change_seeds_only_exact_containing_caller_and_reverse_callers() {
    let source = r#"module impact.call;
@id("generic.marker") fn marker<T,U>()->bool{true}
@id("impact.selected") fn selected()->bool{marker<i64,bool>()}
@id("impact.unrelated") fn unrelated()->bool{marker<i64,bool>()}
@id("impact.outer") fn outer()->bool{selected()}
@id("app.main") fn main()->i64{if outer()&&unrelated(){1}else{0}}
"#;
    let program = parse(source, Path::new("impact-call.spx")).unwrap();
    let resolved = hir::resolve(&program).unwrap();
    let selected = resolved
        .functions
        .iter()
        .find(|function| function.id.as_str() == "impact.selected")
        .unwrap();
    let call = first_call(&selected.body, "generic.marker").unwrap();
    let ResolvedExprKind::Call {
        instance: Some(instance),
        ..
    } = &call.kind
    else {
        panic!("generic call must have an instance")
    };
    let (fixture, revision) = Fixture::new("grouped-call", source);
    std::fs::write(
        &fixture.patch,
        format!(
            "schema semaprax.semantic-patch.v2\nbase {revision}\nreplace-call-type-argument expression {} template generic.marker old-instance {} index 0 from i64 to bool\nreplace-call-type-argument expression {} template generic.marker old-instance {} index 1 from bool to i64\nrequire no-new-effects\n",
            call.id, instance, call.id, instance
        ),
    )
    .unwrap();
    let parsed: serde_json::Value = serde_json::from_str(
        &impact::preview(
            &fixture.source,
            &fixture.patch,
            &impact::SemanticImpactOptions::new(2, 64 * 1024, 256).unwrap(),
        )
        .unwrap(),
    )
    .unwrap();
    assert_eq!(parsed["changes"].as_array().unwrap().len(), 1);
    assert_eq!(
        parsed["changes"][0]["operation_indices"],
        serde_json::json!([0, 1])
    );
    assert_eq!(
        parsed["changes"][0]["after_type_arguments"],
        serde_json::json!(["bool", "i64"])
    );
    let affected = parsed["affected_functions"].as_array().unwrap();
    assert_eq!(affected[0]["id"], "impact.selected");
    assert_eq!(affected[0]["depth"], 0);
    assert_eq!(affected[1]["id"], "impact.outer");
    assert_eq!(affected[1]["depth"], 1);
    assert_eq!(affected[2]["id"], "app.main");
    assert_eq!(affected[2]["depth"], 2);
    assert!(!affected.iter().any(|fact| fact["id"] == "impact.unrelated"));
    let bounded: serde_json::Value = serde_json::from_str(
        &impact::preview(
            &fixture.source,
            &fixture.patch,
            &impact::SemanticImpactOptions::new(0, 64 * 1024, 256).unwrap(),
        )
        .unwrap(),
    )
    .unwrap();
    assert_eq!(bounded["truncation"]["omitted_known_nodes"], 2);
    assert_eq!(bounded["truncation"]["deferred_known_nodes"], 1);
    assert_eq!(bounded["frontier"][0]["id"], "impact.outer");
    assert_eq!(
        bounded["frontier"][0]["reasons"],
        serde_json::json!(["depth"])
    );
    assert_eq!(std::fs::read_to_string(&fixture.source).unwrap(), source);
}

#[test]
fn duplicate_v1_rename_is_one_change_and_apply_preview_revision_parity_holds() {
    let source = r#"module impact.duplicate;
@id("helper.answer") fn answer()->i64{42}
@id("app.main") fn main()->i64{answer()}
"#;
    let (preview_fixture, revision) = Fixture::new("duplicate-preview", source);
    let patch_source = format!(
        "base {revision}\nrename helper.answer to computed\nrename helper.answer to computed\n"
    );
    std::fs::write(&preview_fixture.patch, &patch_source).unwrap();
    let preview: serde_json::Value = serde_json::from_str(
        &impact::preview(
            &preview_fixture.source,
            &preview_fixture.patch,
            &impact::SemanticImpactOptions::default(),
        )
        .unwrap(),
    )
    .unwrap();
    assert_eq!(preview["changes"].as_array().unwrap().len(), 1);
    assert_eq!(
        preview["changes"][0]["operation_indices"],
        serde_json::json!([0, 1])
    );
    assert_eq!(
        preview["changes"][0]["source_consumers"][0]["site_count"],
        1
    );
    assert_eq!(
        preview["changes"][0]["source_consumers"][1]["site_count"],
        1
    );

    let (apply_fixture, apply_revision) = Fixture::new("duplicate-apply", source);
    assert_eq!(revision, apply_revision);
    std::fs::write(&apply_fixture.patch, patch_source).unwrap();
    let returned = patch::apply(&apply_fixture.source, &apply_fixture.patch).unwrap();
    assert_eq!(returned, preview["candidate_revision"].as_str().unwrap());
}

#[test]
fn cli_rejects_confused_options_before_semantic_output() {
    let binary = env!("CARGO_BIN_EXE_semaprax");
    for options in [
        vec!["--depth", "01"],
        vec!["--depth", "-1"],
        vec!["--depth", "1", "--depth", "2"],
        vec!["--direction", "reverse"],
        vec!["--max-bytes"],
    ] {
        let output = Command::new(binary)
            .arg("impact")
            .arg("missing.spx")
            .arg("missing.spatch")
            .args(options)
            .output()
            .unwrap();
        assert_eq!(output.status.code(), Some(2));
        assert!(output.stdout.is_empty());
    }
}

#[test]
fn automatic_call_owner_applies_but_impact_fails_g110_without_artifacts() {
    let source = r#"module impact.automatic;
@id("generic.marker") fn marker<T>()->bool{true}
fn automatic()->bool{marker<i64>()}
@id("app.main") fn main()->i64{0}
"#;
    let program = parse(source, Path::new("automatic-owner.spx")).unwrap();
    let resolved = hir::resolve(&program).unwrap();
    let automatic = resolved
        .functions
        .iter()
        .find(|function| function.name == "automatic")
        .unwrap();
    let call = first_call(&automatic.body, "generic.marker").unwrap();
    let ResolvedExprKind::Call {
        instance: Some(instance),
        ..
    } = &call.kind
    else {
        panic!("generic call must have an instance")
    };
    let (preview_fixture, revision) = Fixture::new("automatic-preview", source);
    let patch_source = format!(
        "schema semaprax.semantic-patch.v2\nbase {revision}\nreplace-call-type-argument expression {} template generic.marker old-instance {} index 0 from i64 to bool\n",
        call.id, instance
    );
    std::fs::write(&preview_fixture.patch, &patch_source).unwrap();
    let before_inventory = preview_fixture.inventory();
    let error = impact::preview(
        &preview_fixture.source,
        &preview_fixture.patch,
        &impact::SemanticImpactOptions::default(),
    )
    .unwrap_err();
    assert_eq!(error[0].code, "SPX-G110");
    assert_eq!(preview_fixture.inventory(), before_inventory);
    assert_eq!(
        std::fs::read_to_string(&preview_fixture.source).unwrap(),
        source
    );

    let (apply_fixture, apply_revision) = Fixture::new("automatic-apply", source);
    assert_eq!(revision, apply_revision);
    std::fs::write(&apply_fixture.patch, patch_source).unwrap();
    patch::apply(&apply_fixture.source, &apply_fixture.patch).unwrap();
    assert!(std::fs::read_to_string(&apply_fixture.source)
        .unwrap()
        .contains("marker<bool>()"));
}

#[test]
fn automatic_rename_consumer_is_reported_exactly_but_automatic_reverse_caller_fails() {
    let rename_source = r#"module impact.automatic_consumer;
@id("helper.answer") fn answer()->i64{42}
fn automatic()->i64{answer()}
@id("app.main") fn main()->i64{0}
"#;
    let (rename_fixture, rename_revision) = Fixture::new("automatic-consumer", rename_source);
    std::fs::write(
        &rename_fixture.patch,
        format!("base {rename_revision}\nrename helper.answer to computed\n"),
    )
    .unwrap();
    let report: serde_json::Value = serde_json::from_str(
        &impact::preview(
            &rename_fixture.source,
            &rename_fixture.patch,
            &impact::SemanticImpactOptions::default(),
        )
        .unwrap(),
    )
    .unwrap();
    assert!(report["changes"][0]["source_consumers"]
        .as_array()
        .unwrap()
        .iter()
        .any(|consumer| consumer["identity_origin"] == "automatic"));

    let call_source = r#"module impact.automatic_reverse;
@id("generic.marker") fn marker<T>()->bool{true}
@id("impact.selected") fn selected()->bool{marker<i64>()}
fn automatic()->bool{selected()}
@id("app.main") fn main()->i64{0}
"#;
    let program = parse(call_source, Path::new("automatic-reverse.spx")).unwrap();
    let resolved = hir::resolve(&program).unwrap();
    let selected = resolved
        .functions
        .iter()
        .find(|function| function.id.as_str() == "impact.selected")
        .unwrap();
    let call = first_call(&selected.body, "generic.marker").unwrap();
    let ResolvedExprKind::Call {
        instance: Some(instance),
        ..
    } = &call.kind
    else {
        panic!("generic call must have an instance")
    };
    let (call_fixture, revision) = Fixture::new("automatic-reverse", call_source);
    std::fs::write(
        &call_fixture.patch,
        format!(
            "schema semaprax.semantic-patch.v2\nbase {revision}\nreplace-call-type-argument expression {} template generic.marker old-instance {} index 0 from i64 to bool\n",
            call.id, instance
        ),
    )
    .unwrap();
    let error = impact::preview(
        &call_fixture.source,
        &call_fixture.patch,
        &impact::SemanticImpactOptions::default(),
    )
    .unwrap_err();
    assert_eq!(error[0].code, "SPX-G110");
}

#[test]
fn multi_seed_cycle_and_diamond_merge_only_minimum_depth_provenance() {
    let source = r#"module impact.provenance;
@id("generic.marker") fn marker<T>()->bool{true}
@id("impact.seed.a") fn seed_a()->bool{marker<i64>()&&joiner()}
@id("impact.seed.b") fn seed_b()->bool{marker<i64>()}
@id("impact.shared") fn joiner()->bool{seed_a()||seed_b()}
@id("app.main") fn main()->i64{if joiner(){1}else{0}}
"#;
    let program = parse(source, Path::new("impact-provenance.spx")).unwrap();
    let resolved = hir::resolve(&program).unwrap();
    let call_for = |id: &str| {
        let function = resolved
            .functions
            .iter()
            .find(|function| function.id.as_str() == id)
            .unwrap();
        let call = first_call(&function.body, "generic.marker").unwrap();
        let ResolvedExprKind::Call {
            instance: Some(instance),
            ..
        } = &call.kind
        else {
            panic!("generic call must have an instance")
        };
        (call.id.as_str().to_owned(), instance.as_str().to_owned())
    };
    let (call_a, instance_a) = call_for("impact.seed.a");
    let (call_b, instance_b) = call_for("impact.seed.b");
    let (fixture, revision) = Fixture::new("provenance", source);
    std::fs::write(
        &fixture.patch,
        format!(
            "schema semaprax.semantic-patch.v2\nbase {revision}\nreplace-call-type-argument expression {call_a} template generic.marker old-instance {instance_a} index 0 from i64 to bool\nreplace-call-type-argument expression {call_b} template generic.marker old-instance {instance_b} index 0 from i64 to bool\n"
        ),
    )
    .unwrap();
    let full_output = impact::preview(
        &fixture.source,
        &fixture.patch,
        &impact::SemanticImpactOptions::new(8, 64 * 1024, 256).unwrap(),
    )
    .unwrap();
    let report: serde_json::Value = serde_json::from_str(&full_output).unwrap();
    let affected = report["affected_functions"].as_array().unwrap();
    assert_eq!(affected[0]["id"], "impact.seed.a");
    assert_eq!(affected[0]["operation_indices"], serde_json::json!([0]));
    assert_eq!(affected[1]["id"], "impact.seed.b");
    assert_eq!(affected[1]["operation_indices"], serde_json::json!([1]));
    let shared = affected
        .iter()
        .find(|fact| fact["id"] == "impact.shared")
        .unwrap();
    assert_eq!(shared["depth"], 1);
    assert_eq!(shared["operation_indices"], serde_json::json!([0, 1]));
    let main = affected
        .iter()
        .find(|fact| fact["id"] == "app.main")
        .unwrap();
    assert_eq!(main["depth"], 2);
    assert_eq!(main["operation_indices"], serde_json::json!([0, 1]));

    let node_bounded: serde_json::Value = serde_json::from_str(
        &impact::preview(
            &fixture.source,
            &fixture.patch,
            &impact::SemanticImpactOptions::new(8, 64 * 1024, 1).unwrap(),
        )
        .unwrap(),
    )
    .unwrap();
    assert_eq!(node_bounded["budget"]["used_nodes"], 1);
    assert_eq!(node_bounded["frontier"][0]["id"], "impact.seed.b");
    assert_eq!(
        node_bounded["frontier"][0]["reasons"],
        serde_json::json!(["max_nodes"])
    );
    assert_eq!(node_bounded["truncation"]["omitted_known_nodes"], 3);
    assert_eq!(node_bounded["truncation"]["deferred_known_nodes"], 2);
    let byte_output = impact::preview(
        &fixture.source,
        &fixture.patch,
        &impact::SemanticImpactOptions::new(8, 2609, 256).unwrap(),
    )
    .unwrap();
    let byte_bounded: serde_json::Value = serde_json::from_str(&byte_output).unwrap();
    assert_eq!(byte_bounded["budget"]["used_bytes"], byte_output.len());
    assert!(byte_output.len() <= 2609);
    assert_eq!(byte_bounded["budget"]["used_nodes"], 1);
    assert_eq!(byte_bounded["truncation"]["omitted_known_nodes"], 3);
    assert_eq!(byte_bounded["truncation"]["deferred_known_nodes"], 2);
    assert_eq!(
        byte_bounded["truncation"]["reasons"],
        serde_json::json!(["max_bytes"])
    );
    assert_eq!(
        byte_bounded["frontier"][0]["reasons"],
        serde_json::json!(["max_bytes"])
    );
    assert_eq!(
        impact::preview(
            &fixture.source,
            &fixture.patch,
            &impact::SemanticImpactOptions::new(8, 2609, 256).unwrap(),
        )
        .unwrap(),
        byte_output
    );
    let byte_error = impact::preview(
        &fixture.source,
        &fixture.patch,
        &impact::SemanticImpactOptions::new(8, 2608, 256).unwrap(),
    )
    .unwrap_err();
    assert_eq!(byte_error[0].code, "SPX-G109");
}

#[test]
fn every_graph_v10_v14_schema_is_equal_across_preview() {
    let cases = [
        (
            r#"module impact.schema_v10;
@id("schema.target") fn target()->i64{1}
@id("app.main") fn main()->i64{target()}
"#,
            "schema.target",
            "renamed_v10",
            "semaprax.graph.v10",
        ),
        (
            r#"module impact.schema_v11;
@id("schema.target") fn target(input:Option<i64>)->Option<bool>{let checked=input?;Option<bool>::Some { value: checked>0 }}
@id("app.main") fn main()->i64{0}
"#,
            "schema.target",
            "renamed_v11",
            "semaprax.graph.v11",
        ),
        (
            include_str!("../platform-tests/component-runtime/v7.spx"),
            "component.transform-i64-bool",
            "renamed_v12",
            "semaprax.graph.v12",
        ),
        (
            include_str!("../platform-tests/component-runtime/v8.spx"),
            "component.pattern.preserve-phantom-i64",
            "renamed_v13",
            "semaprax.graph.v13",
        ),
        (
            r#"module impact.schema_v14;
@id("schema.target") fn target<T>()->bool{true}
@id("app.main") fn main()->i64{if target<i64>(){1}else{0}}
"#,
            "schema.target",
            "renamed_v14",
            "semaprax.graph.v14",
        ),
    ];
    for (index, (source, target, renamed, schema)) in cases.into_iter().enumerate() {
        let (fixture, revision) = Fixture::new(&format!("schema-{index}"), source);
        std::fs::write(
            &fixture.patch,
            format!("base {revision}\nrename {target} to {renamed}\n"),
        )
        .unwrap();
        let report: serde_json::Value = serde_json::from_str(
            &impact::preview(
                &fixture.source,
                &fixture.patch,
                &impact::SemanticImpactOptions::default(),
            )
            .unwrap(),
        )
        .unwrap();
        assert_eq!(report["source_graph_schema"], schema);
    }
}

#[test]
fn requires_body_and_ensures_calls_all_seed_the_exact_monomorphic_owner() {
    let source = r#"module impact.call_regions;
@id("generic.marker") fn marker<T>()->bool{true}
@id("impact.checked") fn checked()->bool requires marker<i64>() ensures marker<i64>() { marker<i64>() }
@id("app.main") fn main()->i64{0}
"#;
    let program = parse(source, Path::new("impact-call-regions.spx")).unwrap();
    let resolved = hir::resolve(&program).unwrap();
    let checked = resolved
        .functions
        .iter()
        .find(|function| function.id.as_str() == "impact.checked")
        .unwrap();
    let calls = [
        first_call(&checked.requires[0], "generic.marker").unwrap(),
        first_call(&checked.body, "generic.marker").unwrap(),
        first_call(&checked.ensures[0], "generic.marker").unwrap(),
    ];
    let (fixture, revision) = Fixture::new("call-regions", source);
    let mut patch_source = format!("schema semaprax.semantic-patch.v2\nbase {revision}\n");
    for call in calls {
        let ResolvedExprKind::Call {
            instance: Some(instance),
            ..
        } = &call.kind
        else {
            panic!("generic call must have an instance")
        };
        patch_source.push_str(&format!(
            "replace-call-type-argument expression {} template generic.marker old-instance {} index 0 from i64 to bool\n",
            call.id, instance
        ));
    }
    std::fs::write(&fixture.patch, patch_source).unwrap();
    let report: serde_json::Value = serde_json::from_str(
        &impact::preview(
            &fixture.source,
            &fixture.patch,
            &impact::SemanticImpactOptions::default(),
        )
        .unwrap(),
    )
    .unwrap();
    assert_eq!(report["changes"].as_array().unwrap().len(), 3);
    assert_eq!(report["affected_functions"].as_array().unwrap().len(), 1);
    assert_eq!(report["affected_functions"][0]["id"], "impact.checked");
    assert_eq!(
        report["affected_functions"][0]["operation_indices"],
        serde_json::json!([0, 1, 2])
    );
}

#[test]
fn rename_only_all_domains_have_exact_consumer_coverage_and_no_behavioral_impact() {
    let source = r#"module impact.rename_domains;
@id("impact.handle") resource Handle { @id("impact.handle.drop") drop trivial; }
@id("impact.box") record Box { @id("impact.box.value") value: i64, }
@id("impact.outcome") variant Outcome { @id("impact.outcome.ok") Ok { @id("impact.outcome.ok.value") value: i64, }, @id("impact.outcome.err") Err, }
@id("impact.use") fn use(input:Box)->i64{match input {Box { value }=>value,}}
@id("impact.use-handle") fn use_handle(input:borrow Handle)->i64{0}
@id("impact.unwrap") fn unwrap(input:Outcome)->i64{match input {Outcome::Ok { value }=>value,Outcome::Err {}=>0,}}
@id("app.main") fn main()->i64{0}
"#;
    let (fixture, revision) = Fixture::new("rename-domains", source);
    std::fs::write(
        &fixture.patch,
        format!(
            "schema semaprax.semantic-patch.v2\nbase {revision}\nrename impact.use to consume\nrename impact.handle to Token\nrename-member owner impact.box member impact.box.value to payload\nrename-case owner impact.outcome case impact.outcome.ok to Success\nrename-member owner impact.outcome.ok member impact.outcome.ok.value to payload\nrequire no-new-effects\n"
        ),
    )
    .unwrap();
    let output = impact::preview(
        &fixture.source,
        &fixture.patch,
        &impact::SemanticImpactOptions::default(),
    )
    .unwrap();
    let report: serde_json::Value = serde_json::from_str(&output).unwrap();
    assert_eq!(report["changes"].as_array().unwrap().len(), 5);
    assert_eq!(report["affected_functions"], serde_json::json!([]));
    assert_eq!(report["frontier"], serde_json::json!([]));
    for change in report["changes"].as_array().unwrap() {
        let consumers = change["source_consumers"].as_array().unwrap();
        assert!(!consumers.is_empty());
        assert!(consumers
            .iter()
            .all(|consumer| consumer["site_count"].as_u64().unwrap() > 0));
    }
    assert!(!output.contains(&fixture.source.display().to_string()));
    assert!(!output.contains("\"span\""));
    assert!(!output.contains("\"start\""));
}

#[test]
fn interleaved_operation_kinds_control_change_order_and_consumer_wire_order() {
    let source = r#"module impact.interleaved;
@id("impact.box") record Box { @id("impact.box.value") value: i64, }
@id("generic.marker") fn marker<T>()->bool{true}
@id("impact.selected") fn selected()->bool{marker<i64>()&&selected()}
@id("impact.read") fn read(input:Box)->i64{match input {Box { value }=>value,}}
@id("app.main") fn main()->i64{if selected(){read(Box { value: 1 })}else{0}}
"#;
    let program = parse(source, Path::new("impact-interleaved.spx")).unwrap();
    let resolved = hir::resolve(&program).unwrap();
    let selected = resolved
        .functions
        .iter()
        .find(|function| function.id.as_str() == "impact.selected")
        .unwrap();
    let call = first_call(&selected.body, "generic.marker").unwrap();
    let ResolvedExprKind::Call {
        instance: Some(instance),
        ..
    } = &call.kind
    else {
        panic!("generic call must have an instance")
    };
    let (fixture, revision) = Fixture::new("interleaved", source);
    std::fs::write(
        &fixture.patch,
        format!(
            "schema semaprax.semantic-patch.v2\nbase {revision}\nreplace-call-type-argument expression {} template generic.marker old-instance {} index 0 from i64 to bool\nrename impact.selected to chosen\nrename-member owner impact.box member impact.box.value to payload\n",
            call.id, instance
        ),
    )
    .unwrap();
    let report: serde_json::Value = serde_json::from_str(
        &impact::preview(
            &fixture.source,
            &fixture.patch,
            &impact::SemanticImpactOptions::default(),
        )
        .unwrap(),
    )
    .unwrap();
    assert_eq!(
        report["changes"]
            .as_array()
            .unwrap()
            .iter()
            .map(|change| change["kind"].as_str().unwrap())
            .collect::<Vec<_>>(),
        ["call_instance", "rename", "rename"]
    );
    assert_eq!(
        report["changes"][0]["operation_indices"],
        serde_json::json!([0])
    );
    assert_eq!(
        report["changes"][1]["operation_indices"],
        serde_json::json!([1])
    );
    assert_eq!(
        report["changes"][2]["operation_indices"],
        serde_json::json!([2])
    );

    let selected_consumers = report["changes"][1]["source_consumers"].as_array().unwrap();
    let selected_self = selected_consumers
        .iter()
        .find(|consumer| consumer["id"] == "impact.selected")
        .unwrap();
    assert_eq!(
        selected_self["roles"],
        serde_json::json!(["declaration", "reference"])
    );
    let field_consumers = report["changes"][2]["source_consumers"].as_array().unwrap();
    assert_eq!(
        field_consumers
            .iter()
            .map(|consumer| consumer["kind"].as_str().unwrap())
            .collect::<Vec<_>>(),
        ["function", "function", "field"]
    );
    assert_eq!(field_consumers[0]["id"], "app.main");
    assert_eq!(field_consumers[1]["id"], "impact.read");
    assert_eq!(field_consumers[2]["id"], "impact.box.value");
    assert_eq!(
        field_consumers[0]["roles"],
        serde_json::json!(["reference"])
    );
    assert_eq!(
        field_consumers[2]["roles"],
        serde_json::json!(["declaration"])
    );
}

#[test]
fn high_cardinality_patch_provenance_and_tight_byte_selection_are_deterministic() {
    const COUNT: usize = 1024;
    let mut rename_source = String::from("module impact.large_patch;\n");
    let mut rename_patch_body = String::new();
    for index in 0..COUNT {
        rename_source.push_str(&format!(
            "@id(\"bulk.{index:04}\") fn f{index:04}()->i64{{{index}}}\n"
        ));
        rename_patch_body.push_str(&format!("rename bulk.{index:04} to g{index:04}\n"));
    }
    rename_source.push_str("@id(\"app.main\") fn main()->i64{0}\n");
    let (rename_fixture, revision) = Fixture::new("large-patch", &rename_source);
    std::fs::write(
        &rename_fixture.patch,
        format!("base {revision}\n{rename_patch_body}"),
    )
    .unwrap();
    let rename_report: serde_json::Value = serde_json::from_str(
        &impact::preview(
            &rename_fixture.source,
            &rename_fixture.patch,
            &impact::SemanticImpactOptions::new(1, 16 * 1024 * 1024, 256).unwrap(),
        )
        .unwrap(),
    )
    .unwrap();
    assert_eq!(rename_report["operations"].as_array().unwrap().len(), COUNT);
    assert_eq!(rename_report["changes"].as_array().unwrap().len(), COUNT);
    assert_eq!(
        rename_report["changes"][0]["operation_indices"],
        serde_json::json!([0])
    );
    assert_eq!(
        rename_report["changes"][COUNT - 1]["operation_indices"],
        serde_json::json!([COUNT - 1])
    );

    let mut chain_source = String::from(
        "module impact.large_closure;\n@id(\"generic.marker\") fn marker<T>()->bool{true}\n",
    );
    chain_source.push_str("@id(\"chain.0000\") fn f0000()->bool{marker<i64>()}\n");
    for index in 1..COUNT {
        chain_source.push_str(&format!(
            "@id(\"chain.{index:04}\") fn f{index:04}()->bool{{f{:04}()}}\n",
            index - 1
        ));
    }
    chain_source.push_str(&format!(
        "@id(\"app.main\") fn main()->i64{{if f{:04}(){{1}}else{{0}}}}\n",
        COUNT - 1
    ));
    let program = parse(&chain_source, Path::new("impact-large-closure.spx")).unwrap();
    let resolved = hir::resolve(&program).unwrap();
    let seed = resolved
        .functions
        .iter()
        .find(|function| function.id.as_str() == "chain.0000")
        .unwrap();
    let call = first_call(&seed.body, "generic.marker").unwrap();
    let ResolvedExprKind::Call {
        instance: Some(instance),
        ..
    } = &call.kind
    else {
        panic!("generic call must have an instance")
    };
    let (chain_fixture, chain_revision) = Fixture::new("large-closure", &chain_source);
    std::fs::write(
        &chain_fixture.patch,
        format!(
            "schema semaprax.semantic-patch.v2\nbase {chain_revision}\nreplace-call-type-argument expression {} template generic.marker old-instance {} index 0 from i64 to bool\n",
            call.id, instance
        ),
    )
    .unwrap();
    let full = impact::preview(
        &chain_fixture.source,
        &chain_fixture.patch,
        &impact::SemanticImpactOptions::new(1024, 16 * 1024 * 1024, 65_536).unwrap(),
    )
    .unwrap();
    let tight_budget = full.len() / 2;
    let options = impact::SemanticImpactOptions::new(1024, tight_budget, 65_536).unwrap();
    let tight = impact::preview(&chain_fixture.source, &chain_fixture.patch, &options).unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&tight).unwrap();
    assert_eq!(parsed["budget"]["used_bytes"], tight.len());
    assert!(tight.len() <= tight_budget);
    assert!(parsed["budget"]["used_nodes"].as_u64().unwrap() > 0);
    assert!(parsed["budget"]["used_nodes"].as_u64().unwrap() < (COUNT + 1) as u64);
    assert_eq!(
        parsed["truncation"]["reasons"],
        serde_json::json!(["max_bytes"])
    );
    assert_eq!(
        impact::preview(&chain_fixture.source, &chain_fixture.patch, &options).unwrap(),
        tight
    );
}
