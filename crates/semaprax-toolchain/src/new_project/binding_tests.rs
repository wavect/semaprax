//! Namespace observations after publication; no runtime hook is exported.
use super::*;

struct BeforePublication {
    destination: PathBuf,
    fail_stage: bool,
    fail_write: Option<usize>,
    writes: std::cell::Cell<usize>,
}

impl WriteHook for BeforePublication {
    fn after_stage_created(&self) -> Result<(), String> {
        assert_eq!(
            fs::symlink_metadata(&self.destination).unwrap_err().kind(),
            std::io::ErrorKind::NotFound
        );
        if self.fail_stage {
            Err("stage boundary".to_owned())
        } else {
            Ok(())
        }
    }

    fn before_write(&self, index: usize, _path: &str) -> Result<(), String> {
        assert_eq!(
            fs::symlink_metadata(&self.destination).unwrap_err().kind(),
            std::io::ErrorKind::NotFound
        );
        self.writes.set(self.writes.get() + 1);
        if self.fail_write == Some(index) {
            Err("write boundary".to_owned())
        } else {
            Ok(())
        }
    }
}

#[test]
fn staging_collision_is_skipped_before_any_final_path_creation() {
    for uppercase in [false, true] {
        for failure in [None, Some(0), Some(1), Some(3)] {
            let (root, _) = fixture(false);
            let leaf = format!(".semaprax-new-{}-7", std::process::id());
            let leaf = if uppercase {
                leaf.to_ascii_uppercase()
            } else {
                leaf
            };
            let destination = root.join(&leaf);
            let hook = BeforePublication {
                destination: destination.clone(),
                fail_stage: failure == Some(0),
                fail_write: failure.filter(|value| *value != 0).map(|value| value - 1),
                writes: std::cell::Cell::new(0),
            };
            let mut supplied = 0;
            let result = create_with_serial(&destination, "calculator", &hook, &mut || {
                supplied += 1;
                match supplied {
                    1 => 7,
                    2 => 8,
                    _ => panic!("unexpected staging attempt"),
                }
            });
            assert_eq!(supplied, 2);
            if let Some(expected_writes) = failure {
                let error = result.unwrap_err();
                assert_eq!(error.exit_code(), 1);
                assert!(error.to_string().contains("injected"));
                assert_eq!(hook.writes.get(), expected_writes);
                assert_eq!(
                    fs::symlink_metadata(&destination).unwrap_err().kind(),
                    std::io::ErrorKind::NotFound
                );
                assert!(names(&root).is_empty());
            } else {
                assert_eq!(result.unwrap(), destination);
                assert_eq!(hook.writes.get(), 4);
                assert_eq!(names(&root), [leaf]);
                remove_project(&destination);
            }
            assert!(names(&root).is_empty());
            fs::remove_dir(root).unwrap();
        }
    }
}

#[test]
fn skipped_collisions_consume_the_existing_attempt_budget() {
    let (root, _) = fixture(false);
    let destination = root.join(format!(".semaprax-new-{}-7", std::process::id()));
    let mut supplied = 0;
    let error = create_with_serial(&destination, "calculator", &NoopWriteHook, &mut || {
        supplied += 1;
        7
    })
    .unwrap_err();
    assert_eq!(supplied, 32);
    assert_eq!(error.exit_code(), 1);
    assert_eq!(
        error.to_string(),
        "cannot allocate a fresh same-parent new project staging directory"
    );
    assert!(names(&root).is_empty());
    fs::remove_dir(root).unwrap();
}

fn fixture(relative: bool) -> (PathBuf, PathBuf) {
    let name = format!(
        ".semaprax-new-cli-binding-{}-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos(),
        STAGING_SERIAL.fetch_add(1, Ordering::Relaxed)
    );
    let requested = if relative {
        PathBuf::from(name)
    } else {
        std::env::temp_dir().join(name)
    };
    fs::create_dir(&requested).unwrap();
    let root = requested.canonicalize().unwrap();
    (root, requested)
}

fn names(path: &Path) -> Vec<String> {
    assert!(is_plain_directory(&fs::symlink_metadata(path).unwrap()));
    let mut names = fs::read_dir(path)
        .unwrap()
        .map(|entry| entry.unwrap().file_name().into_string().unwrap())
        .collect::<Vec<_>>();
    names.sort();
    names
}

fn remove_file(path: &Path, expected: &[u8]) {
    let metadata = fs::symlink_metadata(path).unwrap();
    assert!(metadata.is_file() && !metadata.file_type().is_symlink());
    assert!(!metadata_is_reparse(&metadata));
    assert_eq!(fs::read(path).unwrap(), expected);
    fs::remove_file(path).unwrap();
}

fn assert_project(path: &Path) {
    assert_eq!(names(path), ["README.md", "semaprax.toml", "src"]);
    assert_eq!(names(&path.join("src")), ["app.spx", "tests.spx"]);
    for file in templates::render("calculator") {
        let metadata = fs::symlink_metadata(path.join(file.path)).unwrap();
        assert!(metadata.is_file() && !metadata.file_type().is_symlink());
        assert!(!metadata_is_reparse(&metadata));
        assert_eq!(fs::read(path.join(file.path)).unwrap(), file.bytes);
    }
}

