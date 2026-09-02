//! Phase-B toolchain authentication: frozen environments, the fixed
//! rustc version parser, the process arena, and prepared invocations.

use super::*;

#[test]
fn phase_b_local_paths_digest_and_stage_names_are_frozen_before_effects() {
    let output = Path::new("phase-b-bundle");
    let digest = format!("sha256:{}", "0".repeat(64));
    let mut pending = PendingBundleFacts::new(output, "module.o", &digest).unwrap();
    pending.bind_manifest_digest(b"manifest\n", false).unwrap();
    let facts = pending.finish();
    assert_eq!(facts.output_directory, output);
    assert_eq!(facts.object_path, output.join("module.o"));
    assert_eq!(facts.descriptor_path, output.join("descriptor.json"));
    assert_eq!(
        facts.manifest_path,
        output.join("semaprax.native-rust-interop.json")
    );
    assert_eq!(
        facts.manifest_digest,
        domain_digest(BUNDLE_DIGEST_DOMAIN, b"manifest\n")
    );

    let parent = Path::new("phase-b-parent");
    let descriptor = "sha256:phase-b-descriptor";
    let mut slot = StageSlot::new(parent, descriptor, "publish").unwrap();
    let name_capacity = slot.name.capacity();
    let path_capacity = slot.path.capacity();
    slot.prepare(parent, 0).unwrap();
    assert_eq!(
        slot.name,
        format!(
            ".semaprax-native-rust-interop-publish-{}-{}-0",
            std::process::id(),
            &full_hash(descriptor)[..16]
        )
    );
    assert_eq!(slot.path, parent.join(&slot.name));
    slot.prepare(parent, 1023).unwrap();
    assert!(slot.name.ends_with("-1023"));
    assert_eq!(slot.name.capacity(), name_capacity);
    assert_eq!(slot.path.capacity(), path_capacity);
}

#[cfg(windows)]
#[test]
fn phase_b_windows_absolute_precarrier_topology_is_cumulatively_bounded() {
    let synthetic_parent = Path::new(r"\\?\C:\semaprax-δ");
    let synthetic_capacity =
        exact_child_path_capacity(synthetic_parent, "artifact.obj".len()).unwrap();
    let synthetic = exact_child_path(synthetic_parent, "artifact.obj", synthetic_capacity).unwrap();
    assert_eq!(synthetic.capacity(), synthetic_capacity);
    assert!(exact_child_path_matches(
        &synthetic,
        synthetic_parent,
        OsStr::new("artifact.obj"),
    ));
    for drive_relative in [Path::new(r"C:"), Path::new(r"\\?\C:")] {
        let capacity = exact_child_path_capacity(drive_relative, "artifact.obj".len()).unwrap();
        assert_eq!(
            capacity,
            drive_relative.as_os_str().len() + "artifact.obj".len(),
        );
        let child = exact_child_path(drive_relative, "artifact.obj", capacity).unwrap();
        assert_eq!(child.capacity(), capacity);
        let child_bytes = child.as_os_str().as_encoded_bytes();
        let parent_bytes = drive_relative.as_os_str().as_encoded_bytes();
        assert_eq!(&child_bytes[..parent_bytes.len()], parent_bytes);
        assert_eq!(&child_bytes[parent_bytes.len()..], b"artifact.obj");
    }

    let ((), synthetic_overflowed, _) =
        crate::bounded_output::with_limit_usage(MAX_BUILDER_BYTES, || {
            let mut slot =
                StageSlot::new(synthetic_parent, "sha256:windows-verbatim-stage", "publish")
                    .unwrap();
            slot.prepare(synthetic_parent, 0).unwrap();
            let allocation = slot.path.as_os_str().as_encoded_bytes().as_ptr();
            let capacity = slot.path.capacity();
            slot.prepare(synthetic_parent, 1023).unwrap();
            assert_eq!(
                slot.path.as_os_str().as_encoded_bytes().as_ptr(),
                allocation,
            );
            assert_eq!(slot.path.capacity(), capacity);
        });
    assert!(!synthetic_overflowed);

    let root = std::fs::canonicalize(std::env::temp_dir())
        .unwrap()
        .join(format!(
            "semaprax-native-rust-precarrier-{}",
            std::process::id()
        ));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir(&root).unwrap();
    let output = root.join("bundle");
    PHASE_B_PREPARED_CARRIER_IDENTITIES.with(|identities| identities.set([0; 7]));

    let ((), overflowed, _) = crate::bounded_output::with_limit_usage(MAX_BUILDER_BYTES, || {
        let (program, canonical_spec) = fixture();
        let prepared = prepare_native_rust_interop_bounded(&program, canonical_spec.as_bytes())
            .unwrap_or_else(|error| {
                panic!(
                    "prepare_native failed with {} bytes remaining: {error:?}",
                    crate::bounded_output::remaining_active().unwrap_or(0),
                )
            });
        let parent = output.parent().unwrap();
        let digest = format!("sha256:{}", "0".repeat(64));
        let pending =
            PendingBundleFacts::new(&output, "module.obj", &digest).unwrap_or_else(|error| {
                panic!(
                    "pending facts failed with {} bytes remaining: {error:?}",
                    crate::bounded_output::remaining_active().unwrap_or(0),
                )
            });
        let publish_slot = StageSlot::new(parent, &prepared.descriptor_digest, "publish")
            .unwrap_or_else(|error| {
                panic!(
                    "publish slot failed with {} bytes remaining: {error:?}",
                    crate::bounded_output::remaining_active().unwrap_or(0),
                )
            });
        let run_slot =
            StageSlot::new(parent, &prepared.descriptor_digest, "run").unwrap_or_else(|error| {
                panic!(
                    "run slot failed with {} bytes remaining: {error:?}",
                    crate::bounded_output::remaining_active().unwrap_or(0),
                )
            });
        let publish_files = prepare_publish_discard_inventory().unwrap_or_else(|error| {
            panic!(
                "publish inventory failed with {} bytes remaining: {error:?}",
                crate::bounded_output::remaining_active().unwrap_or(0),
            )
        });
        let run_files = prepare_run_discard_inventory().unwrap_or_else(|error| {
            panic!(
                "run inventory failed with {} bytes remaining: {error:?}",
                crate::bounded_output::remaining_active().unwrap_or(0),
            )
        });
        let parent_capacity = parent.as_os_str().as_encoded_bytes().len();
        let parent_budget = reserve_temporary_exact(parent_capacity).unwrap_or_else(|error| {
            panic!(
                "parent reservation failed with {} bytes remaining: {error:?}",
                crate::bounded_output::remaining_active().unwrap_or(0),
            )
        });
        let parent_path = exact_path_copy(parent, parent_capacity).unwrap_or_else(|error| {
            panic!(
                "parent copy failed with {} bytes remaining: {error:?}",
                crate::bounded_output::remaining_active().unwrap_or(0),
            )
        });
        parent_budget
            .retain(parent_capacity)
            .unwrap_or_else(|error| {
                panic!(
                    "parent retain failed with {} bytes remaining: {error:?}",
                    crate::bounded_output::remaining_active().unwrap_or(0),
                )
            });
        let carriers = PhaseBErrorCarriers::prepare().unwrap_or_else(|error| {
            panic!(
                "carrier preparation failed with {} bytes remaining: {error:?}",
                crate::bounded_output::remaining_active().unwrap_or(0),
            )
        });
        let identities = PHASE_B_PREPARED_CARRIER_IDENTITIES.with(std::cell::Cell::get);
        assert!(identities.into_iter().all(|identity| identity != 0));
        for (index, carrier) in carriers.carriers.iter().enumerate() {
            assert_eq!(
                identities[index],
                carrier.errors.as_ref().unwrap()[0].message.as_ptr() as usize,
            );
        }
        drop((
            carriers,
            parent_path,
            run_files,
            publish_files,
            run_slot,
            publish_slot,
            pending,
            prepared,
        ));
    });
    assert!(!overflowed);
    std::fs::remove_dir_all(&root).unwrap();
}

