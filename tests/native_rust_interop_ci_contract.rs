use std::fs;
use std::path::Path;
use std::process::Command;

fn read(relative: &str) -> String {
    fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join(relative))
        .unwrap_or_else(|error| panic!("read {relative}: {error}"))
}

fn assert_contains_all(label: &str, source: &str, required: &[&str]) {
    for value in required {
        assert!(source.contains(value), "{label} is missing `{value}`");
    }
}

#[test]
fn private_native_rust_interop_crates_are_unpublished_and_quarantined() {
    let root_manifest = read("Cargo.toml");
    assert_contains_all(
        "workspace membership",
        &root_manifest,
        &[
            "crates/semaprax-native-rust-interop-builder",
            "crates/semaprax-native-rust-interop-platform-sys",
            "crates/semaprax-native-rust-interop-platform",
            "default-members = [\".\"]",
        ],
    );

    for manifest in [
        "crates/semaprax-native-rust-interop-builder/Cargo.toml",
        "crates/semaprax-native-rust-interop-platform/Cargo.toml",
        "crates/semaprax-native-rust-interop-platform-sys/Cargo.toml",
    ] {
        let source = read(manifest);
        assert!(
            source.contains("publish = false"),
            "{manifest} became publishable"
        );
        for forbidden in ["libloading", "dlopen", "dlsym", "LoadLibrary"] {
            assert!(
                !source.contains(forbidden),
                "{manifest} admitted dynamic loading through `{forbidden}`"
            );
        }
    }

    let root_lib = read("src/lib.rs");
    let root_main = read("src/main.rs");
    assert!(!root_lib.contains("native_rust_interop"));
    assert!(!root_main.contains("native-rust-interop"));

    let builder = read("crates/semaprax-native-rust-interop-builder/src/lib.rs");
    let platform = read("crates/semaprax-native-rust-interop-platform/src/lib.rs");
    let sys = read("crates/semaprax-native-rust-interop-platform-sys/src/lib.rs");
    assert!(builder.contains("#![forbid(unsafe_code)]"));
    assert!(platform.contains("#![forbid(unsafe_code)]"));
    assert!(sys.contains("#![deny(unsafe_op_in_unsafe_fn)]"));
    for forbidden in [
        "LoadLibraryA(",
        "LoadLibraryW(",
        "GetProcAddress(",
        "FreeLibrary(",
        "dlopen(",
        "dlsym(",
        "dlclose(",
    ] {
        assert!(
            !platform.contains(forbidden) && !sys.contains(forbidden),
            "private platform source admitted dynamic loading through `{forbidden}`"
        );
    }
}

#[test]
fn private_native_rust_interop_nonclaims_are_the_frozen_ordered_set() {
    let implementation = read("crates/semaprax-native-rust-interop-builder/src/implementation.rs");
    let nonclaims = implementation
        .split("const NONCLAIMS: &[&str] = &[")
        .nth(1)
        .and_then(|tail| tail.split("];\n").next())
        .expect("private nonclaim constant");
    let actual = nonclaims
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .map(|line| {
            line.strip_prefix('"')
                .and_then(|line| line.strip_suffix("\","))
                .expect("canonical nonclaim literal")
        })
        .collect::<Vec<_>>();
    assert_eq!(
        actual,
        [
            "no_resource_owned_borrow_shared_or_aggregate_abi",
            "no_pointer_reference_slice_string_trait_object_or_generic_abi",
            "no_cross_boundary_allocator_or_deallocator",
            "no_wasm_component_or_canonical_abi_detour",
            "no_dynamic_loading_symbol_lookup_unload_or_hot_reload",
            "no_public_execution_or_spx_b104_change_in_private_ab",
            "no_callable_v2_v3_proof_bundle_or_loader_wire_change",
            "no_graph_schema_api_kat_or_semantic_projection_change",
            "no_agent_runtime_economic_workspace_or_patch_wire_change",
            "no_untrusted_native_code_sandbox_or_memory_safety",
            "no_same_uid_process_signal_or_task_port_isolation",
            "no_same_uid_active_filesystem_mutation_or_namespace_race_isolation",
            "no_same_user_process_handle_or_thread_resume_isolation",
            "no_abi_compatibility_outside_exact_descriptor_target_toolchain",
            "no_cross_target_cross_toolchain_or_cross_build_bundle_reuse",
            "no_panic_or_unwind_across_ffi",
            "no_abort_oom_stack_overflow_signal_seh_or_process_crash_recovery",
            "no_power_loss_durability_or_crash_atomicity",
            "no_async_reentrant_parallel_cross_thread_or_send_sync_bridge",
            "no_host_capability_provenance_or_os_authority",
            "no_ambient_effect_capability_or_callback_discovery",
            "no_host_error_text_panic_payload_secret_or_pointer_evidence",
            "no_exactly_once_external_effect",
            "no_exception_cpp_rust_unwind_translation",
            "no_dynamic_library_code_signing_supply_chain_or_linker_provenance",
            "no_dynamic_dependency_identity_or_filesystem_race_isolation",
            "no_c_cpp_objective_c_swift_kotlin_jni_or_other_ecosystem_binding",
            "no_stable_rust_abi_claim_beyond_generated_c_abi_wrapper",
            "no_public_cli_package_registry_or_build_script_network",
            "no_general_interop_or_production_readiness",
            "no_completion_matrix_status_promotion",
        ]
    );
}

