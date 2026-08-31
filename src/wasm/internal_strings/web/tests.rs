use super::*;
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::io::Read;
use std::sync::atomic::{AtomicU64, Ordering};

mod descriptor_bounds;
mod package_bounds;

const SOURCE: &str =
    "module web.bounds;\n@id(\"main\")\nfn main() -> i64 { string_len(\"hello\") }\n";
static NEXT: AtomicU64 = AtomicU64::new(0);
const INVENTORY: [&str; 8] = [
    "app.wasm",
    "semaprax.js",
    "semaprax.d.ts",
    "semaprax.internal-strings.json",
    "semaprax.manifest.json",
    "package.json",
    "index.html",
    "app.js",
];

fn plain(path: &Path, directory: bool) -> std::fs::Metadata {
    let metadata = std::fs::symlink_metadata(path).unwrap();
    assert!(!metadata.file_type().is_symlink());
    assert_eq!(metadata.is_dir(), directory);
    assert_eq!(metadata.is_file(), !directory);
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt as _;
        assert_eq!(metadata.file_attributes() & 0x400, 0);
    }
    metadata
}

fn exact_entries(path: &Path, expected: &[&str]) {
    plain(path, true);
    let mut actual = std::fs::read_dir(path)
        .unwrap()
        .map(|entry| entry.unwrap().file_name().into_string().unwrap())
        .collect::<Vec<_>>();
    actual.sort();
    let mut expected = expected.to_vec();
    expected.sort_unstable();
    assert_eq!(actual, expected);
}

fn reopened_package(path: &Path) -> BTreeMap<&'static str, Vec<u8>> {
    exact_entries(path, &INVENTORY);
    let mut remaining = PACKAGE_LIMIT;
    INVENTORY
        .into_iter()
        .map(|name| {
            let file = path.join(name);
            let length = usize::try_from(plain(&file, false).len()).unwrap();
            assert!(length <= remaining);
            let mut bytes = Vec::new();
            std::fs::File::open(file)
                .unwrap()
                .take(length as u64 + 1)
                .read_to_end(&mut bytes)
                .unwrap();
            assert_eq!(bytes.len(), length);
            remaining -= length;
            (name, bytes)
        })
        .collect()
}

