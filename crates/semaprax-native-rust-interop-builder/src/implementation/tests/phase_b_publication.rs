//! Phase-B publication: pre-effect race hooks, sticky publish failures,
//! and the exact final inventory and stage-name proofs.

use super::*;

#[test]
fn build_race_hooks_reject_each_pre_effect_mutation_and_preserve_foreign_bytes() {
    use std::io::Write as _;

    let points = [
        NativeRustBuildPoint::BeforeClang,
        NativeRustBuildPoint::BeforeRustLink,
        NativeRustBuildPoint::BeforeExecutableAuthentication,
        NativeRustBuildPoint::BeforeExecute,
        NativeRustBuildPoint::BeforeObjectRead,
        NativeRustBuildPoint::BeforeManifestPublish,
        NativeRustBuildPoint::BeforeBundlePublish,
    ];
    let complete_order = [
        NativeRustBuildPoint::BeforeClang,
        NativeRustBuildPoint::BeforeRustLink,
        NativeRustBuildPoint::BeforeExecutableAuthentication,
        NativeRustBuildPoint::BeforeExecute,
        NativeRustBuildPoint::BeforeExecutableAuthentication,
        NativeRustBuildPoint::BeforeExecute,
        NativeRustBuildPoint::BeforeObjectRead,
        NativeRustBuildPoint::BeforeManifestPublish,
        NativeRustBuildPoint::BeforeBundlePublish,
    ];
    let (program, spec) = fixture();
    for (index, selected) in points.into_iter().enumerate() {
        let root = std::fs::canonicalize(std::env::temp_dir())
            .unwrap()
            .join(format!(
                "semaprax-native-rust-race-hook-{}-{index}",
                std::process::id()
            ));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir(&root).unwrap();
        let output = root.join("bundle");
        let mut fired = false;
        let mut observed = Vec::with_capacity(complete_order.len());
        PHASE_B_DISCARD_ATTEMPTS.with(|attempts| attempts.set(0));
        PHASE_B_INVENTORY_EXACT_PLANS.with(|count| count.set(0));
        PHASE_B_INVENTORY_EXACT_SCANS.with(|count| count.set(0));
        PHASE_B_PUBLISH_PLANS.with(|count| count.set(0));
        PHASE_B_PUBLISH_CONSUMPTIONS.with(|count| count.set(0));
        reset_phase_b_object_authority_observer();
        reset_phase_b_manifest_authority_observer();
        let result = build_native_rust_interop_bundle_with_hook(
            &program,
            spec.as_bytes(),
            &output,
            |point, publish, run, final_output| {
                observed.push(point);
                if point != selected {
                    return;
                }
                assert!(!fired, "hook {selected:?} fired more than once");
                fired = true;
                match point {
                    NativeRustBuildPoint::BeforeClang => {
                        append_hostile(&publish.join("module.c"));
                    }
                    NativeRustBuildPoint::BeforeRustLink => {
                        append_hostile(&run.join("__semaprax_native_rust_link.rs"));
                    }
                    NativeRustBuildPoint::BeforeExecutableAuthentication => {
                        append_hostile(&run.join("__semaprax_native_rust_main.o"));
                    }
                    NativeRustBuildPoint::BeforeExecute => {
                        append_hostile(&run.join(if cfg!(windows) {
                            "__semaprax_native_rust_link_O0.exe"
                        } else {
                            "__semaprax_native_rust_link_O0"
                        }));
                    }
                    NativeRustBuildPoint::BeforeObjectRead => {
                        append_hostile(&publish.join(if cfg!(windows) {
                            "module.obj"
                        } else {
                            "module.o"
                        }));
                    }
                    NativeRustBuildPoint::BeforeManifestPublish => {
                        std::fs::write(publish.join("foreign-sentinel"), b"foreign").unwrap();
                    }
                    NativeRustBuildPoint::BeforeBundlePublish => {
                        std::fs::create_dir(final_output).unwrap();
                        std::fs::write(final_output.join("foreign-sentinel"), b"foreign").unwrap();
                    }
                }
            },
        );
        assert!(fired, "hook {selected:?} was not reached");
        let selected_index = complete_order
            .iter()
            .position(|point| *point == selected)
            .unwrap();
        assert_eq!(
            observed,
            complete_order[..=selected_index],
            "hook {selected:?} allowed a later action or skipped an earlier action",
        );
        let error = match result {
            Ok(_) => panic!("hostile hook {selected:?} unexpectedly published"),
            Err(error) => error,
        };
        assert_eq!(error.len(), 1);
        assert_eq!(
            error[0].code,
            if selected == NativeRustBuildPoint::BeforeExecute {
                "SPX-I231"
            } else {
                "SPX-I232"
            }
        );
        let carrier = if selected == NativeRustBuildPoint::BeforeExecute {
            PhaseBLocalError::Link
        } else {
            PhaseBLocalError::Publication
        };
        assert_eq!(
            error[0].message.as_ptr() as usize,
            PHASE_B_PREPARED_CARRIER_IDENTITIES.with(std::cell::Cell::get)[carrier.index()],
            "hook {selected:?} did not return its pre-effect carrier",
        );
        assert_eq!(
            PHASE_B_POST_EFFECT_ERROR_MATERIALIZATIONS.with(std::cell::Cell::get),
            0,
            "hook {selected:?} materialized an error after effects",
        );
        assert_eq!(
            PHASE_B_DISCARD_ATTEMPTS.with(std::cell::Cell::get),
            2,
            "hook {selected:?} did not attempt run and publish settlement exactly once",
        );
        assert_eq!(PHASE_B_INVENTORY_EXACT_PLANS.with(std::cell::Cell::get), 1);
        assert_eq!(
            PHASE_B_INVENTORY_EXACT_SCANS.with(std::cell::Cell::get),
            match selected {
                NativeRustBuildPoint::BeforeManifestPublish => 1,
                NativeRustBuildPoint::BeforeBundlePublish => 2,
                _ => 0,
            },
            "hook {selected:?} crossed an unexpected exact-inventory scan boundary",
        );
        assert_eq!(PHASE_B_PUBLISH_PLANS.with(std::cell::Cell::get), 1);
        assert_eq!(
            PHASE_B_PUBLISH_CONSUMPTIONS.with(std::cell::Cell::get),
            usize::from(selected == NativeRustBuildPoint::BeforeBundlePublish),
            "hook {selected:?} consumed final publication unexpectedly",
        );
        let expected_transfer = usize::from(selected != NativeRustBuildPoint::BeforeClang);
        assert_eq!(
            PHASE_B_OBJECT_AUTHORITY_TRANSFERS.with(std::cell::Cell::get),
            expected_transfer,
            "hook {selected:?} transferred the O2 authority at the wrong boundary",
        );
        assert_eq!(
            PHASE_B_OBJECT_AUTHORITY_DROPS.with(std::cell::Cell::get),
            expected_transfer,
            "hook {selected:?} did not release the O2 authority exactly once",
        );
        let expected_outer_observation =
            usize::from(selected == NativeRustBuildPoint::BeforeBundlePublish);
        assert_eq!(
            PHASE_B_OBJECT_AUTHORITY_MANIFEST_OBSERVATIONS.with(std::cell::Cell::get),
            expected_outer_observation,
        );
        assert_eq!(
            PHASE_B_OBJECT_AUTHORITY_PUBLISH_OBSERVATIONS.with(std::cell::Cell::get),
            expected_outer_observation,
        );
        assert_phase_b_object_drop_order(expected_transfer);
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
        if selected == NativeRustBuildPoint::BeforeBundlePublish {
            assert_eq!(
                std::fs::read(output.join("foreign-sentinel")).unwrap(),
                b"foreign"
            );
        } else {
            assert!(!output.exists());
        }
        if selected == NativeRustBuildPoint::BeforeManifestPublish {
            let sentinel = std::fs::read_dir(&root)
                .unwrap()
                .filter_map(Result::ok)
                .map(|entry| entry.path().join("foreign-sentinel"))
                .find(|path| path.is_file())
                .unwrap();
            assert_eq!(std::fs::read(sentinel).unwrap(), b"foreign");
        }
        let stages = std::fs::read_dir(&root)
            .unwrap()
            .filter_map(Result::ok)
            .filter(|entry| {
                entry
                    .file_name()
                    .to_string_lossy()
                    .starts_with(".semaprax-native-rust-interop-")
            })
            .map(|entry| entry.path())
            .collect::<Vec<_>>();
        match selected {
            NativeRustBuildPoint::BeforeManifestPublish => {
                assert_eq!(
                    stages.len(),
                    1,
                    "foreign inventory uncertainty must leave one inert stage"
                );
            }
            NativeRustBuildPoint::BeforeClang
            | NativeRustBuildPoint::BeforeRustLink
            | NativeRustBuildPoint::BeforeExecutableAuthentication
            | NativeRustBuildPoint::BeforeExecute => {
                assert_eq!(
                    stages.len(),
                    1,
                    "mutated held bytes must leave one inert stage rather than deleting uncertain data"
                );
            }
            NativeRustBuildPoint::BeforeObjectRead => {
                let expected = if cfg!(any(target_os = "linux", windows)) {
                    2
                } else {
                    1
                };
                assert_eq!(
                    stages.len(),
                    expected,
                    "mutating a hard-linked object must preserve both uncertain stages"
                );
            }
            NativeRustBuildPoint::BeforeBundlePublish => {
                assert!(
                    stages.is_empty(),
                    "owned publish stage must settle when only the foreign final output conflicts"
                );
            }
        }
        std::fs::remove_dir_all(&root).unwrap();
    }

    fn append_hostile(path: &Path) {
        std::fs::OpenOptions::new()
            .append(true)
            .open(path)
            .unwrap()
            .write_all(b"hostile")
            .unwrap();
    }
}

