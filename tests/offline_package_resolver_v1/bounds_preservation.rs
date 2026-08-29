use super::support::*;

#[test]
fn catalog_limits_are_global_and_selected_graph_overflow_is_structural() {
    let package = "resolver.bounds";
    let report = report(package);
    let one = subject(&report, package, "1.0.0", &[], &[]);
    assert_eq!(
        error_code(&input(&[(package, "=1.0.0")], vec![], "native64", &[])),
        "SPX-PR505"
    );
    assert_eq!(
        error_code(&input(
            &[(package, "=1.0.0")],
            vec![one.clone(); MAX_SUBJECTS + 1],
            "native64",
            &[]
        )),
        "SPX-PR505"
    );
    let versions = (0..=MAX_VERSIONS_PER_PACKAGE)
        .map(|patch| subject(&report, package, &format!("1.0.{patch}"), &[], &[]))
        .collect::<Vec<_>>();
    assert_eq!(
        error_code(&input(&[(package, "^1.0.0")], versions, "native64", &[])),
        "SPX-PR505"
    );

    let names = [
        "resolver.depth.a",
        "resolver.depth.b",
        "resolver.depth.c",
        "resolver.depth.d",
        "resolver.depth.e",
    ];
    let reports = names.iter().map(|name| report(name)).collect::<Vec<_>>();
    let chain = names
        .iter()
        .enumerate()
        .map(|(index, name)| {
            let dependencies = names
                .get(index + 1)
                .map(|next| vec![coordinate(next, "1.0.0")])
                .unwrap_or_default();
            subject(&reports[index], name, "1.0.0", &dependencies, &[])
        })
        .collect::<Vec<_>>();
    assert_eq!(
        error_code(&input(&[(names[0], "=1.0.0")], chain, "native64", &[])),
        "SPX-PR503"
    );
}

#[test]
fn decision_and_work_exhaustion_abort_the_whole_search() {
    let names = [
        "resolver.decisions.a",
        "resolver.decisions.b",
        "resolver.decisions.c",
    ];
    let reports = names.iter().map(|name| report(name)).collect::<Vec<_>>();
    let missing = coordinate("resolver.decisions.missing", "1.0.0");
    let mut subjects = Vec::new();
    for (index, name) in names.iter().enumerate() {
        for patch in 0..21 {
            let dependencies = if index == 2 {
                std::slice::from_ref(&missing)
            } else {
                &[]
            };
            subjects.push(subject(
                &reports[index],
                name,
                &format!("1.0.{patch}"),
                dependencies,
                &[],
            ));
        }
    }
    assert_eq!(subjects.len(), 63);
    assert_eq!(
        error_code(&input(
            &[
                (names[0], "^1.0.0"),
                (names[1], "^1.0.0"),
                (names[2], "^1.0.0"),
            ],
            subjects,
            "native64",
            &[]
        )),
        "SPX-PR505"
    );

    let package = "resolver.work";
    let literal = "a".repeat(940_000);
    let large_report = report_from_source(
        package,
        &format!("module {package};\nfn main() -> string {{ \"{literal}\" }}\n"),
    );
    let subjects = (0..9)
        .map(|patch| subject(&large_report, package, &format!("1.0.{patch}"), &[], &[]))
        .collect::<Vec<_>>();
    assert_eq!(
        error_code(&input(&[(package, "^1.0.0")], subjects, "native64", &[])),
        "SPX-PR505"
    );
}

#[test]
fn capability_allowlist_exact_count_and_plus_one_are_distinct() {
    let package = "resolver.cap.bound";
    let subject = subject(&report(package), package, "1.0.0", &[], &[]);
    let allowed = (0..MAX_ALLOWED_CAPABILITIES)
        .map(|index| format!("cap.{index:03}"))
        .collect::<Vec<_>>();
    let exact = ResolutionInput {
        requirements: vec![Requirement {
            package: package.to_owned(),
            range: "=1.0.0".to_owned(),
        }],
        subjects: vec![subject.clone()],
        target: "native64".to_owned(),
        allowed_capabilities: allowed.clone(),
    };
    assert!(package_resolver::generate(&exact, &ResolutionOptions::default()).is_ok());
    let mut overflow = exact;
    overflow
        .allowed_capabilities
        .push(format!("cap.{MAX_ALLOWED_CAPABILITIES:03}"));
    assert_eq!(error_code(&overflow), "SPX-PR501");
}

#[test]
fn meaning_v1_kat_and_same_binary_resolver_purity_are_pinned() {
    let meaning_v1 = package_report::generate(
        Path::new("examples/meaning.spx"),
        &PackageReportOptions::default(),
    )
    .unwrap();
    assert_eq!(
        sha256(meaning_v1.as_bytes()),
        "sha256:97bcde287804d9311f343157058926fb0648e66282461ede138e98824aac06f2"
    );

    let package = "resolver.preservation";
    let report = report(package);
    let v1 = subject(&report, package, "1.0.0", &[], &[]);
    let v2 = subject(&report, package, "1.1.0", &[], &[]);
    let lock_before =
        package_lock_v2::generate(std::slice::from_ref(&v1), &LockOptions::default()).unwrap();
    let candidate_lock =
        package_lock_v2::generate(std::slice::from_ref(&v2), &LockOptions::default()).unwrap();
    let base = CompatibilityInput {
        coordinate: coordinate(package, "1.0.0"),
        report: report.clone(),
        lock: lock_before.clone(),
        lock_subjects: vec![v1.clone()],
    };
    let candidate = CompatibilityInput {
        coordinate: coordinate(package, "1.1.0"),
        report: report.clone(),
        lock: candidate_lock.clone(),
        lock_subjects: vec![v2.clone()],
    };
    let compatibility_before =
        package_compatibility::generate(&base, &candidate, &CompatibilityOptions::default())
            .unwrap();

    let request = input(
        &[(package, "^1.0.0")],
        vec![v1.clone(), v2.clone()],
        "native64",
        &[],
    );
    let resolution = generate(&request);
    let receipt =
        package_resolver::verify(&resolution, &request, &ResolutionOptions::default()).unwrap();
    assert_eq!(receipt.packages, vec![coordinate(package, "1.1.0")]);

    // These same-binary before/after checks establish only that resolution is
    // pure and does not mutate caller-owned reports/subjects or legacy module
    // state. They are not independent byte-compatibility KATs.
    assert_eq!(
        package_lock_v2::generate(std::slice::from_ref(&v1), &LockOptions::default()).unwrap(),
        lock_before
    );
    assert_eq!(
        package_lock_v2::generate(std::slice::from_ref(&v2), &LockOptions::default()).unwrap(),
        candidate_lock
    );
    assert_eq!(
        package_compatibility::generate(&base, &candidate, &CompatibilityOptions::default())
            .unwrap(),
        compatibility_before
    );
}
