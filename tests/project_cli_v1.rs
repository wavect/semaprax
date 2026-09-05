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
    "calculator.multiply",
    "calculator.not",
    "calculator.subtract",
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
    let json_default_value: serde_json::Value =
        serde_json::from_str(stdout(&json_default).trim_end()).unwrap();
    assert_eq!(json_default_value["status"], "verified");
    assert_eq!(json_default_value["name"], "calculator");
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
    assert_eq!(stdout(&json_explicit), stdout(&json_default));
}

#[test]
fn directory_operands_select_the_project_manifest() {
    let fixture = fixture("directory");
    let manifest = cli(&fixture.root, &["check", "semaprax.toml"]);
    assert!(manifest.status.success(), "{}", stderr(&manifest));
    let root = fixture.root.to_str().unwrap();
    for operand in [".", "./", root] {
        let directory = cli(&fixture.root, &["check", operand]);
        assert!(
            directory.status.success(),
            "check {operand} failed: {}",
            stderr(&directory)
        );
        assert_eq!(stdout(&directory), stdout(&manifest), "check {operand}");
    }
    let run = cli(&fixture.root, &["run", "."]);
    assert!(run.status.success(), "{}", stderr(&run));
    assert_eq!(stdout(&run), "42\n");
    let test = cli(&fixture.root, &["test", root]);
    assert!(test.status.success(), "{}", stderr(&test));
    assert_eq!(stdout(&test), "project tests passed\n");
    // `fmt` resolves the same operands; the committed example is canonical.
    for operand in [".", "semaprax.toml", root] {
        let formatted = cli(&fixture.root, &["fmt", operand, "--check"]);
        assert!(
            formatted.status.success(),
            "fmt {operand}: {}",
            stderr(&formatted)
        );
        assert!(formatted.stdout.is_empty() && formatted.stderr.is_empty());
    }
    // `--manifest-path` stays exact: a directory there is not a manifest.
    let exact = cli(&fixture.root, &["check", "--manifest-path", "."]);
    assert_eq!(exact.status.code(), Some(1));
    assert!(stdout(&exact).is_empty());
    assert!(
        stderr(&exact).starts_with("error[SPX-J1"),
        "{}",
        stderr(&exact)
    );
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
            "wasm",
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

    let default_output = fixture.root.join("calculator-web");
    let default = cli(&fixture.root, &["build", "--json"]);
    assert!(
        default.status.success(),
        "default JSON project build failed: {}",
        stderr(&default)
    );
    assert!(stderr(&default).is_empty());
    let report: serde_json::Value = serde_json::from_str(stdout(&default).trim()).unwrap();
    assert_eq!(report["status"], "built");
    assert_eq!(report["target"], "web");
    assert_eq!(report["product"], "project web package");
    assert_eq!(report["output"], default_output.display().to_string());

    let implicit_files = package_files(&implicit_output);
    let explicit_files = package_files(&explicit_output);
    assert_eq!(implicit_files, explicit_files);
    assert_eq!(implicit_files, package_files(&default_output));
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
fn standalone_project_target_error_uses_the_project_catalog() {
    let fixture = fixture("target-catalog");
    let rejected = cli(&fixture.root, &["build", "--target", "bogus"]);
    assert_eq!(rejected.status.code(), Some(2));
    assert!(rejected.stdout.is_empty());
    assert_eq!(
        stderr(&rejected),
        "unsupported target `bogus`; available: native, web, wasm, npm\n\
hint: run `semaprax build --help` for usage\n"
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
            "unsupported target `native-callable`; available: native, web, wasm, npm",
        ),
    ] {
        let output = cli(&fixture.root, &arguments);
        assert_eq!(output.status.code(), Some(2), "{label}: {output:?}");
        assert!(stderr(&output).contains(expected), "{label}: {}", stderr(&output));
        assert_eq!(std::fs::read(&blocked_output).unwrap(), sentinel, "{label}");
    }

    let v1_npm = cli(
        &fixture.root,
        &[
            "build",
            "semaprax.toml",
            "--target",
            "npm",
            "-o",
            blocked_output.to_str().unwrap(),
        ],
    );
    assert_eq!(v1_npm.status.code(), Some(1), "{v1_npm:?}");
    assert!(
        stderr(&v1_npm).contains("npm facade requires the useful-text-consumer.v1 Project profile"),
        "{}",
        stderr(&v1_npm)
    );
    assert_eq!(std::fs::read(&blocked_output).unwrap(), sentinel);

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
    assert!(stderr(&existing_native).contains("exists"));
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

/// A `use` of a module no listed source declares keeps its `SPX-G172` code and
/// message; the project layer adds the `help` that names the unlisted file, or
/// says that no listed file declares the module.
#[test]
fn unresolved_import_hint_names_the_unlisted_source_file() {
    let fixture = fixture("unlisted-module");
    let app = fixture.root.join("src/app.spx");
    let source = std::fs::read_to_string(&app).unwrap();
    let source = source.replacen(
        "use function @id(\"calculator.divide\") from calculator.core as divide;\n",
        "use function @id(\"calculator.divide\") from calculator.core as divide;\nuse function @id(\"calculator.util.double\") from calculator.util as double;\n",
        1,
    );
    let source = source.replacen(
        "add(multiply(6, 7), subtract(divide(4, 2), 2))",
        "double(add(multiply(6, 7), subtract(divide(4, 2), 2)))",
        1,
    );
    std::fs::write(&app, &source).unwrap();
    std::fs::write(
        fixture.root.join("src/util.spx"),
        "module calculator.util;\n\n@id(\"calculator.util.double\")\nfn double(value: i64) -> i64\n{\n    value * 2\n}\n",
    )
    .unwrap();

    let unlisted = cli(&fixture.root, &["check", "."]);
    assert_eq!(unlisted.status.code(), Some(1));
    assert_eq!(
        stderr(&unlisted),
        "error[SPX-G172]: target module is missing or equals the caller module at src/app.spx:4:1\n  help: `src/util.spx` declares module `calculator.util` but is not listed under `sources` in semaprax.toml; add it there\n"
    );
    let json = cli(&fixture.root, &["check", ".", "--json"]);
    assert_eq!(json.status.code(), Some(1));
    let diagnostic: serde_json::Value = serde_json::from_str(stdout(&json).trim_end()).unwrap();
    assert_eq!(diagnostic["code"], "SPX-G172");
    assert_eq!(
        diagnostic["message"],
        "target module is missing or equals the caller module"
    );
    assert_eq!(
        diagnostic["help"],
        "`src/util.spx` declares module `calculator.util` but is not listed under `sources` in semaprax.toml; add it there"
    );

    std::fs::remove_file(fixture.root.join("src/util.spx")).unwrap();
    let missing = cli(&fixture.root, &["check", "."]);
    assert_eq!(missing.status.code(), Some(1));
    assert_eq!(
        stderr(&missing),
        "error[SPX-G172]: target module is missing or equals the caller module at src/app.spx:4:1\n  help: no file listed under `sources` in semaprax.toml declares module `calculator.util`; declare it in a listed `.spx` file or add that file to `sources`\n"
    );

    // Listing the file resolves the import without any hint.
    std::fs::write(
        fixture.root.join("src/util.spx"),
        "module calculator.util;\n\n@id(\"calculator.util.double\")\nfn double(value: i64) -> i64\n{\n    value * 2\n}\n",
    )
    .unwrap();
    let manifest = fixture.root.join("semaprax.toml");
    let toml = std::fs::read_to_string(&manifest).unwrap();
    std::fs::write(
        &manifest,
        toml.replacen(
            "\"src/tests.spx\"]",
            "\"src/tests.spx\", \"src/util.spx\"]",
            1,
        ),
    )
    .unwrap();
    let listed = cli(&fixture.root, &["check", "."]);
    assert!(listed.status.success(), "{}", stderr(&listed));
    let run = cli(&fixture.root, &["run", "."]);
    assert_eq!(stdout(&run), "84\n", "{}", stderr(&run));
}
