//! Phase-B stage preparation: discard inventories, link copies, the
//! prepared bundle plan, and held stage directories.

use super::*;

#[cfg(test)]
#[allow(clippy::enum_variant_names)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum NativeRustBuildPoint {
    BeforeClang,
    BeforeRustLink,
    BeforeExecutableAuthentication,
    BeforeExecute,
    BeforeObjectRead,
    BeforeManifestPublish,
    BeforeBundlePublish,
}

pub(super) type PublishDiscardInventory = platform::PreparedDiscardInventory<7>;
pub(super) type RunDiscardInventory = platform::PreparedDiscardInventory<10>;

fn native_relative_name_capacity(name: &OsStr) -> Result<usize, Diagnostic> {
    let bytes = name
        .to_str()
        .filter(|name| name.is_ascii())
        .ok_or_else(platform_publication_error)?
        .len();
    if cfg!(windows) {
        bytes
            .checked_mul(2)
            .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))
    } else {
        bytes
            .checked_add(1)
            .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))
    }
}

pub(super) fn prepare_discard_inventory<const N: usize>(
    names: [&'static OsStr; N],
) -> Result<platform::PreparedDiscardInventory<N>, Diagnostic> {
    let retained = names.iter().try_fold(0usize, |bytes, name| {
        bytes
            .checked_add(native_relative_name_capacity(name)?)
            .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))
    })?;
    let authority = reserve_temporary_exact(retained)?;
    let inventory = platform::prepare_discard_inventory_bounded(names, retained)
        .map_err(|_| platform_publication_error())?;
    if platform::prepared_discard_inventory_owned_capacity(&inventory) != retained {
        return Err(b109("max_builder_bytes", MAX_BUILDER_BYTES));
    }
    authority.retain(retained)?;
    Ok(inventory)
}

pub(super) fn prepare_publish_discard_inventory() -> Result<PublishDiscardInventory, Diagnostic> {
    prepare_discard_inventory([
        OsStr::new("descriptor.json"),
        OsStr::new("module.c"),
        OsStr::new("semaprax_native_rust_interop.h"),
        OsStr::new("semaprax_native_rust_interop.rs"),
        OsStr::new("semaprax_native_rust_interop_ffi.rs"),
        OsStr::new(if cfg!(windows) {
            "module.obj"
        } else {
            "module.o"
        }),
        OsStr::new("semaprax.native-rust-interop.json"),
    ])
}

pub(super) fn prepare_run_discard_inventory() -> Result<RunDiscardInventory, Diagnostic> {
    prepare_discard_inventory([
        OsStr::new("module_O0.o"),
        OsStr::new("__semaprax_native_rust_link.rs"),
        OsStr::new("semaprax_native_rust_interop.rs"),
        OsStr::new("semaprax_native_rust_interop_ffi.rs"),
        OsStr::new("module_O2.o"),
        OsStr::new(if cfg!(windows) {
            "semaprax_bridge.lib"
        } else {
            "libsemaprax_bridge.a"
        }),
        OsStr::new("__semaprax_native_rust_main.c"),
        OsStr::new("__semaprax_native_rust_main.o"),
        OsStr::new(if cfg!(windows) {
            "__semaprax_native_rust_link_O0.exe"
        } else {
            "__semaprax_native_rust_link_O0"
        }),
        OsStr::new(if cfg!(windows) {
            "__semaprax_native_rust_link_O2.exe"
        } else {
            "__semaprax_native_rust_link_O2"
        }),
    ])
}

pub(super) struct PreparedLinkCopies {
    pub(super) safe_rust: (platform::PreparedLinkOrCopy, TemporaryBudget),
    pub(super) private_ffi: (platform::PreparedLinkOrCopy, TemporaryBudget),
    pub(super) optimized_object: (platform::PreparedLinkOrCopy, TemporaryBudget),
}

