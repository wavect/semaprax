//! Complete physical-input successors for the immutable CI and Source36 views.
//! This test-only projection owns no compiler authority and grants no admission.
use sha2::{Digest, Sha256};
fn project(rows: &str, source: &str, current: &str) -> String {
    let rows: serde_json::Value = serde_json::from_str(rows).expect("fixed merge projections");
    let matching: Vec<_> = rows
        .as_array()
        .unwrap()
        .iter()
        .filter(|r| r["source"] == source)
        .collect();
    assert!(matching.len() <= 1, "unique merge source view");
    let Some(row) = matching.first() else {
        return current.to_owned();
    };
    assert_eq!(
        format!(
            "{:x}",
            crate::digest_hex::LowerHex(Sha256::digest(current.as_bytes()))
        ),
        row["current_sha256"]
    );
    let mut result = current.to_owned();
    for edit in row["edits"].as_array().unwrap() {
        let new = edit["new"].as_str().unwrap();
        assert_eq!(
            result.matches(new).count(),
            1,
            "complete unique merge inverse"
        );
        result = result.replacen(new, edit["old"].as_str().unwrap(), 1);
    }
    assert_eq!(
        format!(
            "{:x}",
            crate::digest_hex::LowerHex(Sha256::digest(result.as_bytes()))
        ),
        row["original_sha256"]
    );
    result
}
// Exact post-build name/visibility repair, before every protected older domain.
// Unlisted sources remain unchanged; listed inputs must match their full new SHA.
fn before_build_fix(source: &str, current: &str) -> String {
    project(
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/docs/evidence/compiler-receiving-fix-7800-20261015/predecessor-projections.json"
        )),
        source,
        current,
    )
}
pub(crate) fn ours(source: &str, current: &str) -> String {
    let staged = project(
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/docs/evidence/source-composition-f813-held-20261015/pre-ours.json"
        )),
        source,
        current,
    );
    let current = staged.as_str();
    project(
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/docs/evidence/source36-main88884-merge-20261015/ours-projections.json"
        )),
        source,
        current,
    )
}
fn prior_main(source: &str, current: &str) -> String {
    let predecessor = assembled_previous(source, current);
    let current = predecessor.as_str();
    project(
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/docs/evidence/source-composition-f813-held-20261015/main-projections.json"
        )),
        source,
        current,
    )
}

pub(crate) fn held(source: &str, current: &str) -> String {
    let predecessor = assembled_previous(source, current);
    let current = predecessor.as_str();
    project(
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/docs/evidence/source-composition-f813-held-20261015/held-projections.json"
        )),
        source,
        current,
    )
}

pub(crate) fn weighted(path: &str, current: &str) -> String {
    let staged = if path == "../epsilon_alias/tests.rs" {
        final_ready_input(
            "d8",
            "src/cleanup_plan/replay/factored/canonical_shared/epsilon_alias/tests.rs",
            current,
        )
    } else {
        current.to_owned()
    };
    let current = staged.as_str();
    if path == "../../../../../../docs/ARCHITECTURE.md" {
        return assembled_previous("docs/ARCHITECTURE.md", current);
    }
    if path != "../epsilon_alias/tests.rs" {
        return current.to_owned();
    }
    project(
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/docs/evidence/source-composition-f813-held-20261015/weighted-projections.json"
        )),
        "src/cleanup_plan/replay/factored/canonical_shared/epsilon_alias/tests.rs",
        current,
    )
}

// NEXT842 source transport precedes every immutable runtime domain.
fn inline_weighted_previous(source: &str, current: &str) -> String {
    project(
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/docs/evidence/inline-weighted-successor-842/source-inverses.json"
        )),
        source,
        current,
    )
}

// Separate authenticated #843 predecessor; older runtime manifests stay immutable.
pub(crate) fn closure_previous(source: &str, current: &str) -> String {
    project(
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/docs/evidence/held-closure-receiver-843/runtime-projections.json"
        )),
        source,
        current,
    )
}

