//! Additional input corpus for the unchanged external consumer source and SDK.
use super::*;

pub(super) fn run_node(consumer: &Path) {
    let corpus = consumer.join("corpus.json");
    assert_eq!(fs::read(&corpus).unwrap(), CORPUS);
    // The same existing entry point reads this invocation-local input. Leave
    // the supplement in place on failure for diagnosis; restore the original
    // bytes only after the complete consumer returns its original success line.
    fs::write(&corpus, adversarial::CORPUS).unwrap();
    run_node_consumer(consumer);
    assert_eq!(fs::read(&corpus).unwrap(), adversarial::CORPUS);
    fs::write(&corpus, CORPUS).unwrap();
}

pub(super) fn run_rust(consumer: &Path, manifest: &str, lock: &[u8], target: &Path) {
    // A fresh sibling consumer uses the same published dependency and exact
    // source. Its distinct include_str! input path avoids relying on Cargo
    // noticing an in-place corpus rewrite by timestamp alone. Dependency
    // compilation is still shared through the existing explicit target dir.
    fs::create_dir(consumer).unwrap();
    fs::create_dir(consumer.join("src")).unwrap();
    let source = include_bytes!("../../examples/frame-payload-rust/src/main.rs");
    for (path, bytes) in [
        ("Cargo.toml", manifest.as_bytes()),
        ("Cargo.lock", lock),
        ("corpus.json", adversarial::CORPUS),
        ("src/main.rs", source.as_slice()),
    ] {
        use std::io::Write as _;
        fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(consumer.join(path))
            .unwrap()
            .write_all(bytes)
            .unwrap();
    }
    let result = native_rust_cargo::cargo_command()
        .args(["run", "--quiet", "--locked", "--offline", "--manifest-path"])
        .arg(consumer.join("Cargo.toml"))
        .current_dir(consumer)
        .env("CARGO_TARGET_DIR", target)
        .output()
        .unwrap();
    assert!(
        result.status.success(),
        "supplemental Rust stdout={} stderr={}",
        String::from_utf8_lossy(&result.stdout),
        String::from_utf8_lossy(&result.stderr)
    );
    assert_eq!(
        String::from_utf8_lossy(&result.stdout).trim(),
        "frame-payload-rust-v1-ok"
    );
    assert_eq!(fs::read(consumer.join("Cargo.lock")).unwrap(), lock);
    assert_eq!(
        fs::read(consumer.join("Cargo.toml")).unwrap(),
        manifest.as_bytes()
    );
    assert_eq!(
        fs::read(consumer.join("src/main.rs")).unwrap(),
        source.as_slice()
    );
    assert_eq!(
        fs::read(consumer.join("corpus.json")).unwrap(),
        adversarial::CORPUS
    );
}
