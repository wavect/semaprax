#[path = "support/full_toolchain.rs"]
mod full_toolchain;
#[path = "support/native_rust_cargo.rs"]
mod native_rust_cargo;

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

const QUICKSTART_COMMANDS: &str = "semaprax-full new first-semaprax\n\
cd first-semaprax\n\
semaprax check semaprax.toml\n\
semaprax test semaprax.toml\n\
semaprax run semaprax.toml\n\
semaprax graph src/app.spx\n\
semaprax build semaprax.toml --target web -o dist/web";

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

fn cli(root: &Path, arguments: &[&str]) -> Output {
    let binary = if arguments.first() == Some(&"new") {
        full_toolchain::binary()
    } else {
        Path::new(env!("CARGO_BIN_EXE_semaprax"))
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

    let target = fixture.root.join("symlink-target");
    std::fs::create_dir(&target).unwrap();
    let linked = fixture.root.join("linked");
    if create_directory_symlink(&target, &linked) {
        assert!(build_cli::ProjectOutputParent::prepare(&linked.join("web")).is_err());
        assert!(target.read_dir().unwrap().next().is_none());
    }
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

#[cfg(unix)]
fn create_directory_symlink(target: &Path, link: &Path) -> bool {
    std::os::unix::fs::symlink(target, link).unwrap();
    true
}

#[cfg(windows)]
fn create_directory_symlink(target: &Path, link: &Path) -> bool {
    match std::os::windows::fs::symlink_dir(target, link) {
        Ok(()) => true,
        Err(error) if error.kind() == std::io::ErrorKind::PermissionDenied => false,
        Err(error) => panic!("cannot create test directory symlink: {error}"),
    }
}
