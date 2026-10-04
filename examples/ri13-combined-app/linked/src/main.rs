mod selected_index;
mod m2 {
    include!("../generated/m2/module.rs");
}
mod m3 {
    include!("../generated/m3.rs");
}
use selected_index::admit_selected_index;
use semaprax::project::ProjectRevision;
use semaprax_native_rust_interop::{
    indexed_binding::SelectedPackage, with_authenticated_indexed_regex_url_project,
    IndexedProjectRegexRegistrySelection, IndexedProjectUrlRegistrySelection,
};
use std::sync::Arc;

const REGEX_INDEX: &[u8] = include_bytes!(
    "../../../../crates/semaprax-rust-api-index/fixtures/regex-1.13.1-index-envelope.json"
);
const URL_INDEX: &[u8] = include_bytes!(
    "../../../../crates/semaprax-rust-api-index/fixtures/url-2.5.8-index-envelope.json"
);

fn unified_revision(manifest: &std::path::Path) -> Arc<ProjectRevision> {
    let source = std::fs::read_to_string(manifest.parent().unwrap().join("src/app.spx"))
        .expect("read unified authenticated Project source");
    let regex_index = admit_selected_index("regex-1.13.1-index-envelope.json", REGEX_INDEX);
    let url_index = admit_selected_index("url-2.5.8-index-envelope.json", URL_INDEX);
    let regex_package = SelectedPackage {
        cargo_alias: "regex_alias",
        name: "regex",
        version: "1.13.1",
        source_sha256: regex_index.package().source_sha256.as_str(),
        target: regex_index.target(),
        feature_digest: regex_index.feature_digest(),
        stable_rustc_version: regex_index.stable_rustc_version(),
    };
    let url_package = SelectedPackage {
        cargo_alias: "url_alias",
        name: "url",
        version: "2.5.8",
        source_sha256: url_index.package().source_sha256.as_str(),
        target: url_index.target(),
        feature_digest: url_index.feature_digest(),
        stable_rustc_version: url_index.stable_rustc_version(),
    };
    let regex =
        ["regex.new", "regex.match"].map(|import_id| IndexedProjectRegexRegistrySelection {
            source_path: "src/app.spx",
            source: &source,
            import_id,
            index_bytes: regex_index.canonical_json().as_bytes(),
            package: regex_package,
        });
    let url = ["url.new", "url.view"].map(|import_id| IndexedProjectUrlRegistrySelection {
        source_path: "src/app.spx",
        source: &source,
        import_id,
        index_bytes: url_index.canonical_json().as_bytes(),
        package: url_package,
    });
    with_authenticated_indexed_regex_url_project(manifest, &regex, &url, |snapshot| {
        snapshot.check()?;
        Ok(snapshot.retain_revision())
    })
    .expect("admitted held indexed unified Project")
}

