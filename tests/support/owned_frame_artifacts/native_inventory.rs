//! Fixed native SDK inventory and canonical manifest observations, not a public verifier.
use super::*;
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;

pub(super) fn read_inventory(directory: &Path, names: &[&str]) -> BTreeMap<String, Vec<u8>> {
    let mut actual = fs::read_dir(directory)
        .unwrap()
        .map(|entry| entry.unwrap().file_name().into_string().unwrap())
        .collect::<Vec<_>>();
    actual.sort();
    let mut expected = names.to_vec();
    expected.sort_unstable();
    assert_eq!(actual, expected, "{}", directory.display());
    names
        .iter()
        .map(|name| {
            let path = directory.join(name);
            let metadata = fs::symlink_metadata(&path).unwrap();
            assert!(metadata.is_file() && !metadata.file_type().is_symlink());
            assert!(metadata.len() <= 16 * 1024 * 1024);
            ((*name).to_owned(), fs::read(path).unwrap())
        })
        .collect()
}

fn hash(bytes: &[u8]) -> String {
    let hex = Sha256::digest(bytes)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>();
    format!("sha256:{hex}")
}

fn quote(value: &str) -> String {
    serde_json::to_string(value).unwrap()
}

fn target() -> (&'static str, &'static str) {
    let triple = match (std::env::consts::ARCH, std::env::consts::OS) {
        ("x86_64", "linux") => "x86_64-unknown-linux-gnu",
        ("aarch64", "linux") => "aarch64-unknown-linux-gnu",
        ("x86_64", "macos") => "x86_64-apple-darwin",
        ("aarch64", "macos") => "aarch64-apple-darwin",
        ("x86_64", "windows") => "x86_64-pc-windows-msvc",
        other => panic!("unsupported physical SDK fixture target: {other:?}"),
    };
    let archive = if cfg!(windows) {
        "semaprax_native_rust_owned_data_sdk.lib"
    } else {
        "libsemaprax_native_rust_owned_data_sdk.a"
    };
    (triple, archive)
}

pub(super) fn verify_native_inventory(
    root: &Path,
    descriptor: &PublicApiDescriptor,
    provider: &[u8],
) -> Vec<u8> {
    let (triple, archive) = target();
    let manifest_name = "semaprax.native-rust-owned-data-sdk.json";
    let files = read_inventory(
        root,
        &[
            "Cargo.toml",
            "build.rs",
            "lib.rs",
            "owned_data_ffi.rs",
            "descriptor.json",
            archive,
            manifest_name,
        ],
    );
    let descriptor_bytes = descriptor.canonical_bytes();
    assert_eq!(files["descriptor.json"], descriptor_bytes);
    let rows = files
        .iter()
        .filter(|(path, _)| path.as_str() != manifest_name)
        .map(|(path, bytes)| {
            format!(
                "{{\"path\":{},\"bytes\":{},\"sha256\":{}}}",
                quote(path),
                bytes.len(),
                quote(&hash(bytes)),
            )
        })
        .collect::<Vec<_>>()
        .join(",");
    // Derive every expected field from the real subject, actual compiler
    // provider, fixed package contract and reopened bytes—not manifest claims.
    // Exact raw equality rejects unknown/duplicate fields and noncanonical JSON.
    let expected = format!(
        "{{\"schema\":\"semaprax.native-rust-owned-data-sdk.v1\",\"crate\":{{\"name\":\"semaprax-generated-native-rust-owned-data-sdk\",\"version\":\"0.1.0\"}},\"target\":{},\"descriptor\":{{\"schema\":\"semaprax.public-owned-data-api.v1\",\"bytes\":{},\"digest\":{}}},\"provider\":{{\"abi\":\"opaque-handle.v1\",\"archive\":{},\"descriptor_digest\":{},\"source_sha256\":{},\"operations\":[\"len\",\"copy\",\"drop\"]}},\"files\":[{rows}],\"limits\":{{\"max_borrowed_input_bytes\":65536,\"max_owned_output_bytes\":65536,\"max_handles\":4096,\"exact_package_files\":7}},\"nonclaims\":[\"no_raw_handle_or_context_public_api\",\"no_allocator_transfer\",\"no_allocator_oom_abort_or_panic_recovery_proof\",\"no_send_sync\"]}}\n",
        quote(triple), descriptor_bytes.len(), quote(&descriptor.digest()),
        quote(archive), quote(&descriptor.digest()), quote(&hash(provider)),
    );
    assert_eq!(files[manifest_name], expected.as_bytes());
    files["descriptor.json"].clone()
}
