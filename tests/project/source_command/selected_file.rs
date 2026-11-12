//! Owning v33 native O0/O2 process tests. Host acquisition is lazy.
use super::{project, Command, Fixture, ProjectExecutionOptions, RESOURCE_MANIFEST};
use std::path::Path;

const APP: &str = r#"module decimal.command;
permit { fs.read, process.args.read, process.stderr.write, process.stdout.write }

@id("decimal.command.main")
fn main() -> i64 uses { fs.read, process.args.read, process.stderr.write, process.stdout.write }
{
    if args_len() != 3usize {
        let usage = "usage\n";
        let view = string_as_str(usage);
        let ignored = stderr_write(str_as_bytes(view));
        2
    } else {
        let mode_view = arg_utf8(0usize);
        let mode = string_from_str(mode_view);
        let suffix_view = arg_utf8(2usize);
        let suffix = string_from_str(suffix_view);
        if mode != "read" || suffix != "--text" {
            2
        } else {
            let path = arg_utf8(1usize);
            let contents = file_read_text(path);
            let view = string_as_str(contents);
            let written = stdout_write(str_as_bytes(view));
            0
        }
    }
}
"#;

fn fixture(app: &str) -> Fixture {
    let manifest = RESOURCE_MANIFEST
        .replace(
            "source-command.resource-output.v1",
            "source-command.selected-file.v1",
        )
        .replace(
            "argv-utf8+file-text.v1",
            "argv-utf8+selected-file.follow.v1",
        );
    Fixture::new(app, &manifest)
}

fn compile(fixture: &Fixture, optimization: &str, extra: &str, suffix: &str) -> std::path::PathBuf {
    let source =
        project::with_authenticated_project(&fixture.0.join("semaprax.toml"), |snapshot| {
            semaprax::codegen::emit_hir_c_with_selected_file_command(snapshot.entry_program())
                .map_err(|error| vec![error])
        })
        .unwrap();
    let c = fixture.0.join(format!("selected-{optimization}.c"));
    let binary = fixture.0.join(format!("selected-{optimization}"));
    std::fs::write(&c, format!("{extra}{source}{suffix}")).unwrap();
    let output = Command::new("clang")
        .args(["-std=c11", "-Wall", "-Wextra", "-Werror", optimization])
        .arg(&c)
        .arg("-o")
        .arg(&binary)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    binary
}

fn selected(binary: &Path, cwd: &Path, path: &str) -> std::process::Output {
    Command::new(binary)
        .current_dir(cwd)
        .args(["--spx-selected-file-v1", "1", "--", "read", path, "--text"])
        .output()
        .unwrap()
}

#[test]
fn legacy_file_commands_refuse_symlinks_at_each_path_boundary() {
    for manifest in [super::MANIFEST, RESOURCE_MANIFEST] {
        let fixture = Fixture::new(APP, manifest);
        let binary = fixture.0.join("legacy-command");
        project::with_authenticated_project(&fixture.0.join("semaprax.toml"), |snapshot| {
            snapshot.build_native(&binary)
        })
        .unwrap();

        std::fs::create_dir(fixture.0.join("data")).unwrap();
        std::fs::write(fixture.0.join("data/input"), b"kept\n").unwrap();
        std::os::unix::fs::symlink("data/input", fixture.0.join("file-link")).unwrap();
        std::os::unix::fs::symlink("data", fixture.0.join("dir-link")).unwrap();
        let invoke = |path: &str| {
            Command::new(&binary)
                .current_dir(&fixture.0)
                .args(["read", path, "--text"])
                .output()
                .unwrap()
        };

        let accepted = invoke("data/input");
        assert!(accepted.status.success(), "{accepted:?}");
        assert_eq!(accepted.stdout, b"kept\n");
        assert!(accepted.stderr.is_empty(), "{accepted:?}");
        for path in ["file-link", "dir-link/input"] {
            let refused = invoke(path);
            assert_eq!(refused.status.code(), Some(1), "{path}: {refused:?}");
            assert!(refused.stdout.is_empty(), "{path}: {refused:?}");
            assert!(
                String::from_utf8_lossy(&refused.stderr).contains("semaprax.filesystem.v1/7"),
                "{path}: {refused:?}"
            );
        }
    }
}