fn remove_project(path: &Path) {
    assert_project(path);
    for file in templates::render("calculator") {
        remove_file(&path.join(file.path), &file.bytes);
    }
    fs::remove_dir(path.join("src")).unwrap();
    fs::remove_dir(path).unwrap();
}

#[test]
fn relative_and_parent_relative_destinations_keep_the_original_return_value() {
    for relative in [false, true] {
        let (root, requested) = fixture(relative);
        fs::create_dir(root.join("child")).unwrap();
        let destination = requested.join("child/../calculator");
        assert_eq!(
            create_with_hook(&destination, "calculator", &NoopWriteHook).unwrap(),
            destination
        );
        assert_eq!(names(&root), ["calculator", "child"]);
        remove_project(&root.join("calculator"));
        fs::remove_dir(root.join("child")).unwrap();
        fs::remove_dir(root).unwrap();
    }
}

#[cfg(unix)]
struct AfterPublish(Box<dyn Fn() -> Result<(), String>>);

#[cfg(unix)]
impl WriteHook for AfterPublish {
    fn before_write(&self, _index: usize, _relative_path: &str) -> Result<(), String> {
        Ok(())
    }

    fn after_publish(&self) -> Result<(), String> {
        (self.0)()
    }
}

#[cfg(unix)]
#[test]
fn post_publish_parent_and_output_displacement_preserve_both_trees() {
    for move_parent in [false, true] {
        let (root, _) = fixture(false);
        let parent = root.join("parent");
        fs::create_dir(&parent).unwrap();
        let destination = parent.join("calculator");
        let moved = root.join("moved");
        let replaced = if move_parent {
            parent.clone()
        } else {
            destination.clone()
        };
        let target = replaced.clone();
        let displaced = moved.clone();
        let hook = AfterPublish(Box::new(move || {
            fs::rename(&target, &displaced).unwrap();
            fs::create_dir(&target).unwrap();
            fs::write(target.join("foreign"), b"unchanged\n").unwrap();
            Ok(())
        }));
        let error = create_with_hook(&destination, "calculator", &hook).unwrap_err();
        assert_eq!(error.exit_code(), 1);
        assert_eq!(names(&root), ["moved", "parent"]);
        assert_eq!(names(&replaced), ["foreign"]);
        let project = if move_parent {
            moved.join("calculator")
        } else {
            moved.clone()
        };
        assert_project(&project);
        remove_file(&replaced.join("foreign"), b"unchanged\n");
        fs::remove_dir(replaced).unwrap();
        remove_project(&project);
        if move_parent {
            assert!(names(&moved).is_empty());
            fs::remove_dir(moved).unwrap();
        } else {
            assert!(names(&parent).is_empty());
            fs::remove_dir(parent).unwrap();
        }
        fs::remove_dir(root).unwrap();
    }
}

#[cfg(unix)]
#[test]
fn original_ancestor_alias_is_rechecked_without_adopting_its_new_target() {
    for replace_alias in [false, true] {
        let (root, _) = fixture(false);
        let real = root.join("real");
        let foreign = root.join("foreign");
        for path in [&real, &foreign] {
            fs::create_dir(path).unwrap();
            fs::create_dir(path.join("parent")).unwrap();
        }
        fs::write(foreign.join("parent/sentinel"), b"foreign\n").unwrap();
        let alias = root.join("alias");
        std::os::unix::fs::symlink(&real, &alias).unwrap();
        let hook_alias = alias.clone();
        let hook_foreign = foreign.clone();
        let hook = AfterPublish(Box::new(move || {
            if replace_alias {
                assert!(fs::symlink_metadata(&hook_alias)
                    .unwrap()
                    .file_type()
                    .is_symlink());
                fs::remove_file(&hook_alias).unwrap();
                std::os::unix::fs::symlink(&hook_foreign, &hook_alias).unwrap();
            }
            Ok(())
        }));
        let destination = alias.join("parent/calculator");
        let result = create_with_hook(&destination, "calculator", &hook);
        if replace_alias {
            assert_eq!(result.unwrap_err().exit_code(), 1);
        } else {
            assert_eq!(result.unwrap(), destination);
        }
        // Canonical parent and output still bind in BOTH cases. Only the
        // original spelling distinguishes the displaced-alias failure.
        assert_project(&real.join("parent/calculator"));
        assert_eq!(names(&foreign.join("parent")), ["sentinel"]);
        assert!(fs::symlink_metadata(&alias)
            .unwrap()
            .file_type()
            .is_symlink());
        assert_eq!(
            fs::read_link(&alias).unwrap(),
            if replace_alias {
                foreign.clone()
            } else {
                real.clone()
            }
        );
        fs::remove_file(alias).unwrap();
        remove_project(&real.join("parent/calculator"));
        remove_file(&foreign.join("parent/sentinel"), b"foreign\n");
        for path in [&real, &foreign] {
            assert_eq!(names(path), ["parent"]);
            fs::remove_dir(path.join("parent")).unwrap();
            fs::remove_dir(path).unwrap();
        }
        fs::remove_dir(root).unwrap();
    }
}
