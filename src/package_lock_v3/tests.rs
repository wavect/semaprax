use super::*;
use std::collections::BTreeMap;

// Preserve the exact-coordinate hostile fixtures as exact range requirements.
fn create_subject(
    coordinate: &Coordinate,
    report: &str,
    dependencies: &[Coordinate],
    capabilities: &[String],
) -> Result<String, Vec<Diagnostic>> {
    super::create_subject(
        coordinate,
        report,
        &dependencies
            .iter()
            .map(|d| DependencyRequirement {
                package: d.package.clone(),
                range: format!("={}", d.version),
            })
            .collect::<Vec<_>>(),
        capabilities,
    )
}

#[test]
fn option_and_graph_limits_have_exact_boundaries() {
    assert!(LockOptions::new(MIN_OUTPUT_BYTES).is_ok());
    assert!(LockOptions::new(MIN_OUTPUT_BYTES - 1).is_err());
    assert!(LockOptions::new(MAX_OUTPUT_BYTES).is_ok());
    assert!(LockOptions::new(MAX_OUTPUT_BYTES + 1).is_err());
    let mut work = 0;
    assert!(charge(&mut work, MAX_WORK_UNITS).is_ok());
    assert!(charge(&mut work, 1).is_err());
}

#[test]
fn exact_v2_subject_lock_and_replay_are_deterministic() {
    let report = crate::package_report_v2::generate(
        std::path::Path::new("examples/meaning.spx"),
        &crate::package_report_v2::PackageReportV2Options::default(),
    )
    .unwrap();
    let coordinate = Coordinate {
        package: "examples.meaning".to_owned(),
        version: "1.0.0".to_owned(),
    };
    let subject = create_subject(&coordinate, &report, &[], &[]).unwrap();
    let lock = generate(std::slice::from_ref(&subject), &LockOptions::default()).unwrap();
    assert_eq!(
        generate(std::slice::from_ref(&subject), &LockOptions::default()).unwrap(),
        lock
    );
    assert_eq!(
        verify(&lock, &[subject], &LockOptions::default())
            .unwrap()
            .packages,
        vec![coordinate]
    );
}

#[test]
fn ternary_targets_preserve_unproven() {
    let subject = |status: &str| Subject {
        coordinate: Coordinate {
            package: status.to_owned(),
            version: "1.0.0".to_owned(),
        },
        digest: String::new(),
        bytes: 0,
        report: String::new(),
        report_digest: String::new(),
        revision: String::new(),
        targets: BTreeMap::from([("native64".to_owned(), status.to_owned())]),
        dependencies: vec![],
        capabilities: vec![],
    };
    let available = subject("available");
    let unproven = subject("unproven");
    assert_eq!(
        aggregate_targets([&available, &unproven].into_iter()).unwrap()["native64"],
        "unproven"
    );
}

#[test]
fn missing_dependency_rejects_after_exact_subject_replay() {
    let report = crate::package_report_v2::generate(
        std::path::Path::new("examples/meaning.spx"),
        &crate::package_report_v2::PackageReportV2Options::default(),
    )
    .unwrap();
    let coordinate = Coordinate {
        package: "examples.meaning".to_owned(),
        version: "1.0.0".to_owned(),
    };
    let missing = Coordinate {
        package: "missing.package".to_owned(),
        version: "1.0.0".to_owned(),
    };
    let subject = create_subject(&coordinate, &report, &[missing], &[]).unwrap();
    assert!(generate(&[subject], &LockOptions::default()).is_err());
}