pub(super) fn prepare_link_copy<const S: usize, const D: usize>(
    source: &platform::PreparedDiscardInventory<S>,
    source_name: &'static str,
    destination: &platform::PreparedDiscardInventory<D>,
    destination_name: &'static str,
) -> Result<(platform::PreparedLinkOrCopy, TemporaryBudget), PhaseBLocalError> {
    let required = platform::link_or_copy_required_capacity(
        source,
        source_name,
        destination,
        destination_name,
    )
    .map_err(|_| PhaseBLocalError::BuilderBudget)?;
    let budget = reserve_phase_b(required)?;
    let prepared =
        platform::prepare_link_or_copy(source, source_name, destination, destination_name)
            .map_err(|_| PhaseBLocalError::BuilderBudget)?;
    if platform::prepared_link_or_copy_owned_capacity(&prepared) != required {
        return Err(PhaseBLocalError::BuilderBudget);
    }
    #[cfg(test)]
    PHASE_B_LINK_COPY_PLANS.with(|count| count.set(count.get().saturating_add(1)));
    Ok((prepared, budget))
}

pub(super) fn prepare_link_copies(
    publish: &PublishDiscardInventory,
    run: &RunDiscardInventory,
    object_name: &'static str,
) -> Result<PreparedLinkCopies, PhaseBLocalError> {
    let prepared = PreparedLinkCopies {
        safe_rust: prepare_link_copy(
            publish,
            "semaprax_native_rust_interop.rs",
            run,
            "semaprax_native_rust_interop.rs",
        )?,
        private_ffi: prepare_link_copy(
            publish,
            "semaprax_native_rust_interop_ffi.rs",
            run,
            "semaprax_native_rust_interop_ffi.rs",
        )?,
        optimized_object: prepare_link_copy(publish, object_name, run, "module_O2.o")?,
    };
    #[cfg(all(test, debug_assertions))]
    let prepared = {
        let mut prepared = prepared;
        if PHASE_B_LINK_COPY_FAIL_BEFORE_AUTHENTICATION.with(std::cell::Cell::get) {
            platform::inject_link_or_copy_failure_before_authentication(&mut prepared.safe_rust.0);
        }
        prepared
    };
    Ok(prepared)
}

pub(super) fn consume_link_copy<const S: usize, const D: usize>(
    plan: (platform::PreparedLinkOrCopy, TemporaryBudget),
    source: &platform::PreparedDiscardInventory<S>,
    destination_directory: &HeldStage,
    destination: &mut platform::PreparedDiscardInventory<D>,
    source_bytes: &[u8],
) -> Result<(), PhaseBLocalError> {
    let (prepared, budget) = plan;
    #[cfg(test)]
    PHASE_B_LINK_COPY_CONSUMPTIONS.with(|count| count.set(count.get().saturating_add(1)));
    let result = platform::link_or_copy_new_prepared(
        prepared,
        source,
        destination_directory.authority.held(),
        destination,
        source_bytes,
    )
    .map_err(|_| PhaseBLocalError::Publication);
    drop(budget);
    result
}

pub(super) fn prepare_publish_inventory_exact(
    publish: &PublishDiscardInventory,
) -> Result<(platform::PreparedInventoryExact<7>, TemporaryBudget), PhaseBLocalError> {
    let required = platform::inventory_exact_required_capacity(publish)
        .map_err(|_| PhaseBLocalError::BuilderBudget)?;
    let budget = reserve_phase_b(required)?;
    let prepared =
        platform::prepare_inventory_exact(publish).map_err(|_| PhaseBLocalError::BuilderBudget)?;
    if platform::prepared_inventory_exact_owned_capacity(&prepared) != required
        || platform::prepared_inventory_exact_remaining(&prepared) != 2
    {
        return Err(PhaseBLocalError::BuilderBudget);
    }
    #[cfg(test)]
    PHASE_B_INVENTORY_EXACT_PLANS.with(|count| count.set(count.get().saturating_add(1)));
    Ok((prepared, budget))
}

