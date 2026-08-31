use super::*;

pub(super) fn prepare_web(root: &Path) {
    for (name, bytes) in [
        (
            "consumer.mjs",
            include_bytes!("../../../examples/frame-payload-web/consumer.mjs").as_slice(),
        ),
        (
            "corpus-runner.mjs",
            include_bytes!("../../../examples/frame-payload-web/corpus-runner.mjs").as_slice(),
        ),
        (
            "package.json",
            include_bytes!("../../../examples/frame-payload-web/package.json").as_slice(),
        ),
        ("corpus.json", CORPUS),
    ] {
        files::write(root, name, bytes);
    }
}

pub(super) fn node(runner: &mut Runner, node: &Path, root: &Path) {
    for corpus in [CORPUS, SUPPLEMENT] {
        // Only invocation-local input data changes; no generated artifact or
        // unchanged consumer source is patched for either corpus.
        fs::write(root.join("corpus.json"), corpus).unwrap();
        let mut command = Command::new(node);
        command.arg("consumer.mjs").current_dir(root);
        let result = runner.run(&mut command, Duration::from_secs(60));
        assert_eq!(result.stdout, b"frame-payload-web-v1-ok\n");
        assert!(result.stderr.is_empty());
        assert_eq!(fs::read(root.join("corpus.json")).unwrap(), corpus);
    }
    fs::write(root.join("corpus.json"), CORPUS).unwrap();
    files::assert_names(
        root,
        &[
            "consumer.mjs",
            "corpus-runner.mjs",
            "package.json",
            "corpus.json",
            "generated",
        ],
    );
    for (name, bytes) in [
        (
            "consumer.mjs",
            include_bytes!("../../../examples/frame-payload-web/consumer.mjs").as_slice(),
        ),
        (
            "corpus-runner.mjs",
            include_bytes!("../../../examples/frame-payload-web/corpus-runner.mjs").as_slice(),
        ),
        (
            "package.json",
            include_bytes!("../../../examples/frame-payload-web/package.json").as_slice(),
        ),
        ("corpus.json", CORPUS),
    ] {
        assert_eq!(fs::read(root.join(name)).unwrap(), bytes);
    }
}

pub(super) fn rust(runner: &mut Runner, cargo: &Path, root: &Path, label: &str, target: &Path) {
    let original_manifest = include_str!("../../../examples/frame-payload-rust/Cargo.toml");
    assert_eq!(
        original_manifest
            .matches("../frame-payload-generated-sdk")
            .count(),
        1
    );
    let manifest = original_manifest.replace(
        "../frame-payload-generated-sdk",
        &format!("../{label}-generated-sdk"),
    );
    let lock = include_bytes!("../../../examples/frame-payload-rust/Cargo.lock");
    let source = include_bytes!("../../../examples/frame-payload-rust/src/main.rs");
    for (suffix, corpus) in [("rust", CORPUS), ("rust-adversarial", SUPPLEMENT)] {
        // Distinct include_str! paths prevent timestamp-only in-place corpus
        // replacement from reusing the preceding consumer's compiled data.
        let consumer = root.join(format!("{label}-{suffix}"));
        fs::create_dir(&consumer).unwrap();
        fs::create_dir(consumer.join("src")).unwrap();
        for (name, bytes) in [
            ("Cargo.toml", manifest.as_bytes()),
            ("Cargo.lock", lock.as_slice()),
            ("src/main.rs", source.as_slice()),
            ("corpus.json", corpus),
        ] {
            files::write(&consumer, name, bytes);
        }
        let mut command = native_rust_cargo::cargo_command();
        assert_eq!(Path::new(command.get_program()), cargo);
        command
            .args(["run", "--quiet", "--locked", "--offline", "--manifest-path"])
            .arg(consumer.join("Cargo.toml"))
            .current_dir(&consumer)
            .env("CARGO_TARGET_DIR", target)
            .env("CARGO_NET_OFFLINE", "true");
        let result = runner.run(&mut command, Duration::from_secs(300));
        assert_eq!(result.stdout, b"frame-payload-rust-v1-ok\n");
        files::assert_names(
            &consumer,
            &["Cargo.toml", "Cargo.lock", "corpus.json", "src"],
        );
        files::assert_names(&consumer.join("src"), &["main.rs"]);
        for (name, bytes) in [
            ("Cargo.toml", manifest.as_bytes()),
            ("Cargo.lock", lock.as_slice()),
            ("src/main.rs", source.as_slice()),
            ("corpus.json", corpus),
        ] {
            assert_eq!(fs::read(consumer.join(name)).unwrap(), bytes);
        }
    }
}
