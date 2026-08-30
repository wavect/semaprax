use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use sha2::{Digest as _, Sha256};

use super::*;

fn remint(schema: &str, domain: &[u8], payload: &str) -> String {
    let digest = test_digest(domain, payload.as_bytes());
    format!(
        "{{\"schema\":{},\"digest\":{},\"bytes\":{},\"payload\":{}}}",
        crate::diagnostic::quote_json(schema),
        crate::diagnostic::quote_json(&digest),
        payload.len(),
        payload
    )
}

fn test_digest(domain: &[u8], bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(domain);
    hasher.update((bytes.len() as u64).to_le_bytes());
    hasher.update(bytes);
    format!(
        "sha256:{:x}",
        crate::digest_hex::LowerHex(hasher.finalize()),
    )
}

fn replace_subject_report(subject: &str, report: &str) -> String {
    const PAYLOAD: &str = "\"payload\":";
    const REPORT: &str = "\"report\":";
    const END: &str = ",\"dependencies\":";
    let payload_offset = subject.find(PAYLOAD).unwrap() + PAYLOAD.len();
    let payload = &subject[payload_offset..subject.len() - 1];
    let report_offset = payload.find(REPORT).unwrap() + REPORT.len();
    let report_end = payload[report_offset..].find(END).unwrap() + report_offset;
    let old_report = &payload[report_offset..report_end];
    let value: serde_json::Value = serde_json::from_str(payload).unwrap();
    let old_digest = value["report_digest"].as_str().unwrap();
    let new_digest = test_digest(
        b"semaprax.offline-semantic-package-report.v2\0",
        report.as_bytes(),
    );
    let rebound = payload
        .replacen(old_report, report, 1)
        .replacen(old_digest, &new_digest, 1)
        .replacen(
            &format!("\"report_bytes\":{}", old_report.len()),
            &format!("\"report_bytes\":{}", report.len()),
            1,
        );
    remint(
        package_lock_v3::SUBJECT_SCHEMA,
        b"semaprax.offline-semantic-package-subject.v3\0",
        &rebound,
    )
}

fn report(path: &str) -> String {
    crate::package_report_v2::generate(
        Path::new(path),
        &crate::package_report_v2::PackageReportV2Options::default(),
    )
    .expect("v2 report fixture")
}

fn subject(
    package: &str,
    version: &str,
    report: &str,
    dependencies: &[package_lock_v3::Coordinate],
    capabilities: &[&str],
) -> String {
    package_lock_v3::create_subject(
        &package_lock_v3::Coordinate {
            package: package.to_owned(),
            version: version.to_owned(),
        },
        report,
        &dependencies
            .iter()
            .map(|d| package_lock_v3::DependencyRequirement {
                package: d.package.clone(),
                range: format!("={}", d.version),
            })
            .collect::<Vec<_>>(),
        &capabilities
            .iter()
            .map(|value| (*value).to_owned())
            .collect::<Vec<_>>(),
    )
    .expect("v2 subject fixture")
}

fn input(subjects: Vec<String>, range: &str) -> ResolutionInput {
    ResolutionInput {
        requirements: vec![Requirement {
            package: "examples.meaning".to_owned(),
            range: range.to_owned(),
        }],
        subjects,
        target: "native64".to_owned(),
        allowed_capabilities: vec![],
    }
}

