//! The separation gate of the Public Generic Ownership milestone.
//!
//! The milestone exists because a public generic surface is not a consequence
//! of admitting a generic inside a body. This module is the executable half of
//! that claim: it selects generic templates, concrete generic instances, and an
//! owned generic parameter through every public projection the repository has
//! today, and pins the exact closed reason each one refuses with.
//!
//! An accidental widening — an internal generic admission that starts producing
//! a public signature, descriptor, header, or Wasm export edge — reddens here
//! instead of becoming a silent public claim. The charter in
//! `docs/PUBLIC-GENERIC-OWNERSHIP-MILESTONE-V1.md` owns the prerequisite gates
//! and the standing support decision; the last case pins that the charter still
//! says unsupported and unpublished, so the document cannot drift into one.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use semaprax::verify::verify;
use semaprax::{abi_report, c_header, hir, parse, wasm};
use serde_json::Value;

static NEXT_ID: AtomicU64 = AtomicU64::new(0);

const MILESTONE: &str = "docs/PUBLIC-GENERIC-OWNERSHIP-MILESTONE-V1.md";

/// One template, one concrete instance result, and one owned generic parameter.
/// Each declaration is admitted by `check`, so every rejection below is the
/// public projection's decision rather than a source or HIR failure.
const SOURCE: &str = r#"
module test.public_generic_boundary;

@id("boundary.pair")
record Pair<T, U> {
    @id("boundary.pair.left")
    left: T,
    @id("boundary.pair.right")
    right: U,
}

@id("boundary.relay")
fn relay<T>(value: own Pair<Bytes, T>) -> Pair<Bytes, T> { value }

@id("boundary.produce")
fn produce() -> Pair<Bytes, bool> {
    let source = [1u8, 2u8];
    Pair<Bytes, bool> {
        left: bytes_copy(array_as_slice(source)),
        right: true,
    }
}

@id("boundary.consume")
fn consume(value: own Pair<Bytes, bool>) -> i64 {
    match own value {
        Pair { left: payload, right: present } =>
            if present && byte_len(bytes_as_slice(payload)) > 0usize { 1 } else { 0 },
    }
}

@id("boundary.scalar")
fn scalar(value: i64) -> i64 { value }

@id("app.main")
fn main() -> i64 { 0 }
"#;

/// The same scalar export, with no generic declaration anywhere in the module.
const SCALAR_SOURCE: &str = r#"
module test.public_generic_boundary_scalar;

@id("boundary.scalar")
fn scalar(value: i64) -> i64 { value }

@id("app.main")
fn main() -> i64 { 0 }
"#;

/// Every selected identity, paired with the closed reason the report-shaped
/// projections record for it. `own` is refused as a mode before its type is
/// examined; that is the recorded fact, not an approximation of one.
const RECORDED_EXCLUSIONS: &[(&str, &str)] = &[
    ("boundary.relay", "generic_function"),
    ("boundary.produce", "unsupported_result_type"),
    ("boundary.consume", "unsupported_parameter_mode"),
];

fn source_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).to_path_buf()
}

/// A distinct path per case: the projections keep source identity in their
/// envelopes, so two cases must not share one.
fn write_source(label: &str) -> PathBuf {
    let root = std::env::temp_dir().join(format!(
        "spx-public-generic-milestone-{}-{}-{label}",
        std::process::id(),
        NEXT_ID.fetch_add(1, Ordering::Relaxed)
    ));
    std::fs::create_dir_all(&root).unwrap();
    let path = root.join("boundary.spx");
    std::fs::write(&path, SOURCE).unwrap();
    path
}

fn selection(ids: &[&str]) -> Vec<String> {
    ids.iter().map(|id| (*id).to_owned()).collect()
}

fn payload(envelope: &str) -> Value {
    let value: Value = serde_json::from_str(envelope).unwrap();
    value["payload"].clone()
}

fn exclusion_reasons(payload: &Value) -> Vec<(String, String)> {
    payload["exclusions"]
        .as_array()
        .unwrap()
        .iter()
        .map(|row| {
            (
                row["stable_id"].as_str().unwrap().to_owned(),
                row["reason"].as_str().unwrap().to_owned(),
            )
        })
        .collect()
}

/// The generic surface must reach the projections as a verified program, or a
/// rejection below would prove nothing about the public boundary.
#[test]
fn the_selected_generic_surface_is_an_admitted_program() {
    let path = write_source("admitted");
    let parsed = parse(SOURCE, &path).unwrap();
    let resolved = hir::resolve(&parsed).unwrap();
    assert!(
        verify(&parsed).is_empty(),
        "the fixture must verify cleanly"
    );
    let mut ids = resolved
        .functions
        .iter()
        .map(|function| function.id.as_str().to_owned())
        .collect::<Vec<_>>();
    ids.extend(
        resolved
            .function_templates
            .iter()
            .map(|template| template.id.as_str().to_owned()),
    );
    for (id, _) in RECORDED_EXCLUSIONS {
        assert!(
            ids.iter().any(|candidate| candidate == id),
            "{id} must be a checked declaration"
        );
    }
    assert!(
        resolved
            .types
            .iter()
            .any(|declaration| declaration.id.as_str() == "boundary.pair"),
        "the authored record template must be a checked declaration"
    );
}