pub(super) fn scan_publish_inventory_exact(
    prepared: &mut platform::PreparedInventoryExact<7>,
    stage: &HeldStage,
    publish: &PublishDiscardInventory,
) -> Result<(), PhaseBLocalError> {
    #[cfg(test)]
    PHASE_B_INVENTORY_EXACT_SCANS.with(|count| count.set(count.get().saturating_add(1)));
    platform::inventory_exact_prepared(prepared, stage.authority.held(), publish)
        .map_err(|_| PhaseBLocalError::Publication)
}

pub(super) fn prepare_final_publish(
    output: &Path,
) -> Result<(platform::PreparedPublishDirectory, TemporaryBudget), PhaseBLocalError> {
    let output_name = output.file_name().ok_or(PhaseBLocalError::Publication)?;
    let required = platform::publish_directory_required_capacity(output_name)
        .map_err(|_| PhaseBLocalError::Publication)?;
    let budget = reserve_phase_b(required)?;
    let prepared = platform::prepare_publish_directory(output_name)
        .map_err(|_| PhaseBLocalError::Publication)?;
    if platform::prepared_publish_directory_owned_capacity(&prepared) != required
        || platform::prepared_publish_directory_remaining(&prepared) != 1
    {
        return Err(PhaseBLocalError::BuilderBudget);
    }
    #[cfg(test)]
    PHASE_B_PUBLISH_PLANS.with(|count| count.set(count.get().saturating_add(1)));
    Ok((prepared, budget))
}

#[cfg(not(test))]
#[allow(clippy::enum_variant_names)]
#[derive(Clone, Copy)]
pub(super) enum NativeRustBuildPoint {
    BeforeClang,
    BeforeRustLink,
    BeforeExecutableAuthentication,
    BeforeExecute,
    BeforeObjectRead,
    BeforeManifestPublish,
    BeforeBundlePublish,
}

#[cfg(test)]
pub(super) fn build_native_rust_interop_bundle_with_hook(
    program: &Program,
    spec_bytes: &[u8],
    output: &Path,
    mut hook: impl FnMut(NativeRustBuildPoint, &Path, &Path, &Path),
) -> Result<NativeRustInteropBundleFacts, Vec<Diagnostic>> {
    reset_phase_b_error_materialization_observer();
    let (result, overflowed) = crate::bounded_output::with_limit(MAX_BUILDER_BYTES, || {
        build_native_rust_interop_bundle_bounded(program, spec_bytes, output, &mut hook)
    });
    finish_bounded_bundle(result, overflowed)
}

pub(super) struct PreparedPhaseB {
    prepared: PreparedNativeRustInterop,
    pending_facts: PendingBundleFacts,
    publish_slot: StageSlot,
    run_slot: StageSlot,
    publish_files: PublishDiscardInventory,
    run_files: RunDiscardInventory,
    parent_path: PathBuf,
    carriers: PhaseBErrorCarriers,
    toolchain_plan: PreparedToolchainPlan,
    harness_plan: (String, TemporaryBudget),
    build_invocations: PreparedBuildInvocations,
    manifest_plan: PreparedManifestPlan,
    link_copies: PreparedLinkCopies,
    inventory_exact: (platform::PreparedInventoryExact<7>, TemporaryBudget),
    final_publish: (platform::PreparedPublishDirectory, TemporaryBudget),
}

pub(super) fn prepare_phase_b(
    program: &Program,
    spec_bytes: &[u8],
    output: &Path,
) -> Result<PreparedPhaseB, BundleBuildError> {
    let prepared = prepare_native_rust_interop_bounded(program, spec_bytes)?;
    prepare_phase_b_from_prepared(prepared, output)
}

