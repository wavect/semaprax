//! Held-handle staging, authentication, settlement, and publication authority.
//!
//! Keep the state machine here: its private stage types prevent callers from
//! bypassing cleanup or publishing an unauthenticated package.

use super::authentication::{
    authenticate_inventory, read_inner, verify_inner_payload_bindings, verify_published_package,
};
use super::descriptor::{canonical_spec, parse_descriptor};
use super::package::{render_package_sources, render_sdk_manifest, verify_sdk_manifest};
use super::*;

fn simple_output_name(output: &Path) -> Result<&OsStr, Diagnostic> {
    use std::path::Component;
    let parent = output.parent().ok_or_else(publication_error)?;
    let name = output.file_name().ok_or_else(publication_error)?;
    let mut components = Path::new(name).components();
    if !matches!(components.next(), Some(Component::Normal(_)))
        || components.next().is_some()
        || output.strip_prefix(parent).ok() != Some(Path::new(name))
    {
        return Err(publication_error());
    }
    Ok(name)
}

fn planned_child(
    parent_path: &Path,
    parent: &crate::platform::HeldDirectory,
    purpose: &str,
) -> Result<(String, PathBuf, crate::platform::PreparedStageName), Diagnostic> {
    for _ in 0..32 {
        let nonce = STAGE_NONCE.fetch_add(1, Ordering::Relaxed);
        let name = format!(
            ".semaprax-native-rust-sdk-{}-{nonce}-{purpose}",
            std::process::id()
        );
        let path = parent_path.join(&name);
        let probe = crate::platform::prepare_child_name(OsStr::new(&name))
            .map_err(|_| publication_error())?;
        if crate::platform::child_absent_prepared(parent, &probe)
            .map_err(|_| publication_error())?
        {
            let stage = crate::platform::prepare_stage_name(OsStr::new(&name))
                .map_err(|_| publication_error())?;
            return Ok((name, path, stage));
        }
    }
    Err(publication_error())
}

struct InnerBundle {
    directory: crate::platform::HeldDirectory,
    name: crate::platform::PreparedStageName,
    inventory: crate::platform::PreparedDiscardInventory<7>,
}

fn authenticate_inner_bundle(
    parent: &crate::platform::HeldDirectory,
    prepared_name: crate::platform::PreparedStageName,
    path: &Path,
    object_name: &'static str,
    inventory: crate::platform::PreparedDiscardInventory<7>,
    mut scan: crate::platform::PreparedInventoryExact<7>,
) -> Result<InnerBundle, Diagnostic> {
    crate::platform::recheck_directory(parent).map_err(|_| publication_error())?;
    let directory = crate::platform::hold_directory(path).map_err(|_| publication_error())?;
    let mut inner = InnerBundle {
        directory,
        name: prepared_name,
        inventory,
    };
    let authentication = (|| -> Result<(), Diagnostic> {
        if !crate::platform::same_directory_path(&inner.directory, path)
            .map_err(|_| publication_error())?
        {
            return Err(publication_error());
        }
        for name in [
            "descriptor.json",
            "module.c",
            object_name,
            "semaprax_native_rust_interop.h",
            "semaprax_native_rust_interop.rs",
            "semaprax_native_rust_interop_ffi.rs",
            "semaprax.native-rust-interop.json",
        ] {
            let file = crate::platform::hold_regular_file(&inner.directory, OsStr::new(name))
                .map_err(|_| publication_error())?;
            inner
                .inventory
                .attach(name, file)
                .map_err(|_| publication_error())?;
        }
        authenticate_inventory(&mut scan, &inner.directory, &inner.inventory)
    })();
    if let Err(error) = authentication {
        let _ = discard_inner_bundle(parent, &inner);
        return Err(error);
    }
    Ok(inner)
}

fn discard_inner_bundle(
    parent: &crate::platform::HeldDirectory,
    inner: &InnerBundle,
) -> Result<(), Diagnostic> {
    crate::platform::discard_owned_stage_prepared(
        parent,
        &inner.directory,
        &inner.name,
        &inner.inventory,
    )
    .map_err(|_| publication_error())
}

fn fail_after_inner(
    parent: &crate::platform::HeldDirectory,
    inner: &InnerBundle,
    primary: Diagnostic,
) -> PublicBuildError {
    if discard_inner_bundle(parent, inner).is_err() {
        PublicBuildError::Many(vec![primary, publication_error()])
    } else {
        PublicBuildError::One(primary)
    }
}

