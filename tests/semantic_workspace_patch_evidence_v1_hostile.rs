use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

use semaprax::{workspace, workspace_patch_evidence};

static SERIAL: AtomicU64 = AtomicU64::new(0);

struct SourceCase<'a> {
    path: &'a str,
    source: &'a str,
    target: &'a str,
    renamed: &'a str,
}

struct Fixture {
    root: PathBuf,
    patch: PathBuf,
    evidence: PathBuf,
}

impl Fixture {
    fn new(label: &str, sources: &[SourceCase<'_>]) -> Self {
        let serial = SERIAL.fetch_add(1, Ordering::Relaxed);
        let root = std::env::temp_dir().join(format!(
            "semaprax-workspace-patch-evidence-hostile-{label}-{}-{serial}",
            std::process::id()
        ));
        std::fs::create_dir(&root).unwrap();

        let mut paths = Vec::with_capacity(sources.len());
        let mut patches = Vec::with_capacity(sources.len());
        for source_case in sources {
            let source = canonical(source_case.source, source_case.path);
            std::fs::write(root.join(source_case.path), &source).unwrap();
            paths.push(source_case.path);
            patches.push(format!(
                "base {}\nrename {} to {}\n",
                revision(&source, source_case.path),
                source_case.target,
                source_case.renamed
            ));
        }

        let path_set = root.join("paths.json");
        let files = paths
            .iter()
            .map(|path| format!("{{\"path\":{}}}", serde_json::to_string(path).unwrap()))
            .collect::<Vec<_>>()
            .join(",");
        std::fs::write(
            &path_set,
            format!("{{\"schema\":\"semaprax.workspace-path-set.v1\",\"files\":[{files}]}}\n"),
        )
        .unwrap();
        let workspace_revision = workspace::initialize(&root, &path_set).unwrap();

        let patch = root.join("change.wspatch");
        let files = paths
            .iter()
            .zip(patches)
            .map(|(path, patch)| {
                format!(
                    "{{\"path\":{},\"patch\":{}}}",
                    serde_json::to_string(path).unwrap(),
                    serde_json::to_string(&patch).unwrap()
                )
            })
            .collect::<Vec<_>>()
            .join(",");
        std::fs::write(
            &patch,
            format!(
                "{{\"schema\":\"semaprax.semantic-workspace-patch.v1\",\"base_workspace_revision\":\"{workspace_revision}\",\"files\":[{files}]}}\n"
            ),
        )
        .unwrap();

        Self {
            evidence: root.join("evidence.json"),
            root,
            patch,
        }
    }

    fn verify_code(&self, evidence: &str) -> &'static str {
        std::fs::write(&self.evidence, evidence).unwrap();
        workspace_patch_evidence::verify(&self.root, &self.patch, &self.evidence)
            .expect_err("hostile evidence must reject")[0]
            .code
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.root);
    }
}

fn canonical(source: &str, path: &str) -> String {
    semaprax::format::canonical(&semaprax::parse(source, path).unwrap())
}

fn revision(source: &str, path: &str) -> String {
    semaprax::graph::revision(&semaprax::parse(source, path).unwrap())
}

fn replace_once(source: &str, old: &str, new: &str) -> String {
    assert!(source.contains(old), "mutation anchor must exist: {old}");
    source.replacen(old, new, 1)
}

fn alternate_digest(digest: &str) -> String {
    assert_eq!(digest.len(), 71);
    let mut changed = digest.to_owned();
    let replacement = if changed.ends_with('0') { '1' } else { '0' };
    changed.pop();
    changed.push(replacement);
    changed
}

fn reaccount_capsule(original: &str, mutation: &str) -> String {
    replace_once(
        mutation,
        &format!("\"used_workspace_evidence_bytes\":{}", original.len()),
        &format!("\"used_workspace_evidence_bytes\":{}", mutation.len()),
    )
}

