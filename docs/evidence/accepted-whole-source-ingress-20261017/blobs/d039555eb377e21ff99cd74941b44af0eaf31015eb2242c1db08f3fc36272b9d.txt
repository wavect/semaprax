//! Complete source inputs and owning-module coverage, independently of purity.
use std::collections::BTreeSet;
use std::fs;
use std::path::{Component, Path, PathBuf};

fn confined(repository: &Path, path: &Path) -> PathBuf {
    assert!(!fs::symlink_metadata(path).unwrap().file_type().is_symlink());
    let path = path.canonicalize().unwrap();
    assert!(
        path.starts_with(repository),
        "source escaped the repository"
    );
    path
}

fn directory(repository: &Path, path: &Path, owned: &mut BTreeSet<PathBuf>) {
    for entry in fs::read_dir(path).unwrap() {
        let path = confined(repository, &entry.unwrap().path());
        if path.is_dir() {
            directory(repository, &path, owned);
        } else if path.extension().is_some_and(|extension| extension == "rs") {
            owned.insert(path);
        }
    }
}

fn module(repository: &Path, path: &Path, owned: &mut BTreeSet<PathBuf>) {
    let path = confined(repository, path);
    assert!(path.is_file());
    owned.insert(path.clone());
    let children = if path.file_name().is_some_and(|name| name == "mod.rs") {
        path.parent().unwrap().to_path_buf()
    } else {
        path.with_extension("")
    };
    if children.is_dir() {
        directory(repository, &children, owned);
    }
}

pub(super) fn verify(
    repository: &Path,
    owner: &str,
    complete: &str,
    projection: &str,
    input_list: &str,
) {
    let repository = repository.canonicalize().unwrap();
    let inputs = input_list
        .lines()
        .map(|line| {
            assert!(!line.is_empty() && line.trim() == line);
            let path = Path::new(line);
            assert!(path
                .components()
                .all(|part| matches!(part, Component::Normal(_))));
            confined(&repository, &repository.join(path))
        })
        .collect::<Vec<_>>();
    let expected = inputs
        .iter()
        .map(|path| fs::read_to_string(path).unwrap())
        .collect::<String>();
    assert_eq!(
        complete, expected,
        "broader inventory lost or reordered source bytes"
    );

    let mut owned = BTreeSet::new();
    module(&repository, &repository.join(owner), &mut owned);
    let mut visited = BTreeSet::new();
    while let Some(path) = owned.iter().find(|path| !visited.contains(*path)).cloned() {
        visited.insert(path.clone());
        // Follow explicit module paths too, including modules stored outside
        // their parent's physical directory. Every such file remains required.
        for line in fs::read_to_string(&path).unwrap().lines() {
            if !line.trim_start().starts_with("#[path") {
                continue;
            }
            let relative = line.split('"').nth(1).expect("literal module path");
            module(
                &repository,
                &path.parent().unwrap().join(relative),
                &mut owned,
            );
        }
    }
    let mut included = BTreeSet::new();
    let expected_projection = inputs
        .iter()
        .filter(|path| owned.contains(*path))
        .map(|path| {
            assert!(included.insert(path.clone()), "owned module repeated");
            fs::read_to_string(path).unwrap()
        })
        .collect::<String>();
    assert_eq!(
        included, owned,
        "owning-module source is missing from the full inventory"
    );
    assert_eq!(
        projection, expected_projection,
        "purity input lost owning-module source bytes"
    );
}

