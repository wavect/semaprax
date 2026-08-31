//! Authored, unrun archive-boundary evidence; no compiler or archiver is run.

#[test]
fn provider_compilation_preserves_the_fixed_windows_process_budget() {
    use crate::HostTarget;

    assert_eq!(
        super::provider_optimization(HostTarget::X86_64WindowsMsvc),
        1
    );
    for target in [
        HostTarget::X86_64LinuxGnu,
        HostTarget::Aarch64LinuxGnu,
        HostTarget::X86_64Darwin,
        HostTarget::Aarch64Darwin,
    ] {
        assert_eq!(super::provider_optimization(target), 2);
    }
}
use super::*;
use std::cell::RefCell;
use std::fs;
use std::io::Write as _;

const PHASES: [platform::ArchiveToolFailurePhase; 13] = [
    platform::ArchiveToolFailurePhase::Platform,
    platform::ArchiveToolFailurePhase::Preflight,
    platform::ArchiveToolFailurePhase::ScratchCreation,
    platform::ArchiveToolFailurePhase::Process,
    platform::ArchiveToolFailurePhase::ScratchCleanup,
    platform::ArchiveToolFailurePhase::ArchiverRecheck,
    platform::ArchiveToolFailurePhase::WorkingDirectoryRecheck,
    platform::ArchiveToolFailurePhase::InputRecheck,
    platform::ArchiveToolFailurePhase::ProcessOutput,
    platform::ArchiveToolFailurePhase::OutputHold,
    platform::ArchiveToolFailurePhase::ExactArchive,
    platform::ArchiveToolFailurePhase::LaunchPathRecheck,
    platform::ArchiveToolFailurePhase::OutputRecheck,
];

#[test]
fn uncertain_archive_failures_block_cleanup_recheck_and_publication() {
    for phase in PHASES {
        let mut uncertain = false;
        let primary = archive_failure(
            platform::ArchiveToolFailure {
                error: platform::Error::Changed,
                phase,
                settlement: platform::ArchiveToolSettlement::Uncertain,
            },
            &mut uncertain,
        );
        assert!(uncertain);
        // A later settled observation cannot restore cleanup authority.
        archive_failure(
            platform::ArchiveToolFailure {
                error: platform::Error::Changed,
                phase: platform::ArchiveToolFailurePhase::Preflight,
                settlement: platform::ArchiveToolSettlement::Settled,
            },
            &mut uncertain,
        );
        assert!(uncertain);
        let events = RefCell::new(Vec::new());
        let result = finish_archive_stage(
            Err(primary),
            uncertain,
            || {
                events.borrow_mut().push("cleanup");
                Ok(())
            },
            || {
                events.borrow_mut().push("recheck");
                Ok(())
            },
        )
        .inspect(|_| {
            events.borrow_mut().push("publish");
        });
        assert_eq!(result, Err(PackageError::publication()));
        assert!(events.into_inner().is_empty(), "phase {phase:?}");
    }
    // Even an inconsistent successful payload cannot bypass the sticky latch.
    assert_eq!(
        finish_archive_stage(
            Ok(b"untrusted archive".to_vec()),
            true,
            || panic!("cleanup after uncertainty"),
            || panic!("recheck after uncertainty"),
        ),
        Err(PackageError::publication())
    );
}

#[test]
fn settled_archive_outcomes_keep_cleanup_order_and_sticky_primary() {
    let mut uncertain = false;
    assert_eq!(
        archive_failure(
            platform::ArchiveToolFailure {
                error: platform::Error::Invalid,
                phase: platform::ArchiveToolFailurePhase::Preflight,
                settlement: platform::ArchiveToolSettlement::Settled,
            },
            &mut uncertain,
        ),
        PackageError::publication()
    );
    assert!(!uncertain);
    let primary = PackageError::provider();
    for (input, cleanup_ok, recheck_ok, expected, expected_events) in [
        (Err(primary), true, true, Err(primary), vec!["cleanup"]),
        (Err(primary), false, true, Err(primary), vec!["cleanup"]),
        (
            Ok(vec![7]),
            false,
            true,
            Err(PackageError::publication()),
            vec!["cleanup"],
        ),
        (
            Ok(vec![7]),
            true,
            false,
            Err(PackageError::publication()),
            vec!["cleanup", "recheck"],
        ),
        (
            Ok(vec![7]),
            true,
            true,
            Ok(vec![7]),
            vec!["cleanup", "recheck", "publish"],
        ),
    ] {
        let events = RefCell::new(Vec::new());
        let result = finish_archive_stage(
            input,
            uncertain,
            || {
                events.borrow_mut().push("cleanup");
                cleanup_ok.then_some(()).ok_or(platform::Error::Changed)
            },
            || {
                events.borrow_mut().push("recheck");
                recheck_ok
                    .then_some(())
                    .ok_or_else(PackageError::publication)
            },
        )
        .inspect(|_| {
            events.borrow_mut().push("publish");
        });
        assert_eq!(result, expected);
        assert_eq!(events.into_inner(), expected_events);
    }
}

