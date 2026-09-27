use super::super::{
    unix::{Point, FAIL},
    HeldTrustStore as V1,
};
use super::*;
use crate::package_registry::trust::registry_v3::tests::{
    host_fixture, host_rotation_fixture, Admission,
};
use std::os::unix::fs::{symlink, PermissionsExt};
use std::sync::atomic::{AtomicU64, Ordering};
#[path = "cache_tests.rs"]
mod cache_tests;
#[path = "mirror_acceptance_tests.rs"]
mod mirror_acceptance_tests;
#[path = "read_tests.rs"]
mod read_tests;
static SERIAL: AtomicU64 = AtomicU64::new(0);
struct Temp(std::path::PathBuf);
impl Temp {
    fn new() -> Self {
        let path = std::env::temp_dir().canonicalize().unwrap().join(format!(
            "semaprax-registry-v3-host-{}-{}",
            std::process::id(),
            SERIAL.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir(&path).unwrap();
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o700)).unwrap();
        Self(path)
    }
}
impl Drop for Temp {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}
fn code<T>(result: Result<T>, expected: &str) {
    match result {
        Ok(_) => panic!("unexpected acceptance"),
        Err(error) => assert_eq!(error.code, expected),
    }
}
struct Fixture {
    root: String,
    pin: String,
    timestamp: String,
    snapshot: String,
    publishers: [String; 2],
    admitted: &'static Admission,
}
impl Fixture {
    fn new(yanked: bool, version: u64) -> Self {
        let (root, timestamp, snapshot, publishers, admitted) = host_fixture(yanked, version);
        Self {
            pin: hash(root.as_bytes()),
            root,
            timestamp,
            snapshot,
            publishers,
            admitted,
        }
    }
    fn expired_time(&self) -> u64 {
        serde_json::from_str::<serde_json::Value>(&self.root).unwrap()["expires"]
            .as_u64()
            .unwrap()
    }
    fn update<'a>(
        &'a self,
        publishers: &'a [(&'a str, &'a str)],
        artifacts: &'a [Artifact<'a>],
    ) -> Update<'a, 'a> {
        Update {
            metadata: proof::UpdateInputs {
                timestamp: &self.timestamp,
                snapshot: &self.snapshot,
                publishers,
                registry: &self.admitted.0,
            },
            rotation: None,
            lock: &self.admitted.2,
            subjects: &self.admitted.1,
            artifacts,
            trusted_time: 100,
        }
    }
    fn publishers(&self) -> [(&str, &str); 2] {
        [
            ("publisher-app", &self.publishers[0]),
            ("publisher-lib", &self.publishers[1]),
        ]
    }
    fn artifacts(&self) -> [Artifact<'_>; 2] {
        [
            Artifact {
                package: "app.root",
                version: "1.0.0",
                path: "module.wasm",
                bytes: &self.admitted.3,
            },
            Artifact {
                package: "lib.leaf",
                version: "1.0.0",
                path: "module.wasm",
                bytes: &self.admitted.4,
            },
        ]
    }
}
fn inventory(temp: &Temp) -> Vec<(String, Vec<u8>)> {
    let mut values = std::fs::read_dir(&temp.0)
        .unwrap()
        .map(|entry| {
            let entry = entry.unwrap();
            (
                entry.file_name().to_str().unwrap().to_owned(),
                std::fs::read(entry.path()).unwrap(),
            )
        })
        .collect::<Vec<_>>();
    values.sort();
    values
}

