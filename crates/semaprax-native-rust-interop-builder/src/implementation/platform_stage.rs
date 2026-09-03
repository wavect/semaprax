//! The only production code that holds physical platform authority:
//! stage building and the final publication pivot.

use super::*;

pub(super) fn platform_publication_error() -> Diagnostic {
    observe_phase_b_error_materialization();
    Diagnostic::io("SPX-I232", PHASE_B_PUBLICATION_MESSAGE)
}

fn platform_compile_error() -> Diagnostic {
    Diagnostic::io("SPX-I230", "Native Rust Interop Clang compilation failed")
}

fn platform_link_error() -> Diagnostic {
    Diagnostic::io(
        "SPX-I231",
        "Native Rust Interop Rust compilation or link failed",
    )
}

fn map_link_error(_error: platform::Error) -> Diagnostic {
    #[cfg(test)]
    eprintln!("native rust platform link error: {_error:?}");
    platform_link_error()
}

fn write_platform_file<const N: usize>(
    directory: &HeldStage,
    files: &mut platform::PreparedDiscardInventory<N>,
    name: &str,
    bytes: &[u8],
) -> Result<(), PhaseBLocalError> {
    files
        .validate_next(name)
        .map_err(|_| PhaseBLocalError::Publication)?;
    directory.recheck_local()?;
    platform::write_file_new_prepared(directory.authority.held(), files, name, bytes, 0o600)
        .map_err(|_| PhaseBLocalError::Publication)
}

pub(super) fn discard_run_stage<const N: usize>(
    parent: &HeldStage,
    stage: &HeldStage,
    files: &platform::PreparedDiscardInventory<N>,
) -> Result<(), PhaseBLocalError> {
    #[cfg(test)]
    PHASE_B_DISCARD_ATTEMPTS.with(|attempts| {
        attempts.set(attempts.get().saturating_add(1));
    });
    let stage_name = stage
        .discard_name
        .as_ref()
        .ok_or(PhaseBLocalError::Publication)?;
    platform::discard_owned_stage_prepared(
        parent.authority.held(),
        stage.authority.held(),
        stage_name,
        files,
    )
    .map_err(|_| PhaseBLocalError::Publication)
}

fn track_run_file<const N: usize>(
    files: &mut platform::PreparedDiscardInventory<N>,
    name: &str,
    file: platform::HeldRegularFile,
) -> Result<(), PhaseBLocalError> {
    files
        .attach(name, file)
        .map_err(|_| PhaseBLocalError::Publication)
}

fn recheck_tracked_files<const N: usize>(
    files: &platform::PreparedDiscardInventory<N>,
    required: &[&str],
) -> Result<(), PhaseBLocalError> {
    files
        .recheck(required)
        .map_err(|_| PhaseBLocalError::Publication)
}

pub(super) const REQUIRED_NATIVE_RUST_SANITIZER_FLAGS: [&str; 2] =
    ["-fsanitize=address,undefined", "-fno-sanitize-recover=all"];

pub(super) fn sanitizer_mode() -> Result<bool, PhaseBLocalError> {
    match std::env::var_os("SEMAPRAX_REQUIRE_NATIVE_RUST_INTEROP_SANITIZERS") {
        None => Ok(false),
        Some(value) if value == "1" && cfg!(target_os = "linux") => {
            let _ = REQUIRED_NATIVE_RUST_SANITIZER_FLAGS;
            Ok(true)
        }
        Some(_) => Err(PhaseBLocalError::Link),
    }
}