fn directory() -> std::path::PathBuf {
    let path = std::env::temp_dir().join(format!(
        "semaprax-string-web-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    std::fs::create_dir(&path).unwrap();
    path
}

#[test]
fn descriptor_and_complete_package_bounds_are_exact_and_checked() {
    assert!(bounded(DESCRIPTOR_LIMIT, DESCRIPTOR_LIMIT, "descriptor").is_ok());
    assert_eq!(
        bounded(DESCRIPTOR_LIMIT + 1, DESCRIPTOR_LIMIT, "descriptor")
            .unwrap_err()
            .code,
        "SPX-W111"
    );
    assert!(package_size([PACKAGE_LIMIT - 1, 1]).is_ok());
    assert_eq!(
        package_size([PACKAGE_LIMIT, 1]).unwrap_err().code,
        "SPX-W111"
    );
    assert_eq!(package_size([usize::MAX, 1]).unwrap_err().code, "SPX-W111");
}

#[test]
fn exact_source_bound_is_read_and_plus_one_fails_before_output() {
    let root = directory();
    let source = root.join("input.spx");
    let mut text = SOURCE.to_owned();
    text.push_str("//");
    text.extend(std::iter::repeat_n('x', SOURCE_LIMIT - text.len()));
    std::fs::write(&source, &text).unwrap();
    let snapshot =
        crate::patch::read_source_snapshot_bounded(&source, SOURCE_LIMIT, "SPX-W111").unwrap();
    assert_eq!(snapshot.source().len(), SOURCE_LIMIT);
    assert_eq!(snapshot.source(), text);
    let output = root.join("output");
    build_web_from_source(&source, &output, &["main".to_owned()]).unwrap();
    let files = reopened_package(&output);
    let program = crate::check(&text, "input.spx").unwrap();
    let module = emit_module(
        &program,
        &["main".to_owned()],
        InternalStringOptions::default(),
    )
    .unwrap();
    assert_eq!(files["app.wasm"], module.wasm_bytes());
    assert_eq!(files["semaprax.js"], module.runtime_source().as_bytes());
    assert_eq!(
        files["semaprax.internal-strings.json"],
        module.descriptor().as_bytes()
    );
    let manifest: serde_json::Value =
        serde_json::from_slice(&files["semaprax.manifest.json"]).unwrap();
    let source_digest = format!(
        "sha256:{:x}",
        crate::digest_hex::LowerHex(Sha256::digest(text.as_bytes()))
    );
    assert_eq!(
        manifest["source_digest"].as_str(),
        Some(source_digest.as_str())
    );
    plain(&source, false);
    assert_eq!(std::fs::read(&source).unwrap(), text.as_bytes());

    text.push('x');
    assert_eq!(text.len(), SOURCE_LIMIT + 1);
    std::fs::write(&source, &text).unwrap();
    let excess = root.join("excess");
    let errors = build_web_from_source(&source, &excess, &["main".to_owned()]).unwrap_err();
    assert_eq!(errors[0].code, "SPX-W111");
    assert_eq!(
        std::fs::symlink_metadata(&excess).unwrap_err().kind(),
        std::io::ErrorKind::NotFound
    );
    // Validate the complete owned tree before explicit, nonrecursive cleanup.
    // A failed oracle leaves all evidence in place.
    exact_entries(&root, &["input.spx", "output"]);
    assert_eq!(reopened_package(&output), files);
    plain(&source, false);
    assert_eq!(std::fs::read(&source).unwrap(), text.as_bytes());
    for name in INVENTORY {
        std::fs::remove_file(output.join(name)).unwrap();
    }
    std::fs::remove_dir(output).unwrap();
    std::fs::remove_file(source).unwrap();
    std::fs::remove_dir(root).unwrap();
}

#[test]
fn source_drift_and_growth_are_rejected_before_destination_creation() {
    for grow in [false, true] {
        let root = directory();
        let source = root.join("input.spx");
        let output = root.join("output");
        std::fs::write(&source, SOURCE).unwrap();
        let errors = build(&source, &output, &["main".to_owned()], || {
            if grow {
                std::fs::write(&source, vec![b' '; SOURCE_LIMIT + 1]).unwrap();
            } else {
                std::fs::write(&source, SOURCE.replace("hello", "world")).unwrap();
            }
        })
        .unwrap_err();
        assert_eq!(errors[0].code, "SPX-I207");
        assert!(!output.exists());
        std::fs::remove_file(source).unwrap();
        std::fs::remove_dir(root).unwrap();
    }
}

#[test]
fn empty_identity_rejects_and_leading_dash_identity_remains_exact() {
    let root = directory();
    let source = root.join("input.spx");
    let output = root.join("output");
    std::fs::write(&source, SOURCE.replace("@id(\"main\")", "@id(\"\")")).unwrap();
    let errors = build_web_from_source(&source, &output, &[String::new()]).unwrap_err();
    assert_eq!(errors[0].code, "SPX-H006");
    assert!(!output.exists());
    std::fs::write(&source, SOURCE.replace("@id(\"main\")", "@id(\"-main\")")).unwrap();
    build_web_from_source(&source, &output, &["-main".to_owned()]).unwrap();
    let declarations = std::fs::read_to_string(output.join("semaprax.d.ts")).unwrap();
    assert!(declarations.contains("call(id: \"-main\")"));
    let descriptor =
        std::fs::read_to_string(output.join("semaprax.internal-strings.json")).unwrap();
    assert!(descriptor.contains("\"stable_id\":\"-main\""));
    for path in [
        "app.wasm",
        "semaprax.js",
        "semaprax.d.ts",
        "semaprax.internal-strings.json",
        "semaprax.manifest.json",
        "package.json",
        "index.html",
        "app.js",
    ] {
        std::fs::remove_file(output.join(path)).unwrap();
    }
    std::fs::remove_dir(output).unwrap();
    std::fs::remove_file(source).unwrap();
    std::fs::remove_dir(root).unwrap();
}