// Each isolated branch projector remains strict. Authenticate the actual final
// input separately into its named branch before comparing complete base bytes.
pub(crate) fn assembly_view(stage: &str, source: &str, current: &str) -> String {
    let previous = latest_previous(source, current);
    let current = previous.as_str();
    let data = match stage {
        "9d" => include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/docs/evidence/final-842-843-source-assembly-20261015/view-9d.json"
        )),
        "842" => include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/docs/evidence/final-842-843-source-assembly-20261015/view-842.json"
        )),
        "843" => include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/docs/evidence/final-842-843-source-assembly-20261015/view-843.json"
        )),
        _ => panic!("unknown immutable assembly source domain"),
    };
    project(data, source, current)
}
fn assembled_previous(source: &str, current: &str) -> String {
    let repaired = assembly_view("9d", source, current);
    let repaired = before_build_fix(source, &repaired);
    let inline = assembly_view("842", source, current);
    let inline = inline_weighted_previous(source, &inline);
    let closure = assembly_view("843", source, current);
    let closure = closure_previous(source, &closure);
    assert_eq!(
        repaired, inline,
        "independent branch sources must reach the same complete7800 input"
    );
    assert_eq!(
        repaired, closure,
        "held branch source must reach the same complete7800 input"
    );
    repaired
}

// Strict newest input, before the immutable9365 branch domains. No old raw input
// is accepted as an alternative to its complete current source.
fn latest_previous(source: &str, current: &str) -> String {
    let corrected = corrected_previous(source, current);
    let current = corrected.as_str();
    project(
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/docs/evidence/main7f-source-successor-20261015/current-to-9365.json"
        )),
        source,
        current,
    )
}
pub(crate) fn main(source: &str, current: &str) -> String {
    if source == "src/cleanup_plan/replay/skeleton_work/ordinary_dag/tests.rs" {
        return main_f045_input(source, current);
    }
    let branch = final_main_input(source, current);
    let physical = before_f045(source, &branch);
    let _prior = prior_main(source, current);
    let corrected = corrected_previous(source, current);
    let _historical_main7f = project(
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/docs/evidence/main7f-source-successor-20261015/current-to-main7f.json"
        )),
        source,
        &corrected,
    );
    project(
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/docs/evidence/mainff869-source-successor-20261015/current-to-mainff.json"
        )),
        source,
        &physical,
    )
}

// Actual corrective owners authenticate before the immutable d1bb source view.
fn corrected_previous(source: &str, current: &str) -> String {
    let baseline = consumer_previous(source, current);
    let current = baseline.as_str();
    project(
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/docs/evidence/d1bb-corrective-source-successor-20261015/current-to-d1bb.json"
        )),
        source,
        current,
    )
}
pub(crate) fn scalar_policy_input(source: &str, current: &str) -> String {
    let previous = before_main927(source, current);
    let current = previous.as_str();
    project(
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/docs/evidence/d1bb-corrective-source-successor-20261015/scalar-policy-inputs.json"
        )),
        source,
        current,
    )
}

// The consumer correction has one strict current→frozen3d4b predecessor.
fn consumer_previous(source: &str, current: &str) -> String {
    let predecessor = corrected_consumers_previous(source, current);
    let current = predecessor.as_str();
    project(
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/docs/evidence/source-consumer-closure-fixes-20261015/current-to-3d4b.json"
        )),
        source,
        current,
    )
}
pub(crate) fn consumer_input(domain: &str, source: &str, current: &str) -> String {
    let predecessor = corrected_consumers_previous(source, current);
    let current = predecessor.as_str();
    let data = match domain {
        "decl825" => include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/docs/evidence/source-consumer-closure-fixes-20261015/decl825.json"
        )),
        "cost825" => include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/docs/evidence/source-consumer-closure-fixes-20261015/cost825.json"
        )),
        "signature825" => include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/docs/evidence/source-consumer-closure-fixes-20261015/signature825.json"
        )),
        "capacity834" => include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/docs/evidence/source-consumer-closure-fixes-20261015/capacity834.json"
        )),
        "registry833" => include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/docs/evidence/source-consumer-closure-fixes-20261015/registry833.json"
        )),
        "receiving831" => include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/docs/evidence/source-consumer-closure-fixes-20261015/receiving831.json"
        )),
        "lexical832" => include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/docs/evidence/source-consumer-closure-fixes-20261015/lexical832.json"
        )),
        "cold835" => include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/docs/evidence/source-consumer-closure-fixes-20261015/cold835.json"
        )),
        _ => panic!("unknown complete source consumer domain"),
    };
    project(data, source, current)
}

