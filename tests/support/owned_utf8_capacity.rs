//! Minimal real Project-v10 source: exactly one owned String literal.
use std::fs::{self, OpenOptions};
use std::io::Write as _;
use std::path::{Path, PathBuf};

const LITERAL_UNIT: &str = concat!("\u{feff}", r"\u{0}世é🙂");
const TEST_SOURCE: &str =
    "module capacity.tests;\n\n@id(\"capacity.tests.main\")\nfn main() -> i64\n{\n    0\n}\n";

fn app_source(literal: &str) -> String {
    format!("module utf8.capacity;\n\n@id(\"utf8.maximum\")\nfn maximum() -> string\n{{\n    \"{literal}\"\n}}\n\n@id(\"capacity.main\")\nfn main() -> i64\n{{\n    0\n}}\n")
}

#[test]
fn authored_fixture_template_matches_the_canonical_formatter() {
    // Pin the same template and scalar alphabet used by the full boundary
    // inputs. Project admission separately parses and canonical-checks each
    // complete 65 KiB fixture; this oracle is not a substitute for that check.
    for (repetitions, padding) in [(0, ""), (1, "a"), (2, "aaa")] {
        let source = app_source(&format!("{}{padding}", LITERAL_UNIT.repeat(repetitions)));
        let parsed = semaprax::parse(&source, "app.spx").unwrap();
        assert_eq!(semaprax::format::canonical(&parsed), source);
    }
    let parsed = semaprax::parse(TEST_SOURCE, "tests.spx").unwrap();
    assert_eq!(semaprax::format::canonical(&parsed), TEST_SOURCE);
}

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
    // Author the canonical fixture directly. Running the unbounded formatter
    // here replayed its proof-only scalar renderer tens of thousands of times
    // before any capacity evidence began. Ordinary Project admission still
    // independently checks the full source and all unchanged boundary cases.
    // Source escapes use SPX syntax, not JSON's control escape grammar.
    let literal = format!("{}{}", LITERAL_UNIT.repeat(5_041), padding);
    let app = app_source(&literal);
    fs::create_dir(root.join("src")).unwrap();
    let manifest = root.join("semaprax.toml");
    write_new(&manifest, b"schema = \"semaprax.project.v10\"\nname = \"owned-utf8-capacity\"\nversion = \"0.1.0\"\nprofile = \"owned-utf8-api.v1\"\nentry = \"utf8.capacity\"\nsources = [\"src/app.spx\", \"src/tests.spx\"]\nweb_exports = [\"utf8.maximum\"]\ntests = [\"capacity.tests\"]\n");
    for (name, source) in [("app.spx", app.as_str()), ("tests.spx", TEST_SOURCE)] {
        let path = root.join("src").join(name);
        write_new(&path, source.as_bytes());
    }
    manifest
}
