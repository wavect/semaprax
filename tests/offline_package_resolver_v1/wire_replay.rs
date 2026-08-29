use super::support::*;

#[test]
fn subject_and_nested_report_remints_do_not_bypass_source_replay() {
    const SUBJECT_SCHEMA: &str = "semaprax.offline-semantic-package-subject.v2";
    const SUBJECT_DOMAIN: &[u8] = b"semaprax.offline-semantic-package-subject.v2\0";
    const REPORT_SCHEMA: &str = "semaprax.semantic-package-report.v2";
    const REPORT_PAYLOAD_DOMAIN: &[u8] = b"semaprax.package-report-v2.payload.v1\0";
    const SUBJECT_REPORT_DOMAIN: &[u8] = b"semaprax.offline-semantic-package-report.v2\0";
    let package = "resolver.auth";
    let report = report(package);
    let subject = subject(&report, package, "1.0.0", &[], &[]);

    let changed_payload = payload(&subject).replacen(package, "resolver.authx", 1);
    let forged_subject = remint(SUBJECT_SCHEMA, SUBJECT_DOMAIN, &changed_payload);
    assert_eq!(
        error_code(&input(
            &[(package, "=1.0.0")],
            vec![forged_subject],
            "native64",
            &[]
        )),
        "SPX-PR502"
    );

    let changed_report_payload =
        payload(&report).replacen("\"ownership\":\"value\"", "\"ownership\":\"own\"", 1);
    let forged_report = remint(
        REPORT_SCHEMA,
        REPORT_PAYLOAD_DOMAIN,
        &changed_report_payload,
    );
    let old_report_digest = domain_digest(SUBJECT_REPORT_DOMAIN, report.as_bytes());
    let new_report_digest = domain_digest(SUBJECT_REPORT_DOMAIN, forged_report.as_bytes());
    let changed_subject_payload = payload(&subject)
        .replacen(&old_report_digest, &new_report_digest, 1)
        .replacen(
            &format!("\"report_bytes\":{}", report.len()),
            &format!("\"report_bytes\":{}", forged_report.len()),
            1,
        )
        .replacen(&report, &forged_report, 1);
    let forged_nested = remint(SUBJECT_SCHEMA, SUBJECT_DOMAIN, &changed_subject_payload);
    assert_eq!(
        error_code(&input(
            &[(package, "=1.0.0")],
            vec![forged_nested],
            "native64",
            &[]
        )),
        "SPX-PR502"
    );
}

