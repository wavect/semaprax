//! Real v8/v9 subjects for a String plus two byte-slice borrowed tuple.
use std::fs::{self, OpenOptions};
use std::io::Write as _;
use std::path::{Path, PathBuf};

const V8: &str = r#"module tuple.app;
@id("tuple.bytes")
fn bytes(text: borrow str, left: borrow Slice<u8>, right: borrow Slice<u8>) -> Bytes {
    bytes_copy(left)
}
@id("tuple.text")
fn text(text: borrow str, left: borrow Slice<u8>, right: borrow Slice<u8>) -> Bytes {
    bytes_copy(str_as_bytes(text))
}
@id("tuple.maybe")
fn maybe(text: borrow str, left: borrow Slice<u8>, right: borrow Slice<u8>, present: bool) -> Option<Bytes> {
    if present { Option<Bytes>::Some { value: bytes_copy(left) } }
    else { Option<Bytes>::None {} }
}
@id("tuple.result")
fn outcome(text: borrow str, left: borrow Slice<u8>, right: borrow Slice<u8>, ok: bool) -> Result<Bytes, i64> {
    if ok { Result<Bytes, i64>::Ok { value: bytes_copy(left) } }
    else { Result<Bytes, i64>::Err { error: -7 } }
}
@id("tuple.main") fn main() -> i64 { 0 }
"#;

const V9: &str = r#"module tuple.app;
@id("tuple.Record") record Payload {
    @id("bytes") payload: Bytes,
    @id("text") text_bytes: usize,
    @id("left") left_bytes: usize,
    @id("right") right_bytes: usize,
}
@id("tuple.bytes")
fn bytes(text: borrow str, left: borrow Slice<u8>, right: borrow Slice<u8>) -> Payload {
    Payload { payload: bytes_copy(left), text_bytes: byte_len(str_as_bytes(text)), left_bytes: byte_len(left), right_bytes: byte_len(right) }
}
@id("tuple.text")
fn text(text: borrow str, left: borrow Slice<u8>, right: borrow Slice<u8>) -> Payload {
    Payload { payload: bytes_copy(str_as_bytes(text)), text_bytes: byte_len(str_as_bytes(text)), left_bytes: byte_len(left), right_bytes: byte_len(right) }
}
@id("tuple.main") fn main() -> i64 { 0 }
"#;

fn write_new(path: &Path, bytes: &[u8]) {
    OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
        .unwrap()
        .write_all(bytes)
        .unwrap();
}

pub fn write_project(root: &Path, flat: bool) -> PathBuf {
    assert!(root.is_absolute());
    let metadata = fs::symlink_metadata(root).unwrap();
    assert!(metadata.is_dir() && !metadata.file_type().is_symlink());
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt as _;
        assert_eq!(metadata.file_attributes() & 0x400, 0);
    }
    assert_eq!(fs::read_dir(root).unwrap().count(), 0);
    let (schema, profile, exports, source) = if flat {
        (
            "semaprax.project.v9",
            "flat-owned-record-api.v1",
            "\"tuple.bytes\", \"tuple.text\"",
            V9,
        )
    } else {
        (
            "semaprax.project.v8",
            "owned-data-api.v1",
            "\"tuple.bytes\", \"tuple.maybe\", \"tuple.result\", \"tuple.text\"",
            V8,
        )
    };
    let manifest = root.join("semaprax.toml");
    let text = format!("schema = \"{schema}\"\nname = \"owned-tuple-product\"\nversion = \"0.1.0\"\nprofile = \"{profile}\"\nentry = \"tuple.app\"\nsources = [\"src/app.spx\", \"src/tests.spx\"]\nweb_exports = [{exports}]\ntests = [\"tuple.tests\"]\n");
    write_new(&manifest, text.as_bytes());
    fs::create_dir(root.join("src")).unwrap();
    for (name, source) in [
        ("app.spx", source),
        (
            "tests.spx",
            "module tuple.tests; @id(\"tuple.tests.main\") fn main() -> i64 { 0 }",
        ),
    ] {
        let path = root.join("src").join(name);
        let parsed = semaprax::check(source, &path).unwrap();
        let canonical = semaprax::format::canonical(&parsed);
        assert_eq!(
            semaprax::format::canonical(&semaprax::parse(&canonical, &path).unwrap()),
            canonical
        );
        write_new(&path, canonical.as_bytes());
    }
    manifest
}