#[test]
fn genuine_signed_root_leaf_lock_artifacts_commit_and_reopen() {
    let f = Fixture::new(false, 1);
    let temp = Temp::new();
    let publishers = f.publishers();
    let artifacts = f.artifacts();
    let update = f.update(&publishers, &artifacts);
    let mut store = HeldTrustStore::install(&temp.0, &f.root, &f.pin, 90).unwrap();
    let before = store.receipt().generation_digest;
    let receipt = store.commit_update(&update).unwrap();
    assert_ne!(receipt.generation_digest, before);
    drop(store);
    let opened = HeldTrustStore::open(&temp.0, &f.pin).unwrap();
    assert_eq!(
        opened.receipt().generation_digest,
        receipt.generation_digest
    );
    let active = std::fs::read_to_string(temp.0.join("ACTIVE")).unwrap();
    let value: serde_json::Value =
        serde_json::from_slice(&std::fs::read(temp.0.join(active)).unwrap()).unwrap();
    assert_eq!(value["cache"]["lock"], f.admitted.2);
    assert_eq!(value["cache"]["artifacts"].as_array().unwrap().len(), 2);
    assert_eq!(
        receipt.checkpoint_digest,
        hash(value["checkpoint"].as_str().unwrap().as_bytes())
    );
    let other = Temp::new();
    let mut permuted = HeldTrustStore::install(&other.0, &f.root, &f.pin, 90).unwrap();
    let publishers = [publishers[1], publishers[0]];
    let artifacts = [Artifact { ..artifacts[1] }, Artifact { ..artifacts[0] }];
    let mut subjects = f.admitted.1.clone();
    subjects.reverse();
    let mut update = f.update(&publishers, &artifacts);
    update.subjects = &subjects;
    assert_eq!(
        permuted.commit_update(&update).unwrap().generation_digest,
        receipt.generation_digest
    );
}

#[test]
fn pin_permissions_links_exclusivity_and_v1_downgrade_refuse() {
    let f = Fixture::new(false, 1);
    let temp = Temp::new();
    code(
        HeldTrustStore::install(&temp.0, &f.root, "sha256:wrong", 90),
        "SPX-PKR626",
    );
    assert!(inventory(&temp).is_empty());
    let store = HeldTrustStore::install(&temp.0, &f.root, &f.pin, 90).unwrap();
    code(HeldTrustStore::open(&temp.0, &f.pin), "SPX-PKR626");
    drop(store);
    assert!(V1::open(&temp.0, &f.pin).is_err());
    assert!(HeldTrustStore::open(&temp.0, "sha256:wrong").is_err());
    let alias = Temp::new();
    symlink(&temp.0, alias.0.join("alias")).unwrap();
    code(
        HeldTrustStore::open(&alias.0.join("alias"), &f.pin),
        "SPX-PKR626",
    );
    std::fs::set_permissions(&temp.0, std::fs::Permissions::from_mode(0o750)).unwrap();
    code(HeldTrustStore::open(&temp.0, &f.pin), "SPX-PKR626");
}

#[test]
fn full_lock_artifact_and_time_refusals_have_no_effects() {
    let f = Fixture::new(false, 1);
    let temp = Temp::new();
    let publishers = f.publishers();
    let artifacts = f.artifacts();
    let mut update = f.update(&publishers, &artifacts);
    let mut store = HeldTrustStore::install(&temp.0, &f.root, &f.pin, 90).unwrap();
    let before = inventory(&temp);
    update.subjects = &f.admitted.1[..1];
    assert!(store.commit_update(&update).is_err());
    update.subjects = &f.admitted.1;
    update.lock = "{}";
    assert!(store.commit_update(&update).is_err());
    update.lock = &f.admitted.2;
    update.artifacts = &[];
    code(store.commit_update(&update), "SPX-PKR626");
    update.artifacts = &artifacts[..1];
    code(store.commit_update(&update), "SPX-PKR626");
    let duplicate = [
        Artifact { ..artifacts[0] },
        Artifact { ..artifacts[0] },
        Artifact { ..artifacts[1] },
    ];
    update.artifacts = &duplicate;
    code(store.commit_update(&update), "SPX-PKR626");
    let extra = [
        Artifact { ..artifacts[0] },
        Artifact { ..artifacts[1] },
        Artifact {
            path: "unknown.wasm",
            ..artifacts[0]
        },
    ];
    update.artifacts = &extra;
    code(store.commit_update(&update), "SPX-PKR624");
    update.artifacts = &artifacts;
    update.trusted_time = 89;
    code(store.commit_update(&update), "SPX-PKR623");
    update.trusted_time = f.expired_time();
    code(store.commit_update(&update), "SPX-PKR623");
    update.trusted_time = 100;
    let tampered = [Artifact {
        bytes: b"tamper",
        ..artifacts[0]
    }];
    update.artifacts = &tampered;
    code(store.commit_update(&update), "SPX-PKR624");
    assert_eq!(inventory(&temp), before);
}