#[cfg(debug_assertions)]
#[test]
fn phase_b_prepared_publish_open_information_and_rename_failures_are_sticky() {
    let (program, spec) = fixture();
    for point in [1_u8, 2, 4] {
        let root = std::fs::canonicalize(std::env::temp_dir())
            .unwrap()
            .join(format!(
                "semaprax-native-rust-publish-failure-{}-{point}",
                std::process::id()
            ));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir(&root).unwrap();
        let output = root.join("bundle");
        PHASE_B_PUBLISH_FAILURE.with(|selected| selected.set(point));
        PHASE_B_DISCARD_ATTEMPTS.with(|count| count.set(0));
        PHASE_B_PUBLISH_PLANS.with(|count| count.set(0));
        PHASE_B_PUBLISH_CONSUMPTIONS.with(|count| count.set(0));
        let mut reached_publish = false;
        let result = build_native_rust_interop_bundle_with_hook(
            &program,
            spec.as_bytes(),
            &output,
            |boundary, _, _, _| {
                if boundary == NativeRustBuildPoint::BeforeBundlePublish {
                    reached_publish = true;
                }
            },
        );
        PHASE_B_PUBLISH_FAILURE.with(|selected| selected.set(0));
        assert!(reached_publish);
        let diagnostics = match result {
            Ok(_) => panic!("injected publish failure {point} unexpectedly succeeded"),
            Err(diagnostics) => diagnostics,
        };
        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].code, "SPX-I232");
        assert_eq!(
            diagnostics[0].message.as_ptr() as usize,
            PHASE_B_PREPARED_CARRIER_IDENTITIES.with(std::cell::Cell::get)
                [PhaseBLocalError::Publication.index()]
        );
        assert_eq!(
            PHASE_B_POST_EFFECT_ERROR_MATERIALIZATIONS.with(std::cell::Cell::get),
            0
        );
        assert_eq!(PHASE_B_DISCARD_ATTEMPTS.with(std::cell::Cell::get), 2);
        assert_eq!(PHASE_B_PUBLISH_PLANS.with(std::cell::Cell::get), 1);
        assert_eq!(PHASE_B_PUBLISH_CONSUMPTIONS.with(std::cell::Cell::get), 1);
        assert!(!output.exists());
        assert!(
            std::fs::read_dir(&root)
                .unwrap()
                .filter_map(Result::ok)
                .all(|entry| !entry
                    .file_name()
                    .to_string_lossy()
                    .starts_with(".semaprax-native-rust-interop-")),
            "owned stages must settle after injected publish failure {point}"
        );
        std::fs::remove_dir_all(root).unwrap();
    }
}

