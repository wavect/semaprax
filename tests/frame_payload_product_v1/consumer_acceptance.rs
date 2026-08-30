//! Explicit provisioned-tool acceptance; never downloads or silently skips.
use super::*;

pub(super) fn write_web_support(root: &Path) {
    for (path, bytes) in [
        ("corpus.json", CORPUS),
        (
            "consumer.mjs",
            include_bytes!("../../examples/frame-payload-web/consumer.mjs").as_slice(),
        ),
        (
            "corpus-runner.mjs",
            include_bytes!("../../examples/frame-payload-web/corpus-runner.mjs").as_slice(),
        ),
        (
            "consumer.ts",
            include_bytes!("../../examples/frame-payload-web/consumer.ts").as_slice(),
        ),
        (
            "package.json",
            include_bytes!("../../examples/frame-payload-web/package.json").as_slice(),
        ),
        (
            "browser.mjs",
            include_bytes!("../../examples/frame-payload-web/browser.mjs").as_slice(),
        ),
        (
            "index.html",
            include_bytes!("../../examples/frame-payload-web/index.html").as_slice(),
        ),
    ] {
        fs::write(root.join(path), bytes).unwrap();
    }
}

#[test]
#[ignore = "requires provisioned TypeScript 5.8.3 via TSC; no downloads"]
fn strict_typescript_accepts_both_display_names_and_rejects_wrong_types() {
    let tsc = std::env::var_os("TSC").expect("TSC must name provisioned TypeScript 5.8.3");
    let version = Command::new(&tsc)
        .arg("--version")
        .output()
        .expect("cannot invoke TSC");
    assert!(version.status.success());
    assert_eq!(
        String::from_utf8_lossy(&version.stdout).trim(),
        "Version 5.8.3"
    );
    let root = temporary("strict-frame-typescript");
    for renamed in [false, true] {
        let project = root.join(if renamed {
            "renamed-project"
        } else {
            "baseline-project"
        });
        let consumer = root.join(if renamed {
            "renamed-web"
        } else {
            "baseline-web"
        });
        copy_project(&project, renamed);
        fs::create_dir(&consumer).unwrap();
        build(
            Path::new(env!("CARGO_BIN_EXE_semaprax")),
            &project.join("semaprax.toml"),
            "npm",
            &consumer.join("generated"),
        );
        write_web_support(&consumer);
        let compile = |file: &str| {
            Command::new(&tsc)
                .args([
                    "--strict",
                    "--noEmit",
                    "--pretty",
                    "false",
                    "--target",
                    "ES2022",
                    "--module",
                    "NodeNext",
                    "--moduleResolution",
                    "NodeNext",
                    file,
                ])
                .current_dir(&consumer)
                .output()
                .unwrap()
        };
        let positive = compile("consumer.ts");
        assert!(
            positive.status.success(),
            "{}{}",
            String::from_utf8_lossy(&positive.stdout),
            String::from_utf8_lossy(&positive.stderr)
        );
        for (name, expression, code) in [
            ("wrong-argument.ts", "runtime.functions[\"frame.payload\"](42);", "TS2345"),
            ("unchecked-result.ts", "const value: bigint = runtime.functions[\"frame.payload-result\"](new Uint8Array()).error;", "TS2339"),
        ] {
            fs::write(consumer.join(name), format!("import {{ instantiate }} from './generated/semaprax.bindings.js';\nconst runtime = await instantiate(new Uint8Array());\n{expression}\n")).unwrap();
            let negative = compile(name);
            assert!(!negative.status.success());
            let diagnostics = String::from_utf8(negative.stdout).unwrap();
            assert!(diagnostics.contains(code), "{diagnostics}");
            assert!(diagnostics.contains(name), "{diagnostics}");
        }
    }
    // Failed assertions retain the fixture for inspection; successful runs
    // remove only this test's freshly created, exclusive temporary tree.
    fs::remove_dir_all(root).unwrap();
}
