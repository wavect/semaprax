#[path = "support/full_toolchain.rs"]
mod full_toolchain;
#[path = "support/native_rust_cargo.rs"]
mod native_rust_cargo;
#[path = "support/project_directory_link.rs"]
mod project_directory_link;

use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::sync::atomic::{AtomicU64, Ordering};

mod project {
    use std::path::Path;

    pub(crate) const DEFAULT_MANIFEST: &str = "semaprax.toml";

    pub(crate) fn is_project_manifest(path: &Path) -> bool {
        path.file_name().and_then(|name| name.to_str()) == Some(DEFAULT_MANIFEST)
    }
}

#[allow(
    clippy::result_large_err,
    reason = "the included CLI module preserves the production Diagnostic API"
)]
#[path = "../src/cli/build.rs"]
mod build_cli;

static SERIAL: AtomicU64 = AtomicU64::new(0);

const QUICKSTART_INSTALL_COMMANDS: &str = "cargo install --locked --path .\n\
cargo install --locked --path crates/semaprax-toolchain";

const QUICKSTART_COMMANDS: &str = "semaprax-full new first-semaprax\n\
cd first-semaprax\n\
semaprax check semaprax.toml\n\
semaprax test semaprax.toml\n\
semaprax run semaprax.toml\n\
semaprax graph src/app.spx\n\
semaprax build semaprax.toml --target web -o dist/web";

const QUICKSTART_SCAFFOLD_COMMAND: &str = "semaprax project-scaffold --name first-semaprax";

struct Fixture {
    root: PathBuf,
}

impl Fixture {
    fn new(label: &str) -> Self {
        let root = std::env::temp_dir().join(format!(
            "semaprax-quickstart-v1-{label}-{}-{}",
            std::process::id(),
            SERIAL.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir(&root).unwrap();
        Self {
            root: root.canonicalize().unwrap(),
        }
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.root);
    }
}

/// The published standalone compiler, never the unpublished full toolchain.
fn standalone_binary() -> &'static Path {
    Path::new(env!("CARGO_BIN_EXE_semaprax"))
}

fn cli(root: &Path, arguments: &[&str]) -> Output {
    let binary = if arguments.first() == Some(&"new") {
        full_toolchain::binary()
    } else {
        standalone_binary()
    };
    Command::new(binary)
        .args(arguments)
        .current_dir(root)
        .output()
        .unwrap()
}

fn success(root: &Path, arguments: &[&str]) -> Output {
    let output = cli(root, arguments);
    assert!(
        output.status.success(),
        "{arguments:?} failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    output
}

#[test]
fn documented_quickstart_executes_the_exact_seven_commands() {
    let documentation =
        std::fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join("docs/QUICKSTART.md"))
            .unwrap();
    assert!(documentation.contains(&format!("```sh\n{QUICKSTART_COMMANDS}\n```")));
    let installs = documentation
        .find(&format!("```sh\n{QUICKSTART_INSTALL_COMMANDS}\n```"))
        .expect("the source quickstart must install both CLIs it invokes");
    let flow = documentation.find(QUICKSTART_COMMANDS).unwrap();
    assert!(installs < flow, "install both CLIs before invoking either");

    let fixture = Fixture::new("flow");
    success(&fixture.root, &["new", "first-semaprax"]);
    let project = fixture.root.join("first-semaprax");
    success(&project, &["check", "semaprax.toml"]);
    assert_eq!(
        String::from_utf8(success(&project, &["test", "semaprax.toml"]).stdout).unwrap(),
        "project tests passed\n"
    );
    assert_eq!(
        String::from_utf8(success(&project, &["run", "semaprax.toml"]).stdout).unwrap(),
        "42\n"
    );
    let graph = String::from_utf8(success(&project, &["graph", "src/app.spx"]).stdout).unwrap();
    assert!(graph.starts_with("{\"schema\":\"semaprax.graph.v"));
    assert!(graph.contains("\"module\":\"first_semaprax.app\""));
    success(
        &project,
        &[
            "build",
            "semaprax.toml",
            "--target",
            "web",
            "-o",
            "dist/web",
        ],
    );
    assert!(project.join("dist").is_dir());
    assert!(project.join("dist/web/app.wasm").is_file());
    assert!(project
        .join("dist/web/semaprax.scalar-exports.json")
        .is_file());
}