#[test]
fn manifest_graph_and_unsupported_routes_are_exact() {
    let fixture = fixture(APP);
    project::with_authenticated_project(&fixture.0.join("semaprax.toml"), |snapshot| {
        let graph: serde_json::Value = serde_json::from_str(snapshot.semantic_graph()).unwrap();
        assert_eq!(graph["project_schema"], "semaprax.project.v33");
        assert_eq!(
            snapshot
                .execute_entry(&ProjectExecutionOptions::default())
                .unwrap_err()[0]
                .code,
            "SPX-F102"
        );
        assert_eq!(snapshot.test_wasm_module().unwrap_err()[0].code, "SPX-W120");
        assert_eq!(
            snapshot.build_npm_inline(1_000_000).unwrap_err()[0].code,
            "SPX-W120"
        );
        let lock: serde_json::Value =
            serde_json::from_str(&project::render_project_lock(snapshot)?).unwrap();
        assert_eq!(
            lock["payload"]["package"]["contract"],
            "semaprax.project.v33"
        );
        assert_eq!(
            lock["payload"]["interface"]["digest"],
            serde_json::Value::Null
        );
        Ok(())
    })
    .unwrap();
}

#[test]
fn selected_profile_rejects_wrong_input_missing_authority_and_portable_targets() {
    let manifest = RESOURCE_MANIFEST
        .replace(
            "source-command.resource-output.v1",
            "source-command.selected-file.v1",
        )
        .replace(
            "argv-utf8+file-text.v1",
            "argv-utf8+selected-file.follow.v1",
        );
    for invalid in [
        manifest.replace(
            "argv-utf8+selected-file.follow.v1",
            "argv-utf8+file-text.v1",
        ),
        manifest.replace("\"fs.read\", ", ""),
        manifest.replace("\"process.args.read\", ", ""),
        manifest.replace("matrix = [\"native64\"]", "matrix = [\"wasm32\"]"),
    ] {
        let errors = super::ProjectManifest::parse(&invalid).unwrap_err();
        assert_eq!(errors[0].code, "SPX-J100");
    }
}

#[test]
fn selected_paths_keep_application_modes_order_and_named_follow_policy() {
    let fixture = fixture(APP);
    let payload = "selected UTF-8 ☃\n".as_bytes();
    let file = fixture.0.join(".hidden input");
    std::fs::write(&file, payload).unwrap();
    let nested = fixture.0.join("nested");
    std::fs::create_dir(&nested).unwrap();
    std::os::unix::fs::symlink(&file, fixture.0.join("chosen-link")).unwrap();
    for optimization in ["-O0", "-O2"] {
        let binary = compile(&fixture, optimization, "", "");
        for (cwd, path) in [
            (fixture.0.as_path(), file.to_str().unwrap()),
            (fixture.0.as_path(), ".hidden input"),
            (nested.as_path(), "../.hidden input"),
            (fixture.0.as_path(), "chosen-link"),
        ] {
            let output = selected(&binary, cwd, path);
            assert_eq!(
                output.status.code(),
                Some(0),
                "{}",
                String::from_utf8_lossy(&output.stderr)
            );
            assert_eq!(output.stdout, payload);
            assert!(output.stderr.is_empty());
        }
        // Checked usage wins even when a valid host selector names a missing file.
        let usage = Command::new(&binary)
            .args([
                "--spx-selected-file-v1",
                "1",
                "--",
                "wrong-mode",
                "missing",
                "--text",
            ])
            .output()
            .unwrap();
        assert_eq!(usage.status.code(), Some(2));
        assert!(usage.stdout.is_empty());
        assert!(usage.stderr.is_empty());
        let no_args = Command::new(&binary).output().unwrap();
        assert_eq!(no_args.status.code(), Some(2));
        assert_eq!(no_args.stderr, b"usage\n");
        assert!(no_args.stdout.is_empty());
    }
}