#[test]
fn options_and_semver_boundaries_are_closed() {
    assert!(ResolutionOptions::new(MIN_OUTPUT_BYTES).is_ok());
    assert_eq!(
        ResolutionOptions::new(MIN_OUTPUT_BYTES - 1)
            .unwrap_err()
            .code,
        "SPX-PR601"
    );
    assert!(ResolutionOptions::new(MAX_OUTPUT_BYTES).is_ok());
    assert_eq!(
        ResolutionOptions::new(MAX_OUTPUT_BYTES + 1)
            .unwrap_err()
            .code,
        "SPX-PR601"
    );
    assert!(semver::parse_range("=1.2.3")
        .unwrap()
        .contains(semver::Version(1, 2, 3)));
    assert!(semver::parse_range("^0.2.3")
        .unwrap()
        .contains(semver::Version(0, 2, u32::MAX)));
    assert!(!semver::parse_range("^0.2.3")
        .unwrap()
        .contains(semver::Version(0, 3, 0)));
    assert!(semver::parse_range("~1.2.3")
        .unwrap()
        .contains(semver::Version(1, 2, u32::MAX)));
    for invalid in [
        "1.2.3",
        "=01.2.3",
        "=1.2",
        "=1.2.3 ",
        "^4294967295.0.0",
        "~1.4294967295.0",
        "^0.0.4294967295",
    ] {
        assert_eq!(semver::parse_range(invalid).unwrap_err().code, "SPX-PR601");
    }
    assert!(semver::parse_range("=4294967295.0.0").is_ok());
    assert_eq!(
        semver::parse_range("=4294967296.0.0").unwrap_err().code,
        "SPX-PR601"
    );
    assert_eq!(
        semver::parse_range("=10000000000.0.0").unwrap_err().code,
        "SPX-PR601"
    );
    let huge = format!("={}.0.0", "1".repeat(1024 * 1024));
    assert_eq!(semver::parse_range(&huge).unwrap_err().code, "SPX-PR601");
}

#[test]
fn nested_report_bounds_and_authentication_keep_distinct_resolver_codes() {
    let report = report("examples/meaning.spx");
    let subject = subject("examples.meaning", "1.0.0", &report, &[], &[]);
    let marker = "\"payload\":";
    let offset = report.find(marker).unwrap() + marker.len();
    let payload = &report[offset..report.len() - 1];
    let value: serde_json::Value = serde_json::from_str(payload).unwrap();
    let requested = value["limits"]["requested_max_bytes"].as_u64().unwrap();
    let bounded_payload = payload.replacen(
        &format!("\"requested_max_bytes\":{requested}"),
        &format!(
            "\"requested_max_bytes\":{}",
            crate::package_report_v2::MAX_OUTPUT_BYTES + 1
        ),
        1,
    );
    let bounded_report = remint(
        crate::package_report_v2::SCHEMA,
        b"semaprax.package-report-v2.payload.v1\0",
        &bounded_payload,
    );
    let bounded_subject = replace_subject_report(&subject, &bounded_report);
    assert_eq!(
        generate(
            &input(vec![bounded_subject], "=1.0.0"),
            &ResolutionOptions::default()
        )
        .unwrap_err()[0]
            .code,
        "SPX-PR605"
    );

    let changed_payload = payload.replacen("add(19, 23)", "add(18, 23)", 1);
    let changed_report = remint(
        crate::package_report_v2::SCHEMA,
        b"semaprax.package-report-v2.payload.v1\0",
        &changed_payload,
    );
    let changed_subject = replace_subject_report(&subject, &changed_report);
    assert_eq!(
        generate(
            &input(vec![changed_subject], "=1.0.0"),
            &ResolutionOptions::default()
        )
        .unwrap_err()[0]
            .code,
        "SPX-PR602"
    );
}

#[test]
fn first_feasible_backtracking_and_catalog_permutation_are_exact() {
    let meaning = report("examples/meaning.spx");
    let missing = package_lock_v3::Coordinate {
        package: "missing.package".to_owned(),
        version: "1.0.0".to_owned(),
    };
    let high = subject(
        "examples.meaning",
        "1.2.0",
        &meaning,
        std::slice::from_ref(&missing),
        &[],
    );
    let low = subject("examples.meaning", "1.1.0", &meaning, &[], &[]);
    let forward = input(vec![high.clone(), low.clone()], "^1.0.0");
    let reverse = input(vec![low, high], "^1.0.0");
    let evidence = generate(&forward, &ResolutionOptions::default()).expect("resolution");
    assert_eq!(
        generate(&reverse, &ResolutionOptions::default()).expect("permuted resolution"),
        evidence
    );
    assert!(evidence.contains("\"version\":\"1.1.0\""));
    assert!(evidence.contains("\"used_decisions\":2"));
    let verified = verify(&evidence, &reverse, &ResolutionOptions::default()).expect("replay");
    assert_eq!(
        verified.packages,
        vec![package_lock_v3::Coordinate {
            package: "examples.meaning".to_owned(),
            version: "1.1.0".to_owned(),
        }]
    );
    let independent_lock = package_lock_v3::generate(
        std::slice::from_ref(&reverse.subjects[0]),
        &package_lock_v3::LockOptions::default(),
    )
    .unwrap();
    let extracted = model::exact_lock_bytes(&evidence).unwrap();
    assert!(serde_json::from_str::<serde_json::Value>(extracted).is_ok());
    assert_eq!(extracted, independent_lock);
    assert_eq!(verified.lock, independent_lock);
    assert!(evidence.contains(&format!("\"lock\":{independent_lock},\"limits\":")));
}

