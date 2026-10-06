use super::*;
use crate::Fixture;

const RUNTIME: &str = "{\n    \"os\": \"linux\",\n    \"min_os_version\": null,\n    \"cpu\": \"x86_64\",\n    \"libc_family\": \"glibc\",\n    \"min_libc_version\": \"2.17\",\n    \"dynamic_libraries\": [\"libc.so.6\"]\n  }";

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
    let readme_bytes = readme(target);
    for (name, bytes) in [
        ("LICENSE", include_bytes!("../../../LICENSE").as_slice()),
        ("README.md", readme_bytes.as_slice()),
        ("smoke/meaning.spx", SMOKE),
    ] {
        fs::write(root.join(name), bytes).unwrap();
    }
    let runtime = RUNTIME;
    fs::write(
        root.join("release-manifest.json"),
        manifest_with_runtime(commit, target, runtime),
    )
    .unwrap();
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
    let correct = manifest_with_runtime(commit, target, runtime);
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

#[test]
fn archive_admission_rejects_a_forged_or_inconsistent_runtime_record() {
    let commit = "1111111111111111111111111111111111111111";
    let target = "x86_64-unknown-linux-gnu";
    let good = manifest_with_runtime(commit, target, RUNTIME);
    assert!(manifest_check(good.as_bytes(), commit, target).is_ok());
    // Varying build values are admitted; the target-consistency rules are not.
    for runtime in [
        RUNTIME.replace("2.17", "2.35"),
        RUNTIME.replace("[\"libc.so.6\"]", "[]"),
    ] {
        let text = manifest_with_runtime(commit, target, &runtime);
        assert!(manifest_check(text.as_bytes(), commit, target).is_ok());
    }
    for runtime in [
        RUNTIME.replace("\"linux\"", "\"macos\""),
        RUNTIME.replace("\"x86_64\"", "\"aarch64\""),
        RUNTIME.replace("\"glibc\"", "\"musl\""),
        RUNTIME.replace("\"2.17\"", "null"),
        RUNTIME.replace("\"min_os_version\": null", "\"min_os_version\": \"11.0\""),
        RUNTIME.replace(
            "\"dynamic_libraries\": [\"libc.so.6\"]",
            "\"dynamic_libraries\": null",
        ),
        RUNTIME.replace("    \"cpu\": \"x86_64\",\n", ""),
        RUNTIME.replace(
            "\"os\": \"linux\",",
            "\"os\": \"linux\", \"os\": \"linux\",",
        ),
        RUNTIME.replace("{\n", "{\n    \"extra\": 1,\n"),
        "[]".to_owned(),
    ] {
        let text = manifest_with_runtime(commit, target, &runtime);
        assert!(
            manifest_check(text.as_bytes(), commit, target).is_err(),
            "{runtime}"
        );
    }
    // No runtime record, a runtime record in the wrong place, and a changed
    // fixed field are all rejected.
    assert!(manifest_check(manifest(commit, target).as_bytes(), commit, target).is_err());
    let moved = good.replace("\"schema\"", "\"schema2\"");
    assert!(manifest_check(moved.as_bytes(), commit, target).is_err());
    assert!(manifest_check(good.as_bytes(), commit, "aarch64-unknown-linux-gnu").is_err());
}