#[cfg(debug_assertions)]
#[test]
fn phase_b_prepared_publish_close_failure_child() {
    let Ok(root) = std::env::var("SEMAPRAX_PUBLISH_CLOSE_FAILURE_ROOT") else {
        return;
    };
    let root = Path::new(&root);
    let (program, spec) = fixture();
    PHASE_B_PUBLISH_FAILURE.with(|selected| selected.set(3));
    let _ = build_native_rust_interop_bundle_with_hook(
        &program,
        spec.as_bytes(),
        &root.join("bundle"),
        |_, _, _, _| {},
    );
    std::fs::write(root.join("later-action"), b"must not exist").unwrap();
}

#[cfg(debug_assertions)]
#[test]
fn phase_b_prepared_publish_close_uncertainty_is_fail_stop() {
    let root = std::fs::canonicalize(std::env::temp_dir())
        .unwrap()
        .join(format!(
            "semaprax-native-rust-publish-close-{}",
            std::process::id()
        ));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir(&root).unwrap();
    let status = Command::new(std::env::current_exe().unwrap())
        .arg("--exact")
        .arg("implementation::tests::phase_b_publication::phase_b_prepared_publish_close_failure_child")
        .arg("--nocapture")
        .env("SEMAPRAX_PUBLISH_CLOSE_FAILURE_ROOT", &root)
        .status()
        .unwrap();
    assert!(!status.success());
    assert!(!root.join("later-action").exists());
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn phase_b_final_comparisons_precede_scan_and_allocate_no_file_buffers() {
    let source = IMPLEMENTATION_SOURCE;
    let start = source.find("fn publish_stage_platform").unwrap();
    let publish = &source[start..];
    let comparison = publish.find("platform::compare_exact").unwrap();
    let scan = publish.find("scan_publish_inventory_exact").unwrap();
    let settle = publish.find(".settle_for_publish()").unwrap();
    let rename = publish
        .find("platform::publish_directory_new_prepared")
        .unwrap();
    assert!(comparison < scan && scan < settle && settle < rename);
    assert!(publish.contains("platform::FILE_COMPARE_SCRATCH_BYTES"));
    assert!(publish.contains(".discard_name"));
    assert!(!publish.contains("platform::read_exact"));
    assert!(!publish.contains("debit_phase_b"));
    assert!(!publish.contains("try_clone"));
}

#[cfg(unix)]
#[test]
fn phase_b_inventory_exact_rejects_every_missing_and_substituted_slot_and_extra() {
    const NAMES: [&str; 7] = [
        "descriptor.json",
        "module.c",
        "semaprax_native_rust_interop.h",
        "semaprax_native_rust_interop.rs",
        "semaprax_native_rust_interop_ffi.rs",
        "module.o",
        "semaprax.native-rust-interop.json",
    ];

    fn fixture(
        root: &Path,
    ) -> (
        platform::HeldDirectory,
        PublishDiscardInventory,
        platform::PreparedInventoryExact<7>,
    ) {
        std::fs::create_dir(root).unwrap();
        let directory = platform::hold_directory(root).unwrap();
        let mut inventory = prepare_publish_discard_inventory().unwrap();
        let prepared = platform::prepare_inventory_exact(&inventory).unwrap();
        for (index, name) in NAMES.iter().enumerate() {
            platform::write_file_new_prepared(
                &directory,
                &mut inventory,
                name,
                &[u8::try_from(index).unwrap()],
                0o600,
            )
            .unwrap();
        }
        (directory, inventory, prepared)
    }

    let base = std::fs::canonicalize(std::env::temp_dir())
        .unwrap()
        .join(format!(
            "semaprax-native-rust-inventory-exact-{}",
            std::process::id()
        ));
    let _ = std::fs::remove_dir_all(&base);
    std::fs::create_dir(&base).unwrap();

    for (index, name) in NAMES.iter().enumerate() {
        let root = base.join(format!("missing-{index}"));
        let (directory, inventory, mut prepared) = fixture(&root);
        std::fs::remove_file(root.join(name)).unwrap();
        assert!(platform::inventory_exact_prepared(&mut prepared, &directory, &inventory).is_err());
        drop((prepared, inventory, directory));
        std::fs::remove_dir_all(&root).unwrap();
    }

    for (index, name) in NAMES.iter().enumerate() {
        let root = base.join(format!("substituted-{index}"));
        let (directory, inventory, mut prepared) = fixture(&root);
        std::fs::rename(root.join(name), root.join(format!("tracked-{index}"))).unwrap();
        std::fs::write(root.join(name), b"foreign").unwrap();
        assert!(platform::inventory_exact_prepared(&mut prepared, &directory, &inventory).is_err());
        assert_eq!(std::fs::read(root.join(name)).unwrap(), b"foreign");
        drop((prepared, inventory, directory));
        std::fs::remove_dir_all(&root).unwrap();
    }

    let root = base.join("extra");
    let (directory, inventory, mut prepared) = fixture(&root);
    std::fs::write(root.join("foreign-extra"), b"foreign").unwrap();
    assert!(platform::inventory_exact_prepared(&mut prepared, &directory, &inventory).is_err());
    drop((prepared, inventory, directory));
    std::fs::remove_dir_all(&root).unwrap();

    #[cfg(target_os = "linux")]
    {
        let root = base.join("invalid-encoding");
        let (directory, inventory, mut prepared) = fixture(&root);
        use std::os::unix::ffi::OsStringExt as _;
        std::fs::write(
            root.join(std::ffi::OsString::from_vec(vec![0xff])),
            b"foreign",
        )
        .unwrap();
        assert!(platform::inventory_exact_prepared(&mut prepared, &directory, &inventory).is_err());
        drop((prepared, inventory, directory));
        std::fs::remove_dir_all(&root).unwrap();
    }
    std::fs::remove_dir_all(&base).unwrap();
}

#[cfg(unix)]
#[test]
fn phase_b_inventory_exact_is_bound_to_one_inventory_and_exactly_two_scans() {
    const NAMES: [&str; 7] = [
        "descriptor.json",
        "module.c",
        "semaprax_native_rust_interop.h",
        "semaprax_native_rust_interop.rs",
        "semaprax_native_rust_interop_ffi.rs",
        "module.o",
        "semaprax.native-rust-interop.json",
    ];
    let base = std::fs::canonicalize(std::env::temp_dir())
        .unwrap()
        .join(format!(
            "semaprax-native-rust-inventory-binding-{}",
            std::process::id()
        ));
    let _ = std::fs::remove_dir_all(&base);
    std::fs::create_dir(&base).unwrap();
    let mut fixtures = Vec::with_capacity(2);
    for suffix in ["a", "b"] {
        let root = base.join(suffix);
        std::fs::create_dir(&root).unwrap();
        let directory = platform::hold_directory(&root).unwrap();
        let mut inventory = prepare_publish_discard_inventory().unwrap();
        for (index, name) in NAMES.iter().enumerate() {
            platform::write_file_new_prepared(
                &directory,
                &mut inventory,
                name,
                &[u8::try_from(index).unwrap()],
                0o600,
            )
            .unwrap();
        }
        fixtures.push((root, directory, inventory));
    }
    let mut prepared = platform::prepare_inventory_exact(&fixtures[0].2).unwrap();
    assert!(
        platform::inventory_exact_prepared(&mut prepared, &fixtures[1].1, &fixtures[1].2).is_err()
    );
    assert_eq!(platform::prepared_inventory_exact_remaining(&prepared), 2);
    assert!(
        platform::inventory_exact_prepared(&mut prepared, &fixtures[0].1, &fixtures[0].2).is_ok()
    );
    assert_eq!(platform::prepared_inventory_exact_remaining(&prepared), 1);
    assert!(
        platform::inventory_exact_prepared(&mut prepared, &fixtures[0].1, &fixtures[0].2).is_ok()
    );
    assert_eq!(platform::prepared_inventory_exact_remaining(&prepared), 0);
    assert!(
        platform::inventory_exact_prepared(&mut prepared, &fixtures[0].1, &fixtures[0].2).is_err()
    );
    let hardlink_root = base.join("hardlink-second-directory");
    std::fs::create_dir(&hardlink_root).unwrap();
    for name in NAMES {
        std::fs::hard_link(fixtures[0].0.join(name), hardlink_root.join(name)).unwrap();
    }
    let hardlink_directory = platform::hold_directory(&hardlink_root).unwrap();
    let mut hardlink_prepared = platform::prepare_inventory_exact(&fixtures[0].2).unwrap();
    assert!(platform::inventory_exact_prepared(
        &mut hardlink_prepared,
        &fixtures[0].1,
        &fixtures[0].2
    )
    .is_ok());
    assert_eq!(
        platform::prepared_inventory_exact_remaining(&hardlink_prepared),
        1
    );
    assert!(platform::inventory_exact_prepared(
        &mut hardlink_prepared,
        &hardlink_directory,
        &fixtures[0].2
    )
    .is_err());
    assert_eq!(
        platform::prepared_inventory_exact_remaining(&hardlink_prepared),
        1
    );
    let mut consumed_failure = platform::prepare_inventory_exact(&fixtures[1].2).unwrap();
    assert!(platform::inventory_exact_prepared(
        &mut consumed_failure,
        &fixtures[1].1,
        &fixtures[1].2
    )
    .is_ok());
    assert_eq!(
        platform::prepared_inventory_exact_remaining(&consumed_failure),
        1
    );
    for name in NAMES {
        std::fs::remove_file(fixtures[1].0.join(name)).unwrap();
    }
    assert!(platform::inventory_exact_prepared(
        &mut consumed_failure,
        &fixtures[1].1,
        &fixtures[1].2
    )
    .is_err());
    assert_eq!(
        platform::prepared_inventory_exact_remaining(&consumed_failure),
        0
    );
    drop((
        consumed_failure,
        hardlink_prepared,
        hardlink_directory,
        prepared,
        fixtures,
    ));
    std::fs::remove_dir_all(&base).unwrap();
}

#[test]
fn phase_b_nonce_exists_retries_reuse_native_stage_name_arena_without_materialization() {
    let root = std::fs::canonicalize(std::env::temp_dir())
        .unwrap()
        .join(format!(
            "semaprax-native-rust-nonce-retry-{}",
            std::process::id()
        ));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir(&root).unwrap();
    let parent = hold_stage(root.clone()).unwrap();
    let digest = "sha256:nonce-retry";
    reset_phase_b_native_stage_arena_observer();
    let mut collision = StageSlot::new(&root, digest, "retry").unwrap();
    let native_capacity = collision.native_name.capacity();
    assert_eq!(
        PHASE_B_NATIVE_STAGE_ARENA_ALLOCATIONS.with(std::cell::Cell::get),
        1,
    );
    collision.prepare(&root, 0).unwrap();
    assert_eq!(collision.native_name.capacity(), native_capacity);
    std::fs::create_dir(&collision.path).unwrap();
    let inventory = platform::prepare_discard_inventory([]).unwrap();
    let slot = StageSlot::new(&root, digest, "retry").unwrap();
    assert_eq!(slot.native_name.capacity(), native_capacity);
    assert_eq!(
        PHASE_B_NATIVE_STAGE_ARENA_ALLOCATIONS.with(std::cell::Cell::get),
        2,
    );
    assert_eq!(
        PHASE_B_NATIVE_STAGE_ARENA_SETS.with(std::cell::Cell::get),
        1,
    );
    assert_eq!(
        PHASE_B_NATIVE_STAGE_ARENA_CONSUMPTIONS.with(std::cell::Cell::get),
        0,
    );
    reset_phase_b_error_materialization_observer();
    mark_phase_b_effect_started();
    let stage = create_stage(&parent, slot, &inventory).unwrap();
    assert!(stage
        .path
        .file_name()
        .unwrap()
        .to_string_lossy()
        .ends_with("-1"));
    assert_eq!(
        PHASE_B_POST_EFFECT_ERROR_MATERIALIZATIONS.with(std::cell::Cell::get),
        0,
    );
    assert_eq!(
        PHASE_B_NATIVE_STAGE_ARENA_ALLOCATIONS.with(std::cell::Cell::get),
        2,
        "nonce retries allocated a new native name after effects",
    );
    assert_eq!(
        PHASE_B_NATIVE_STAGE_ARENA_SETS.with(std::cell::Cell::get),
        3,
        "the collision probe plus nonce zero and nonce one must set one arena",
    );
    assert_eq!(
        PHASE_B_NATIVE_STAGE_ARENA_CONSUMPTIONS.with(std::cell::Cell::get),
        2,
        "both create attempts must consume the prepared native arena",
    );
    assert_eq!(
        stage.discard_name.as_ref().unwrap().capacity(),
        native_capacity,
    );
    discard_run_stage(&parent, &stage, &inventory).unwrap();
    drop(stage);
    drop(parent);
    std::fs::remove_dir_all(&root).unwrap();
}

#[cfg(debug_assertions)]
#[test]
fn phase_b_every_mid_discard_failure_moves_prebuilt_i232_without_materialization() {
    const NAMES: [&str; 3] = ["first", "second", "third"];
    for failure_after_delete in 0..=NAMES.len() {
        let root = std::fs::canonicalize(std::env::temp_dir())
            .unwrap()
            .join(format!(
                "semaprax-native-rust-mid-discard-{}-{failure_after_delete}",
                std::process::id()
            ));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir(&root).unwrap();
        let parent = hold_stage(root.clone()).unwrap();
        let mut inventory = platform::prepare_discard_inventory(NAMES.map(OsStr::new)).unwrap();
        let slot = StageSlot::new(&root, "sha256:mid-discard", "discard").unwrap();
        let stage = create_stage(&parent, slot, &inventory).unwrap();
        for name in NAMES {
            platform::write_file_new_prepared(
                stage.authority.held(),
                &mut inventory,
                name,
                b"owned",
                0o600,
            )
            .unwrap();
        }
        inventory.inject_discard_failure_after_delete(Some(failure_after_delete));
        reset_phase_b_error_materialization_observer();
        let mut carriers = PhaseBErrorCarriers::prepare().unwrap();
        mark_phase_b_effect_started();
        assert!(discard_run_stage(&parent, &stage, &inventory).is_err());
        let error = carriers.take(PhaseBLocalError::Publication);
        assert_eq!(error.len(), 1);
        assert_eq!(error[0].code, "SPX-I232");
        assert_eq!(error[0].message, PHASE_B_PUBLICATION_MESSAGE);
        assert_eq!(
            PHASE_B_POST_EFFECT_ERROR_MATERIALIZATIONS.with(std::cell::Cell::get),
            0,
            "delete boundary {failure_after_delete} materialized an error after effects",
        );
        drop(error);
        drop(inventory);
        drop(stage);
        drop(parent);
        std::fs::remove_dir_all(&root).unwrap();
    }
}
