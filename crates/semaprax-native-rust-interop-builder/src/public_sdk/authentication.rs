//! Read-only authentication for staged inputs and the published package.
//!
//! This module can hold, read, and recheck exact files. It has no creation,
//! mutation, cleanup, process, or publication authority.

use super::*;

pub(super) fn authenticate_inventory<const N: usize>(
    scan: &mut crate::platform::PreparedInventoryExact<N>,
    directory: &crate::platform::HeldDirectory,
    inventory: &crate::platform::PreparedDiscardInventory<N>,
) -> Result<(), Diagnostic> {
    crate::platform::inventory_exact_prepared(scan, directory, inventory)
        .map_err(|_| publication_error())?;
    crate::platform::recheck_directory(directory).map_err(|_| publication_error())?;
    Ok(())
}

pub(super) fn read_inner<const N: usize>(
    inventory: &crate::platform::PreparedDiscardInventory<N>,
    name: &str,
    maximum: usize,
) -> Result<Vec<u8>, Diagnostic> {
    crate::platform::read_exact(
        inventory.file(name).map_err(|_| publication_error())?,
        maximum,
    )
    .map_err(|_| publication_error())
}

// Private B has already canonically replayed its own manifest before returning.
// Phase C composes with that trusted result by authenticating the returned
// digest and binding every exact payload row; it does not redefine B's wire.
pub(super) fn verify_inner_payload_bindings(
    manifest: &[u8],
    artifacts: &InnerArtifacts<'_>,
    expected_digest: &str,
) -> Result<(), Diagnostic> {
    if manifest.len() > MAX_INNER_MANIFEST_BYTES
        || !manifest.ends_with(b"\n")
        || domain_digest(INNER_BUNDLE_DOMAIN, manifest) != expected_digest
    {
        return Err(sdk_error("Native Rust SDK inner payload binding failed"));
    }
    let value: Value = serde_json::from_slice(manifest)
        .map_err(|_| sdk_error("Native Rust SDK inner payload binding failed"))?;
    let root = value
        .as_object()
        .filter(|root| root.len() == 6)
        .ok_or_else(|| sdk_error("Native Rust SDK inner payload binding failed"))?;
    if root.get("schema").and_then(Value::as_str) != Some("semaprax.native-rust-interop-bundle.v1")
    {
        return Err(sdk_error("Native Rust SDK inner payload binding failed"));
    }
    let descriptor_row = root
        .get("descriptor")
        .and_then(Value::as_object)
        .filter(|row| row.len() == 3)
        .ok_or_else(|| sdk_error("Native Rust SDK inner payload binding failed"))?;
    if descriptor_row.get("schema").and_then(Value::as_str) != Some(DESCRIPTOR_SCHEMA)
        || descriptor_row.get("digest").and_then(Value::as_str)
            != Some(domain_digest(DESCRIPTOR_DOMAIN, artifacts.descriptor).as_str())
        || descriptor_row.get("bytes").and_then(Value::as_u64)
            != u64::try_from(artifacts.descriptor.len()).ok()
    {
        return Err(sdk_error("Native Rust SDK inner payload binding failed"));
    }
    let rows = root
        .get("files")
        .and_then(Value::as_array)
        .filter(|rows| rows.len() == 6)
        .ok_or_else(|| sdk_error("Native Rust SDK inner payload binding failed"))?;
    let known = [
        ("descriptor.json", artifacts.descriptor),
        ("module.c", artifacts.generated_c),
        (artifacts.object_name, artifacts.object),
        ("semaprax_native_rust_interop.h", artifacts.generated_header),
        ("semaprax_native_rust_interop.rs", artifacts.safe_rust),
        ("semaprax_native_rust_interop_ffi.rs", artifacts.ffi_rust),
    ];
    for (path, bytes) in known {
        let row = rows
            .iter()
            .find(|row| row.get("path").and_then(Value::as_str) == Some(path))
            .and_then(Value::as_object)
            .filter(|row| row.len() == 3)
            .ok_or_else(|| sdk_error("Native Rust SDK inner payload binding failed"))?;
        if row.get("bytes").and_then(Value::as_u64) != u64::try_from(bytes.len()).ok()
            || row.get("sha256").and_then(Value::as_str) != Some(raw_digest(bytes).as_str())
        {
            return Err(sdk_error("Native Rust SDK inner payload binding failed"));
        }
    }
    Ok(())
}