#[test]
fn signed_yank_and_metadata_rollback_leave_generation_unchanged() {
    let f = Fixture::new(true, 1);
    let temp = Temp::new();
    let publishers = f.publishers();
    let artifacts = f.artifacts();
    let mut store = HeldTrustStore::install(&temp.0, &f.root, &f.pin, 90).unwrap();
    let before = inventory(&temp);
    assert!(store
        .commit_update(&f.update(&publishers, &artifacts))
        .is_err());
    assert_eq!(inventory(&temp), before);
    drop(store);
    let high = Fixture::new(false, 2);
    let publishers = high.publishers();
    let artifacts = high.artifacts();
    let mut store = HeldTrustStore::open(&temp.0, &f.pin).unwrap();
    store
        .commit_update(&high.update(&publishers, &artifacts))
        .unwrap();
    let before = inventory(&temp);
    let low = Fixture::new(false, 1);
    let publishers = low.publishers();
    let artifacts = low.artifacts();
    code(
        store.commit_update(&low.update(&publishers, &artifacts)),
        "SPX-PKR623",
    );
    assert_eq!(inventory(&temp), before);
}

#[test]
fn root_rotation_is_signed_and_replayed_on_open() {
    let mut f = Fixture::new(false, 1);
    let temp = Temp::new();
    let mut store = HeldTrustStore::install(&temp.0, &f.root, &f.pin, 90).unwrap();
    let (rotation, timestamp, snapshot, publishers, admitted) = host_rotation_fixture();
    f.timestamp = timestamp;
    f.snapshot = snapshot;
    f.publishers = publishers;
    f.admitted = admitted;
    let publishers = f.publishers();
    let artifacts = f.artifacts();
    let mut update = f.update(&publishers, &artifacts);
    assert!(store.commit_update(&update).is_err());
    update.rotation = Some(&rotation);
    store.commit_update(&update).unwrap();
    drop(store);
    assert!(HeldTrustStore::open(&temp.0, &f.pin).is_ok());
}

#[test]
fn bootstrap_crash_points_exact_retry_and_partial_stage_fail_stop() {
    let f = Fixture::new(false, 1);
    for point in [
        Point::PartialWrite,
        Point::Staged,
        Point::Generation,
        Point::BeforeActive,
        Point::AfterActive,
    ] {
        let temp = Temp::new();
        FAIL.with(|fail| fail.set(Some(point)));
        code(
            HeldTrustStore::install(&temp.0, &f.root, &f.pin, 90),
            "SPX-PKR627",
        );
        assert!(HeldTrustStore::open(&temp.0, &f.pin).is_err());
        let before = inventory(&temp);
        let recovered = HeldTrustStore::recover_install(&temp.0, &f.root, &f.pin, 90, 100);
        if point == Point::PartialWrite {
            code(recovered, "SPX-PKR627");
            assert_eq!(inventory(&temp), before);
        } else {
            drop(recovered.unwrap());
            assert!(HeldTrustStore::open(&temp.0, &f.pin).is_ok());
        }
    }
}

#[test]
fn update_crash_points_recover_only_exact_fresh_full_request() {
    let f = Fixture::new(false, 1);
    let publishers = f.publishers();
    let artifacts = f.artifacts();
    let update = f.update(&publishers, &artifacts);
    for point in [
        Point::PartialWrite,
        Point::Staged,
        Point::Generation,
        Point::BeforeActive,
        Point::AfterActive,
    ] {
        let temp = Temp::new();
        let mut store = HeldTrustStore::install(&temp.0, &f.root, &f.pin, 90).unwrap();
        let previous = store.receipt().generation_digest;
        FAIL.with(|fail| fail.set(Some(point)));
        code(store.commit_update(&update), "SPX-PKR627");
        drop(store);
        assert!(HeldTrustStore::open(&temp.0, &f.pin).is_err());
        let before = inventory(&temp);
        code(
            HeldTrustStore::recover_update(&temp.0, &f.pin, &previous, &update, f.expired_time()),
            "SPX-PKR623",
        );
        assert_eq!(inventory(&temp), before);
        let mut changed = f.update(&publishers, &artifacts);
        changed.trusted_time = 101;
        assert!(HeldTrustStore::recover_update(&temp.0, &f.pin, &previous, &changed, 102).is_err());
        assert_eq!(inventory(&temp), before);
        let recovered = HeldTrustStore::recover_update(&temp.0, &f.pin, &previous, &update, 101);
        if point == Point::PartialWrite {
            code(recovered, "SPX-PKR627");
            assert_eq!(inventory(&temp), before);
        } else {
            drop(recovered.unwrap());
            assert!(HeldTrustStore::open(&temp.0, &f.pin).is_ok());
        }
    }
}