fn standard_fixture(label: &str) -> Fixture {
    Fixture::new(
        label,
        &[
            SourceCase {
                path: "alpha.spx",
                source: r#"module hostile.alpha;
@id("hostile.alpha.helper") fn helper()->i64{1}
@id("hostile.alpha.main") fn main()->i64{helper()}
"#,
                target: "hostile.alpha.helper",
                renamed: "alpha_answer",
            },
            SourceCase {
                path: "beta.spx",
                source: r#"module hostile.beta;
@id("hostile.beta.helper") fn helper()->i64{2}
@id("hostile.beta.main") fn main()->i64{helper()}
"#,
                target: "hostile.beta.helper",
                renamed: "beta_answer",
            },
        ],
    )
}

#[test]
fn canonical_wire_mutations_and_typed_substitutions_fail_closed() {
    let fixture = standard_fixture("wire");
    let capsule = workspace_patch_evidence::generate(&fixture.root, &fixture.patch).unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&capsule).unwrap();

    let manifest_schema = "semaprax.workspace-manifest.v1";
    let top_order = replace_once(
        &capsule,
        &format!(
            "{{\"schema\":\"semaprax.semantic-workspace-patch-evidence.v1\",\"workspace_manifest_schema\":\"{manifest_schema}\","
        ),
        &format!(
            "{{\"workspace_manifest_schema\":\"{manifest_schema}\",\"schema\":\"semaprax.semantic-workspace-patch-evidence.v1\","
        ),
    );
    let over_depth = replace_once(
        &replace_once(&capsule, "\"nonclaims\":[", "\"nonclaims\":[[[[[[[[["),
        "]}\n",
        "]]]]]]]]]}\n",
    );
    let format_mutations = [
        ("trailing-line", format!("{capsule}x\n"), "SPX-G160"),
        ("top-order", top_order, "SPX-G160"),
        (
            "manifest-schema",
            replace_once(&capsule, manifest_schema, "semaprax.workspace-manifest.v2"),
            "SPX-G160",
        ),
        (
            "workspace-patch-schema",
            replace_once(
                &capsule,
                "\"workspace_patch\":{\"schema\":\"semaprax.semantic-workspace-patch.v1\"",
                "\"workspace_patch\":{\"schema\":\"semaprax.semantic-workspace-patch.v2\"",
            ),
            "SPX-G160",
        ),
        (
            "preview-schema",
            replace_once(
                &capsule,
                "\"workspace_preview\":{\"schema\":\"semaprax.semantic-workspace-preview.v1\"",
                "\"workspace_preview\":{\"schema\":\"semaprax.semantic-workspace-preview.v2\"",
            ),
            "SPX-G160",
        ),
        (
            "nested-extra-key",
            replace_once(
                &capsule,
                "\"base_source\":{\"digest\":",
                "\"base_source\":{\"extra\":0,\"digest\":",
            ),
            "SPX-G160",
        ),
        (
            "child-evidence-v2",
            replace_once(
                &capsule,
                "\"patch_evidence\":{\"schema\":\"semaprax.semantic-patch-evidence.v1\"",
                "\"patch_evidence\":{\"schema\":\"semaprax.semantic-patch-evidence.v2\"",
            ),
            "SPX-G160",
        ),
        (
            "target-aggregation",
            replace_once(
                &capsule,
                ",\"limits\":",
                ",\"target_evidence\":{},\"limits\":",
            ),
            "SPX-G160",
        ),
        (
            "assessment-value",
            replace_once(
                &capsule,
                "\"behavior\":\"unchanged_within_admitted_domain\"",
                "\"behavior\":\"verified_safe\"",
            ),
            "SPX-G160",
        ),
        (
            "support-id",
            replace_once(&capsule, "\"id\":\"evidence:0\"", "\"id\":\"evidence:1\""),
            "SPX-G160",
        ),
        (
            "support-kind-correlation",
            replace_once(
                &capsule,
                "\"kind\":\"semantic_impact_v1\",\"schema\":\"semaprax.semantic-impact.v1\"",
                "\"kind\":\"identity_rebase_v1\",\"schema\":\"semaprax.identity-rebase.v1\"",
            ),
            "SPX-G160",
        ),
        (
            "nonclaim",
            replace_once(
                &capsule,
                "\"no_target_evidence_or_evidence_v2_aggregation\"",
                "\"target_evidence_aggregated\"",
            ),
            "SPX-G160",
        ),
        (
            "limit-value",
            replace_once(
                &capsule,
                "\"max_managed_files\":16",
                "\"max_managed_files\":17",
            ),
            "SPX-G160",
        ),
        (
            "budget-key",
            replace_once(&capsule, "\"used_changed_files\":", "\"changed_files\":"),
            "SPX-G160",
        ),
        ("json-depth", over_depth, "SPX-G161"),
    ];
    for (label, mutation, expected) in format_mutations {
        assert_eq!(fixture.verify_code(&mutation), expected, "{label}");
    }

    let base_workspace_revision = parsed["base_workspace_revision"].as_str().unwrap();
    let candidate_workspace_revision = parsed["candidate_workspace_revision"].as_str().unwrap();
    let workspace_patch_digest = parsed["workspace_patch"]["digest"].as_str().unwrap();
    let preview_digest = parsed["workspace_preview"]["digest"].as_str().unwrap();
    let first = &parsed["files"][0];
    let base_source_digest = first["base_source"]["digest"].as_str().unwrap();
    let patch_digest = first["patch"]["digest"].as_str().unwrap();
    let review_digest = first["review"]["digest"].as_str().unwrap();
    let supporting_digest = first["supporting_evidence"]["digest"].as_str().unwrap();
    let child_digest = first["patch_evidence"]["digest"].as_str().unwrap();
    let replay_mutations = [
        (
            "base-workspace-revision",
            replace_once(
                &capsule,
                base_workspace_revision,
                &alternate_digest(base_workspace_revision),
            ),
        ),
        (
            "candidate-workspace-revision",
            replace_once(
                &capsule,
                candidate_workspace_revision,
                &alternate_digest(candidate_workspace_revision),
            ),
        ),
        (
            "workspace-patch-digest",
            replace_once(
                &capsule,
                workspace_patch_digest,
                &alternate_digest(workspace_patch_digest),
            ),
        ),
        (
            "preview-digest",
            replace_once(
                &capsule,
                preview_digest,
                &alternate_digest(preview_digest),
            ),
        ),
        (
            "base-source-digest",
            replace_once(
                &capsule,
                &format!("\"base_source\":{{\"digest\":\"{base_source_digest}\"}}"),
                &format!(
                    "\"base_source\":{{\"digest\":\"{}\"}}",
                    alternate_digest(base_source_digest)
                ),
            ),
        ),
        (
            "patch-digest",
            replace_once(
                &capsule,
                &format!("\"patch\":{{\"schema\":\"semaprax.semantic-patch.v1\",\"digest\":\"{patch_digest}\"}}"),
                &format!(
                    "\"patch\":{{\"schema\":\"semaprax.semantic-patch.v1\",\"digest\":\"{}\"}}",
                    alternate_digest(patch_digest)
                ),
            ),
        ),
        (
            "review-digest",
            replace_once(
                &capsule,
                &format!("\"review\":{{\"schema\":\"semaprax.semantic-review.v1\",\"digest\":\"{review_digest}\"}}"),
                &format!(
                    "\"review\":{{\"schema\":\"semaprax.semantic-review.v1\",\"digest\":\"{}\"}}",
                    alternate_digest(review_digest)
                ),
            ),
        ),
        (
            "supporting-digest",
            replace_once(
                &capsule,
                supporting_digest,
                &alternate_digest(supporting_digest),
            ),
        ),
        (
            "child-evidence-digest",
            replace_once(&capsule, child_digest, &alternate_digest(child_digest)),
        ),
        (
            "valid-assessment-substitution",
            reaccount_capsule(
                &capsule,
                &replace_once(
                    &capsule,
                    "\"behavior\":\"unchanged_within_admitted_domain\"",
                    "\"behavior\":\"mixed\"",
                ),
            ),
        ),
        (
            "valid-path-substitution",
            reaccount_capsule(
                &capsule,
                &replace_once(
                    &capsule,
                    "\"path\":\"alpha.spx\"",
                    "\"path\":\"aardvark.spx\"",
                ),
            ),
        ),
    ];
    for (label, mutation) in replay_mutations {
        assert_eq!(fixture.verify_code(&mutation), "SPX-G162", "{label}");
    }
}

