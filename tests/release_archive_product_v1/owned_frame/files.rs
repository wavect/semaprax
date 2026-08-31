use super::*;
use std::collections::BTreeMap;
use std::io::Write as _;

pub(super) fn write(root: &Path, path: &str, bytes: &[u8]) {
    fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(root.join(path))
        .unwrap()
        .write_all(bytes)
        .unwrap();
}

pub(super) fn assert_names(root: &Path, names: &[&str]) {
    let metadata = fs::symlink_metadata(root).unwrap();
    assert!(metadata.is_dir() && !metadata.file_type().is_symlink());
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt as _;
        assert_eq!(metadata.file_attributes() & 0x400, 0);
    }
    let mut actual = fs::read_dir(root)
        .unwrap()
        .map(|entry| entry.unwrap().file_name().into_string().unwrap())
        .collect::<Vec<_>>();
    actual.sort();
    let mut expected = names.to_vec();
    expected.sort_unstable();
    assert_eq!(actual, expected);
}

pub(super) fn project(root: &Path, renamed: bool) {
    fs::create_dir(root).unwrap();
    fs::create_dir(root.join("src")).unwrap();
    let original = include_str!("../../../examples/frame-payload-project/src/frame.spx");
    assert_eq!(original.matches("fn payload_result(").count(), 1);
    let frame = if renamed {
        original.replace("fn payload_result(", "fn decoded_payload_result(")
    } else {
        original.to_owned()
    };
    for (name, bytes) in [
        (
            "semaprax.toml",
            include_bytes!("../../../examples/frame-payload-project/semaprax.toml").as_slice(),
        ),
        (
            "src/app.spx",
            include_bytes!("../../../examples/frame-payload-project/src/app.spx").as_slice(),
        ),
        (
            "src/tests.spx",
            include_bytes!("../../../examples/frame-payload-project/src/tests.spx").as_slice(),
        ),
        ("src/frame.spx", frame.as_bytes()),
    ] {
        write(root, name, bytes);
    }
}

pub(super) fn flat_snapshot(root: &Path) -> BTreeMap<String, Vec<u8>> {
    fs::read_dir(root)
        .unwrap()
        .map(|entry| {
            let entry = entry.unwrap();
            let metadata = fs::symlink_metadata(entry.path()).unwrap();
            assert!(metadata.is_file() && !metadata.file_type().is_symlink());
            #[cfg(windows)]
            {
                use std::os::windows::fs::MetadataExt as _;
                assert_eq!(metadata.file_attributes() & 0x400, 0);
            }
            assert!(metadata.len() <= 16 * 1024 * 1024);
            (
                entry.file_name().into_string().unwrap(),
                fs::read(entry.path()).unwrap(),
            )
        })
        .collect()
}

pub(super) fn project_snapshot(root: &Path) -> BTreeMap<String, Vec<u8>> {
    let mut entries = fs::read_dir(root)
        .unwrap()
        .map(|entry| entry.unwrap().file_name().into_string().unwrap())
        .collect::<Vec<_>>();
    entries.sort();
    assert_eq!(entries, ["semaprax.toml", "src"]);
    let metadata = fs::symlink_metadata(root.join("src")).unwrap();
    assert!(metadata.is_dir() && !metadata.file_type().is_symlink());
    let mut result = flat_snapshot(&root.join("src"));
    assert_eq!(
        result.keys().map(String::as_str).collect::<Vec<_>>(),
        ["app.spx", "frame.spx", "tests.spx"]
    );
    let manifest = fs::symlink_metadata(root.join("semaprax.toml")).unwrap();
    assert!(manifest.is_file() && !manifest.file_type().is_symlink());
    result.insert(
        "semaprax.toml".to_owned(),
        fs::read(root.join("semaprax.toml")).unwrap(),
    );
    result
}
