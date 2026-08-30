//! Test-specific exact manifest oracle, not a public archive/semantic verifier.
use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

use semaprax::project::FlatOwnedRecordApiDescriptor;
use semaprax_native_rust_owned_data_package::{provider_sha256, HostTarget};

pub(super) fn names(path: &Path) -> Vec<String> {
    let mut names = fs::read_dir(path)
        .unwrap()
        .map(|row| row.unwrap().file_name().into_string().unwrap())
        .collect::<Vec<_>>();
    names.sort();
    names
}

pub(super) fn read(path: &Path) -> BTreeMap<String, Vec<u8>> {
    let archive = HostTarget::current().unwrap().archive_name();
    let mut expected = vec![
        "Cargo.toml",
        "build.rs",
        "descriptor.json",
        "lib.rs",
        "owned_data_ffi.rs",
        "semaprax.native-rust-owned-data-sdk.json",
        archive,
    ];
    expected.sort_unstable();
    assert_eq!(names(path), expected);
    expected
        .into_iter()
        .map(|name| {
            let file = path.join(name);
            let metadata = fs::symlink_metadata(&file).unwrap();
            assert!(metadata.is_file() && !metadata.file_type().is_symlink());
            #[cfg(windows)]
            {
                use std::os::windows::fs::MetadataExt as _;
                assert_eq!(metadata.file_attributes() & 0x400, 0);
            }
            assert!(metadata.len() <= 16 * 1024 * 1024);
            (name.to_owned(), fs::read(file).unwrap())
        })
        .collect()
}

fn quote(value: &str) -> String {
    serde_json::to_string(value).unwrap()
}

pub(super) fn verify(path: &Path, descriptor: &FlatOwnedRecordApiDescriptor, provider: &[u8]) {
    let target = HostTarget::current().unwrap();
    let archive = target.archive_name();
    let files = read(path);
    let descriptor_bytes = descriptor.canonical_bytes();
    assert_eq!(files["descriptor.json"], descriptor_bytes);
    assert!(!files[archive].is_empty());
    let manifest_name = "semaprax.native-rust-owned-data-sdk.json";
    let rows = files
        .iter()
        .filter(|(name, _)| name.as_str() != manifest_name)
        .map(|(name, bytes)| {
            format!(
                "{{\"path\":{},\"bytes\":{},\"sha256\":{}}}",
                quote(name),
                bytes.len(),
                quote(&provider_sha256(bytes))
            )
        })
        .collect::<Vec<_>>()
        .join(",");
    // All subject facts come from checked Project HIR; file facts are reopened.
    // Exact equality also rejects duplicate keys and alternate JSON spelling.
    let expected = format!("{{\"schema\":\"semaprax.native-rust-flat-owned-record-sdk.v1\",\"crate\":{{\"name\":\"semaprax-generated-native-rust-owned-data-sdk\",\"version\":\"0.1.0\"}},\"target\":{},\"descriptor\":{{\"schema\":\"semaprax.public-flat-owned-record-api.v1\",\"bytes\":{},\"digest\":{}}},\"provider\":{{\"abi\":\"opaque-handle-plus-scalars.v1\",\"archive\":{},\"descriptor_digest\":{},\"source_sha256\":{},\"operations\":[\"len\",\"copy\",\"drop\"]}},\"files\":[{rows}],\"limits\":{{\"max_borrowed_input_bytes\":65536,\"max_owned_output_bytes\":65536,\"max_handles\":4096,\"exact_package_files\":7}},\"nonclaims\":[\"lower_does_not_authenticate_provider_semantics\",\"no_public_aggregate_abi\",\"no_raw_handle_or_context_public_api\",\"no_allocator_transfer\",\"no_allocator_oom_abort_or_panic_recovery_proof\",\"no_send_sync\"]}}\n",
        quote(target.triple()), descriptor_bytes.len(), quote(&descriptor.digest()),
        quote(archive), quote(&descriptor.digest()), quote(&provider_sha256(provider)));
    assert_eq!(files[manifest_name], expected.as_bytes());
}

pub(super) fn identities(descriptor: &FlatOwnedRecordApiDescriptor, renamed: bool) {
    assert_eq!(descriptor.exports().len(), 2);
    let left = &descriptor.exports()[0];
    let right = &descriptor.exports()[1];
    assert_eq!(left.stable_id().as_str(), "left.payload");
    assert_eq!(right.stable_id().as_str(), "right.payload");
    assert_eq!(
        left.record_id().as_str(),
        "left.Payload\u{8}\u{c}\u{7f}\u{85}"
    );
    assert_eq!(right.record_id().as_str(), "right.Payload");
    assert_eq!(
        left.record_host_name(),
        "SpxRecordId6c6566742e5061796c6f6164080c7fc285"
    );
    assert_eq!(
        right.record_host_name(),
        "SpxRecordId72696768742e5061796c6f6164"
    );
    assert_eq!(left.record_source_name(), "Payload");
    assert_eq!(
        right.record_source_name(),
        if renamed { "RenamedPayload" } else { "Payload" }
    );
    assert_eq!(left.fields()[0].stable_id().as_str(), "");
    assert_eq!(left.fields()[0].host_name(), "spx_field_id_");
    assert_eq!(descriptor.carrier_plans()[0].owned_field_ordinal, 0);
    assert_eq!(descriptor.carrier_plans()[1].owned_field_ordinal, 3);
}

pub(super) fn rename_preserves_ids(
    before: &FlatOwnedRecordApiDescriptor,
    after: &FlatOwnedRecordApiDescriptor,
) {
    assert_ne!(before.digest(), after.digest());
    for (before, after) in before.exports().iter().zip(after.exports()) {
        assert_eq!(before.stable_id(), after.stable_id());
        assert_eq!(before.rust_method_name(), after.rust_method_name());
        assert_eq!(before.parameters(), after.parameters());
        assert_eq!(before.record_id(), after.record_id());
        assert_eq!(before.record_host_name(), after.record_host_name());
        assert_eq!(before.fields(), after.fields());
    }
    let mut before: serde_json::Value = serde_json::from_slice(&before.canonical_bytes()).unwrap();
    let after: serde_json::Value = serde_json::from_slice(&after.canonical_bytes()).unwrap();
    for binding in [
        "project_revision",
        "workspace_revision",
        "project_graph_digest",
    ] {
        assert_ne!(before[binding], after[binding], "unchanged {binding}");
        before[binding] = after[binding].clone();
    }
    assert_eq!(before["exports"][1]["stable_id"], "right.payload");
    assert_eq!(
        before["exports"][1]["result"]["record_source_name"],
        "Payload"
    );
    assert_eq!(
        after["exports"][1]["result"]["record_source_name"],
        "RenamedPayload"
    );
    before["exports"][1]["result"]["record_source_name"] =
        serde_json::Value::String("RenamedPayload".to_owned());
    // Both inputs are actual canonical compiler descriptors. Only these four
    // expected presentation/subject facts may differ; signatures, parameters,
    // limits and all other fields must remain exactly equal.
    assert_eq!(before, after);
}
