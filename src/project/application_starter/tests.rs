use super::*;
use crate::project::json_codec::plan_tests::{counts, fixture, measure_draft_overlap};
const PLAN: &[u8] = br#"{"schema":"semaprax.json-codec-plan.v1","entries":[{"source":"src/first.spx","record":"first.row","profile":"flat-scalars.v1"},{"source":"src/second.spx","record":"second.row","profile":"flat-scalars.v1"}]}"#;
fn authored(root: &std::path::Path) -> Vec<(String, Vec<u8>)> {
    [
        "semaprax.toml",
        "src/app.spx",
        "src/first.spx",
        "src/second.spx",
        "src/tests.spx",
    ]
    .map(|path| (path.to_owned(), std::fs::read(root.join(path)).unwrap()))
    .to_vec()
}
#[test]
fn complete_old_starter_matches_every_final_file_receipt_and_distinct_revision() {
    let (root, revision) = fixture(false);
    let original = authored(&root);
    let activated = ApplicationCommandSource {
        path: "src/app.spx".into(),
        source: format!(
            "// explicit command activation\n{}",
            revision
                .sources()
                .iter()
                .find(|s| s.path() == "src/app.spx")
                .unwrap()
                .source()
        ),
    };
    for command in [None, Some(&activated)] {
        let old_command = command.map(|c| before::ApplicationCommandSource {
            path: c.path.clone(),
            source: c.source.clone(),
        });
        let old = before::derive_application_starter(
            &revision,
            PLAN,
            "source-command.v1",
            old_command.as_ref(),
        )
        .unwrap();
        assert_eq!(counts(), [2, 2, 2]);
        let actual =
            derive_application_starter(&revision, PLAN, "source-command.v1", command).unwrap();
        assert_eq!(counts(), [1, 1, 1]);
        assert_eq!(actual.files(), old.files());
        assert_eq!(actual.project_revision(), old.project_revision());
        assert_eq!(actual.workspace_revision(), old.workspace_revision());
        let readme = std::str::from_utf8(&actual.files()[0].1).unwrap();
        let receipt: serde_json::Value = serde_json::from_str(
            readme
                .split("```json\n")
                .nth(1)
                .unwrap()
                .split("\n```")
                .next()
                .unwrap(),
        )
        .unwrap();
        assert_eq!(receipt["project_revision"], actual.project_revision());
        assert_eq!(receipt["workspace_revision"], actual.workspace_revision());
        assert_eq!(
            receipt["codec_bundle"]["authored_project_revision"],
            revision.project_revision()
        );
        assert_eq!(
            receipt["codec_bundle"]["entries"].as_array().unwrap().len(),
            2
        );
        // Full old/current file equality includes every pre-activation bundle
        // field and both revisions, not merely these selected receipt fields.
        assert_eq!(authored(&root), original);
    }
    std::fs::remove_dir_all(root).unwrap();
}
#[test]
fn old_starter_error_order_stale_activation_and_mixed_member_publish_nothing() {
    let (root, revision) = fixture(false);
    let original = authored(&root);
    let overlap = ApplicationCommandSource {
        path: "src/first.spx".into(),
        source: "bad command".into(),
    };
    let invalid_source = ApplicationCommandSource {
        path: "src/app.spx".into(),
        source: "not semaprax".into(),
    };
    let stale = String::from_utf8(PLAN.to_vec()).unwrap().replace(
        "\"entries\":",
        "\"expected_project_revision\":\"sha256:stale\",\"entries\":",
    );
    let invalid_member = String::from_utf8(PLAN.to_vec())
        .unwrap()
        .replace("second.row", "missing.row");
    for (plan, profile, command) in [
        (stale.as_bytes(), "source-command.v1", None),
        (PLAN, "unknown", None),
        (
            invalid_member.as_bytes(),
            "source-command.v1",
            Some(&overlap),
        ),
        (PLAN, "source-command.v1", Some(&overlap)),
        (PLAN, "source-command.v1", Some(&invalid_source)),
    ] {
        let old_command = command.map(|c| before::ApplicationCommandSource {
            path: c.path.clone(),
            source: c.source.clone(),
        });
        let expected =
            before::derive_application_starter(&revision, plan, profile, old_command.as_ref())
                .unwrap_err();
        counts();
        let actual = derive_application_starter(&revision, plan, profile, command).unwrap_err();
        assert_eq!(actual, expected); // code/severity/message/path/span/help + order
        counts();
        assert_eq!(authored(&root), original);
        assert!(!root.join("README.md").exists());
        assert!(!root.join("AGENTS.md").exists());
    }
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn whole_starter_allocation_observation_counts_complete_drop_without_gain_assumption() {
    let (root, revision) = fixture(false);
    // These immutable renderer tables live for the process, independently of
    // either starter. Acquire their five exact-source lanes before observing
    // temporary starter ownership; neither starter is run or retained here.
    let mut token = [0; crate::kernel_zero::rung_two_authority::MAX_TOKEN_BYTES];
    use crate::kernel_zero::rung_two_authority as tokens;
    tokens::char_into(u32::from('a'), "'a'", &mut token);
    tokens::bool_into(false, "false", &mut token);
    tokens::int_into(0, "0", &mut token);
    tokens::binary_operator_into(crate::ast::BinaryOp::Add, "+", &mut token);
    tokens::string_scalar_into(u32::from('a'), "a", &mut token);
    measure_draft_overlap(&revision);
    let (_, old) = crate::test_allocation_observer::measure_with_capacity(131_072, || {
        let artifact =
            before::derive_application_starter(&revision, PLAN, "source-command.v1", None).unwrap();
        assert!(!artifact.files().is_empty());
        drop(artifact);
    });
    assert_eq!(counts(), [2, 2, 2]);
    let (_, current) = crate::test_allocation_observer::measure_with_capacity(131_072, || {
        let artifact =
            derive_application_starter(&revision, PLAN, "source-command.v1", None).unwrap();
        assert!(!artifact.files().is_empty());
        drop(artifact);
    });
    assert_eq!(counts(), [1, 1, 1]);
    eprintln!("complete starter before={old:?} current={current:?}");
    for metrics in [old, current] {
        assert!(!metrics.invalid);
        assert_eq!(metrics.live, 0);
        assert_eq!(metrics.allocated, metrics.freed);
    }
    eprintln!(
        "OPT895 complete starter old={old:?} current={current:?}; no net-gain threshold assumed"
    );
    std::fs::remove_dir_all(root).unwrap();
}
