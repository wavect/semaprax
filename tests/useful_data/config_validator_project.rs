use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};

use semaprax::interpreter::{self, InterpreterOptions};
use semaprax::{codegen, project};

static SERIAL: AtomicU64 = AtomicU64::new(0);

fn fixture() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("examples/config-validator-project")
}

fn copy_fixture(label: &str) -> PathBuf {
    let root = std::env::temp_dir().join(format!(
        "semaprax-config-validator-{label}-{}-{}",
        std::process::id(),
        SERIAL.fetch_add(1, Ordering::Relaxed)
    ));
    std::fs::create_dir_all(root.join("src")).unwrap();
    for relative in [
        "semaprax.toml",
        "src/app.spx",
        "src/core.spx",
        "src/rules.spx",
        "src/tests.spx",
    ] {
        std::fs::copy(fixture().join(relative), root.join(relative)).unwrap();
    }
    root.canonicalize().unwrap()
}

fn interpret_source(source: &str, stable_id: &str, arguments: &[&str]) -> String {
    let path = std::env::temp_dir().join(format!(
        "semaprax-config-validator-interpret-{}-{}.spx",
        std::process::id(),
        SERIAL.fetch_add(1, Ordering::Relaxed)
    ));
    std::fs::write(
        &path,
        format!(
            "{source}\n@id(\"config-validator.interpreter.main\")\nfn main() -> i64\n{{\n    0\n}}\n"
        ),
    )
    .unwrap();
    let arguments = arguments
        .iter()
        .map(|value| (*value).to_owned())
        .collect::<Vec<_>>();
    let result =
        interpreter::interpret(&path, stable_id, &arguments, &InterpreterOptions::default());
    let _ = std::fs::remove_file(path);
    result.unwrap().envelope
}

fn interpreted(stable_id: &str, arguments: &[&str]) -> String {
    interpret_source(
        &std::fs::read_to_string(fixture().join("src/rules.spx")).unwrap(),
        stable_id,
        arguments,
    )
}

fn symbol(id: &str) -> String {
    use std::fmt::Write as _;
    let mut hex = String::new();
    for byte in id.bytes() {
        write!(hex, "{byte:02x}").unwrap();
    }
    format!("spx_decl_{hex}")
}

fn command_available(command: &str) -> bool {
    Command::new(command).arg("--version").output().is_ok()
}

fn npm_inventory(root: &Path) -> BTreeSet<String> {
    std::fs::read_dir(root)
        .unwrap()
        .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
        .collect()
}

fn expected_npm_inventory() -> BTreeSet<String> {
    BTreeSet::from([
        "app.wasm".to_owned(),
        "package.json".to_owned(),
        "semaprax.bindings.d.ts".to_owned(),
        "semaprax.bindings.js".to_owned(),
        "semaprax.js".to_owned(),
        "semaprax.text-exports.json".to_owned(),
    ])
}

#[test]
fn v2_project_check_graph_and_disconnected_export_roots_are_real() {
    project::with_authenticated_project(&fixture().join("semaprax.toml"), |snapshot| {
        snapshot.check()?;
        assert!(snapshot.manifest().is_v2());
        assert_eq!(snapshot.manifest().package_version(), Some("1.0.0"));
        assert_eq!(
            snapshot.manifest().profile(),
            Some(project::PROJECT_PROFILE_USEFUL_TEXT_CONSUMER_V1)
        );
        let entry_ids = snapshot
            .entry_program()
            .functions
            .iter()
            .map(|function| function.id.as_str())
            .collect::<BTreeSet<_>>();
        for selected in snapshot.manifest().web_exports() {
            assert!(
                entry_ids.contains(selected.as_str()),
                "missing selected root {selected}"
            );
            assert!(snapshot.semantic_graph().contains(selected));
        }
        assert!(entry_ids.contains("config-validator.app.main"));
        assert!(entry_ids.contains("config-validator.rules-ready"));
        assert!(!entry_ids.contains("config-validator.tests.main"));
        assert!(!entry_ids.contains("config-validator.unselected"));
        Ok(())
    })
    .unwrap();
}

