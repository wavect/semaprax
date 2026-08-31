//! Real archived-CLI publication and unchanged external frame consumers.
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::time::Duration;

#[path = "../support/owned_frame_artifacts.rs"]
mod artifacts;
#[path = "owned_frame/consumers.rs"]
mod consumers;
#[path = "owned_frame/files.rs"]
mod files;
#[path = "../support/native_rust_cargo.rs"]
mod native_rust_cargo;
#[path = "../support/native_rust_target.rs"]
mod native_rust_target;

const CORPUS: &[u8] = include_bytes!("../../examples/frame-payload-project/corpus.json");
const SUPPLEMENT: &[u8] = include_bytes!("../frame_payload_product_v1/adversarial.json");

struct Runner {
    captures: PathBuf,
    next: usize,
}

impl Runner {
    fn run(&mut self, command: &mut Command, timeout: Duration) -> Output {
        let captures = self.captures.join(format!("call-{}", self.next));
        self.next += 1;
        let result = super::command::run(
            command,
            &[],
            &captures,
            timeout,
            1024 * 1024,
            4 * 1024 * 1024,
        );
        assert!(
            result.status.success(),
            "{command:?}: status={:?} stdout={} stderr={}",
            result.status.code(),
            String::from_utf8_lossy(&result.stdout),
            String::from_utf8_lossy(&result.stderr)
        );
        result
    }
}

fn tool(variable: &str) -> PathBuf {
    let path = PathBuf::from(
        std::env::var_os(variable)
            .unwrap_or_else(|| panic!("archive frame gate requires absolute {variable}")),
    );
    assert!(path.is_absolute() && path.is_file(), "invalid {variable}");
    path
}

fn build(
    runner: &mut Runner,
    cli: &Path,
    root: &Path,
    project: &Path,
    target: &str,
    output: &Path,
    tools: &Tools,
) {
    assert!(!output.exists());
    let mut command = Command::new(cli);
    command
        .current_dir(root)
        .args(["build", "--manifest-path"])
        .arg(project.join("semaprax.toml"))
        .args(["--target", target, "-o"])
        .arg(output)
        .env("CLANG", &tools.clang)
        .env("SEMAPRAX_ARCHIVER", &tools.archiver)
        .env("CARGO_NET_OFFLINE", "true");
    runner.run(&mut command, Duration::from_secs(300));
}

struct Tools {
    node: PathBuf,
    clang: PathBuf,
    archiver: PathBuf,
    cargo: PathBuf,
}

/// The archive gate supplies its admitted CLI and a fresh, empty outside-checkout root.
pub(super) fn run(cli: &Path, root: &Path) {
    assert!(cli.is_absolute() && cli.is_file());
    assert!(root.is_absolute());
    let metadata = fs::symlink_metadata(root).unwrap();
    assert!(metadata.is_dir() && !metadata.file_type().is_symlink());
    assert!(fs::read_dir(root).unwrap().next().is_none());
    let tools = Tools {
        node: tool("NODE"),
        clang: tool("CLANG"),
        archiver: tool("SEMAPRAX_ARCHIVER"),
        cargo: tool("CARGO"),
    };
    let captures = root.join("captures");
    fs::create_dir(&captures).unwrap();
    let mut runner = Runner { captures, next: 0 };
    // The finite-command helper settles only the direct Cargo child. On a
    // failed command it cannot prove that rustc descendants have stopped
    // using this cache, so this archive gate never recursively deletes it.
    let cargo_target = std::mem::ManuallyDrop::new(native_rust_target::CargoTarget::new());
    eprintln!(
        "retained archive consumer Cargo target: {}",
        cargo_target.path().display()
    );
    let mut subjects = Vec::new();
    for (label, renamed) in [("before", false), ("after", true)] {
        let project = root.join(format!("{label}-project"));
        files::project(&project, renamed);
        let sources = files::project_snapshot(&project);
        let web = root.join(format!("{label}-web"));
        fs::create_dir(&web).unwrap();
        consumers::prepare_web(&web);
        let npm = web.join("generated");
        build(&mut runner, cli, root, &project, "npm", &npm, &tools);
        let sdk = root.join(format!("{label}-generated-sdk"));
        build(&mut runner, cli, root, &project, "rust", &sdk, &tools);
        // One shared oracle replays the real source subject and reconstructs
        // the exact manifest/provider/inventory facts from published bytes.
        let bound = artifacts::verify_artifacts(&project, &npm, &sdk);
        assert_eq!(bound.revision().manifest().schema(), "semaprax.project.v8");
        assert_eq!(bound.descriptor().exports().len(), 3);
        let npm_before = files::flat_snapshot(&npm);
        let sdk_before = files::flat_snapshot(&sdk);
        consumers::node(&mut runner, &tools.node, &web);
        consumers::rust(&mut runner, &tools.cargo, root, label, cargo_target.path());
        assert_eq!(files::flat_snapshot(&npm), npm_before);
        assert_eq!(files::flat_snapshot(&sdk), sdk_before);
        assert_eq!(files::project_snapshot(&project), sources);
        subjects.push(bound);
    }
    artifacts::verify_display_rename(&subjects[0], &subjects[1]);
    // Packages, captures and the separately owned Cargo cache are retained on
    // success and failure; later cleanup requires independent ownership checks.
}