pub(super) fn prepare_phase_b_from_prepared(
    prepared: PreparedNativeRustInterop,
    output: &Path,
) -> Result<PreparedPhaseB, BundleBuildError> {
    let object_name: &'static str = if cfg!(windows) {
        "module.obj"
    } else {
        "module.o"
    };
    let parent = output.parent().ok_or_else(platform_publication_error)?;
    let pending_facts = PendingBundleFacts::new(output, object_name, &prepared.descriptor_digest)?;
    let publish_slot = StageSlot::new(parent, &prepared.descriptor_digest, "publish")?;
    let run_slot = StageSlot::new(parent, &prepared.descriptor_digest, "run")?;
    let publish_files = prepare_publish_discard_inventory()?;
    let run_files = prepare_run_discard_inventory()?;
    #[cfg(all(test, debug_assertions))]
    let run_files = {
        let mut run_files = run_files;
        run_files.inject_discard_failure_after_delete(
            PHASE_B_DISCARD_FAILURE_AFTER_DELETE.with(std::cell::Cell::get),
        );
        run_files
    };
    let parent_capacity = parent.as_os_str().as_encoded_bytes().len();
    let parent_budget = reserve_temporary_exact(parent_capacity)?;
    let parent_path = exact_path_copy(parent, parent_capacity)?;
    parent_budget.retain(parent_capacity)?;
    let mut carriers = PhaseBErrorCarriers::prepare()?;
    let toolchain_plan = match prepare_toolchain_plan() {
        Ok(plan) => plan,
        Err(error) => return Err(carriers.error(error)),
    };
    let harness_plan = match prepare_rust_harness(&prepared) {
        Ok(plan) => plan,
        Err(error) => return Err(carriers.error(error)),
    };
    let build_invocations = match prepare_build_invocations(
        &prepared,
        planned_sanitizers(&toolchain_plan),
        planned_linker(&toolchain_plan),
        planned_vctools(&toolchain_plan),
    ) {
        Ok(plan) => plan,
        Err(error) => return Err(carriers.error(error)),
    };
    let manifest_plan = match PreparedManifestPlan::prepare(object_name) {
        Ok(plan) => plan,
        Err(error) => return Err(carriers.error(error)),
    };
    let link_copies = match prepare_link_copies(&publish_files, &run_files, object_name) {
        Ok(plan) => plan,
        Err(error) => return Err(carriers.error(error)),
    };
    let inventory_exact = match prepare_publish_inventory_exact(&publish_files) {
        Ok(plan) => plan,
        Err(error) => return Err(carriers.error(error)),
    };
    let final_publish = match prepare_final_publish(output) {
        Ok(plan) => plan,
        Err(error) => return Err(carriers.error(error)),
    };
    Ok(PreparedPhaseB {
        prepared,
        pending_facts,
        publish_slot,
        run_slot,
        publish_files,
        run_files,
        parent_path,
        carriers,
        toolchain_plan,
        harness_plan,
        build_invocations,
        manifest_plan,
        link_copies,
        inventory_exact,
        final_publish,
    })
}

pub(super) fn build_native_rust_interop_bundle_bounded(
    program: &Program,
    spec_bytes: &[u8],
    output: &Path,
    hook: &mut dyn FnMut(NativeRustBuildPoint, &Path, &Path, &Path),
) -> Result<BundleBuildSuccess, BundleBuildError> {
    let phase = prepare_phase_b(program, spec_bytes, output)?;
    build_prepared_phase_b_bounded(phase, output, hook)
}