#[allow(
    clippy::too_many_arguments,
    reason = "held publish and run inventories stay explicit"
)]
pub(super) fn build_stage_platform(
    prepared: &PreparedNativeRustInterop,
    tools: &mut ToolchainFacts,
    stage: &HeldStage,
    run_stage: &HeldStage,
    harness_plan: (String, TemporaryBudget),
    build_invocations: PreparedBuildInvocations,
    link_copies: PreparedLinkCopies,
    mut inventory_exact: (platform::PreparedInventoryExact<7>, TemporaryBudget),
    manifest_plan: PreparedManifestPlan,
    output: &Path,
    hook: &mut dyn FnMut(NativeRustBuildPoint, &Path, &Path, &Path),
    run_files: &mut RunDiscardInventory,
    publish_files: &mut PublishDiscardInventory,
) -> Result<BuildStageFacts, PhaseBLocalError> {
    let PreparedBuildInvocations {
        c_o0,
        c_o2,
        rust,
        c_main,
        link_o0,
        run_o0,
        link_o2,
        run_o2,
    } = build_invocations;
    let PreparedLinkCopies {
        safe_rust,
        private_ffi,
        optimized_object,
    } = link_copies;
    for (name, bytes) in [
        ("descriptor.json", prepared.descriptor.as_bytes()),
        ("module.c", prepared.generated_c.as_bytes()),
        (
            "semaprax_native_rust_interop.h",
            prepared.generated_header.as_bytes(),
        ),
        (
            "semaprax_native_rust_interop.rs",
            prepared.generated_rust.as_bytes(),
        ),
        (
            "semaprax_native_rust_interop_ffi.rs",
            prepared.private_ffi_source.as_bytes(),
        ),
    ] {
        write_platform_file(stage, publish_files, name, bytes)?;
    }
    let object_name = if cfg!(windows) {
        "module.obj"
    } else {
        "module.o"
    };
    hook(
        NativeRustBuildPoint::BeforeClang,
        &stage.path,
        &run_stage.path,
        output,
    );
    stage.recheck_local()?;
    recheck_tracked_files(
        publish_files,
        &["module.c", "semaprax_native_rust_interop.h"],
    )?;
    let mut retained_object = None;
    for (optimization, invocation) in [(0_u8, c_o0), (2_u8, c_o2)] {
        let (invocation, invocation_budget) = consume_invocation(invocation);
        let output = platform::compile_c_tool_prepared(
            &tools.clang,
            stage.authority.held(),
            invocation,
            tools
                .process_arena
                .as_mut()
                .ok_or(PhaseBLocalError::BuilderBudget)?
                .arena_mut()?,
        )
        .map_err(|_| PhaseBLocalError::Compile)?;
        let object = output.into_bytes();
        if optimization == 0 {
            let name = "module_O0.o";
            write_platform_file(run_stage, run_files, name, &object)?;
            drop(object);
            drop(invocation_budget);
        } else {
            retained_object = Some((object, invocation_budget));
        }
    }
    let (object, object_invocation_budget) = retained_object.ok_or(PhaseBLocalError::Compile)?;
    let object = AuthorizedObject::new(object, object_invocation_budget)?;
    object.check()?;
    write_platform_file(stage, publish_files, object_name, object.as_slice())?;

    let (harness, harness_budget) = harness_plan;
    write_platform_file(
        run_stage,
        run_files,
        "__semaprax_native_rust_link.rs",
        harness.as_bytes(),
    )?;
    drop(harness);
    drop(harness_budget);
    consume_link_copy(
        safe_rust,
        publish_files,
        run_stage,
        run_files,
        prepared.generated_rust.as_bytes(),
    )?;
    consume_link_copy(
        private_ffi,
        publish_files,
        run_stage,
        run_files,
        prepared.private_ffi_source.as_bytes(),
    )?;
    consume_link_copy(
        optimized_object,
        publish_files,
        run_stage,
        run_files,
        object.as_slice(),
    )?;
    #[cfg(windows)]
    platform::transition_regular_file_to_external_read_prepared(
        stage.authority.held(),
        publish_files,
        object_name,
    )
    .map_err(|_| PhaseBLocalError::Publication)?;

    let staticlib_name = if cfg!(windows) {
        "semaprax_bridge.lib"
    } else {
        "libsemaprax_bridge.a"
    };
    hook(
        NativeRustBuildPoint::BeforeRustLink,
        &stage.path,
        &run_stage.path,
        output,
    );
    recheck_tracked_files(
        run_files,
        &[
            "__semaprax_native_rust_link.rs",
            "semaprax_native_rust_interop.rs",
            "semaprax_native_rust_interop_ffi.rs",
            "module_O0.o",
            "module_O2.o",
        ],
    )?;
    let (rust, rust_invocation_budget) = consume_invocation(rust);
    let staticlib = platform::compile_rust_tool_prepared(
        &tools.rustc,
        run_stage.authority.held(),
        rust,
        tools
            .process_arena
            .as_mut()
            .ok_or(PhaseBLocalError::BuilderBudget)?
            .arena_mut()?,
    )
    .map_err(|_| PhaseBLocalError::Link)?;
    drop(rust_invocation_budget);
    track_run_file(run_files, staticlib_name, staticlib)?;

    let c_harness = "extern int spxnr1_rust_harness_run(void);int main(void){return spxnr1_rust_harness_run();}\n";
    write_platform_file(
        run_stage,
        run_files,
        "__semaprax_native_rust_main.c",
        c_harness.as_bytes(),
    )?;
    let (c_main, c_main_invocation_budget) = consume_invocation(c_main);
    let harness_object = platform::compile_c_tool_prepared(
        &tools.clang,
        run_stage.authority.held(),
        c_main,
        tools
            .process_arena
            .as_mut()
            .ok_or(PhaseBLocalError::BuilderBudget)?
            .arena_mut()?,
    )
    .map_err(|_| PhaseBLocalError::Compile)?;
    write_platform_file(
        run_stage,
        run_files,
        "__semaprax_native_rust_main.o",
        harness_object.bytes(),
    )?;
    drop(harness_object);
    drop(c_main_invocation_budget);
    #[cfg(windows)]
    for name in ["__semaprax_native_rust_main.o", staticlib_name] {
        platform::transition_regular_file_to_external_read_prepared(
            run_stage.authority.held(),
            run_files,
            name,
        )
        .map_err(|_| PhaseBLocalError::Publication)?;
    }
    for (optimization, link_invocation, run_invocation) in
        [(0_u8, link_o0, run_o0), (2_u8, link_o2, run_o2)]
    {
        let c_object = if optimization == 0 {
            "module_O0.o"
        } else {
            "module_O2.o"
        };
        let executable_name = if cfg!(windows) {
            if optimization == 0 {
                "__semaprax_native_rust_link_O0.exe"
            } else {
                "__semaprax_native_rust_link_O2.exe"
            }
        } else if optimization == 0 {
            "__semaprax_native_rust_link_O0"
        } else {
            "__semaprax_native_rust_link_O2"
        };
        #[cfg(windows)]
        platform::transition_regular_file_to_external_read_prepared(
            run_stage.authority.held(),
            run_files,
            c_object,
        )
        .map_err(|_| PhaseBLocalError::Publication)?;
        hook(
            NativeRustBuildPoint::BeforeExecutableAuthentication,
            &stage.path,
            &run_stage.path,
            output,
        );
        recheck_tracked_files(
            run_files,
            &["__semaprax_native_rust_main.o", c_object, staticlib_name],
        )?;
        let (link_invocation, link_invocation_budget) = consume_invocation(link_invocation);
        let executable = platform::link_tool_prepared(
            &tools.clang,
            tools.linker.as_ref(),
            run_stage.authority.held(),
            link_invocation,
            tools
                .process_arena
                .as_mut()
                .ok_or(PhaseBLocalError::BuilderBudget)?
                .arena_mut()?,
        )
        .map_err(|_| PhaseBLocalError::Link)?;
        drop(link_invocation_budget);
        let executable_file = platform::executable_regular_file(&executable)
            .map_err(|_| PhaseBLocalError::Publication)?;
        track_run_file(run_files, executable_name, executable_file)?;
        hook(
            NativeRustBuildPoint::BeforeExecute,
            &stage.path,
            &run_stage.path,
            output,
        );
        let (run_invocation, run_invocation_budget) = consume_invocation(run_invocation);
        platform::execute_tool_prepared(
            &executable,
            run_stage.authority.held(),
            run_invocation,
            tools
                .process_arena
                .as_mut()
                .ok_or(PhaseBLocalError::BuilderBudget)?
                .arena_mut()?,
        )
        .map_err(|_| PhaseBLocalError::Link)?;
        drop(run_invocation_budget);
    }
    let process_arena = tools
        .process_arena
        .take()
        .ok_or(PhaseBLocalError::BuilderBudget)?;
    if platform::prepared_process_arena_remaining(process_arena.arena()?) != 0 {
        return Err(PhaseBLocalError::BuilderBudget);
    }
    let process_arena_capacity = process_arena.authorized_capacity()?;
    if platform::prepared_process_arena_owned_capacity(process_arena.arena()?)
        != process_arena_capacity
    {
        return Err(PhaseBLocalError::BuilderBudget);
    }
    drop(process_arena);
    hook(
        NativeRustBuildPoint::BeforeObjectRead,
        &stage.path,
        &run_stage.path,
        output,
    );
    recheck_tracked_files(publish_files, &[object_name])?;

    let manifest_file_names = canonical_manifest_file_names();
    let files = [
        (manifest_file_names[0], prepared.descriptor.as_bytes()),
        (manifest_file_names[1], prepared.generated_c.as_bytes()),
        (manifest_file_names[2], object.as_slice()),
        (manifest_file_names[3], prepared.generated_header.as_bytes()),
        (manifest_file_names[4], prepared.generated_rust.as_bytes()),
        (
            manifest_file_names[5],
            prepared.private_ffi_source.as_bytes(),
        ),
    ];
    let manifest = manifest_plan.render(
        prepared,
        &files,
        platform::tool_path(&tools.clang),
        &tools.clang_version,
        &tools.rustc_version,
        &prepared.target.triple,
    )?;
    replay_manifest(manifest.as_str(), prepared, &files, tools)?;
    hook(
        NativeRustBuildPoint::BeforeManifestPublish,
        &stage.path,
        &run_stage.path,
        output,
    );
    recheck_tracked_files(
        publish_files,
        &[
            "descriptor.json",
            "module.c",
            object_name,
            "semaprax_native_rust_interop.h",
            "semaprax_native_rust_interop.rs",
            "semaprax_native_rust_interop_ffi.rs",
        ],
    )?;
    write_platform_file(
        stage,
        publish_files,
        "semaprax.native-rust-interop.json",
        manifest.as_bytes(),
    )?;
    scan_publish_inventory_exact(&mut inventory_exact.0, stage, publish_files)?;
    Ok(BuildStageFacts {
        object_name,
        object,
        manifest,
        inventory_exact,
    })
}

