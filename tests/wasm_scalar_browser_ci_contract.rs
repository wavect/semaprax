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
        "SEMAPRAX_CALCULATOR_ROOT=$fixture",
        "npm ci --ignore-scripts",
        "npx --no-install playwright install --with-deps chromium",
        "npx --no-install tsc --strict --noEmit --target ES2022",
        "\"$SEMAPRAX_CALCULATOR_ROOT/consumer.ts\"",
        "npm test -- --workers=1 --retries=0",
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
        "baseURL: \"http://127.0.0.1:4173\"",
        "reuseExistingServer: false",
    ] {
        assert!(
            config.contains(required),
            "browser fixture lost `{required}`"
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
