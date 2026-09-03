use super::*;
use crate::semantic_retention::{RetentionObservation, RetentionSubject};

struct Receipt(RetentionObservation);
impl RetentionReceipt for Receipt {
    fn retention_observation(&self) -> Result<RetentionObservation> {
        Ok(self.0.clone())
    }
}

fn digest(byte: char) -> String {
    format!("sha256:{}", byte.to_string().repeat(64))
}

fn receipt(byte: char, stored_bytes: u64) -> Receipt {
    Receipt(
        RetentionObservation::new(
            RetentionSubject::candidate(digest(byte), digest('c'), digest('b')).unwrap(),
            stored_bytes,
        )
        .unwrap(),
    )
}

#[test]
#[cfg(all(
    unix,
    any(
        target_os = "linux",
        target_os = "android",
        target_vendor = "apple",
        target_os = "redox"
    )
))]
fn exact_root_initializes_recovers_and_cas_advances_without_deleting_subjects() {
    use std::os::unix::fs::PermissionsExt;

    let temporary = std::env::temp_dir().canonicalize().unwrap().join(format!(
        "semaprax-retention-registry-{}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&temporary);
    std::fs::create_dir(&temporary).unwrap();
    std::fs::create_dir(temporary.join("metadata")).unwrap();
    std::fs::set_permissions(&temporary, std::fs::Permissions::from_mode(0o700)).unwrap();
    std::fs::set_permissions(
        temporary.join("metadata"),
        std::fs::Permissions::from_mode(0o700),
    )
    .unwrap();

    let first = receipt('1', 10);
    let policy = RetentionPolicy::new(2, 100, 1).unwrap();
    assert_eq!(
        initialize(&temporary, policy, &[]).unwrap_err()[0].code,
        "SPX-G464"
    );
    let initial = initialize(&temporary, policy, &[&first]).unwrap();
    assert_eq!(initial.authority(), RetentionAuthority::None);
    assert_eq!(initial.metadata().checkpoint().sequence(), 1);
    let recovered = recover(&temporary).unwrap();
    assert_eq!(recovered.cursor_digest(), initial.cursor_digest());

    std::fs::write(temporary.join(".CURRENT-stage"), b"interrupted cursor").unwrap();
    std::fs::set_permissions(
        temporary.join(".CURRENT-stage"),
        std::fs::Permissions::from_mode(0o600),
    )
    .unwrap();
    assert_eq!(recover(&temporary).unwrap_err()[0].code, "SPX-G464");
    std::fs::remove_file(temporary.join(".CURRENT-stage")).unwrap();

    std::fs::write(temporary.join(".CURRENT-stage"), initial.cursor_json()).unwrap();
    std::fs::set_permissions(
        temporary.join(".CURRENT-stage"),
        std::fs::Permissions::from_mode(0o600),
    )
    .unwrap();
    assert_eq!(recover(&temporary).unwrap_err()[0].code, "SPX-G466");
    std::fs::remove_file(temporary.join(".CURRENT-stage")).unwrap();

    let mut foreign = Cursor {
        sequence: 2,
        checkpoint: digest('e'),
        previous: Some(
            initial
                .metadata()
                .checkpoint()
                .checkpoint_digest()
                .to_owned(),
        ),
        plan: digest('f'),
        policy,
        json: String::new(),
        digest: String::new(),
    };
    foreign.json = foreign.render().unwrap();
    std::fs::write(temporary.join(".CURRENT-stage"), &foreign.json).unwrap();
    std::fs::set_permissions(
        temporary.join(".CURRENT-stage"),
        std::fs::Permissions::from_mode(0o600),
    )
    .unwrap();
    assert_eq!(recover(&temporary).unwrap_err()[0].code, "SPX-G429");
    std::fs::remove_file(temporary.join(".CURRENT-stage")).unwrap();

    let second = receipt('2', 20);
    let derived = checkpoint_receipts(
        Some(initial.metadata().checkpoint()),
        Some(initial.metadata().checkpoint().checkpoint_digest()),
        2,
        policy,
        &[&second],
    )
    .unwrap();
    semantic_retention_store::persist(
        &temporary.join("metadata"),
        derived.checkpoint(),
        derived.checkpoint().checkpoint_digest(),
        derived.checkpoint().previous_checkpoint_digest(),
        derived.plan(),
        derived.plan_digest(),
    )
    .unwrap();
    let staged = Cursor::new(derived.checkpoint(), derived.plan()).unwrap();
    std::fs::write(temporary.join(".CURRENT-stage"), staged.json).unwrap();
    std::fs::set_permissions(
        temporary.join(".CURRENT-stage"),
        std::fs::Permissions::from_mode(0o600),
    )
    .unwrap();
    let advanced = advance(&temporary, initial.cursor_digest(), &[&second]).unwrap();
    assert_eq!(advanced.metadata().checkpoint().sequence(), 2);
    assert_eq!(
        advanced.metadata().checkpoint().retained_subjects().len(),
        2
    );
    assert_eq!(
        advance(&temporary, initial.cursor_digest(), &[&second]).unwrap_err()[0].code,
        "SPX-G467"
    );
    assert_eq!(
        recover(&temporary).unwrap().cursor_digest(),
        advanced.cursor_digest()
    );
    assert!(!temporary.join(".CURRENT-stage").exists());
    assert_eq!(
        std::fs::read_dir(temporary.join("metadata"))
            .unwrap()
            .count(),
        2
    );
    std::fs::remove_dir_all(temporary).unwrap();
}

#[test]
#[cfg(all(
    unix,
    any(
        target_os = "linux",
        target_os = "android",
        target_vendor = "apple",
        target_os = "redox"
    )
))]
fn metadata_swap_after_held_operation_preserves_current_and_recovery() {
    use std::os::unix::fs::PermissionsExt;

    let temporary = std::env::temp_dir().canonicalize().unwrap().join(format!(
        "semaprax-retention-registry-seeded-pivot-swap-{}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&temporary);
    std::fs::create_dir(&temporary).unwrap();
    std::fs::create_dir(temporary.join("metadata")).unwrap();
    std::fs::set_permissions(&temporary, std::fs::Permissions::from_mode(0o700)).unwrap();
    std::fs::set_permissions(
        temporary.join("metadata"),
        std::fs::Permissions::from_mode(0o700),
    )
    .unwrap();

    let first = receipt('3', 30);
    let initial = initialize(
        &temporary,
        RetentionPolicy::new(2, 100, 1).unwrap(),
        &[&first],
    )
    .unwrap();
    let current = std::fs::read(temporary.join("CURRENT")).unwrap();
    assert_eq!(current, initial.cursor_json().as_bytes());

    let errors = unix::transaction(&temporary, |_current, _held_metadata| {
        std::fs::rename(
            temporary.join("metadata"),
            temporary.join("displaced-metadata"),
        )
        .unwrap();
        std::fs::create_dir(temporary.join("metadata")).unwrap();
        std::fs::set_permissions(
            temporary.join("metadata"),
            std::fs::Permissions::from_mode(0o700),
        )
        .unwrap();
        Ok((initial.cursor_json().as_bytes().to_vec(), ()))
    })
    .unwrap_err();
    assert_eq!(errors[0].code, "SPX-G466");
    assert_eq!(std::fs::read(temporary.join("CURRENT")).unwrap(), current);

    std::fs::remove_dir(temporary.join("metadata")).unwrap();
    std::fs::rename(
        temporary.join("displaced-metadata"),
        temporary.join("metadata"),
    )
    .unwrap();
    assert_eq!(
        recover(&temporary).unwrap().cursor_digest(),
        initial.cursor_digest()
    );
    std::fs::remove_dir_all(temporary).unwrap();
}
