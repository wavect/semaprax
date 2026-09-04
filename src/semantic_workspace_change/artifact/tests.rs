use std::fs::OpenOptions;
use std::io::Write as _;

use serde_json::Value;

use super::*;
use crate::semantic_workspace_change::tests::Fixture;

fn raw_sha(bytes: &str) -> String {
    format!(
        "sha256:{:x}",
        crate::digest_hex::LowerHex(Sha256::digest(bytes.as_bytes()))
    )
}

fn top_keys(source: &str) -> Vec<String> {
    let bytes = source.as_bytes();
    let mut keys = Vec::new();
    let mut depth = 0usize;
    let mut index = 0usize;
    while index < bytes.len() {
        match bytes[index] {
            b'{' | b'[' => {
                depth += 1;
                index += 1;
            }
            b'}' | b']' => {
                depth -= 1;
                index += 1;
            }
            b'"' => {
                let start = index + 1;
                index += 1;
                let mut escape = false;
                while index < bytes.len() {
                    if escape {
                        escape = false;
                    } else if bytes[index] == b'\\' {
                        escape = true;
                    } else if bytes[index] == b'"' {
                        break;
                    }
                    index += 1;
                }
                let end = index;
                index += 1;
                if depth == 1 && bytes.get(index) == Some(&b':') {
                    keys.push(source[start..end].to_owned());
                }
            }
            _ => index += 1,
        }
    }
    keys
}

fn assert_reference(value: &Value, key: &str, artifact: &Artifact) {
    let reference = &value[key];
    assert_eq!(reference["schema"], artifact.schema);
    assert_eq!(reference["digest"], artifact.digest);
    assert_eq!(reference["bytes"], artifact.bytes.len());
}