#[test]
fn generation_tamper_active_rollback_and_unknown_effects_fail_closed() {
    let f = Fixture::new(false, 1);
    let publishers = f.publishers();
    let artifacts = f.artifacts();
    let update = f.update(&publishers, &artifacts);
    for mode in 0..3 {
        let temp = Temp::new();
        let mut store = HeldTrustStore::install(&temp.0, &f.root, &f.pin, 90).unwrap();
        let before = std::fs::read(temp.0.join("ACTIVE")).unwrap();
        let previous = store.receipt().generation_digest;
        store.commit_update(&update).unwrap();
        drop(store);
        match mode {
            0 => {
                let active = std::fs::read_to_string(temp.0.join("ACTIVE")).unwrap();
                std::fs::write(temp.0.join(active), b"tampered").unwrap();
            }
            1 => std::fs::write(temp.0.join("ACTIVE"), before).unwrap(),
            _ => std::fs::write(temp.0.join("unknown"), b"retained").unwrap(),
        }
        assert!(HeldTrustStore::open(&temp.0, &f.pin).is_err());
        let damaged = inventory(&temp);
        if mode != 1 {
            assert!(
                HeldTrustStore::recover_update(&temp.0, &f.pin, &previous, &update, 101).is_err()
            );
            assert_eq!(inventory(&temp), damaged);
        }
    }
}

#[test]
fn explicit_v1_migration_exact_cas_crash_recovery_and_downgrade_refusal() {
    let f = Fixture::new(false, 2);
    let publishers = f.publishers();
    let artifacts = f.artifacts();
    let update = f.update(&publishers, &artifacts);
    for point in [None, Some(Point::Staged), Some(Point::AfterActive)] {
        let temp = Temp::new();
        let (root, timestamp, snapshot, old_publishers, registry, entry) =
            crate::package_registry::trust::tests::host_v3_migration_fixture();
        assert_eq!(root, f.root);
        let old_publishers = [
            ("publisher-app", old_publishers[0].as_str()),
            ("publisher-lib", old_publishers[1].as_str()),
        ];
        let entries = [entry];
        let mut legacy = V1::install(&temp.0, &root, &f.pin, 90).unwrap();
        legacy
            .commit_update(&super::super::Update {
                metadata: crate::package_registry::trust::UpdateInputs {
                    timestamp: &timestamp,
                    snapshot: &snapshot,
                    publishers: &old_publishers,
                    registry_snapshot: &registry,
                    admitted_entries: &entries,
                },
                rotation: None,
                lock: None,
                subjects: &[],
                artifacts: &[],
                trusted_time: 95,
            })
            .unwrap();
        drop(legacy);
        assert!(HeldTrustStore::open(&temp.0, &f.pin).is_err());
        let previous_name = std::fs::read_to_string(temp.0.join("ACTIVE")).unwrap();
        let previous = hash(&std::fs::read(temp.0.join(&previous_name)).unwrap());
        let before = inventory(&temp);
        let low = Fixture::new(false, 1);
        let low_publishers = low.publishers();
        let low_artifacts = low.artifacts();
        code(
            HeldTrustStore::migrate_v1(
                &temp.0,
                &f.pin,
                &previous,
                &low.update(&low_publishers, &low_artifacts),
            ),
            "SPX-PKR623",
        );
        assert_eq!(inventory(&temp), before);
        code(
            HeldTrustStore::migrate_v1(&temp.0, &f.pin, "sha256:wrong", &update),
            "SPX-PKR626",
        );
        assert_eq!(inventory(&temp), before);
        FAIL.with(|fail| fail.set(point));
        let result = HeldTrustStore::migrate_v1(&temp.0, &f.pin, &previous, &update);
        let store = if point.is_some() {
            code(result, "SPX-PKR627");
            assert!(HeldTrustStore::open(&temp.0, &f.pin).is_err());
            HeldTrustStore::recover_migration_v1(&temp.0, &f.pin, &previous, &update, 101).unwrap()
        } else {
            result.unwrap()
        };
        drop(store);
        assert!(HeldTrustStore::open(&temp.0, &f.pin).is_ok());
        assert!(V1::open(&temp.0, &f.pin).is_err());
        assert!(HeldTrustStore::migrate_v1(&temp.0, &f.pin, &previous, &update).is_err());
    }
}
