//! Phase-B staging: discard inventories, prebuilt carriers, manifest
//! capacity, prepared file names, and prepared link copies.

use super::*;

#[test]
fn phase_b_discard_inventory_capacity_minus_one_rejects_without_effects() {
    let ((publish, run), overflowed, exact) =
        crate::bounded_output::with_limit_usage(MAX_BUILDER_BYTES, || {
            (
                prepare_publish_discard_inventory(),
                prepare_run_discard_inventory(),
            )
        });
    assert!(!overflowed);
    let publish = publish.unwrap();
    let run = run.unwrap();
    assert_eq!((publish.capacity(), publish.attached()), (7, 0));
    assert_eq!((run.capacity(), run.attached()), (10, 0));
    assert!(exact > 0);

    let ((publish, run), overflowed) = crate::bounded_output::with_limit(exact - 1, || {
        (
            prepare_publish_discard_inventory(),
            prepare_run_discard_inventory(),
        )
    });
    assert!(!overflowed);
    assert!(publish.is_ok());
    let error = match run {
        Ok(_) => panic!("capacity-minus-one admitted the run discard plan"),
        Err(error) => error,
    };
    assert_eq!(error.code, "SPX-B109");
    assert_eq!(
        error.message,
        "Native Rust Interop max_builder_bytes exceeds 33554432"
    );
}

#[test]
fn phase_b_created_directory_auth_disagreement_attempts_one_discard_and_stops() {
    for mode in [
        CreateAuthDisagreement::Clean,
        CreateAuthDisagreement::Substituted,
    ] {
        let (program, spec) = fixture();
        let root = std::fs::canonicalize(std::env::temp_dir())
            .unwrap()
            .join(format!(
                "semaprax-native-rust-auth-disagreement-{}-{mode:?}",
                std::process::id()
            ));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir(&root).unwrap();
        CREATE_AUTH_DISCARD_ATTEMPTS.with(|attempts| attempts.set(0));
        CREATE_AUTH_DISAGREEMENT.with(|injection| injection.set(Some(mode)));
        let mut hooks = 0usize;
        let result = build_native_rust_interop_bundle_with_hook(
            &program,
            spec.as_bytes(),
            &root.join("bundle"),
            |_, _, _, _| hooks += 1,
        );
        CREATE_AUTH_DISAGREEMENT.with(|injection| injection.set(None));
        let error = match result {
            Ok(_) => panic!("created-directory authentication disagreement was accepted"),
            Err(error) => error,
        };
        assert_eq!(error.len(), 1);
        assert_eq!(error[0].code, "SPX-I232");
        assert_eq!(CREATE_AUTH_DISCARD_ATTEMPTS.with(std::cell::Cell::get), 1);
        assert_eq!(
            PHASE_B_POST_EFFECT_ERROR_MATERIALIZATIONS.with(std::cell::Cell::get),
            0,
            "authentication disagreement materialized its sticky diagnostic after effects",
        );
        assert_eq!(hooks, 0, "later build action followed sticky I232");
        assert!(!root.join("bundle").exists());
        match mode {
            CreateAuthDisagreement::Clean => {
                assert_eq!(std::fs::read_dir(&root).unwrap().count(), 0);
            }
            CreateAuthDisagreement::Substituted => {
                assert!(root.join("auth-displaced").is_dir());
                let substitute = std::fs::read_dir(&root)
                    .unwrap()
                    .map(Result::unwrap)
                    .find(|entry| {
                        entry
                            .file_name()
                            .to_string_lossy()
                            .starts_with(".semaprax-native-rust-interop-publish-")
                    })
                    .expect("substituted stage remains inert");
                assert_eq!(
                    std::fs::read(substitute.path().join("foreign-sentinel")).unwrap(),
                    b"foreign"
                );
            }
        }
        std::fs::remove_dir_all(&root).unwrap();
    }
}