pub(super) fn build_prepared_phase_b_bounded(
    phase: PreparedPhaseB,
    output: &Path,
    hook: &mut dyn FnMut(NativeRustBuildPoint, &Path, &Path, &Path),
) -> Result<BundleBuildSuccess, BundleBuildError> {
    let PreparedPhaseB {
        prepared,
        mut pending_facts,
        publish_slot,
        run_slot,
        mut publish_files,
        mut run_files,
        parent_path,
        mut carriers,
        toolchain_plan,
        harness_plan,
        build_invocations,
        manifest_plan,
        link_copies,
        inventory_exact,
        mut final_publish,
    } = phase;

    mark_phase_b_effect_started();
    #[cfg(test)]
    PHASE_B_OUTPUT_PROBES.with(|count| count.set(count.get().saturating_add(1)));
    if output.exists() {
        return Err(carriers.error(PhaseBLocalError::Publication));
    }
    let parent_authority = match hold_stage(parent_path) {
        Ok(parent) => parent,
        Err(error) => return Err(carriers.error(error)),
    };
    if parent_authority.recheck_local().is_err() {
        return Err(carriers.error(PhaseBLocalError::Publication));
    }
    let stage = match create_stage(&parent_authority, publish_slot, &publish_files) {
        Ok(stage) => stage,
        Err(error) => return Err(carriers.error(error)),
    };
    let run_stage = match parent_authority
        .recheck_local()
        .and_then(|()| create_stage(&parent_authority, run_slot, &run_files))
    {
        Ok(run_stage) => run_stage,
        Err(error) => {
            let _ = discard_run_stage(&parent_authority, &stage, &publish_files);
            return Err(carriers.error(error));
        }
    };
    let build = (|| {
        #[cfg(test)]
        if let Some(error) = PHASE_B_LOCAL_FAILURE_INJECTION.with(std::cell::Cell::get) {
            return Err(error);
        }
        let mut tools =
            authenticate_toolchain(toolchain_plan, &prepared.target, run_stage.authority.held())?;
        build_stage_platform(
            &prepared,
            &mut tools,
            &stage,
            &run_stage,
            harness_plan,
            build_invocations,
            link_copies,
            inventory_exact,
            manifest_plan,
            output,
            hook,
            &mut run_files,
            &mut publish_files,
        )
    })();
    let cleanup = discard_run_stage(&parent_authority, &run_stage, &run_files);
    let mut facts = match (build, cleanup) {
        (Err(error), _) => {
            let _ = discard_run_stage(&parent_authority, &stage, &publish_files);
            return Err(carriers.error(error));
        }
        (Ok(_), Err(error)) => {
            let _ = discard_run_stage(&parent_authority, &stage, &publish_files);
            return Err(carriers.error(error));
        }
        (Ok(facts), Ok(())) => facts,
    };
    if let Err(error) = facts.observe_object_authority_for_manifest() {
        let _ = discard_run_stage(&parent_authority, &stage, &publish_files);
        return Err(carriers.error(error));
    }
    if let Err(error) =
        pending_facts.bind_manifest_digest(facts.manifest.as_bytes(), prepared.is_project())
    {
        let _ = discard_run_stage(&parent_authority, &stage, &publish_files);
        return Err(carriers.error(error));
    }
    let bundle_facts = pending_facts.finish();
    let publication: Result<NativeRustInteropBundleFacts, PhaseBLocalError> = (|| {
        parent_authority.recheck_local()?;
        stage.recheck_local()?;
        hook(
            NativeRustBuildPoint::BeforeBundlePublish,
            &stage.path,
            &run_stage.path,
            output,
        );
        publish_stage_platform(
            &parent_authority,
            &stage,
            output,
            &prepared,
            &mut facts,
            &mut publish_files,
            &mut final_publish.0,
        )?;
        Ok(bundle_facts)
    })();
    if publication.is_err() {
        let _ = discard_run_stage(&parent_authority, &stage, &publish_files);
    }
    match publication {
        Ok(facts) => Ok(BundleBuildSuccess {
            facts,
            overflow: carriers.take(PhaseBLocalError::BuilderBudget),
        }),
        Err(error) => Err(carriers.error(error)),
    }
}

pub(super) struct HeldStage {
    pub(super) path: PathBuf,
    pub(super) authority: crate::workspace::AuthenticatedDirectory,
    pub(super) discard_name: Option<platform::PreparedStageName>,
}

pub(super) struct StageSlot {
    purpose: &'static str,
    digest_prefix: [u8; 16],
    pub(super) name: String,
    pub(super) path: PathBuf,
    path_capacity: usize,
    pub(super) native_name: platform::PreparedStageName,
}