fn names(path: &Path) -> Vec<String> {
    let metadata = fs::symlink_metadata(path).unwrap();
    assert!(metadata.is_dir() && !metadata.file_type().is_symlink());
    no_reparse(&metadata);
    let mut names = fs::read_dir(path)
        .unwrap()
        .map(|row| row.unwrap().file_name().into_string().unwrap())
        .collect::<Vec<_>>();
    names.sort();
    names
}

fn no_reparse(metadata: &fs::Metadata) {
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt as _;
        assert_eq!(metadata.file_attributes() & 0x400, 0);
    }
    #[cfg(not(windows))]
    let _ = metadata;
}

#[test]
fn real_held_stage_is_preserved_on_uncertainty_and_settled_otherwise() {
    for (uncertain_failure, success, foreign) in [
        (true, false, false),
        (true, false, true),
        (false, false, false),
        (false, true, false),
    ] {
        let root = std::env::temp_dir().join(format!(
            "semaprax-owned-archive-settlement-{}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos(),
            STAGE_NONCE.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&root).unwrap();
        let root = root.canonicalize().unwrap();
        eprintln!("retained archive boundary fixture: {}", root.display());
        let authority = PublicationAuthority::new(&root.join("sdk")).unwrap();
        let stage_name = platform::prepare_stage_name(OsStr::new("stage")).unwrap();
        let stage =
            platform::create_directory_new_prepared(&authority.parent, &stage_name, 0o700).unwrap();
        let mut inventory = platform::prepare_discard_inventory([
            OsStr::new("provider.c"),
            OsStr::new("module.obj"),
            OsStr::new("module.a"),
        ])
        .unwrap();
        let files: [(&str, &[u8]); 3] = [
            ("provider.c", b"provider input"),
            ("module.obj", b"object input"),
            ("module.a", b"archive output"),
        ];
        let attached = if success { 3 } else { 2 };
        for (name, bytes) in &files[..attached] {
            platform::write_file_new_prepared(&stage, &mut inventory, name, bytes, 0o600).unwrap();
        }
        if foreign {
            fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(root.join("stage/foreign"))
                .unwrap()
                .write_all(b"foreign sentinel")
                .unwrap();
        }
        let mut uncertain = false;
        let result = if success {
            Ok(b"archive output".to_vec())
        } else {
            Err(archive_failure(
                platform::ArchiveToolFailure {
                    error: platform::Error::Changed,
                    phase: if uncertain_failure {
                        platform::ArchiveToolFailurePhase::Process
                    } else {
                        platform::ArchiveToolFailurePhase::Preflight
                    },
                    settlement: if uncertain_failure {
                        platform::ArchiveToolSettlement::Uncertain
                    } else {
                        platform::ArchiveToolSettlement::Settled
                    },
                },
                &mut uncertain,
            ))
        };
        let events = RefCell::new(Vec::new());
        let result = finish_archive_stage(
            result,
            uncertain,
            || {
                events.borrow_mut().push("cleanup");
                platform::discard_owned_stage_prepared(
                    &authority.parent,
                    &stage,
                    &stage_name,
                    &inventory,
                )
            },
            || {
                events.borrow_mut().push("recheck");
                authority.recheck()
            },
        );
        assert_eq!(
            result,
            if success {
                Ok(b"archive output".to_vec())
            } else {
                Err(PackageError::publication())
            }
        );
        assert_eq!(
            events.into_inner(),
            if uncertain {
                vec![]
            } else if success {
                vec!["cleanup", "recheck"]
            } else {
                vec!["cleanup"]
            }
        );
        // Releasing handles is not a second cleanup path. Inspect afterwards
        // so Windows sharing restrictions do not weaken the exact-byte oracle.
        drop(inventory);
        drop(stage);
        drop(authority);
        if uncertain {
            assert_eq!(names(&root), ["stage"]);
            let mut expected = vec!["module.obj", "provider.c"];
            if foreign {
                expected.insert(0, "foreign");
            }
            assert_eq!(names(&root.join("stage")), expected);
            for (name, bytes) in files[..attached]
                .iter()
                .copied()
                .chain(foreign.then_some(("foreign", b"foreign sentinel".as_slice())))
            {
                let path = root.join("stage").join(name);
                let metadata = fs::symlink_metadata(&path).unwrap();
                assert!(metadata.is_file() && !metadata.file_type().is_symlink());
                no_reparse(&metadata);
                assert_eq!(fs::read(path).unwrap(), bytes);
            }
        } else {
            assert!(names(&root).is_empty());
        }
        // All fixture roots are deliberately retained. This proves the shared
        // archive-result boundary, not process quiescence or archiver behavior.
    }
}