#[test]
fn literal_kats_wire_order_domains_and_reference_parity() {
    let fixture = Fixture::new("artifact-kats");
    let proposal = fixture.proposal();
    let proposal_source = proposal.source().to_owned();
    let prepared = super::super::build_authenticated_change(
        &fixture.root,
        super::super::parse_proposal(&proposal_source).unwrap(),
    )
    .unwrap();
    let candidate_manifest = prepared.candidate_manifest().to_owned();
    let artifacts = build_authenticated_artifacts(&fixture.root, proposal).unwrap();

    assert_eq!(
        artifacts.proposal_digest(),
        digest(PROPOSAL_DIGEST_DOMAIN, proposal_source.as_bytes())
    );
    assert_eq!(
        artifacts.candidate_manifest_digest(),
        digest(
            CANDIDATE_MANIFEST_DIGEST_DOMAIN,
            candidate_manifest.as_bytes()
        )
    );
    for (artifact, domain) in [
        (&artifacts.preview, PREVIEW_DIGEST_DOMAIN),
        (&artifacts.context, CONTEXT_DIGEST_DOMAIN),
        (&artifacts.impact, IMPACT_DIGEST_DOMAIN),
        (&artifacts.review, REVIEW_DIGEST_DOMAIN),
        (&artifacts.evidence, EVIDENCE_DIGEST_DOMAIN),
    ] {
        assert_eq!(artifact.digest, digest(domain, artifact.bytes.as_bytes()));
        assert!(artifact.bytes.ends_with('\n'));
        assert!(!artifact.bytes[..artifact.bytes.len() - 1].contains('\n'));
        let mut mutated = artifact.bytes.clone().into_bytes();
        let middle = mutated.len() / 2;
        mutated[middle] ^= 1;
        assert_ne!(artifact.digest, digest(domain, &mutated));
    }

    assert_eq!(
        [
            raw_sha(artifacts.preview()),
            raw_sha(artifacts.context()),
            raw_sha(artifacts.impact()),
            raw_sha(artifacts.review()),
            raw_sha(artifacts.evidence()),
        ],
        [
            "sha256:deebc462e4fe518ea8e6cad524482b0462b27e2df1fd1a7efdb99defb213a3c8",
            "sha256:eaaefd773d821dafeaf4ffc966bdb23912ae178ed506be013162bf2f70090462",
            "sha256:a751292986c411a535eec6e0de30c2610919825d315a717eebd5423998c5efef",
            "sha256:047948e62089a75d12becc88c8dc8bf44e24a1521991e99c667d76a3ec814400",
            "sha256:7e7a27238f22e8347bada8c69d27b0f7aafd98a7264dc336f7c844f91698647d"
        ]
    );

    assert_eq!(
        top_keys(artifacts.preview()),
        [
            "schema",
            "workspace_manifest_schema",
            "base_workspace_revision",
            "candidate_workspace_revision",
            "entry_module",
            "proposal",
            "base_workspace_graph",
            "candidate_workspace_graph",
            "candidate_manifest",
            "files",
            "delta",
            "limits",
            "budget",
            "nonclaims",
        ]
    );
    assert_eq!(
        top_keys(artifacts.context()),
        [
            "schema",
            "base_workspace_revision",
            "candidate_workspace_revision",
            "entry_module",
            "proposal",
            "change_preview",
            "nodes",
            "limits",
            "budget",
            "nonclaims",
        ]
    );
    assert_eq!(
        top_keys(artifacts.impact()),
        [
            "schema",
            "base_workspace_revision",
            "candidate_workspace_revision",
            "entry_module",
            "proposal",
            "change_preview",
            "context",
            "affected",
            "dependency_edges",
            "limits",
            "budget",
            "nonclaims",
        ]
    );
    assert_eq!(
        top_keys(artifacts.review()),
        [
            "schema",
            "base_workspace_revision",
            "candidate_workspace_revision",
            "entry_module",
            "proposal",
            "change_preview",
            "context",
            "impact",
            "sections",
            "evidence",
            "limits",
            "budget",
            "nonclaims",
        ]
    );
    assert_eq!(
        top_keys(artifacts.evidence()),
        [
            "schema",
            "workspace_manifest_schema",
            "base_workspace_revision",
            "candidate_workspace_revision",
            "entry_module",
            "proposal",
            "base_workspace_graph",
            "candidate_workspace_graph",
            "candidate_manifest",
            "change_preview",
            "context",
            "impact",
            "review",
            "files",
            "limits",
            "budget",
            "nonclaims",
        ]
    );

    let preview: Value = serde_json::from_str(artifacts.preview()).unwrap();
    let context: Value = serde_json::from_str(artifacts.context()).unwrap();
    let impact: Value = serde_json::from_str(artifacts.impact()).unwrap();
    let review: Value = serde_json::from_str(artifacts.review()).unwrap();
    let evidence: Value = serde_json::from_str(artifacts.evidence()).unwrap();
    for value in [&preview, &context, &impact, &review, &evidence] {
        assert_eq!(value["proposal"]["digest"], artifacts.proposal_digest());
        assert_eq!(value["proposal"]["bytes"], proposal_source.len());
    }
    assert_reference(&context, "change_preview", &artifacts.preview);
    assert_reference(&impact, "change_preview", &artifacts.preview);
    assert_reference(&impact, "context", &artifacts.context);
    assert_reference(&review, "change_preview", &artifacts.preview);
    assert_reference(&review, "context", &artifacts.context);
    assert_reference(&review, "impact", &artifacts.impact);
    assert_reference(&evidence, "change_preview", &artifacts.preview);
    assert_reference(&evidence, "context", &artifacts.context);
    assert_reference(&evidence, "impact", &artifacts.impact);
    assert_reference(&evidence, "review", &artifacts.review);
    assert_eq!(
        preview["delta"]["roots"].as_array().unwrap().len(),
        prepared.roots().len()
    );
    assert_eq!(
        preview["delta"]["edges"].as_array().unwrap().len(),
        prepared.delta_edges().len()
    );
    assert_eq!(
        context["nodes"].as_array().unwrap().len(),
        prepared.context_nodes().len()
    );
    assert_eq!(
        impact["affected"].as_array().unwrap().len(),
        prepared.impact().len()
    );

    let mut escaped = CappedString::new();
    push_json(&mut escaped, "quote\" slash\\ lf\n cr\r tab\t \u{0001}");
    assert_eq!(
        escaped.into_string(),
        "\"quote\\\" slash\\\\ lf\\n cr\\r tab\\t \\u0001\""
    );
}

