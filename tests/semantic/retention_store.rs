//! Immutable retention metadata store evidence, authored and intentionally unrun.

#![cfg(all(
    unix,
    any(
        target_os = "linux",
        target_os = "android",
        target_vendor = "apple",
        target_os = "redox"
    )
))]

use semaprax::diagnostic::Diagnostic;
use semaprax::semantic_retention::{
    checkpoint, RetentionAuthority, RetentionObservation, RetentionPolicy, RetentionSubject,
    RetentionTransition, MAX_RETENTION_TOTAL_BYTES,
};
use semaprax::semantic_retention_store::{load, persist};
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

static SERIAL: AtomicU64 = AtomicU64::new(0);

struct Fixture(PathBuf);
impl Fixture {
    fn new() -> Self {
        let root = std::env::temp_dir().join(format!(
            "spx-retention-metadata-store-{}-{}",
            std::process::id(),
            SERIAL.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir_all(&root).unwrap();
        fs::set_permissions(&root, fs::Permissions::from_mode(0o700)).unwrap();
        Self(root.canonicalize().unwrap())
    }

    fn pair_path(&self, transition: &RetentionTransition) -> PathBuf {
        self.0.join(format!(
            "{}-{}.spxr",
            &transition.checkpoint().checkpoint_digest()["sha256:".len()..],
            &transition.plan_digest()["sha256:".len()..]
        ))
    }

    fn stage_path(&self, transition: &RetentionTransition) -> PathBuf {
        self.0.join(format!(
            ".stage-{}-{}",
            &transition.checkpoint().checkpoint_digest()["sha256:".len()..],
            &transition.plan_digest()["sha256:".len()..]
        ))
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn digest(byte: char) -> String {
    format!("sha256:{}", byte.to_string().repeat(64))
}

fn transition() -> RetentionTransition {
    let observations = [
        RetentionObservation::new(
            RetentionSubject::image(digest('1'), digest('2'), digest('3')).unwrap(),
            11,
        )
        .unwrap(),
        RetentionObservation::new(
            RetentionSubject::candidate(digest('4'), digest('5'), digest('6')).unwrap(),
            13,
        )
        .unwrap(),
    ];
    checkpoint(
        None,
        None,
        1,
        RetentionPolicy::new(1, MAX_RETENTION_TOTAL_BYTES, 0).unwrap(),
        &observations,
    )
    .unwrap()
}

fn code<T>(result: Result<T, Vec<Diagnostic>>, expected: &str) {
    let diagnostics = result.err().expect("hostile store operation succeeded");
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == expected),
        "{diagnostics:?}"
    );
}

#[test]
fn exact_pair_round_trips_only_through_independent_selectors_and_ordinary_restore() {
    let fixture = Fixture::new();
    let transition = transition();
    let checkpoint = transition.checkpoint();
    let receipt = persist(
        &fixture.0,
        checkpoint,
        checkpoint.checkpoint_digest(),
        None,
        transition.plan(),
        transition.plan_digest(),
    )
    .unwrap();
    assert_eq!(receipt.checkpoint_digest(), checkpoint.checkpoint_digest());
    assert_eq!(receipt.plan_digest(), transition.plan_digest());
    assert_eq!(receipt.authority(), RetentionAuthority::None);
    assert!(receipt.envelope_bytes() > checkpoint.to_json().len());
    let path = fixture.pair_path(&transition);
    assert!(path.is_file());
    assert_eq!(
        fs::metadata(&path).unwrap().permissions().mode() & 0o7777,
        0o600
    );

    let restored = load(
        &fixture.0,
        checkpoint.checkpoint_digest(),
        None,
        transition.plan_digest(),
    )
    .unwrap();
    assert_eq!(restored.authority(), RetentionAuthority::None);
    assert_eq!(restored.checkpoint().to_json(), checkpoint.to_json());
    assert_eq!(restored.plan().to_json(), transition.plan().to_json());
    assert_eq!(restored.plan().evicted_subjects().len(), 1);
    assert!(path.exists());
}

#[test]
fn duplicate_publication_is_no_adoption_and_tamper_never_restores() {
    let fixture = Fixture::new();
    let transition = transition();
    let checkpoint = transition.checkpoint();
    persist(
        &fixture.0,
        checkpoint,
        checkpoint.checkpoint_digest(),
        None,
        transition.plan(),
        transition.plan_digest(),
    )
    .unwrap();
    let path = fixture.pair_path(&transition);
    let original = fs::read(&path).unwrap();
    code(
        persist(
            &fixture.0,
            checkpoint,
            checkpoint.checkpoint_digest(),
            None,
            transition.plan(),
            transition.plan_digest(),
        ),
        "SPX-G429",
    );
    assert_eq!(fs::read(&path).unwrap(), original);

    let mut tampered = original;
    tampered[0] ^= 1;
    fs::write(&path, &tampered).unwrap();
    code(
        load(
            &fixture.0,
            checkpoint.checkpoint_digest(),
            None,
            transition.plan_digest(),
        ),
        "SPX-G427",
    );
    assert_eq!(fs::read(&path).unwrap(), tampered);
}

#[test]
fn interrupted_stage_is_retained_and_never_adopted_cleaned_or_loaded() {
    let fixture = Fixture::new();
    let transition = transition();
    let checkpoint = transition.checkpoint();
    let stage = fixture.stage_path(&transition);
    fs::write(&stage, b"interrupted metadata stage").unwrap();
    fs::set_permissions(&stage, fs::Permissions::from_mode(0o600)).unwrap();

    code(
        persist(
            &fixture.0,
            checkpoint,
            checkpoint.checkpoint_digest(),
            None,
            transition.plan(),
            transition.plan_digest(),
        ),
        "SPX-G429",
    );
    code(
        load(
            &fixture.0,
            checkpoint.checkpoint_digest(),
            None,
            transition.plan_digest(),
        ),
        "SPX-G429",
    );
    assert_eq!(fs::read(&stage).unwrap(), b"interrupted metadata stage");
    assert!(!fixture.pair_path(&transition).exists());
}
