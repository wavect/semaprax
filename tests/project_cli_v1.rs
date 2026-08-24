use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::sync::atomic::{AtomicU64, Ordering};

use wasmparser::{ExternalKind, Parser, Payload};

const PROJECT_FILES: &[&str] = &[
    "semaprax.toml",
    "src/app.spx",
    "src/core.spx",
    "src/tests.spx",
];
const WEB_INVENTORY: &[&str] = &[
    "app.wasm",
    "index.html",
    "package.json",
    "semaprax.bindings.d.ts",
    "semaprax.bindings.js",
    "semaprax.js",
    "semaprax.scalar-exports.json",
];
const WEB_EXPORTS: &[&str] = &[
    "calculator.add",
    "calculator.divide",
    "calculator.is-negative",
    "calculator.not",
];

static SERIAL: AtomicU64 = AtomicU64::new(0);

struct Fixture {
    root: PathBuf,
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.root);
    }
}

fn fixture(label: &str) -> Fixture {
    let root = std::env::temp_dir().join(format!(
        "semaprax-project-cli-v1-{label}-{}-{}",
        std::process::id(),
        SERIAL.fetch_add(1, Ordering::Relaxed)
    ));
    std::fs::create_dir_all(root.join("src")).unwrap();
    let source = Path::new(env!("CARGO_MANIFEST_DIR")).join("examples/calculator-project");
    for file in PROJECT_FILES {
        std::fs::copy(source.join(file), root.join(file)).unwrap();
    }
    Fixture {
        root: root.canonicalize().unwrap(),
    }
}

fn cli(root: &Path, arguments: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_semaprax"))
        .args(arguments)
        .current_dir(root)
        .output()
        .unwrap()
}

fn stdout(output: &Output) -> String {
    String::from_utf8(output.stdout.clone()).unwrap()
}

fn stderr(output: &Output) -> String {
    String::from_utf8(output.stderr.clone()).unwrap()
}

fn package_files(directory: &Path) -> BTreeMap<String, Vec<u8>> {
    std::fs::read_dir(directory)
        .unwrap()
        .map(|entry| {
            let entry = entry.unwrap();
            assert!(entry.file_type().unwrap().is_file());
            (
                entry.file_name().into_string().unwrap(),
                std::fs::read(entry.path()).unwrap(),
            )
        })
        .collect()
}

fn raw_symbol(id: &str) -> String {
    let mut symbol = String::from("spx_scalar_");
    for byte in id.bytes() {
        symbol.push_str(&format!("{byte:02x}"));
    }
    symbol
}

fn wasm_function_exports(bytes: &[u8]) -> Vec<String> {
    Parser::new(0)
        .parse_all(bytes)
        .filter_map(|payload| match payload.unwrap() {
            Payload::ExportSection(section) => Some(
                section
                    .into_iter()
                    .filter_map(|export| {
                        let export = export.unwrap();
                        (export.kind == ExternalKind::Func).then(|| export.name.to_owned())
                    })
                    .collect::<Vec<_>>(),
            ),
            _ => None,
        })
        .flatten()
        .collect()
}

#[test]
fn implicit_and_explicit_project_checks_authenticate_the_same_manifest() {
    let fixture = fixture("check");
    let implicit = cli(&fixture.root, &["check"]);
    assert!(
        implicit.status.success(),
        "implicit check failed: {}",
        stderr(&implicit)
    );

    let explicit_path = fixture.root.join("semaprax.toml");
    let explicit = cli(
        &fixture.root,
        &["check", "--manifest-path", explicit_path.to_str().unwrap()],
    );
    assert!(
        explicit.status.success(),
        "explicit check failed: {}",
        stderr(&explicit)
    );
    assert_eq!(stdout(&implicit), stdout(&explicit));
    assert!(stdout(&implicit).starts_with("verified project calculator (sha256:"));

    let named = cli(&fixture.root, &["check", "semaprax.toml"]);
    assert!(
        named.status.success(),
        "named check failed: {}",
        stderr(&named)
    );
    assert_eq!(stdout(&implicit), stdout(&named));

    let json_default = cli(&fixture.root, &["check", "--json"]);
    assert!(
        json_default.status.success(),
        "JSON default check failed: {}",
        stderr(&json_default)
    );
    assert!(stdout(&json_default).is_empty());
    let json_explicit = cli(
        &fixture.root,
        &[
            "check",
            "--json",
            "--manifest-path",
            explicit_path.to_str().unwrap(),
        ],
    );
    assert!(
        json_explicit.status.success(),
        "JSON explicit check failed: {}",
        stderr(&json_explicit)
    );
    assert!(stdout(&json_explicit).is_empty());
}

