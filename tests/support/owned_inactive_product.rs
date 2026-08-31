//! Exact shared Project source for initialized-owner inactive-result consumers.
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};

pub fn write_project(root: &Path) -> PathBuf {
    assert!(root.is_absolute());
    assert_eq!(fs::read_dir(root).unwrap().count(), 0);
    fs::create_dir(root.join("src")).unwrap();
    for (name, source) in [
        (
            "app.spx",
            include_str!("../project_owned_inactive_cleanup_v1/source.spx"),
        ),
        (
            "tests.spx",
            "module inactive.tests; @id(\"inactive.tests.main\") fn main() -> i64 { 0 }",
        ),
    ] {
        let path = root.join("src").join(name);
        let ast = semaprax::check(source, &path).unwrap();
        let canonical = semaprax::format::canonical(&ast);
        let reparsed = semaprax::check(&canonical, &path).unwrap();
        assert_eq!(semaprax::format::canonical(&reparsed), canonical);
        assert_eq!(
            semaprax::graph::to_json(&ast).unwrap(),
            semaprax::graph::to_json(&reparsed).unwrap()
        );
        write_new(&path, canonical.as_bytes());
    }
    let manifest = b"schema = \"semaprax.project.v8\"\nname = \"inactive-cleanup\"\nversion = \"0.1.0\"\nprofile = \"owned-data-api.v1\"\nentry = \"inactive.app\"\nsources = [\"src/app.spx\", \"src/tests.spx\"]\nweb_exports = [\"inactive.maybe\", \"inactive.result\"]\ntests = [\"inactive.tests\"]\n";
    let path = root.join("semaprax.toml");
    write_new(&path, manifest);
    path
}

fn write_new(path: &Path, bytes: &[u8]) {
    OpenOptions::new()
        .create_new(true)
        .write(true)
        .open(path)
        .unwrap()
        .write_all(bytes)
        .unwrap();
}
