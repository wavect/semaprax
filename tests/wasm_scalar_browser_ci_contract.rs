use std::fs;
use std::path::Path;

fn read(root: &Path, relative: &str) -> String {
    fs::read_to_string(root.join(relative)).unwrap_or_else(|error| {
        panic!("cannot read {relative}: {error}");
    })
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
        "npm ci --ignore-scripts",
        "npx --no-install playwright install --with-deps chromium",
        "npx --no-install tsc --strict --noEmit --target ES2022",
        "\"$SEMAPRAX_DIRECT_CALCULATOR_ROOT/consumer.ts\"",
        "\"$SEMAPRAX_PROJECT_CALCULATOR_ROOT/consumer.ts\"",
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