impl StageSlot {
    pub(super) fn new(
        parent: &Path,
        digest: &str,
        purpose: &'static str,
    ) -> Result<Self, Diagnostic> {
        let digest = Sha256::digest(digest.as_bytes());
        let mut digest_prefix = [0_u8; 16];
        const HEX: &[u8; 16] = b"0123456789abcdef";
        for (index, byte) in digest.iter().take(8).copied().enumerate() {
            digest_prefix[index * 2] = HEX[usize::from(byte >> 4)];
            digest_prefix[index * 2 + 1] = HEX[usize::from(byte & 0x0f)];
        }
        let path_capacity = exact_child_path_capacity(parent, PHASE_B_STAGE_NAME_CAPACITY)
            .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
        let retained = PHASE_B_STAGE_NAME_CAPACITY
            .checked_add(path_capacity)
            .and_then(|bytes| {
                bytes.checked_add(if cfg!(windows) {
                    PHASE_B_STAGE_NAME_CAPACITY.checked_mul(2)?
                } else {
                    PHASE_B_STAGE_NAME_CAPACITY.checked_add(1)?
                })
            })
            .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
        let authority = reserve_temporary_exact(retained)?;
        let name = String::with_capacity(PHASE_B_STAGE_NAME_CAPACITY);
        let path = PathBuf::with_capacity(path_capacity);
        let native_name = platform::prepare_stage_name_arena(PHASE_B_STAGE_NAME_CAPACITY)
            .map_err(|_| platform_publication_error())?;
        #[cfg(test)]
        PHASE_B_NATIVE_STAGE_ARENA_ALLOCATIONS.with(|count| {
            count.set(count.get().saturating_add(1));
        });
        if name.capacity() != PHASE_B_STAGE_NAME_CAPACITY || path.capacity() != path_capacity {
            return Err(b109("max_builder_bytes", MAX_BUILDER_BYTES));
        }
        authority.retain(retained)?;
        Ok(Self {
            purpose,
            digest_prefix,
            name,
            path,
            path_capacity,
            native_name,
        })
    }

    pub(super) fn prepare(&mut self, parent: &Path, nonce: u32) -> Result<(), PhaseBLocalError> {
        self.name.clear();
        write!(
            self.name,
            ".semaprax-native-rust-interop-{}-{}-{}-{nonce}",
            self.purpose,
            std::process::id(),
            std::str::from_utf8(&self.digest_prefix).map_err(|_| PhaseBLocalError::Publication)?,
        )
        .map_err(|_| PhaseBLocalError::Publication)?;
        if self.name.capacity() != PHASE_B_STAGE_NAME_CAPACITY {
            return Err(PhaseBLocalError::Publication);
        }
        if !fill_exact_child_path(&mut self.path, parent, self.name.as_ref())
            || self.path.capacity() != self.path_capacity
            || !exact_child_path_matches(&self.path, parent, self.name.as_ref())
        {
            return Err(PhaseBLocalError::Publication);
        }
        self.native_name
            .set(self.name.as_ref())
            .map_err(|_| PhaseBLocalError::Publication)?;
        #[cfg(test)]
        PHASE_B_NATIVE_STAGE_ARENA_SETS.with(|count| {
            count.set(count.get().saturating_add(1));
        });
        Ok(())
    }
}

impl HeldStage {
    pub(super) fn recheck_local(&self) -> Result<(), PhaseBLocalError> {
        self.authority
            .recheck()
            .map_err(|_| PhaseBLocalError::Publication)?;
        if !self.authority.same_directory_path(&self.path) {
            return Err(PhaseBLocalError::Publication);
        }
        Ok(())
    }

    pub(super) fn recheck(&self) -> Result<(), Diagnostic> {
        self.recheck_local()
            .map_err(|_| platform_publication_error())
    }
}

