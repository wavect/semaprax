//! Exact plan decoder and complete assembly preconditions reuse codec owners.
use super::*;

#[test]
fn starter_shared_plan_decoder_retains_closed_schema_bounds_and_stale_source_refusals() {
    let root = super::plan::paired_fixture("starter-plan-controls");
    let plan = br#"{"schema":"semaprax.json-codec-plan.v1","entries":[{"source":"src/schema.spx","record":"application.patient","profile":"flat-scalars.v1"},{"source":"src/second.spx","record":"application.other","profile":"flat-scalars.v1"}]}"#;
    let before = super::plan::paired_originals(&root);
    project::with_authenticated_project(&root.join("semaprax.toml"), |snapshot| {
        assert_eq!(snapshot.native_test_roots_with_profile(project::ProjectNativeTestProfile::CollectionRecordPureV1).unwrap_err()[0].code, "SPX-B104");
        let revision = snapshot.retain_revision();
        // This fixture intentionally has no command authority. A codec bundle
        // cannot turn an arbitrary profile into a runnable command.
        for profile in ["unknown", "source-command.v1", "language-command-io.collection-record.v1"] {
            assert_eq!(project::derive_application_starter(&revision, plan, profile, None).unwrap_err()[0].code, "SPX-J181");
        }
        let stale = String::from_utf8(plan.to_vec()).unwrap().replace("\"entries\":", "\"expected_project_revision\":\"sha256:stale\",\"entries\":");
        assert!(project::derive_application_starter(&revision, stale.as_bytes(), "owned-data-api.v1", None).unwrap_err()[0].message.contains("stale"));
        for malformed in [
            b"{}".as_slice(),
            b"{\"schema\":\"semaprax.json-codec-plan.v1\",\"schema\":\"semaprax.json-codec-plan.v1\",\"entries\":[]}".as_slice(),
            b"{\"schema\":\"semaprax.json-codec-plan.v1\",\"entries\":[{\"source\":\"src/schema.spx\",\"record\":\"application.patient\",\"profile\":\"flat-scalars.v1\",\"max_string_bytes\":1}]}".as_slice(),
            &vec![b' '; 16385],
        ] { assert!(project::decode_json_codec_plan(malformed).is_err()); }
        Ok(())
    }).unwrap();
    assert_eq!(before, super::plan::paired_originals(&root));
    std::fs::remove_dir_all(root).unwrap();
}