#[cfg(test)]
mod tests {
    use super::verify;
    use std::fs;
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicU64, Ordering};
    static SERIAL: AtomicU64 = AtomicU64::new(0);
    const INPUTS: &str = "src/owner.rs\nsrc/owner/child.rs\nforeign.rs\n";
    struct Fixture(PathBuf);
    impl Fixture {
        fn new() -> Self {
            let root = std::env::temp_dir().join(format!(
                "semaprax-projection-source-inventory-{}-{}-{}",
                std::process::id(),
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_nanos(),
                SERIAL.fetch_add(1, Ordering::Relaxed)
            ));
            fs::create_dir(&root).unwrap();
            fs::create_dir_all(root.join("src/owner")).unwrap();
            fs::write(
                root.join("src/owner.rs"),
                "#[path = \"owner/child.rs\"]\nmod child;\n",
            )
            .unwrap();
            fs::write(root.join("src/owner/child.rs"), "fn pure() {}\n").unwrap();
            fs::write(
                root.join("foreign.rs"),
                "use super::*;\nfn unrelated() {}\n",
            )
            .unwrap();
            Self(root)
        }
        fn bytes(&self, paths: &str) -> String {
            paths
                .lines()
                .map(|path| fs::read_to_string(self.0.join(path)).unwrap())
                .collect()
        }
        fn projection(&self) -> String {
            self.bytes("src/owner.rs\nsrc/owner/child.rs\n")
        }
    }
    impl Drop for Fixture {
        fn drop(&mut self) {
            fs::remove_dir_all(&self.0).unwrap();
        }
    }
    #[test]
    fn complete_inventory_retains_foreign_bytes_and_every_owned_child() {
        let fixture = Fixture::new();
        let complete = fixture.bytes(INPUTS);
        let projection = fixture.projection();
        assert!(complete.contains("use super::*;"));
        assert!(!projection.contains("use super::*;"));
        verify(&fixture.0, "src/owner.rs", &complete, &projection, INPUTS);
    }
    #[test]
    fn missing_or_reordered_broader_inputs_are_rejected() {
        let fixture = Fixture::new();
        for paths in [
            "src/owner.rs\nsrc/owner/child.rs\n",
            "foreign.rs\nsrc/owner.rs\nsrc/owner/child.rs\n",
        ] {
            assert!(std::panic::catch_unwind(|| verify(
                &fixture.0,
                "src/owner.rs",
                &fixture.bytes(paths),
                &fixture.projection(),
                INPUTS
            ))
            .is_err());
        }
    }
    #[test]
    fn missing_owned_bytes_and_new_nested_files_are_rejected() {
        let fixture = Fixture::new();
        let complete = fixture.bytes(INPUTS);
        assert!(std::panic::catch_unwind(|| verify(
            &fixture.0,
            "src/owner.rs",
            &complete,
            &fixture.bytes("src/owner.rs\n"),
            INPUTS
        ))
        .is_err());
        let projection = fixture.projection();
        fs::write(fixture.0.join("src/owner/new.rs"), "fn new_child() {}\n").unwrap();
        assert!(std::panic::catch_unwind(|| verify(
            &fixture.0,
            "src/owner.rs",
            &complete,
            &projection,
            INPUTS
        ))
        .is_err());
    }
    #[test]
    fn declared_out_of_directory_children_cannot_escape_the_owned_inventory() {
        let fixture = Fixture::new();
        fs::write(
            fixture.0.join("src/owner.rs"),
            "#[path = \"owner/child.rs\"]\nmod child;\n#[path = \"../extra.rs\"]\nmod extra;\n",
        )
        .unwrap();
        fs::write(fixture.0.join("extra.rs"), "fn extra() {}\n").unwrap();
        assert!(std::panic::catch_unwind(|| verify(
            &fixture.0,
            "src/owner.rs",
            &fixture.bytes(INPUTS),
            &fixture.projection(),
            INPUTS
        ))
        .is_err());
        let inputs = format!("{INPUTS}extra.rs\n");
        let projection = fixture.projection() + &fixture.bytes("extra.rs\n");
        verify(
            &fixture.0,
            "src/owner.rs",
            &fixture.bytes(&inputs),
            &projection,
            &inputs,
        );
    }
}

#[cfg(test)]
mod current_union {
    #[test]
    fn current_union_retains_every_frozen_legacy_input_and_all_owned_paths() {
        let legacy = include_str!("../../../docs/evidence/projection-audit-scope-v1/legacy-inputs.tsv");
        let current = include_str!("../../../docs/evidence/main-c883-next-source-merge-20261017/current-projection-inputs.tsv");
        let owned = include_str!("../../../docs/evidence/main-c883-next-source-merge-20261017/current-owned-inputs.tsv");
        let current: Vec<_> = current.lines().collect();
        assert_eq!(legacy.lines().count(), 90);
        for input in legacy.lines() {
            assert!(current.contains(&input), "frozen legacy source omitted: {input}");
        }
        for input in owned.lines() {
            assert_eq!(current.iter().filter(|path| **path == input).count(), 1, "current owned source missing or duplicated: {input}");
        }
    }
}