#[test]
fn exact_lock_boundary_ignores_nested_limits_text_strings_and_escapes() {
    let evidence = r#"{"payload":{"lock":{"inner":{"limits":{"n":1}},"text":"},\"limits\": still inside","escaped":"\\","rows":[{"value":"]"}]},"limits":{"outer":true}}}"#;
    let extracted = model::exact_lock_bytes(evidence).unwrap();
    assert_eq!(
        extracted,
        r#"{"inner":{"limits":{"n":1}},"text":"},\"limits\": still inside","escaped":"\\","rows":[{"value":"]"}]}"#
    );
    assert!(serde_json::from_str::<serde_json::Value>(extracted).is_ok());
    assert_eq!(
        &evidence[evidence.find(extracted).unwrap() + extracted.len()..],
        ",\"limits\":{\"outer\":true}}}"
    );
}

#[test]
fn exact_transitive_graph_and_capability_policy_are_enforced() {
    let meaning = report("examples/meaning.spx");
    let calculator = report("examples/calculator.spx");
    let dependency = package_lock_v3::Coordinate {
        package: "examples.calculator".to_owned(),
        version: "1.0.0".to_owned(),
    };
    let root = subject(
        "examples.meaning",
        "1.0.0",
        &meaning,
        std::slice::from_ref(&dependency),
        &["root.execute"],
    );
    let dependency_subject = subject(
        "examples.calculator",
        "1.0.0",
        &calculator,
        &[],
        &["dependency.read"],
    );
    let mut admitted = input(vec![root.clone(), dependency_subject.clone()], "=1.0.0");
    admitted.allowed_capabilities = vec!["dependency.read".to_owned(), "root.execute".to_owned()];
    let evidence = generate(&admitted, &ResolutionOptions::default()).expect("policy admitted");
    assert!(evidence.contains("\"capability_closure\":[\"dependency.read\",\"root.execute\"]"));
    admitted.allowed_capabilities.pop();
    assert_eq!(
        generate(&admitted, &ResolutionOptions::default()).unwrap_err()[0].code,
        "SPX-PR604"
    );
}