pub(super) fn hold_stage(path: PathBuf) -> Result<HeldStage, PhaseBLocalError> {
    let authority = crate::workspace::authenticate_directory_held(&path)
        .map_err(|_| PhaseBLocalError::Publication)?;
    Ok(HeldStage {
        path,
        authority,
        discard_name: None,
    })
}

pub(super) fn create_stage<const N: usize>(
    parent: &HeldStage,
    mut slot: StageSlot,
    inventory: &platform::PreparedDiscardInventory<N>,
) -> Result<HeldStage, PhaseBLocalError> {
    for nonce in 0_u32..1024 {
        slot.prepare(&parent.path, nonce)?;
        #[cfg(test)]
        PHASE_B_NATIVE_STAGE_ARENA_CONSUMPTIONS.with(|count| {
            count.set(count.get().saturating_add(1));
        });
        match platform::create_directory_new_prepared(
            parent.authority.held(),
            &slot.native_name,
            0o700,
        ) {
            Ok(held) => {
                #[cfg(test)]
                let authentication_path = match CREATE_AUTH_DISAGREEMENT.with(std::cell::Cell::get)
                {
                    None => None,
                    Some(CreateAuthDisagreement::Clean) => Some(parent.path.clone()),
                    Some(CreateAuthDisagreement::Substituted) => {
                        let displaced = parent.path.join("auth-displaced");
                        std::fs::rename(&slot.path, &displaced)
                            .map_err(|_| PhaseBLocalError::Publication)?;
                        std::fs::create_dir(&slot.path)
                            .map_err(|_| PhaseBLocalError::Publication)?;
                        std::fs::write(slot.path.join("foreign-sentinel"), b"foreign")
                            .map_err(|_| PhaseBLocalError::Publication)?;
                        Some(slot.path.clone())
                    }
                };
                #[cfg(test)]
                let authentication_path = authentication_path.as_deref().unwrap_or(&slot.path);
                #[cfg(not(test))]
                let authentication_path = &slot.path;
                let authority = match crate::workspace::authenticate_created_directory(
                    authentication_path,
                    held,
                ) {
                    Ok(authority) => authority,
                    Err(crate::workspace::CreatedDirectoryAuthenticationError::Disagreement(
                        raw_child,
                    )) => {
                        #[cfg(test)]
                        CREATE_AUTH_DISCARD_ATTEMPTS.with(|attempts| {
                            attempts.set(attempts.get().saturating_add(1));
                        });
                        let _ = platform::discard_owned_stage_prepared(
                            parent.authority.held(),
                            &raw_child,
                            &slot.native_name,
                            inventory,
                        );
                        return Err(PhaseBLocalError::Publication);
                    }
                };
                return Ok(HeldStage {
                    path: slot.path,
                    authority,
                    discard_name: Some(slot.native_name),
                });
            }
            Err(platform::Error::Exists) => {}
            Err(_) => break,
        }
    }
    Err(PhaseBLocalError::Publication)
}