#[test]
fn phase_b_rejects_non_component_output_before_build_hooks() {
    let (program, spec) = fixture();
    let mut hooks = 0usize;
    let error = match build_native_rust_interop_bundle_with_hook(
        &program,
        spec.as_bytes(),
        Path::new("/"),
        |_, _, _, _| hooks += 1,
    ) {
        Ok(_) => panic!("non-component output unexpectedly reached Phase B"),
        Err(error) => error,
    };
    assert_eq!(hooks, 0);
    assert_eq!(error.len(), 1);
    assert_eq!(error[0].code, "SPX-I232");
    assert_eq!(
        error[0].message,
        "Native Rust Interop output publication failed"
    );
}

#[test]
fn phase_b_output_exists_precedes_invalid_tool_environment_without_tool_activity() {
    let (program, spec) = fixture();
    let root = std::fs::canonicalize(std::env::temp_dir())
        .unwrap()
        .join(format!(
            "semaprax-native-rust-output-precedes-tools-{}",
            std::process::id()
        ));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir(&root).unwrap();
    let output = root.join("bundle");
    std::fs::create_dir(&output).unwrap();
    std::fs::write(output.join("sentinel"), b"foreign").unwrap();
    PHASE_B_OUTPUT_PROBES.with(|count| count.set(0));
    PHASE_B_TOOL_HOLDS.with(|count| count.set(0));
    PHASE_B_TOOL_PROCESSES.with(|count| count.set(0));
    PHASE_B_INVALID_TOOL_ENV_INJECTION.with(|injection| injection.set(true));
    let mut hooks = 0usize;
    let result = build_native_rust_interop_bundle_with_hook(
        &program,
        spec.as_bytes(),
        &output,
        |_, _, _, _| hooks += 1,
    );
    PHASE_B_INVALID_TOOL_ENV_INJECTION.with(|injection| injection.set(false));
    let error = match result {
        Ok(_) => panic!("existing output unexpectedly allowed the invalid tool environment"),
        Err(error) => error,
    };
    assert_eq!(error.len(), 1);
    assert_eq!(error[0].code, "SPX-I232");
    assert_eq!(error[0].message, PHASE_B_PUBLICATION_MESSAGE);
    assert_eq!(
        error[0].message.as_ptr() as usize,
        PHASE_B_PREPARED_CARRIER_IDENTITIES.with(std::cell::Cell::get)
            [PhaseBLocalError::Publication.index()],
    );
    assert_eq!(PHASE_B_OUTPUT_PROBES.with(std::cell::Cell::get), 1);
    assert_eq!(PHASE_B_TOOL_HOLDS.with(std::cell::Cell::get), 0);
    assert_eq!(PHASE_B_TOOL_PROCESSES.with(std::cell::Cell::get), 0);
    assert_eq!(hooks, 0);
    assert_eq!(std::fs::read(output.join("sentinel")).unwrap(), b"foreign");
    std::fs::remove_dir_all(&root).unwrap();
}

