use super::*;
use crate::Fixture;

#[test]
fn archive_admission_rejects_labels_extra_entries_and_modified_literals() {
    let fixture = Fixture::new("admission");
    let root = &fixture.root;
    let commit = "1111111111111111111111111111111111111111";
    let target = "x86_64-unknown-linux-gnu";
    fs::create_dir(root.join("smoke")).unwrap();
    for name in ["semaprax", "semapraxd"] {
        let path = root.join(format!("{name}{}", std::env::consts::EXE_SUFFIX));
        fs::write(&path, b"not executed: admission-only control").unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt as _;
            fs::set_permissions(path, fs::Permissions::from_mode(0o700)).unwrap();
        }
    }
    for (name, bytes) in [
        ("LICENSE", include_bytes!("../../../LICENSE").as_slice()),
        ("README.md", include_bytes!("../../../README.md").as_slice()),
        ("smoke/meaning.spx", SMOKE),
    ] {
        fs::write(root.join(name), bytes).unwrap();
    }
    fs::write(root.join("release-manifest.json"), manifest(commit, target)).unwrap();
    let original = inspect(root, commit, target).unwrap();
    for invalid in [
        "",
        "A111111111111111111111111111111111111111",
        "../1111111111111111111111111111111111111",
    ] {
        assert!(inspect(root, invalid, target).is_err());
    }
    assert!(inspect(root, commit, "foreign-target").is_err());
    fs::write(root.join("extra"), b"sentinel").unwrap();
    assert!(inspect(root, commit, target).is_err());
    fs::remove_file(root.join("extra")).unwrap();
    fs::write(root.join("smoke/meaning.spx"), b"wrong\n").unwrap();
    assert!(inspect(root, commit, target).is_err());
    fs::write(root.join("smoke/meaning.spx"), SMOKE).unwrap();
    let metadata_path = root.join("release-manifest.json");
    let correct = manifest(commit, target);
    fs::write(
        &metadata_path,
        correct.replacen("{", "{\"schema\":\"duplicate\",", 1),
    )
    .unwrap();
    assert!(inspect(root, commit, target).is_err());
    fs::write(&metadata_path, &correct).unwrap();
    let smoke = root.join("smoke/meaning.spx");
    File::options()
        .write(true)
        .open(&smoke)
        .unwrap()
        .set_len(1024 * 1024 + 1)
        .unwrap();
    assert!(inspect(root, commit, target).is_err());
    fs::write(&smoke, SMOKE).unwrap();
    fs::remove_file(&smoke).unwrap();
    fs::create_dir(&smoke).unwrap();
    assert!(inspect(root, commit, target).is_err());
    fs::remove_dir(&smoke).unwrap();
    #[cfg(unix)]
    {
        std::os::unix::fs::symlink(root.join("README.md"), &smoke).unwrap();
        assert!(inspect(root, commit, target).is_err());
        fs::remove_file(&smoke).unwrap();
    }
    fs::write(&smoke, SMOKE).unwrap();
    assert_eq!(inspect(root, commit, target).unwrap(), original);
}