#[test]
fn catalog_confusion_and_outer_remints_fail_closed() {
    let meaning = report("examples/meaning.spx");
    let one = subject("examples.meaning", "1.0.0", &meaning, &[], &[]);
    let duplicate = input(vec![one.clone(), one.clone()], "=1.0.0");
    assert_eq!(
        generate(&duplicate, &ResolutionOptions::default()).unwrap_err()[0].code,
        "SPX-PR603"
    );
    let valid = input(vec![one], "=1.0.0");
    let evidence = generate(&valid, &ResolutionOptions::default()).expect("evidence");
    let marker = "\"payload\":";
    let offset = evidence.find(marker).unwrap() + marker.len();
    let payload = &evidence[offset..evidence.len() - 1];
    let changed = payload.replacen("\"used_decisions\":1", "\"used_decisions\":0", 1);
    let reminted = wire::render_wrapper(&changed);
    assert_eq!(
        verify(&reminted, &valid, &ResolutionOptions::default())
            .unwrap_err()
            .code,
        "SPX-PR607"
    );
    let duplicate_key = payload.replacen(
        "\"target\":\"native64\"",
        "\"target\":\"native64\",\"target\":\"native64\"",
        1,
    );
    let reminted = wire::render_wrapper(&duplicate_key);
    assert_eq!(
        verify(&reminted, &valid, &ResolutionOptions::default())
            .unwrap_err()
            .code,
        "SPX-PR606"
    );

    let missing = payload.replacen(",\"target\":\"native64\"", "", 1);
    assert_eq!(
        verify(
            &wire::render_wrapper(&missing),
            &valid,
            &ResolutionOptions::default()
        )
        .unwrap_err()
        .code,
        "SPX-PR606"
    );
    let unknown = payload.replacen(
        "\"schema\":\"semaprax.offline-package-resolution-evidence.v2\"",
        "\"schema\":\"semaprax.offline-package-resolution-evidence.v2\",\"unknown\":0",
        1,
    );
    assert_eq!(
        verify(
            &wire::render_wrapper(&unknown),
            &valid,
            &ResolutionOptions::default()
        )
        .unwrap_err()
        .code,
        "SPX-PR606"
    );
    let noncanonical = payload.replacen("native64", "native\\u00364", 1);
    assert_eq!(
        verify(
            &wire::render_wrapper(&noncanonical),
            &valid,
            &ResolutionOptions::default()
        )
        .unwrap_err()
        .code,
        "SPX-PR606"
    );
    for malformed in [
        format!("\u{feff}{evidence}"),
        format!("{evidence}\r"),
        format!("{evidence}x"),
        evidence[..evidence.len() - 1].to_owned(),
        evidence.replacen(
            "{\"schema\":\"semaprax.offline-package-resolution-evidence.v2\",",
            "{",
            1,
        ),
        evidence.replacen("{", "{\"unknown\":0,", 1),
    ] {
        assert_eq!(
            verify(&malformed, &valid, &ResolutionOptions::default())
                .unwrap_err()
                .code,
            "SPX-PR606"
        );
    }
    let too_deep = format!(
        "{}0{}",
        "[".repeat(MAX_JSON_DEPTH + 1),
        "]".repeat(MAX_JSON_DEPTH + 1)
    );
    assert_eq!(
        verify(&too_deep, &valid, &ResolutionOptions::default())
            .unwrap_err()
            .code,
        "SPX-PR605"
    );

    let alternate = subject("examples.meaning", "1.1.0", &meaning, &[], &[]);
    let substituted = input(vec![valid.subjects[0].clone(), alternate], "=1.0.0");
    assert_eq!(
        verify(&evidence, &substituted, &ResolutionOptions::default())
            .unwrap_err()
            .code,
        "SPX-PR607"
    );

    let subject = &valid.subjects[0];
    let offset = subject.find(marker).unwrap() + marker.len();
    let payload = &subject[offset..subject.len() - 1];
    let changed = payload.replacen("add(19, 23)", "add(18, 23)", 1);
    let forged = remint(
        package_lock_v3::SUBJECT_SCHEMA,
        b"semaprax.offline-semantic-package-subject.v3\0",
        &changed,
    );
    let forged_input = input(vec![forged], "=1.0.0");
    assert_eq!(
        generate(&forged_input, &ResolutionOptions::default()).unwrap_err()[0].code,
        "SPX-PR602"
    );
}

#[test]
fn input_and_catalog_count_limits_precede_subject_replay() {
    let invalid_options = ResolutionOptions { max_bytes: 1 };
    let empty = input(vec![], "=1.0.0");
    assert_eq!(
        generate(&empty, &invalid_options).unwrap_err()[0].code,
        "SPX-PR601"
    );
    assert_eq!(
        generate(&empty, &ResolutionOptions::default()).unwrap_err()[0].code,
        "SPX-PR605"
    );
    let too_many = input(vec!["not a subject".to_owned(); MAX_SUBJECTS + 1], "=1.0.0");
    assert_eq!(
        generate(&too_many, &ResolutionOptions::default()).unwrap_err()[0].code,
        "SPX-PR605"
    );
    let mut work = 0;
    assert!(wire::charge(&mut work, MAX_WORK_UNITS).is_ok());
    assert_eq!(wire::charge(&mut work, 1).unwrap_err().code, "SPX-PR605");
    assert!(solver::admit_edge_count(MAX_EDGES));
    assert!(!solver::admit_edge_count(MAX_EDGES + 1));
    assert!(solver::admit_depth(MAX_DEPTH));
    assert!(!solver::admit_depth(MAX_DEPTH + 1));
}