fn corrected_consumers_previous(source: &str, current: &str) -> String {
    let fixes = final_fix_input(source, current);
    let fixes = project(
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/docs/evidence/ae91-source-consumer-corrections-20261015/current-to-ae91.json"
        )),
        source,
        &fixes,
    );
    let fixes = project(
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/docs/evidence/final-ae91-mainf045-assembly-20261015/fixes-ae91-reconcile.json"
        )),
        source,
        &fixes,
    );
    let main = final_main_input(source, current);
    let main = ff_previous(source, &main);
    assert_eq!(
        fixes, main,
        "independent final branches must reach the same complete ae91 input"
    );
    fixes
}

pub(crate) fn consumer_child(domain: &str, source: &str, current: &str) -> String {
    let branch = final_fix_input(source, current);
    let current = branch.as_str();
    let rows = match domain {
        "registry833" => include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/docs/evidence/ae91-source-consumer-corrections-20261015/registry833-children.json"
        )),
        "cold835" => include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/docs/evidence/ae91-source-consumer-corrections-20261015/cold835-children.json"
        )),
        "ordered838" => include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/docs/evidence/ae91-source-consumer-corrections-20261015/ordered838-observer.json"
        )),
        _ => panic!("unknown source child domain"),
    };
    project(rows, source, current)
}

fn final_fix_input(source: &str, current: &str) -> String {
    let previous = before_main927(source, current);
    let current = previous.as_str();
    project(
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/docs/evidence/final-ae91-mainf045-assembly-20261015/view-fixes.json"
        )),
        source,
        current,
    )
}
fn final_main_input(source: &str, current: &str) -> String {
    let previous = before_main927(source, current);
    let current = previous.as_str();
    project(
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/docs/evidence/final-ae91-mainf045-assembly-20261015/view-main.json"
        )),
        source,
        current,
    )
}
fn ff_previous(source: &str, current: &str) -> String {
    let predecessor = before_f045(source, current);
    let previous = project(
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/docs/evidence/mainff869-source-successor-20261015/current-to-ae91.json"
        )),
        source,
        &predecessor,
    );
    project(
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/docs/evidence/final-ae91-mainf045-assembly-20261015/main-ae91-reconcile.json"
        )),
        source,
        &previous,
    )
}
// MAIN's exact guards retain their complete immutable f045 physical domain.
pub(crate) fn main_f045_input(source: &str, current: &str) -> String {
    let branch = final_main_input(source, current);
    project(
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/docs/evidence/mainf045-source-successor-20261015/current-to-f045.json"
        )),
        source,
        &branch,
    )
}
fn before_f045(source: &str, current: &str) -> String {
    project(
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/docs/evidence/mainf045-source-successor-20261015/current-to-0bf.json"
        )),
        source,
        current,
    )
}

// Preserve the complete frozen9b domains independently of the newer MAIN proof.
fn before_main927(source: &str, current: &str) -> String {
    let previous = before_setup825(source, current);
    let current = previous.as_str();
    let main_view = assembly_main927_input(source, current);
    let hex_view = project(
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/docs/evidence/main927-hex-starter-assembly-20261015/view-hex.json"
        )),
        source,
        current,
    );
    let hex_previous = facade_hex_previous(source, &hex_view);
    let previous = main927_previous(source, &main_view);
    assert_eq!(
        previous, hex_previous,
        "complete independent predecessor: {source}"
    );
    previous
}
fn main927_previous(source: &str, current: &str) -> String {
    project(
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/docs/evidence/main927-source-successor-20261015/current-to-9b.json"
        )),
        source,
        current,
    )
}
// Only these named inputs may enter MAIN's unchanged semantic source proofs.
pub(crate) fn main927_input(source: &str, current: &str) -> String {
    let previous = before_setup825(source, current);
    let current = previous.as_str();
    let view = assembly_main927_input(source, current);
    let current = view.as_str();
    project(
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/docs/evidence/main927-source-successor-20261015/current-to-main927.json"
        )),
        source,
        current,
    )
}