#[test]
fn phase_b_invalid_frozen_tool_environment_fails_after_stages_before_hold_or_spawn() {
    let (program, spec) = fixture();
    let root = std::fs::canonicalize(std::env::temp_dir())
        .unwrap()
        .join(format!(
            "semaprax-native-rust-invalid-frozen-tools-{}",
            std::process::id()
        ));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir(&root).unwrap();
    PHASE_B_DISCARD_ATTEMPTS.with(|attempts| attempts.set(0));
    PHASE_B_OUTPUT_PROBES.with(|count| count.set(0));
    PHASE_B_TOOL_HOLDS.with(|count| count.set(0));
    PHASE_B_TOOL_PROCESSES.with(|count| count.set(0));
    PHASE_B_INVALID_TOOL_ENV_INJECTION.with(|injection| injection.set(true));
    let mut hooks = 0usize;
    let result = build_native_rust_interop_bundle_with_hook(
        &program,
        spec.as_bytes(),
        &root.join("bundle"),
        |_, _, _, _| hooks += 1,
    );
    PHASE_B_INVALID_TOOL_ENV_INJECTION.with(|injection| injection.set(false));
    let error = match result {
        Ok(_) => panic!("invalid frozen tool environment unexpectedly authenticated"),
        Err(error) => error,
    };
    assert_eq!(error.len(), 1);
    assert_eq!(error[0].code, "SPX-B110");
    assert_eq!(error[0].message, PHASE_B_UNSUPPORTED_MESSAGE);
    assert_eq!(
        error[0].message.as_ptr() as usize,
        PHASE_B_PREPARED_CARRIER_IDENTITIES.with(std::cell::Cell::get)
            [PhaseBLocalError::Unsupported.index()],
    );
    assert_eq!(PHASE_B_OUTPUT_PROBES.with(std::cell::Cell::get), 1);
    assert_eq!(PHASE_B_TOOL_HOLDS.with(std::cell::Cell::get), 0);
    assert_eq!(PHASE_B_TOOL_PROCESSES.with(std::cell::Cell::get), 0);
    assert_eq!(PHASE_B_DISCARD_ATTEMPTS.with(std::cell::Cell::get), 2);
    assert_eq!(hooks, 0);
    assert_eq!(std::fs::read_dir(&root).unwrap().count(), 0);
    std::fs::remove_dir_all(&root).unwrap();
}

#[test]
fn phase_b_direct_rustc_fixed_point_mismatch_is_b110_before_artifact_processes() {
    let (program, spec) = fixture();
    let root = std::fs::canonicalize(std::env::temp_dir())
        .unwrap()
        .join(format!(
            "semaprax-native-rust-direct-fixed-point-{}",
            std::process::id()
        ));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir(&root).unwrap();
    PHASE_B_DISCARD_ATTEMPTS.with(|attempts| attempts.set(0));
    PHASE_B_TOOL_PROCESSES.with(|count| count.set(0));
    PHASE_B_BUILD_INVOCATION_CONSUMPTIONS.with(|count| count.set(0));
    PHASE_B_DIRECT_SYSROOT_MISMATCH_INJECTION.with(|injection| injection.set(true));
    let mut hooks = 0usize;
    let result = build_native_rust_interop_bundle_with_hook(
        &program,
        spec.as_bytes(),
        &root.join("bundle"),
        |_, _, _, _| hooks += 1,
    );
    PHASE_B_DIRECT_SYSROOT_MISMATCH_INJECTION.with(|injection| injection.set(false));
    let error = match result {
        Ok(_) => panic!("mismatched direct rustc sysroot unexpectedly admitted"),
        Err(error) => error,
    };
    assert_eq!(error.len(), 1);
    assert_eq!(error[0].code, "SPX-B110");
    assert_eq!(error[0].message, PHASE_B_UNSUPPORTED_MESSAGE);
    assert_eq!(PHASE_B_TOOL_PROCESSES.with(std::cell::Cell::get), 2);
    assert_eq!(
        PHASE_B_BUILD_INVOCATION_CONSUMPTIONS.with(std::cell::Cell::get),
        0
    );
    assert_eq!(PHASE_B_DISCARD_ATTEMPTS.with(std::cell::Cell::get), 2);
    assert_eq!(hooks, 0);
    assert_eq!(std::fs::read_dir(&root).unwrap().count(), 0);
    std::fs::remove_dir_all(&root).unwrap();
}