pub(super) fn verify_published_package(
    output: &Path,
    package: &PublishedPackage<'_>,
) -> Result<Vec<u8>, Diagnostic> {
    let root = crate::platform::hold_directory(output).map_err(|_| publication_error())?;
    let src =
        crate::platform::hold_directory(&output.join("src")).map_err(|_| publication_error())?;
    let native =
        crate::platform::hold_directory(&output.join("native")).map_err(|_| publication_error())?;
    let manifest =
        crate::platform::hold_regular_file(&root, OsStr::new("semaprax.native-rust-sdk.json"))
            .map_err(|_| publication_error())?;
    let bytes = crate::platform::read_exact(&manifest, MAX_SDK_MANIFEST_BYTES)
        .map_err(|_| publication_error())?;
    if bytes != package.manifest.as_bytes() {
        return Err(publication_error());
    }
    let cargo = hold_matching(
        &root,
        "Cargo.toml",
        package.sources.cargo_toml.as_bytes(),
        MAX_SDK_MANIFEST_BYTES,
    )?;
    let build = hold_matching(
        &root,
        "build.rs",
        package.sources.build_rs.as_bytes(),
        MAX_SDK_MANIFEST_BYTES,
    )?;
    let lib = hold_matching(
        &src,
        "lib.rs",
        package.sources.lib_rs.as_bytes(),
        MAX_GENERATED_RUST_BYTES,
    )?;
    let safe = hold_matching(
        &src,
        "semaprax_native_rust_interop.rs",
        package.safe_inner,
        MAX_GENERATED_RUST_BYTES,
    )?;
    let ffi = hold_matching(
        &src,
        "semaprax_native_rust_interop_ffi.rs",
        package.ffi_inner,
        MAX_GENERATED_RUST_BYTES,
    )?;
    let descriptor_file = hold_matching(
        &native,
        "descriptor.json",
        package.descriptor,
        MAX_DESCRIPTOR_BYTES,
    )?;
    let archive_file = hold_matching(
        &native,
        package.archive_name,
        package.archive,
        MAX_ARCHIVE_BYTES,
    )?;
    let inner_manifest_file = hold_matching(
        &native,
        "semaprax.native-rust-interop.json",
        package.inner_manifest,
        MAX_INNER_MANIFEST_BYTES,
    )?;

    let mut src_scan = crate::platform::prepare_inventory_entries_exact(
        [
            OsStr::new("lib.rs"),
            OsStr::new("semaprax_native_rust_interop.rs"),
            OsStr::new("semaprax_native_rust_interop_ffi.rs"),
        ],
        3,
    )
    .map_err(|_| publication_error())?;
    crate::platform::inventory_entries_exact_prepared(&mut src_scan, &src, [&lib, &safe, &ffi], [])
        .map_err(|_| publication_error())?;
    let mut native_scan = crate::platform::prepare_inventory_entries_exact(
        [
            OsStr::new("descriptor.json"),
            OsStr::new(package.archive_name),
            OsStr::new("semaprax.native-rust-interop.json"),
        ],
        3,
    )
    .map_err(|_| publication_error())?;
    crate::platform::inventory_entries_exact_prepared(
        &mut native_scan,
        &native,
        [&descriptor_file, &archive_file, &inner_manifest_file],
        [],
    )
    .map_err(|_| publication_error())?;
    let mut root_scan = crate::platform::prepare_inventory_entries_exact(
        [
            OsStr::new("Cargo.toml"),
            OsStr::new("build.rs"),
            OsStr::new("semaprax.native-rust-sdk.json"),
            OsStr::new("src"),
            OsStr::new("native"),
        ],
        3,
    )
    .map_err(|_| publication_error())?;
    crate::platform::inventory_entries_exact_prepared(
        &mut root_scan,
        &root,
        [&cargo, &build, &manifest],
        [&src, &native],
    )
    .map_err(|_| publication_error())?;
    crate::platform::recheck_directory(&root).map_err(|_| publication_error())?;
    crate::platform::recheck_directory(&src).map_err(|_| publication_error())?;
    crate::platform::recheck_directory(&native).map_err(|_| publication_error())?;
    Ok(bytes)
}

pub(super) fn hold_matching(
    directory: &crate::platform::HeldDirectory,
    name: &str,
    expected: &[u8],
    maximum: usize,
) -> Result<crate::platform::HeldRegularFile, Diagnostic> {
    let held = crate::platform::hold_regular_file(directory, OsStr::new(name))
        .map_err(|_| publication_error())?;
    let actual = crate::platform::read_exact(&held, maximum).map_err(|_| publication_error())?;
    if actual != expected || raw_digest(&actual) != raw_digest(expected) {
        return Err(publication_error());
    }
    Ok(held)
}