#[test]
fn public_semaprax_package_excludes_private_interop_crate_sources() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let output = Command::new(std::env::var_os("CARGO").unwrap_or_else(|| "cargo".into()))
        .current_dir(root)
        .args([
            "package",
            "--locked",
            "--allow-dirty",
            "-p",
            "semaprax",
            "--list",
        ])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "cargo package --list failed without exposing output bytes"
    );
    let inventory = String::from_utf8(output.stdout).unwrap();
    assert!(!inventory.contains("crates/semaprax-native-rust-interop"));
    assert!(!inventory.contains("src/native_rust_interop.rs"));
}

#[test]
fn private_builder_uses_held_platform_authority_for_every_physical_step() {
    let implementation = read("crates/semaprax-native-rust-interop-builder/src/implementation.rs");
    assert_contains_all(
        "named private replay and hostile evidence",
        &implementation,
        &[
            "fn private_a_is_canonical_and_pure()",
            "fn source_descriptor_and_generated_views_reconstruct_from_authenticated_facts()",
            "fn descriptor_and_generated_source_replay_reject_every_bound_family()",
            "fn exact_replayers_reject_every_generated_and_descriptor_byte_substitution()",
            "fn six_output_artifact_known_answer_vectors_are_frozen()",
            "fn cumulative_builder_limit_is_exact_and_cannot_be_widened()",
            "fn private_b_builds_exact_static_inventory_without_clobber()",
            "fn build_race_hooks_reject_each_pre_effect_mutation_and_preserve_foreign_bytes()",
            "fn linked_bridge_round_trips_rust_to_semaprax_to_rust_and_closes_failures()",
            "fn bool_and_infallible_import_abi_is_exact_at_o0_and_o2()",
        ],
    );
    let physical_tail = implementation
        .split("fn platform_publication_error")
        .nth(1)
        .expect("physical production implementation");
    let physical = physical_tail
        .split("#[cfg(test)]\nmod tests")
        .next()
        .expect("active production implementation prefix");

    assert_contains_all(
        "held platform authority",
        &implementation,
        &[
            "platform::create_directory_new_prepared",
            "platform::write_file_new_prepared",
            "platform::compile_c_tool_prepared",
            "platform::compile_rust_tool_prepared",
            "platform::link_tool_prepared",
            "platform::execute_tool_prepared",
            "platform::compare_exact",
            "platform::inventory_exact_prepared",
            "platform::publish_directory_new_prepared",
            "platform::discard_owned_stage_prepared",
        ],
    );
    for forbidden in [
        "std::process::Command",
        "std::fs::hard_link",
        "std::fs::rename",
        "std::fs::OpenOptions",
    ] {
        assert!(
            !physical.contains(forbidden),
            "private B bypassed held platform authority through `{forbidden}`"
        );
    }
    assert!(
        physical.contains("for (optimization, invocation) in [(0_u8, c_o0), (2_u8, c_o2)]"),
        "private B does not execute both O0 and O2 evidence builds"
    );
    assert_contains_all(
        "complete O0/O2 static-link and runtime evidence",
        physical,
        &[
            "module_O0.o",
            "module_O2.o",
            "__semaprax_native_rust_link_O0",
            "__semaprax_native_rust_link_O2",
            "platform::link_tool_prepared",
            "platform::execute_tool_prepared",
        ],
    );
    assert_eq!(
        implementation
            .matches("fn discard_run_stage<const N: usize>(")
            .count(),
        1,
        "private B must define one narrow exact-inventory settlement helper"
    );
    assert_contains_all(
        "unconditional one-attempt run-stage settlement",
        &implementation,
        &[
            "run_files: RunDiscardInventory,",
            "let run_files = prepare_run_discard_inventory()?;",
            "mut run_files,",
            "let build = (|| {",
            "let cleanup = discard_run_stage(&parent_authority, &run_stage, &run_files);",
            "let mut facts = match (build, cleanup)",
            "discard_run_stage(&parent_authority, &stage, &publish_files)",
            "if publication.is_err()",
        ],
    );

    let sys = read("crates/semaprax-native-rust-interop-platform-sys/src/lib.rs");
    assert!(sys.contains("\"-O0\""));
    assert!(sys.contains("\"-O2\""));
    let windows = sys
        .split("#[cfg(windows)]")
        .nth(1)
        .expect("Windows quarantine implementation");
    assert!(
        !windows.contains("unsupported!("),
        "Windows physical authority is still an unsupported stub"
    );
    let windows_run = windows
        .split("fn run_argv(")
        .nth(1)
        .and_then(|tail| tail.split("pub fn rustc_version(").next())
        .expect("bounded Windows run_argv implementation");
    for forbidden in [
        "_authority_markers",
        "_authority_types",
        "Command::new",
        ".spawn()",
    ] {
        assert!(
            !windows_run.contains(forbidden),
            "Windows run authority is still a marker or std::process baseline: `{forbidden}`"
        );
    }
    let ordered_calls = [
        "InitializeProcThreadAttributeList(",
        "UpdateProcThreadAttribute(",
        "CreateJobObjectW(",
        "SetInformationJobObject(",
        "CreateProcessW(",
        "QueryFullProcessImageNameW(",
        "AssignProcessToJobObject(",
        "ResumeThread(",
    ];
    let mut prior = None;
    for call in ordered_calls {
        let offset = windows_run
            .find(call)
            .unwrap_or_else(|| panic!("Windows run authority does not call `{call}`"));
        if let Some(previous) = prior {
            assert!(
                offset > previous,
                "Windows suspended-process authority calls `{call}` out of order"
            );
        }
        prior = Some(offset);
    }
    assert_contains_all(
        "Windows contained error settlement",
        windows_run,
        &[
            "PROC_THREAD_ATTRIBUTE_HANDLE_LIST",
            "CREATE_SUSPENDED",
            "EXTENDED_STARTUPINFO_PRESENT",
            "TerminateJobObject(",
            "WaitForSingleObject(",
            "must_terminate_unassigned(process_handle.raw());",
            "must_settle_job(job.raw(), process_handle.raw(), true);",
            "DeleteProcThreadAttributeList(",
        ],
    );
    assert!(
        !windows_run.contains("let _ = settle_job(")
            && !windows_run.contains("let _ = terminate_unassigned("),
        "Windows run authority ignores failed process-settlement proof"
    );
    assert_contains_all(
        "fail-stop process settlement",
        &sys,
        &[
            "fn must_settle_failed_group(",
            "if settle_failed_group(pid, pipe, leader_reaped).is_err()",
            "fn must_terminate_unassigned(",
            "if terminate_unassigned(process).is_err()",
            "fn must_settle_job(",
            "if settle_job(job, process, terminate).is_err()",
            "std::process::abort();",
        ],
    );
    assert_contains_all(
        "Windows handle settlement RAII",
        windows,
        &[
            "struct CheckedHandle(Option<HANDLE>);",
            "impl Drop for CheckedHandle",
            "if unsafe { CloseHandle(handle) } == 0",
        ],
    );
    let ambient_settlement_variable =
        ["SEMAPRAX_NATIVE_RUST", "_INTEROP_TEST_SETTLEMENT_FAILURE"].concat();
    assert!(
        !sys.contains(&ambient_settlement_variable),
        "production sys retains ambient settlement-failure authority"
    );
    assert_contains_all(
        "cfg(test)-local process settlement evidence",
        &sys,
        &[
            "#[cfg(test)]\nstatic TEST_SETTLEMENT_FAILURES",
            "linux_runner_boundaries_settle_or_fail_stop_without_later_action",
            "helper_linux_parent_write_close",
            "helper_linux_waitpid",
            "windows_runner_failures_use_only_explicit_test_state",
            "execute_harness_with_argument",
            "helper_windows_query_job_fail_stop",
            "later action ran after fail-stop",
            "later action ran after destroy uncertainty",
        ],
    );
    let windows_filesystem = windows
        .split("fn open_directory(")
        .nth(1)
        .and_then(|tail| tail.split("fn run_argv(").next())
        .expect("bounded Windows filesystem authority implementation");
    for forbidden in [
        "_authority_markers",
        "_authority_types",
        "std::fs::OpenOptions",
        "std::fs::create_dir",
        "std::fs::hard_link",
        "std::fs::read_dir",
        "std::fs::rename",
        "std::fs::remove_file",
        "std::fs::remove_dir",
        ".path.join(",
    ] {
        assert!(
            !windows_filesystem.contains(forbidden),
            "Windows filesystem authority still uses a stored-path or marker baseline: `{forbidden}`"
        );
    }
    assert_contains_all(
        "Windows root-handle-relative filesystem authority",
        windows_filesystem,
        &[
            "NtCreateFile(",
            "NtSetInformationFile(",
            "OBJECT_ATTRIBUTES",
            "RootDirectory",
            "FILE_OPEN_REPARSE_POINT",
            "FileIdBothDirectoryInfo",
            "SetFileInformationByHandle(",
            "FileLinkInformationEx",
            "FileRenameInformation",
            "FileDispositionInfoEx",
        ],
    );
    assert_contains_all(
        "Windows 128-bit held identity",
        windows,
        &[
            "GetFileInformationByHandleEx(",
            "FileIdInfo",
            "file_id: [u8; 16]",
        ],
    );
    assert!(
        windows_filesystem.matches("relative_file(").count() == 4,
        "Windows Nt RootDirectory helper definition/caller topology drifted"
    );
    assert!(
        windows_filesystem.matches("NtSetInformationFile(").count() == 2
            && windows_filesystem
                .matches("SetFileInformationByHandle(")
                .count()
                == 1
            && windows_filesystem.matches("disposition_delete(").count() >= 3,
        "Windows link, publish, and delete callers are not all held-handle operations"
    );
    let link_helper = windows_filesystem
        .split("pub fn link_or_copy_new_prepared")
        .nth(1)
        .and_then(|tail| tail.split("pub fn inventory_exact_prepared").next())
        .expect("bounded Windows prepared NT link operation");
    assert_contains_all(
        "Windows held-handle link authority",
        link_helper,
        &["NtSetInformationFile(", "FileLinkInformationEx"],
    );
    assert!(
        !link_helper.contains("SetFileInformationByHandle("),
        "Windows native FileLinkInformationEx was sent through the Win32 information API"
    );
    let windows_evidence =
        read("crates/semaprax-native-rust-interop-platform/tests/windows_authority.rs");
    assert_contains_all(
        "Windows physical authority evidence",
        &windows_evidence,
        &[
            "windows_junctions_and_same_path_directory_substitution_are_rejected",
            "windows_create_inventory_publish_and_exact_discard_are_no_clobber",
            "windows_discard_stops_on_inventory_and_stage_identity_drift",
            "windows_held_executable_uses_held_identity_and_empty_environment",
            "windows_run_argv_handles_zero_and_small_stdout_at_normal_eof",
            "windows_names_are_exact_ascii_non_dos_and_casefold_no_clobber",
            "windows_descendant_held_stdout_is_quiesced_without_output_overflow",
            "windows_silent_timeout_is_bounded_and_reaps_the_leader",
            "windows_output_overflow_kills_and_reaps_the_process_tree_with_a_bounded_wait",
            "windows_external_consumer_cannot_extract_handles_or_reach_sys_quarantine",
        ],
    );
}

