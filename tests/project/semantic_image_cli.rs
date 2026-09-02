//! CLI boundary regressions; image bytes never grant source authority.

use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::sync::atomic::{AtomicU64, Ordering};

use semaprax::project::{self, ProjectSemanticImage, MAX_SEMANTIC_IMAGE_BYTES};

static SERIAL: AtomicU64 = AtomicU64::new(0);

struct Fixture(PathBuf);

impl Fixture {
    fn new() -> Self {
        let root = std::env::temp_dir().join(format!(
            "semaprax-image-cli-{}-{}",
            std::process::id(),
            SERIAL.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir(&root).unwrap();
        let root = root.canonicalize().unwrap();
        std::fs::create_dir(root.join("src")).unwrap();
        let source = Path::new(env!("CARGO_MANIFEST_DIR")).join("examples/calculator-project");
        for path in [
            "semaprax.toml",
            "src/app.spx",
            "src/core.spx",
            "src/tests.spx",
        ] {
            std::fs::copy(source.join(path), root.join(path)).unwrap();
        }
        Self(root)
    }

    fn cli(&self, arguments: &[&str]) -> Output {
        Command::new(env!("CARGO_BIN_EXE_semaprax"))
            .current_dir(&self.0)
            .args(arguments)
            .output()
            .unwrap()
    }

    fn image(&self) -> Vec<u8> {
        let output = self.cli(&["project-image", "semaprax.toml"]);
        assert!(output.status.success(), "{:?}", output);
        assert!(output.stderr.is_empty());
        output.stdout
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn assert_domain_failure(output: Output, code: &str) {
    assert_eq!(output.status.code(), Some(1), "{output:?}");
    assert!(output.stdout.is_empty(), "{output:?}");
    assert!(
        String::from_utf8_lossy(&output.stderr).contains(code),
        "{output:?}"
    );
}

#[test]
fn image_commands_require_exact_positional_operands_before_loading_inputs() {
    let fixture = Fixture::new();
    for (command, required) in [
        ("project-image", 1),
        ("project-image-verify", 2),
        ("project-symbol", 2),
    ] {
        for count in [0, required - 1, required + 1] {
            let mut arguments = vec![command];
            arguments.extend(std::iter::repeat_n("missing", count));
            let output = fixture.cli(&arguments);
            assert_eq!(output.status.code(), Some(2), "{output:?}");
            assert!(output.stdout.is_empty());
            assert!(String::from_utf8_lossy(&output.stderr).contains("requires exactly"));
        }
        let mut arguments = vec![command, "--unknown"];
        arguments.extend(std::iter::repeat_n("missing", required - 1));
        let output = fixture.cli(&arguments);
        assert_eq!(output.status.code(), Some(2), "{output:?}");
        assert!(output.stdout.is_empty());
    }
}

#[test]
fn image_stdout_is_exact_rebuildable_api_bytes_and_creates_no_cache() {
    let fixture = Fixture::new();
    let first = fixture.image();
    assert_eq!(first, fixture.image());
    assert!(first.ends_with(b"\n"));
    assert!(!first.ends_with(b"\n\n"));
    let expected =
        project::with_authenticated_project(&fixture.0.join("semaprax.toml"), |snapshot| {
            let image = ProjectSemanticImage::derive(
                snapshot.retain_revision(),
                snapshot.project_revision(),
            )?;
            Ok(image.to_json().as_bytes().to_vec())
        })
        .unwrap();
    assert_eq!(first, expected);
    let mut files = std::fs::read_dir(&fixture.0)
        .unwrap()
        .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
        .collect::<Vec<_>>();
    files.sort();
    assert_eq!(files, ["semaprax.toml", "src"]);
}

#[test]
fn exact_replay_receipt_and_symbol_use_the_current_authenticated_revision() {
    let fixture = Fixture::new();
    let image = fixture.image();
    let image_json: serde_json::Value = serde_json::from_slice(&image).unwrap();
    std::fs::write(fixture.0.join("image.json"), &image).unwrap();
    let output = fixture.cli(&["project-image-verify", "semaprax.toml", "image.json"]);
    assert!(output.status.success(), "{output:?}");
    assert!(output.stderr.is_empty());
    let receipt: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(
        receipt["schema"],
        "semaprax.semantic-workspace-image-receipt.v1"
    );
    let expected_digest =
        project::with_authenticated_project(&fixture.0.join("semaprax.toml"), |snapshot| {
            let image = ProjectSemanticImage::derive(
                snapshot.retain_revision(),
                snapshot.project_revision(),
            )?;
            Ok(image.image_digest().to_owned())
        })
        .unwrap();
    assert_eq!(receipt["image_digest"], expected_digest);
    assert_eq!(receipt["project_revision"], image_json["project_revision"]);
    assert_eq!(receipt["verified"], true);
    assert_eq!(receipt["source_authority"], false);

    let output = fixture.cli(&["project-symbol", "semaprax.toml", "calculator.add"]);
    assert!(output.status.success(), "{output:?}");
    let expected =
        project::with_authenticated_project(&fixture.0.join("semaprax.toml"), |snapshot| {
            let image = ProjectSemanticImage::derive(
                snapshot.retain_revision(),
                snapshot.project_revision(),
            )?;
            Ok(format!(
                "{}\n",
                image.symbol(image.image_digest(), "calculator.add")?
            ))
        })
        .unwrap();
    assert_eq!(output.stdout, expected.as_bytes());
    assert_domain_failure(
        fixture.cli(&["project-symbol", "semaprax.toml", "calculator.missing"]),
        "SPX-G219",
    );
}

#[test]
fn replay_rejects_old_source_revision_without_emitting_a_receipt() {
    let fixture = Fixture::new();
    std::fs::write(fixture.0.join("image.json"), fixture.image()).unwrap();
    let path = fixture.0.join("src/core.spx");
    let original = std::fs::read_to_string(&path).unwrap();
    let changed = original.replace("left + right", "left - right");
    assert_ne!(original, changed);
    std::fs::write(&path, &changed).unwrap();
    assert_domain_failure(
        fixture.cli(&["project-image-verify", "semaprax.toml", "image.json"]),
        "SPX-G221",
    );
    assert_eq!(std::fs::read_to_string(&path).unwrap(), changed);
}

#[test]
fn replay_rejects_noncanonical_or_corrupted_image_bytes() {
    let fixture = Fixture::new();
    let image = fixture.image();
    let mut extra_lf = image.clone();
    extra_lf.push(b'\n');
    for bytes in [
        extra_lf,
        image[..image.len() - 1].to_vec(),
        b"{}\n".to_vec(),
    ] {
        std::fs::write(fixture.0.join("image.json"), &bytes).unwrap();
        assert_domain_failure(
            fixture.cli(&["project-image-verify", "semaprax.toml", "image.json"]),
            "SPX-G221",
        );
    }
}

#[test]
fn replay_inputs_are_bounded_regular_files() {
    let fixture = Fixture::new();
    assert_domain_failure(
        fixture.cli(&["project-image-verify", "semaprax.toml", "src"]),
        "SPX-G219",
    );
    assert_domain_failure(
        fixture.cli(&["project-image-verify", "semaprax.toml", "missing.json"]),
        "SPX-G219",
    );
    let oversized = std::fs::File::create(fixture.0.join("oversized.json")).unwrap();
    oversized
        .set_len(MAX_SEMANTIC_IMAGE_BYTES as u64 + 1)
        .unwrap();
    drop(oversized);
    assert_domain_failure(
        fixture.cli(&["project-image-verify", "semaprax.toml", "oversized.json"]),
        "SPX-G220",
    );
}

#[cfg(unix)]
#[test]
fn replay_rejects_symlink_inputs() {
    let fixture = Fixture::new();
    std::fs::write(fixture.0.join("image.json"), fixture.image()).unwrap();
    std::os::unix::fs::symlink("image.json", fixture.0.join("image-link.json")).unwrap();
    assert_domain_failure(
        fixture.cli(&["project-image-verify", "semaprax.toml", "image-link.json"]),
        "SPX-G219",
    );
}

// rustix deliberately does not expose mkfifoat on Apple platforms.
#[cfg(target_os = "linux")]
#[test]
fn replay_rejects_fifo_inputs_without_waiting_for_a_writer() {
    use rustix::fs::{mkfifoat, Mode, CWD};
    let fixture = Fixture::new();
    mkfifoat(
        CWD,
        fixture.0.join("image-pipe.json"),
        Mode::RUSR | Mode::WUSR,
    )
    .unwrap();
    assert_domain_failure(
        fixture.cli(&["project-image-verify", "semaprax.toml", "image-pipe.json"]),
        "SPX-G219",
    );
}
