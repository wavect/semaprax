use std::sync::atomic::{AtomicU64, Ordering};

use semaprax::package_lock_v3::{self, Coordinate, DependencyRequirement, LockOptions};
use semaprax::package_report_v2::{self, PackageReportV2Options};
use semaprax::package_resolver_v2::{self, Requirement, ResolutionInput, ResolutionOptions};

static NEXT: AtomicU64 = AtomicU64::new(0);

fn report(package: &str) -> String {
    let path = std::env::temp_dir().join(format!(
        "semaprax-range-v3-{}-{}.spx",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    std::fs::write(
        &path,
        format!("module {package};\n@id(\"{package}.main\")\nfn main() -> i64 {{ 42 }}\n"),
    )
    .unwrap();
    let result = package_report_v2::generate(&path, &PackageReportV2Options::default()).unwrap();
    std::fs::remove_file(path).unwrap();
    result
}

fn subject(package: &str, version: &str, dependencies: &[(&str, &str)]) -> String {
    package_lock_v3::create_subject(
        &Coordinate {
            package: package.into(),
            version: version.into(),
        },
        &report(package),
        &dependencies
            .iter()
            .map(|(package, range)| DependencyRequirement {
                package: (*package).into(),
                range: (*range).into(),
            })
            .collect::<Vec<_>>(),
        &[],
    )
    .unwrap()
}

#[test]
fn authenticated_range_selects_numeric_highest_and_replays_lock() {
    let input = ResolutionInput {
        requirements: vec![Requirement {
            package: "app".into(),
            range: "=1.0.0".into(),
        }],
        subjects: vec![
            subject("dep", "1.2.0", &[]),
            subject("app", "1.0.0", &[("dep", "^1.0.0")]),
            subject("dep", "1.10.0", &[]),
        ],
        target: "wasm32".into(),
        allowed_capabilities: vec![],
    };
    let evidence = package_resolver_v2::generate(&input, &ResolutionOptions::default()).unwrap();
    let receipt =
        package_resolver_v2::verify(&evidence, &input, &ResolutionOptions::default()).unwrap();
    assert!(receipt
        .packages
        .iter()
        .any(|p| p.package == "dep" && p.version == "1.10.0"));
    package_lock_v3::verify(
        &receipt.lock,
        &[input.subjects[1].clone(), input.subjects[2].clone()],
        &LockOptions::default(),
    )
    .unwrap();
}

#[test]
fn transitive_range_conflict_and_subject_mutation_fail_closed() {
    let subjects = vec![
        subject("dep", "2.0.0", &[]),
        subject("app", "1.0.0", &[("dep", "^1.0.0")]),
    ];
    let input = ResolutionInput {
        requirements: vec![Requirement {
            package: "app".into(),
            range: "=1.0.0".into(),
        }],
        subjects,
        target: "native64".into(),
        allowed_capabilities: vec![],
    };
    assert_eq!(
        package_resolver_v2::generate(&input, &ResolutionOptions::default()).unwrap_err()[0].code,
        "SPX-PR603"
    );
    let mutated =
        subject("dep", "1.0.0", &[]).replacen("\"version\":\"1.0.0\"", "\"version\":\"1.0.1\"", 1);
    assert_eq!(
        package_lock_v3::generate(&[mutated], &LockOptions::default()).unwrap_err()[0].code,
        "SPX-PL603"
    );
}

#[test]
fn range_grammar_self_dependency_and_output_bounds_are_closed() {
    let coordinate = Coordinate {
        package: "app".into(),
        version: "1.0.0".into(),
    };
    for range in ["1.0.0", "^01.0.0", "^1.0", "*", "^4294967295.0.0"] {
        let error = package_lock_v3::create_subject(
            &coordinate,
            &report("app"),
            &[DependencyRequirement {
                package: "dep".into(),
                range: range.into(),
            }],
            &[],
        )
        .unwrap_err();
        assert_eq!(error[0].code, "SPX-PL604");
    }
    let self_error = package_lock_v3::create_subject(
        &coordinate,
        &report("app"),
        &[DependencyRequirement {
            package: "app".into(),
            range: "=1.0.0".into(),
        }],
        &[],
    )
    .unwrap_err();
    assert_eq!(self_error[0].code, "SPX-PL604");
    assert_eq!(LockOptions::new(4095).unwrap_err().code, "SPX-PL601");
    assert_eq!(ResolutionOptions::new(4095).unwrap_err().code, "SPX-PR601");
}

#[test]
fn later_root_range_intersects_selected_dependency_and_backtracks_without_leakage() {
    let input = ResolutionInput {
        requirements: vec![
            Requirement {
                package: "app".into(),
                range: "=1.0.0".into(),
            },
            Requirement {
                package: "peer".into(),
                range: "=1.0.0".into(),
            },
        ],
        subjects: vec![
            subject("app", "1.0.0", &[("dep", "^1.0.0")]),
            subject("dep", "1.10.0", &[]),
            subject("dep", "1.2.9", &[]),
            subject("peer", "1.0.0", &[("dep", "~1.2.0")]),
        ],
        target: "wasm32".into(),
        allowed_capabilities: vec![],
    };
    let evidence = package_resolver_v2::generate(&input, &ResolutionOptions::default()).unwrap();
    let receipt =
        package_resolver_v2::verify(&evidence, &input, &ResolutionOptions::default()).unwrap();
    assert!(receipt
        .packages
        .iter()
        .any(|p| p.package == "dep" && p.version == "1.2.9"));
    assert!(receipt
        .lock
        .contains("\"package\":\"dep\",\"range\":\"~1.2.0\",\"selected_version\":\"1.2.9\""));
    let mut permuted = input.clone();
    permuted.subjects.reverse();
    assert_eq!(
        package_resolver_v2::generate(&permuted, &ResolutionOptions::default()).unwrap(),
        evidence
    );
    let mut changed = input.clone();
    changed.subjects[0] = subject("app", "1.0.0", &[("dep", "~1.2.0")]);
    assert_eq!(
        package_resolver_v2::verify(&evidence, &changed, &ResolutionOptions::default())
            .unwrap_err()
            .code,
        "SPX-PR607"
    );
}

#[test]
fn lock_replays_requirement_to_selected_version_and_exact_raw_report() {
    let report = report("app");
    let root = package_lock_v3::create_subject(
        &Coordinate {
            package: "app".into(),
            version: "1.0.0".into(),
        },
        &report,
        &[DependencyRequirement {
            package: "dep".into(),
            range: "~1.2.0".into(),
        }],
        &[],
    )
    .unwrap();
    assert!(root.contains(&format!("\"report\":{report},\"dependencies\":")));
    let good = subject("dep", "1.2.5", &[]);
    let bad = subject("dep", "1.3.0", &[]);
    let lock =
        package_lock_v3::generate(&[root.clone(), good.clone()], &LockOptions::default()).unwrap();
    assert!(lock.contains("\"requirement\":{\"package\":\"dep\",\"range\":\"~1.2.0\"},\"selected\":{\"package\":\"dep\",\"version\":\"1.2.5\"}"));
    assert_eq!(
        package_lock_v3::generate(&[root.clone(), bad], &LockOptions::default()).unwrap_err()[0]
            .code,
        "SPX-PL604"
    );
    let mut reordered = vec![root, good];
    reordered.reverse();
    assert_eq!(
        package_lock_v3::generate(&reordered, &LockOptions::default()).unwrap(),
        lock
    );
}
