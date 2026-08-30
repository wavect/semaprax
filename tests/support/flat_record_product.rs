//! Shared source subject for the published npm and native Rust v9 consumers.
//! No target generator or descriptor supplies these source/identity literals.

use std::fs::{self, OpenOptions};
use std::io::Write as _;
use std::path::{Path, PathBuf};

const MANIFEST: &str = concat!(
    "schema = \"semaprax.project.v9\"\n",
    "name = \"flat-record-product\"\n",
    "version = \"0.1.0\"\n",
    "profile = \"flat-owned-record-api.v1\"\n",
    "entry = \"product.app\"\n",
    "sources = [\"src/app.spx\", \"src/left.spx\", \"src/right.spx\", \"src/tests.spx\"]\n",
    "web_exports = [\"left.payload\", \"right.payload\"]\n",
    "tests = [\"product.tests\"]\n",
);

const LEFT: &str = r#"module product.left;
@id("left.Payload\u{8}\u{c}\u{7f}\u{85}")
record Payload {
    @id("") bytes: Bytes,
    @id("left.count") count: i64,
    @id("left.valid") valid: bool,
    @id("left.size") size: usize,
}
@id("left.payload")
fn payload(input: borrow Slice<u8>, divisor: i64, valid: bool) -> Payload {
    Payload {
        bytes: bytes_copy(input),
        count: 84 / divisor,
        valid: valid,
        size: byte_len(input),
    }
}
"#;

const RIGHT: &str = r#"module product.right;
@id("right.Payload")
record Payload {
    @id("right.size") size: usize,
    @id("right.valid") valid: bool,
    @id("right.count") count: i64,
    @id("right.bytes") bytes: Bytes,
}
@id("right.payload")
fn payload(input: borrow Slice<u8>, divisor: i64, valid: bool) -> Payload {
    Payload {
        size: byte_len(input),
        valid: valid,
        count: 84 / divisor,
        bytes: bytes_copy(input),
    }
}
"#;

fn rename_right(source: &str) -> String {
    let mut renamed = source.to_owned();
    // Match syntax, not arbitrary display-name substrings: the persistent
    // right.Payload and right.payload identities must remain byte-identical.
    for (before, after) in [
        ("record Payload {", "record RenamedPayload {"),
        ("-> Payload {", "-> RenamedPayload {"),
        ("    Payload {", "    RenamedPayload {"),
        ("fn payload(", "fn renamed_payload("),
    ] {
        assert_eq!(renamed.matches(before).count(), 1);
        renamed = renamed.replacen(before, after, 1);
    }
    renamed
}

fn write_new(path: &Path, bytes: &[u8]) {
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
        .unwrap();
    file.write_all(bytes).unwrap();
}

/// The caller owns a freshly created root; never overwrite an existing source
/// tree. Each backend consumes the same source constants, declaration order,
/// stable identities, and division-before/after-allocation cases.
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
    write_new(&manifest, MANIFEST.as_bytes());
    let right = if renamed {
        rename_right(RIGHT)
    } else {
        RIGHT.to_owned()
    };
    for (name, source) in [
        (
            "app.spx",
            "module product.app;\n@id(\"product.main\") fn main() -> i64 { 0 }\n",
        ),
        ("left.spx", LEFT),
        ("right.spx", right.as_str()),
        (
            "tests.spx",
            "module product.tests;\n@id(\"product.tests.main\") fn main() -> i64 { 0 }\n",
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