#[test]
fn phase_b_fixed_rustc_version_parser_is_no_growth_at_representative_and_maximum() {
    for source in [
        String::from(
            "rustc 1.88.0 (012345678 2026-01-01)\nbinary: rustc\ncommit-hash: 0123456789abcdef\ncommit-date: 2026-01-01\nhost: x86_64-unknown-linux-gnu\nrelease: 1.88.0\nLLVM version: 20.1.0",
        ),
        format!(
            "rustc 1.88.0 (012345678 2026-01-01)\nbinary: rustc\ncommit-hash: 0123456789abcdef\ncommit-date: 2026-01-01\nhost: x86_64-unknown-linux-gnu\nrelease: 1.88.0\nLLVM version: {}",
            "1".repeat(PHASE_B_TOOL_VERSION_CAPACITY - 200)
        ),
    ] {
        assert!(source.len() <= PHASE_B_TOOL_VERSION_CAPACITY);
        let mut parsed = RustcVersion::prepared().unwrap();
        let capacity = parsed.capacity();
        parse_rustc_version(&source, &mut parsed).unwrap();
        assert_eq!(parsed.capacity(), capacity);
        assert_eq!(parsed.release(), "1.88.0");
        assert_eq!(parsed.commit_hash(), "0123456789abcdef");
        assert_eq!(parsed.host(), "x86_64-unknown-linux-gnu");
    }

    let exact_first = "r".repeat(PHASE_B_TOOL_VERSION_CAPACITY - 3);
    let mut exact = RustcVersion::prepared().unwrap();
    let exact_pointer = exact.storage.as_ptr();
    exact.store([&exact_first, "c", "h", "l"]).unwrap();
    assert_eq!(exact.storage.len(), PHASE_B_TOOL_VERSION_CAPACITY);
    assert_eq!(exact.storage.capacity(), PHASE_B_TOOL_VERSION_CAPACITY);
    assert_eq!(exact.storage.as_ptr(), exact_pointer);

    let overflow_first = "r".repeat(PHASE_B_TOOL_VERSION_CAPACITY - 2);
    let mut overflow = RustcVersion::prepared().unwrap();
    let overflow_pointer = overflow.storage.as_ptr();
    assert_eq!(
        overflow.store([&overflow_first, "c", "h", "l"]),
        Err(PhaseBLocalError::Unsupported),
    );
    assert!(overflow.storage.is_empty());
    assert_eq!(overflow.boundaries, [0; 5]);
    assert_eq!(overflow.storage.capacity(), PHASE_B_TOOL_VERSION_CAPACITY);
    assert_eq!(overflow.storage.as_ptr(), overflow_pointer);

    for invalid in [
        "rustc 1.88.0\nrelease: 1.88.0\nrelease: 1.88.0\ncommit-hash: 0123456\nhost: h\nLLVM version: 1",
        "rustc 1.88.0\nrelease: 1.88.0\ncommit-hash: 0123456\nhost: h\nunknown: value\nLLVM version: 1",
        "rustc 1.88.0\nrelease: 1.88.0\ncommit-hash: 0123456\nhost: h",
    ] {
        let mut parsed = RustcVersion::prepared().unwrap();
        assert_eq!(
            parse_rustc_version(invalid, &mut parsed),
            Err(PhaseBLocalError::Unsupported),
        );
    }
}

#[test]
fn phase_b_process_arena_reservation_precedes_materialization_source_contract() {
    let source = IMPLEMENTATION_SOURCE;
    let toolchain_start = source.find("fn prepare_toolchain_plan()").unwrap();
    let toolchain_end = source[toolchain_start..]
        .find("fn authenticate_toolchain(")
        .map(|offset| toolchain_start + offset)
        .unwrap();
    let toolchain = &source[toolchain_start..toolchain_end];
    let arena = toolchain.find("prepare_process_arena_authorized(").unwrap();
    let include_drop = toolchain.find("drop(include)").unwrap();
    let libraries_drop = toolchain.find("drop(libraries)").unwrap();
    let environment_shrink = toolchain
        .find("shrink_phase_b(&mut environment.budget")
        .unwrap();
    let resolver_reservation = toolchain
        .find("reserve_phase_b(PHASE_B_TOOL_RESOLVER_CAPACITY)")
        .unwrap();
    assert!(
        arena < include_drop
            && include_drop < libraries_drop
            && libraries_drop < environment_shrink
            && environment_shrink < resolver_reservation
    );

    let start = source.find("fn prepare_process_arena_authorized(").unwrap();
    let end = source[start..]
        .find("#[cfg(test)]\npub(super) fn reset_phase_b_error_materialization_observer")
        .map(|offset| start + offset)
        .unwrap();
    let helper = &source[start..end];
    let plan = helper
        .find("platform::prepare_process_arena_plan_with_environment(")
        .unwrap();
    let required = helper
        .find("platform::prepared_process_arena_plan_capacity(&plan)")
        .unwrap();
    let reserve = helper.find("reserve_phase_b(required)?").unwrap();
    let allocate = helper
        .find("platform::materialize_process_arena_with_environment(")
        .unwrap();
    assert!(plan < required && required < reserve && reserve < allocate);
    assert!(helper.contains("required > PHASE_B_PROCESS_ARENA_MAX_CAPACITY"));
    assert!(helper.contains("platform::prepared_process_arena_owned_capacity(&arena) != required"));

    let wrapper_start = source.find("struct AuthorizedProcessArena {").unwrap();
    let wrapper_end = source[wrapper_start..]
        .find("struct PreparedToolchainPlan {")
        .map(|offset| wrapper_start + offset)
        .unwrap();
    let wrapper = &source[wrapper_start..wrapper_end];
    assert!(wrapper.find("arena:").unwrap() < wrapper.find("budget:").unwrap());
    assert!(wrapper.find("drop(arena)").unwrap() < wrapper.find("drop(budget)").unwrap());
    assert!(
        !source[source.find("struct PreparedToolchainPlan {").unwrap()..]
            .split("struct ToolchainFacts {")
            .next()
            .unwrap()
            .contains("process_arena_budget")
    );
}