struct ArchiveStage {
    directory: crate::platform::HeldDirectory,
    name: crate::platform::PreparedStageName,
    inventory: crate::platform::PreparedDiscardInventory<2>,
}

fn create_archive_stage(
    parent: &crate::platform::HeldDirectory,
    prepared_name: crate::platform::PreparedStageName,
    path: &Path,
    object_name: &'static str,
    object: &[u8],
    inventory: crate::platform::PreparedDiscardInventory<2>,
) -> Result<ArchiveStage, StageCreationError> {
    let directory = crate::platform::create_directory_new_prepared(parent, &prepared_name, 0o700)
        .map_err(|_| StageCreationError::certain(publication_error()))?;
    let mut stage = ArchiveStage {
        directory,
        name: prepared_name,
        inventory,
    };
    let result = (|| -> Result<(), Diagnostic> {
        #[cfg(test)]
        if test_hook(TestBuildPoint::ArchiveCreationCleanupUncertainty) {
            return Err(injected_error());
        }
        if !crate::platform::same_directory_path(&stage.directory, path)
            .map_err(|_| publication_error())?
        {
            return Err(publication_error());
        }
        crate::platform::write_file_new_prepared(
            &stage.directory,
            &mut stage.inventory,
            object_name,
            object,
            0o600,
        )
        .map_err(|_| publication_error())
    })();
    if let Err(error) = result {
        let cleanup = discard_archive_stage(parent, &stage);
        return Err(if cleanup.is_err() {
            StageCreationError::uncertain(error)
        } else {
            StageCreationError::certain(error)
        });
    }
    Ok(stage)
}

fn discard_archive_stage(
    parent: &crate::platform::HeldDirectory,
    stage: &ArchiveStage,
) -> Result<(), Diagnostic> {
    crate::platform::discard_owned_stage_prepared(
        parent,
        &stage.directory,
        &stage.name,
        &stage.inventory,
    )
    .map_err(|_| publication_error())
}

fn fail_after_archive(
    parent: &crate::platform::HeldDirectory,
    archive: &ArchiveStage,
    inner: &InnerBundle,
    primary: Diagnostic,
) -> PublicBuildError {
    if discard_archive_stage(parent, archive).is_err() {
        return PublicBuildError::Many(vec![primary, publication_error()]);
    }
    if discard_inner_bundle(parent, inner).is_err() {
        return PublicBuildError::Many(vec![primary, publication_error()]);
    }
    PublicBuildError::One(primary)
}

struct OuterStage {
    directory: crate::platform::HeldDirectory,
    src: crate::platform::HeldDirectory,
    native: crate::platform::HeldDirectory,
    name: crate::platform::PreparedStageName,
    src_name: crate::platform::PreparedStageName,
    native_name: crate::platform::PreparedStageName,
    root_files: crate::platform::PreparedDiscardInventory<3>,
    src_files: crate::platform::PreparedDiscardInventory<3>,
    native_files: crate::platform::PreparedDiscardInventory<3>,
}

struct OuterStagePlan {
    name: crate::platform::PreparedStageName,
    src_name: crate::platform::PreparedStageName,
    native_name: crate::platform::PreparedStageName,
    root_files: crate::platform::PreparedDiscardInventory<3>,
    src_files: crate::platform::PreparedDiscardInventory<3>,
    native_files: crate::platform::PreparedDiscardInventory<3>,
}