#[test]
fn wire_mutation_remint_truncation_insertion_and_input_drift_are_rejected() {
    const DOMAIN: &[u8] = b"semaprax.offline-package-resolution-evidence.v1\0";
    let package = "resolver.wire";
    let request = input(
        &[(package, "=1.0.0")],
        vec![subject(&report(package), package, "1.0.0", &[], &[])],
        "native64",
        &[],
    );
    let evidence = generate(&request);
    let options = ResolutionOptions::default();
    assert!(evidence.starts_with(
        "{\"schema\":\"semaprax.offline-package-resolution-evidence.v1\",\"digest\":\"sha256:"
    ));
    assert!(evidence.contains(
        "\"nonclaims\":[\"offline_deterministic_resolution_evidence\",\"no_registry_network_fetch_build_script_execution_cache_or_publication\",\"capability_allowlist_is_resolution_admission_not_runtime_enforcement\",\"target_availability_is_projection_not_execution\",\"evidence_is_not_authority\"]"
    ));
    let receipt = package_resolver::verify(&evidence, &request, &options).unwrap();
    assert!(evidence.contains(&format!("\"lock\":{}", receipt.lock)));
    let missing_key = evidence.replacen(
        "\"schema\":\"semaprax.offline-package-resolution-evidence.v1\",",
        "",
        1,
    );
    let unknown_key = evidence.replacen('{', "{\"unknown\":0,", 1);
    let duplicate_key = evidence.replacen(
        '{',
        "{\"schema\":\"semaprax.offline-package-resolution-evidence.v1\",",
        1,
    );
    for malformed in [
        missing_key,
        unknown_key,
        duplicate_key,
        format!("\u{feff}{evidence}"),
        format!("{evidence}\r"),
        evidence.replacen('{', "{ ", 1),
    ] {
        assert_eq!(
            package_resolver::verify(&malformed, &request, &options)
                .unwrap_err()
                .code,
            "SPX-PR506"
        );
    }
    assert_eq!(
        package_resolver::verify(&evidence[..evidence.len() - 1], &request, &options)
            .unwrap_err()
            .code,
        "SPX-PR506"
    );
    assert_eq!(
        package_resolver::verify(&(evidence.clone() + " "), &request, &options)
            .unwrap_err()
            .code,
        "SPX-PR506"
    );
    let mutated = evidence.replacen("\"target\":\"native64\"", "\"target\":\"wasm32\"", 1);
    assert_eq!(
        package_resolver::verify(&mutated, &request, &options)
            .unwrap_err()
            .code,
        "SPX-PR507"
    );
    let reminted_payload =
        payload(&evidence).replacen("\"target\":\"native64\"", "\"target\":\"wasm32\"", 1);
    let reminted = remint(package_resolver::SCHEMA, DOMAIN, &reminted_payload);
    assert_eq!(
        package_resolver::verify(&reminted, &request, &options)
            .unwrap_err()
            .code,
        "SPX-PR507"
    );
    let mut drifted = request.clone();
    drifted.allowed_capabilities.push("drift".to_owned());
    assert_eq!(
        package_resolver::verify(&evidence, &drifted, &options)
            .unwrap_err()
            .code,
        "SPX-PR507"
    );

    let substitute_package = "resolver.wire.substitute";
    let substituted = input(
        &[(substitute_package, "=1.0.0")],
        vec![subject(
            &report(substitute_package),
            substitute_package,
            "1.0.0",
            &[],
            &[],
        )],
        "native64",
        &[],
    );
    assert_eq!(
        package_resolver::verify(&evidence, &substituted, &options)
            .unwrap_err()
            .code,
        "SPX-PR507"
    );
    let substitute_evidence = generate(&substituted);
    let substitute_value: serde_json::Value = serde_json::from_str(&substitute_evidence).unwrap();
    let substitute_catalog_digest = substitute_value["payload"]["catalog"]["digest"]
        .as_str()
        .unwrap();
    let original_value: serde_json::Value = serde_json::from_str(&evidence).unwrap();
    let original_catalog_digest = original_value["payload"]["catalog"]["digest"]
        .as_str()
        .unwrap();
    let catalog_remint = remint(
        package_resolver::SCHEMA,
        DOMAIN,
        &payload(&evidence).replacen(original_catalog_digest, substitute_catalog_digest, 1),
    );
    assert_eq!(
        package_resolver::verify(&catalog_remint, &request, &options)
            .unwrap_err()
            .code,
        "SPX-PR507"
    );
}

