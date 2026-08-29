use super::*;

#[test]
fn digest_domains_have_frozen_known_answers() {
    assert_eq!(
        domain_digest(SUBJECT_DIGEST_DOMAIN, b"abc"),
        "sha256:3a5bc22fed7d57475195df02bd54d7546a0cc6f858e6ff07568e3212ffeb5bec"
    );
    assert_eq!(
        domain_digest(LOCK_DIGEST_DOMAIN, b"abc"),
        "sha256:26327840e0dde3792fb76ee174ec079aebe3098ede3cbc26b2fe9478efc81a5f"
    );
}

#[test]
fn closed_scalar_grammars_reject_confusion() {
    for accepted in ["pkg", "pkg.core", "_private.v1"] {
        validate_package_identity(accepted).unwrap();
    }
    for rejected in ["", ".pkg", "pkg.", "pkg..core", "9pkg", "pkg-core"] {
        assert!(validate_package_identity(rejected).is_err());
    }
    for accepted in ["0.0.0", "1.2.3", "4294967295.0.1"] {
        validate_version(accepted).unwrap();
    }
    for rejected in ["1", "1.2", "01.2.3", "1.02.3", "1.2.03", "1.2.3-alpha"] {
        assert!(validate_version(rejected).is_err());
    }
}

#[test]
fn capability_and_optional_fact_vocabularies_are_closed() {
    for accepted in ["filesystem", "network.read", "process.spawn-tool"] {
        validate_capability(accepted).unwrap();
    }
    for rejected in ["", "audit.write", "network.", "network..read"] {
        assert!(validate_capability(rejected).is_err());
    }
    assert!(validate_license("MIT").is_ok());
    assert!(validate_license("").is_err());
}

#[test]
fn output_budget_nonconvergence_fails_closed() {
    let error = converge_output_budget(Budget::default(), 100, 2, |budget| {
        if budget.output_bytes == 0 {
            "x".to_owned()
        } else {
            "xx".to_owned()
        }
    })
    .unwrap_err();
    assert_eq!(error.code, "SPX-L407");
}

#[test]
fn frozen_limit_helpers_accept_exact_and_reject_one_more() {
    validate_package_count(MAX_PACKAGES).unwrap();
    assert_eq!(
        validate_package_count(MAX_PACKAGES + 1).unwrap_err().code,
        "SPX-L406"
    );

    for (label, maximum) in [
        ("subject_bytes", MAX_SUBJECT_BYTES),
        ("total_subject_bytes", MAX_TOTAL_SUBJECT_BYTES),
        ("dependencies_per_package", MAX_DEPENDENCIES_PER_PACKAGE),
        ("dependency_edges", MAX_EDGES),
        ("dependency_depth", MAX_DEPENDENCY_DEPTH),
        ("capabilities", MAX_CAPABILITIES),
        ("capability_closure", MAX_CAPABILITIES),
        ("licenses", MAX_LICENSES),
        ("provenance", MAX_PROVENANCE),
    ] {
        ensure_at_most(maximum, maximum, label).unwrap();
        assert_eq!(
            ensure_at_most(maximum + 1, maximum, label)
                .unwrap_err()
                .code,
            "SPX-L406",
            "{label}"
        );
    }

    let mut exact_work = 0usize;
    checked_add(
        &mut exact_work,
        MAX_WORK_UNITS,
        MAX_WORK_UNITS,
        "builder_work_units",
    )
    .unwrap();
    assert_eq!(exact_work, MAX_WORK_UNITS);
    assert_eq!(
        checked_add(&mut exact_work, 1, MAX_WORK_UNITS, "builder_work_units")
            .unwrap_err()
            .code,
        "SPX-L406"
    );

    let package = Coordinate {
        package: "p".to_owned(),
        version: "v".to_owned(),
    };
    let dependency = Coordinate {
        package: "d".to_owned(),
        version: "1".to_owned(),
    };
    let subject = PackageSubject {
        coordinate: package.clone(),
        subject_digest: "sd".to_owned(),
        subject_bytes: 7,
        report_digest: "rd".to_owned(),
        report_bytes: 11,
        report_envelope_digest: "ed".to_owned(),
        targets: vec![TargetFact {
            target: "t".to_owned(),
            available: true,
        }],
        dependencies: vec![dependency.clone()],
        capabilities: vec!["c".to_owned()],
        licenses: vec!["l".to_owned()],
        provenance: vec![ProvenanceFact {
            kind: "k".to_owned(),
            value: "v".to_owned(),
        }],
    };
    let packages = BTreeMap::from([(package.clone(), subject)]);
    let identities = BTreeMap::from([("p".to_owned(), "v".to_owned())]);
    let order = vec![package.clone()];
    let depth = BTreeMap::from([(package.clone(), 1)]);
    let closures = BTreeMap::from([(package.clone(), BTreeSet::from(["c".to_owned()]))]);
    let capabilities = BTreeSet::from(["c".to_owned()]);
    let targets = vec![TargetFact {
        target: "t".to_owned(),
        available: true,
    }];
    let depended_on = BTreeSet::from([dependency.clone()]);
    let roots = vec![package.clone()];
    let edges = vec![(dependency, package)];
    let account = |subject_bytes| {
        retained_state_bytes(
            subject_bytes,
            &packages,
            &identities,
            &order,
            &depth,
            &closures,
            &capabilities,
            &targets,
            &depended_on,
            &roots,
            &edges,
        )
    };
    // 96 fixed bytes + 7 subject bytes + 62 bytes across every nonempty
    // retained package/identity/order/depth/closure/target/root/edge location.
    assert_eq!(account(7).unwrap(), 165);
    let exact_subject_bytes = MAX_BUILDER_BYTES - (165 - 7);
    assert_eq!(account(exact_subject_bytes).unwrap(), MAX_BUILDER_BYTES);
    assert_eq!(
        account(exact_subject_bytes + 1).unwrap_err().code,
        "SPX-L406"
    );

    let exact_depth = format!(
        "{}0{}",
        "[".repeat(MAX_JSON_DEPTH),
        "]".repeat(MAX_JSON_DEPTH)
    );
    validate_json_wire(&exact_depth, "depth fixture").unwrap();
    let excess_depth = format!(
        "{}0{}",
        "[".repeat(MAX_JSON_DEPTH + 1),
        "]".repeat(MAX_JSON_DEPTH + 1)
    );
    assert_eq!(
        validate_json_wire(&excess_depth, "depth fixture")
            .unwrap_err()
            .code,
        "SPX-L406"
    );

    let exact_output = converge_output_budget(Budget::default(), MAX_OUTPUT_BYTES, 2, |_| {
        "x".repeat(MAX_OUTPUT_BYTES)
    })
    .unwrap();
    assert_eq!(exact_output.len(), MAX_OUTPUT_BYTES);
    assert_eq!(
        converge_output_budget(Budget::default(), MAX_OUTPUT_BYTES, 1, |_| {
            "x".repeat(MAX_OUTPUT_BYTES + 1)
        })
        .unwrap_err()
        .code,
        "SPX-L406"
    );
}