#[cfg(test)]
fn exact_inventory(directory: &Path, expected: &BTreeSet<&str>) -> Result<(), Diagnostic> {
    let metadata = std::fs::symlink_metadata(directory)
        .map_err(|_| Diagnostic::io("SPX-I232", "Native Rust Interop output publication failed"))?;
    if !metadata.is_dir() || metadata.file_type().is_symlink() {
        return Err(Diagnostic::io(
            "SPX-I232",
            "Native Rust Interop output publication failed",
        ));
    }
    let mut actual = BTreeSet::new();
    for entry in std::fs::read_dir(directory)
        .map_err(|_| Diagnostic::io("SPX-I232", "Native Rust Interop output publication failed"))?
    {
        if actual.len() >= expected.len() {
            return Err(Diagnostic::io(
                "SPX-I232",
                "Native Rust Interop output publication failed",
            ));
        }
        let entry = entry.map_err(|_| {
            Diagnostic::io("SPX-I232", "Native Rust Interop output publication failed")
        })?;
        let name = entry.file_name().into_string().map_err(|_| {
            Diagnostic::io("SPX-I232", "Native Rust Interop output publication failed")
        })?;
        if !expected.contains(name.as_str()) || !crate::bounded_output::reserve_active(name.len()) {
            return Err(Diagnostic::io(
                "SPX-I232",
                "Native Rust Interop output publication failed",
            ));
        }
        let metadata = std::fs::symlink_metadata(entry.path()).map_err(|_| {
            Diagnostic::io("SPX-I232", "Native Rust Interop output publication failed")
        })?;
        if !metadata.is_file() || metadata.file_type().is_symlink() {
            return Err(Diagnostic::io(
                "SPX-I232",
                "Native Rust Interop output publication failed",
            ));
        }
        if !actual.insert(name) {
            return Err(Diagnostic::io(
                "SPX-I232",
                "Native Rust Interop output publication failed",
            ));
        }
    }
    if actual.iter().map(String::as_str).collect::<BTreeSet<_>>() != *expected {
        return Err(Diagnostic::io(
            "SPX-I232",
            "Native Rust Interop output publication failed",
        ));
    }
    Ok(())
}

#[cfg(test)]
pub(super) fn match_regular_file(path: &Path, expected: &[u8]) -> Result<(), Diagnostic> {
    let before_path = std::fs::symlink_metadata(path)
        .map_err(|_| Diagnostic::io("SPX-I232", "Native Rust Interop output publication failed"))?;
    if !before_path.is_file() || before_path.file_type().is_symlink() {
        return Err(Diagnostic::io(
            "SPX-I232",
            "Native Rust Interop output publication failed",
        ));
    }
    let mut options = std::fs::OpenOptions::new();
    options.read(true).write(false);
    #[cfg(all(unix, target_os = "macos"))]
    {
        use std::os::unix::fs::OpenOptionsExt as _;
        options.custom_flags(0x0000_0100);
    }
    #[cfg(all(unix, not(target_os = "macos")))]
    {
        use std::os::unix::fs::OpenOptionsExt as _;
        options.custom_flags(0x0002_0000);
    }
    #[cfg(windows)]
    {
        use std::os::windows::fs::OpenOptionsExt as _;
        options.custom_flags(0x0020_0000);
    }
    let mut file = options
        .open(path)
        .map_err(|_| Diagnostic::io("SPX-I232", "Native Rust Interop output publication failed"))?;
    let before = file
        .metadata()
        .map_err(|_| Diagnostic::io("SPX-I232", "Native Rust Interop output publication failed"))?;
    if !before.is_file()
        || !same_file_metadata(&before_path, &before)
        || before.len() != u64::try_from(expected.len()).unwrap_or(u64::MAX)
    {
        return Err(Diagnostic::io(
            "SPX-I232",
            "Native Rust Interop output publication failed",
        ));
    }
    let mut offset = 0_usize;
    let mut buffer = [0_u8; 8192];
    loop {
        let length = std::io::Read::read(&mut file, &mut buffer).map_err(|_| {
            Diagnostic::io("SPX-I232", "Native Rust Interop output publication failed")
        })?;
        if length == 0 {
            break;
        }
        let end = offset.checked_add(length).ok_or_else(|| {
            Diagnostic::io("SPX-I232", "Native Rust Interop output publication failed")
        })?;
        if expected.get(offset..end) != Some(&buffer[..length]) {
            return Err(Diagnostic::io(
                "SPX-I232",
                "Native Rust Interop output publication failed",
            ));
        }
        offset = end;
    }
    let after = file
        .metadata()
        .map_err(|_| Diagnostic::io("SPX-I232", "Native Rust Interop output publication failed"))?;
    if offset != expected.len() || !same_file_metadata(&before, &after) {
        return Err(Diagnostic::io(
            "SPX-I232",
            "Native Rust Interop output publication failed",
        ));
    }
    Ok(())
}
