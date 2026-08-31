//! Test-only exact manifest oracle; no independent provider-semantic authority.
use semaprax_native_rust_owned_data_package::{provider_sha256, HostTarget};
use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

const MANIFEST: &str = "semaprax.native-rust-owned-data-sdk.json";

pub(crate) fn read(path: &Path) -> BTreeMap<String, Vec<u8>> {
    let archive = HostTarget::current().unwrap().archive_name();
    let mut expected = vec![
        "Cargo.toml",
        "build.rs",
        "descriptor.json",
        "lib.rs",
        "owned_data_ffi.rs",
        MANIFEST,
        archive,
    ];
    expected.sort_unstable();
    let mut names = fs::read_dir(path)
        .unwrap()
        .map(|row| row.unwrap().file_name().into_string().unwrap())
        .collect::<Vec<_>>();
    names.sort();
    assert_eq!(names, expected);
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

pub(crate) fn verify(path: &Path, descriptor: &[u8], digest: &str, provider: &[u8], flat: bool) {
    let target = HostTarget::current().unwrap();
    let archive = target.archive_name();
    let files = read(path);
    assert_eq!(files["descriptor.json"], descriptor);
    assert!(!files[archive].is_empty());
    let rows = files
        .iter()
        .filter(|(name, _)| name.as_str() != MANIFEST)
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
    let (schema, api, abi, extra) = if flat {
        (
            "semaprax.native-rust-flat-owned-record-sdk.v1",
            "semaprax.public-flat-owned-record-api.v1",
            "opaque-handle-plus-scalars.v1",
            "\"lower_does_not_authenticate_provider_semantics\",\"no_public_aggregate_abi\",",
        )
    } else {
        (
            "semaprax.native-rust-owned-data-sdk.v1",
            "semaprax.public-owned-data-api.v1",
            "opaque-handle.v1",
            "",
        )
    };
    let expected = format!("{{\"schema\":{schema},\"crate\":{{\"name\":\"semaprax-generated-native-rust-owned-data-sdk\",\"version\":\"0.1.0\"}},\"target\":{target},\"descriptor\":{{\"schema\":{api},\"bytes\":{length},\"digest\":{digest}}},\"provider\":{{\"abi\":{abi},\"archive\":{archive},\"descriptor_digest\":{digest},\"source_sha256\":{provider},\"operations\":[\"len\",\"copy\",\"drop\"]}},\"files\":[{rows}],\"limits\":{{\"max_borrowed_input_bytes\":65536,\"max_owned_output_bytes\":65536,\"max_handles\":4096,\"exact_package_files\":7}},\"nonclaims\":[{extra}\"no_raw_handle_or_context_public_api\",\"no_allocator_transfer\",\"no_allocator_oom_abort_or_panic_recovery_proof\",\"no_send_sync\"]}}\n", schema=quote(schema), target=quote(target.triple()), api=quote(api), length=descriptor.len(), digest=quote(digest), abi=quote(abi), archive=quote(archive), provider=quote(&provider_sha256(provider)));
    assert_eq!(files[MANIFEST], expected.as_bytes());
}