// Compiler-confirmed facade digest correction precedes immutable9b source views.
fn facade_hex_previous(source: &str, current: &str) -> String {
    project(
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/docs/evidence/workspace-facade-hex-fix-9b-20261015/current-to-9b.json"
        )),
        source,
        current,
    )
}

// Independent complete branch inputs; neither branch is fed the other's view.
fn assembly_main927_input(source: &str, current: &str) -> String {
    project(
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/docs/evidence/main927-hex-starter-assembly-20261015/view-main927.json"
        )),
        source,
        current,
    )
}

// Authenticated physical successor for the bounded preowned-setup/starter batch.
fn before_setup825(source: &str, current: &str) -> String {
    let previous = before_setup_reference_controls(source, current);
    let current = previous.as_str();
    project(
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/docs/evidence/setup825-relative-starter-assembly-20261015/view-d4.json"
        )),
        source,
        current,
    )
}
// Preserve the complete immutable typed825 runtime manifest's own exact domain.
pub(crate) fn setup_typed_input(source: &str, current: &str) -> String {
    let previous = before_setup_reference_controls(source, current);
    let current = previous.as_str();
    project(
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/docs/evidence/setup825-relative-starter-assembly-20261015/view-typed825.json"
        )),
        source,
        current,
    )
}

// Corrected physical receiving fees precede the immutable initial setup views.
fn before_setup_reference_controls(source: &str, current: &str) -> String {
    let previous = before_region_status_view(source, current);
    let current = previous.as_str();
    project(
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/docs/evidence/setup825-reference-controls-assembly-20261015/current-to-9cb.json"
        )),
        source,
        current,
    )
}

// Full physical successor; the prior receiving correction domain stays exact.
fn before_region_status_view(source: &str, current: &str) -> String {
    let previous = before_constructor_complete_views(source, current);
    let current = previous.as_str();
    project(
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/docs/evidence/region-status-import-view-d4-20261015/current-to-a0c.json"
        )),
        source,
        current,
    )
}

fn before_constructor_complete_views(source: &str, current: &str) -> String {
    let previous = before_current_main(source, current);
    let current = previous.as_str();
    project(
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/docs/evidence/constructor-complete-runtime-views-d4-20261015/current-to-aa03.json"
        )),
        source,
        current,
    )
}
// These two complete manifests describe physical e4b inputs, not CI reflows.
pub(crate) fn constructor_complete_input(source: &str, current: &str) -> String {
    let previous = before_current_main(source, current);
    let current = previous.as_str();
    project(
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/docs/evidence/constructor-complete-runtime-views-d4-20261015/runtime-views.json"
        )),
        source,
        current,
    )
}

// New MAIN/fixture bytes authenticate into the complete previous private batch.
fn before_current_main(source: &str, current: &str) -> String {
    let previous = before_fixture841(source, current);
    let current = previous.as_str();
    project(
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/docs/evidence/main5ba-future-source-assembly-20261015/current-to-1c.json"
        )),
        source,
        current,
    )
}
// MAIN's refusal-site source proofs retain their actual5ba physical domain.
pub(crate) fn main_current_input(source: &str, current: &str) -> String {
    let previous = before_fixture841(source, current);
    let current = previous.as_str();
    project(
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/docs/evidence/main5ba-future-source-assembly-20261015/current-to-main5ba.json"
        )),
        source,
        current,
    )
}

// Independent branches: inherited guards see13d; the pilot sees its exact5f input.
fn before_fixture841(source: &str, current: &str) -> String {
    let previous = before_retained833(source, current);
    let current = previous.as_str();
    project(
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/docs/evidence/compressed-fixture-841-assembly-20261015/current-to-13d.json"
        )),
        source,
        current,
    )
}
pub(crate) fn compressed_fixture_input(source: &str, current: &str) -> String {
    let previous = before_retained833(source, current);
    let current = previous.as_str();
    project(
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/docs/evidence/compressed-fixture-841-assembly-20261015/pilot-runtime-views.json"
        )),
        source,
        current,
    )
}

// The retained registry owner and inherited guards have distinct whole inputs.
fn before_retained833(source: &str, current: &str) -> String {
    let previous = before_main66517(source, current);
    let current = previous.as_str();
    project(
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/docs/evidence/retained-registry-833-assembly-20261015/current-to-381.json"
        )),
        source,
        current,
    )
}
pub(crate) fn retained833_input(source: &str, current: &str) -> String {
    let previous = before_main66517(source, current);
    let current = previous.as_str();
    project(
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/docs/evidence/retained-registry-833-assembly-20261015/current-to-c3.json"
        )),
        source,
        current,
    )
}