#[test]
fn phase_b_all_local_failures_move_exact_prebuilt_carrier_and_settle_without_later_action() {
    let kinds = [
        PhaseBLocalError::BuilderBudget,
        PhaseBLocalError::ManifestBudget,
        PhaseBLocalError::Unsupported,
        PhaseBLocalError::Replay,
        PhaseBLocalError::Compile,
        PhaseBLocalError::Link,
        PhaseBLocalError::Publication,
    ];
    for (index, kind) in kinds.into_iter().enumerate() {
        let (program, spec) = fixture();
        let root = std::fs::canonicalize(std::env::temp_dir())
            .unwrap()
            .join(format!(
                "semaprax-native-rust-local-carrier-{}-{index}",
                std::process::id()
            ));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir(&root).unwrap();
        PHASE_B_DISCARD_ATTEMPTS.with(|attempts| attempts.set(0));
        PHASE_B_BUILD_INVOCATION_PLANS.with(|count| count.set(0));
        PHASE_B_BUILD_INVOCATION_CONSUMPTIONS.with(|count| count.set(0));
        PHASE_B_LOCAL_FAILURE_INJECTION.with(|injection| injection.set(Some(kind)));
        let mut hooks = 0usize;
        let result = build_native_rust_interop_bundle_with_hook(
            &program,
            spec.as_bytes(),
            &root.join("bundle"),
            |_, _, _, _| hooks += 1,
        );
        PHASE_B_LOCAL_FAILURE_INJECTION.with(|injection| injection.set(None));
        let error = match result {
            Ok(_) => panic!("{kind:?} failure injection unexpectedly succeeded"),
            Err(error) => error,
        };
        let (code, message) = kind.diagnostic();
        assert_eq!(error.len(), 1);
        assert_eq!(error[0].code, code);
        assert_eq!(error[0].message, message);
        assert_eq!(
            error[0].message.as_ptr() as usize,
            PHASE_B_PREPARED_CARRIER_IDENTITIES.with(std::cell::Cell::get)[kind.index()],
            "{kind:?} did not move its pre-effect String allocation",
        );
        assert_eq!(
            PHASE_B_POST_EFFECT_ERROR_MATERIALIZATIONS.with(std::cell::Cell::get),
            0,
            "{kind:?} materialized a Diagnostic, String, or Vec after effects",
        );
        assert_eq!(PHASE_B_DISCARD_ATTEMPTS.with(std::cell::Cell::get), 2);
        assert_eq!(PHASE_B_BUILD_INVOCATION_PLANS.with(std::cell::Cell::get), 8);
        assert_eq!(
            PHASE_B_BUILD_INVOCATION_CONSUMPTIONS.with(std::cell::Cell::get),
            0,
        );
        assert_eq!(hooks, 0, "{kind:?} allowed a later build action");
        assert!(!root.join("bundle").exists());
        assert_eq!(std::fs::read_dir(&root).unwrap().count(), 0);
        std::fs::remove_dir_all(&root).unwrap();
    }
}

