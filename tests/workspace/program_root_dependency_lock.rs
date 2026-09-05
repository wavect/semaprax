use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use semaprax::diagnostic::Diagnostic;
use semaprax::project::{
    render_project_lock, with_authenticated_project, ProgramRootDependencyLockAssociation,
    MAX_PROGRAM_ROOT_DEPENDENCY_LOCK_ASSOCIATION_BYTES,
    PROGRAM_ROOT_DEPENDENCY_LOCK_ASSOCIATION_SCHEMA, PROGRAM_ROOT_SEGMENT_SCHEMA,
};
use serde_json::Value;
use sha2::{Digest, Sha256};

static SERIAL: AtomicU64 = AtomicU64::new(0);

struct Fixture(PathBuf);

impl Fixture {
    fn new(label: &str, changed: bool) -> Self {
        let root = std::env::temp_dir().join(format!(
            "semaprax-program-root-lock-{label}-{}-{}",
            std::process::id(),
            SERIAL.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir_all(root.join("src")).unwrap();
        let sample = Path::new(env!("CARGO_MANIFEST_DIR")).join("examples/calculator-project");
        for path in [
            "semaprax.toml",
            "src/app.spx",
            "src/core.spx",
            "src/tests.spx",
        ] {
            std::fs::copy(sample.join(path), root.join(path)).unwrap();
        }
        if changed {
            let path = root.join("src/core.spx");
            let source = std::fs::read_to_string(&path)
                .unwrap()
                .replace("left + right", "left + right + 1");
            std::fs::write(path, source).unwrap();
        }
        Self(root.canonicalize().unwrap())
    }

    fn manifest(&self) -> PathBuf {
        self.0.join("semaprax.toml")
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn inventory(root: &Path) -> BTreeMap<PathBuf, Vec<u8>> {
    fn visit(root: &Path, current: &Path, entries: &mut BTreeMap<PathBuf, Vec<u8>>) {
        let mut paths = std::fs::read_dir(current)
            .unwrap()
            .map(|entry| entry.unwrap().path())
            .collect::<Vec<_>>();
        paths.sort();
        for path in paths {
            let relative = path.strip_prefix(root).unwrap().to_owned();
            if path.is_dir() {
                entries.insert(relative, Vec::new());
                visit(root, &path, entries);
            } else {
                entries.insert(relative, std::fs::read(path).unwrap());
            }
        }
    }
    let mut entries = BTreeMap::new();
    visit(root, root, &mut entries);
    entries
}

fn framed(domain: &[u8], bytes: &[u8]) -> String {
    let mut digest = Sha256::new();
    digest.update(domain);
    digest.update((bytes.len() as u64).to_le_bytes());
    digest.update(bytes);
    format!(
        "sha256:{:x}",
        semaprax::digest_hex::LowerHex(digest.finalize())
    )
}

fn canonical(mut value: Value) -> String {
    value.sort_all_objects();
    serde_json::to_string(&value).unwrap() + "\n"
}

fn assert_code<T>(result: Result<T, Vec<Diagnostic>>, code: &str) {
    let errors = result.err().unwrap_or_else(|| panic!("expected {code}"));
    assert!(errors.iter().any(|error| error.code == code), "{errors:?}");
}

#[test]
fn exact_project_lock_association_is_replayable_and_changes_no_default_bytes() {
    const ASSOCIATION_DOMAIN: &[u8] =
        b"semaprax.program-root.dependency-lock-association.digest.v1\0";
    const LOCK_BYTES_DOMAIN: &[u8] =
        b"semaprax.program-root.dependency-lock-association.lock-bytes.digest.v1\0";

    let fixture = Fixture::new("base", false);
    let before = inventory(&fixture.0);
    with_authenticated_project(&fixture.manifest(), |snapshot| {
        let workspace = snapshot.canonical_workspace_revision()?;
        let workspace_bytes = workspace.to_json().to_owned();
        let root = workspace.program_root()?;
        let root_bytes = root.to_json().to_owned();
        let lock = render_project_lock(snapshot)?;
        let lock_value: Value = serde_json::from_str(&lock).unwrap();

        let association =
            root.associate_dependency_lock(snapshot, root.program_root_digest(), &lock)?;
        assert_eq!(
            association,
            ProgramRootDependencyLockAssociation::derive(
                snapshot,
                &root,
                root.program_root_digest(),
                &lock,
            )?
        );
        let value: Value = serde_json::from_str(association.to_json()).unwrap();
        assert_eq!(
            value["schema"],
            PROGRAM_ROOT_DEPENDENCY_LOCK_ASSOCIATION_SCHEMA
        );
        assert_eq!(value["program_root_digest"], root.program_root_digest());
        assert_eq!(value["project_revision"], snapshot.project_revision());
        assert_eq!(
            value["canonical_workspace_revision"],
            workspace.workspace_revision()
        );
        assert_eq!(
            value["canonical_workspace_dependency_lock_digest"],
            workspace.dependency_lock_digest()
        );
        assert_eq!(value["project_lock"]["bytes"], lock.len());
        assert_eq!(value["project_lock"]["digest"], lock_value["digest"]);
        assert_eq!(
            association.project_lock_bytes_digest(),
            framed(LOCK_BYTES_DOMAIN, lock.as_bytes())
        );
        assert_eq!(association.project_lock_bytes(), lock);
        let mut identity_subject = value.clone();
        identity_subject
            .as_object_mut()
            .unwrap()
            .remove("association_digest");
        assert_eq!(
            association.association_digest(),
            framed(ASSOCIATION_DOMAIN, canonical(identity_subject).as_bytes())
        );

        let segment = association.program_root_segment()?;
        assert_eq!(segment.kind(), "project_lock_association");
        assert_eq!(
            segment.node_schema(),
            PROGRAM_ROOT_DEPENDENCY_LOCK_ASSOCIATION_SCHEMA
        );
        assert_eq!(segment.node_digest(), association.association_digest());
        assert_eq!(segment.node_bytes(), association.to_json().len());
        assert_eq!(
            serde_json::from_str::<Value>(segment.to_json()).unwrap()["schema"],
            PROGRAM_ROOT_SEGMENT_SCHEMA
        );

        assert_eq!(
            ProgramRootDependencyLockAssociation::replay(
                snapshot,
                &root,
                association.association_digest(),
                &lock,
                association.to_json().as_bytes(),
            )?,
            association
        );
        assert_code(
            ProgramRootDependencyLockAssociation::replay(
                snapshot,
                &root,
                association.association_digest(),
                &lock,
                association.to_json().trim_end().as_bytes(),
            ),
            "SPX-G550",
        );
        assert_code(
            ProgramRootDependencyLockAssociation::replay(
                snapshot,
                &root,
                association.association_digest(),
                &lock,
                &vec![b' '; MAX_PROGRAM_ROOT_DEPENDENCY_LOCK_ASSOCIATION_BYTES + 1],
            ),
            "SPX-G550",
        );
        let mut fixed_field_mutation = value.clone();
        fixed_field_mutation["nonclaims"][0] = Value::String("forged_nonclaim".to_owned());
        fixed_field_mutation
            .as_object_mut()
            .unwrap()
            .remove("association_digest");
        let reminted_digest = framed(
            ASSOCIATION_DOMAIN,
            canonical(fixed_field_mutation.clone()).as_bytes(),
        );
        fixed_field_mutation["association_digest"] = Value::String(reminted_digest);
        let reminted = canonical(fixed_field_mutation);
        assert_code(
            ProgramRootDependencyLockAssociation::replay(
                snapshot,
                &root,
                association.association_digest(),
                &lock,
                reminted.as_bytes(),
            ),
            "SPX-G550",
        );
        let mut self_digest_mutation = value.clone();
        self_digest_mutation["association_digest"] =
            Value::String(workspace.workspace_revision().to_owned());
        let self_digest_mutation = canonical(self_digest_mutation);
        assert_code(
            ProgramRootDependencyLockAssociation::replay(
                snapshot,
                &root,
                association.association_digest(),
                &lock,
                self_digest_mutation.as_bytes(),
            ),
            "SPX-G550",
        );
        assert_code(
            ProgramRootDependencyLockAssociation::replay(
                snapshot,
                &root,
                association.association_digest(),
                &format!("{lock} "),
                association.to_json().as_bytes(),
            ),
            "SPX-G550",
        );
        assert_code(
            ProgramRootDependencyLockAssociation::derive(
                snapshot,
                &root,
                workspace.workspace_revision(),
                &lock,
            ),
            "SPX-G551",
        );
        assert_code(
            ProgramRootDependencyLockAssociation::replay(
                snapshot,
                &root,
                workspace.dependency_lock_digest(),
                &lock,
                association.to_json().as_bytes(),
            ),
            "SPX-G551",
        );

        assert_eq!(workspace.to_json(), workspace_bytes);
        assert_eq!(root.to_json(), root_bytes);
        assert_eq!(render_project_lock(snapshot)?, lock);
        Ok(())
    })
    .unwrap();
    assert_eq!(inventory(&fixture.0), before);
}

#[test]
fn stale_and_malformed_project_locks_delegate_to_project_lock_authority() {
    let base = Fixture::new("stale-base", false);
    let changed = Fixture::new("stale-changed", true);
    let stale_lock = with_authenticated_project(&changed.manifest(), |snapshot| {
        render_project_lock(snapshot)
    })
    .unwrap();
    let foreign_root =
        with_authenticated_project(&changed.manifest(), |snapshot| snapshot.program_root())
            .unwrap();
    with_authenticated_project(&base.manifest(), |snapshot| {
        let root = snapshot.program_root()?;
        let lock = render_project_lock(snapshot)?;
        assert_code(
            ProgramRootDependencyLockAssociation::derive(
                snapshot,
                &foreign_root,
                foreign_root.program_root_digest(),
                &lock,
            ),
            "SPX-G551",
        );
        assert_code(
            root.associate_dependency_lock(snapshot, root.program_root_digest(), &stale_lock),
            "SPX-J123",
        );
        assert_code(
            root.associate_dependency_lock(snapshot, root.program_root_digest(), "not-json"),
            "SPX-J124",
        );
        Ok(())
    })
    .unwrap();
}