#[test]
fn project_web_builds_are_manifest_owned_and_byte_exact() {
    let fixture = fixture("web");
    let implicit_output = fixture.root.join("implicit-web");
    let implicit = cli(
        &fixture.root,
        &["build", "-o", implicit_output.to_str().unwrap()],
    );
    assert!(
        implicit.status.success(),
        "implicit project web build failed: {}",
        stderr(&implicit)
    );
    assert_eq!(
        stdout(&implicit),
        format!("built project web package {}\n", implicit_output.display())
    );

    let explicit_output = fixture.root.join("explicit-web");
    let manifest = fixture.root.join("semaprax.toml");
    let explicit = cli(
        &fixture.root,
        &[
            "build",
            "--manifest-path",
            manifest.to_str().unwrap(),
            "--target",
            "web",
            "--output",
            explicit_output.to_str().unwrap(),
        ],
    );
    assert!(
        explicit.status.success(),
        "explicit project web build failed: {}",
        stderr(&explicit)
    );
    assert_eq!(
        stdout(&explicit),
        format!("built project web package {}\n", explicit_output.display())
    );

    let implicit_files = package_files(&implicit_output);
    let explicit_files = package_files(&explicit_output);
    assert_eq!(implicit_files, explicit_files);
    assert_eq!(
        implicit_files
            .keys()
            .map(String::as_str)
            .collect::<Vec<_>>(),
        WEB_INVENTORY
    );
    assert_eq!(
        wasm_function_exports(&implicit_files["app.wasm"]),
        WEB_EXPORTS
            .iter()
            .copied()
            .map(raw_symbol)
            .collect::<Vec<_>>()
    );
    let manifest: serde_json::Value =
        serde_json::from_slice(&implicit_files["semaprax.scalar-exports.json"]).unwrap();
    assert_eq!(manifest["schema"], "semaprax.web-project.v1");
    assert_eq!(manifest["project_schema"], "semaprax.project.v1");
    assert_eq!(manifest["project"], "calculator");
    assert!(manifest["project_graph_digest"]
        .as_str()
        .is_some_and(|digest| digest.starts_with("sha256:") && digest.len() == 71));
    assert_eq!(
        manifest["scalar_abi"]["functions"]
            .as_array()
            .unwrap()
            .iter()
            .map(|function| function["stable_id"].as_str().unwrap())
            .collect::<Vec<_>>(),
        WEB_EXPORTS
    );
}

#[test]
fn project_build_rejections_happen_before_any_output_clobber() {
    let fixture = fixture("rejections");
    let source = fixture.root.join("src/app.spx");
    let manifest = fixture.root.join("semaprax.toml");
    let blocked_output = fixture
        .root
        .join(format!("blocked-output{}", std::env::consts::EXE_SUFFIX));
    let sentinel = b"foreign output must survive";
    std::fs::write(&blocked_output, sentinel).unwrap();

    for (label, arguments, expected) in [
        (
            "source-and-manifest",
            vec![
                "build",
                source.to_str().unwrap(),
                "--manifest-path",
                manifest.to_str().unwrap(),
                "--target",
                "web",
                "-o",
                blocked_output.to_str().unwrap(),
            ],
            "build cannot combine an input file with --manifest-path",
        ),
        (
            "manifest-owned-export",
            vec![
                "build",
                "semaprax.toml",
                "--target",
                "web",
                "--export",
                "calculator.add",
                "-o",
                blocked_output.to_str().unwrap(),
            ],
            "Project v1 takes its entry and web exports only from the authenticated manifest",
        ),
        (
            "native-callable-target",
            vec![
                "build",
                "semaprax.toml",
                "--target",
                "native-callable",
                "--function",
                "calculator.add",
                "-o",
                blocked_output.to_str().unwrap(),
            ],
            "Project v1 publishes only explicit web and native targets; native-callable publication remains held",
        ),
    ] {
        let output = cli(&fixture.root, &arguments);
        assert_eq!(output.status.code(), Some(2), "{label}: {output:?}");
        assert!(stderr(&output).contains(expected), "{label}: {}", stderr(&output));
        assert_eq!(std::fs::read(&blocked_output).unwrap(), sentinel, "{label}");
    }

    let existing_native = cli(
        &fixture.root,
        &[
            "build",
            "semaprax.toml",
            "--target",
            "native",
            "-o",
            blocked_output.to_str().unwrap(),
        ],
    );
    assert_eq!(existing_native.status.code(), Some(1));
    assert!(
        stderr(&existing_native).contains("SPX-I307"),
        "{}",
        stderr(&existing_native)
    );
    assert!(stderr(&existing_native).contains("already exists"));
    assert_eq!(std::fs::read(&blocked_output).unwrap(), sentinel);

    let missing_build = cli(&fixture.root, &["build", "--manifest-path"]);
    assert_eq!(missing_build.status.code(), Some(2));
    assert!(stderr(&missing_build).contains("build option `--manifest-path` requires a value"));
    assert_eq!(std::fs::read(&blocked_output).unwrap(), sentinel);

    let missing_check = cli(&fixture.root, &["check", "--manifest-path"]);
    assert_eq!(missing_check.status.code(), Some(2));
    assert!(stderr(&missing_check).contains("check option `--manifest-path` requires a value"));
    assert_eq!(std::fs::read(&blocked_output).unwrap(), sentinel);

    let mixed_check = cli(
        &fixture.root,
        &["check", "src/app.spx", "--manifest-path", "semaprax.toml"],
    );
    assert_eq!(mixed_check.status.code(), Some(2));
    assert!(stderr(&mixed_check).contains("check cannot combine an input file"));
    assert_eq!(std::fs::read(&blocked_output).unwrap(), sentinel);

    let unknown_check = cli(&fixture.root, &["check", "src/app.spx", "--unknown"]);
    assert_eq!(unknown_check.status.code(), Some(2));
    assert!(stderr(&unknown_check).contains("unknown check option `--unknown`"));
    assert_eq!(std::fs::read(&blocked_output).unwrap(), sentinel);

    let existing = cli(
        &fixture.root,
        &[
            "build",
            "semaprax.toml",
            "--target",
            "web",
            "-o",
            blocked_output.to_str().unwrap(),
        ],
    );
    assert_eq!(existing.status.code(), Some(1));
    assert!(
        stderr(&existing).contains("SPX-I307"),
        "{}",
        stderr(&existing)
    );
    assert_eq!(std::fs::read(&blocked_output).unwrap(), sentinel);
}
