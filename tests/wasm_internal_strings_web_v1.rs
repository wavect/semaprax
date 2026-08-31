//! Consumers of the actual explicit-profile CLI package.
//! Provisioned TypeScript/browser gates never install or silently skip tools.
#[cfg(any(unix, windows))]
use std::fs;
use std::path::Path;
use std::process::{Command, Output};

use semaprax::wasm::internal_strings::{emit_module, InternalStringOptions};

#[path = "wasm_internal_strings_web_v1/package_replay.rs"]
mod package_replay;
#[cfg(any(unix, windows))]
#[path = "support/project_directory_link.rs"]
mod project_directory_link;
use package_replay::{reopen, replay, Fixture, INVENTORY};

const IDS: [&str; 8] = [
    "-web.constant",
    "__proto__",
    "web.\"</script>λ",
    "web.bool",
    "web.capacity",
    "web.content",
    "web.divide",
    "web.required",
];

fn source() -> String {
    let source = include_str!("wasm_internal_strings_web_v1/source.spx")
        .replace("__CAPACITY_TEXT__", &"x".repeat(4096));
    let ast = semaprax::check(&source, "web.spx").unwrap();
    let canonical = semaprax::format::canonical(&ast);
    let reparsed = semaprax::check(&canonical, "canonical.spx").unwrap();
    assert_eq!(canonical, semaprax::format::canonical(&reparsed));
    assert_eq!(
        semaprax::graph::to_json(&ast).unwrap(),
        semaprax::graph::to_json(&reparsed).unwrap()
    );
    canonical
}

fn cli(source: &Path, output: &Path, target: &str, ids: &[&str]) -> Output {
    let mut command = Command::new(env!("CARGO_BIN_EXE_semaprax"));
    command
        .arg("build")
        .arg(source)
        .args(["--target", target, "--profile", "internal-strings-v1"]);
    for id in ids {
        // Equals spelling preserves source-valid leading-dash identities.
        command.arg(format!("--export={id}"));
    }
    command.arg("-o").arg(output).output().unwrap()
}