#[test]
fn windows_direct_rustc_tests_use_the_frozen_native_linker() {
    let source = IMPLEMENTATION_SOURCE;
    let tests = TESTS_SOURCE;
    let linker = source.find("fn bind_test_rust_linker(").unwrap();
    let linker_end = source[linker..]
        .find("\n}\n\npub(super) struct RustcVersion")
        .map(|offset| linker + offset)
        .unwrap();
    let helper = &source[linker..linker_end];
    assert!(helper.contains("std::env::var_os(\"SEMAPRAX_LINKER\")"));
    assert!(helper.contains("std::fs::canonicalize(configured)"));
    let callsites = tests
        .match_indices(
            "fn phase_b_process_arena_drops_bytes_before_authority_on_early_plan_failure(",
        )
        .nth(1)
        .map(|(offset, _)| offset)
        .unwrap();
    assert_eq!(
        tests[callsites..]
            .matches("bind_test_rust_linker(&mut ")
            .count(),
        5
    );
    assert!(!source.contains("format!(\"linker={}\", clang.path.display())"));
}

#[test]
fn phase_b_process_arena_drops_bytes_before_authority_on_early_plan_failure() {
    reset_phase_b_process_arena_drop_observer();
    let (plan, overflowed) =
        crate::bounded_output::with_limit(MAX_BUILDER_BYTES, prepare_toolchain_plan);
    assert!(!overflowed);
    let plan = plan.unwrap();
    assert_eq!(PHASE_B_PROCESS_ARENA_DROPS.with(std::cell::Cell::get), 0);
    assert_eq!(
        PHASE_B_PROCESS_ARENA_BUDGET_DROPS.with(std::cell::Cell::get),
        0
    );
    drop(plan);
    assert_eq!(PHASE_B_PROCESS_ARENA_DROPS.with(std::cell::Cell::get), 1);
    assert_eq!(
        PHASE_B_PROCESS_ARENA_BUDGET_DROPS.with(std::cell::Cell::get),
        1
    );
    assert_eq!(
        PHASE_B_PROCESS_ARENA_DROP_ORDER.with(std::cell::Cell::get),
        [1, 2]
    );
    assert_eq!(
        PHASE_B_PROCESS_ARENA_DROP_ORDER_LENGTH.with(std::cell::Cell::get),
        2
    );
}

#[cfg(windows)]
#[test]
fn phase_b_process_arena_exact_and_one_less_is_zero_effect() {
    let include = OsStr::new(r"C:\sdk\include");
    let libraries = OsStr::new(r"C:\sdk\lib");
    let sizing = platform::prepare_process_arena_plan_with_environment(
        PHASE_B_PROCESS_INVOCATIONS,
        Some(include),
        Some(libraries),
    )
    .unwrap();
    let required = platform::prepared_process_arena_plan_capacity(&sizing);
    assert!(required > 0 && required <= PHASE_B_PROCESS_ARENA_MAX_CAPACITY);

    let (exact, overflowed, used) = crate::bounded_output::with_limit_usage(required, || {
        prepare_process_arena_authorized(Some(include), Some(libraries))
    });
    assert!(!overflowed);
    let arena = exact.unwrap();
    assert_eq!(used, required);
    assert_eq!(arena.authorized_capacity().unwrap(), required);
    assert_eq!(
        platform::prepared_process_arena_owned_capacity(arena.arena().unwrap()),
        required
    );
    assert_eq!(
        platform::prepared_process_arena_remaining(arena.arena().unwrap()),
        PHASE_B_PROCESS_INVOCATIONS
    );
    drop(arena);

    PHASE_B_OUTPUT_PROBES.with(|count| count.set(0));
    PHASE_B_TOOL_HOLDS.with(|count| count.set(0));
    PHASE_B_TOOL_PROCESSES.with(|count| count.set(0));
    reset_phase_b_error_materialization_observer();
    let (one_less, overflowed) = crate::bounded_output::with_limit(required - 1, || {
        prepare_process_arena_authorized(Some(include), Some(libraries))
    });
    assert!(!overflowed);
    assert!(matches!(one_less, Err(PhaseBLocalError::BuilderBudget)));
    assert_eq!(PHASE_B_OUTPUT_PROBES.with(std::cell::Cell::get), 0);
    assert_eq!(PHASE_B_TOOL_HOLDS.with(std::cell::Cell::get), 0);
    assert_eq!(PHASE_B_TOOL_PROCESSES.with(std::cell::Cell::get), 0);
    assert_eq!(
        PHASE_B_POST_EFFECT_ERROR_MATERIALIZATIONS.with(std::cell::Cell::get),
        0
    );
    assert!(!PHASE_B_EFFECT_STARTED.with(std::cell::Cell::get));
}