#[test]
fn minimal_evidence_has_an_independent_canonical_wire_oracle() {
    const DOMAIN: &[u8] = b"semaprax.offline-package-resolution-evidence.v1\0";
    let package = "resolver.oracle";
    let report = report(package);
    let subject = subject(&report, package, "1.0.0", &[], &[]);
    let request = input(
        &[(package, "=1.0.0")],
        vec![subject.clone()],
        "native64",
        &[],
    );
    let evidence = generate(&request);
    let value: serde_json::Value = serde_json::from_str(&evidence).unwrap();
    let raw_payload = payload(&evidence);
    assert_eq!(
        evidence,
        remint(package_resolver::SCHEMA, DOMAIN, raw_payload)
    );
    assert_ordered(
        &evidence,
        &["\"schema\":", "\"digest\":", "\"bytes\":", "\"payload\":"],
    );
    assert_ordered(
        raw_payload,
        &[
            "\"schema\":",
            "\"requirements\":",
            "\"target\":",
            "\"allowed_capabilities\":",
            "\"catalog\":",
            "\"selected\":",
            "\"lock_digest\":",
            "\"lock_bytes\":",
            "\"lock\":",
        ],
    );
    let catalog = &value["payload"]["catalog"];
    let expected_catalog_digest = catalog_digest(&[&subject]);
    assert_eq!(catalog["subjects"].as_u64(), Some(1));
    assert_eq!(catalog["bytes"].as_u64(), Some(subject.len() as u64));
    assert_eq!(
        catalog["digest"].as_str(),
        Some(expected_catalog_digest.as_str())
    );
    assert!(raw_payload.contains(&format!(
        "\"catalog\":{{\"subjects\":1,\"bytes\":{},\"digest\":\"{}\"}}",
        subject.len(),
        expected_catalog_digest
    )));
    let selected = value["payload"]["selected"].as_array().unwrap();
    assert_eq!(selected.len(), 1);
    assert_eq!(selected[0]["package"].as_str(), Some(package));
    assert_eq!(selected[0]["version"].as_str(), Some("1.0.0"));
    let subject_value: serde_json::Value = serde_json::from_str(&subject).unwrap();
    assert_eq!(
        selected[0]["subject_digest"].as_str(),
        subject_value["digest"].as_str()
    );
    assert_eq!(
        selected[0]["subject_bytes"].as_u64(),
        Some(subject.len() as u64)
    );
    assert!(raw_payload.contains(&format!(
        "\"selected\":[{{\"package\":\"{package}\",\"version\":\"1.0.0\",\"subject_digest\":\"{}\",\"subject_bytes\":{}}}]",
        subject_value["digest"].as_str().unwrap(),
        subject.len()
    )));

    let lock = &value["payload"]["lock"];
    assert_eq!(
        value["payload"]["lock_digest"].as_str(),
        lock["digest"].as_str()
    );
    let receipt =
        package_resolver::verify(&evidence, &request, &ResolutionOptions::default()).unwrap();
    assert_eq!(
        value["payload"]["lock_bytes"].as_u64(),
        Some(receipt.lock.len() as u64)
    );
    assert!(raw_payload.contains(&format!("\"lock\":{},\"limits\":", receipt.lock)));

    let limits = &value["payload"]["limits"];
    let limits_raw = between(raw_payload, "\"limits\":", ",\"budget\":");
    assert_ordered(
        limits_raw,
        &[
            "max_requirements",
            "max_subjects",
            "max_versions_per_package",
            "max_selected_packages",
            "max_allowed_capabilities",
            "max_subject_bytes",
            "max_total_subject_bytes",
            "max_edges",
            "max_depth",
            "max_decisions",
            "max_work_units",
            "max_json_depth",
            "max_render_bytes",
            "max_output_bytes",
            "requested_max_bytes",
        ],
    );
    for (key, expected) in [
        ("max_requirements", MAX_REQUIREMENTS),
        ("max_subjects", MAX_SUBJECTS),
        ("max_versions_per_package", MAX_VERSIONS_PER_PACKAGE),
        ("max_selected_packages", MAX_SELECTED_PACKAGES),
        ("max_allowed_capabilities", MAX_ALLOWED_CAPABILITIES),
        ("max_subject_bytes", MAX_SUBJECT_BYTES),
        ("max_total_subject_bytes", MAX_TOTAL_SUBJECT_BYTES),
        ("max_edges", MAX_EDGES),
        ("max_depth", MAX_DEPTH),
        ("max_decisions", MAX_DECISIONS),
        ("max_work_units", MAX_WORK_UNITS),
        ("max_json_depth", MAX_JSON_DEPTH),
        ("max_render_bytes", MAX_RENDER_BYTES),
        ("max_output_bytes", MAX_OUTPUT_BYTES),
        ("requested_max_bytes", MAX_OUTPUT_BYTES),
    ] {
        assert_eq!(limits[key].as_u64(), Some(expected as u64), "{key}");
    }
    let source_bytes = serde_json::from_str::<serde_json::Value>(&report).unwrap()["payload"]
        ["source"]["bytes"]
        .as_u64()
        .unwrap();
    let budget = &value["payload"]["budget"];
    let budget_raw = between(raw_payload, "\"budget\":", ",\"nonclaims\":");
    assert_ordered(
        budget_raw,
        &[
            "used_subjects",
            "used_subject_bytes",
            "used_selected_packages",
            "used_edges",
            "used_depth",
            "used_decisions",
            "used_allowed_capabilities",
            "used_work_units",
        ],
    );
    assert_eq!(budget["used_subjects"].as_u64(), Some(1));
    assert_eq!(
        budget["used_subject_bytes"].as_u64(),
        Some(subject.len() as u64)
    );
    assert_eq!(budget["used_selected_packages"].as_u64(), Some(1));
    assert_eq!(budget["used_edges"].as_u64(), Some(0));
    assert_eq!(budget["used_depth"].as_u64(), Some(1));
    assert_eq!(budget["used_decisions"].as_u64(), Some(1));
    assert_eq!(budget["used_allowed_capabilities"].as_u64(), Some(0));
    assert_eq!(budget["used_work_units"].as_u64(), Some(source_bytes + 11));
}
