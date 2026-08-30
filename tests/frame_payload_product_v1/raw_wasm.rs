//! Independent raw-ABI consumer; the arena itself is the production runtime.
use super::*;
use semaprax::project::PublicApiDescriptor;
use sha2::{Digest, Sha256};

pub(super) fn run(package: &Path, descriptor: &PublicApiDescriptor) {
    let root = temporary("raw-wasm");
    fs::create_dir(&root).unwrap();
    let wasm = fs::read(package.join("app.wasm")).unwrap();
    let digest = Sha256::digest(&wasm)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>();
    let descriptor_bytes = descriptor.canonical_bytes();
    // Use exactly the same input/arena/core source as the selected v8 renderer.
    // Only fixture bindings and a test-module export are added; no production
    // text is rewritten and no separately maintained allocator is introduced.
    // V8 fixes its arena capacity at sixteen; this corpus needs at most one.
    let runtime = format!(
        "const EXPECTED_WASM_SHA256={};\nconst __SPX_CAPACITY__=16;\n{}\n{}\n{}\nexport {{instantiateCore}};\n",
        serde_json::to_string(&digest).unwrap(),
        include_str!("../../src/project/npm/owned_data_input_v8.js"),
        include_str!("../../src/project/npm/owned_invocation/arena.js"),
        include_str!("../../src/project/npm/owned_invocation/core.js"),
    );
    for (path, bytes) in [
        ("runtime.mjs", runtime.as_bytes()),
        ("app.wasm", wasm.as_slice()),
        ("descriptor.json", descriptor_bytes.as_slice()),
        ("corpus.json", CORPUS),
        ("adversarial.json", super::adversarial::CORPUS),
        ("probe.mjs", include_bytes!("raw_wasm.mjs").as_slice()),
    ] {
        fs::write(root.join(path), bytes).unwrap();
    }
    let result = Command::new("node")
        .arg("probe.mjs")
        .current_dir(&root)
        .output()
        .unwrap();
    assert!(
        result.status.success(),
        "raw-Wasm stdout={} stderr={}",
        String::from_utf8_lossy(&result.stdout),
        String::from_utf8_lossy(&result.stderr)
    );
    assert_eq!(result.stdout, b"frame-payload-raw-wasm-v1-ok\n");
    // Only this fixture's exact known regular files are removed on success.
    let names = [
        "runtime.mjs",
        "app.wasm",
        "descriptor.json",
        "corpus.json",
        "adversarial.json",
        "probe.mjs",
    ];
    let mut actual = fs::read_dir(&root)
        .unwrap()
        .map(|row| row.unwrap().file_name())
        .collect::<Vec<_>>();
    actual.sort();
    let mut expected = names.map(std::ffi::OsString::from);
    expected.sort();
    assert_eq!(actual, expected);
    for name in names {
        let metadata = fs::symlink_metadata(root.join(name)).unwrap();
        assert!(metadata.is_file() && !metadata.file_type().is_symlink());
    }
    for name in names {
        fs::remove_file(root.join(name)).unwrap();
    }
    fs::remove_dir(root).unwrap();
}