#[test]
fn phase_b_harness_is_exact_capacity_at_representative_and_maximum_and_one_less_is_pre_effect() {
    let (program, spec) = fixture();
    let prepared = prepare_native_rust_interop(&program, spec.as_bytes()).unwrap();
    let (representative, representative_budget) = prepare_rust_harness(&prepared).unwrap();
    assert_eq!(representative.len(), representative.capacity());
    assert_eq!(representative.len(), representative_budget.maximum());
    drop((representative, representative_budget));

    let mut maximum = prepared;
    let mut import = maximum.imports[0].clone();
    import.parameters = (0..MAX_PARAMETERS)
        .map(|index| ParameterFact {
            name: format!("p{index}"),
            ty: ScalarType::I64,
        })
        .collect();
    import.capabilities = (0..MAX_EFFECTS)
        .map(|index| format!("capability.{index:02}"))
        .collect();
    maximum.imports = vec![import; MAX_IMPORTS];
    let mut export = maximum.exports[0].clone();
    export.parameters = (0..MAX_PARAMETERS)
        .map(|index| ParameterFact {
            name: format!("p{index}"),
            ty: ScalarType::I64,
        })
        .collect();
    maximum.exports = vec![export; MAX_EXPORTS];
    let (harness, overflowed, exact) =
        crate::bounded_output::with_limit_usage(MAX_BUILDER_BYTES, || {
            prepare_rust_harness(&maximum)
        });
    assert!(!overflowed);
    let (harness, budget) = harness.unwrap();
    assert_eq!(harness.len(), harness.capacity());
    assert_eq!(harness.len(), budget.maximum());
    assert_eq!(exact, harness.len());
    drop((harness, budget));

    PHASE_B_OUTPUT_PROBES.with(|count| count.set(0));
    PHASE_B_TOOL_HOLDS.with(|count| count.set(0));
    PHASE_B_TOOL_PROCESSES.with(|count| count.set(0));
    let (one_less, overflowed) =
        crate::bounded_output::with_limit(exact - 1, || prepare_rust_harness(&maximum));
    assert!(!overflowed);
    assert!(matches!(one_less, Err(PhaseBLocalError::BuilderBudget)));
    assert_eq!(PHASE_B_OUTPUT_PROBES.with(std::cell::Cell::get), 0);
    assert_eq!(PHASE_B_TOOL_HOLDS.with(std::cell::Cell::get), 0);
    assert_eq!(PHASE_B_TOOL_PROCESSES.with(std::cell::Cell::get), 0);
}

#[test]
fn phase_b_prepared_invocations_admit_linux_underscore_and_reject_other_punctuation_as_b110() {
    let mut prepared = with_test_target(
        Target {
            triple: "x86_64-unknown-linux-gnu".to_owned(),
            pointer_width: 64,
            endian: "little".to_owned(),
            panic_strategy: "unwind".to_owned(),
            thread_policy: "same_thread".to_owned(),
        },
        || {
            let (program, spec) = fixture();
            prepare_native_rust_interop(&program, spec.as_bytes()).unwrap()
        },
    );
    PHASE_B_BUILD_INVOCATION_PLANS.with(|count| count.set(0));
    let linker = std::env::var_os("SEMAPRAX_LINKER");
    let linker = if cfg!(windows) {
        Some(
            linker
                .as_deref()
                .unwrap_or_else(|| OsStr::new(PHASE_B_MISSING_WINDOWS_LINKER)),
        )
    } else {
        None
    };
    let vctools = std::env::var_os("SEMAPRAX_VCTOOLS");
    let vctools = if cfg!(windows) {
        Some(
            vctools
                .as_deref()
                .unwrap_or_else(|| OsStr::new(PHASE_B_MISSING_WINDOWS_VCTOOLS)),
        )
    } else {
        None
    };
    let plans = prepare_build_invocations(&prepared, false, linker, vctools).unwrap();
    assert_eq!(PHASE_B_BUILD_INVOCATION_PLANS.with(std::cell::Cell::get), 8);
    drop(plans);

    prepared.target.triple = "x86_64-unknown/linux-gnu".to_owned();
    let error = match prepare_build_invocations(&prepared, false, linker, vctools) {
        Ok(_) => panic!("noncanonical target punctuation was admitted"),
        Err(error) => error,
    };
    assert_eq!(error, PhaseBLocalError::Unsupported);
}

