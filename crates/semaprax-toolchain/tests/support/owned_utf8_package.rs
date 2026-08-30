//! Test-specific canonical manifest oracle, not an independent semantic verifier.
use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

use semaprax::project::PublicApiDescriptor;
use semaprax_native_rust_owned_data_package::{provider_sha256, HostTarget};

const MANIFEST: &str = "semaprax.native-rust-owned-utf8-sdk.json";

pub(crate) fn names(path: &Path) -> Vec<String> {
    let mut names = fs::read_dir(path)
        .unwrap()
        .map(|row| row.unwrap().file_name().into_string().unwrap())
        .collect::<Vec<_>>();
    names.sort();
    names
}

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

pub(crate) fn verify(path: &Path, descriptor: &PublicApiDescriptor, provider: &[u8]) {
    let target = HostTarget::current().unwrap();
    let archive = target.archive_name();
    let files = read(path);
    let descriptor_bytes = descriptor.canonical_bytes();
    assert_eq!(files["descriptor.json"], descriptor_bytes);
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
    // Reopened file hashes bind the exact generated/archive bytes. The provider
    // source hash comes from regeneration over retained checked Project HIR;
    // neither that manifest nor this comparison proves execution on its own.
    let expected = format!("{{\"schema\":\"semaprax.native-rust-owned-utf8-sdk.v1\",\"crate\":{{\"name\":\"semaprax-generated-native-rust-owned-data-sdk\",\"version\":\"0.1.0\"}},\"target\":{},\"descriptor\":{{\"schema\":\"semaprax.public-owned-utf8-api.v1\",\"bytes\":{},\"digest\":{}}},\"provider\":{{\"abi\":\"opaque-handle.v1\",\"archive\":{},\"descriptor_digest\":{},\"source_sha256\":{},\"operations\":[\"len\",\"copy\",\"drop\"]}},\"files\":[{rows}],\"limits\":{{\"max_borrowed_input_bytes\":65536,\"max_owned_output_bytes\":65536,\"max_handles\":4096,\"exact_package_files\":7}},\"nonclaims\":[\"no_raw_handle_or_context_public_api\",\"no_allocator_transfer\",\"no_allocator_oom_abort_or_panic_recovery_proof\",\"no_send_sync\"]}}\n",
        quote(target.triple()), descriptor_bytes.len(), quote(&descriptor.digest()),
        quote(archive), quote(&descriptor.digest()), quote(&provider_sha256(provider)));
    assert_eq!(files[MANIFEST], expected.as_bytes());
}