fn main() {
    assert_eq!(ri06_regex_owner::run(), Ok(41));
    assert_eq!(ri06_url_owner::run(), Ok(41));
    assert!(ri06_regex_owner::projected_borrow_matches_target());
    assert_eq!(ri06_regex_owner::spx_result_owner_adapter_copies(), 0);
    assert_eq!(ri06_regex_owner::spx_result_owner_adapter_copied_bytes(), 0);
    assert_eq!(ri06_regex_owner::spx_result_owner_last_input_length(), 28);
    assert!(ri06_url_owner::projected_borrow_matches_target());
    assert_eq!(ri06_regex_owner::live_string_count(), 0);
    assert_eq!(ri06_url_owner::live_string_count(), 0);
    let mut generated_mirror_to_record_transferred_string_bytes = 0usize;
    let records = [
        r#"{"value":1,"label":"one"}"#,
        r#"{"value":2,"label":"two"}"#,
    ]
    .into_iter()
    .map(|source| {
        let (record, transfer) = m2::deserialize_spxmirrorri13event_with_transfer_metrics(source)?;
        assert!(transfer.string_pointers_preserved);
        assert_eq!(transfer.copied_string_bytes, Some(0));
        generated_mirror_to_record_transferred_string_bytes += transfer.transferred_string_bytes;
        Ok::<_, serde_json::Error>(record)
    })
    .collect::<Result<Vec<_>, _>>()
    .unwrap();
    assert_eq!(records[0].label, "one");
    let generated_mirror_string_clone_copied_bytes = records[0].label.len();
    assert_eq!(
        m2::serialize_spxmirrorri13event(&records[0]).unwrap(),
        r#"{"value":1,"label":"one"}"#
    );
    let domain = m2::SpxCallbackDomain::new(2).unwrap();
    let callback = m2::SpxCallback::new(domain.clone(), 7).unwrap();
    let mapped = records
        .iter()
        .map(|record| record.value)
        .map(callback.as_fn())
        .collect::<Result<Vec<_>, _>>()
        .unwrap();
    assert_eq!(mapped, [8, 9]);
    let mut stateful = m2::SpxStatefulProxy::new(domain.clone(), 10).unwrap();
    let states = records
        .iter()
        .map(|record| record.value)
        .map(stateful.as_fn_mut())
        .collect::<Result<Vec<_>, _>>()
        .unwrap();
    assert_eq!(states, [11, 13]);
    assert_eq!(stateful.state(), Ok(13));
    callback.unregister().unwrap();
    stateful.unregister().unwrap();
    drop(callback);
    drop(stateful);
    assert_eq!(domain.live_environments(), 0);
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let unified_manifest = root.parent().unwrap().join("unified-project/semaprax.toml");
    let revision = unified_revision(&unified_manifest);
    let runtime = tokio::runtime::Builder::new_current_thread()
        .build()
        .unwrap();
    let call = m3::register(Arc::<ProjectRevision>::clone(&revision), |_request| async {
        Ok::<i64, ()>(43)
    })
    .unwrap()
    .call_typed(41, 10_000)
    .unwrap();
    assert_eq!(runtime.block_on(call).unwrap(), 84);
    // This line is consumed by the measurement harness. It is limited to
    // generated APIs that expose an exact count or scalar-only boundary.
    println!(
        "ri13-linked-copy-ledger:{{\"schema\":\"semaprax.ri13.linked-copy-ledger.v1\",\"m1\":{{\"regex_result_owner\":{{\"status\":\"measured\",\"adapter_copy_events\":{},\"adapter_copied_bytes\":{},\"adapter_borrowed_scan_input_bytes\":{},\"borrow_matches_target\":true,\"foreign_target_copied_bytes\":{{\"status\":\"unavailable\",\"reason\":\"regex::Regex::is_match does not expose internal copied-byte counts\"}}}},\"url_owner_view\":{{\"status\":\"measured\",\"adapter_copy_events\":{},\"adapter_copied_bytes\":{},\"borrow_matches_target\":true,\"foreign_target_copied_bytes\":{{\"status\":\"unavailable\",\"reason\":\"url::Url::parse does not expose a copied-byte counter\"}}}}}},\"m2\":{{\"serde_record\":{{\"input_json_bytes\":25,\"output_json_bytes\":25,\"generated_mirror_string_clone_copied_bytes\":{},\"generated_mirror_to_record_transferred_string_bytes\":{},\"generated_mirror_to_record_copied_string_bytes\":0,\"generated_mirror_to_record_pointers_preserved\":true,\"deserialize_owned_string_copied_bytes\":{{\"status\":\"unavailable\",\"reason\":\"serde_json deserialization does not expose a copied-byte counter\"}}}},\"iterator_callback\":{{\"fn_invocations\":1,\"fn_mut_invocations\":1,\"scalar_argument_result_copied_bytes\":0}}}}}}",
        ri06_regex_owner::spx_result_owner_adapter_copies(),
        ri06_regex_owner::spx_result_owner_adapter_copied_bytes(),
        ri06_regex_owner::spx_result_owner_last_input_length(),
        ri06_url_owner::adapter_copy_count(),
        ri06_url_owner::adapter_copied_bytes(),
        generated_mirror_string_clone_copied_bytes,
        generated_mirror_to_record_transferred_string_bytes,
    );
    println!("ri13-linked-project-ok");
}