#[test]
fn exact_output_and_builder_caps_and_complete_only_children() {
    let fixture = Fixture::new("artifact-limits");
    let artifacts = build_authenticated_artifacts(&fixture.root, fixture.proposal()).unwrap();
    for (source, schema, domain, field) in [
        (
            &artifacts.preview,
            PREVIEW_SCHEMA,
            PREVIEW_DIGEST_DOMAIN,
            "change_preview_bytes",
        ),
        (
            &artifacts.context,
            CONTEXT_SCHEMA,
            CONTEXT_DIGEST_DOMAIN,
            "context_bytes",
        ),
        (
            &artifacts.impact,
            IMPACT_SCHEMA,
            IMPACT_DIGEST_DOMAIN,
            "impact_bytes",
        ),
        (
            &artifacts.review,
            REVIEW_SCHEMA,
            REVIEW_DIGEST_DOMAIN,
            "review_bytes",
        ),
        (
            &artifacts.evidence,
            EVIDENCE_SCHEMA,
            EVIDENCE_DIGEST_DOMAIN,
            "evidence_bytes",
        ),
    ] {
        let exact = artifact(schema, domain, source.bytes.len(), field, |output| {
            output.push_str(&source.bytes)
        })
        .unwrap();
        assert_eq!(exact, *source);
        let error = artifact(schema, domain, source.bytes.len() - 1, field, |output| {
            output.push_str(&source.bytes)
        })
        .unwrap_err();
        assert_eq!(error[0].code, "SPX-G183");
    }

    let mut exact_builder =
        super::super::build_authenticated_change(&fixture.root, fixture.proposal()).unwrap();
    exact_builder.used_builder_bytes = MAX_ANALYSIS_BUILDER_BYTES;
    let exact = render_artifacts(&exact_builder).unwrap();
    let evidence: Value = serde_json::from_str(exact.evidence()).unwrap();
    assert_eq!(
        evidence["budget"]["used_analysis_builder_bytes"],
        MAX_ANALYSIS_BUILDER_BYTES
    );
    let mut over_builder =
        super::super::build_authenticated_change(&fixture.root, fixture.proposal()).unwrap();
    over_builder.used_builder_bytes = MAX_ANALYSIS_BUILDER_BYTES + 1;
    let error = render_artifacts(&over_builder)
        .err()
        .expect("over-limit builder must fail");
    assert_eq!(error[0].code, "SPX-G183");
    assert_eq!(
        error[0].message,
        "Semantic Workspace Change `analysis_builder_bytes` exceeds 33554432"
    );

    let mut incomplete_context =
        super::super::build_authenticated_change(&fixture.root, fixture.proposal()).unwrap();
    let root = &incomplete_context.roots[0];
    let index = incomplete_context
        .context_nodes
        .iter()
        .position(|node| node.state == root.state && node.kind == root.kind && node.id == root.id)
        .unwrap();
    incomplete_context.context_nodes.remove(index);
    let error = render_artifacts(&incomplete_context)
        .err()
        .expect("incomplete Context must fail");
    assert_eq!(error[0].code, "SPX-G186");
    let mut incomplete_provenance =
        super::super::build_authenticated_change(&fixture.root, fixture.proposal()).unwrap();
    incomplete_provenance.impact[0]
        .root_provenance
        .push(incomplete_provenance.roots.len());
    let error = render_artifacts(&incomplete_provenance)
        .err()
        .expect("incomplete provenance must fail");
    assert_eq!(error[0].code, "SPX-G186");
}

#[test]
fn after_render_drift_discards_artifacts_and_releases_authority() {
    let fixture = Fixture::new("artifact-final-recheck");
    let called = std::cell::Cell::new(false);
    let result =
        build_authenticated_artifacts_with_hook(&fixture.root, fixture.proposal(), |artifacts| {
            called.set(true);
            assert!(!artifacts.evidence().is_empty());
            OpenOptions::new()
                .append(true)
                .open(fixture.root.join(".semaprax-workspace/ACTIVE"))
                .unwrap()
                .write_all(b"x")
                .unwrap();
        });
    assert!(called.get());
    let error = result.err().unwrap();
    assert_eq!(error.len(), 1);
    assert_eq!(error[0].code, "SPX-G153");
    assert_eq!(
        error[0].message,
        "workspace object changed during authentication"
    );
    fixture.assert_exclusive_reacquire();
}
