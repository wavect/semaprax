use super::*;

#[test]
fn typed_public_generation_verification_and_application_keep_full_legacy_bytes() {
    let fixture = admitted_fixture("typed-report-parity");
    let old = semantic_workspace_change::generate(&fixture.root, &fixture.proposal).unwrap();
    let current =
        semantic_workspace_change::reporting::generate(&fixture.root, &fixture.proposal).unwrap();
    assert_eq!(old.evidence(), current.evidence());
    let evidence = fixture.root.join("typed-evidence.json");
    std::fs::write(&evidence, old.evidence()).unwrap();
    let old_receipt =
        semantic_workspace_change::verify(&fixture.root, &fixture.proposal, &evidence).unwrap();
    let current_receipt =
        semantic_workspace_change::reporting::verify(&fixture.root, &fixture.proposal, &evidence)
            .unwrap();
    assert_eq!(old_receipt, current_receipt);
    let receipt =
        semantic_workspace_change::reporting::apply(&fixture.root, &fixture.proposal, &evidence)
            .unwrap();
    let legacy_fixture = admitted_fixture("typed-report-legacy-apply");
    let legacy_artifacts =
        semantic_workspace_change::generate(&legacy_fixture.root, &legacy_fixture.proposal)
            .unwrap();
    let legacy_evidence = legacy_fixture.root.join("typed-evidence.json");
    std::fs::write(&legacy_evidence, legacy_artifacts.evidence()).unwrap();
    let legacy_receipt = semantic_workspace_change::apply(
        &legacy_fixture.root,
        &legacy_fixture.proposal,
        &legacy_evidence,
    )
    .unwrap();
    assert_eq!(receipt, legacy_receipt);
}

#[test]
fn typed_public_evidence_refusal_keeps_complete_original_diagnostics() {
    let fixture = Fixture::new("typed-report-negative");
    let evidence = fixture.root.join("invalid-evidence.json");
    std::fs::write(&evidence, "{}\n").unwrap();
    let old =
        semantic_workspace_change::verify(&fixture.root, &fixture.proposal, &evidence).unwrap_err();
    let current =
        semantic_workspace_change::reporting::verify(&fixture.root, &fixture.proposal, &evidence)
            .unwrap_err();
    assert_complete_report_equality(current.reported().unwrap(), old.as_slice());
}

// Complete semantic equality without the library's cfg(test)-only PartialEq.
fn assert_complete_report_equality(
    actual: &[semaprax::diagnostic::Diagnostic],
    expected: &[semaprax::diagnostic::Diagnostic],
) {
    use semaprax::{ast::Span, diagnostic::Diagnostic};
    assert_eq!(actual.len(), expected.len());
    for (actual, expected) in actual.iter().zip(expected) {
        // Exhaustive patterns ensure a future field cannot be silently omitted.
        let Diagnostic {
            code,
            severity,
            message,
            path,
            span,
            help,
        } = actual;
        let Diagnostic {
            code: expected_code,
            severity: expected_severity,
            message: expected_message,
            path: expected_path,
            span: expected_span,
            help: expected_help,
        } = expected;
        assert_eq!(code, expected_code);
        assert_eq!(severity, expected_severity);
        assert_eq!(message.as_ref(), expected_message.as_ref());
        assert_eq!(path.as_deref(), expected_path.as_deref());
        assert_eq!(
            span.map(
                |Span {
                     start,
                     end,
                     line,
                     column,
                 }| (start, end, line, column)
            ),
            expected_span.map(
                |Span {
                     start,
                     end,
                     line,
                     column,
                 }| (start, end, line, column)
            ),
        );
        assert_eq!(help.as_deref(), expected_help.as_deref());
    }
}

// An irrefutable Copy record pattern has the same field result and uses the
// existing Graph v13 family, avoiding a projected-loan Graph outside Change v1.
fn admitted_provider(source: &str, record: &str, increment: bool) -> String {
    let body = if increment {
        "{ value.value + 1 }"
    } else {
        "{ value.value }"
    };
    assert_eq!(source.matches(body).count(), 1);
    let result = if increment { "field + 1" } else { "field" };
    source.replacen(
        body,
        &format!("{{ match value {{ {record} {{ value: field }} => {result}, }} }}"),
        1,
    )
}

fn admitted_fixture(label: &str) -> Fixture {
    let serial = SERIAL.fetch_add(1, Ordering::Relaxed);
    let root = std::env::temp_dir().join(format!(
        "semaprax-semantic-workspace-change-v1-{label}-{}-{serial}",
        std::process::id()
    ));
    std::fs::create_dir(&root).unwrap();
    let provider = admitted_provider(provider_base(), "Point", false);
    for (path, source) in [
        ("a/provider.spx", provider.as_str()),
        ("m/consumer.spx", consumer_source()),
        ("z/entry.spx", entry_base()),
    ] {
        write_source(&root, path, source);
    }
    let path_set = root.join("paths.json");
    std::fs::write(&path_set, "{\"schema\":\"semaprax.workspace-semantic-path-set.v1\",\"files\":[{\"path\":\"a/provider.spx\"},{\"path\":\"m/consumer.spx\"},{\"path\":\"z/entry.spx\"}]}\n").unwrap();
    semantic_workspace::initialize(&root, &path_set).unwrap();
    let graph = workspace_graph::snapshot(&root, "change.entry").unwrap();
    for module in graph.modules() {
        assert!(
            matches!(
                module.source_graph_schema(),
                "semaprax.graph.v10"
                    | "semaprax.graph.v11"
                    | "semaprax.graph.v12"
                    | "semaprax.graph.v13"
                    | "semaprax.graph.v14"
            ),
            "actual positive fixture selected unsupported schema {}",
            module.source_graph_schema()
        );
    }
    let old_candidate = json(&canonical(provider_candidate(), "a/provider.spx"));
    let candidate = admitted_provider(provider_candidate(), "Metric", true);
    let new_candidate = json(&canonical(&candidate, "a/provider.spx"));
    let old_proposal = proposal_source(&root);
    assert_eq!(old_proposal.matches(&old_candidate).count(), 1);
    let proposal = root.join("change.json");
    std::fs::write(
        &proposal,
        old_proposal.replacen(&old_candidate, &new_candidate, 1),
    )
    .unwrap();
    Fixture { root, proposal }
}

#[test]
fn full_original_projected_record_fixture_keeps_unsupported_graph_refusal_and_legacy_report() {
    let fixture = Fixture::new("typed-original-unsupported");
    let before = inventory(&fixture.root);
    let old = semantic_workspace_change::generate(&fixture.root, &fixture.proposal)
        .err()
        .expect("full original unsupported proposal must refuse");
    let current = semantic_workspace_change::reporting::generate(&fixture.root, &fixture.proposal)
        .err()
        .expect("full original unsupported proposal must refuse");
    assert_eq!(old[0].code, "SPX-G181");
    assert_eq!(
        old[0].message,
        "Semantic Workspace change base source Graph schema is unsupported"
    );
    assert_complete_report_equality(current.reported().unwrap(), old.as_slice());
    assert_eq!(inventory(&fixture.root), before);
}
