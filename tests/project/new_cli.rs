//! The standalone compiler's `new`: the bounded create-new route owned by
//! `docs/NEW-PROJECT-STANDALONE-V1.md`. Grammar, template bytes, and the
//! success line match the full toolchain; only the publication mechanism
//! differs, and these cases pin what that mechanism promises.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::sync::atomic::{AtomicU64, Ordering};

use semaprax::project::derive_project_scaffold_v1;

static SERIAL: AtomicU64 = AtomicU64::new(0);

struct Fixture {
    root: PathBuf,
}

impl Fixture {
    fn new(label: &str) -> Self {
        let root = std::env::temp_dir().join(format!(
            "semaprax-standalone-new-{label}-{}-{}",
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

fn read_tree(root: &Path) -> BTreeMap<String, Vec<u8>> {
    fn visit(base: &Path, path: &Path, files: &mut BTreeMap<String, Vec<u8>>) {
        for entry in std::fs::read_dir(path).unwrap() {
            let entry = entry.unwrap();
            let metadata = std::fs::symlink_metadata(entry.path()).unwrap();
            assert!(!metadata.file_type().is_symlink());
            if metadata.is_dir() {
                visit(base, &entry.path(), files);
            } else {
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

fn scaffold_files(name: &str) -> BTreeMap<String, Vec<u8>> {
    derive_project_scaffold_v1(name, "calculator")
        .unwrap()
        .files()
        .iter()
        .map(|file| (file.path().to_owned(), file.bytes().to_vec()))
        .collect()
}

#[test]
fn standalone_new_creates_the_exact_template_and_the_project_works() {
    let fixture = Fixture::new("success");
    let created = cli(&fixture.root, &["new", "first-semaprax"]);
    assert!(created.status.success(), "{}", stderr(&created));
    assert!(created.stderr.is_empty());
    assert_eq!(
        stdout(&created),
        "created calculator project first-semaprax\n"
    );
    let project = fixture.root.join("first-semaprax");
    assert_eq!(read_tree(&project), scaffold_files("first-semaprax"));

    let named = cli(
        &fixture.root,
        &[
            "new",
            "elsewhere",
            "--template",
            "calculator",
            "--name",
            "demo-project",
        ],
    );
    assert!(named.status.success(), "{}", stderr(&named));
    assert_eq!(
        read_tree(&fixture.root.join("elsewhere")),
        scaffold_files("demo-project")
    );

    // The created project is a working project for the same binary.
    let check = cli(&project, &["check", "."]);
    assert!(check.status.success(), "{}", stderr(&check));
    assert!(stdout(&check).starts_with("verified project first-semaprax (sha256:"));
    assert_eq!(
        stdout(&cli(&project, &["test", "."])),
        "project tests passed\n"
    );
    assert_eq!(stdout(&cli(&project, &["run", "."])), "42\n");

    // Nested destinations under an existing parent work; the leaf names the project.
    std::fs::create_dir(fixture.root.join("apps")).unwrap();
    let nested = cli(&fixture.root, &["new", "apps/second"]);
    assert!(nested.status.success(), "{}", stderr(&nested));
    assert_eq!(
        read_tree(&fixture.root.join("apps/second")),
        scaffold_files("second")
    );
}

#[test]
fn standalone_new_refuses_existing_invalid_and_parentless_destinations() {
    let fixture = Fixture::new("rejections");
    std::fs::create_dir(fixture.root.join("existing")).unwrap();
    std::fs::write(fixture.root.join("existing/keep"), b"kept\n").unwrap();
    let existing = cli(&fixture.root, &["new", "existing"]);
    assert_eq!(existing.status.code(), Some(1));
    assert!(existing.stdout.is_empty());
    assert_eq!(
        stderr(&existing),
        "new: cannot create project existing: an entry already exists\n"
    );
    assert_eq!(
        std::fs::read(fixture.root.join("existing/keep")).unwrap(),
        b"kept\n"
    );
    assert_eq!(
        std::fs::read_dir(fixture.root.join("existing"))
            .unwrap()
            .count(),
        1
    );

    std::fs::write(fixture.root.join("file"), b"").unwrap();
    let file = cli(&fixture.root, &["new", "file"]);
    assert_eq!(file.status.code(), Some(1));
    assert!(stderr(&file).contains("an entry already exists"));

    let invalid = cli(&fixture.root, &["new", "Bad_Name"]);
    assert_eq!(invalid.status.code(), Some(2));
    assert_eq!(
        stderr(&invalid),
        "new: project name must match lowercase [a-z][a-z0-9-]* and be at most 64 bytes\n\
hint: run `semaprax new --help` for usage\n"
    );
    assert!(!fixture.root.join("Bad_Name").exists());

    let template = cli(&fixture.root, &["new", "fine", "--template", "web"]);
    assert_eq!(template.status.code(), Some(2));
    assert_eq!(
        stderr(&template),
        "new: unknown new template `web`; expected calculator\nhint: run `semaprax new --help` for usage\n"
    );
    assert!(!fixture.root.join("fine").exists());

    let missing = cli(&fixture.root, &["new"]);
    assert_eq!(missing.status.code(), Some(2));
    assert_eq!(
        stderr(&missing),
        "new: new requires one destination\nhint: run `semaprax new --help` for usage\n"
    );

    let parentless = cli(&fixture.root, &["new", "missing-parent/project"]);
    assert_eq!(parentless.status.code(), Some(1));
    assert!(stderr(&parentless).starts_with("new: cannot inspect new project parent"));
    assert!(!fixture.root.join("missing-parent").exists());

    // Nothing above created an entry besides the two fixtures.
    let mut names: Vec<_> = std::fs::read_dir(&fixture.root)
        .unwrap()
        .map(|entry| entry.unwrap().file_name().into_string().unwrap())
        .collect();
    names.sort();
    assert_eq!(names, ["existing", "file"]);
}

#[test]
fn standalone_new_is_listed_by_help_and_describes_its_grammar() {
    let fixture = Fixture::new("help");
    let scoped = cli(&fixture.root, &["help", "new"]);
    assert!(scoped.status.success());
    assert_eq!(
        stdout(&scoped),
        "Usage:\n  semaprax new <destination> [--name project-name] [--template calculator]\n"
    );
    let guided = cli(&fixture.root, &["--help"]);
    assert!(stdout(&guided).contains("\n  new <destination>"));
    assert_eq!(std::fs::read_dir(&fixture.root).unwrap().count(), 0);
}