#[test]
fn documented_public_scaffold_is_stdout_only_and_precedes_publication() {
    let documentation =
        std::fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join("docs/QUICKSTART.md"))
            .unwrap();
    let scaffold = documentation
        .find(&format!("```sh\n{QUICKSTART_SCAFFOLD_COMMAND}\n```"))
        .unwrap();
    let publication = documentation.find(QUICKSTART_COMMANDS).unwrap();
    assert!(scaffold < publication);

    let fixture = Fixture::new("public-scaffold");
    let output = success(
        &fixture.root,
        &["project-scaffold", "--name", "first-semaprax"],
    );
    assert!(output.stderr.is_empty());
    assert_eq!(std::fs::read_dir(&fixture.root).unwrap().count(), 0);
    let value: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(value["schema"], "semaprax.project-scaffold.v1");
    assert_eq!(value["project_name"], "first-semaprax");
    assert_eq!(value["project_schema"], "semaprax.project.v1");
    assert_eq!(value["files"].as_array().unwrap().len(), 4);
}

#[test]
fn project_output_parent_is_single_level_and_retained_only_on_success() {
    let fixture = Fixture::new("parent-lifecycle");

    let existing = fixture.root.join("existing");
    std::fs::create_dir(&existing).unwrap();
    drop(build_cli::ProjectOutputParent::prepare(&existing.join("web")).unwrap());
    assert!(existing.is_dir());

    let retained = fixture.root.join("retained");
    let mut lease = build_cli::ProjectOutputParent::prepare(&retained.join("web")).unwrap();
    assert!(retained.is_dir());
    lease.retain().unwrap();
    drop(lease);
    assert!(retained.is_dir());

    let failed = fixture.root.join("failed");
    drop(build_cli::ProjectOutputParent::prepare(&failed.join("web")).unwrap());
    assert!(!failed.exists());

    let nested = fixture.root.join("missing/nested/web");
    assert!(build_cli::ProjectOutputParent::prepare(&nested).is_err());
    assert!(!fixture.root.join("missing").exists());

    let parsed = build_cli::parse(&[
        "semaprax.toml".to_owned(),
        "--target".to_owned(),
        "web".to_owned(),
        "-o".to_owned(),
        "dist/web".to_owned(),
    ])
    .unwrap();
    assert_eq!(parsed.output, Some(PathBuf::from("dist/web")));
    assert_eq!(parsed.function, None);
}

#[test]
fn failed_build_parent_cleanup_preserves_foreign_bytes_and_rejects_bad_parents() {
    let fixture = Fixture::new("hostile-parent");
    let foreign_parent = fixture.root.join("foreign");
    let lease = build_cli::ProjectOutputParent::prepare(&foreign_parent.join("web")).unwrap();
    std::fs::write(foreign_parent.join("foreign-byte"), b"retain me\n").unwrap();
    drop(lease);
    assert_eq!(
        std::fs::read(foreign_parent.join("foreign-byte")).unwrap(),
        b"retain me\n"
    );

    let file_parent = fixture.root.join("file-parent");
    std::fs::write(&file_parent, b"not a directory\n").unwrap();
    assert!(build_cli::ProjectOutputParent::prepare(&file_parent.join("web")).is_err());
    assert_eq!(std::fs::read(&file_parent).unwrap(), b"not a directory\n");

    let linked = project_directory_link::create(&fixture.root);
    let inventory = project_directory_link::entries(&fixture.root);
    let rejected = build_cli::ProjectOutputParent::prepare(&linked.join("web"))
        .err()
        .expect("a directory link must not be admitted as the Project output parent");
    assert_eq!(rejected.code, "SPX-I301");
    assert_eq!(
        rejected.message,
        "explicit Project output parent must be a real non-reparse directory"
    );
    project_directory_link::assert_intact(&fixture.root);
    assert_eq!(project_directory_link::entries(&fixture.root), inventory);
    project_directory_link::remove_link(&fixture.root);
}