/// The canonical ABI report admits the scalar function beside the generic
/// selections and excludes each generic one with its recorded closed reason.
#[test]
fn the_abi_report_excludes_every_generic_selection() {
    let path = write_source("abi-report");
    let mut requested = RECORDED_EXCLUSIONS
        .iter()
        .map(|(id, _)| *id)
        .collect::<Vec<_>>();
    requested.push("boundary.scalar");
    let options = abi_report::AbiReportOptions::new(selection(&requested), 64 * 1024).unwrap();
    let payload = payload(&abi_report::generate(&path, &options).unwrap());

    assert_eq!(payload["selection"]["admitted"], 1);
    assert_eq!(payload["selection"]["excluded"], 3);
    assert_eq!(payload["functions"].as_array().unwrap().len(), 1);
    assert_eq!(
        payload["functions"][0]["stable_id"], "boundary.scalar",
        "only the monomorphic scalar function may be admitted"
    );
    let reasons = exclusion_reasons(&payload);
    for (id, reason) in RECORDED_EXCLUSIONS {
        assert!(
            reasons
                .iter()
                .any(|(row, value)| row == id && value == reason),
            "{id} must be excluded with {reason}, got {reasons:?}"
        );
    }
    let rendered = serde_json::to_string(&payload).unwrap();
    assert!(
        !rendered.contains("boundary.pair"),
        "no template identity may appear in a public report: {rendered}"
    );
}

/// C header emission shares the admission profile, so it must share the
/// refusals, and the emitted header must carry no template material at all.
#[test]
fn c_header_emission_excludes_every_generic_selection() {
    let path = write_source("c-header");
    for (id, reason) in RECORDED_EXCLUSIONS {
        let options = c_header::CHeaderOptions::new(selection(&[id]), 64 * 1024).unwrap();
        let payload = payload(&c_header::generate(&path, &options).unwrap());
        assert_eq!(payload["selection"]["admitted"], 0, "{id} must be excluded");
        assert_eq!(
            exclusion_reasons(&payload),
            vec![((*id).to_owned(), (*reason).to_owned())]
        );
        let header = payload["header"].as_str().unwrap();
        assert!(
            !header.contains("boundary.pair") && !header.contains("Pair"),
            "{id} produced a header mentioning the template: {header}"
        );
        assert!(
            header.contains("Admitted functions: 0"),
            "{id} must emit an empty header"
        );
    }
}

/// The Wasm scalar export edge is the only public Wasm export profile today.
/// Every generic selection must fail closed on its profile diagnostics.
///
/// The positive control is a separate module. This profile requires an exact
/// internal generic closure across the whole module, so the fixture's
/// never-instantiated template refuses even a monomorphic scalar selection —
/// a stricter fact than the per-selection refusals, and the reason the scalar
/// control cannot be taken from the same module.
#[test]
fn wasm_scalar_exports_reject_every_generic_selection() {
    let path = write_source("wasm");
    let parsed = parse(SOURCE, &path).unwrap();
    let resolved = hir::resolve(&parsed).unwrap();
    for (id, _) in RECORDED_EXCLUSIONS {
        let error = wasm::emit_resolved_module_with_scalar_exports(&resolved, &selection(&[id]))
            .expect_err(&format!("{id} must stay outside the scalar export profile"));
        assert!(
            matches!(error.code, "SPX-W115" | "SPX-W116"),
            "{id} rejected with {}",
            error.code
        );
    }
    let error =
        wasm::emit_resolved_module_with_scalar_exports(&resolved, &selection(&["boundary.scalar"]))
            .expect_err("an inexact internal generic closure refuses the whole module");
    assert_eq!(error.code, "SPX-W115");

    let scalar_path = path.with_file_name("scalar.spx");
    std::fs::write(&scalar_path, SCALAR_SOURCE).unwrap();
    let parsed = parse(SCALAR_SOURCE, &scalar_path).unwrap();
    let resolved = hir::resolve(&parsed).unwrap();
    wasm::emit_resolved_module_with_scalar_exports(&resolved, &selection(&["boundary.scalar"]))
        .expect("the same scalar export is admitted from a generic-free module");
}

/// The charter is part of the gate. Nine prerequisite gates must be named, the
/// standing decision must still be unsupported and unpublished, and no row may
/// claim a passing state while this module is the milestone's only evidence.
#[test]
fn the_milestone_charter_still_records_an_undecided_unsupported_surface() {
    let text = std::fs::read_to_string(source_root().join(MILESTONE)).unwrap();
    for index in 1..=9u32 {
        assert!(
            text.contains(&format!("| PG-{index} |")),
            "the charter must state gate PG-{index}"
        );
    }
    assert!(
        text.contains("**public generic ownership is not supported and not\npublished.**"),
        "the charter must keep its standing support and publication decision"
    );
    assert_eq!(
        text.matches("| Open |").count(),
        9,
        "every prerequisite gate is Open until its owning artifact records \
         passing evidence for an exact implementation commit"
    );
}