#[test]
fn phase_b_real_oversize_manifest_uses_exact_manifest_carrier_and_settles() {
    let (program, spec) = fixture();
    let root = std::fs::canonicalize(std::env::temp_dir())
        .unwrap()
        .join(format!(
            "semaprax-native-rust-oversize-manifest-{}",
            std::process::id()
        ));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir(&root).unwrap();
    PHASE_B_DISCARD_ATTEMPTS.with(|attempts| attempts.set(0));
    reset_phase_b_object_authority_observer();
    reset_phase_b_manifest_authority_observer();
    PHASE_B_OVERSIZE_MANIFEST_INJECTION.with(|injection| injection.set(true));
    let mut observed = Vec::new();
    let result = build_native_rust_interop_bundle_with_hook(
        &program,
        spec.as_bytes(),
        &root.join("bundle"),
        |point, _, _, _| observed.push(point),
    );
    PHASE_B_OVERSIZE_MANIFEST_INJECTION.with(|injection| injection.set(false));
    let error = match result {
        Ok(_) => panic!("oversize manifest unexpectedly published"),
        Err(error) => error,
    };
    assert_eq!(error.len(), 1);
    assert_eq!(error[0].code, "SPX-B109");
    assert_eq!(error[0].message, PHASE_B_MANIFEST_BUDGET_MESSAGE);
    assert_eq!(
        error[0].message.as_ptr() as usize,
        PHASE_B_PREPARED_CARRIER_IDENTITIES.with(std::cell::Cell::get)
            [PhaseBLocalError::ManifestBudget.index()],
    );
    assert_eq!(
        PHASE_B_POST_EFFECT_ERROR_MATERIALIZATIONS.with(std::cell::Cell::get),
        0,
    );
    assert_eq!(PHASE_B_DISCARD_ATTEMPTS.with(std::cell::Cell::get), 2);
    assert_eq!(
        PHASE_B_OBJECT_AUTHORITY_TRANSFERS.with(std::cell::Cell::get),
        1
    );
    assert_eq!(PHASE_B_OBJECT_AUTHORITY_DROPS.with(std::cell::Cell::get), 1);
    assert_eq!(
        PHASE_B_OBJECT_AUTHORITY_MANIFEST_OBSERVATIONS.with(std::cell::Cell::get),
        0
    );
    assert_eq!(
        PHASE_B_OBJECT_AUTHORITY_PUBLISH_OBSERVATIONS.with(std::cell::Cell::get),
        0
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
    assert_eq!(
        observed,
        [
            NativeRustBuildPoint::BeforeClang,
            NativeRustBuildPoint::BeforeRustLink,
            NativeRustBuildPoint::BeforeExecutableAuthentication,
            NativeRustBuildPoint::BeforeExecute,
            NativeRustBuildPoint::BeforeExecutableAuthentication,
            NativeRustBuildPoint::BeforeExecute,
            NativeRustBuildPoint::BeforeObjectRead,
        ],
        "oversize manifest allowed a later manifest-write or publish hook",
    );
    assert!(!root.join("bundle").exists());
    assert_eq!(std::fs::read_dir(&root).unwrap().count(), 0);
    std::fs::remove_dir_all(&root).unwrap();
}

#[test]
fn phase_b_manifest_capacity_minus_one_fails_before_any_effect() {
    let (program, spec) = fixture();
    let root = std::fs::canonicalize(std::env::temp_dir())
        .unwrap()
        .join(format!(
            "semaprax-native-rust-manifest-capacity-minus-one-{}",
            std::process::id()
        ));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir(&root).unwrap();
    reset_phase_b_manifest_authority_observer();
    PHASE_B_OUTPUT_PROBES.with(|count| count.set(0));
    PHASE_B_TOOL_HOLDS.with(|count| count.set(0));
    PHASE_B_TOOL_PROCESSES.with(|count| count.set(0));
    PHASE_B_BUILD_INVOCATION_CONSUMPTIONS.with(|count| count.set(0));
    PHASE_B_DISCARD_ATTEMPTS.with(|count| count.set(0));
    PHASE_B_MANIFEST_PLAN_CAPACITY.with(|capacity| capacity.set(MAX_MANIFEST_BYTES - 1));
    let mut hooks = 0_usize;
    let result = build_native_rust_interop_bundle_with_hook(
        &program,
        spec.as_bytes(),
        &root.join("bundle"),
        |_, _, _, _| hooks += 1,
    );
    PHASE_B_MANIFEST_PLAN_CAPACITY.with(|capacity| capacity.set(MAX_MANIFEST_BYTES));
    let error = match result {
        Ok(_) => panic!("capacity-minus-one manifest plan unexpectedly reached effects"),
        Err(error) => error,
    };
    assert_eq!(error.len(), 1);
    assert_eq!(error[0].code, "SPX-B109");
    assert_eq!(error[0].message, PHASE_B_MANIFEST_BUDGET_MESSAGE);
    assert_eq!(
        error[0].message.as_ptr() as usize,
        PHASE_B_PREPARED_CARRIER_IDENTITIES.with(std::cell::Cell::get)
            [PhaseBLocalError::ManifestBudget.index()]
    );
    assert_eq!(hooks, 0);
    assert_eq!(PHASE_B_OUTPUT_PROBES.with(std::cell::Cell::get), 0);
    assert_eq!(PHASE_B_TOOL_HOLDS.with(std::cell::Cell::get), 0);
    assert_eq!(PHASE_B_TOOL_PROCESSES.with(std::cell::Cell::get), 0);
    assert_eq!(
        PHASE_B_BUILD_INVOCATION_CONSUMPTIONS.with(std::cell::Cell::get),
        0
    );
    assert_eq!(PHASE_B_DISCARD_ATTEMPTS.with(std::cell::Cell::get), 0);
    assert_eq!(
        PHASE_B_POST_EFFECT_ERROR_MATERIALIZATIONS.with(std::cell::Cell::get),
        0
    );
    assert_eq!(
        PHASE_B_MANIFEST_ARENA_ALLOCATIONS.with(std::cell::Cell::get),
        1
    );
    assert_eq!(
        PHASE_B_MANIFEST_AUTHORITY_TRANSFERS.with(std::cell::Cell::get),
        0
    );
    assert_phase_b_manifest_drop_order(0);
    assert!(!root.join("bundle").exists());
    assert_eq!(std::fs::read_dir(&root).unwrap().count(), 0);
    std::fs::remove_dir_all(&root).unwrap();
}

#[cfg(debug_assertions)]
#[test]
fn phase_b_primary_failure_is_sticky_over_mid_cleanup_failure_without_materialization() {
    let (program, spec) = fixture();
    let root = std::fs::canonicalize(std::env::temp_dir())
        .unwrap()
        .join(format!(
            "semaprax-native-rust-sticky-cleanup-{}",
            std::process::id()
        ));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir(&root).unwrap();
    PHASE_B_DISCARD_ATTEMPTS.with(|attempts| attempts.set(0));
    PHASE_B_DISCARD_FAILURE_AFTER_DELETE.with(|failure| failure.set(Some(0)));
    PHASE_B_LOCAL_FAILURE_INJECTION
        .with(|injection| injection.set(Some(PhaseBLocalError::Compile)));
    let mut hooks = 0usize;
    let result = build_native_rust_interop_bundle_with_hook(
        &program,
        spec.as_bytes(),
        &root.join("bundle"),
        |_, _, _, _| hooks += 1,
    );
    PHASE_B_LOCAL_FAILURE_INJECTION.with(|injection| injection.set(None));
    PHASE_B_DISCARD_FAILURE_AFTER_DELETE.with(|failure| failure.set(None));
    let error = match result {
        Ok(_) => panic!("compile failure injection unexpectedly succeeded"),
        Err(error) => error,
    };
    assert_eq!(error.len(), 1);
    assert_eq!(error[0].code, "SPX-I230");
    assert_eq!(error[0].message, PHASE_B_COMPILE_MESSAGE);
    assert_eq!(
        error[0].message.as_ptr() as usize,
        PHASE_B_PREPARED_CARRIER_IDENTITIES.with(std::cell::Cell::get)
            [PhaseBLocalError::Compile.index()],
    );
    assert_eq!(
        PHASE_B_POST_EFFECT_ERROR_MATERIALIZATIONS.with(std::cell::Cell::get),
        0,
    );
    assert_eq!(PHASE_B_DISCARD_ATTEMPTS.with(std::cell::Cell::get), 2);
    assert_eq!(hooks, 0);
    assert!(!root.join("bundle").exists());
    assert_eq!(
        std::fs::read_dir(&root)
            .unwrap()
            .filter_map(Result::ok)
            .filter(|entry| entry
                .file_name()
                .to_string_lossy()
                .starts_with(".semaprax-native-rust-interop-run-"))
            .count(),
        1,
        "failed run-stage cleanup must leave one inert owned residue",
    );
    std::fs::remove_dir_all(&root).unwrap();
}

#[test]
fn phase_b_fixed_discard_plans_remove_every_partial_prefix_without_growth() {
    fn exercise<const N: usize>(label: &str, names: [&'static str; N]) {
        let root = std::fs::canonicalize(std::env::temp_dir())
            .unwrap()
            .join(format!(
                "semaprax-native-rust-prefix-{label}-{}",
                std::process::id()
            ));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir(&root).unwrap();
        let parent = platform::hold_directory(&root).unwrap();
        for prefix in 0..=N {
            let stage_name = format!("s{prefix}");
            let prepared_stage = platform::prepare_stage_name(stage_name.as_ref()).unwrap();
            let stage =
                platform::create_directory_new(&parent, stage_name.as_ref(), 0o700).unwrap();
            let os_names = names.map(OsStr::new);
            let mut inventory = platform::prepare_discard_inventory(os_names).unwrap();
            let native_capacity = platform::prepared_discard_inventory_owned_capacity(&inventory);
            assert_eq!(inventory.capacity(), N);
            for (index, name) in names.iter().take(prefix).enumerate() {
                platform::write_file_new_prepared(&stage, &mut inventory, name, b"owned", 0o600)
                    .unwrap();
                assert_eq!(inventory.attached(), index + 1);
                assert_eq!(inventory.capacity(), N);
                assert_eq!(
                    platform::prepared_discard_inventory_owned_capacity(&inventory),
                    native_capacity
                );
            }
            assert_eq!(inventory.attached(), prefix);
            platform::discard_owned_stage_prepared(&parent, &stage, &prepared_stage, &inventory)
                .unwrap();
            // Windows finalizes the delete disposition only after the
            // last authenticated directory handle closes.
            drop(stage);
            assert!(!root.join(stage_name).exists());
        }
        std::fs::remove_dir_all(&root).unwrap();
    }

    exercise("publish", ["p0", "p1", "p2", "p3", "p4", "p5", "p6"]);
    exercise(
        "run",
        ["r0", "r1", "r2", "r3", "r4", "r5", "r6", "r7", "r8", "r9"],
    );
}

#[test]
fn phase_b_prepared_file_names_reject_invalid_duplicate_and_wrong_order_before_create() {
    for invalid in ["", ".", "..", "slash/name", "nul\0name"] {
        assert!(platform::prepare_discard_inventory([OsStr::new(invalid)]).is_err());
    }
    assert!(platform::prepare_discard_inventory([
        OsStr::new("duplicate"),
        OsStr::new("duplicate")
    ])
    .is_err());
    let bounded_names = [OsStr::new("first"), OsStr::new("second")];
    let bounded = platform::prepare_discard_inventory(bounded_names).unwrap();
    let exact_native = platform::prepared_discard_inventory_owned_capacity(&bounded);
    assert!(exact_native > 0);
    drop(bounded);
    assert!(platform::prepare_discard_inventory_bounded(bounded_names, exact_native).is_ok());
    assert!(platform::prepare_discard_inventory_bounded(bounded_names, exact_native - 1).is_err());
    #[cfg(windows)]
    {
        for invalid in ["back\\slash", "CON", "com1.txt", "trailing.", "trailing "] {
            assert!(platform::prepare_discard_inventory([OsStr::new(invalid)]).is_err());
        }
        assert!(
            platform::prepare_discard_inventory([OsStr::new("Case"), OsStr::new("case")]).is_err()
        );
    }

    let root = std::fs::canonicalize(std::env::temp_dir())
        .unwrap()
        .join(format!(
            "semaprax-native-rust-prepared-file-order-{}",
            std::process::id()
        ));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir(&root).unwrap();
    let directory = platform::hold_directory(&root).unwrap();
    let mut inventory =
        platform::prepare_discard_inventory([OsStr::new("first"), OsStr::new("second")]).unwrap();
    reset_phase_b_error_materialization_observer();
    assert!(platform::write_file_new_prepared(
        &directory,
        &mut inventory,
        "second",
        b"must-not-exist",
        0o600,
    )
    .is_err());
    assert!(platform::hold_regular_file_prepared(&directory, &inventory, "second").is_err());
    assert!(platform::hold_regular_file_prepared(&directory, &inventory, "unknown").is_err());
    assert_eq!(inventory.attached(), 0);
    assert_eq!(std::fs::read_dir(&root).unwrap().count(), 0);
    platform::write_file_new_prepared(&directory, &mut inventory, "first", b"first", 0o600)
        .unwrap();
    assert!(platform::write_file_new_prepared(
        &directory,
        &mut inventory,
        "first",
        b"duplicate",
        0o600,
    )
    .is_err());
    assert!(platform::hold_regular_file_prepared(&directory, &inventory, "second").is_err());
    assert_eq!(inventory.attached(), 1);
    assert!(!root.join("second").exists());
    platform::write_file_new_prepared(&directory, &mut inventory, "second", b"second", 0o600)
        .unwrap();
    let held = platform::hold_regular_file_prepared(&directory, &inventory, "second").unwrap();
    platform::recheck_regular_file(&held).unwrap();
    drop(held);
    std::fs::rename(root.join("second"), root.join("tracked-original")).unwrap();
    std::fs::write(root.join("second"), b"foreign").unwrap();
    assert!(platform::hold_regular_file_prepared(&directory, &inventory, "second").is_err());
    assert_eq!(
        platform::read_exact(inventory.file("second").unwrap(), b"second".len()).unwrap(),
        b"second"
    );
    assert_eq!(std::fs::read(root.join("second")).unwrap(), b"foreign");
    assert_eq!(
        PHASE_B_POST_EFFECT_ERROR_MATERIALIZATIONS.with(std::cell::Cell::get),
        0
    );
    drop(inventory);
    drop(directory);
    std::fs::remove_dir_all(&root).unwrap();
}

#[test]
fn phase_b_prepared_link_copy_binds_tracked_source_and_exact_next_destination() {
    let root = std::fs::canonicalize(std::env::temp_dir())
        .unwrap()
        .join(format!(
            "semaprax-native-rust-prepared-link-copy-{}",
            std::process::id()
        ));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir(&root).unwrap();
    let directory = platform::hold_directory(&root).unwrap();
    let mut source = platform::prepare_discard_inventory([OsStr::new("source")]).unwrap();
    let mut destination =
        platform::prepare_discard_inventory([OsStr::new("first"), OsStr::new("copy")]).unwrap();
    let future = platform::prepare_link_or_copy(&source, "source", &destination, "copy").unwrap();
    let unattached =
        platform::prepare_link_or_copy(&source, "source", &destination, "copy").unwrap();
    let substituted =
        platform::prepare_link_or_copy(&source, "source", &destination, "copy").unwrap();
    let duplicate =
        platform::prepare_link_or_copy(&source, "source", &destination, "copy").unwrap();
    assert!(platform::prepare_link_or_copy(&source, "unknown", &destination, "copy").is_err());
    assert!(platform::prepare_link_or_copy(&source, "source", &destination, "unknown").is_err());

    reset_phase_b_error_materialization_observer();
    assert!(platform::link_or_copy_new_prepared(
        unattached,
        &source,
        &directory,
        &mut destination,
        b"original",
    )
    .is_err());
    assert!(platform::link_or_copy_new_prepared(
        future,
        &source,
        &directory,
        &mut destination,
        b"original",
    )
    .is_err());
    assert_eq!(destination.attached(), 0);
    assert_eq!(std::fs::read_dir(&root).unwrap().count(), 0);

    platform::write_file_new_prepared(&directory, &mut source, "source", b"original", 0o600)
        .unwrap();
    platform::write_file_new_prepared(&directory, &mut destination, "first", b"first", 0o600)
        .unwrap();
    std::fs::rename(root.join("source"), root.join("tracked-source")).unwrap();
    std::fs::write(root.join("source"), b"foreign").unwrap();
    platform::link_or_copy_new_prepared(
        substituted,
        &source,
        &directory,
        &mut destination,
        b"original",
    )
    .unwrap();
    assert_eq!(destination.attached(), 2);
    assert_eq!(
        platform::read_exact(destination.file("copy").unwrap(), b"original".len()).unwrap(),
        b"original"
    );
    assert_eq!(std::fs::read(root.join("source")).unwrap(), b"foreign");
    assert!(platform::link_or_copy_new_prepared(
        duplicate,
        &source,
        &directory,
        &mut destination,
        b"original",
    )
    .is_err());
    assert_eq!(destination.attached(), 2);

    let mut exists_destination = platform::prepare_discard_inventory([
        OsStr::new("exists-first"),
        OsStr::new("exists-copy"),
    ])
    .unwrap();
    let exists =
        platform::prepare_link_or_copy(&source, "source", &exists_destination, "exists-copy")
            .unwrap();
    platform::write_file_new_prepared(
        &directory,
        &mut exists_destination,
        "exists-first",
        b"first",
        0o600,
    )
    .unwrap();
    std::fs::write(root.join("exists-copy"), b"foreign-destination").unwrap();
    assert!(platform::link_or_copy_new_prepared(
        exists,
        &source,
        &directory,
        &mut exists_destination,
        b"original",
    )
    .is_err());
    assert_eq!(exists_destination.attached(), 1);
    assert_eq!(
        std::fs::read(root.join("exists-copy")).unwrap(),
        b"foreign-destination"
    );
    assert_eq!(
        PHASE_B_POST_EFFECT_ERROR_MATERIALIZATIONS.with(std::cell::Cell::get),
        0
    );
    drop((exists_destination, destination, source, directory));
    std::fs::remove_dir_all(&root).unwrap();
}

#[cfg(debug_assertions)]
#[test]
fn phase_b_post_link_pre_auth_failure_is_sticky_and_preserves_inert_stage() {
    let (program, spec) = fixture();
    let root = std::fs::canonicalize(std::env::temp_dir())
        .unwrap()
        .join(format!(
            "semaprax-native-rust-post-link-pre-auth-{}",
            std::process::id()
        ));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir(&root).unwrap();
    let output = root.join("bundle");
    let mut run_stage = None;
    let mut hooks = 0_usize;
    let mut unexpected_hook = false;
    PHASE_B_LINK_COPY_PLANS.with(|count| count.set(0));
    PHASE_B_LINK_COPY_CONSUMPTIONS.with(|count| count.set(0));
    PHASE_B_DISCARD_ATTEMPTS.with(|count| count.set(0));
    PHASE_B_LINK_COPY_FAIL_BEFORE_AUTHENTICATION.with(|enabled| enabled.set(true));
    let result = build_native_rust_interop_bundle_with_hook(
        &program,
        spec.as_bytes(),
        &output,
        |point, _, run, _| {
            hooks += 1;
            unexpected_hook |= point != NativeRustBuildPoint::BeforeClang;
            if point == NativeRustBuildPoint::BeforeClang {
                run_stage = Some(run.to_path_buf());
                std::fs::write(run.join("foreign-sentinel"), b"foreign").unwrap();
            }
        },
    );
    PHASE_B_LINK_COPY_FAIL_BEFORE_AUTHENTICATION.with(|enabled| enabled.set(false));
    let error = match result {
        Ok(_) => panic!("post-link authentication failure unexpectedly succeeded"),
        Err(error) => error,
    };
    assert_eq!(error.len(), 1);
    assert_eq!(error[0].code, "SPX-I232");
    assert_eq!(error[0].message, PHASE_B_PUBLICATION_MESSAGE);
    assert_eq!(hooks, 1);
    assert!(!unexpected_hook);
    assert_eq!(PHASE_B_LINK_COPY_PLANS.with(std::cell::Cell::get), 3);
    assert_eq!(PHASE_B_LINK_COPY_CONSUMPTIONS.with(std::cell::Cell::get), 1);
    assert_eq!(PHASE_B_DISCARD_ATTEMPTS.with(std::cell::Cell::get), 2);
    assert_eq!(
        PHASE_B_POST_EFFECT_ERROR_MATERIALIZATIONS.with(std::cell::Cell::get),
        0
    );
    let run_stage = run_stage.expect("BeforeClang records the run stage");
    assert!(run_stage.is_dir());
    assert_eq!(
        std::fs::read(run_stage.join("foreign-sentinel")).unwrap(),
        b"foreign"
    );
    assert!(run_stage.join("semaprax_native_rust_interop.rs").is_file());
    assert!(!output.exists());
    std::fs::remove_dir_all(&root).unwrap();
}

#[test]
fn phase_b_link_copy_capacity_minus_one_is_pre_effect() {
    let publish = prepare_publish_discard_inventory().unwrap();
    let run = prepare_run_discard_inventory().unwrap();
    let required = platform::link_or_copy_required_capacity(
        &publish,
        "semaprax_native_rust_interop.rs",
        &run,
        "semaprax_native_rust_interop.rs",
    )
    .unwrap();
    assert!(required > 0);

    PHASE_B_OUTPUT_PROBES.with(|count| count.set(0));
    PHASE_B_TOOL_HOLDS.with(|count| count.set(0));
    PHASE_B_TOOL_PROCESSES.with(|count| count.set(0));
    PHASE_B_LINK_COPY_PLANS.with(|count| count.set(0));
    let (plan, overflowed) = crate::bounded_output::with_limit(required, || {
        prepare_link_copy(
            &publish,
            "semaprax_native_rust_interop.rs",
            &run,
            "semaprax_native_rust_interop.rs",
        )
    });
    assert!(!overflowed);
    let (prepared, budget) = plan.unwrap();
    assert_eq!(
        platform::prepared_link_or_copy_owned_capacity(&prepared),
        required
    );
    assert_eq!(budget.maximum(), required);
    drop((prepared, budget));

    PHASE_B_LINK_COPY_PLANS.with(|count| count.set(0));
    let (one_less, overflowed) = crate::bounded_output::with_limit(required - 1, || {
        prepare_link_copy(
            &publish,
            "semaprax_native_rust_interop.rs",
            &run,
            "semaprax_native_rust_interop.rs",
        )
    });
    assert!(!overflowed);
    assert!(matches!(one_less, Err(PhaseBLocalError::BuilderBudget)));
    assert_eq!(PHASE_B_LINK_COPY_PLANS.with(std::cell::Cell::get), 0);
    assert_eq!(PHASE_B_OUTPUT_PROBES.with(std::cell::Cell::get), 0);
    assert_eq!(PHASE_B_TOOL_HOLDS.with(std::cell::Cell::get), 0);
    assert_eq!(PHASE_B_TOOL_PROCESSES.with(std::cell::Cell::get), 0);
    assert_eq!(
        PHASE_B_POST_EFFECT_ERROR_MATERIALIZATIONS.with(std::cell::Cell::get),
        0
    );
}

#[test]
fn phase_b_inventory_exact_plan_is_exact_capacity_and_one_less_is_pre_effect() {
    let publish = prepare_publish_discard_inventory().unwrap();
    let required = platform::inventory_exact_required_capacity(&publish).unwrap();
    assert!(required > 0);
    PHASE_B_OUTPUT_PROBES.with(|count| count.set(0));
    PHASE_B_TOOL_HOLDS.with(|count| count.set(0));
    PHASE_B_TOOL_PROCESSES.with(|count| count.set(0));
    PHASE_B_INVENTORY_EXACT_PLANS.with(|count| count.set(0));
    let (exact, overflowed) =
        crate::bounded_output::with_limit(required, || prepare_publish_inventory_exact(&publish));
    assert!(!overflowed);
    let (prepared, budget) = exact.unwrap();
    assert_eq!(
        platform::prepared_inventory_exact_owned_capacity(&prepared),
        required
    );
    assert_eq!(budget.maximum(), required);
    assert_eq!(platform::prepared_inventory_exact_remaining(&prepared), 2);
    drop((prepared, budget));

    PHASE_B_INVENTORY_EXACT_PLANS.with(|count| count.set(0));
    let (one_less, overflowed) = crate::bounded_output::with_limit(required - 1, || {
        prepare_publish_inventory_exact(&publish)
    });
    assert!(!overflowed);
    assert!(matches!(one_less, Err(PhaseBLocalError::BuilderBudget)));
    assert_eq!(PHASE_B_INVENTORY_EXACT_PLANS.with(std::cell::Cell::get), 0);
    assert_eq!(PHASE_B_OUTPUT_PROBES.with(std::cell::Cell::get), 0);
    assert_eq!(PHASE_B_TOOL_HOLDS.with(std::cell::Cell::get), 0);
    assert_eq!(PHASE_B_TOOL_PROCESSES.with(std::cell::Cell::get), 0);
    assert_eq!(
        PHASE_B_POST_EFFECT_ERROR_MATERIALIZATIONS.with(std::cell::Cell::get),
        0
    );
}

#[test]
fn phase_b_final_publish_plan_is_exact_capacity_and_one_less_is_pre_effect() {
    let output = Path::new("bundle");
    let required = platform::publish_directory_required_capacity(OsStr::new("bundle")).unwrap();
    assert!(required > 0);
    PHASE_B_EFFECT_STARTED.with(|started| started.set(false));
    PHASE_B_OUTPUT_PROBES.with(|count| count.set(0));
    PHASE_B_TOOL_HOLDS.with(|count| count.set(0));
    PHASE_B_TOOL_PROCESSES.with(|count| count.set(0));
    PHASE_B_PUBLISH_PLANS.with(|count| count.set(0));
    let (exact, overflowed) =
        crate::bounded_output::with_limit(required, || prepare_final_publish(output));
    assert!(!overflowed);
    let (prepared, budget) = exact.unwrap();
    assert_eq!(
        platform::prepared_publish_directory_owned_capacity(&prepared),
        required
    );
    assert_eq!(platform::prepared_publish_directory_remaining(&prepared), 1);
    assert_eq!(budget.maximum(), required);
    assert_eq!(PHASE_B_PUBLISH_PLANS.with(std::cell::Cell::get), 1);
    drop((prepared, budget));

    PHASE_B_PUBLISH_PLANS.with(|count| count.set(0));
    let (one_less, overflowed) =
        crate::bounded_output::with_limit(required - 1, || prepare_final_publish(output));
    assert!(!overflowed);
    assert!(matches!(one_less, Err(PhaseBLocalError::BuilderBudget)));
    assert_eq!(PHASE_B_PUBLISH_PLANS.with(std::cell::Cell::get), 0);
    assert!(!PHASE_B_EFFECT_STARTED.with(std::cell::Cell::get));
    assert_eq!(PHASE_B_OUTPUT_PROBES.with(std::cell::Cell::get), 0);
    assert_eq!(PHASE_B_TOOL_HOLDS.with(std::cell::Cell::get), 0);
    assert_eq!(PHASE_B_TOOL_PROCESSES.with(std::cell::Cell::get), 0);
}
