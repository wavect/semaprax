use super::super::model::Report;
use super::api::MIN_OUTPUT_BYTES;
use super::*;
use crate::package_lock_v2;
use std::collections::{BTreeMap, BTreeSet};
use std::sync::atomic::{AtomicU64, Ordering};

fn report_from_source(tag: &str, source: &str) -> String {
    static NEXT: AtomicU64 = AtomicU64::new(0);
    let path = std::env::temp_dir().join(format!(
        "semaprax-package-compatibility-{}-{}-{tag}.spx",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    std::fs::write(&path, source).unwrap();
    let report = crate::package_report_v2::generate(
        &path,
        &crate::package_report_v2::PackageReportV2Options::default(),
    );
    std::fs::remove_file(path).unwrap();
    report.unwrap()
}

fn input_for(report: &str, version: &str, capabilities: &[&str]) -> CompatibilityInput {
    let value: serde_json::Value = serde_json::from_str(report).unwrap();
    let package = value["payload"]["package"]["name"]
        .as_str()
        .unwrap()
        .to_owned();
    let coordinate = package_lock_v2::Coordinate {
        package,
        version: version.to_owned(),
    };
    let capabilities = capabilities
        .iter()
        .map(|value| (*value).to_owned())
        .collect::<Vec<_>>();
    let subject = package_lock_v2::create_subject(&coordinate, report, &[], &capabilities).unwrap();
    let lock = package_lock_v2::generate(
        std::slice::from_ref(&subject),
        &package_lock_v2::LockOptions::default(),
    )
    .unwrap();
    CompatibilityInput {
        coordinate,
        report: report.to_owned(),
        lock,
        lock_subjects: vec![subject],
    }
}

fn input_with_dependency(
    report: &str,
    version: &str,
    dependency_report: &str,
) -> CompatibilityInput {
    let value: serde_json::Value = serde_json::from_str(report).unwrap();
    let dependency_value: serde_json::Value = serde_json::from_str(dependency_report).unwrap();
    let coordinate = package_lock_v2::Coordinate {
        package: value["payload"]["package"]["name"]
            .as_str()
            .unwrap()
            .to_owned(),
        version: version.to_owned(),
    };
    let dependency = package_lock_v2::Coordinate {
        package: dependency_value["payload"]["package"]["name"]
            .as_str()
            .unwrap()
            .to_owned(),
        version: "1.0.0".to_owned(),
    };
    let dependency_subject =
        package_lock_v2::create_subject(&dependency, dependency_report, &[], &[]).unwrap();
    let subject = package_lock_v2::create_subject(
        &coordinate,
        report,
        std::slice::from_ref(&dependency),
        &[],
    )
    .unwrap();
    let lock_subjects = vec![subject, dependency_subject];
    let lock = package_lock_v2::generate(&lock_subjects, &package_lock_v2::LockOptions::default())
        .unwrap();
    CompatibilityInput {
        coordinate,
        report: report.to_owned(),
        lock,
        lock_subjects,
    }
}

#[test]
fn outcome_precedence_and_option_boundaries_are_closed() {
    assert!(CompatibilityOptions::new(MIN_OUTPUT_BYTES).is_ok());
    assert!(CompatibilityOptions::new(MIN_OUTPUT_BYTES - 1).is_err());
    assert!(CompatibilityOptions::new(MAX_OUTPUT_BYTES).is_ok());
    assert!(CompatibilityOptions::new(MAX_OUTPUT_BYTES + 1).is_err());
    let mut findings = Vec::new();
    let mut breaking = true;
    let mut indeterminate = false;
    compare_targets(
        &BTreeMap::from([("native64".to_owned(), "available".to_owned())]),
        &BTreeMap::from([("native64".to_owned(), "unproven".to_owned())]),
        &mut findings,
        &mut breaking,
        &mut indeterminate,
    )
    .unwrap();
    assert!(breaking && indeterminate);
}

#[test]
fn nested_primitive_name_change_is_breaking_not_scrubbed() {
    let export = |primitive: &str| {
        serde_json::json!({
            "name":"f","parameters":[{"index":0,"name":"value","type":{"kind":"nominal","declaration":"row","arguments":[]},"ownership":"value"}],
            "result":{"type":{"kind":"primitive","name":primitive},"ownership":"value"},"effects":[],"requires":[],"ensures":[]
        })
    };
    let mut findings = Vec::new();
    let mut breaking = false;
    compare_export(
        "f",
        &export("i64"),
        &export("i32"),
        &mut findings,
        &mut breaking,
    )
    .unwrap();
    assert!(breaking);
}

#[test]
fn exact_selected_subjects_replay_to_compatible_evidence() {
    let report = crate::package_report_v2::generate(
        std::path::Path::new("examples/meaning.spx"),
        &crate::package_report_v2::PackageReportV2Options::default(),
    )
    .unwrap();
    let make = |version: &str| {
        let coordinate = package_lock_v2::Coordinate {
            package: "examples.meaning".to_owned(),
            version: version.to_owned(),
        };
        let subject = package_lock_v2::create_subject(&coordinate, &report, &[], &[]).unwrap();
        let lock = package_lock_v2::generate(
            std::slice::from_ref(&subject),
            &package_lock_v2::LockOptions::default(),
        )
        .unwrap();
        CompatibilityInput {
            coordinate,
            report: report.clone(),
            lock,
            lock_subjects: vec![subject],
        }
    };
    let base = make("1.0.0");
    let candidate = make("1.1.0");
    let evidence = generate(&base, &candidate, &CompatibilityOptions::default()).unwrap();
    assert_eq!(
        verify(
            &evidence,
            &base,
            &candidate,
            &CompatibilityOptions::default()
        )
        .unwrap()
        .outcome,
        "compatible"
    );
}

#[test]
fn contract_only_nominal_type_is_in_shared_reachable_closure() {
    let export = serde_json::json!({"parameters":[],"result":{},"requires":[{"fact":{"type":{"kind":"nominal","declaration":"contract.only","arguments":[]}}}],"ensures":[]});
    let report = Report {
        exports: BTreeMap::from([("f".to_owned(), export)]),
        types: BTreeMap::from([(
            "contract.only".to_owned(),
            serde_json::json!({"stable_id":"contract.only","definition":{"kind":"record","fields":[]}}),
        )]),
        targets: BTreeMap::new(),
        unproven: false,
        call_contract: false,
        imported_resource: false,
    };
    assert!(
        reachable_shared_types(&BTreeSet::from(["f".to_owned()]), &report)
            .contains("contract.only")
    );
}

#[test]
fn type_display_scrub_keeps_primitive_semantics() {
    let row = |display: &str, primitive: &str| serde_json::json!({"stable_id":"row","name":display,"type_parameters":[],"definition":{"kind":"record","fields":[{"id":"field","name":display,"index":0,"type":{"kind":"primitive","name":primitive}}]}});
    assert_eq!(
        scrub_type_display(row("Old", "i64")),
        scrub_type_display(row("New", "i64"))
    );
    assert_ne!(
        scrub_type_display(row("Old", "i64")),
        scrub_type_display(row("New", "i32"))
    );
}

#[test]
fn selected_report_mismatch_and_context_drift_are_fail_closed() {
    let meaning = crate::package_report_v2::generate(
        std::path::Path::new("examples/meaning.spx"),
        &crate::package_report_v2::PackageReportV2Options::default(),
    )
    .unwrap();
    let calculator = crate::package_report_v2::generate(
        std::path::Path::new("examples/calculator.spx"),
        &crate::package_report_v2::PackageReportV2Options::default(),
    )
    .unwrap();
    let base = input_for(&meaning, "1.0.0", &[]);
    let mut mismatch = base.clone();
    mismatch.report = calculator;
    assert_eq!(
        generate(&mismatch, &base, &CompatibilityOptions::default()).unwrap_err()[0].code,
        "SPX-PC502"
    );
    let drift = input_for(&meaning, "1.1.0", &["network"]);
    let evidence = generate(&base, &drift, &CompatibilityOptions::default()).unwrap();
    assert_eq!(
        verify(&evidence, &base, &drift, &CompatibilityOptions::default())
            .unwrap()
            .outcome,
        "indeterminate"
    );
}

#[test]
fn evidence_outer_remint_cannot_forge_outcome() {
    let report = crate::package_report_v2::generate(
        std::path::Path::new("examples/meaning.spx"),
        &crate::package_report_v2::PackageReportV2Options::default(),
    )
    .unwrap();
    let base = input_for(&report, "1.0.0", &[]);
    let candidate = input_for(&report, "1.1.0", &[]);
    let evidence = generate(&base, &candidate, &CompatibilityOptions::default()).unwrap();
    let marker = "\"payload\":";
    let offset = evidence.find(marker).unwrap() + marker.len();
    let payload = &evidence[offset..evidence.len() - 1];
    let forged = super::super::wire::render_wrapper(&payload.replacen(
        "\"outcome\":\"compatible\"",
        "\"outcome\":\"breaking\"",
        1,
    ));
    assert_eq!(
        verify(&forged, &base, &candidate, &CompatibilityOptions::default())
            .unwrap_err()
            .code,
        "SPX-PC505"
    );
}

#[test]
fn aggregate_dependency_unproven_is_publicly_indeterminate_and_subject_order_is_canonical() {
    let root = crate::package_report_v2::generate(
        std::path::Path::new("examples/meaning.spx"),
        &crate::package_report_v2::PackageReportV2Options::default(),
    )
    .unwrap();
    let dependency = crate::package_report_v2::generate(
        std::path::Path::new("examples/calculator-rust/callback.spx"),
        &crate::package_report_v2::PackageReportV2Options::default(),
    )
    .unwrap();
    assert!(dependency.contains("\"status\":\"unproven\""));
    let base = input_with_dependency(&root, "1.0.0", &dependency);
    let candidate = input_with_dependency(&root, "1.1.0", &dependency);
    let evidence = generate(&base, &candidate, &CompatibilityOptions::default()).unwrap();
    assert_eq!(
        verify(
            &evidence,
            &base,
            &candidate,
            &CompatibilityOptions::default()
        )
        .unwrap()
        .outcome,
        "indeterminate"
    );
    let mut reordered = candidate.clone();
    reordered.lock_subjects.reverse();
    assert_eq!(
        generate(&base, &reordered, &CompatibilityOptions::default()).unwrap(),
        evidence
    );
}

#[test]
fn contract_calls_and_imported_resources_are_publicly_indeterminate() {
    let call_report = crate::package_report_v2::generate(
        std::path::Path::new("examples/bytes_u8.spx"),
        &crate::package_report_v2::PackageReportV2Options::default(),
    )
    .unwrap();
    assert!(call_report.contains("\"kind\":\"call\""));
    let call_base = input_for(&call_report, "1.0.0", &[]);
    let call_candidate = input_for(&call_report, "1.1.0", &[]);
    let call_evidence = generate(
        &call_base,
        &call_candidate,
        &CompatibilityOptions::default(),
    )
    .unwrap();
    assert_eq!(
        verify(
            &call_evidence,
            &call_base,
            &call_candidate,
            &CompatibilityOptions::default()
        )
        .unwrap()
        .outcome,
        "indeterminate"
    );

    let imported_report = report_from_source(
        "imported-resource",
        r#"module compatibility.imported;
permit { filesystem.handle.release }
@id("io.file")
resource File { @id("io.file.drop") drop import "io.file.finalize"; }
@id("io.file.host")
interface FileHost permits { filesystem.handle.release } {
    @id("io.file.finalize")
    import fn finalize(file: own File) -> unit
        effects { filesystem.handle.release }
        failure infallible
        consumes file always;
}
@id("io.consume")
fn consume(file: own File) -> i64 uses { filesystem.handle.release } { 1 }
@id("app.main")
fn main() -> i64 { 0 }
"#,
    );
    assert!(imported_report.contains("\"kind\":\"imported\""));
    let imported_base = input_for(&imported_report, "1.0.0", &[]);
    let imported_candidate = input_for(&imported_report, "1.1.0", &[]);
    let imported_evidence = generate(
        &imported_base,
        &imported_candidate,
        &CompatibilityOptions::default(),
    )
    .unwrap();
    assert_eq!(
        verify(
            &imported_evidence,
            &imported_base,
            &imported_candidate,
            &CompatibilityOptions::default()
        )
        .unwrap()
        .outcome,
        "indeterminate"
    );
}

#[test]
fn finding_and_public_output_bounds_fail_with_stable_limit_diagnostic() {
    let mut findings = Vec::new();
    for index in 0..MAX_FINDINGS {
        push(
            &mut findings,
            "informational",
            "display",
            &index.to_string(),
            "before",
            "after",
            "display_name_changed",
        )
        .unwrap();
    }
    assert_eq!(
        push(
            &mut findings,
            "informational",
            "display",
            "overflow",
            "before",
            "after",
            "display_name_changed",
        )
        .unwrap_err()
        .code,
        "SPX-PC503"
    );

    let source = |prefix: &str| {
        let mut source = String::from("module compatibility.output_bound;\n");
        for index in 0..48 {
            source.push_str(&format!(
                "@id(\"compatibility.function.{index}\")\nfn {prefix}_{index}() -> i64 {{ {index} }}\n"
            ));
        }
        source.push_str("@id(\"app.main\")\nfn main() -> i64 { 0 }\n");
        source
    };
    let base_report = report_from_source("output-base", &source("before"));
    let candidate_report = report_from_source("output-candidate", &source("after"));
    let base = input_for(&base_report, "1.0.0", &[]);
    let candidate = input_for(&candidate_report, "1.1.0", &[]);
    assert_eq!(
        generate(
            &base,
            &candidate,
            &CompatibilityOptions::new(MIN_OUTPUT_BYTES).unwrap()
        )
        .unwrap_err()[0]
            .code,
        "SPX-PC503"
    );
}