#[test]
fn hostile_input_grammar_and_subject_bounds_have_stable_codes() {
    let mut malformed = input(vec!["not JSON".to_owned()], "=1.0.0");
    assert_eq!(
        generate(&malformed, &ResolutionOptions::default()).unwrap_err()[0].code,
        "SPX-PR602"
    );
    malformed.requirements.push(Requirement {
        package: "a".to_owned(),
        range: "=1.0.0".to_owned(),
    });
    assert_eq!(
        generate(&malformed, &ResolutionOptions::default()).unwrap_err()[0].code,
        "SPX-PR601"
    );
    malformed.requirements.truncate(1);
    malformed.target = "host".to_owned();
    assert_eq!(
        generate(&malformed, &ResolutionOptions::default()).unwrap_err()[0].code,
        "SPX-PR601"
    );
    malformed.target = "native64".to_owned();
    malformed.allowed_capabilities = vec!["x".to_owned(); MAX_ALLOWED_CAPABILITIES + 1];
    assert_eq!(
        generate(&malformed, &ResolutionOptions::default()).unwrap_err()[0].code,
        "SPX-PR601"
    );
    let oversized = input(vec!["x".repeat(MAX_SUBJECT_BYTES + 1)], "=1.0.0");
    assert_eq!(
        generate(&oversized, &ResolutionOptions::default()).unwrap_err()[0].code,
        "SPX-PR605"
    );
}

#[test]
fn unavailable_and_unproven_candidates_are_policy_rejections() {
    for status in ["unavailable", "unproven"] {
        let coordinate = package_lock_v3::Coordinate {
            package: "fixture.package".to_owned(),
            version: "1.0.0".to_owned(),
        };
        let entry = catalog::Entry {
            subject: package_lock_v3::ResolutionSubject {
                coordinate: coordinate.clone(),
                subject_digest: "sha256:fixture".to_owned(),
                subject_bytes: 0,
                dependencies: vec![],
                capabilities: vec![],
                targets: BTreeMap::from([("native64".to_owned(), status.to_owned())]),
            },
            version: semver::Version(1, 0, 0),
            dependency_ranges: vec![],
            bytes: "",
        };
        let catalog = catalog::Catalog {
            entries: vec![entry],
            by_package: BTreeMap::from([("fixture.package".to_owned(), vec![0])]),
            by_coordinate: BTreeMap::from([(
                ("fixture.package".to_owned(), semver::Version(1, 0, 0)),
                0,
            )]),
            target_inventory: BTreeSet::from(["native64".to_owned()]),
            total_bytes: 0,
            digest: "sha256:fixture".to_owned(),
        };
        let input = ResolutionInput {
            requirements: vec![Requirement {
                package: "fixture.package".to_owned(),
                range: "=1.0.0".to_owned(),
            }],
            subjects: vec![],
            target: "native64".to_owned(),
            allowed_capabilities: vec![],
        };
        let mut work = 0;
        let requirements = model::validate_input(&input, &mut work).unwrap();
        assert_eq!(
            solver::solve(&input, &requirements, &catalog, &mut work)
                .err()
                .unwrap()
                .code,
            "SPX-PR604"
        );
    }
}

#[test]
fn public_selected_package_limit_accepts_four_and_rejects_five() {
    let fixtures = [
        ("examples/meaning.spx", "examples.meaning"),
        ("examples/calculator.spx", "examples.calculator"),
        ("examples/chars.spx", "examples.chars"),
        ("examples/floats.spx", "examples.floats"),
        ("examples/integers_i32.spx", "examples.integers_i32"),
    ];
    let reports = fixtures
        .iter()
        .map(|(path, _)| report(path))
        .collect::<Vec<_>>();
    let coordinates = fixtures
        .iter()
        .map(|(_, package)| package_lock_v3::Coordinate {
            package: (*package).to_owned(),
            version: "1.0.0".to_owned(),
        })
        .collect::<Vec<_>>();

    let build = |count: usize| {
        let mut dependencies = coordinates[1..count].to_vec();
        dependencies.sort();
        let mut subjects = vec![subject(
            "examples.meaning",
            "1.0.0",
            &reports[0],
            &dependencies,
            &[],
        )];
        for index in 1..count {
            subjects.push(subject(
                &coordinates[index].package,
                "1.0.0",
                &reports[index],
                &[],
                &[],
            ));
        }
        input(subjects, "=1.0.0")
    };

    let four = generate(&build(4), &ResolutionOptions::default()).expect("four selected packages");
    assert!(four.contains("\"used_selected_packages\":4"));
    assert_eq!(
        generate(&build(5), &ResolutionOptions::default()).unwrap_err()[0].code,
        "SPX-PR603"
    );
}