pub(super) fn publish_stage_platform(
    parent: &HeldStage,
    stage: &HeldStage,
    output: &Path,
    prepared: &PreparedNativeRustInterop,
    facts: &mut BuildStageFacts,
    publish_files: &mut PublishDiscardInventory,
    final_publish: &mut platform::PreparedPublishDirectory,
) -> Result<(), PhaseBLocalError> {
    facts.observe_object_authority_for_publish()?;
    parent.recheck_local()?;
    stage.recheck_local()?;
    let mut comparison_scratch = [0_u8; platform::FILE_COMPARE_SCRATCH_BYTES];
    for (name, expected) in [
        ("descriptor.json", prepared.descriptor.as_bytes()),
        ("module.c", prepared.generated_c.as_bytes()),
        (
            "semaprax.native-rust-interop.json",
            facts.manifest.as_bytes(),
        ),
        (
            "semaprax_native_rust_interop.h",
            prepared.generated_header.as_bytes(),
        ),
        (
            "semaprax_native_rust_interop.rs",
            prepared.generated_rust.as_bytes(),
        ),
        (
            "semaprax_native_rust_interop_ffi.rs",
            prepared.private_ffi_source.as_bytes(),
        ),
        (facts.object_name, facts.object.as_slice()),
    ] {
        let held = publish_files
            .file(name)
            .map_err(|_| PhaseBLocalError::Publication)?;
        let matches = platform::compare_exact(held, expected, &mut comparison_scratch)
            .map_err(|_| PhaseBLocalError::Publication)?;
        if !matches {
            return Err(PhaseBLocalError::Publication);
        }
    }
    scan_publish_inventory_exact(&mut facts.inventory_exact.0, stage, publish_files)?;
    let output_name = output.file_name().ok_or(PhaseBLocalError::Publication)?;
    let stage_name = stage
        .discard_name
        .as_ref()
        .ok_or(PhaseBLocalError::Publication)?;
    #[cfg(all(test, debug_assertions))]
    if let Some(point) =
        PHASE_B_PUBLISH_FAILURE.with(|point| (point.get() != 0).then(|| point.get()))
    {
        platform::inject_publish_directory_failure(final_publish, point)
            .map_err(|_| PhaseBLocalError::Publication)?;
    }
    publish_files
        .settle_for_publish()
        .map_err(|_| PhaseBLocalError::Publication)?;
    let publication = platform::publish_directory_new_prepared(
        final_publish,
        parent.authority.held(),
        stage.authority.held(),
        stage_name,
        output_name,
    );
    #[cfg(test)]
    if platform::prepared_publish_directory_remaining(final_publish) == 0 {
        PHASE_B_PUBLISH_CONSUMPTIONS.with(|count| count.set(count.get().saturating_add(1)));
    }
    publication.map_err(|_| PhaseBLocalError::Publication)?;
    Ok(())
}