#[test]
fn phase_b_all_eight_build_invocations_are_prepared_bounded_and_consumed_once() {
    let (program, spec) = fixture();
    let prepared = prepare_native_rust_interop(&program, spec.as_bytes()).unwrap();
    PHASE_B_BUILD_INVOCATION_PLANS.with(|count| count.set(0));
    PHASE_B_BUILD_INVOCATION_CONSUMPTIONS.with(|count| count.set(0));
    PHASE_B_LINK_COPY_PLANS.with(|count| count.set(0));
    PHASE_B_LINK_COPY_CONSUMPTIONS.with(|count| count.set(0));
    PHASE_B_INVENTORY_EXACT_PLANS.with(|count| count.set(0));
    PHASE_B_INVENTORY_EXACT_SCANS.with(|count| count.set(0));
    PHASE_B_PUBLISH_PLANS.with(|count| count.set(0));
    PHASE_B_PUBLISH_CONSUMPTIONS.with(|count| count.set(0));
    reset_phase_b_object_authority_observer();
    reset_phase_b_manifest_authority_observer();
    let linker = std::env::var_os("SEMAPRAX_LINKER");
    let linker = if cfg!(windows) {
        Some(
            linker
                .as_deref()
                .unwrap_or_else(|| OsStr::new(PHASE_B_MISSING_WINDOWS_LINKER)),
        )
    } else {
        None
    };
    let vctools = std::env::var_os("SEMAPRAX_VCTOOLS");
    let vctools = if cfg!(windows) {
        Some(
            vctools
                .as_deref()
                .unwrap_or_else(|| OsStr::new(PHASE_B_MISSING_WINDOWS_VCTOOLS)),
        )
    } else {
        None
    };
    let plans = prepare_build_invocations(&prepared, false, linker, vctools).unwrap();
    assert_eq!(PHASE_B_BUILD_INVOCATION_PLANS.with(std::cell::Cell::get), 8);
    assert_eq!(
        platform::prepared_c_compile_owned_capacity(&plans.c_o0.0),
        plans.c_o0.1.maximum()
    );
    assert_eq!(
        platform::prepared_c_compile_owned_capacity(&plans.c_o2.0),
        plans.c_o2.1.maximum()
    );
    assert_eq!(
        platform::prepared_rust_compile_owned_capacity(&plans.rust.0),
        plans.rust.1.maximum()
    );
    assert_eq!(
        platform::prepared_c_compile_owned_capacity(&plans.c_main.0),
        plans.c_main.1.maximum()
    );
    assert_eq!(
        platform::prepared_link_owned_capacity(&plans.link_o0.0),
        plans.link_o0.1.maximum()
    );
    assert_eq!(
        platform::prepared_run_owned_capacity(&plans.run_o0.0),
        plans.run_o0.1.maximum()
    );
    assert_eq!(
        platform::prepared_link_owned_capacity(&plans.link_o2.0),
        plans.link_o2.1.maximum()
    );
    assert_eq!(
        platform::prepared_run_owned_capacity(&plans.run_o2.0),
        plans.run_o2.1.maximum()
    );
    drop(plans);

    let publish = prepare_publish_discard_inventory().unwrap();
    let run = prepare_run_discard_inventory().unwrap();
    let copies = prepare_link_copies(
        &publish,
        &run,
        if cfg!(windows) {
            "module.obj"
        } else {
            "module.o"
        },
    )
    .unwrap();
    assert_eq!(PHASE_B_LINK_COPY_PLANS.with(std::cell::Cell::get), 3);
    for (prepared, budget) in [
        &copies.safe_rust,
        &copies.private_ffi,
        &copies.optimized_object,
    ] {
        assert_eq!(
            platform::prepared_link_or_copy_owned_capacity(prepared),
            budget.maximum()
        );
    }
    drop((copies, publish, run));

    let root = std::fs::canonicalize(std::env::temp_dir())
        .unwrap()
        .join(format!(
            "semaprax-native-rust-prepared-build-plans-{}",
            std::process::id()
        ));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir(&root).unwrap();
    PHASE_B_BUILD_INVOCATION_PLANS.with(|count| count.set(0));
    PHASE_B_BUILD_INVOCATION_CONSUMPTIONS.with(|count| count.set(0));
    PHASE_B_LINK_COPY_PLANS.with(|count| count.set(0));
    PHASE_B_LINK_COPY_CONSUMPTIONS.with(|count| count.set(0));
    build_native_rust_interop_bundle_with_hook(
        &program,
        spec.as_bytes(),
        &root.join("bundle"),
        |_, _, _, _| {},
    )
    .unwrap_or_else(|errors| {
        panic!(
            "prepared build failed after {} invocation consumptions: {errors:?}",
            PHASE_B_BUILD_INVOCATION_CONSUMPTIONS.with(std::cell::Cell::get),
        )
    });
    assert_eq!(PHASE_B_BUILD_INVOCATION_PLANS.with(std::cell::Cell::get), 8);
    assert_eq!(
        PHASE_B_BUILD_INVOCATION_CONSUMPTIONS.with(std::cell::Cell::get),
        8
    );
    assert_eq!(PHASE_B_LINK_COPY_PLANS.with(std::cell::Cell::get), 3);
    assert_eq!(PHASE_B_LINK_COPY_CONSUMPTIONS.with(std::cell::Cell::get), 3);
    assert_eq!(PHASE_B_INVENTORY_EXACT_PLANS.with(std::cell::Cell::get), 1);
    assert_eq!(PHASE_B_INVENTORY_EXACT_SCANS.with(std::cell::Cell::get), 2);
    assert_eq!(PHASE_B_PUBLISH_PLANS.with(std::cell::Cell::get), 1);
    assert_eq!(PHASE_B_PUBLISH_CONSUMPTIONS.with(std::cell::Cell::get), 1);
    assert_eq!(
        PHASE_B_POST_EFFECT_ERROR_MATERIALIZATIONS.with(std::cell::Cell::get),
        0
    );
    assert_eq!(
        PHASE_B_OBJECT_AUTHORITY_TRANSFERS.with(std::cell::Cell::get),
        1
    );
    assert_eq!(PHASE_B_OBJECT_AUTHORITY_DROPS.with(std::cell::Cell::get), 1);
    assert_eq!(
        PHASE_B_OBJECT_AUTHORITY_MANIFEST_OBSERVATIONS.with(std::cell::Cell::get),
        1
    );
    assert_eq!(
        PHASE_B_OBJECT_AUTHORITY_PUBLISH_OBSERVATIONS.with(std::cell::Cell::get),
        1
    );
    assert_phase_b_object_drop_order(1);
    assert_eq!(
        PHASE_B_MANIFEST_ARENA_ALLOCATIONS.with(std::cell::Cell::get),
        1
    );
    assert_eq!(PHASE_B_MANIFEST_ARENA_GROWTHS.with(std::cell::Cell::get), 0);
    assert_eq!(
        PHASE_B_MANIFEST_AUTHORITY_TRANSFERS.with(std::cell::Cell::get),
        1
    );
    assert_phase_b_manifest_drop_order(1);
    std::fs::remove_dir_all(&root).unwrap();
}

