use super::support::*;

#[test]
fn public_limits_and_input_grammar_are_closed() {
    assert!(ResolutionOptions::new(4_096).is_ok());
    assert_eq!(ResolutionOptions::new(4_095).unwrap_err().code, "SPX-PR501");
    assert!(ResolutionOptions::new(MAX_OUTPUT_BYTES).is_ok());
    assert_eq!(
        ResolutionOptions::new(MAX_OUTPUT_BYTES + 1)
            .unwrap_err()
            .code,
        "SPX-PR501"
    );
    assert_eq!(ResolutionOptions::default().max_bytes, 16 * 1024 * 1024);
    assert_eq!(MAX_REQUIREMENTS, 4);
    assert_eq!(MAX_SUBJECTS, 64);
    assert_eq!(MAX_VERSIONS_PER_PACKAGE, 32);
    assert_eq!(MAX_SELECTED_PACKAGES, 4);
    assert_eq!(MAX_ALLOWED_CAPABILITIES, 256);
    assert_eq!(MAX_SUBJECT_BYTES, 17 * 1024 * 1024);
    assert_eq!(MAX_TOTAL_SUBJECT_BYTES, 128 * 1024 * 1024);
    assert_eq!(MAX_EDGES, 256);
    assert_eq!(MAX_DEPTH, 32);
    assert_eq!(MAX_DECISIONS, 4_096);
    assert_eq!(MAX_WORK_UNITS, 8 * 1024 * 1024);
    assert_eq!(MAX_JSON_DEPTH, 128);
    assert_eq!(MAX_RENDER_BYTES, 64 * 1024 * 1024);
    // The public graph is capped at four selected identities, so 256-edge and
    // depth-32 exact/+1 construction is intentionally unreachable here. Exact
    // helper boundaries remain owned by the core solver unit evidence. CLI
    // grammar, held-file, cumulative-read, and stdout evidence is a separate
    // lane and is not simulated through this authority-free Rust API.

    let package = "resolver.grammar";
    let one = subject(&report(package), package, "1.2.3", &[], &[]);
    for range in [
        "1.2.3",
        ">=1.2.3",
        "=01.2.3",
        "=1.02.3",
        "=1.2.03",
        "=1.2",
        "=1.2.3-alpha",
        "=1.2.3+build",
        "=1.*.3",
        "=1.2.3 ",
        "^4294967295.0.0",
        "~1.4294967295.0",
        "^0.0.4294967295",
    ] {
        assert_eq!(
            error_code(&input(
                &[(package, range)],
                vec![one.clone()],
                "native64",
                &[]
            )),
            "SPX-PR501",
            "range {range} must reject"
        );
    }
    assert_eq!(
        error_code(&input(
            &[("resolver.z", "=1.0.0"), (package, "=1.2.3")],
            vec![one.clone()],
            "native64",
            &[]
        )),
        "SPX-PR501"
    );
    assert_eq!(
        error_code(&ResolutionInput {
            requirements: vec![Requirement {
                package: package.to_owned(),
                range: "=1.2.3".to_owned(),
            }],
            subjects: vec![one.clone()],
            target: "linux".to_owned(),
            allowed_capabilities: vec![],
        }),
        "SPX-PR501"
    );
    assert_eq!(
        error_code(&input(
            &[(package, "=1.2.3")],
            vec![one],
            "native64",
            &["z", "a"]
        )),
        "SPX-PR501"
    );
}

#[test]
fn exact_caret_and_tilde_boundaries_select_numeric_versions() {
    let package = "resolver.semver";
    let report = report(package);
    let versions = [
        "0.0.3", "0.0.4", "0.2.3", "0.2.9", "0.3.0", "1.2.3", "1.2.9", "1.3.0", "1.9.9", "2.0.0",
    ];
    let subjects = versions
        .iter()
        .map(|version| subject(&report, package, version, &[], &[]))
        .collect::<Vec<_>>();
    let selected = |range: &str| {
        let request = input(&[(package, range)], subjects.clone(), "native64", &[]);
        package_resolver::verify(&generate(&request), &request, &ResolutionOptions::default())
            .unwrap()
            .packages[0]
            .version
            .clone()
    };
    assert_eq!(selected("=1.2.3"), "1.2.3");
    assert_eq!(selected("~1.2.3"), "1.2.9");
    assert_eq!(selected("^1.2.3"), "1.9.9");
    assert_eq!(selected("^0.2.3"), "0.2.9");
    assert_eq!(selected("^0.0.3"), "0.0.3");
}

#[test]
fn permutation_and_first_feasible_backtracking_are_deterministic() {
    let a = "resolver.backtrack.a";
    let b = "resolver.backtrack.b";
    let a_report = report(a);
    let b_report = report(b);
    let b1 = coordinate(b, "1.0.0");
    let b2 = coordinate(b, "2.0.0");
    let mut subjects = vec![
        subject(&a_report, a, "1.1.0", std::slice::from_ref(&b2), &[]),
        subject(&a_report, a, "1.0.0", std::slice::from_ref(&b1), &[]),
        subject(&b_report, b, "2.0.0", &[], &["denied"]),
        subject(&b_report, b, "1.0.0", &[], &[]),
    ];
    let forward = input(&[(a, "^1.0.0")], subjects.clone(), "native64", &[]);
    let forward_bytes = generate(&forward);
    subjects.reverse();
    let reverse = input(&[(a, "^1.0.0")], subjects, "native64", &[]);
    let reverse_bytes = generate(&reverse);
    assert_eq!(forward_bytes, reverse_bytes);
    assert_eq!(
        package_resolver::verify(&forward_bytes, &forward, &ResolutionOptions::default())
            .unwrap()
            .packages,
        vec![coordinate(a, "1.0.0"), b1]
    );
}