#[test]
fn subject_and_lock_outer_remints_do_not_bypass_replay() {
    let report = crate::package_report_v2::generate(
        std::path::Path::new("examples/meaning.spx"),
        &crate::package_report_v2::PackageReportV2Options::default(),
    )
    .unwrap();
    let coordinate = Coordinate {
        package: "examples.meaning".to_owned(),
        version: "1.0.0".to_owned(),
    };
    let subject = create_subject(&coordinate, &report, &[], &[]).unwrap();
    let marker = "\"payload\":";
    let offset = subject.find(marker).unwrap() + marker.len();
    let payload = &subject[offset..subject.len() - 1];
    let forged_subject = wire::render_wrapper(
        SUBJECT_SCHEMA,
        SUBJECT_DOMAIN,
        &payload.replacen("add(19, 23)", "add(18, 23)", 1),
    );
    assert_eq!(
        generate(&[forged_subject], &LockOptions::default()).unwrap_err()[0].code,
        "SPX-PL603"
    );
    let lock = generate(std::slice::from_ref(&subject), &LockOptions::default()).unwrap();
    let offset = lock.find(marker).unwrap() + marker.len();
    let payload = &lock[offset..lock.len() - 1];
    let forged_lock = wire::render_wrapper(
        SCHEMA,
        LOCK_DOMAIN,
        &payload.replacen("\"used_edges\":0", "\"used_edges\":1", 1),
    );
    assert_eq!(
        verify(&forged_lock, &[subject], &LockOptions::default())
            .unwrap_err()
            .code,
        "SPX-PL607"
    );
}

#[test]
fn cycle_version_confusion_and_transitive_capability_closure_are_publicly_closed() {
    let meaning_report = crate::package_report_v2::generate(
        std::path::Path::new("examples/meaning.spx"),
        &crate::package_report_v2::PackageReportV2Options::default(),
    )
    .unwrap();
    let calculator_report = crate::package_report_v2::generate(
        std::path::Path::new("examples/calculator.spx"),
        &crate::package_report_v2::PackageReportV2Options::default(),
    )
    .unwrap();
    let meaning = Coordinate {
        package: "examples.meaning".to_owned(),
        version: "1.0.0".to_owned(),
    };
    let calculator = Coordinate {
        package: "examples.calculator".to_owned(),
        version: "1.0.0".to_owned(),
    };

    let meaning_cycle = create_subject(
        &meaning,
        &meaning_report,
        std::slice::from_ref(&calculator),
        &[],
    )
    .unwrap();
    let calculator_cycle = create_subject(
        &calculator,
        &calculator_report,
        std::slice::from_ref(&meaning),
        &[],
    )
    .unwrap();
    assert_eq!(
        generate(&[meaning_cycle, calculator_cycle], &LockOptions::default()).unwrap_err()[0].code,
        "SPX-PL605"
    );

    let meaning_v2 = Coordinate {
        package: meaning.package.clone(),
        version: "2.0.0".to_owned(),
    };
    let meaning_v1_subject = create_subject(&meaning, &meaning_report, &[], &[]).unwrap();
    let meaning_v2_subject = create_subject(&meaning_v2, &meaning_report, &[], &[]).unwrap();
    assert_eq!(
        generate(
            &[meaning_v1_subject, meaning_v2_subject],
            &LockOptions::default()
        )
        .unwrap_err()[0]
            .code,
        "SPX-PL604"
    );

    let dependency_subject = create_subject(
        &calculator,
        &calculator_report,
        &[],
        &["dependency.read".to_owned()],
    )
    .unwrap();
    let root_subject = create_subject(
        &meaning,
        &meaning_report,
        std::slice::from_ref(&calculator),
        &["root.execute".to_owned()],
    )
    .unwrap();
    let subjects = vec![root_subject, dependency_subject];
    let lock = generate(&subjects, &LockOptions::default()).unwrap();
    assert!(lock.contains("\"package\":\"examples.meaning\",\"version\":\"1.0.0\""));
    assert!(lock.contains("\"capability_closure\":[\"dependency.read\",\"root.execute\"]"));
    assert_eq!(
        verify(&lock, &subjects, &LockOptions::default())
            .unwrap()
            .packages,
        vec![calculator, meaning]
    );
}