#[test]
fn straight_line_rules_cover_empty_prefix_contains_unicode_and_embedded_nul() {
    for (arguments, expected) in [
        (["\"\"", "\"cfg.\"", "\"!\""], "0"),
        (["\"other=value\"", "\"cfg.\"", "\"!\""], "1"),
        (["\"cfg.bad!\"", "\"cfg.\"", "\"!\""], "2"),
        (["\"cfg.κόσμος\"", "\"cfg.\"", "\"!\""], "3"),
        (["\"cfg.a\\u0000b\"", "\"cfg.\"", "\"\\u0000\""], "2"),
    ] {
        let envelope = interpreted("config-validator.classify", &arguments);
        assert!(
            envelope.contains(&format!(r#""type":"i64","value":"{expected}""#)),
            "unexpected classification: {envelope}"
        );
    }
    let unicode = interpreted("config-validator.byte-length", &["\"cfg.κόσμος\""]);
    assert!(unicode.contains(r#""type":"i64","value":"16""#));
    let accepted = interpreted(
        "config-validator.accepts-line",
        &["\"cfg.κόσμος\"", "\"cfg.\"", "\"!\""],
    );
    assert!(accepted.contains(r#""type":"bool","value":"true""#));
}

#[test]
fn selected_stable_id_survives_a_display_rename() {
    let root = copy_fixture("rename");
    let rules_path = root.join("src/rules.spx");
    let original = std::fs::read_to_string(&rules_path).unwrap();
    let renamed = original.replacen("fn classify(", "fn inspect(", 1);
    assert_ne!(renamed, original);
    std::fs::write(&rules_path, renamed).unwrap();

    project::with_authenticated_project(&root.join("semaprax.toml"), |snapshot| {
        snapshot.check()?;
        let selected = snapshot
            .entry_program()
            .functions
            .iter()
            .find(|function| function.id.as_str() == "config-validator.classify")
            .expect("stable export identity disappeared after display rename");
        assert_eq!(selected.name, "inspect");
        assert!(snapshot
            .semantic_graph()
            .contains("config-validator.classify"));
        Ok(())
    })
    .unwrap();

    let result = interpret_source(
        &std::fs::read_to_string(&rules_path).unwrap(),
        "config-validator.classify",
        &["\"cfg.ok\"", "\"cfg.\"", "\"!\""],
    );
    assert!(result.contains(r#""type":"i64","value":"3""#));
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn project_linked_native_rules_are_length_aware_when_clang_is_available() {
    if Command::new("clang").arg("--version").output().is_err() {
        return;
    }
    let generated =
        project::with_authenticated_project(&fixture().join("semaprax.toml"), |snapshot| {
            codegen::emit_hir_c(snapshot.entry_program()).map_err(|error| vec![error])
        })
        .unwrap();
    let probe = format!(
        r#"
int main(void) {{
    struct spx_status_entry entries[UINT32_C(16)];
    struct spx_context context = {{0}};
    if (!spx_context_init(&context, UINT64_C(32), entries, UINT32_C(16), NULL, NULL, NULL)) return 10;
    const uint8_t line_bytes[] = {{'c','f','g','.',0xe2,0x98,0x83,0,'x'}};
    const uint8_t prefix_bytes[] = {{'c','f','g','.'}};
    const uint8_t nul_bytes[] = {{0}};
    spx_str_v1 line = {{line_bytes, UINT64_C(9)}};
    spx_str_v1 prefix = {{prefix_bytes, UINT64_C(4)}};
    spx_str_v1 nul = {{nul_bytes, UINT64_C(1)}};
    int64_t classification = -1;
    if ({classify}(&context, line, prefix, nul, &classification) != SPX_STATUS_SUCCESS || classification != 2) return 11;
    int64_t length = -1;
    if ({length}(&context, line, &length) != SPX_STATUS_SUCCESS || length != 9) return 12;
    return 0;
}}
"#,
        classify = symbol("config-validator.classify"),
        length = symbol("config-validator.byte-length"),
    );
    let ordinal = SERIAL.fetch_add(1, Ordering::Relaxed);
    let stem = format!(
        "semaprax-config-validator-native-{}-{ordinal}",
        std::process::id()
    );
    let source = std::env::temp_dir().join(format!("{stem}.c"));
    let executable = std::env::temp_dir().join(format!("{stem}{}", std::env::consts::EXE_SUFFIX));
    std::fs::write(&source, format!("{generated}\n{probe}")).unwrap();
    let compiled = Command::new("clang")
        .args([
            "-std=c11",
            "-O2",
            "-Wall",
            "-Wextra",
            "-Werror",
            "-DSPX_NO_ENTRY_WRAPPER",
        ])
        .arg(&source)
        .arg("-o")
        .arg(&executable)
        .output()
        .unwrap();
    assert!(
        compiled.status.success(),
        "{}",
        String::from_utf8_lossy(&compiled.stderr)
    );
    let executed = Command::new(&executable).output().unwrap();
    let _ = std::fs::remove_file(source);
    let _ = std::fs::remove_file(executable);
    assert!(
        executed.status.success(),
        "native exit {:?}",
        executed.status.code()
    );
}

#[test]
fn real_fixture_builds_installs_and_typechecks_as_an_offline_npm_package() {
    let root = copy_fixture("npm");
    let output = root.join("package");
    let inline = project::with_authenticated_project(&root.join("semaprax.toml"), |snapshot| {
        snapshot.build_npm_inline(project::MAX_PROJECT_NPM_BUILD_BYTES)
    })
    .unwrap();
    assert!(!inline.envelope().contains('\n'));
    assert!(!inline.envelope().contains('\r'));
    inline.verify().unwrap();
    project::ProjectNpmBuild::inspect_envelope(inline.envelope(), inline.max_bytes()).unwrap();
    project::with_authenticated_project(&root.join("semaprax.toml"), |snapshot| {
        snapshot.build_npm(&output)
    })
    .unwrap();
    let inventory = std::fs::read_dir(&output)
        .unwrap()
        .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
        .collect::<BTreeSet<_>>();
    assert_eq!(
        inventory,
        BTreeSet::from([
            "app.wasm".to_owned(),
            "package.json".to_owned(),
            "semaprax.bindings.d.ts".to_owned(),
            "semaprax.bindings.js".to_owned(),
            "semaprax.js".to_owned(),
            "semaprax.text-exports.json".to_owned(),
        ])
    );

    if command_available("npm") {
        let packed = Command::new("npm")
            .args(["pack", "--offline", "--ignore-scripts", "--json"])
            .env("npm_config_cache", root.join("npm-cache"))
            .current_dir(&output)
            .output()
            .unwrap();
        assert!(
            packed.status.success(),
            "{}",
            String::from_utf8_lossy(&packed.stderr)
        );
        let rows: serde_json::Value = serde_json::from_slice(&packed.stdout).unwrap();
        let tarball = output.join(rows[0]["filename"].as_str().unwrap());
        let consumer = root.join("consumer");
        std::fs::create_dir(&consumer).unwrap();
        std::fs::write(
            consumer.join("package.json"),
            "{\"name\":\"consumer\",\"private\":true,\"type\":\"module\"}\n",
        )
        .unwrap();
        let installed = Command::new("npm")
            .args([
                "install",
                "--offline",
                "--ignore-scripts",
                "--no-audit",
                "--no-fund",
            ])
            .arg(&tarball)
            .env("npm_config_cache", root.join("npm-cache"))
            .current_dir(&consumer)
            .output()
            .unwrap();
        assert!(
            installed.status.success(),
            "{}",
            String::from_utf8_lossy(&installed.stderr)
        );

        if command_available("node") {
            std::fs::write(
                consumer.join("run.mjs"),
                r#"import assert from "node:assert/strict";
import fs from "node:fs";
import { instantiate } from "config-validator";
const runtime = instantiate(fs.readFileSync("node_modules/config-validator/app.wasm"));
assert.equal(runtime.functions["config-validator.classify"]("", "cfg.", "!"), 0n);
assert.equal(runtime.functions["config-validator.classify"]("cfg.κόσμος", "cfg.", "!"), 3n);
assert.equal(runtime.functions["config-validator.classify"]("cfg.a\0b", "cfg.", "\0"), 2n);
assert.equal(runtime.functions["config-validator.byte-length"]("cfg.κόσμος"), 16n);
"#,
            )
            .unwrap();
            let ran = Command::new("node")
                .arg("run.mjs")
                .current_dir(&consumer)
                .output()
                .unwrap();
            assert!(
                ran.status.success(),
                "{}",
                String::from_utf8_lossy(&ran.stderr)
            );
        }

        if command_available("tsc") {
            std::fs::write(
                consumer.join("tsconfig.json"),
                "{\"compilerOptions\":{\"strict\":true,\"noEmit\":true,\"module\":\"NodeNext\",\"moduleResolution\":\"NodeNext\",\"target\":\"ES2022\",\"lib\":[\"ES2022\",\"DOM\"]},\"files\":[\"index.ts\"]}\n",
            )
            .unwrap();
            std::fs::write(
                consumer.join("index.ts"),
                "import { instantiate } from \"config-validator\";\ndeclare const bytes: Uint8Array;\nconst runtime = instantiate(bytes);\nconst code: bigint = runtime.functions[\"config-validator.classify\"](\"cfg.ok\", \"cfg.\", \"!\");\nconst accepted: boolean = runtime.functions[\"config-validator.accepts-line\"](\"cfg.ok\", \"cfg.\", \"!\");\nvoid code; void accepted;\n",
            )
            .unwrap();
            let checked = Command::new("tsc")
                .arg("-p")
                .arg("tsconfig.json")
                .current_dir(&consumer)
                .output()
                .unwrap();
            assert!(
                checked.status.success(),
                "{}",
                String::from_utf8_lossy(&checked.stderr)
            );
        }
    }
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn v2_api_explicit_web_and_default_cli_share_the_exact_text_package() {
    let root = copy_fixture("web-routing");
    project::with_authenticated_project(&root.join("semaprax.toml"), |snapshot| {
        let diagnostics = snapshot
            .build_web_inline(project::MAX_PROJECT_WEB_BUILD_BYTES)
            .unwrap_err();
        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].code, "SPX-W120");
        assert_eq!(
            diagnostics[0].message,
            "Project v2 pathless Web builds use build_npm_inline"
        );
        let inline = snapshot.build_npm_inline(project::MAX_PROJECT_NPM_BUILD_BYTES)?;
        inline.verify().map_err(|error| vec![error])
    })
    .unwrap();
    let api = root.join("api-web");
    project::with_authenticated_project(&root.join("semaprax.toml"), |snapshot| {
        snapshot.build_web(&api)
    })
    .unwrap();
    assert_eq!(npm_inventory(&api), expected_npm_inventory());

    let binary = env!("CARGO_BIN_EXE_semaprax");
    let explicit = root.join("explicit-web");
    let status = Command::new(binary)
        .current_dir(&root)
        .args([
            "build",
            "semaprax.toml",
            "--target",
            "web",
            "-o",
            explicit.to_str().unwrap(),
        ])
        .output()
        .unwrap();
    assert!(
        status.status.success(),
        "{}",
        String::from_utf8_lossy(&status.stderr)
    );
    assert_eq!(npm_inventory(&explicit), expected_npm_inventory());

    let wasm = root.join("explicit-wasm");
    let status = Command::new(binary)
        .current_dir(&root)
        .args([
            "build",
            "semaprax.toml",
            "--target",
            "wasm",
            "-o",
            wasm.to_str().unwrap(),
        ])
        .output()
        .unwrap();
    assert!(
        status.status.success(),
        "{}",
        String::from_utf8_lossy(&status.stderr)
    );
    assert_eq!(npm_inventory(&wasm), expected_npm_inventory());

    let implicit = root.join("implicit-web");
    let status = Command::new(binary)
        .current_dir(&root)
        .args(["build", "-o", implicit.to_str().unwrap()])
        .output()
        .unwrap();
    assert!(
        status.status.success(),
        "{}",
        String::from_utf8_lossy(&status.stderr)
    );
    assert_eq!(npm_inventory(&implicit), expected_npm_inventory());
    assert_eq!(
        std::fs::read(api.join("app.wasm")).unwrap(),
        std::fs::read(explicit.join("app.wasm")).unwrap()
    );
    assert_eq!(
        std::fs::read(api.join("app.wasm")).unwrap(),
        std::fs::read(implicit.join("app.wasm")).unwrap()
    );
    assert_eq!(
        std::fs::read(api.join("app.wasm")).unwrap(),
        std::fs::read(wasm.join("app.wasm")).unwrap()
    );
    let _ = std::fs::remove_dir_all(root);
}