impl OuterStage {
    fn recheck_all(
        &self,
        root_scan: &mut crate::platform::PreparedInventoryEntriesExact<5>,
        src_scan: &mut crate::platform::PreparedInventoryExact<3>,
        native_scan: &mut crate::platform::PreparedInventoryExact<3>,
    ) -> Result<(), Diagnostic> {
        crate::platform::recheck_directory(&self.directory).map_err(|_| publication_error())?;
        crate::platform::recheck_directory(&self.src).map_err(|_| publication_error())?;
        crate::platform::recheck_directory(&self.native).map_err(|_| publication_error())?;
        authenticate_inventory(src_scan, &self.src, &self.src_files)?;
        authenticate_inventory(native_scan, &self.native, &self.native_files)?;
        for name in ["Cargo.toml", "build.rs", "semaprax.native-rust-sdk.json"] {
            crate::platform::recheck_regular_file(
                self.root_files
                    .file(name)
                    .map_err(|_| publication_error())?,
            )
            .map_err(|_| publication_error())?;
        }
        crate::platform::inventory_entries_exact_prepared(
            root_scan,
            &self.directory,
            [
                self.root_files
                    .file("Cargo.toml")
                    .map_err(|_| publication_error())?,
                self.root_files
                    .file("build.rs")
                    .map_err(|_| publication_error())?,
                self.root_files
                    .file("semaprax.native-rust-sdk.json")
                    .map_err(|_| publication_error())?,
            ],
            [&self.src, &self.native],
        )
        .map_err(|_| publication_error())?;
        Ok(())
    }
}

fn create_outer_stage(
    parent: &crate::platform::HeldDirectory,
    path: &Path,
    plan: OuterStagePlan,
) -> Result<OuterStage, StageCreationError> {
    let OuterStagePlan {
        name: prepared_name,
        src_name,
        native_name,
        root_files,
        src_files,
        native_files,
    } = plan;
    let directory = crate::platform::create_directory_new_prepared(parent, &prepared_name, 0o700)
        .map_err(|_| StageCreationError::certain(publication_error()))?;
    let same_root = crate::platform::same_directory_path(&directory, path)
        .map_err(|_| publication_error())
        .unwrap_or(false);
    if !same_root {
        let cleanup = crate::platform::discard_owned_stage_prepared(
            parent,
            &directory,
            &prepared_name,
            &root_files,
        );
        return Err(if cleanup.is_err() {
            StageCreationError::uncertain(publication_error())
        } else {
            StageCreationError::certain(publication_error())
        });
    }
    let src = match crate::platform::create_directory_new_prepared(&directory, &src_name, 0o700) {
        Ok(src) => src,
        Err(_) => {
            let cleanup = crate::platform::discard_owned_stage_prepared(
                parent,
                &directory,
                &prepared_name,
                &root_files,
            );
            return Err(if cleanup.is_err() {
                StageCreationError::uncertain(publication_error())
            } else {
                StageCreationError::certain(publication_error())
            });
        }
    };
    let native =
        match crate::platform::create_directory_new_prepared(&directory, &native_name, 0o700) {
            Ok(native) => native,
            Err(_) => {
                if crate::platform::discard_owned_stage_prepared(
                    &directory, &src, &src_name, &src_files,
                )
                .is_err()
                {
                    return Err(StageCreationError::uncertain(publication_error()));
                }
                if crate::platform::discard_owned_stage_prepared(
                    parent,
                    &directory,
                    &prepared_name,
                    &root_files,
                )
                .is_err()
                {
                    return Err(StageCreationError::uncertain(publication_error()));
                }
                return Err(StageCreationError::certain(publication_error()));
            }
        };
    let same_src = crate::platform::same_directory_path(&src, &path.join("src"))
        .map_err(|_| publication_error())
        .unwrap_or(false);
    let same_native = crate::platform::same_directory_path(&native, &path.join("native"))
        .map_err(|_| publication_error())
        .unwrap_or(false);
    if !same_src || !same_native {
        if crate::platform::discard_owned_stage_prepared(&directory, &src, &src_name, &src_files)
            .is_err()
        {
            return Err(StageCreationError::uncertain(publication_error()));
        }
        if crate::platform::discard_owned_stage_prepared(
            &directory,
            &native,
            &native_name,
            &native_files,
        )
        .is_err()
        {
            return Err(StageCreationError::uncertain(publication_error()));
        }
        if crate::platform::discard_owned_stage_prepared(
            parent,
            &directory,
            &prepared_name,
            &root_files,
        )
        .is_err()
        {
            return Err(StageCreationError::uncertain(publication_error()));
        }
        return Err(StageCreationError::certain(publication_error()));
    }
    Ok(OuterStage {
        directory,
        src,
        native,
        name: prepared_name,
        src_name,
        native_name,
        root_files,
        src_files,
        native_files,
    })
}