#[test]
fn graph_v10_v14_bindings_are_exact_and_wrong_schemas_reject() {
    let cases = [
        (
            "v10",
            r#"module evidence.schema_v10;
@id("evidence.schema.target_v10") fn target()->i64{1}
@id("evidence.schema.main_v10") fn main()->i64{target()}
"#,
            "evidence.schema.target_v10",
            "renamed_v10",
            "semaprax.graph.v10",
        ),
        (
            "v11",
            r#"module evidence.schema_v11;
@id("evidence.schema.target_v11") fn target(input:Option<i64>)->Option<bool>{let checked=input?;Option<bool>::Some { value: checked>0 }}
@id("evidence.schema.main_v11") fn main()->i64{0}
"#,
            "evidence.schema.target_v11",
            "renamed_v11",
            "semaprax.graph.v11",
        ),
        (
            "v12",
            include_str!("../platform-tests/component-runtime/v7.spx"),
            "component.transform-i64-bool",
            "renamed_v12",
            "semaprax.graph.v12",
        ),
        (
            "v13",
            include_str!("../platform-tests/component-runtime/v8.spx"),
            "component.pattern.preserve-phantom-i64",
            "renamed_v13",
            "semaprax.graph.v13",
        ),
        (
            "v14",
            r#"module evidence.schema_v14;
@id("evidence.schema.target_v14") fn target<T>()->bool{true}
@id("evidence.schema.main_v14") fn main()->i64{if target<i64>(){1}else{0}}
"#,
            "evidence.schema.target_v14",
            "renamed_v14",
            "semaprax.graph.v14",
        ),
    ];

    for (index, (label, source, target, renamed, expected_schema)) in cases.into_iter().enumerate()
    {
        let auxiliary_source = format!(
            "module evidence.aux_{index}; @id(\"evidence.aux_{index}.target\") fn target()->i64{{{index}}} @id(\"evidence.aux_{index}.main\") fn main()->i64{{target()}}"
        );
        let auxiliary_target = format!("evidence.aux_{index}.target");
        let auxiliary_renamed = format!("renamed_aux_{index}");
        let fixture = Fixture::new(
            &format!("graph-{label}"),
            &[
                SourceCase {
                    path: "helper.spx",
                    source: &auxiliary_source,
                    target: &auxiliary_target,
                    renamed: &auxiliary_renamed,
                },
                SourceCase {
                    path: "schema.spx",
                    source,
                    target,
                    renamed,
                },
            ],
        );
        let capsule = workspace_patch_evidence::generate(&fixture.root, &fixture.patch).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&capsule).unwrap();
        let schema_file = parsed["files"]
            .as_array()
            .unwrap()
            .iter()
            .find(|file| file["path"] == "schema.spx")
            .unwrap();
        assert_eq!(schema_file["base_source_graph_schema"], expected_schema);
        assert_eq!(
            schema_file["candidate_source_graph_schema"],
            expected_schema
        );

        let alternate_schema = if expected_schema == "semaprax.graph.v10" {
            "semaprax.graph.v11"
        } else {
            "semaprax.graph.v10"
        };
        let valid_substitution = replace_once(&capsule, expected_schema, alternate_schema);
        assert_eq!(
            fixture.verify_code(&valid_substitution),
            "SPX-G162",
            "valid wrong Graph schema {label}"
        );
        let unsupported = replace_once(&capsule, expected_schema, "semaprax.graph.v15");
        assert_eq!(
            fixture.verify_code(&unsupported),
            "SPX-G160",
            "unsupported Graph schema {label}"
        );
    }
}