#[test]
fn missing_grants_hostile_inputs_and_exact_file_bound_do_not_publish() {
    let fixture = fixture(APP);
    std::fs::write(fixture.0.join("full"), vec![b'x'; 65_536]).unwrap();
    std::fs::write(fixture.0.join("overflow"), vec![b'x'; 65_537]).unwrap();
    std::fs::write(fixture.0.join("invalid-utf8"), [0xff]).unwrap();
    std::fs::write(fixture.0.join("empty"), []).unwrap();
    std::os::unix::fs::symlink("loop", fixture.0.join("loop")).unwrap();
    for optimization in ["-O0", "-O2"] {
        let binary = compile(&fixture, optimization, "", "");
        let full = selected(&binary, &fixture.0, "full");
        assert_eq!(full.status.code(), Some(0));
        assert_eq!(full.stdout, vec![b'x'; 65_536]);
        assert!(full.stderr.is_empty());
        let empty = selected(&binary, &fixture.0, "empty");
        assert_eq!(empty.status.code(), Some(0));
        assert!(empty.stdout.is_empty());
        for path in ["overflow", "invalid-utf8", ".", "missing", "loop"] {
            let output = selected(&binary, &fixture.0, path);
            assert_eq!(output.status.code(), Some(1), "{path}");
            assert!(output.stdout.is_empty(), "{path}");
            assert!(!output.stderr.is_empty(), "{path}");
        }
        let absent = Command::new(&binary)
            .current_dir(&fixture.0)
            .args(["read", "full", "--text"])
            .output()
            .unwrap();
        assert_eq!(absent.status.code(), Some(1));
        assert!(absent.stdout.is_empty());
        for prefix in ["01", "16", "-1", "3"] {
            let output = Command::new(&binary)
                .args([
                    "--spx-selected-file-v1",
                    prefix,
                    "--",
                    "read",
                    "full",
                    "--text",
                ])
                .output()
                .unwrap();
            assert_eq!(output.status.code(), Some(2));
            assert!(output.stdout.is_empty());
        }
    }
}

#[test]
fn arbitrary_source_string_cannot_change_selected_authority() {
    let app = APP.replace("let contents = file_read_text(path);", "let other = \"other\";\n            let other_view = string_as_str(other);\n            let contents = file_read_text(other_view);");
    let fixture = fixture(&app);
    std::fs::write(fixture.0.join("selected"), "selected").unwrap();
    std::fs::write(fixture.0.join("other"), "other").unwrap();
    for optimization in ["-O0", "-O2"] {
        let binary = compile(&fixture, optimization, "", "");
        let output = selected(&binary, &fixture.0, "selected");
        assert_eq!(output.status.code(), Some(1));
        assert!(output.stdout.is_empty());
        assert!(!output.stderr.is_empty());
    }
}

#[test]
fn held_descriptor_budget_drift_and_settlement_are_physical_at_o0_o2() {
    let fixture = fixture(APP);
    for optimization in ["-O0", "-O2"] {
        let binary = compile(
            &fixture,
            optimization,
            include_str!("selected_file/probe_prefix.c"),
            include_str!("selected_file/probe.c"),
        );
        for scenario in [
            "held",
            "drift",
            "budget",
            "close-fail",
            "sticky-root",
            "acquire-fail",
        ] {
            let original = fixture.0.join("original");
            let other = fixture.0.join("other");
            let selected = fixture.0.join("chosen");
            std::fs::write(&original, "original").unwrap();
            std::fs::write(&other, "other").unwrap();
            let _ = std::fs::remove_file(&selected);
            std::os::unix::fs::symlink(&original, &selected).unwrap();
            let path = if scenario == "acquire-fail" {
                fixture.0.join("missing")
            } else {
                selected
            };
            let output = Command::new(&binary)
                .current_dir(&fixture.0)
                .args(["--spx-selected-file-v1", "0", "--"])
                .arg(path)
                .arg(&other)
                .arg(scenario)
                .output()
                .unwrap();
            assert!(
                output.status.success(),
                "{scenario}: {}",
                String::from_utf8_lossy(&output.stderr)
            );
            if scenario == "held" {
                assert_eq!(output.stdout, b"published");
                assert!(output.stderr.is_empty());
            } else {
                assert!(output.stdout.is_empty(), "{scenario}");
                let code = match scenario {
                    "drift" => 7,
                    "budget" => 3,
                    "sticky-root" => 2,
                    _ => 5,
                };
                assert_eq!(
                    output.stderr,
                    format!("SEMAPRAX operation failure: semaprax.selected-file.v1/{code}\n")
                        .as_bytes(),
                    "{scenario}"
                );
            }
        }
    }
}
