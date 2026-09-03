use std::sync::atomic::{AtomicUsize, Ordering};

use super::*;
use crate::{graph, parse};

static NEXT_FIXTURE: AtomicUsize = AtomicUsize::new(0);

const SOURCE: &str = r#"module impact.final_check;
@id("helper.answer") fn answer()->i64{42}
@id("app.main") fn main()->i64{answer()}
"#;

fn fixture(label: &str) -> (std::path::PathBuf, std::path::PathBuf, String) {
    let sequence = NEXT_FIXTURE.fetch_add(1, Ordering::Relaxed);
    let directory = std::env::temp_dir().join(format!(
        "semaprax-impact-unit-{}-{label}-{sequence}",
        std::process::id()
    ));
    std::fs::create_dir_all(&directory).unwrap();
    let source = directory.join("module.spx");
    let patch = directory.join("change.spatch");
    std::fs::write(&source, SOURCE).unwrap();
    let revision = graph::revision(&parse(SOURCE, &source).unwrap());
    std::fs::write(
        &patch,
        format!("base {revision}\nrename helper.answer to computed\n"),
    )
    .unwrap();
    (source, patch, revision)
}

#[test]
fn canonical_equivalent_source_byte_drift_is_rejected_at_final_check() {
    let (source, patch, _) = fixture("format-drift");
    let error = preview_with_hook(
        &source,
        &patch,
        &SemanticImpactOptions::default(),
        |phase, source, _| {
            if phase == PreviewPhase::BeforeFinalCheck {
                std::fs::write(source, SOURCE.replace("fn answer()", "fn  answer()"))?;
            }
            Ok(())
        },
    )
    .unwrap_err();
    assert_eq!(error[0].code, "SPX-I207");
}

#[test]
fn same_bytes_with_replaced_identity_are_rejected_at_final_check() {
    let (source, patch, _) = fixture("identity-drift");
    let displaced = source.with_extension("original.spx");
    let error = preview_with_hook(
        &source,
        &patch,
        &SemanticImpactOptions::default(),
        |phase, source, _| {
            if phase == PreviewPhase::BeforeFinalCheck {
                std::fs::rename(source, &displaced)?;
                std::fs::write(source, SOURCE)?;
            }
            Ok(())
        },
    )
    .unwrap_err();
    assert_eq!(error[0].code, "SPX-I207");
    assert_eq!(std::fs::read_to_string(source).unwrap(), SOURCE);
}

#[test]
fn patch_path_mutation_after_one_read_does_not_change_processed_digest() {
    let (source, patch, revision) = fixture("patch-drift");
    let original = std::fs::read_to_string(&patch).unwrap();
    let report = preview_with_hook(
        &source,
        &patch,
        &SemanticImpactOptions::default(),
        |phase, _, patch| {
            if phase == PreviewPhase::AfterPatchRead {
                std::fs::write(
                    patch,
                    format!("base {revision}\nrename helper.answer to changed_again\n"),
                )?;
            }
            Ok(())
        },
    )
    .unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&report).unwrap();
    assert_eq!(parsed["patch"]["digest"], patch_digest(&original));
    assert_eq!(parsed["operations"][0]["to"], "computed");
}

#[test]
fn exhausted_complete_node_budget_stops_before_wide_frontier_materialization() {
    let mut source = String::from(
            "module impact.aggregate_bound;\n@id(\"generic.marker\") fn marker<T>()->bool{true}\n@id(\"impact.seed\") fn seed()->bool{marker<i64>()}\n",
        );
    for index in 0..128 {
        source.push_str(&format!(
            "@id(\"impact.caller.{index}\") fn caller{index}()->bool{{seed()}}\n"
        ));
    }
    source.push_str("@id(\"app.main\") fn main()->i64{if caller0(){1}else{0}}\n");
    let program = parse(&source, Path::new("aggregate-bound.spx")).unwrap();
    let resolved = hir::resolve(&program).unwrap();
    let seed = resolved
        .functions
        .iter()
        .find(|function| function.id.as_str() == "impact.seed")
        .unwrap();
    let hir::ResolvedExprKind::Block { tail, .. } = &seed.body.kind else {
        panic!("seed body must be a block")
    };
    let hir::ResolvedExprKind::Call {
        instance: Some(instance),
        ..
    } = &tail.kind
    else {
        panic!("seed tail must be a generic call")
    };
    let patch_source = format!(
            "schema semaprax.semantic-patch.v2\nbase {}\nreplace-call-type-argument expression {} template generic.marker old-instance {} index 0 from i64 to bool\n",
            graph::revision(&program),
            tail.id,
            instance,
        );
    let preflight = patch::preflight_review_owned(
        source,
        patch_source,
        Path::new("aggregate-bound.spx").to_path_buf(),
        4096,
    )
    .unwrap();
    let Err(error) = complete_review_evidence_bounded(&preflight, 1, 16 * 1024 * 1024) else {
        panic!("wide frontier must stop at the remaining aggregate node bound")
    };
    assert_eq!(error[0].code, "SPX-G120");
}

#[test]
fn tiny_complete_byte_budget_stops_large_operation_and_change_serialization() {
    let mut source = String::from("module impact.aggregate_bytes;\n");
    let mut operations = String::new();
    for index in 0..128 {
        source.push_str(&format!(
            "@id(\"impact.item.{index}\") fn item{index}()->i64{{{index}}}\n"
        ));
        operations.push_str(&format!("rename impact.item.{index} to renamed{index}\n"));
    }
    source.push_str("@id(\"app.main\") fn main()->i64{item0()}\n");
    let program = parse(&source, Path::new("aggregate-bytes.spx")).unwrap();
    let patch_source = format!("base {}\n{operations}", graph::revision(&program));
    let preflight = patch::preflight_review_owned(
        source,
        patch_source,
        Path::new("aggregate-bytes.spx").to_path_buf(),
        4096,
    )
    .unwrap();
    let Err(error) = complete_review_evidence_bounded(&preflight, 1024, 64) else {
        panic!("large operations and changes must respect the remaining byte budget")
    };
    assert_eq!(error[0].code, "SPX-G120");
}
