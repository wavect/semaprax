//! Authored Windows filesystem observations; no target/runtime is executed.
use super::*;
use std::collections::BTreeMap;
use std::fs;
use std::io::Write as _;
use std::os::windows::fs::MetadataExt as _;

mod project_tests;

const FILES: [(&str, &[u8]); 6] = [
    ("app.wasm", b"wasm\0bytes"),
    ("semaprax.js", b"javascript\n"),
    ("semaprax.bindings.js", b"bindings\n"),
    ("semaprax.bindings.d.ts", b"types\n"),
    ("semaprax.api.json", b"metadata\n"),
    ("package.json", b"package\n"),
];

fn fixture() -> PathBuf {
    let path = std::env::temp_dir().join(format!(
        "semaprax-held-npm-{}-{}-{}",
        std::process::id(),
        SERIAL.fetch_add(1, Ordering::Relaxed),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    fs::create_dir(&path).unwrap();
    path.canonicalize().unwrap()
}

fn stage_name() -> String {
    format!(".semaprax-owned-npm-{}-7", std::process::id())
}
fn assert_failure(result: Result<(), Failure>) {
    let errors = result.unwrap_err();
    assert_eq!(errors.len(), 1);
    assert_eq!(errors[0].code, "SPX-W120");
}

fn package(directory: &str) -> Vec<(String, Vec<u8>)> {
    FILES
        .iter()
        .map(|(name, bytes)| (format!("{directory}/{name}"), bytes.to_vec()))
        .collect()
}

fn plain(path: &Path) -> fs::Metadata {
    let metadata = fs::symlink_metadata(path).unwrap();
    assert!(!metadata.file_type().is_symlink());
    assert_eq!(metadata.file_attributes() & 0x400, 0);
    metadata
}

// Authenticate the entire fixed removal plan before any deletion. Failed
// assertions retain the fixture; no recursive deletion or deleting Drop.
fn finish(root: &Path, directories: &[String], files: &[(String, Vec<u8>)]) {
    let mut expected = BTreeMap::<PathBuf, Vec<OsString>>::new();
    expected.insert(PathBuf::new(), Vec::new());
    for directory in directories {
        let path = Path::new(directory);
        assert!(path
            .components()
            .all(|part| matches!(part, Component::Normal(_))));
        assert!(expected.insert(path.to_path_buf(), Vec::new()).is_none());
    }
    for entry in directories
        .iter()
        .map(String::as_str)
        .chain(files.iter().map(|(name, _)| name.as_str()))
    {
        let path = Path::new(entry);
        assert!(path
            .components()
            .all(|part| matches!(part, Component::Normal(_))));
        expected
            .get_mut(path.parent().unwrap())
            .unwrap()
            .push(path.file_name().unwrap().to_os_string());
    }
    for (directory, names) in &mut expected {
        let path = root.join(directory);
        assert!(plain(&path).is_dir());
        let mut actual = fs::read_dir(&path)
            .unwrap()
            .map(|entry| entry.unwrap().file_name())
            .collect::<Vec<_>>();
        actual.sort();
        names.sort();
        assert_eq!(&actual, names);
    }
    for (name, bytes) in files {
        let path = root.join(name);
        assert!(plain(&path).is_file());
        assert_eq!(fs::read(&path).unwrap(), *bytes);
    }
    for (name, _) in files {
        fs::remove_file(root.join(name)).unwrap();
    }
    let mut paths = directories.iter().map(Path::new).collect::<Vec<_>>();
    paths.sort_by_key(|path| std::cmp::Reverse(path.components().count()));
    for path in paths {
        fs::remove_dir(root.join(path)).unwrap();
    }
    fs::remove_dir(root).unwrap();
}

#[test]
fn exact_six_files_publish_in_unicode_parent_without_early_output() {
    let root = fixture();
    let parent = root.join("parent space λ");
    fs::create_dir(&parent).unwrap();
    let output = parent.join("package");
    let mut writes = 0;
    publish_files(
        &output,
        FILES,
        || 7,
        |point, _| {
            if matches!(
                point,
                Point::Created
                    | Point::BeforeWrite(_)
                    | Point::BeforeSettlement
                    | Point::AfterSettlement
            ) {
                assert!(!output.exists());
            }
            if let Point::BeforeWrite(index) = point {
                assert_eq!(index, writes);
                writes += 1;
            }
            Ok(())
        },
    )
    .unwrap();
    assert_eq!(writes, 6);
    finish(
        &root,
        &["parent space λ".into(), "parent space λ/package".into()],
        &package("parent space λ/package"),
    );
}

#[test]
fn existing_file_and_directory_are_preserved_before_staging() {
    for directory in [false, true] {
        let root = fixture();
        let output = root.join("package");
        let (dirs, files) = if directory {
            fs::create_dir(&output).unwrap();
            fs::write(output.join("foreign"), b"foreign").unwrap();
            (
                vec!["package".into()],
                vec![("package/foreign".into(), b"foreign".to_vec())],
            )
        } else {
            fs::write(&output, b"foreign").unwrap();
            (vec![], vec![("package".into(), b"foreign".to_vec())])
        };
        assert_failure(publish_files(
            &output,
            FILES,
            || panic!("must not select stage"),
            |_, _| panic!("must not create stage"),
        ));
        finish(&root, &dirs, &files);
    }
}

#[test]
fn invalid_names_inventory_and_missing_parent_have_no_effects() {
    let root = fixture();
    for name in [
        "NUL",
        "aux.txt",
        "package:stream",
        "package.",
        "package ",
        "λ",
        "missing/package",
        "../package",
    ] {
        assert_failure(publish_files(
            &root.join(name),
            FILES,
            || panic!("invalid input selected stage: {name:?}"),
            |_, _| panic!("invalid input created stage: {name:?}"),
        ));
    }
    let mut files = FILES;
    files[0].0 = "../app.wasm";
    assert_failure(publish_files(
        &root.join("package"),
        files,
        || panic!("bad inventory selected stage"),
        |_, _| panic!("bad inventory created stage"),
    ));
    finish(&root, &[], &[]);
}

#[test]
fn stage_alias_is_skipped_and_existing_stage_is_not_adopted() {
    let root = fixture();
    let first = stage_name();
    let second = format!(".semaprax-owned-npm-{}-8", std::process::id());
    let output = root.join(first.to_ascii_uppercase());
    fs::create_dir(root.join(&second)).unwrap();
    fs::write(root.join(&second).join("foreign"), b"foreign").unwrap();
    let mut next = 6;
    publish_files(
        &output,
        FILES,
        || {
            next += 1;
            next
        },
        |_, _| Ok(()),
    )
    .unwrap();
    assert_eq!(next, 9);
    let output_name = output.file_name().unwrap().to_str().unwrap();
    let mut files = package(output_name);
    files.push((format!("{second}/foreign"), b"foreign".to_vec()));
    finish(&root, &[output_name.into(), second], &files);
}

#[test]
fn exact_stage_alias_exhausts_32_attempts_without_creating_anything() {
    let root = fixture();
    let mut attempts = 0;
    assert_failure(publish_files(
        &root.join(stage_name()),
        FILES,
        || {
            attempts += 1;
            7
        },
        |_, _| panic!("exhausted alias created a stage"),
    ));
    assert_eq!(attempts, ATTEMPTS);
    finish(&root, &[], &[]);
}

#[test]
fn relative_output_resolves_without_changing_process_directory() {
    let current = std::env::current_dir().unwrap();
    let name = format!(
        ".semaprax-npm-relative-{}-{}",
        std::process::id(),
        SERIAL.fetch_add(1, Ordering::Relaxed)
    );
    let root = current.join(&name);
    fs::create_dir(&root).unwrap();
    publish_files(
        &Path::new(&name).join("package"),
        FILES,
        || 7,
        |_, _| Ok(()),
    )
    .unwrap();
    assert_eq!(std::env::current_dir().unwrap(), current);
    finish(&root, &["package".into()], &package("package"));
}

#[test]
fn pre_settlement_failure_discards_only_tracked_prefix() {
    let root = fixture();
    assert_failure(publish_files(
        &root.join("package"),
        FILES,
        || 7,
        |point, _| {
            if point == Point::BeforeWrite(3) {
                Err(failure("injected write boundary"))
            } else {
                Ok(())
            }
        },
    ));
    finish(&root, &[], &[]);
}

#[test]
fn untracked_partial_file_preserves_stage_and_primary_failure() {
    let root = fixture();
    let name = stage_name();
    let errors = publish_files(
        &root.join("package"),
        FILES,
        || 7,
        |point, stage| {
            if point == Point::BeforeWrite(2) {
                fs::OpenOptions::new()
                    .write(true)
                    .create_new(true)
                    .open(stage.join(NAMES[2]))
                    .unwrap();
                Err(failure("injected partial file"))
            } else {
                Ok(())
            }
        },
    )
    .unwrap_err();
    assert_eq!(errors[0].message, "injected partial file");
    let mut files = package(&name);
    files.truncate(2);
    files.push((format!("{name}/{}", NAMES[2]), vec![]));
    finish(&root, &[name], &files);
}

#[test]
fn settled_failure_and_rename_collision_never_discard_stage() {
    for collision in [false, true] {
        let root = fixture();
        let output = root.join("package");
        let name = stage_name();
        assert_failure(publish_files(
            &output,
            FILES,
            || 7,
            |point, _| {
                if point == Point::AfterSettlement {
                    if collision {
                        fs::write(&output, b"foreign output").unwrap();
                    } else {
                        return Err(failure("injected settlement boundary"));
                    }
                }
                Ok(())
            },
        ));
        let mut files = package(&name);
        if collision {
            files.push(("package".into(), b"foreign output".to_vec()));
        }
        finish(&root, &[name], &files);
    }
}

#[test]
fn real_settlement_recheck_failure_retains_changed_file_and_all_stage_entries() {
    let root = fixture();
    let name = stage_name();
    let mut reached_after_settlement = false;
    assert_failure(publish_files(
        &root.join("package"),
        FILES,
        || 7,
        |point, stage| {
            if point == Point::BeforeSettlement {
                fs::OpenOptions::new()
                    .append(true)
                    .open(stage.join(NAMES[0]))
                    .unwrap()
                    .write_all(b"+")
                    .unwrap();
            }
            if point == Point::AfterSettlement {
                reached_after_settlement = true;
            }
            Ok(())
        },
    ));
    assert!(!reached_after_settlement);
    let mut files = package(&name);
    files[0].1.push(b'+');
    finish(&root, &[name], &files);
}

#[test]
fn published_failure_and_move_back_never_restore_cleanup() {
    for move_back in [false, true] {
        let root = fixture();
        let output = root.join("package");
        let name = stage_name();
        assert_failure(publish_files(
            &output,
            FILES,
            || 7,
            |point, published| {
                if point == Point::AfterRename {
                    if move_back {
                        fs::rename(published, root.join(&name)).unwrap();
                    }
                    return Err(failure("injected post-publication failure"));
                }
                Ok(())
            },
        ));
        let directory = if move_back { name } else { "package".into() };
        finish(
            &root,
            std::slice::from_ref(&directory),
            &package(&directory),
        );
    }
}

#[test]
fn substituted_stage_is_neither_written_nor_cleaned() {
    let root = fixture();
    let name = stage_name();
    assert_failure(publish_files(
        &root.join("package"),
        FILES,
        || 7,
        |point, stage| {
            if point == Point::Created {
                fs::rename(stage, root.join("moved")).unwrap();
                fs::create_dir(stage).unwrap();
                fs::write(stage.join("foreign"), b"foreign").unwrap();
            }
            Ok(())
        },
    ));
    finish(
        &root,
        &["moved".into(), name.clone()],
        &[(format!("{name}/foreign"), b"foreign".to_vec())],
    );
}

#[test]
fn substituted_published_path_fails_final_binding_and_preserves_both_trees() {
    let root = fixture();
    assert_failure(publish_files(
        &root.join("package"),
        FILES,
        || 7,
        |point, output| {
            if point == Point::AfterRename {
                fs::rename(output, root.join("moved")).unwrap();
                fs::create_dir(output).unwrap();
                fs::write(output.join("foreign"), b"foreign").unwrap();
            }
            Ok(())
        },
    ));
    let mut files = package("moved");
    files.push(("package/foreign".into(), b"foreign".to_vec()));
    finish(&root, &["moved".into(), "package".into()], &files);
}

#[test]
fn displaced_parent_cannot_turn_held_publication_into_success_at_replacement_path() {
    let root = fixture();
    let parent = root.join("parent");
    fs::create_dir(&parent).unwrap();
    let mut displacement_denied = false;
    assert_failure(publish_files(
        &parent.join("package"),
        FILES,
        || 7,
        |point, _| {
            if point == Point::AfterRename {
                if let Err(error) = fs::rename(&parent, root.join("moved-parent")) {
                    if error.kind() == std::io::ErrorKind::PermissionDenied {
                        displacement_denied = true;
                        return Err(failure(
                            "retained Windows authority denied parent displacement",
                        ));
                    }
                    panic!("unexpected parent displacement failure: {error}");
                }
                fs::create_dir(&parent).unwrap();
                fs::write(parent.join("foreign"), b"foreign").unwrap();
            }
            Ok(())
        },
    ));
    if displacement_denied {
        finish(
            &root,
            &["parent".into(), "parent/package".into()],
            &package("parent/package"),
        );
    } else {
        let mut files = package("moved-parent/package");
        files.push(("parent/foreign".into(), b"foreign".to_vec()));
        finish(
            &root,
            &[
                "parent".into(),
                "moved-parent".into(),
                "moved-parent/package".into(),
            ],
            &files,
        );
    }
}

#[test]
#[ignore = "requires Windows symbolic-link creation privilege; no privilege skip"]
fn reparse_destination_is_not_followed() {
    let root = fixture();
    let target = root.join("target");
    let output = root.join("package");
    fs::create_dir(&target).unwrap();
    fs::write(target.join("foreign"), b"foreign").unwrap();
    std::os::windows::fs::symlink_dir(&target, &output).unwrap();
    assert_failure(publish_files(
        &output,
        FILES,
        || panic!("reparse output selected stage"),
        |_, _| panic!("reparse output created stage"),
    ));
    // Inspect the exact link and its target before the only link unlink.
    assert!(fs::symlink_metadata(&output)
        .unwrap()
        .file_type()
        .is_symlink());
    assert_eq!(fs::read_link(&output).unwrap(), target);
    assert!(plain(&root).is_dir());
    assert!(plain(&target).is_dir());
    assert!(plain(&target.join("foreign")).is_file());
    let mut entries = fs::read_dir(&root)
        .unwrap()
        .map(|entry| entry.unwrap().file_name())
        .collect::<Vec<_>>();
    entries.sort();
    assert_eq!(
        entries,
        [OsString::from("package"), OsString::from("target")]
    );
    assert_eq!(
        fs::read_dir(&target)
            .unwrap()
            .map(|entry| entry.unwrap().file_name())
            .collect::<Vec<_>>(),
        [OsString::from("foreign")]
    );
    assert_eq!(fs::read(target.join("foreign")).unwrap(), b"foreign");
    fs::remove_dir(&output).unwrap();
    finish(
        &root,
        &["target".into()],
        &[("target/foreign".into(), b"foreign".to_vec())],
    );
}
