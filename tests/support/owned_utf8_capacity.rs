//! Minimal real Project-v10 source: exactly one owned String literal.
use std::fs::{self, OpenOptions};
use std::io::Write as _;
use std::path::{Path, PathBuf};

fn write_new(path: &Path, bytes: &[u8]) {
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
        .unwrap();
    file.write_all(bytes).unwrap();
}

pub fn write_project(root: &Path, byte_len: usize) -> PathBuf {
    assert!(matches!(byte_len, 65_535..=65_537));
    assert!(root.is_absolute());
    let metadata = fs::symlink_metadata(root).unwrap();
    assert!(metadata.is_dir() && !metadata.file_type().is_symlink());
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt as _;
        assert_eq!(metadata.file_attributes() & 0x400, 0);
    }
    assert_eq!(fs::read_dir(root).unwrap().count(), 0);
    let unit = "\u{feff}\0世é🙂";
    assert_eq!(
        unit.as_bytes(),
        &[239, 187, 191, 0, 228, 184, 150, 195, 169, 240, 159, 153, 130]
    );
    assert_eq!(unit.len() * 5_041, 65_533);
    let padding = "a".repeat(byte_len - 65_533);
    assert_eq!(unit.repeat(5_041).len() + padding.len(), byte_len);
    // Source escapes must use SPX syntax, not JSON's control escape grammar.
    let literal = format!("{}{}", r"\u{feff}\u{0}世é🙂".repeat(5_041), padding);
    let app = format!("module utf8.capacity;\n@id(\"utf8.maximum\") fn maximum() -> string {{ \"{literal}\" }}\n@id(\"capacity.main\") fn main() -> i64 {{ 0 }}\n");
    fs::create_dir(root.join("src")).unwrap();
    let manifest = root.join("semaprax.toml");
    write_new(&manifest, b"schema = \"semaprax.project.v10\"\nname = \"owned-utf8-capacity\"\nversion = \"0.1.0\"\nprofile = \"owned-utf8-api.v1\"\nentry = \"utf8.capacity\"\nsources = [\"src/app.spx\", \"src/tests.spx\"]\nweb_exports = [\"utf8.maximum\"]\ntests = [\"capacity.tests\"]\n");
    for (name, source) in [
        ("app.spx", app.as_str()),
        (
            "tests.spx",
            "module capacity.tests; @id(\"capacity.tests.main\") fn main() -> i64 { 0 }",
        ),
    ] {
        let path = root.join("src").join(name);
        let parsed = semaprax::parse(source, &path).unwrap();
        let canonical = semaprax::format::canonical(&parsed);
        assert_eq!(
            semaprax::format::canonical(&semaprax::parse(&canonical, &path).unwrap()),
            canonical
        );
        write_new(&path, canonical.as_bytes());
    }
    manifest
}
