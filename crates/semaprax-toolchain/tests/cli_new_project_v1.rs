use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::sync::atomic::{AtomicU64, Ordering};
#[cfg(unix)]
use std::sync::Mutex;

#[path = "../src/new_project.rs"]
mod new_project;

#[path = "../../../tests/support/project_directory_link.rs"]
mod project_directory_link;

static SERIAL: AtomicU64 = AtomicU64::new(0);

struct Fixture {
    root: PathBuf,
}

impl Fixture {
    fn new(label: &str) -> Self {
        let root = std::env::temp_dir().join(format!(
            "semaprax-cli-new-project-{label}-{}-{}",
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
    Command::new(env!("CARGO_BIN_EXE_semaprax-full"))
        .args(arguments)
        .current_dir(root)
        .output()
        .unwrap()
}

fn assert_success(output: &Output) {
    assert!(
        output.status.success(),
        "command failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

fn read_tree(root: &Path) -> BTreeMap<String, Vec<u8>> {
    fn visit(base: &Path, path: &Path, files: &mut BTreeMap<String, Vec<u8>>) {
        let mut entries = std::fs::read_dir(path)
            .unwrap()
            .map(Result::unwrap)
            .collect::<Vec<_>>();
        entries.sort_by_key(|entry| entry.file_name());
        for entry in entries {
            let metadata = std::fs::symlink_metadata(entry.path()).unwrap();
            assert!(!metadata.file_type().is_symlink());
            if metadata.is_dir() {
                visit(base, &entry.path(), files);
            } else {
                assert!(metadata.is_file());
                files.insert(
                    entry
                        .path()
                        .strip_prefix(base)
                        .unwrap()
                        .to_string_lossy()
                        .replace('\\', "/"),
                    std::fs::read(entry.path()).unwrap(),
                );
            }
        }
    }

    let mut files = BTreeMap::new();
    visit(root, root, &mut files);
    files
}

fn parent_names(root: &Path) -> Vec<String> {
    let mut names = std::fs::read_dir(root)
        .unwrap()
        .map(|entry| entry.unwrap().file_name().into_string().unwrap())
        .collect::<Vec<_>>();
    names.sort();
    names
}

#[test]
fn calculator_template_has_exact_deterministic_bytes() {
    let fixture = Fixture::new("deterministic");
    assert_success(&cli(
        &fixture.root,
        &["new", "first", "--name", "demo-project"],
    ));
    assert_success(&cli(
        &fixture.root,
        &[
            "new",
            "second",
            "--template",
            "calculator",
            "--name",
            "demo-project",
        ],
    ));

    let first = read_tree(&fixture.root.join("first"));
    let second = read_tree(&fixture.root.join("second"));
    assert_eq!(first, second);
    assert_eq!(
        first.keys().map(String::as_str).collect::<Vec<_>>(),
        ["README.md", "semaprax.toml", "src/app.spx", "src/tests.spx"]
    );
    assert_eq!(parent_names(&fixture.root), ["first", "second"]);
    assert!(String::from_utf8(first["semaprax.toml"].clone())
        .unwrap()
        .contains("name = \"demo-project\"\nentry = \"demo_project.app\""));
}

#[test]
fn generated_project_validation_never_reopens_the_ambient_staging_tree() {
    let implementation = include_str!("../src/new_project.rs");
    assert!(implementation.contains("project::validate_owned_project_test"));
    assert!(!implementation.contains("project::with_authenticated_project"));
    assert!(
        implementation
            .find("validate_rendered_project(expected)")
            .unwrap()
            < implementation.find("create_staging_authority").unwrap()
    );
}

#[test]
fn generated_project_passes_check_test_and_web_build() {
    let fixture = Fixture::new("developer-loop");
    assert_success(&cli(&fixture.root, &["new", "calculator"]));
    assert_success(&cli(&fixture.root, &["check", "calculator/semaprax.toml"]));
    let tested = cli(&fixture.root, &["test", "calculator/semaprax.toml"]);
    assert_success(&tested);
    assert_eq!(
        String::from_utf8(tested.stdout).unwrap(),
        "project tests passed\n"
    );
    assert_success(&cli(
        &fixture.root,
        &[
            "build",
            "calculator/semaprax.toml",
            "--target",
            "web",
            "-o",
            "calculator-web",
        ],
    ));
    assert!(fixture.root.join("calculator-web/app.wasm").is_file());
    assert!(fixture
        .root
        .join("calculator-web/semaprax.scalar-exports.json")
        .is_file());
}

#[test]
fn existing_invalid_and_directory_link_destinations_are_rejected() {
    let fixture = Fixture::new("destinations");
    std::fs::create_dir(fixture.root.join("existing")).unwrap();
    let existing = cli(&fixture.root, &["new", "existing"]);
    assert_eq!(existing.status.code(), Some(1));

    let invalid = cli(&fixture.root, &["new", "Bad_Name"]);
    assert_eq!(invalid.status.code(), Some(2));
    assert!(!fixture.root.join("Bad_Name").exists());

    let destination = project_directory_link::create(&fixture.root);
    assert_eq!(destination, fixture.root.join("linked"));
    let inventory = project_directory_link::entries(&fixture.root);
    let linked = cli(&fixture.root, &["new", "linked"]);
    assert_eq!(linked.status.code(), Some(1));
    assert!(linked.stdout.is_empty());
    assert_eq!(
        String::from_utf8(linked.stderr).unwrap(),
        "new: cannot create staged project: a no-clobber entry already exists\n"
    );
    project_directory_link::assert_intact(&fixture.root);
    assert_eq!(project_directory_link::entries(&fixture.root), inventory);
    project_directory_link::remove_link(&fixture.root);
}

struct FailBeforeWrite(usize);

impl new_project::WriteHook for FailBeforeWrite {
    fn before_write(&self, index: usize, _relative_path: &str) -> Result<(), String> {
        if index == self.0 {
            Err("synthetic write failure".to_owned())
        } else {
            Ok(())
        }
    }
}

#[test]
fn injected_write_failure_never_publishes_and_cleans_owned_staging() {
    let fixture = Fixture::new("write-failure");
    let destination = fixture.root.join("calculator");
    let error =
        new_project::create_with_hook(&destination, "calculator", &FailBeforeWrite(2)).unwrap_err();
    assert_eq!(error.exit_code(), 1);
    assert!(!destination.exists());
    assert!(parent_names(&fixture.root).is_empty());
}

#[cfg(unix)]
struct SubstituteParentAfterStage {
    parent: PathBuf,
    displaced: PathBuf,
}

#[cfg(unix)]
impl new_project::WriteHook for SubstituteParentAfterStage {
    fn after_stage_created(&self) -> Result<(), String> {
        std::fs::rename(&self.parent, &self.displaced).map_err(|error| error.to_string())?;
        std::fs::create_dir(&self.parent).map_err(|error| error.to_string())?;
        std::fs::write(self.parent.join("foreign"), b"unchanged\n")
            .map_err(|error| error.to_string())
    }

    fn before_write(&self, _index: usize, _relative_path: &str) -> Result<(), String> {
        Err("write hook must not run after parent identity drift".to_owned())
    }
}

#[cfg(unix)]
#[test]
fn post_hold_parent_substitution_fails_before_any_staged_file_write() {
    let fixture = Fixture::new("post-hold-parent-substitution");
    let parent = fixture.root.join("selected");
    let displaced = fixture.root.join("selected-held");
    std::fs::create_dir(&parent).unwrap();
    let destination = parent.join("calculator");
    let hook = SubstituteParentAfterStage {
        parent: parent.clone(),
        displaced: displaced.clone(),
    };

    let error = new_project::create_with_hook(&destination, "calculator", &hook).unwrap_err();
    assert_eq!(error.exit_code(), 1);
    assert_eq!(read_tree(&parent)["foreign"], b"unchanged\n");
    assert!(!parent.join("calculator").exists());
    assert!(parent_names(&displaced).is_empty());
}

#[cfg(unix)]
struct SubstituteParentBeforeWrite {
    parent: PathBuf,
    displaced: PathBuf,
    fired: Mutex<bool>,
}

#[cfg(unix)]
impl new_project::WriteHook for SubstituteParentBeforeWrite {
    fn before_write(&self, index: usize, _relative_path: &str) -> Result<(), String> {
        let mut fired = self.fired.lock().unwrap();
        if index != 1 || *fired {
            return Ok(());
        }
        std::fs::rename(&self.parent, &self.displaced).map_err(|error| error.to_string())?;
        std::fs::create_dir(&self.parent).map_err(|error| error.to_string())?;
        std::fs::write(self.parent.join("foreign"), b"unchanged\n")
            .map_err(|error| error.to_string())?;
        *fired = true;
        Ok(())
    }
}

#[cfg(unix)]
#[test]
fn parent_substitution_cannot_redirect_staged_writes_or_publication() {
    let fixture = Fixture::new("parent-substitution");
    let parent = fixture.root.join("selected");
    let displaced = fixture.root.join("selected-held");
    std::fs::create_dir(&parent).unwrap();
    let destination = parent.join("calculator");
    let hook = SubstituteParentBeforeWrite {
        parent: parent.clone(),
        displaced: displaced.clone(),
        fired: Mutex::new(false),
    };

    let error = new_project::create_with_hook(&destination, "calculator", &hook).unwrap_err();
    assert_eq!(error.exit_code(), 1);
    assert_eq!(read_tree(&parent)["foreign"], b"unchanged\n");
    assert!(!parent.join("calculator").exists());
    assert!(parent_names(&displaced).is_empty());
}

#[cfg(unix)]
struct SubstituteStageBeforeWrite {
    parent: PathBuf,
    fired: Mutex<bool>,
}

#[cfg(unix)]
impl new_project::WriteHook for SubstituteStageBeforeWrite {
    fn before_write(&self, index: usize, _relative_path: &str) -> Result<(), String> {
        let mut fired = self.fired.lock().unwrap();
        if index != 1 || *fired {
            return Ok(());
        }
        let stage_name = std::fs::read_dir(&self.parent)
            .map_err(|error| error.to_string())?
            .map(|entry| entry.map(|entry| entry.file_name()))
            .collect::<Result<Vec<_>, _>>()
            .map_err(|error| error.to_string())?
            .into_iter()
            .find(|name| name.to_string_lossy().starts_with(".semaprax-new-"))
            .ok_or_else(|| "staging directory was not visible".to_owned())?;
        let stage = self.parent.join(&stage_name);
        std::fs::rename(&stage, self.parent.join("held-stage"))
            .map_err(|error| error.to_string())?;
        std::fs::create_dir(&stage).map_err(|error| error.to_string())?;
        std::fs::create_dir(stage.join("src")).map_err(|error| error.to_string())?;
        std::fs::write(stage.join("foreign"), b"unchanged\n").map_err(|error| error.to_string())?;
        *fired = true;
        Ok(())
    }
}

#[cfg(unix)]
#[test]
fn stage_substitution_cannot_receive_writes_or_be_published() {
    let fixture = Fixture::new("stage-substitution");
    let destination = fixture.root.join("calculator");
    let hook = SubstituteStageBeforeWrite {
        parent: fixture.root.clone(),
        fired: Mutex::new(false),
    };

    let error = new_project::create_with_hook(&destination, "calculator", &hook).unwrap_err();
    assert_eq!(error.exit_code(), 1);
    assert!(!destination.exists());
    let replacement = std::fs::read_dir(&fixture.root)
        .unwrap()
        .map(Result::unwrap)
        .find(|entry| {
            entry
                .file_name()
                .to_string_lossy()
                .starts_with(".semaprax-new-")
        })
        .unwrap()
        .path();
    assert_eq!(read_tree(&replacement)["foreign"], b"unchanged\n");
    assert!(!replacement.join("README.md").exists());
    assert!(!replacement.join("semaprax.toml").exists());
    assert!(read_tree(&replacement.join("src")).is_empty());
}

#[test]
fn unexpected_template_entries_are_rejected_without_filesystem_authority() {
    assert!(new_project::validate_template_inventory(&[
        "README.md",
        "semaprax.toml",
        "src/app.spx",
        "src/tests.spx",
    ])
    .is_ok());
    assert!(new_project::validate_template_inventory(&[
        "README.md",
        "semaprax.toml",
        "src/app.spx",
        "src/tests.spx",
        "src/extra.spx",
    ])
    .is_err());
    assert!(new_project::validate_template_inventory(&[
        "README.md",
        "semaprax.toml",
        "src/app.spx",
        "../outside",
    ])
    .is_err());
}

#[test]
fn publication_is_confined_to_the_selected_parent() {
    let fixture = Fixture::new("confinement");
    let chosen = fixture.root.join("chosen");
    let outside = fixture.root.join("outside");
    std::fs::create_dir(&chosen).unwrap();
    std::fs::create_dir(&outside).unwrap();
    std::fs::write(outside.join("sentinel"), b"unchanged\n").unwrap();

    let destination = chosen.join("safe-project");
    new_project::create_with_hook(&destination, "safe-project", &FailBeforeWrite(usize::MAX))
        .unwrap();
    assert_eq!(parent_names(&chosen), ["safe-project"]);
    assert_eq!(read_tree(&outside)["sentinel"], b"unchanged\n");
    assert_eq!(parent_names(&fixture.root), ["chosen", "outside"]);

    for arguments in [
        &["new"][..],
        &["new", "one", "two"][..],
        &["new", "project", "--template", "remote"][..],
        &["new", "project", "--name", "project", "--name", "again"][..],
    ] {
        let direct = arguments[1..]
            .iter()
            .map(|argument| (*argument).to_owned())
            .collect::<Vec<_>>();
        assert!(new_project::run(&direct).is_err(), "{arguments:?}");
        let output = cli(&fixture.root, arguments);
        assert_eq!(output.status.code(), Some(2), "{arguments:?}");
    }
}
