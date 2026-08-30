//! Real four-source Project subject; no descriptor or target supplies literals.
use std::fs::{self, OpenOptions};
use std::io::Write as _;
use std::path::{Path, PathBuf};

const APP: &str = r#"module utf8.product;
use function @id("helper.left\u{8}\u{c}\u{7f}\u{85}") from utf8.left as finish_left;
use function @id("helper.right") from utf8.right as finish_right;
@id("bytes.raw") fn raw(input: borrow Slice<u8>) -> Bytes { bytes_copy(input) }
@id("utf8.left") fn left(divisor: i64) -> string { finish_left("temporary", 84 / divisor) }
@id("utf8.right") fn right(divisor: i64) -> string { finish_right("temporary", 84 / divisor) }
@id("product.main") fn main() -> i64 { 0 }
"#;
const LEFT: &str = r#"module utf8.left;
@id("helper.left\u{8}\u{c}\u{7f}\u{85}")
fn finish(unused: string, checked: i64) -> string { "\u{feff}hello\u{0}世界é" }
"#;
const RIGHT: &str = r#"module utf8.right;
@id("helper.right")
fn finish(unused: string, checked: i64) -> string { "" }
"#;

fn write_new(path: &Path, bytes: &[u8]) {
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
        .unwrap();
    file.write_all(bytes).unwrap();
}

pub fn write_project(root: &Path, renamed: bool) -> PathBuf {
    assert!(root.is_absolute());
    let metadata = fs::symlink_metadata(root).unwrap();
    assert!(metadata.is_dir() && !metadata.file_type().is_symlink());
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt as _;
        assert_eq!(metadata.file_attributes() & 0x400, 0);
    }
    assert_eq!(fs::read_dir(root).unwrap().count(), 0);
    fs::create_dir(root.join("src")).unwrap();
    let manifest = root.join("semaprax.toml");
    write_new(&manifest, b"schema = \"semaprax.project.v10\"\nname = \"owned-utf8-product\"\nversion = \"0.1.0\"\nprofile = \"owned-utf8-api.v1\"\nentry = \"utf8.product\"\nsources = [\"src/app.spx\", \"src/left.spx\", \"src/right.spx\", \"src/tests.spx\"]\nweb_exports = [\"bytes.raw\", \"utf8.left\", \"utf8.right\"]\ntests = [\"utf8.tests\"]\n");
    let right = if renamed {
        assert_eq!(RIGHT.matches("fn finish(").count(), 1);
        RIGHT.replacen("fn finish(", "fn renamed_finish(", 1)
    } else {
        RIGHT.to_owned()
    };
    for (name, source) in [
        ("app.spx", APP),
        ("left.spx", LEFT),
        ("right.spx", right.as_str()),
        (
            "tests.spx",
            "module utf8.tests; @id(\"product.tests.main\") fn main() -> i64 { 0 }",
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
