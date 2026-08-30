//! Cargo artifact selection with literal protocol streams and inert file witnesses.
//! No compiler or selected executable is launched by these tests.
#[path = "support/full_toolchain/artifact.rs"]
mod artifact;

use std::fs;
use std::io::Write as _;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

static SERIAL: AtomicU64 = AtomicU64::new(0);
#[cfg(unix)]
const EMITTED_DIR: &str = "emitted λ with spaces\t\n";
#[cfg(not(unix))]
const EMITTED_DIR: &str = "emitted λ with spaces";

struct Fixture {
    root: PathBuf,
    manifest: PathBuf,
    selected: PathBuf,
    stale: PathBuf,
}

impl Fixture {
    fn new() -> Self {
        let root = std::env::temp_dir().join(format!(
            "semaprax-artifact-selection-{}-{}-{}",
            std::process::id(),
            SERIAL.fetch_add(1, Ordering::Relaxed),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir(&root).unwrap();
        let root = root.canonicalize().unwrap();
        for name in ["target", EMITTED_DIR] {
            fs::create_dir(root.join(name)).unwrap();
        }
        fs::create_dir(root.join("target/debug")).unwrap();
        let manifest = root.join("Cargo.toml");
        let selected = root.join(EMITTED_DIR).join("semaprax-full");
        let stale = root.join("target/debug/semaprax-full");
        for (path, bytes) in [
            (&manifest, b"manifest\n".as_slice()),
            (&selected, b"fresh artifact\n"),
            (&stale, b"stale guessed artifact\n"),
        ] {
            fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(path)
                .unwrap()
                .write_all(bytes)
                .unwrap();
        }
        Self {
            root,
            manifest,
            selected,
            stale,
        }
    }

    fn row(&self) -> serde_json::Value {
        serde_json::json!({
            "reason": "compiler-artifact", "manifest_path": self.manifest,
            "target": {"name": "semaprax-full", "kind": ["bin"]},
            "profile": {"test": false}, "fresh": false, "executable": self.selected
        })
    }

    fn cleanup(self) {
        // Validate the whole fixed tree before the first nonrecursive removal.
        for (relative, expected) in [
            ("", vec!["Cargo.toml", EMITTED_DIR, "target"]),
            ("target", vec!["debug"]),
            ("target/debug", vec!["semaprax-full"]),
            (EMITTED_DIR, vec!["semaprax-full"]),
        ] {
            let path = self.root.join(relative);
            let metadata = fs::symlink_metadata(&path).unwrap();
            assert!(metadata.is_dir() && !metadata.file_type().is_symlink());
            assert_no_reparse(&metadata);
            let mut names = fs::read_dir(path)
                .unwrap()
                .map(|row| row.unwrap().file_name().into_string().unwrap())
                .collect::<Vec<_>>();
            names.sort();
            assert_eq!(names, expected);
        }
        for (path, bytes) in [
            (&self.manifest, b"manifest\n".as_slice()),
            (&self.selected, b"fresh artifact\n"),
            (&self.stale, b"stale guessed artifact\n"),
        ] {
            let metadata = fs::symlink_metadata(path).unwrap();
            assert!(metadata.is_file() && !metadata.file_type().is_symlink());
            assert_no_reparse(&metadata);
            assert_eq!(fs::read(path).unwrap(), bytes);
        }
        for path in [&self.manifest, &self.selected, &self.stale] {
            fs::remove_file(path).unwrap();
        }
        for relative in ["target/debug", "target", EMITTED_DIR] {
            fs::remove_dir(self.root.join(relative)).unwrap();
        }
        fs::remove_dir(self.root).unwrap();
    }
}

fn assert_no_reparse(metadata: &fs::Metadata) {
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        assert_eq!(metadata.file_attributes() & 0x400, 0);
    }
    #[cfg(not(windows))]
    let _ = metadata;
}

fn stream(rows: &[serde_json::Value]) -> Vec<u8> {
    let mut result = String::new();
    for row in rows {
        result.push_str(&row.to_string());
        result.push('\n');
    }
    result.into_bytes()
}

fn finished() -> serde_json::Value {
    serde_json::json!({"reason":"build-finished","success":true})
}

#[test]
fn selects_emitted_location_not_stale_guess_and_accepts_cargo_freshness() {
    let fixture = Fixture::new();
    for fresh in [false, true] {
        let mut row = fixture.row();
        row["fresh"] = fresh.into();
        let mut bytes = b"running 1 test\n\t \nnon-JSON diagnostic\n".to_vec();
        bytes.extend_from_slice(b" \t");
        bytes.extend(stream(&[
            serde_json::json!({"reason":"build-script-executed"}),
            row,
            finished(),
        ]));
        assert_eq!(
            artifact::select_artifact(&bytes, &fixture.manifest).unwrap(),
            fixture.selected
        );
        assert_ne!(fixture.selected, fixture.stale);
    }
    fixture.cleanup();
}

#[test]
fn mismatched_subjects_never_fall_back_to_stale_binary() {
    let fixture = Fixture::new();
    let mut variants = Vec::new();
    let mut row = fixture.row();
    row["manifest_path"] = serde_json::json!(fixture.selected);
    variants.push(row);
    let mut row = fixture.row();
    row["manifest_path"] = "Cargo.toml".into();
    variants.push(row);
    let mut row = fixture.row();
    row["target"]["name"] = "semaprax".into();
    variants.push(row);
    let mut row = fixture.row();
    row["target"]["kind"] = serde_json::json!(["lib"]);
    variants.push(row);
    let mut row = fixture.row();
    row["target"]["kind"] = serde_json::json!(["bin", "example"]);
    variants.push(row);
    let mut row = fixture.row();
    row["profile"]["test"] = true.into();
    variants.push(row);
    for row in variants {
        assert!(
            artifact::select_artifact(&stream(&[row.clone(), finished()]), &fixture.manifest)
                .is_err()
        );
        assert_eq!(
            artifact::select_artifact(
                &stream(&[row, fixture.row(), finished()]),
                &fixture.manifest
            )
            .unwrap(),
            fixture.selected
        );
    }
    fixture.cleanup();
}

#[test]
fn rejects_invalid_executable_witnesses_without_fallback() {
    let fixture = Fixture::new();
    for executable in [
        serde_json::Value::Null,
        serde_json::json!(""),
        serde_json::json!("relative/bin"),
        serde_json::json!(fixture.root.join("missing")),
        serde_json::json!(fixture.root),
    ] {
        let mut row = fixture.row();
        row["executable"] = executable;
        assert!(artifact::select_artifact(&stream(&[row, finished()]), &fixture.manifest).is_err());
    }
    let mut missing = fixture.row();
    missing.as_object_mut().unwrap().remove("executable");
    assert!(artifact::select_artifact(&stream(&[missing, finished()]), &fixture.manifest).is_err());
    fixture.cleanup();
}

#[test]
fn requires_one_successful_finish_and_one_matching_artifact() {
    let fixture = Fixture::new();
    for rows in [
        vec![],
        vec![finished()],
        vec![fixture.row()],
        vec![fixture.row(), fixture.row(), finished()],
        vec![fixture.row(), finished(), finished()],
        vec![
            fixture.row(),
            serde_json::json!({"reason":"build-finished","success":false}),
        ],
        vec![
            fixture.row(),
            serde_json::json!({"reason":"build-finished"}),
        ],
        vec![
            fixture.row(),
            finished(),
            serde_json::json!({"reason":"unknown"}),
        ],
        vec![finished(), fixture.row()],
    ] {
        assert!(artifact::select_artifact(&stream(&rows), &fixture.manifest).is_err());
    }
    for malformed in [
        "{",
        " \t{bad json}\n",
        "{\"reason\":\"compiler-artifact\"\n",
    ] {
        let mut bytes = malformed.as_bytes().to_vec();
        bytes.extend(stream(&[fixture.row(), finished()]));
        assert!(artifact::select_artifact(&bytes, &fixture.manifest).is_err());
    }
    fixture.cleanup();
}