#[allow(clippy::too_many_arguments)]
fn populate_outer_stage(
    stage: &mut OuterStage,
    sources: &PackageSources,
    descriptor: &[u8],
    inner_manifest: &[u8],
    safe_inner: &[u8],
    ffi_inner: &[u8],
    archive: &[u8],
    manifest: &str,
    archive_name: &str,
) -> Result<(), Diagnostic> {
    for (name, bytes) in [
        ("Cargo.toml", sources.cargo_toml.as_bytes()),
        ("build.rs", sources.build_rs.as_bytes()),
        ("semaprax.native-rust-sdk.json", manifest.as_bytes()),
    ] {
        crate::platform::write_file_new_prepared(
            &stage.directory,
            &mut stage.root_files,
            name,
            bytes,
            0o600,
        )
        .map_err(|_| publication_error())?;
        #[cfg(test)]
        if name == "Cargo.toml" && test_hook(TestBuildPoint::AfterFirstOuterWrite) {
            return Err(injected_error());
        }
    }
    for (name, bytes) in [
        ("lib.rs", sources.lib_rs.as_bytes()),
        ("semaprax_native_rust_interop.rs", safe_inner),
        ("semaprax_native_rust_interop_ffi.rs", ffi_inner),
    ] {
        crate::platform::write_file_new_prepared(
            &stage.src,
            &mut stage.src_files,
            name,
            bytes,
            0o600,
        )
        .map_err(|_| publication_error())?;
    }
    for (name, bytes) in [
        ("descriptor.json", descriptor),
        (archive_name, archive),
        ("semaprax.native-rust-interop.json", inner_manifest),
    ] {
        crate::platform::write_file_new_prepared(
            &stage.native,
            &mut stage.native_files,
            name,
            bytes,
            0o600,
        )
        .map_err(|_| publication_error())?;
    }
    Ok(())
}

fn discard_outer_stage(
    parent: &crate::platform::HeldDirectory,
    stage: &OuterStage,
) -> Result<(), Diagnostic> {
    let src = crate::platform::discard_owned_stage_prepared(
        &stage.directory,
        &stage.src,
        &stage.src_name,
        &stage.src_files,
    );
    if src.is_err() {
        return Err(publication_error());
    }
    let native = crate::platform::discard_owned_stage_prepared(
        &stage.directory,
        &stage.native,
        &stage.native_name,
        &stage.native_files,
    );
    if native.is_err() {
        return Err(publication_error());
    }
    crate::platform::discard_owned_stage_prepared(
        parent,
        &stage.directory,
        &stage.name,
        &stage.root_files,
    )
    .map_err(|_| publication_error())
}

fn fail_before_publish(
    parent: &crate::platform::HeldDirectory,
    outer: &OuterStage,
    archive: &ArchiveStage,
    inner: &InnerBundle,
    primary: Diagnostic,
) -> PublicBuildError {
    if discard_outer_stage(parent, outer).is_err() {
        return PublicBuildError::Many(vec![primary, publication_error()]);
    }
    if discard_archive_stage(parent, archive).is_err() {
        return PublicBuildError::Many(vec![primary, publication_error()]);
    }
    if discard_inner_bundle(parent, inner).is_err() {
        return PublicBuildError::Many(vec![primary, publication_error()]);
    }
    PublicBuildError::One(primary)
}

