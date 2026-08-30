//! One checked Project subject shared by every Result-extrema consumer.
use std::fs::{self, OpenOptions};
use std::io::Write as _;
use std::path::{Path, PathBuf};

const SOURCE: &str = r#"module result.app;
@id("result.value")
fn value(input: borrow Slice<u8>) -> Result<Bytes, i64> {
    let length = byte_len(input);
    if length == 0usize { Result<Bytes, i64>::Err { error: 0 } }
    else if length == 1usize { Result<Bytes, i64>::Err { error: 0 - 9223372036854775807 - 1 } }
    else if length == 2usize { Result<Bytes, i64>::Err { error: 9223372036854775807 } }
    else {
        let owned = bytes_copy(input);
        let divisor = if length == 4usize { 0 } else { 1 };
        let checked = 1 / divisor;
        Result<Bytes, i64>::Ok { value: owned }
    }
}
@id("result.main") fn main() -> i64 { 0 }
"#;

pub fn write_project(root: &Path) -> PathBuf {
    assert!(root.is_absolute());
    assert_eq!(fs::read_dir(root).unwrap().count(), 0);
    fs::create_dir(root.join("src")).unwrap();
    let manifest = root.join("semaprax.toml");
    let text = "schema = \"semaprax.project.v8\"\nname = \"owned-result-extrema\"\nversion = \"0.1.0\"\nprofile = \"owned-data-api.v1\"\nentry = \"result.app\"\nsources = [\"src/app.spx\", \"src/tests.spx\"]\nweb_exports = [\"result.value\"]\ntests = [\"result.tests\"]\n";
    for (path, text) in [
        (manifest.clone(), text.to_owned()),
        (root.join("src/app.spx"), canonical(SOURCE)),
        (
            root.join("src/tests.spx"),
            canonical("module result.tests; @id(\"result.tests.main\") fn main() -> i64 { 0 }"),
        ),
    ] {
        OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(path)
            .unwrap()
            .write_all(text.as_bytes())
            .unwrap();
    }
    manifest
}

fn canonical(source: &str) -> String {
    let checked = semaprax::check(source, "result.spx").unwrap();
    let text = semaprax::format::canonical(&checked);
    assert_eq!(
        semaprax::format::canonical(&semaprax::parse(&text, "result.spx").unwrap()),
        text
    );
    text
}