#[cfg(unix)]
#[test]
fn phase_b_prepared_tool_resolver_covers_path_positions_symlink_and_capacity_minus_one() {
    use std::os::unix::fs::symlink;

    let real = configured_tool("CLANG").unwrap().path;
    let canonical = std::fs::canonicalize(&real).unwrap();
    let canonical_text = canonical.to_str().unwrap();
    let root = std::fs::canonicalize(std::env::temp_dir())
        .unwrap()
        .join(format!(
            "semaprax-native-rust-prepared-tool-resolver-{}",
            std::process::id()
        ));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir(&root).unwrap();
    let directories = [root.join("first"), root.join("middle"), root.join("last")];
    for directory in &directories {
        std::fs::create_dir(directory).unwrap();
    }

    for position in 0..directories.len() {
        let link = directories[position].join("clang");
        symlink(&real, &link).unwrap();
        let paths = std::env::join_paths(&directories).unwrap();
        let resolver =
            platform::prepare_tool_resolver("clang", PHASE_B_TOOL_PATH_CAPACITY).unwrap();
        assert!(
            platform::prepared_tool_resolver_owned_capacity(&resolver)
                <= PHASE_B_TOOL_RESOLVER_CAPACITY
        );
        reset_phase_b_error_materialization_observer();
        mark_phase_b_effect_started();
        let held =
            platform::resolve_and_hold_tool_prepared(resolver, None, Some(paths.as_os_str()))
                .unwrap();
        assert_eq!(platform::tool_path(&held), canonical_text);
        assert_eq!(
            platform::tool_path_capacity(&held),
            PHASE_B_TOOL_PATH_CAPACITY
        );
        assert_eq!(
            PHASE_B_POST_EFFECT_ERROR_MATERIALIZATIONS.with(std::cell::Cell::get),
            0,
        );
        std::fs::remove_file(link).unwrap();
    }

    let configured_link = root.join("configured-clang");
    symlink(&real, &configured_link).unwrap();
    let resolver = platform::prepare_tool_resolver("clang", PHASE_B_TOOL_PATH_CAPACITY).unwrap();
    reset_phase_b_error_materialization_observer();
    mark_phase_b_effect_started();
    let held =
        platform::resolve_and_hold_tool_prepared(resolver, Some(configured_link.as_os_str()), None)
            .unwrap();
    assert_eq!(platform::tool_path(&held), canonical_text);
    assert_eq!(
        PHASE_B_POST_EFFECT_ERROR_MATERIALIZATIONS.with(std::cell::Cell::get),
        0,
    );

    let missing_paths = std::env::join_paths(&directories).unwrap();
    let resolver = platform::prepare_tool_resolver("clang", PHASE_B_TOOL_PATH_CAPACITY).unwrap();
    reset_phase_b_error_materialization_observer();
    mark_phase_b_effect_started();
    assert!(platform::resolve_and_hold_tool_prepared(
        resolver,
        None,
        Some(missing_paths.as_os_str()),
    )
    .is_err());
    assert_eq!(
        PHASE_B_POST_EFFECT_ERROR_MATERIALIZATIONS.with(std::cell::Cell::get),
        0,
    );

    let exact = canonical.as_os_str().as_encoded_bytes().len() + 1;
    let resolver = platform::prepare_tool_resolver("clang", exact).unwrap();
    let held =
        platform::resolve_and_hold_tool_prepared(resolver, Some(canonical.as_os_str()), None)
            .unwrap();
    assert_eq!(platform::tool_path(&held), canonical_text);
    assert_eq!(platform::tool_path_capacity(&held), exact);
    let resolver = platform::prepare_tool_resolver("clang", exact - 1).unwrap();
    reset_phase_b_error_materialization_observer();
    mark_phase_b_effect_started();
    assert!(
        platform::resolve_and_hold_tool_prepared(resolver, Some(canonical.as_os_str()), None,)
            .is_err()
    );
    assert_eq!(
        PHASE_B_POST_EFFECT_ERROR_MATERIALIZATIONS.with(std::cell::Cell::get),
        0,
    );
    std::fs::remove_dir_all(&root).unwrap();
}
