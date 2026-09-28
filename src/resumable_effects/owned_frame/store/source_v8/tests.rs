use super::*;
use std::os::unix::fs::{MetadataExt, PermissionsExt};
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
static NEXT: AtomicU64 = AtomicU64::new(0);
struct Directory(PathBuf);
impl Directory {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!(
            "spx-owned-wait-v8-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir(&path).unwrap();
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o700)).unwrap();
        Self(path)
    }
    fn file(&self) -> File {
        File::open(&self.0).unwrap()
    }
    fn facts(&self) -> FreshSourceOwnedWaitFactsV8 {
        let m = std::fs::metadata(&self.0).unwrap();
        let execution = format!("sha256:{}", "1".repeat(64));
        let binding = format!("sha256:{}", "2".repeat(64));
        let invocation = codec::fact_digest(
            ID_DOMAIN,
            &json!({"execution":execution,"owned_wait_binding":binding}),
        );
        FreshSourceOwnedWaitFactsV8 {
            scope: SourceCheckpointScope::new(format!("sha256:{}", "3".repeat(64)), invocation, 7)
                .unwrap(),
            execution,
            binding,
            directory_identity: (m.dev(), m.ino()),
            limits: SourceOwnedWaitLimitsV8 {
                max_steps_per_stage: 100,
                max_total_steps: 1000,
                max_stages: 8,
                max_attempts: 3,
                response_limit: 4096,
            },
        }
    }
}
impl Drop for Directory {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}
fn grant() -> ExplicitStoreRegistrationGrant {
    ExplicitStoreRegistrationGrant::for_trusted_host(true).unwrap()
}
fn fresh(d: &Directory) -> (SourceOwnedWaitStoreRegistrationV8, SourceOwnedWaitLeaseV8) {
    fresh_source_owned_wait_v8(
        prepare_fresh_source_owned_wait_v8(d.file(), d.facts(), grant()).unwrap(),
    )
    .unwrap()
}
#[test]
fn owned_frame_v8_store_fresh_registration_is_postcreate_retained_and_profile_specific() {
    let d = Directory::new();
    let facts = d.facts();
    let (registration, mut lease) = fresh(&d);
    assert_eq!(
        registration.generation(),
        facts.generation(registration.identity()).unwrap()
    );
    assert_eq!(lease.append(b"not-yet-retained\n"), Err(Error::Policy));
    assert!(lease.read().unwrap().is_empty());
    assert_eq!(
        registration
            .acknowledge_retained_by_trusted_host(false)
            .err()
            .map(|e| e),
        Some(Error::Policy)
    );
    lease
        .authorize_fresh_start(
            registration
                .acknowledge_retained_by_trusted_host(true)
                .unwrap(),
        )
        .unwrap();
    assert_eq!(
        lease.authorize_fresh_start(
            registration
                .acknowledge_retained_by_trusted_host(true)
                .unwrap()
        ),
        Err(Error::Binding)
    );
    lease.append(b"v8-held-history\n").unwrap();
    assert_eq!(
        recover_source_owned_wait_v8(d.file(), &registration, facts.clone(), grant()).err(),
        Some(Error::Busy)
    );
    assert!(d.0.join(facts.name()).is_file());
    assert!(!d.0.join(super::super::name(&facts.scope)).exists());
    drop(lease);
    let mut recovered =
        recover_source_owned_wait_v8(d.file(), &registration, facts.clone(), grant()).unwrap();
    assert_eq!(
        recovered.authorize_fresh_start(
            registration
                .acknowledge_retained_by_trusted_host(true)
                .unwrap()
        ),
        Err(Error::Binding)
    );
    assert_eq!(recovered.read().unwrap(), b"v8-held-history\n");
    assert_eq!(
        recovered.validate(&facts, registration.generation()),
        Ok(())
    );
    drop(recovered);
    // V1 naming cannot reinterpret the actual v8 file, even with matching pins.
    let v1 = OwnedFrameStoreRegistration::grant_for_trusted_host(
        registration.identity(),
        &facts.scope,
        true,
    )
    .unwrap();
    assert!(RegisteredJournalLease::recover(d.file(), v1, &facts.scope).is_err());
}
#[test]
fn owned_frame_v8_store_scope_generation_pins_history_and_name_substitution_refuse() {
    let d = Directory::new();
    let facts = d.facts();
    let (registration, lease) = fresh(&d);
    drop(lease);
    for field in 0..6 {
        let mut wrong = registration.clone();
        match field {
            0 => wrong.generation = format!("sha256:{}", "f".repeat(64)),
            1 => wrong.identity.file_inode = wrong.identity.file_inode.wrapping_add(1),
            2 => {
                wrong.expected.scope = SourceCheckpointScope::new(
                    facts.scope.program_root(),
                    facts.scope.invocation_id(),
                    8,
                )
                .unwrap()
            }
            3 => wrong.expected.execution = format!("sha256:{}", "4".repeat(64)),
            4 => wrong.expected.limits.max_total_steps += 1,
            _ => wrong.expected.binding = format!("sha256:{}", "5".repeat(64)),
        }
        assert_eq!(
            recover_source_owned_wait_v8(d.file(), &wrong, facts.clone(), grant()).err(),
            Some(Error::Binding)
        );
    }
    assert_eq!(
        ExplicitStoreRegistrationGrant::for_trusted_host(false).err(),
        Some(Error::Policy)
    );
    let original = d.0.join(facts.name());
    let saved = d.0.join("saved");
    std::fs::rename(&original, &saved).unwrap();
    std::os::unix::fs::symlink(&saved, &original).unwrap();
    assert!(recover_source_owned_wait_v8(d.file(), &registration, facts.clone(), grant()).is_err());
    std::fs::remove_file(&original).unwrap();
    std::fs::copy(&saved, &original).unwrap();
    assert_eq!(
        recover_source_owned_wait_v8(d.file(), &registration, facts.clone(), grant()).err(),
        Some(Error::Binding)
    );
    assert_eq!(
        fresh_source_owned_wait_v8(
            prepare_fresh_source_owned_wait_v8(d.file(), facts, grant()).unwrap()
        )
        .err()
        .map(|e| e),
        Some(Error::Binding)
    );
}
#[test]
fn owned_frame_v8_store_ack_faults_poison_live_lease_and_recover_exact_bytes() {
    for after in [false, true] {
        let d = Directory::new();
        let facts = d.facts();
        let (registration, mut lease) = fresh(&d);
        lease
            .authorize_fresh_start(
                registration
                    .acknowledge_retained_by_trusted_host(true)
                    .unwrap(),
            )
            .unwrap();
        lease.inner.fail_append(1, after);
        assert_eq!(lease.append(b"candidate\n"), Err(Error::InDoubt));
        assert_eq!(lease.append(b"retry\n"), Err(Error::InDoubt));
        assert_eq!(lease.read(), Err(Error::InDoubt));
        drop(lease);
        let mut recovered =
            recover_source_owned_wait_v8(d.file(), &registration, facts, grant()).unwrap();
        assert_eq!(
            recovered.read().unwrap(),
            if after {
                b"candidate\n".as_slice()
            } else {
                b"".as_slice()
            }
        );
        assert_eq!(
            recovered.authorize_fresh_start(
                registration
                    .acknowledge_retained_by_trusted_host(true)
                    .unwrap()
            ),
            Err(Error::Binding)
        );
    }
}