pub(super) fn build_native_rust_sdk_inner(
    program: &crate::ast::Program,
    options: NativeRustSdkOptions,
    output: &Path,
) -> Result<NativeRustSdkBundle, PublicBuildError> {
    let options = NativeRustSdkOptions {
        exports: canonical_values(options.exports, MAX_EXPORTS)?,
        imports: canonical_values(options.imports, MAX_IMPORTS)?,
        capabilities: canonical_values(options.capabilities, MAX_EFFECTS)?,
    };
    if options.exports.is_empty()
        || options
            .exports
            .iter()
            .any(|id| options.imports.binary_search(id).is_ok())
    {
        return Err(sdk_error("Native Rust SDK export and import selections are invalid").into());
    }
    let canonical_source = semaprax::format::canonical(program);
    let source_revision = domain_digest(SOURCE_DOMAIN, canonical_source.as_bytes());
    let target = target_triple()
        .ok_or_else(|| sdk_error("Native Rust SDK current target is unsupported"))?;
    let spec = canonical_spec(&program.module, &source_revision, target, &options)?;
    if output
        .to_str()
        .filter(|path| !path.contains(['\r', '\n']))
        .is_none()
    {
        return Err(publication_error().into());
    }
    let output_name = simple_output_name(output)?;
    let parent_path = output.parent().ok_or_else(publication_error)?;
    if !output.is_absolute() || !parent_path.is_absolute() {
        return Err(publication_error().into());
    }
    let parent = crate::platform::hold_directory(parent_path).map_err(|_| publication_error())?;
    crate::platform::recheck_directory(&parent).map_err(|_| publication_error())?;
    let output_probe =
        crate::platform::prepare_child_name(output_name).map_err(|_| publication_error())?;
    if !crate::platform::child_absent_prepared(&parent, &output_probe)
        .map_err(|_| publication_error())?
    {
        return Err(publication_error().into());
    }

    // All Phase-C process and publication plans are fixed before A+B starts.
    let configured_archiver = std::env::var_os("SEMAPRAX_ARCHIVER")
        .map(PathBuf::from)
        .filter(|path| path.is_absolute())
        .ok_or_else(publication_error)?;
    #[cfg(windows)]
    let vctools_path = std::env::var_os("SEMAPRAX_VCTOOLS")
        .map(PathBuf::from)
        .filter(|path| path.is_absolute())
        .ok_or_else(publication_error)?;
    #[cfg(windows)]
    let vctools = Some(vctools_path.as_path());
    #[cfg(not(windows))]
    let vctools: Option<&Path> = None;
    let archiver = crate::platform::hold_configured_archiver(configured_archiver, vctools)
        .map_err(|_| publication_error())?;
    let process_plan =
        crate::platform::prepare_process_arena_plan(1).map_err(|_| publication_error())?;
    let mut process_arena = crate::platform::materialize_process_arena(process_plan)
        .map_err(|_| publication_error())?;
    let object_name = if cfg!(windows) {
        "module.obj"
    } else {
        "module.o"
    };
    let archive_name = if cfg!(windows) {
        "semaprax_native_rust_sdk.lib"
    } else {
        "libsemaprax_native_rust_sdk.a"
    };
    let (_inner_text, inner_path, inner_stage_name) = planned_child(parent_path, &parent, "inner")?;
    let (_archive_text, archive_stage_path, archive_stage_name) =
        planned_child(parent_path, &parent, "archive")?;
    let (_outer_text, outer_path, outer_stage_name) =
        planned_child(parent_path, &parent, "package")?;
    let inner_inventory = crate::platform::prepare_discard_inventory([
        OsStr::new("descriptor.json"),
        OsStr::new("module.c"),
        OsStr::new(object_name),
        OsStr::new("semaprax_native_rust_interop.h"),
        OsStr::new("semaprax_native_rust_interop.rs"),
        OsStr::new("semaprax_native_rust_interop_ffi.rs"),
        OsStr::new("semaprax.native-rust-interop.json"),
    ])
    .map_err(|_| publication_error())?;
    let archive_inventory = crate::platform::prepare_discard_inventory([
        OsStr::new(object_name),
        OsStr::new(archive_name),
    ])
    .map_err(|_| publication_error())?;
    #[cfg(test)]
    let mut archive_inventory = archive_inventory;
    #[cfg(test)]
    if test_hook(TestBuildPoint::ArchiveCreationCleanupUncertainty) {
        archive_inventory.inject_discard_failure_after_delete(Some(0));
    }
    let root_inventory = crate::platform::prepare_discard_inventory([
        OsStr::new("Cargo.toml"),
        OsStr::new("build.rs"),
        OsStr::new("semaprax.native-rust-sdk.json"),
    ])
    .map_err(|_| publication_error())?;
    let src_inventory = crate::platform::prepare_discard_inventory([
        OsStr::new("lib.rs"),
        OsStr::new("semaprax_native_rust_interop.rs"),
        OsStr::new("semaprax_native_rust_interop_ffi.rs"),
    ])
    .map_err(|_| publication_error())?;
    let native_inventory = crate::platform::prepare_discard_inventory([
        OsStr::new("descriptor.json"),
        OsStr::new(archive_name),
        OsStr::new("semaprax.native-rust-interop.json"),
    ])
    .map_err(|_| publication_error())?;
    let src_stage_name =
        crate::platform::prepare_stage_name(OsStr::new("src")).map_err(|_| publication_error())?;
    let native_stage_name = crate::platform::prepare_stage_name(OsStr::new("native"))
        .map_err(|_| publication_error())?;
    let inner_scan = crate::platform::prepare_inventory_exact(&inner_inventory)
        .map_err(|_| publication_error())?;
    let mut archive_scan = crate::platform::prepare_inventory_exact(&archive_inventory)
        .map_err(|_| publication_error())?;
    let mut src_stage_scan = crate::platform::prepare_inventory_exact(&src_inventory)
        .map_err(|_| publication_error())?;
    let mut native_stage_scan = crate::platform::prepare_inventory_exact(&native_inventory)
        .map_err(|_| publication_error())?;
    let mut src_publish_scan = crate::platform::prepare_inventory_exact(&src_inventory)
        .map_err(|_| publication_error())?;
    let mut native_publish_scan = crate::platform::prepare_inventory_exact(&native_inventory)
        .map_err(|_| publication_error())?;
    let root_entry_names = [
        OsStr::new("Cargo.toml"),
        OsStr::new("build.rs"),
        OsStr::new("semaprax.native-rust-sdk.json"),
        OsStr::new("src"),
        OsStr::new("native"),
    ];
    let mut root_stage_scan = crate::platform::prepare_inventory_entries_exact(root_entry_names, 3)
        .map_err(|_| publication_error())?;
    let mut root_publish_scan =
        crate::platform::prepare_inventory_entries_exact(root_entry_names, 3)
            .map_err(|_| publication_error())?;
    let archive_invocation = crate::platform::prepare_archive_invocation(
        OsStr::new(object_name),
        OsStr::new(archive_name),
    )
    .map_err(|_| publication_error())?;
    let mut final_publish =
        crate::platform::prepare_publish_directory(output_name).map_err(|_| publication_error())?;

    // Private B remains byte-for-byte unchanged and publishes into an owned
    // sibling scratch directory that Phase C authenticates independently.
    let inner_facts = match crate::implementation::build_native_rust_interop_bundle(
        program,
        spec.as_bytes(),
        &inner_path,
    ) {
        Ok(facts) => facts,
        Err(errors) => return Err(PublicBuildError::Many(errors)),
    };
    let inner = authenticate_inner_bundle(
        &parent,
        inner_stage_name,
        &inner_path,
        object_name,
        inner_inventory,
        inner_scan,
    )?;
    #[cfg(test)]
    record_test_build_stage(TestBuildLastStage::InnerAuthenticated);
    let inner_prepared = (|| -> Result<_, Diagnostic> {
        if inner_facts.output_directory() != inner_path.as_path()
            || inner_facts.descriptor_path() != inner_path.join("descriptor.json")
            || inner_facts.manifest_path() != inner_path.join("semaprax.native-rust-interop.json")
        {
            return Err(publication_error());
        }
        let descriptor = read_inner(&inner.inventory, "descriptor.json", MAX_DESCRIPTOR_BYTES)?;
        let inner_manifest = read_inner(
            &inner.inventory,
            "semaprax.native-rust-interop.json",
            MAX_INNER_MANIFEST_BYTES,
        )?;
        let safe_inner = read_inner(
            &inner.inventory,
            "semaprax_native_rust_interop.rs",
            MAX_GENERATED_RUST_BYTES,
        )?;
        let ffi_inner = read_inner(
            &inner.inventory,
            "semaprax_native_rust_interop_ffi.rs",
            MAX_GENERATED_RUST_BYTES,
        )?;
        let object = read_inner(&inner.inventory, object_name, MAX_OBJECT_BYTES)?;
        let generated_c = read_inner(&inner.inventory, "module.c", MAX_OBJECT_BYTES)?;
        let generated_header = read_inner(
            &inner.inventory,
            "semaprax_native_rust_interop.h",
            MAX_DESCRIPTOR_BYTES,
        )?;
        verify_inner_payload_bindings(
            &inner_manifest,
            &InnerArtifacts {
                descriptor: &descriptor,
                generated_c: &generated_c,
                generated_header: &generated_header,
                safe_rust: &safe_inner,
                ffi_rust: &ffi_inner,
                object: &object,
                object_name,
            },
            inner_facts.manifest_digest(),
        )?;
        let descriptor_facts = parse_descriptor(
            &descriptor,
            &program.module,
            &source_revision,
            target,
            &options,
        )?;
        let sources = render_package_sources(&descriptor_facts, &options.capabilities);
        Ok((
            descriptor,
            inner_manifest,
            safe_inner,
            ffi_inner,
            object,
            descriptor_facts,
            sources,
        ))
    })();
    let (descriptor, inner_manifest, safe_inner, ffi_inner, object, descriptor_facts, sources) =
        match inner_prepared {
            Ok(prepared) => prepared,
            Err(error) => return Err(fail_after_inner(&parent, &inner, error)),
        };
    #[cfg(test)]
    record_test_build_stage(TestBuildLastStage::InnerPayloadVerified);

    // The archiver sees a private held run stage and one exact held object.
    let mut archive_stage = match create_archive_stage(
        &parent,
        archive_stage_name,
        &archive_stage_path,
        object_name,
        &object,
        archive_inventory,
    ) {
        Ok(stage) => stage,
        Err(error) if error.settlement_uncertain => return Err(error.stop()),
        Err(error) => return Err(fail_after_inner(&parent, &inner, error.primary)),
    };
    #[cfg(test)]
    record_test_build_stage(TestBuildLastStage::ArchiveStageCreated);
    let archive_result = (|| -> Result<Vec<u8>, Diagnostic> {
        #[cfg(test)]
        if test_hook(TestBuildPoint::BeforeArchive) {
            return Err(injected_error());
        }
        #[cfg(test)]
        if test_hook(TestBuildPoint::ArchiveOutputMutation) {
            std::fs::write(archive_stage_path.join(archive_name), b"foreign")
                .map_err(|_| publication_error())?;
        }
        #[cfg(test)]
        record_archive_attempt();
        let archive_file = crate::platform::archive_tool_prepared(
            &archiver,
            &archive_stage.directory,
            archive_stage
                .inventory
                .file(object_name)
                .map_err(|_| publication_error())?,
            archive_invocation,
            &mut process_arena,
        )
        .map_err(|_| publication_error())?;
        #[cfg(test)]
        record_test_build_stage(TestBuildLastStage::ArchiveToolReturned);
        archive_stage
            .inventory
            .attach(archive_name, archive_file)
            .map_err(|_| publication_error())?;
        #[cfg(test)]
        record_test_build_stage(TestBuildLastStage::ArchiveAttached);
        authenticate_inventory(
            &mut archive_scan,
            &archive_stage.directory,
            &archive_stage.inventory,
        )?;
        #[cfg(test)]
        record_test_build_stage(TestBuildLastStage::ArchiveInventoryAuthenticated);
        let archive = crate::platform::read_exact(
            archive_stage
                .inventory
                .file(archive_name)
                .map_err(|_| publication_error())?,
            MAX_ARCHIVE_BYTES,
        )
        .map_err(|_| publication_error())?;
        #[cfg(test)]
        record_test_build_stage(TestBuildLastStage::ArchiveRead);
        Ok(archive)
    })();
    let archive = match archive_result {
        Ok(archive) => archive,
        Err(error) => {
            return Err(fail_after_archive(&parent, &archive_stage, &inner, error));
        }
    };

    let mut outer = match create_outer_stage(
        &parent,
        &outer_path,
        OuterStagePlan {
            name: outer_stage_name,
            src_name: src_stage_name,
            native_name: native_stage_name,
            root_files: root_inventory,
            src_files: src_inventory,
            native_files: native_inventory,
        },
    ) {
        Ok(stage) => stage,
        Err(error) if error.settlement_uncertain => return Err(error.stop()),
        Err(error) => {
            return Err(fail_after_archive(
                &parent,
                &archive_stage,
                &inner,
                error.primary,
            ));
        }
    };
    #[cfg(test)]
    record_test_build_stage(TestBuildLastStage::OuterStageCreated);
    let outer_result = (|| -> Result<String, Diagnostic> {
        let manifest = render_sdk_manifest(
            &descriptor_facts,
            &options,
            &descriptor,
            &inner_manifest,
            &sources,
            &safe_inner,
            &ffi_inner,
            &archive,
        )?;
        verify_sdk_manifest(
            manifest.as_bytes(),
            &descriptor_facts,
            &options,
            &descriptor,
            &inner_manifest,
            &sources,
            &safe_inner,
            &ffi_inner,
            &archive,
        )?;
        populate_outer_stage(
            &mut outer,
            &sources,
            &descriptor,
            &inner_manifest,
            &safe_inner,
            &ffi_inner,
            &archive,
            &manifest,
            archive_name,
        )?;
        #[cfg(test)]
        record_test_build_stage(TestBuildLastStage::OuterStageWritten);
        outer.recheck_all(
            &mut root_stage_scan,
            &mut src_stage_scan,
            &mut native_stage_scan,
        )?;
        #[cfg(test)]
        record_test_build_stage(TestBuildLastStage::OuterInventoryAuthenticated);
        Ok(manifest)
    })();
    let manifest = match outer_result {
        Ok(manifest) => manifest,
        Err(error) => {
            return Err(fail_before_publish(
                &parent,
                &outer,
                &archive_stage,
                &inner,
                error,
            ));
        }
    };

    // Scratch settlement is mandatory before the public pivot. Any uncertain
    // discard is sticky and prevents later publication.
    #[cfg(test)]
    if test_hook(TestBuildPoint::ScratchCleanupUncertainty) {
        std::fs::write(archive_stage_path.join("foreign"), b"foreign")
            .map_err(|_| publication_error())?;
    }
    if discard_archive_stage(&parent, &archive_stage).is_err() {
        return Err(publication_error().into());
    }
    #[cfg(test)]
    record_test_build_stage(TestBuildLastStage::ArchiveScratchDiscarded);
    if discard_inner_bundle(&parent, &inner).is_err() {
        return Err(publication_error().into());
    }
    #[cfg(test)]
    record_test_build_stage(TestBuildLastStage::InnerScratchDiscarded);
    let publication = (|| -> Result<(), Diagnostic> {
        #[cfg(test)]
        #[cfg(debug_assertions)]
        if test_hook(TestBuildPoint::BeforePublish) {
            crate::platform::inject_publish_directory_failure(&mut final_publish, 4)
                .map_err(|_| publication_error())?;
        }
        crate::platform::recheck_directory(&parent).map_err(|_| publication_error())?;
        outer.recheck_all(
            &mut root_publish_scan,
            &mut src_publish_scan,
            &mut native_publish_scan,
        )?;
        #[cfg(test)]
        record_test_build_stage(TestBuildLastStage::PrePublishAuthenticated);
        #[cfg(test)]
        record_publish_call();
        crate::platform::publish_directory_new_prepared(
            &mut final_publish,
            &parent,
            &outer.directory,
            &outer.name,
            output_name,
        )
        .map_err(|_| publication_error())?;
        #[cfg(test)]
        record_test_build_stage(TestBuildLastStage::PublishReturned);
        Ok(())
    })();
    if let Err(error) = publication {
        let cleanup = discard_outer_stage(&parent, &outer);
        return Err(if cleanup.is_err() {
            publication_error().into()
        } else {
            error.into()
        });
    }

    // Post-publication replay is read-only. Failure leaves the complete,
    // digest-bound package for caller reconciliation; it is never deleted.
    #[cfg(test)]
    if test_hook(TestBuildPoint::PostPivotAuthenticationFailure) {
        return Err(publication_error().into());
    }
    let published_manifest = verify_published_package(
        output,
        &PublishedPackage {
            manifest: &manifest,
            archive_name,
            sources: &sources,
            descriptor: &descriptor,
            inner_manifest: &inner_manifest,
            safe_inner: &safe_inner,
            ffi_inner: &ffi_inner,
            archive: &archive,
        },
    )?;
    #[cfg(test)]
    record_test_build_stage(TestBuildLastStage::PublishedPackageAuthenticated);
    verify_sdk_manifest(
        &published_manifest,
        &descriptor_facts,
        &options,
        &descriptor,
        &inner_manifest,
        &sources,
        &safe_inner,
        &ffi_inner,
        &archive,
    )?;
    #[cfg(test)]
    record_test_build_stage(TestBuildLastStage::PublishedAuthenticated);
    let manifest_digest = domain_digest(SDK_MANIFEST_DOMAIN, manifest.as_bytes());
    Ok(NativeRustSdkBundle {
        output_directory: output.to_path_buf(),
        manifest_path: output.join("semaprax.native-rust-sdk.json"),
        manifest_digest,
        crate_name: CRATE_NAME.to_owned(),
        target_triple: target.to_owned(),
    })
}
