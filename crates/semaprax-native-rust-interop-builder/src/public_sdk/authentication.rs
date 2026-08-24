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
#[derive(Clone, Copy)]
enum InnerPayloadProfile {
    Source,
    Project,
}

impl InnerPayloadProfile {
    const fn bundle_schema(self) -> &'static str {
        match self {
            Self::Source => "semaprax.native-rust-interop-bundle.v1",
            Self::Project => "semaprax.project-native-rust-interop-bundle.v1",
        }
    }

    const fn descriptor_schema(self) -> &'static str {
        match self {
            Self::Source => DESCRIPTOR_SCHEMA,
            Self::Project => PROJECT_DESCRIPTOR_SCHEMA,
        }
    }

    const fn bundle_domain(self) -> &'static [u8] {
        match self {
            Self::Source => INNER_BUNDLE_DOMAIN,
            Self::Project => PROJECT_INNER_BUNDLE_DOMAIN,
        }
    }

    const fn descriptor_domain(self) -> &'static [u8] {
        match self {
            Self::Source => DESCRIPTOR_DOMAIN,
            Self::Project => PROJECT_DESCRIPTOR_DOMAIN,
        }
    }

    const fn root_fields(self) -> usize {
        match self {
            Self::Source => 6,
            Self::Project => 7,
        }
    }

    const fn is_project(self) -> bool {
        matches!(self, Self::Project)
    }
}

pub(super) fn verify_inner_payload_bindings(
    manifest: &[u8],
    artifacts: &InnerArtifacts<'_>,
    expected_digest: &str,
    expected_descriptor_digest: &str,
) -> Result<(), Diagnostic> {
    verify_inner_payload_bindings_for_profile(
        manifest,
        artifacts,
        expected_digest,
        expected_descriptor_digest,
        InnerPayloadProfile::Source,
    )
}

pub(super) fn verify_project_inner_payload_bindings(
    manifest: &[u8],
    artifacts: &InnerArtifacts<'_>,
    expected_digest: &str,
    expected_descriptor_digest: &str,
) -> Result<(), Diagnostic> {
    verify_inner_payload_bindings_for_profile(
        manifest,
        artifacts,
        expected_digest,
        expected_descriptor_digest,
        InnerPayloadProfile::Project,
    )
}

