use std::os::unix::fs::PermissionsExt;

use super::*;

fn inert_stage(store: &Path, digit: char) -> PathBuf {
    let stage = store.join(format!(".stage-{}", digit.to_string().repeat(64)));
    std::fs::create_dir(&stage).unwrap();
    std::fs::set_permissions(&stage, std::fs::Permissions::from_mode(0o700)).unwrap();
    stage
}

#[test]
fn one_inert_stage_is_never_traversed_and_does_not_block_an_unrelated_load() {
    let fixture = Fixture::new("inert-load");
    let revision = revision();
    let receipt = persist(&fixture.store, &revision, revision.project_revision()).unwrap();
    let stage = inert_stage(&fixture.store, '0');
    std::fs::write(stage.join("foreign"), b"must remain untouched").unwrap();
    std::os::unix::fs::symlink("missing", stage.join("foreign-link")).unwrap();

    let loaded = load(
        &fixture.store,
        receipt.entry_digest(),
        receipt.project_revision(),
    )
    .unwrap();
    assert_eq!(loaded.project_revision(), revision.project_revision());
    assert_eq!(
        std::fs::read(stage.join("foreign")).unwrap(),
        b"must remain untouched"
    );
    assert!(std::fs::symlink_metadata(stage.join("foreign-link"))
        .unwrap()
        .file_type()
        .is_symlink());

    let error = persist(&fixture.store, &revision, revision.project_revision()).unwrap_err();
    assert_eq!(error[0].code, "SPX-G193");
    assert!(stage.is_dir());
}

#[test]
fn invalid_excess_or_non_private_stage_inventory_rejects_load() {
    for mutation in ["invalid-name", "second", "special-mode"] {
        let fixture = Fixture::new(mutation);
        let revision = revision();
        let receipt = persist(&fixture.store, &revision, revision.project_revision()).unwrap();
        match mutation {
            "invalid-name" => {
                std::fs::create_dir(fixture.store.join(".stage-foreign")).unwrap();
            }
            "second" => {
                inert_stage(&fixture.store, '0');
                inert_stage(&fixture.store, '1');
            }
            "special-mode" => {
                let stage = inert_stage(&fixture.store, '0');
                std::fs::set_permissions(stage, std::fs::Permissions::from_mode(0o1700)).unwrap();
            }
            _ => unreachable!(),
        }
        let error = load(
            &fixture.store,
            receipt.entry_digest(),
            receipt.project_revision(),
        )
        .err()
        .expect("hostile stage inventory must reject");
        assert_eq!(error[0].code, "SPX-G193", "{mutation}");
    }
}

#[test]
fn inert_stage_top_identity_is_cached_and_rechecked_without_opening_it() {
    let fixture = Fixture::new("inert-identity");
    let revision = revision();
    let receipt = persist(&fixture.store, &revision, revision.project_revision()).unwrap();
    let stage = inert_stage(&fixture.store, '0');
    let displaced = fixture.directory.join("displaced-stage");

    let error = unix::load_with_hook(&fixture.store, receipt.entry_digest(), |point, _| {
        assert_eq!(point, unix::LoadPoint::AfterInventory);
        std::fs::rename(&stage, &displaced)?;
        std::fs::create_dir(&stage)?;
        std::fs::set_permissions(&stage, std::fs::Permissions::from_mode(0o700))?;
        Ok(())
    })
    .err()
    .expect("inert stage substitution must reject");
    assert_eq!(error[0].code, "SPX-G193");
    assert!(displaced.is_dir());
    assert!(stage.is_dir());
}

#[test]
fn root_and_stored_objects_reject_special_permission_bits() {
    let root_fixture = Fixture::new("root-special-mode");
    let revision = revision();
    std::fs::set_permissions(&root_fixture.store, std::fs::Permissions::from_mode(0o1700)).unwrap();
    let error = persist(&root_fixture.store, &revision, revision.project_revision())
        .expect_err("special permission bits must reject");
    assert_eq!(error[0].code, "SPX-G193");

    let file_fixture = Fixture::new("file-special-mode");
    let receipt = persist(&file_fixture.store, &revision, revision.project_revision()).unwrap();
    let entry = file_fixture
        .store
        .join(receipt.entry_digest().trim_start_matches("sha256:"));
    std::fs::set_permissions(
        entry.join("entry.json"),
        std::fs::Permissions::from_mode(0o1600),
    )
    .unwrap();
    let error = load(
        &file_fixture.store,
        receipt.entry_digest(),
        receipt.project_revision(),
    )
    .err()
    .expect("special stored-file permission bits must reject");
    assert_eq!(error[0].code, "SPX-G193");
}

#[test]
fn created_file_replay_consumes_only_expected_plus_one_and_detects_growth() {
    struct Infinite {
        consumed: usize,
    }

    impl std::io::Read for Infinite {
        fn read(&mut self, buffer: &mut [u8]) -> std::io::Result<usize> {
            buffer.fill(b'x');
            self.consumed += buffer.len();
            Ok(buffer.len())
        }
    }

    let expected = b"exact";
    let mut exact = std::io::Cursor::new(expected.as_slice());
    let observed = unix::read_expected_plus_one(&mut exact, expected.len()).unwrap();
    assert_eq!(observed.as_slice(), expected);

    let mut growing = Infinite { consumed: 0 };
    let observed = unix::read_expected_plus_one(&mut growing, expected.len()).unwrap();
    assert_eq!(growing.consumed, expected.len() + 1);
    assert_eq!(observed.len(), expected.len() + 1);
    assert_ne!(observed.as_slice(), expected);
}