fn success(output: Output) {
    assert!(
        output.status.success(),
        "{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(output.stderr.is_empty());
}

fn node(fixture: &Fixture, script: &str, package: &Path) {
    let output = Command::new(std::env::var_os("NODE").unwrap_or_else(|| "node".into()))
        .arg(script)
        .arg(package)
        .current_dir(&fixture.root)
        .output()
        .expect("Node is required for the selected String Web package evidence");
    assert!(
        output.status.success(),
        "{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(output.stderr.is_empty());
    assert_eq!(
        String::from_utf8(output.stdout).unwrap().trim(),
        "string-web-node-ok"
    );
}

#[test]
fn real_cli_packages_replay_exactly_and_preserve_direct_compiler_and_renamed_apis() {
    let source = source();
    let mut fixture = Fixture::new("cli");
    let path = fixture.write("source.spx", &source);
    let first = fixture.package("first");
    let second = fixture.package("second");
    let mut reverse = IDS;
    reverse.reverse();
    success(cli(&path, &first, "web", &IDS));
    success(cli(&path, &second, "wasm", &reverse));
    let files = reopen(&first);
    assert_eq!(files, reopen(&second));
    replay(&files, &source).unwrap();
    let ast = semaprax::check(&source, "source.spx").unwrap();
    let module = emit_module(
        &ast,
        &IDS.map(str::to_owned),
        InternalStringOptions::default(),
    )
    .unwrap();
    assert_eq!(files["app.wasm"], module.wasm_bytes());
    assert_eq!(files["semaprax.js"], module.runtime_source().as_bytes());
    assert_eq!(
        files["semaprax.internal-strings.json"],
        module.descriptor().as_bytes()
    );
    let package: serde_json::Value = serde_json::from_slice(&files["package.json"]).unwrap();
    assert_eq!(
        package,
        serde_json::json!({"private":true,"type":"module","exports":"./semaprax.js","types":"./semaprax.d.ts"})
    );
    let html = std::str::from_utf8(&files["index.html"]).unwrap();
    let app = std::str::from_utf8(&files["app.js"]).unwrap();
    for id in IDS {
        assert!(!html.contains(id));
        assert!(!app.contains(id));
    }

    // No caller refers to the display name, so this real checked rename
    // preserves the persistent identity and complete selected implementation.
    let renamed = source.replace("fn content()", "fn renamed_content()");
    assert_ne!(source, renamed);
    let renamed_ast = semaprax::check(&renamed, "renamed.spx").unwrap();
    assert_ne!(
        semaprax::graph::revision(&ast),
        semaprax::graph::revision(&renamed_ast)
    );
    let renamed_path = fixture.write("renamed.spx", &renamed);
    let renamed_output = fixture.package("renamed");
    success(cli(&renamed_path, &renamed_output, "web", &IDS));
    let renamed_files = reopen(&renamed_output);
    replay(&renamed_files, &renamed).unwrap();
    for name in INVENTORY
        .into_iter()
        .filter(|name| *name != "semaprax.manifest.json")
    {
        assert_eq!(files[name], renamed_files[name], "rename changed {name}");
    }
    assert_ne!(
        files["semaprax.manifest.json"],
        renamed_files["semaprax.manifest.json"]
    );
    fixture.write(
        "node.mjs",
        include_str!("wasm_internal_strings_web_v1/node.mjs"),
    );
    node(&fixture, "node.mjs", &first);
    node(&fixture, "node.mjs", &renamed_output);

    for name in INVENTORY {
        let mut mutated = files.clone();
        mutated.get_mut(name).unwrap().push(b' ');
        assert!(
            replay(&mutated, &source).is_err(),
            "accepted changed {name}"
        );
    }
    let mut missing = files.clone();
    missing.remove("app.js");
    assert!(replay(&missing, &source).is_err());
    let mut extra = files.clone();
    extra.insert("foreign".into(), vec![1]);
    assert!(replay(&extra, &source).is_err());
    assert!(replay(&files, &renamed).is_err());
    for replacement in [
        "{\"schema\":\"foreign\",",
        "{\"unexpected\":true,",
        "{\"schema\":\"semaprax.web-internal-strings.v1\",",
    ] {
        let mut mutated = files.clone();
        let original = std::str::from_utf8(&files["semaprax.manifest.json"]).unwrap();
        mutated.insert(
            "semaprax.manifest.json".into(),
            format!("{replacement}{}", &original[1..]).into_bytes(),
        );
        assert!(replay(&mutated, &source).is_err());
    }
    fixture.cleanup();
}

#[test]
fn invalid_cli_profiles_and_admission_create_no_output_and_existing_bytes_survive() {
    let mut fixture = Fixture::new("admission");
    let absent = fixture.root.join("not-created");
    let missing = fixture.root.join("missing.spx");
    for flags in [
        vec![
            "--profile",
            "unknown",
            "--target",
            "web",
            "--export",
            "main",
        ],
        vec![
            "--profile",
            "internal-strings-v1",
            "--profile",
            "internal-strings-v1",
            "--target",
            "web",
            "--export",
            "main",
        ],
        vec!["--profile", "internal-strings-v1", "--target", "web"],
        vec![
            "--profile",
            "internal-strings-v1",
            "--target",
            "native",
            "--export",
            "main",
        ],
        vec![
            "--profile",
            "internal-strings-v1",
            "--target",
            "web",
            "--function",
            "main",
            "--export",
            "main",
        ],
    ] {
        let output = Command::new(env!("CARGO_BIN_EXE_semaprax"))
            .arg("build")
            .arg(&missing)
            .args(flags)
            .arg("-o")
            .arg(&absent)
            .output()
            .unwrap();
        assert_eq!(output.status.code(), Some(2));
        assert!(!String::from_utf8_lossy(&output.stderr).contains("SPX-I201"));
        assert!(!absent.exists());
    }
    for prefix in [vec![], vec!["--manifest-path", "missing.toml"]] {
        let output = Command::new(env!("CARGO_BIN_EXE_semaprax"))
            .arg("build")
            .args(prefix)
            .args([
                "--profile",
                "internal-strings-v1",
                "--target",
                "web",
                "--export",
                "main",
            ])
            .arg("-o")
            .arg(&absent)
            .current_dir(&fixture.root)
            .output()
            .unwrap();
        assert_eq!(output.status.code(), Some(2));
        assert!(!absent.exists());
    }
    let path = fixture.write("source.spx", source());
    let invalid_source = source().replace("@id(\"-web.constant\")", "@id(\"\")");
    let invalid_path = fixture.write("empty-identity.spx", &invalid_source);
    let errors = semaprax::check(&invalid_source, "empty-identity.spx").unwrap_err();
    assert!(errors.iter().any(|error| error.code == "SPX-H006"));
    // An unselected invalid identity must still fail before publication.
    let error = cli(&invalid_path, &absent, "web", &["web.content"]);
    assert!(!error.status.success());
    assert!(String::from_utf8_lossy(&error.stderr).contains("SPX-H006"));
    assert!(!absent.exists());
    let error = cli(&path, &absent, "web", &["missing"]);
    assert!(!error.status.success());
    assert!(String::from_utf8_lossy(&error.stderr).contains("SPX-W111"));
    assert!(!absent.exists());
    let error = cli(&path, &absent, "web", &["web.content", "web.content"]);
    assert!(!error.status.success());
    assert!(!absent.exists());
    let nested = fixture.root.join("missing-parent").join("output");
    let error = cli(&path, &nested, "web", &IDS);
    assert!(String::from_utf8_lossy(&error.stderr).contains("SPX-I301"));
    assert!(!nested.parent().unwrap().exists());
    let existing = fixture.package("existing");
    success(cli(&path, &existing, "web", &IDS));
    let original = reopen(&existing);
    let error = cli(&path, &existing, "web", &IDS);
    assert!(String::from_utf8_lossy(&error.stderr).contains("SPX-I307"));
    assert_eq!(reopen(&existing), original);
    fixture.cleanup();
}

#[cfg(any(unix, windows))]
#[test]
fn directory_link_output_and_parent_do_not_publish_through_foreign_paths() {
    let mut fixture = Fixture::new("directory-link");
    let path = fixture.write("source.spx", source());
    let alias = project_directory_link::create(&fixture.root);
    let expected_entries = project_directory_link::entries(&fixture.root);
    for (output, code) in [(&alias, "SPX-I307"), (&alias.join("child"), "SPX-I301")] {
        let result = cli(&path, output, "web", &IDS);
        assert!(!result.status.success());
        assert!(String::from_utf8_lossy(&result.stderr).contains(code));
        // The mandatory helper authenticates the link/reparse target, its
        // complete one-file inventory and exact sentinel bytes on both hosts.
        project_directory_link::assert_intact(&fixture.root);
        assert_eq!(
            project_directory_link::entries(&fixture.root),
            expected_entries
        );
    }
    project_directory_link::remove_link(&fixture.root);
    let foreign = fixture.root.join("symlink-target");
    fs::remove_file(foreign.join("sentinel")).unwrap();
    fs::remove_dir(foreign).unwrap();
    fixture.cleanup();
}

#[test]
#[ignore = "requires provisioned TypeScript 5.8.3 via TSC; no downloads"]
fn provisioned_typescript_checks_real_overloads_and_rejects_wrong_consumers() {
    let tsc = std::env::var_os("TSC").expect("TSC must name provisioned TypeScript 5.8.3");
    let version = Command::new(&tsc).arg("--version").output().unwrap();
    assert!(version.status.success());
    assert_eq!(
        String::from_utf8_lossy(&version.stdout).trim(),
        "Version 5.8.3"
    );
    let mut fixture = Fixture::new("typescript");
    let path = fixture.write("source.spx", source());
    let generated = fixture.package("generated");
    success(cli(&path, &generated, "web", &IDS));
    fixture.write("package.json", "{\"private\":true,\"type\":\"module\"}\n");
    fixture.write(
        "consumer.ts",
        include_str!("wasm_internal_strings_web_v1/consumer.ts"),
    );
    let consumer_root = fixture.root.clone();
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
            .current_dir(&consumer_root)
            .output()
            .unwrap()
    };
    success(compile("consumer.ts"));
    for (name, expression, codes) in [
        (
            "wrong-argument.ts",
            "runtime.call('web.divide', 2);",
            &["TS2769", "TS2345"][..],
        ),
        (
            "unknown-export.ts",
            "runtime.call('missing');",
            &["TS2769", "TS2345"][..],
        ),
        (
            "unchecked-result.ts",
            "const value: bigint = runtime.call('web.content').value;",
            &["TS2339"][..],
        ),
    ] {
        fixture.write(name, format!("import {{instantiate}} from './generated/semaprax.js';\ndeclare const runtime: Awaited<ReturnType<typeof instantiate>>;\n{expression}\n"));
        let output = compile(name);
        assert!(!output.status.success());
        let diagnostics = String::from_utf8(output.stdout).unwrap();
        assert!(diagnostics.contains(name));
        assert!(
            codes.iter().any(|code| diagnostics.contains(code)),
            "{diagnostics}"
        );
    }
    fixture.cleanup();
}

#[test]
#[ignore = "requires provisioned PLAYWRIGHT_MODULE and CHROMIUM_EXECUTABLE; no downloads"]
fn provisioned_chromium_consumes_actual_page_and_rejects_descriptor_and_wasm_drift() {
    let playwright = std::env::var_os("PLAYWRIGHT_MODULE")
        .expect("PLAYWRIGHT_MODULE must name provisioned Playwright's ESM entry");
    let chromium = std::env::var_os("CHROMIUM_EXECUTABLE")
        .expect("CHROMIUM_EXECUTABLE must name provisioned Chromium");
    assert!(Path::new(&playwright).is_absolute() && Path::new(&chromium).is_absolute());
    let mut fixture = Fixture::new("chromium");
    let path = fixture.write("source.spx", source());
    let generated = fixture.package("generated");
    success(cli(&path, &generated, "web", &IDS));
    fixture.write(
        "browser.mjs",
        include_str!("wasm_internal_strings_web_v1/browser.mjs"),
    );
    let output = Command::new(std::env::var_os("NODE").unwrap_or_else(|| "node".into()))
        .arg("browser.mjs")
        .arg(&generated)
        .arg(playwright)
        .arg(chromium)
        .current_dir(&fixture.root)
        .output()
        .expect("Node is required for the explicitly selected browser gate");
    assert!(
        output.status.success(),
        "{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(output.stderr.is_empty());
    assert_eq!(
        String::from_utf8(output.stdout).unwrap().trim(),
        "string-web-chromium-ok"
    );
    fixture.cleanup();
}
