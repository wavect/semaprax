use std::fs;
use std::path::Path;

fn read(root: &Path, relative: &str) -> String {
    fs::read_to_string(root.join(relative)).unwrap_or_else(|error| {
        panic!("cannot read {relative}: {error}");
    })
}

#[test]
fn browser_known_answers_match_authenticated_baseline_and_rename_graphs() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let runner = read(
        root,
        "platform-tests/wasm-scalar-browser-v1/test-fixtures.mjs",
    );
    let fields = [
        "project_revision",
        "workspace_revision",
        "project_graph_digest",
    ];
    let expected = fields.map(|field| {
        let prefix = format!("{field}: \"");
        let values: Vec<_> = runner
            .lines()
            .filter_map(|line| line.trim().strip_prefix(&prefix)?.strip_suffix("\","))
            .collect();
        assert_eq!(
            values.len(),
            2,
            "exact baseline and renamed {field} answers"
        );
        values
    });
    struct Fixture(std::path::PathBuf);
    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }
    let fixture = Fixture(std::env::temp_dir().join(format!(
        "spx-browser-graph-known-answers-{}",
        std::process::id()
    )));
    fs::create_dir(&fixture.0).unwrap();
    fs::create_dir(fixture.0.join("src")).unwrap();
    let fixture_root = fixture.0.canonicalize().unwrap();
    let source_root = root.join("examples/calculator-project");
    for path in [
        "semaprax.toml",
        "src/app.spx",
        "src/core.spx",
        "src/tests.spx",
    ] {
        fs::copy(source_root.join(path), fixture_root.join(path)).unwrap();
    }
    for (index, name) in ["baseline", "renamed"].into_iter().enumerate() {
        if index == 1 {
            let path = fixture_root.join("src/core.spx");
            let original = fs::read_to_string(&path).unwrap();
            let renamed = original.replace("\nfn add(", "\nfn sum(");
            assert_ne!(original, renamed);
            fs::write(path, renamed).unwrap();
        }
        semaprax::project::with_authenticated_project(
            &fixture_root.join("semaprax.toml"),
            |snapshot| {
                let revision = snapshot.retain_revision();
                let actual = [
                    revision.project_revision(),
                    revision.workspace_revision(),
                    revision.semantic_graph_digest(),
                ];
                for (field, value) in actual.into_iter().enumerate() {
                    assert_eq!(
                        value, expected[field][index],
                        "{} fixture {name}",
                        fields[field]
                    );
                }
                Ok(())
            },
        )
        .unwrap();
    }
}

#[test]
fn chromium_scalar_calculator_gate_is_isolated_locked_and_serial() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let workflow = read(root, ".github/workflows/ci.yml");
    for required in [
        "wasm-scalar-exports-browser-v1:",
        "name: Public Wasm Scalar Exports v1 Chromium",
        "runs-on: ubuntu-24.04",
        "timeout-minutes: 20",
        "node-version: 22",
        "SEMAPRAX_DIRECT_CALCULATOR_ROOT=$direct",
        "SEMAPRAX_PROJECT_CALCULATOR_ROOT=$project",
        "SEMAPRAX_RENAMED_PROJECT_CALCULATOR_ROOT=$renamed",
        "sed -i 's/^fn add(/fn sum(/'",
        "npm ci --ignore-scripts",
        "npx --no-install playwright install --with-deps chromium",
        "npx --no-install tsc --strict --noEmit --target ES2022",
        "\"$SEMAPRAX_DIRECT_CALCULATOR_ROOT/consumer.ts\"",
        "\"$SEMAPRAX_PROJECT_CALCULATOR_ROOT/consumer.ts\"",
        "\"$SEMAPRAX_RENAMED_PROJECT_CALCULATOR_ROOT/consumer.ts\"",
        "npm run test:fixtures --",
    ] {
        assert!(
            workflow.contains(required),
            "browser CI gate lost `{required}`"
        );
    }

    let fixture = "platform-tests/wasm-scalar-browser-v1";
    let package: serde_json::Value =
        serde_json::from_str(&read(root, &format!("{fixture}/package.json"))).unwrap();
    assert_eq!(package["devDependencies"]["@playwright/test"], "1.62.0");
    assert_eq!(package["devDependencies"]["typescript"], "5.8.3");
    let lock: serde_json::Value =
        serde_json::from_str(&read(root, &format!("{fixture}/package-lock.json"))).unwrap();
    assert_eq!(lock["lockfileVersion"], 3);
    assert_eq!(
        lock["packages"]["node_modules/@playwright/test"]["version"],
        "1.62.0"
    );
    assert_eq!(
        lock["packages"]["node_modules/playwright"]["version"],
        "1.62.0"
    );
    assert_eq!(
        lock["packages"]["node_modules/playwright-core"]["version"],
        "1.62.0"
    );
    assert_eq!(
        lock["packages"]["node_modules/typescript"]["version"],
        "5.8.3"
    );

    let config = read(root, &format!("{fixture}/playwright.config.mjs"));
    for required in [
        "browserName: \"chromium\"",
        "workers: 1",
        "retries: 0",
        "SEMAPRAX_CALCULATOR_FIXTURES",
        "4173 + index",
        "projects,",
        "webServer,",
        "reuseExistingServer: false",
    ] {
        assert!(
            config.contains(required),
            "browser fixture lost `{required}`"
        );
    }

    let fixture_runner = read(root, &format!("{fixture}/test-fixtures.mjs"));
    for required in [
        "semaprax.web.v4",
        "semaprax.web-project.v1",
        "roots.length !== 3",
        "\"direct-source\"",
        "\"project-baseline\"",
        "\"project-renamed\"",
        "sha256:8576caa566cb7f0d265354927c5bc7b481146f05e616f76917f340b4af26f053",
        "sha256:afa7b35b6b057eaa1cbf89c68ccd1e19a8d988f4168049f70717f80c28218fb7",
        "SEMAPRAX_CALCULATOR_FIXTURES",
        "\"--workers=1\"",
        "\"--retries=0\"",
    ] {
        assert!(
            fixture_runner.contains(required),
            "three-fixture browser runner lost `{required}`"
        );
    }

    let test = read(root, &format!("{fixture}/tests/calculator.spec.mjs"));
    for required in [
        "calculator.add",
        "calculator.subtract",
        "calculator.multiply",
        "calculator.divide",
        "semaprax.arithmetic.v1/1",
        "semaprax.contract.v1/1",
        "wasmResponses.length).toBe(1)",
        "requestOrigins",
        "requestFailures",
    ] {
        assert!(
            test.contains(required),
            "browser interaction proof lost `{required}`"
        );
    }
}
