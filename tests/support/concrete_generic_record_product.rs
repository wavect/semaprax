//! Shared GEN-05 source facts. Public Project admission remains deliberately absent.

use std::fs::{self, OpenOptions};
use std::io::Write as _;
use std::path::{Path, PathBuf};

pub const MANIFEST: &str = r#"schema = "semaprax.project.v8"
name = "generic-owned-product"
version = "1.0.0"
profile = "owned-data-api.v1"
entry = "generic.product.app"
sources = ["src/app.spx", "src/tests.spx", "src/types.spx"]
web_exports = ["generic.product.evaluate"]
tests = ["generic.product.tests"]
"#;

pub const TEMPLATE_MODULE: &str = r#"module generic.product.types;

@id("generic.product.pair")
record Pair<T, U> {
    @id("generic.product.pair.left")
    left: T,
    @id("generic.product.pair.right")
    right: U,
}

@id("generic.product.types.marker")
fn type_marker() -> i64 { 0 }
"#;

pub const ENTRY_MODULE: &str = r#"module generic.product.app;
use type @id("generic.product.pair") from generic.product.types as Pair;

@id("generic.product.make")
fn make(input: borrow Slice<u8>) -> Pair<Bytes, bool> {
    Pair<Bytes, bool> {
        left: bytes_copy(input),
        right: byte_len(input) > 0usize,
    }
}

@id("generic.product.consume")
fn consume(value: own Pair<Bytes, bool>) -> i64 {
    match own value {
        Pair { left: payload, right: present } =>
            if present && byte_len(bytes_as_slice(payload)) > 0usize { 1 } else { 0 },
    }
}

@id("generic.product.evaluate")
fn evaluate(input: borrow Slice<u8>) -> i64 { consume(make(input)) }

@id("generic.product.app.main")
fn main() -> i64 {
    let input = [1u8, 2u8, 3u8];
    evaluate(array_as_slice(input))
}
"#;

pub const TEST_MODULE: &str = r#"module generic.product.tests;
use function @id("generic.product.evaluate") from generic.product.app as evaluate;

@id("generic.product.tests.main")
fn main() -> i64 {
    let input = [1u8];
    if evaluate(array_as_slice(input)) == 1 { 0 } else { 1 }
}
"#;

pub fn write_project(root: &Path) -> PathBuf {
    assert!(root.is_absolute());
    assert_eq!(fs::read_dir(root).unwrap().count(), 0);
    fs::create_dir(root.join("src")).unwrap();
    let manifest = root.join("semaprax.toml");
    for (path, bytes) in [
        (manifest.clone(), MANIFEST.to_owned()),
        (root.join("src/app.spx"), canonical(ENTRY_MODULE)),
        (root.join("src/tests.spx"), canonical(TEST_MODULE)),
        (root.join("src/types.spx"), canonical(TEMPLATE_MODULE)),
    ] {
        OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(path)
            .unwrap()
            .write_all(bytes.as_bytes())
            .unwrap();
    }
    manifest
}

/// Standalone equivalent used to prove that legacy public descriptors reject
/// an otherwise compiler-admitted concrete generic result.
pub fn standalone_source() -> String {
    format!(
        r#"module generic.product.standalone;

{}
@id("generic.product.make")
fn make(input: borrow Slice<u8>) -> Pair<Bytes, bool> {{
    Pair<Bytes, bool> {{
        left: bytes_copy(input),
        right: byte_len(input) > 0usize,
    }}
}}

@id("generic.product.app.main")
fn main() -> i64 {{ 0 }}
"#,
        TEMPLATE_MODULE
            .strip_prefix("module generic.product.types;\n")
            .expect("shared template module prefix")
    )
}

pub fn resolved_standalone() -> semaprax::hir::ResolvedProgram {
    let source = standalone_source();
    let parsed = semaprax::check(&source, Path::new("generic-owned-product.spx"))
        .expect("shared concrete generic product checks");
    semaprax::hir::resolve(&parsed).expect("shared concrete generic product resolves")
}

fn canonical(source: &str) -> String {
    let parsed = semaprax::parse(source, Path::new("generic-owned-product.spx")).unwrap();
    let canonical = semaprax::format::canonical(&parsed);
    assert_eq!(
        semaprax::format::canonical(
            &semaprax::parse(&canonical, Path::new("generic-owned-product.spx")).unwrap()
        ),
        canonical
    );
    canonical
}