fn verify_inner_payload_bindings_for_profile(
    manifest: &[u8],
    artifacts: &InnerArtifacts<'_>,
    expected_digest: &str,
    expected_descriptor_digest: &str,
    profile: InnerPayloadProfile,
) -> Result<(), Diagnostic> {
    if manifest.len() > MAX_INNER_MANIFEST_BYTES
        || !manifest.ends_with(b"\n")
        || domain_digest(profile.bundle_domain(), manifest) != expected_digest
    {
        return Err(sdk_error("Native Rust SDK inner payload binding failed"));
    }
    let value: Value = serde_json::from_slice(manifest)
        .map_err(|_| sdk_error("Native Rust SDK inner payload binding failed"))?;
    let root = value
        .as_object()
        .filter(|root| root.len() == profile.root_fields())
        .ok_or_else(|| sdk_error("Native Rust SDK inner payload binding failed"))?;
    if root.get("schema").and_then(Value::as_str) != Some(profile.bundle_schema()) {
        return Err(sdk_error("Native Rust SDK inner payload binding failed"));
    }
    let descriptor_row = root
        .get("descriptor")
        .and_then(Value::as_object)
        .filter(|row| row.len() == 3)
        .ok_or_else(|| sdk_error("Native Rust SDK inner payload binding failed"))?;
    if descriptor_row.get("schema").and_then(Value::as_str) != Some(profile.descriptor_schema())
        || descriptor_row.get("digest").and_then(Value::as_str)
            != Some(domain_digest(profile.descriptor_domain(), artifacts.descriptor).as_str())
        || descriptor_row.get("digest").and_then(Value::as_str) != Some(expected_descriptor_digest)
        || descriptor_row.get("bytes").and_then(Value::as_u64)
            != u64::try_from(artifacts.descriptor.len()).ok()
    {
        return Err(sdk_error("Native Rust SDK inner payload binding failed"));
    }
    if profile.is_project() {
        let subject_digest = root
            .get("project_subject_digest")
            .and_then(Value::as_str)
            .ok_or_else(|| sdk_error("Native Rust SDK inner payload binding failed"))?;
        let descriptor: Value = serde_json::from_slice(artifacts.descriptor)
            .map_err(|_| sdk_error("Native Rust SDK inner payload binding failed"))?;
        if descriptor
            .get("project_subject_digest")
            .and_then(Value::as_str)
            != Some(subject_digest)
        {
            return Err(sdk_error("Native Rust SDK inner payload binding failed"));
        }
    } else if root.contains_key("project_subject_digest") {
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

#[cfg(test)]
mod tests {
    use super::*;

    fn inner_manifest(profile: InnerPayloadProfile, artifacts: &InnerArtifacts<'_>) -> Vec<u8> {
        let mut root = Map::new();
        root.insert(
            "schema".to_owned(),
            Value::String(profile.bundle_schema().to_owned()),
        );
        if profile.is_project() {
            root.insert(
                "project_subject_digest".to_owned(),
                Value::String("sha256:project".to_owned()),
            );
        }
        root.insert(
            "descriptor".to_owned(),
            serde_json::json!({
                "schema": profile.descriptor_schema(),
                "digest": domain_digest(profile.descriptor_domain(), artifacts.descriptor),
                "bytes": artifacts.descriptor.len(),
            }),
        );
        let files = [
            ("descriptor.json", artifacts.descriptor),
            ("module.c", artifacts.generated_c),
            (artifacts.object_name, artifacts.object),
            ("semaprax_native_rust_interop.h", artifacts.generated_header),
            ("semaprax_native_rust_interop.rs", artifacts.safe_rust),
            ("semaprax_native_rust_interop_ffi.rs", artifacts.ffi_rust),
        ]
        .into_iter()
        .map(|(path, bytes)| {
            serde_json::json!({
                "path": path,
                "sha256": raw_digest(bytes),
                "bytes": bytes.len(),
            })
        })
        .collect();
        root.insert("files".to_owned(), Value::Array(files));
        root.insert("toolchain".to_owned(), Value::Null);
        root.insert("limits".to_owned(), Value::Null);
        root.insert("nonclaims".to_owned(), Value::Null);
        let mut bytes = serde_json::to_vec(&Value::Object(root)).unwrap();
        bytes.push(b'\n');
        bytes
    }

    #[test]
    fn inner_payload_profiles_are_closed_and_cannot_cross_authenticate() {
        let project_descriptor = br#"{"project_subject_digest":"sha256:project"}"#.as_slice();
        let source_descriptor = b"source descriptor".as_slice();
        let source = InnerArtifacts {
            descriptor: source_descriptor,
            generated_c: b"c",
            generated_header: b"h",
            safe_rust: b"safe",
            ffi_rust: b"ffi",
            object: b"object",
            object_name: "module.o",
        };
        let project = InnerArtifacts {
            descriptor: project_descriptor,
            generated_c: b"c",
            generated_header: b"h",
            safe_rust: b"safe",
            ffi_rust: b"ffi",
            object: b"object",
            object_name: "module.o",
        };
        let source_manifest = inner_manifest(InnerPayloadProfile::Source, &source);
        let project_manifest = inner_manifest(InnerPayloadProfile::Project, &project);
        let source_bundle_digest = domain_digest(INNER_BUNDLE_DOMAIN, &source_manifest);
        let source_descriptor_digest = domain_digest(DESCRIPTOR_DOMAIN, source.descriptor);
        let project_bundle_digest = domain_digest(PROJECT_INNER_BUNDLE_DOMAIN, &project_manifest);
        let project_descriptor_digest =
            domain_digest(PROJECT_DESCRIPTOR_DOMAIN, project.descriptor);

        verify_inner_payload_bindings(
            &source_manifest,
            &source,
            &source_bundle_digest,
            &source_descriptor_digest,
        )
        .unwrap();
        verify_project_inner_payload_bindings(
            &project_manifest,
            &project,
            &project_bundle_digest,
            &project_descriptor_digest,
        )
        .unwrap();
        assert!(verify_project_inner_payload_bindings(
            &source_manifest,
            &source,
            &source_bundle_digest,
            &source_descriptor_digest,
        )
        .is_err());
        assert!(verify_inner_payload_bindings(
            &project_manifest,
            &project,
            &project_bundle_digest,
            &project_descriptor_digest,
        )
        .is_err());
    }
}