#[test]
fn private_platform_cleanup_surface_is_exact_inventory_only() {
    let facade = read("crates/semaprax-native-rust-interop-platform/src/lib.rs");
    let sys = read("crates/semaprax-native-rust-interop-platform-sys/src/lib.rs");
    assert_contains_all(
        "safe exact-inventory cleanup facade",
        &facade,
        &[
            "pub fn discard_owned_stage_prepared<const N: usize>(",
            "parent: &HeldDirectory",
            "stage: &HeldDirectory",
            "stage_name: &PreparedStageName",
            "inventory: &PreparedDiscardInventory<N>",
        ],
    );
    assert_contains_all(
        "system exact-inventory cleanup quarantine",
        &sys,
        &[
            "pub fn discard_owned_stage_prepared<const N: usize>(",
            "parent: &Directory",
            "stage: &Directory",
            "stage_name: &PreparedRelativeNameArena",
            "names: &PreparedDiscardNames<N>",
            "files: &[Option<&RegularFile>; N]",
        ],
    );
    let production_sys = sys
        .split("#[cfg(test)]\nmod tests")
        .next()
        .expect("production sys prefix");
    for (label, source) in [
        ("safe facade", facade.as_str()),
        ("system quarantine", production_sys),
    ] {
        for forbidden in [
            "remove_dir_all",
            "pub fn remove",
            "pub fn delete",
            "pub fn discard_path",
            "pub fn discard_directory",
        ] {
            assert!(
                !source.contains(forbidden),
                "{label} exposed generic or recursive deletion through `{forbidden}`"
            );
        }
    }
}

