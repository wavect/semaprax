//! Fix hints on the two project-shape rejections a first hand-written
//! `semaprax.toml` most often meets: a Project v1 manifest whose six lines are
//! missing, reordered, or unterminated, and an `entry` that names a module
//! other than the one declaring `main`. Codes and messages are unchanged.

use std::path::{Path, PathBuf};
use std::process::Command;

use semaprax::project::ProjectManifest;

const V1: &str = "schema = \"semaprax.project.v1\"\nname = \"calculator\"\nentry = \"calculator.app\"\nsources = [\"src/app.spx\", \"src/core.spx\", \"src/tests.spx\"]\nweb_exports = [\"calculator.add\"]\ntests = [\"calculator.tests\"]\n";

#[test]
fn v1_shape_rejection_lists_the_six_lines_in_order() {
    for hostile in [
        V1.replace("tests = [\"calculator.tests\"]\n", ""),
        V1.trim_end().to_owned(),
    ] {
        let diagnostics = ProjectManifest::parse(&hostile).unwrap_err();
        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].code, "SPX-J100");
        let help = diagnostics[0]
            .help
            .as_deref()
            .unwrap_or_else(|| panic!("{} carries no fix hint", diagnostics[0]));
        assert!(help.contains("six lines in this order"), "{help}");
        assert!(help.contains("`tests = "), "{help}");
    }
}

#[test]
fn v1_reordered_keys_name_the_expected_key() {
    let hostile = V1.replace(
        "name = \"calculator\"\nentry = \"calculator.app\"\n",
        "entry = \"calculator.app\"\nname = \"calculator\"\n",
    );
    let diagnostics = ProjectManifest::parse(&hostile).unwrap_err();
    assert_eq!(diagnostics[0].code, "SPX-J100");
    assert_eq!(
        diagnostics[0].message,
        "Project v1 manifest expected canonical `name` string assignment"
    );
}

#[test]
fn v1_extra_key_and_valid_manifest_keep_their_behaviour() {
    let hostile = V1.replace(
        "tests = [\"calculator.tests\"]\n",
        "tests = [\"calculator.tests\"]\nversion = \"1.0.0\"\n",
    );
    let diagnostics = ProjectManifest::parse(&hostile).unwrap_err();
    assert_eq!(diagnostics[0].code, "SPX-J100");
    assert!(diagnostics[0].help.is_some());
    assert_eq!(ProjectManifest::parse(V1).unwrap().to_canonical_toml(), V1);
}

#[test]
fn entry_naming_a_module_without_main_explains_the_entry_key() {
    let root = Path::new(env!("CARGO_TARGET_TMPDIR"))
        .join(format!("semaprax-manifest-hints-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    copy_tree(
        &Path::new(env!("CARGO_MANIFEST_DIR")).join("examples/calculator-project"),
        &root,
    );
    let manifest = root.join("semaprax.toml");
    let text = std::fs::read_to_string(&manifest).unwrap();
    std::fs::write(
        &manifest,
        text.replace("entry = \"calculator.app\"", "entry = \"calculator.core\""),
    )
    .unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_semaprax"))
        .arg("check")
        .arg(&manifest)
        .arg("--json")
        .output()
        .unwrap();
    assert!(!output.status.success());
    let stdout = String::from_utf8(output.stdout).unwrap();
    let stderr = String::from_utf8(output.stderr).unwrap();
    let combined = format!("{stdout}{stderr}");
    assert!(combined.contains("\"code\":\"SPX-G172\""), "{combined}");
    assert!(
        combined.contains("workspace scalar provider modules may not declare `main`"),
        "{combined}"
    );
    assert!(
        combined.contains("`entry` in semaprax.toml must name the module that declares `main`"),
        "{combined}"
    );
    std::fs::remove_dir_all(&root).unwrap();
}

#[test]
fn missing_use_line_names_the_import_form() {
    let root = Path::new(env!("CARGO_TARGET_TMPDIR"))
        .join(format!("semaprax-import-hints-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    copy_tree(
        &Path::new(env!("CARGO_MANIFEST_DIR")).join("examples/calculator-project"),
        &root,
    );
    let app = root.join("src").join("app.spx");
    let text = std::fs::read_to_string(&app).unwrap();
    std::fs::write(
        &app,
        text.replace(
            "use function @id(\"calculator.add\") from calculator.core as add;\n",
            "",
        ),
    )
    .unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_semaprax"))
        .arg("check")
        .arg(root.join("semaprax.toml"))
        .output()
        .unwrap();
    assert!(!output.status.success());
    let combined = format!(
        "{}{}",
        String::from_utf8(output.stdout).unwrap(),
        String::from_utf8(output.stderr).unwrap()
    );
    assert!(
        combined.contains("error[SPX-T203]: unknown function `add`"),
        "{combined}"
    );
    assert!(combined.contains("from other.module as add;"), "{combined}");
    std::fs::remove_dir_all(&root).unwrap();
}

fn copy_tree(from: &Path, to: &PathBuf) {
    std::fs::create_dir_all(to).unwrap();
    for entry in std::fs::read_dir(from).unwrap() {
        let entry = entry.unwrap();
        let target = to.join(entry.file_name());
        if entry.file_type().unwrap().is_dir() {
            copy_tree(&entry.path(), &target);
        } else {
            std::fs::copy(entry.path(), target).unwrap();
        }
    }
}
