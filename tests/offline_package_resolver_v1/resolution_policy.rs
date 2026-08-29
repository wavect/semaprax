use super::support::*;

#[test]
fn multi_root_convergence_and_exact_transitive_closure_are_selected_once() {
    let names = [
        "resolver.graph.a",
        "resolver.graph.b",
        "resolver.graph.c",
        "resolver.graph.d",
    ];
    let reports = names.iter().map(|name| report(name)).collect::<Vec<_>>();
    let b = coordinate(names[1], "1.0.0");
    let d = coordinate(names[3], "1.0.0");
    let subjects = vec![
        subject(
            &reports[0],
            names[0],
            "1.0.0",
            std::slice::from_ref(&b),
            &[],
        ),
        subject(
            &reports[1],
            names[1],
            "1.0.0",
            std::slice::from_ref(&d),
            &[],
        ),
        subject(
            &reports[2],
            names[2],
            "1.0.0",
            std::slice::from_ref(&b),
            &[],
        ),
        subject(&reports[3], names[3], "1.0.0", &[], &[]),
    ];
    let request = input(
        &[(names[0], "=1.0.0"), (names[2], "=1.0.0")],
        subjects,
        "wasm32",
        &[],
    );
    let receipt =
        package_resolver::verify(&generate(&request), &request, &ResolutionOptions::default())
            .unwrap();
    assert_eq!(
        receipt.packages,
        names
            .iter()
            .map(|name| coordinate(name, "1.0.0"))
            .collect::<Vec<_>>()
    );
    assert_eq!(
        package_lock_v2::verify(&receipt.lock, &request.subjects, &LockOptions::default())
            .unwrap()
            .packages,
        vec![
            d,
            b,
            coordinate(names[0], "1.0.0"),
            coordinate(names[2], "1.0.0")
        ]
    );
}

#[test]
fn missing_conflicting_duplicate_and_cyclic_catalogs_fail_closed() {
    let a = "resolver.reject.a";
    let b = "resolver.reject.b";
    let c = "resolver.reject.c";
    let reports = [report(a), report(b), report(c)];
    let a1 = coordinate(a, "1.0.0");
    let b1 = coordinate(b, "1.0.0");
    let b2 = coordinate(b, "2.0.0");
    let c1 = coordinate(c, "1.0.0");

    let missing = subject(&reports[0], a, "1.0.0", std::slice::from_ref(&b1), &[]);
    assert_eq!(
        error_code(&input(&[(a, "=1.0.0")], vec![missing], "native64", &[])),
        "SPX-PR503"
    );

    let conflict = vec![
        subject(&reports[0], a, "1.0.0", std::slice::from_ref(&b1), &[]),
        subject(&reports[1], b, "1.0.0", &[], &[]),
        subject(&reports[1], b, "2.0.0", &[], &[]),
        subject(&reports[2], c, "1.0.0", std::slice::from_ref(&b2), &[]),
    ];
    assert_eq!(
        error_code(&input(
            &[(a, "=1.0.0"), (c, "=1.0.0")],
            conflict,
            "native64",
            &[]
        )),
        "SPX-PR503"
    );

    let duplicate = subject(&reports[0], a, "1.0.0", &[], &[]);
    assert_eq!(
        error_code(&input(
            &[(a, "=1.0.0")],
            vec![duplicate.clone(), duplicate],
            "native64",
            &[]
        )),
        "SPX-PR503"
    );

    let cycle = vec![
        subject(&reports[0], a, "1.0.0", std::slice::from_ref(&b1), &[]),
        subject(&reports[1], b, "1.0.0", std::slice::from_ref(&c1), &[]),
        subject(&reports[2], c, "1.0.0", std::slice::from_ref(&a1), &[]),
    ];
    assert_eq!(
        error_code(&input(&[(a, "=1.0.0")], cycle, "native64", &[])),
        "SPX-PR503"
    );
}

#[test]
fn target_and_direct_or_transitive_capability_policy_are_fail_closed() {
    let unavailable = "resolver.target.unavailable";
    let unavailable_subject = subject(
        &unavailable_report(unavailable),
        unavailable,
        "1.0.0",
        &[],
        &[],
    );
    assert_eq!(
        error_code(&input(
            &[(unavailable, "=1.0.0")],
            vec![unavailable_subject],
            "native64",
            &[]
        )),
        "SPX-PR504"
    );

    let unproven = "resolver.target.unproven";
    let unproven_report = unproven_native_report(unproven);
    assert!(unproven_report.contains("\"target\":\"native64\",\"status\":\"unproven\""));
    let unproven_subject = subject(&unproven_report, unproven, "1.0.0", &[], &[]);
    assert_eq!(
        error_code(&input(
            &[(unproven, "=1.0.0")],
            vec![unproven_subject],
            "native64",
            &[]
        )),
        "SPX-PR504"
    );

    let a = "resolver.cap.a";
    let b = "resolver.cap.b";
    let a_report = report(a);
    let b_report = report(b);
    let b1 = coordinate(b, "1.0.0");
    let subjects = vec![
        subject(
            &a_report,
            a,
            "1.0.0",
            std::slice::from_ref(&b1),
            &["root.execute"],
        ),
        subject(&b_report, b, "1.0.0", &[], &["dependency.read"]),
    ];
    assert_eq!(
        error_code(&input(&[(a, "=1.0.0")], subjects.clone(), "native64", &[])),
        "SPX-PR504"
    );
    assert_eq!(
        error_code(&input(
            &[(a, "=1.0.0")],
            subjects.clone(),
            "native64",
            &["root.execute"]
        )),
        "SPX-PR504"
    );
    let allowed = input(
        &[(a, "=1.0.0")],
        subjects,
        "native64",
        &["dependency.read", "root.execute"],
    );
    let evidence = generate(&allowed);
    assert!(evidence.contains("\"capability_closure\":[\"dependency.read\",\"root.execute\"]"));
}

#[test]
fn input_authentication_resolution_and_policy_failure_order_is_stable() {
    let package = "resolver.precedence";
    let invalid_grammar = input(
        &[("resolver.z", "=1.0.0"), (package, "=1.0.0")],
        vec!["not-json".to_owned()],
        "native64",
        &[],
    );
    assert_eq!(error_code(&invalid_grammar), "SPX-PR501");
    assert_eq!(
        error_code(&input(
            &[(package, "=1.0.0")],
            vec!["not-json".to_owned()],
            "native64",
            &[]
        )),
        "SPX-PR502"
    );

    let high = subject(&report(package), package, "1.1.0", &[], &["denied"]);
    let low = subject(
        &report(package),
        package,
        "1.0.0",
        &[coordinate("resolver.precedence.missing", "1.0.0")],
        &[],
    );
    assert_eq!(
        error_code(&input(
            &[(package, "^1.0.0")],
            vec![high, low],
            "native64",
            &[]
        )),
        "SPX-PR503"
    );
}