// Latest MAIN source enters the immutable combined833/841 checkpoint first.
fn before_main66517(source: &str, current: &str) -> String {
    let previous = before_retry_guard(source, current);
    let current = previous.as_str();
    project(
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/docs/evidence/main66517-source-assembly-20261015/current-to-397.json"
        )),
        source,
        current,
    )
}

// Preserve the complete old runtime input before every inherited source domain.
fn before_retry_guard(source: &str, current: &str) -> String {
    let previous = before_direct_carriers831(source, current);
    let current = previous.as_str();
    project(
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/docs/evidence/retry-fixture-full-source-view-61f-20261015/current-to-61f.json"
        )),
        source,
        current,
    )
}
pub(crate) fn retry_fixture_input(source: &str, current: &str) -> String {
    let previous = before_direct_carriers831(source, current);
    let current = previous.as_str();
    project(
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/docs/evidence/retry-fixture-full-source-view-61f-20261015/runtime-view.json"
        )),
        source,
        current,
    )
}

// Every older runtime source guard retains the exact297 physical domain.
fn before_direct_carriers831(source: &str, current: &str) -> String {
    let observed = before_cached_acquisition_observer(source, current);
    let current = observed.as_str();
    project(
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/docs/evidence/direct-canonical-carriers-831-20261015/current-to-297.json"
        )),
        source,
        current,
    )
}

// Test-only observation successor; exact prior runtime tables remain immutable.
fn before_cached_acquisition_observer(source: &str, current: &str) -> String {
    let staged = before_sourceguard_inputs(source, current);
    let current = staged.as_str();
    project(
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/docs/evidence/cached-prelude-acquisition-observer-7a87/current-to-7a87.json"
        )),
        source,
        current,
    )
}

// Preserve complete physical7a87 inputs before the cached observer's older view.
fn before_sourceguard_inputs(source: &str, current: &str) -> String {
    let staged = before_actual_pin_consumers(source, current);
    let current = staged.as_str();
    project(
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/docs/evidence/sourceguard-input-domains-7a87/current-to-d20.json"
        )),
        source,
        current,
    )
}
pub(crate) fn source36_classifier_input(source: &str, current: &str) -> String {
    project(
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/docs/evidence/sourceguard-input-domains-7a87/classifier-input.json"
        )),
        source,
        current,
    )
}

// Actual producer literals are transported only for historical full-source guards.
fn before_actual_pin_consumers(source: &str, current: &str) -> String {
    let current = final_ready_input("d8", source, current);
    let current = current.as_str();
    project(
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/docs/evidence/actual-pin-consumer-7a87-20261015/current-to-92.json"
        )),
        source,
        current,
    )
}

// Independent physical-input domains: no checkpoint table accepts another stage.
fn final_ready_input(stage: &str, source: &str, current: &str) -> String {
    let current = project(
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/docs/evidence/field-update-hir-visitor-836-20261015/current-to-677.json"
        )),
        source,
        current,
    );
    let current = current.as_str();
    let data = match stage {
        "d8" => include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/docs/evidence/ready-main-844-846-836-assembly-20261015/final-to-d8.json"
        )),
        "844" => include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/docs/evidence/ready-main-844-846-836-assembly-20261015/final-to-844.json"
        )),
        "846" => include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/docs/evidence/ready-main-844-846-836-assembly-20261015/final-to-846.json"
        )),
        _ => panic!("unknown exact ready checkpoint"),
    };
    project(data, source, current)
}
pub(crate) fn compact844_current_input(source: &str, current: &str) -> String {
    final_ready_input("844", source, current)
}
pub(crate) fn compact844_input(source: &str, current: &str) -> String {
    let current = final_ready_input("844", source, current);
    project(
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/docs/evidence/compact-automaton-844-immediate7a87/current-to-a3.json"
        )),
        source,
        &current,
    )
}
pub(crate) fn diagnostic846_input(source: &str, current: &str) -> String {
    final_ready_input("846", source, current)
}
