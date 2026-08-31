//! Canonical source shared by independent mixed-arity host consumers.
//! Predicates distinguish argument positions, not evaluation order or full content.
use std::fs::{self, OpenOptions};
use std::io::Write as _;
use std::path::{Path, PathBuf};

pub fn selected(max_arity: usize) -> Vec<String> {
    assert!(max_arity <= 9);
    (0..=max_arity)
        .map(|arity| format!("mixed.arity{arity}"))
        .collect()
}

pub fn source(max_arity: usize) -> String {
    assert!(max_arity <= 9);
    let parameters = [
        "p0: i64",
        "p1: bool",
        "p2: borrow str",
        "p3: borrow Slice<u8>",
        "p4: i64",
        "p5: bool",
        "p6: borrow str",
        "p7: borrow Slice<u8>",
        "p8: i64",
    ];
    let predicates = [
        "p0 == (0 - 13)",
        "p1 == true",
        "byte_len(str_as_bytes(p2)) == 4usize",
        "byte_len(p3) == 3usize",
        "p4 == 29",
        "p5 == false",
        "byte_len(str_as_bytes(p6)) == 5usize",
        "byte_len(p7) == 6usize",
        "p8 == 41",
    ];
    let mut source = String::from("module mixed.app;\n");
    for arity in 0..=max_arity {
        let signature = parameters[..arity].join(", ");
        let condition = if arity == 0 {
            "true".to_owned()
        } else {
            predicates[..arity].join(" && ")
        };
        source.push_str(&format!(
            "@id(\"mixed.arity{arity}\") fn arity{arity}({signature}) -> Bytes {{\n\
             if {condition} {{ let output = [111u8, 107u8]; bytes_copy(array_as_slice(output)) }} \
             else {{ let output = [98u8, 97u8, 100u8]; bytes_copy(array_as_slice(output)) }}\n}}\n"
        ));
    }
    source.push_str("@id(\"mixed.main\") fn main() -> i64 { 0 }\n");
    canonical(&source)
}

pub fn write_project(root: &Path, max_arity: usize) -> PathBuf {
    assert!(root.is_absolute());
    assert_eq!(fs::read_dir(root).unwrap().count(), 0);
    let exports = selected(max_arity)
        .iter()
        .map(|id| format!("\"{id}\""))
        .collect::<Vec<_>>()
        .join(", ");
    let manifest_text = format!(
        "schema = \"semaprax.project.v8\"\nname = \"owned-mixed-arity\"\nversion = \"0.1.0\"\nprofile = \"owned-data-api.v1\"\nentry = \"mixed.app\"\nsources = [\"src/app.spx\", \"src/tests.spx\"]\nweb_exports = [{exports}]\ntests = [\"mixed.tests\"]\n"
    );
    let app = source(max_arity);
    let tests = canonical("module mixed.tests; @id(\"mixed.tests.main\") fn main() -> i64 { 0 }");
    fs::create_dir(root.join("src")).unwrap();
    let manifest = root.join("semaprax.toml");
    for (path, text) in [
        (manifest.clone(), manifest_text),
        (root.join("src/app.spx"), app),
        (root.join("src/tests.spx"), tests),
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
    let checked = semaprax::check(source, "mixed.spx").unwrap();
    let text = semaprax::format::canonical(&checked);
    assert_eq!(
        semaprax::format::canonical(&semaprax::parse(&text, "mixed.spx").unwrap()),
        text
    );
    text
}