#[test]
fn hosted_workflow_names_all_private_interop_evidence_boundaries() {
    let workflow = read(".github/workflows/ci.yml");
    for required in [
        "Require private Native Rust Interop language, HIR, Graph, and Wasm preservation evidence",
        "cargo test --locked -p semaprax --test native_rust_interop_v1 -- --nocapture",
        "cargo test --locked -p semaprax --test native_rust_interop_ci_contract -- --nocapture",
        "Run workspace tests with bounded Windows process concurrency",
        "cargo test --locked --workspace --all-targets --all-features -- --test-threads=2",
        "Require private Native Rust Interop A+B replay, static-link, runtime, and hostile evidence",
        "cargo test --locked -p semaprax-native-rust-interop -- --nocapture",
        "Require private Native Rust Interop A+B replay, static-link, runtime, and hostile evidence (Windows bounded)",
        "cargo test --locked -p semaprax-native-rust-interop -- --nocapture --test-threads=2",
        "Require private Native Rust Interop platform authority evidence",
        "cargo test --locked -p semaprax-native-rust-interop-platform --all-targets -- --nocapture",
        "Require private Native Rust Interop ASan + UBSan round trip (Linux)",
        "VCToolsInstallDir",
        "SEMAPRAX_VCTOOLS=%SEMAPRAX_VCTOOLS%",
        "SEMAPRAX_LINKER=%SEMAPRAX_LINKER%",
        "if not exist \"%SEMAPRAX_VCTOOLS%\" exit /b 1",
        "if not exist \"%SEMAPRAX_LINKER%\" exit /b 1",
        "SEMAPRAX_REQUIRE_NATIVE_RUST_INTEROP_SANITIZERS: \"1\"",
        "implementation::tests::linked_bridge_round_trips_rust_to_semaprax_to_rust_and_closes_failures -- --exact --nocapture",
    ] {
        assert!(workflow.contains(required), "workflow is missing `{required}`");
    }
    assert_eq!(
        workflow
            .lines()
            .filter(|line| {
                line.trim()
                    == "run: cargo test --locked -p semaprax-native-rust-interop -- --nocapture"
            })
            .count(),
        1
    );
    let windows_environment = workflow
        .split("- name: Resolve the authenticated Windows SDK and MSVC environment")
        .nth(1)
        .and_then(|tail| tail.split("      - ").next())
        .expect("Windows SDK and linker environment step");
    assert!(windows_environment.contains("echo INCLUDE="));
    assert!(windows_environment.contains("echo LIB="));
    assert!(windows_environment.contains("echo SEMAPRAX_VCTOOLS="));
    assert!(windows_environment.contains("echo SEMAPRAX_LINKER="));
    assert!(
        !windows_environment.contains("echo PATH="),
        "Windows linker setup must not restore ambient PATH to child processes"
    );
    let sanitizer_step = workflow
        .split("- name: Require private Native Rust Interop ASan + UBSan round trip (Linux)")
        .nth(1)
        .and_then(|tail| tail.split("      - ").next())
        .expect("private sanitizer workflow step");
    assert!(
        !sanitizer_step.contains("CLANG:"),
        "sanitizer workflow must not bypass authenticated tool discovery with a bare CLANG path"
    );

    let implementation = read("crates/semaprax-native-rust-interop-builder/src/implementation.rs");
    for required in [
        "SEMAPRAX_REQUIRE_NATIVE_RUST_INTEROP_SANITIZERS",
        "-fsanitize=address,undefined",
        "-fno-sanitize-recover=all",
    ] {
        assert!(
            implementation.contains(required),
            "sanitizer gate is not enforced by production/test build code: `{required}`"
        );
    }
}
