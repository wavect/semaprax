use super::*;
use semaprax::assurance_manifest::law_set::installed_workflow::{self, Request, SourceGoal, View};
use std::process::Command;

fn fixture(mode: &str) -> (Project, LawSet, StrictLawPolicy, InstalledProofTool) {
    let project = native_project(&format!("law12-{mode}"), "n + 0 == n");
    let law_source = "module fresh.laws;\n@id(\"fresh.law.seventeen\")\nlaw contract \"fresh.seventeen\" ensures (a: i64, result: i64)\n result == a + 17\n evidence smt_proved;\n";
    let law = semaprax::native_law_source::canonical(
        &semaprax::native_law_source::parse(law_source, "src/contracts.spx").unwrap(),
    );
    std::fs::write(project.root.join("src/contracts.spx"), law).unwrap();
    let source_text = if mode == "unsupported" {
        "module app.fresh; @id(\"fresh.helper\") fn helper(a: i64) -> i64 { a + 16 } @id(\"fresh.seventeen\") fn seventeen(a: i64) -> i64 requires a >= 0 requires a <= 100 ensures result == a + 17 { helper(a) } @id(\"fresh.main\") fn main() -> i64 { seventeen(0) }"
    } else {
        "module app.fresh; @id(\"fresh.seventeen\") fn seventeen(a: i64) -> i64 requires a >= 0 requires a <= 100 ensures result == a + 17 { a + 16 } @id(\"fresh.main\") fn main() -> i64 { seventeen(0) }"
    };
    let source = semaprax::format::canonical(&semaprax::parse(source_text, "src/app.spx").unwrap());
    std::fs::write(project.root.join("src/app.spx"), source).unwrap();
    let revision = project.revision();
    let laws = LawSet::derive(
        &revision,
        "law12-status-v1",
        revision.law_modules().to_vec(),
    )
    .unwrap();
    let executable = project.root.join(format!("fixture-{mode}-z3"));
    let source = project.root.join("status-fixture.c");
    let pause = if mode == "timeout" { "sleep(1);" } else { "" };
    let program = format!(
        "#include <stdio.h>\n#include <string.h>\n#include <unistd.h>\nint main(int argc, char **argv) {{ if (argc > 1 && strcmp(argv[1], \"--version\") == 0) {{ puts(\"fixture-z3\"); return 0; }} {pause} puts(\"unknown\"); return 0; }}\n"
    );
    std::fs::write(&source, program).unwrap();
    let status = Command::new(std::env::var("CLANG").expect("explicit C compiler required"))
        .arg(&source)
        .arg("-o")
        .arg(&executable)
        .status()
        .unwrap();
    assert!(status.success());
    let tool = InstalledProofTool::open(
        &executable,
        &project.root,
        ToolKind::Z3,
        "fixture-z3",
        HostProfile::TrustedLocal,
        Limits {
            version_timeout_ms: 1_000,
            proof_timeout_ms: if mode == "timeout" { 10 } else { 1_000 },
            stream_max: 32_752,
        },
        AgentCancellation::new(),
    )
    .unwrap();
    let policy = StrictLawPolicy::new(
        laws.clone(),
        BTreeMap::from([(
            "fresh.law.seventeen".into(),
            RequiredLawEvidence::PinnedSmtSource {
                toolchain: tool.expected_version().into(),
                accepted_translation: semaprax::assurance_manifest::smt_discharge::BOUNDS_V1.into(),
            },
        )]),
    )
    .unwrap();
    (project, laws, policy, tool)
}

#[test]
#[ignore = "requires explicitly provisioned C compiler for held diagnostic process fixtures"]
fn selected_law_unknown_timeout_unsupported_and_stale_preserve_summary_and_detail_counts() {
    for mode in ["unknown", "timeout", "unsupported"] {
        let (project, laws, policy, tool) = fixture(mode);
        let revision = project.revision();
        for view in [
            View::Summary {
                offset: 0,
                limit: 1,
            },
            View::Detail,
        ] {
            let request = Request {
                law_id: "fresh.law.seventeen",
                source_goal: Some(SourceGoal {
                    path: "src/app.spx",
                    declaration: "fresh.seventeen",
                    ensures_index: 0,
                }),
                view,
                max_bytes: 65_536,
                show_witness_values: false,
                expected_candidate_revision: Some(revision.project_revision()),
            };
            let result =
                installed_workflow::check(&revision, &laws, &policy, &tool, &request).unwrap();
            assert!(!result.accepted);
            let result: serde_json::Value = serde_json::from_str(&result.document).unwrap();
            assert_eq!(result["proof_attempt"]["outcome"], mode, "{result}");
            assert_eq!(result["schema"], "semaprax.project-law-workflow-cli.v2");
            assert_eq!(result["validity"]["accepted"], false);
            assert_eq!(result["validity"]["proof_attempt"], mode);
            assert_eq!(result["validity"]["counts"]["required"], 1);
            assert_eq!(result["work"]["cost_status"], "unavailable");
            assert!(result["work"]["provider_cost_micros"].is_null());
            if mode == "unsupported" {
                assert_eq!(result["work"]["reserved_solver_queries"], 0);
            } else {
                assert!(result["work"]["reserved_solver_queries"].as_u64().unwrap() > 0);
            }
            assert_eq!(result["view"]["accepted"], false);
            assert_eq!(result["view"]["counts"]["required"], 1);
            assert_eq!(result["failed_obligation_ids"].as_array().unwrap().len(), 1);
            assert_eq!(result["candidate_revision"], revision.project_revision());
        }
        for view in [
            View::Summary {
                offset: 0,
                limit: 1,
            },
            View::Detail,
        ] {
            let stale = Request {
                law_id: "fresh.law.seventeen",
                source_goal: Some(SourceGoal {
                    path: "src/app.spx",
                    declaration: "fresh.seventeen",
                    ensures_index: 0,
                }),
                view,
                max_bytes: 65_536,
                show_witness_values: false,
                expected_candidate_revision: Some("sha256:stale"),
            };
            let result = installed_workflow::check(&revision, &laws, &policy, &tool, &stale).unwrap();
            let result: serde_json::Value = serde_json::from_str(&result.document).unwrap();
            assert_eq!(result["proof_attempt"]["outcome"], "stale");
            assert_eq!(result["validity"]["proof_attempt"], "stale");
            assert_eq!(result["work"]["reserved_process_invocations"], 0);
            assert_eq!(result["work"]["reserved_solver_queries"], 0);
            assert_eq!(result["view"]["counts"]["required"], 1);
            assert_eq!(result["view"]["accepted"], false);
        }
    }
}