#[test]
fn owned_frame_v8_store_process_and_expected_facts_guard_precedes_path_io() {
    let d = Directory::new();
    let facts = d.facts();
    let (registration, mut lease) = fresh(&d);
    assert_eq!(
        lease.inner.validate_profile(StoreProfile::OwnedFrameV1),
        Err(Error::Binding)
    );
    let original = d.0.join(facts.name());
    let saved = d.0.join("saved");
    std::fs::rename(&original, &saved).unwrap();
    let mut wrong = facts.clone();
    wrong.scope =
        SourceCheckpointScope::new(facts.scope.program_root(), facts.scope.invocation_id(), 8)
            .unwrap();
    // Missing path would be Storage if an I/O check ran first.
    assert_eq!(
        lease.validate(&wrong, registration.generation()),
        Err(Error::Binding)
    );
    let mut foreign_grant = grant();
    foreign_grant.creator = std::process::id().wrapping_add(1);
    assert_eq!(
        prepare_fresh_source_owned_wait_v8(d.file(), facts.clone(), foreign_grant).err(),
        Some(Error::Policy)
    );
    lease.inner.mark_foreign_process_for_test();
    assert_eq!(
        lease.validate(&facts, registration.generation()),
        Err(Error::Policy)
    );
    assert_eq!(lease.read(), Err(Error::Policy));
    assert_eq!(lease.append(b"forbidden\n"), Err(Error::Policy));
    assert_eq!(
        lease.authorize_fresh_start(
            registration
                .acknowledge_retained_by_trusted_host(true)
                .unwrap()
        ),
        Err(Error::Policy)
    );
}