struct SubstituteGrandparent {
    from: PathBuf,
    displaced: PathBuf,
}

impl build_cli::ProjectBuildParentHook for SubstituteGrandparent {
    fn before_create(&self, grandparent: &Path, _parent: &Path) -> Result<(), String> {
        assert_eq!(grandparent, self.from);
        std::fs::rename(&self.from, &self.displaced).map_err(|error| error.to_string())?;
        std::fs::create_dir(&self.from).map_err(|error| error.to_string())
    }
}

#[test]
fn grandparent_substitution_is_rejected_before_parent_creation() {
    let fixture = Fixture::new("substitution");
    let grandparent = fixture.root.join("anchor");
    let displaced = fixture.root.join("anchor-retained");
    std::fs::create_dir(&grandparent).unwrap();
    std::fs::write(grandparent.join("sentinel"), b"original\n").unwrap();
    let hook = SubstituteGrandparent {
        from: grandparent.clone(),
        displaced: displaced.clone(),
    };
    let output = grandparent.join("dist/web");
    assert!(build_cli::ProjectOutputParent::prepare_with_hook(&output, &hook).is_err());
    assert!(!grandparent.join("dist").exists());
    assert_eq!(
        std::fs::read(displaced.join("sentinel")).unwrap(),
        b"original\n"
    );
}

#[test]
fn post_publication_parent_substitution_rejects_retain_and_preserves_both_trees() {
    let fixture = Fixture::new("post-publication-substitution");
    let parent = fixture.root.join("dist");
    let displaced = fixture.root.join("dist-owned-retained");
    let mut lease = build_cli::ProjectOutputParent::prepare(&parent.join("web")).unwrap();
    std::fs::rename(&parent, &displaced).unwrap();
    std::fs::create_dir(&parent).unwrap();
    std::fs::write(parent.join("foreign-byte"), b"foreign\n").unwrap();

    assert!(lease.retain().is_err());
    drop(lease);
    assert!(displaced.is_dir());
    assert!(displaced.read_dir().unwrap().next().is_none());
    assert_eq!(
        std::fs::read(parent.join("foreign-byte")).unwrap(),
        b"foreign\n"
    );
}

/// `docs/INSTALL.md` is the single owner of "how do I get a working SEMAPRAX".
/// These cases bind its command lines, quoted error text, and prerequisite
/// versions to the standalone CLI this harness builds, so the document cannot
/// drift into commands the compiler does not accept or messages it no longer
/// produces. Nothing here builds or installs the unpublished full toolchain;
/// the private route is gated only through the standalone refusal it documents.
mod install_guide {
    use std::path::Path;
    use std::process::{Command, Output};

    /// Commands the standalone compiler hides behind its capability boundary.
    const FULL_TOOLCHAIN_ONLY: &[&str] = &["new", "doctor"];

    const GLOBAL_HELP_BANNER: &str = "SEMAPRAX — Meaning in. Verified machine code out.\n";

    const PRIVATE_NEW_STDERR: &str = "new is unavailable in the standalone crates.io package; \
use the unpublished semaprax-full toolchain CLI\n";

    const HIDDEN_NEW_STDERR: &str = "unknown command `new`\n";

    const TYPO_SUGGESTION_STDERR: &str = "unknown command `chekc`; did you mean `check`?\n";

    const MISSING_GRAPH_OPERAND_STDERR: &str =
        "graph requires exactly <file>\nhint: run `semaprax graph --help` for usage\n";

    const UNSUPPORTED_TARGET_STDERR: &str =
        "unsupported target `webb`; available: native, native-callable, web, wasm, npm\n\
hint: run `semaprax build --help` for usage\n";

    const MISSING_SCAFFOLD_NAME_STDERR: &str = "project-scaffold requires --name project-name\n\
hint: run `semaprax project-scaffold --help` for usage\n";

    const REJECTED_SCAFFOLD_NAME_STDERR: &str =
        "error[SPX-J115]: project scaffold name must match lowercase [a-z][a-z0-9-]*\n";

    /// Trailing operating-system text differs by platform, so only the stable
    /// diagnostic prefix is compared against the CLI.
    const MISSING_SOURCE_STDERR_PREFIX: &str = "error[SPX-I001]: cannot read missing.spx:";

    const DIRECTORY_SOURCE_STDERR_PREFIX: &str = "error[SPX-I001]: cannot read examples:";

    const MISSING_MANIFEST_STDERR_PREFIX: &str =
        "error[SPX-J102]: cannot inspect declared Project v1 manifest";

    const MISSING_CLANG_STDERR_PREFIX: &str =
        "error[SPX-B101]: failed to start clang; install a C11 toolchain:";

    fn checkout() -> &'static Path {
        Path::new(env!("CARGO_MANIFEST_DIR"))
    }

    fn document() -> String {
        std::fs::read_to_string(checkout().join("docs/INSTALL.md")).unwrap()
    }

    fn standalone(directory: &Path, arguments: &[&str]) -> Output {
        Command::new(super::standalone_binary())
            .args(arguments)
            .current_dir(directory)
            .output()
            .unwrap()
    }

    fn stderr(output: &Output) -> String {
        String::from_utf8(output.stderr.clone()).unwrap()
    }

    fn stdout(output: &Output) -> String {
        String::from_utf8(output.stdout.clone()).unwrap()
    }

    /// Every `semaprax`/`semaprax-full` command line the document shows inside a
    /// shell block, split into its whitespace-separated words.
    fn shell_invocations(documentation: &str) -> Vec<Vec<String>> {
        let mut invocations = Vec::new();
        let mut fenced = false;
        let mut shell = false;
        for line in documentation.lines() {
            if let Some(tag) = line.strip_prefix("```") {
                shell = !fenced && tag.trim() == "sh";
                fenced = !fenced;
                continue;
            }
            if !shell {
                continue;
            }
            let mut words = line.split_whitespace().map(str::to_owned);
            let Some(binary) = words.next() else {
                continue;
            };
            if binary != "semaprax" && binary != "semaprax-full" {
                continue;
            }
            let mut invocation = vec![binary];
            invocation.extend(words);
            invocations.push(invocation);
        }
        assert!(!fenced, "docs/INSTALL.md has an unclosed code fence");
        invocations
    }

    #[test]
    fn documented_install_commands_name_subcommands_the_cli_accepts() {
        let documentation = document();
        let invocations = shell_invocations(&documentation);
        assert!(
            invocations.len() >= 8,
            "docs/INSTALL.md must show the install flow this case gates, found {}",
            invocations.len()
        );
        assert!(
            invocations.iter().any(|words| words[0] == "semaprax-full"),
            "docs/INSTALL.md must show the private full-toolchain route"
        );
        let fixture = super::Fixture::new("install-guide-commands");

        for invocation in &invocations {
            let shown = invocation.join(" ");
            let Some(name) = invocation.get(1).map(String::as_str) else {
                panic!("docs/INSTALL.md shows `{shown}` with no command");
            };
            if let Some(flag) = name.strip_prefix("--") {
                assert!(
                    ["version", "help"].contains(&flag),
                    "docs/INSTALL.md shows unsupported global flag `{name}`"
                );
                let output = standalone(checkout(), &[name]);
                assert!(
                    output.status.success(),
                    "docs/INSTALL.md shows `{shown}`, which failed: {}",
                    stderr(&output)
                );
                continue;
            }
            if FULL_TOOLCHAIN_ONLY.contains(&name) {
                // The private route is asserted from the standalone side only:
                // the refusal the document quotes must remain exactly this.
                let output = standalone(&fixture.root, &[name]);
                assert_eq!(
                    output.status.code(),
                    Some(2),
                    "docs/INSTALL.md calls `{name}` private; the CLI disagrees"
                );
                assert_eq!(
                    stderr(&output),
                    format!(
                        "{name} is unavailable in the standalone crates.io package; \
use the unpublished semaprax-full toolchain CLI\n"
                    )
                );
                assert_eq!(std::fs::read_dir(&fixture.root).unwrap().count(), 0);
                continue;
            }
            let output = standalone(checkout(), &[name, "--help"]);
            assert!(
                output.status.success(),
                "docs/INSTALL.md shows `{shown}`, which the CLI does not accept: {}",
                stderr(&output)
            );
            assert!(
                stdout(&output).starts_with("Usage:\n"),
                "`semaprax {name} --help` no longer emits scoped usage"
            );
        }
    }

    #[test]
    fn documented_first_failure_symptoms_match_the_cli() {
        let documentation = document();
        let fixture = super::Fixture::new("install-guide-failures");

        for quoted in [
            PRIVATE_NEW_STDERR,
            HIDDEN_NEW_STDERR,
            TYPO_SUGGESTION_STDERR,
            MISSING_GRAPH_OPERAND_STDERR,
            UNSUPPORTED_TARGET_STDERR,
            MISSING_SCAFFOLD_NAME_STDERR,
            REJECTED_SCAFFOLD_NAME_STDERR,
            MISSING_SOURCE_STDERR_PREFIX,
            DIRECTORY_SOURCE_STDERR_PREFIX,
            MISSING_MANIFEST_STDERR_PREFIX,
            MISSING_CLANG_STDERR_PREFIX,
        ] {
            assert!(
                documentation.contains(quoted.trim_end()),
                "docs/INSTALL.md must quote this symptom: {quoted}"
            );
        }
        for block in [
            MISSING_GRAPH_OPERAND_STDERR,
            UNSUPPORTED_TARGET_STDERR,
            MISSING_SCAFFOLD_NAME_STDERR,
            PRIVATE_NEW_STDERR,
            TYPO_SUGGESTION_STDERR,
        ] {
            assert!(
                documentation.contains(&format!("```text\n{block}```")),
                "docs/INSTALL.md must reproduce this output verbatim: {block}"
            );
        }

        // An empty invocation: global help on stdout, nothing on stderr.
        let empty = standalone(&fixture.root, &[]);
        assert_eq!(empty.status.code(), Some(2));
        assert!(empty.stderr.is_empty());
        assert!(stdout(&empty).starts_with(GLOBAL_HELP_BANNER));

        let private = standalone(&fixture.root, &["new", "first-semaprax"]);
        assert_eq!(private.status.code(), Some(2));
        assert_eq!(stderr(&private), PRIVATE_NEW_STDERR);

        let hidden = standalone(&fixture.root, &["new", "--help"]);
        assert_eq!(hidden.status.code(), Some(2));
        assert!(stderr(&hidden).starts_with(HIDDEN_NEW_STDERR));
        assert!(stdout(&hidden).starts_with(GLOBAL_HELP_BANNER));

        let typo = standalone(&fixture.root, &["chekc"]);
        assert_eq!(typo.status.code(), Some(2));
        assert!(stderr(&typo).starts_with(TYPO_SUGGESTION_STDERR));
        assert!(stdout(&typo).starts_with(GLOBAL_HELP_BANNER));

        let operand = standalone(&fixture.root, &["graph"]);
        assert_eq!(operand.status.code(), Some(2));
        assert_eq!(stderr(&operand), MISSING_GRAPH_OPERAND_STDERR);

        let target = standalone(
            checkout(),
            &["build", "examples/meaning.spx", "--target", "webb"],
        );
        assert_eq!(target.status.code(), Some(2));
        assert_eq!(stderr(&target), UNSUPPORTED_TARGET_STDERR);

        let unnamed = standalone(&fixture.root, &["project-scaffold"]);
        assert_eq!(unnamed.status.code(), Some(2));
        assert_eq!(stderr(&unnamed), MISSING_SCAFFOLD_NAME_STDERR);

        let rejected = standalone(
            &fixture.root,
            &["project-scaffold", "--name", "First-Semaprax"],
        );
        assert_eq!(rejected.status.code(), Some(1));
        assert_eq!(stderr(&rejected), REJECTED_SCAFFOLD_NAME_STDERR);

        let missing = standalone(&fixture.root, &["check", "missing.spx"]);
        assert_eq!(missing.status.code(), Some(1));
        assert!(stderr(&missing).starts_with(MISSING_SOURCE_STDERR_PREFIX));

        let directory = standalone(checkout(), &["check", "examples"]);
        assert_eq!(directory.status.code(), Some(1));
        assert!(stderr(&directory).starts_with(DIRECTORY_SOURCE_STDERR_PREFIX));

        let manifest = standalone(&fixture.root, &["check", "semaprax.toml"]);
        assert_eq!(manifest.status.code(), Some(1));
        assert!(stderr(&manifest).starts_with(MISSING_MANIFEST_STDERR_PREFIX));

        // None of the rejected invocations may write into the fixture root.
        assert_eq!(std::fs::read_dir(&fixture.root).unwrap().count(), 0);
    }

    /// The documented native-lane symptom is reproduced by presenting the
    /// compiler with a search path that contains no C11 driver.
    #[cfg(unix)]
    #[test]
    fn documented_missing_clang_symptom_matches_the_native_lane() {
        let fixture = super::Fixture::new("install-guide-clang");
        let output = Command::new(super::standalone_binary())
            .args(["build", "examples/meaning.spx", "--target", "native", "-o"])
            .arg(fixture.root.join("meaning-native"))
            .current_dir(checkout())
            .env("PATH", "")
            .output()
            .unwrap();
        assert_eq!(output.status.code(), Some(1));
        assert!(
            stderr(&output).starts_with(MISSING_CLANG_STDERR_PREFIX),
            "unexpected native-lane failure: {}",
            stderr(&output)
        );
        assert!(!fixture.root.join("meaning-native").exists());
    }

    #[test]
    fn documented_prerequisite_versions_match_their_owners() {
        let documentation = document();
        let reported = standalone(checkout(), &["version", "--json"]);
        assert!(reported.status.success());
        let identity: serde_json::Value = serde_json::from_slice(&reported.stdout).unwrap();
        let rust_minimum = identity["rust_min"].as_str().unwrap();
        assert!(
            documentation.contains(&format!("| {rust_minimum} or newer |")),
            "docs/INSTALL.md must state Rust {rust_minimum}, the minimum the CLI reports"
        );
        let manifest = std::fs::read_to_string(checkout().join("Cargo.toml")).unwrap();
        assert!(
            manifest.contains(&format!("rust-version = \"{rust_minimum}\"")),
            "the root manifest no longer records Rust {rust_minimum}"
        );

        let node = documentation
            .split_once("| Node.js | ")
            .expect("docs/INSTALL.md must state the Node.js prerequisite")
            .1
            .split_once(' ')
            .expect("the Node.js prerequisite must name a version")
            .0;
        let workflow =
            std::fs::read_to_string(checkout().join(".github/workflows/ci.yml")).unwrap();
        assert!(
            workflow.contains(&format!("node-version: {node}")),
            "docs/INSTALL.md claims Node.js {node}, which CI does not provision"
        );
    }

    #[test]
    fn documented_routes_keep_their_naming_split_and_nonclaims() {
        let documentation = document();
        let source = documentation
            .find("cargo install --locked --path crates/semaprax-toolchain")
            .expect("docs/INSTALL.md must install the private toolchain from source");
        let archive = documentation
            .find("### The archive uses a different command name")
            .expect("docs/INSTALL.md must explain the archive's binary name");
        assert!(
            source < archive,
            "describe the source route before the archive"
        );
        assert!(documentation.contains("semaprax-full new first-semaprax"));
        assert!(documentation.contains("semaprax new first-semaprax"));
        assert!(documentation.contains("The archives are unsigned and are not notarized."));
        assert!(documentation.contains("(RELEASE-PROCESS.md#nonclaims)"));
        assert!(documentation.contains("(COMPLETION-MATRIX.md)"));
    }

    #[test]
    fn install_guide_is_cataloged_immediately_after_the_documentation_overview() {
        let summary = std::fs::read_to_string(checkout().join("docs/SUMMARY.md")).unwrap();
        assert!(
            summary.contains("- [Documentation overview](index.md)\n- [Install](INSTALL.md)\n"),
            "docs/SUMMARY.md must list the install guide right after the overview"
        );
    }
}
