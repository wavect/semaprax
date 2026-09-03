//! Authored, unrun real Project/recipe evidence. Native coverage stops at the
//! rejecting publisher handoff; it does not compile or publish a native SDK.
#![cfg(any(target_os = "linux", target_os = "macos", target_os = "windows"))]

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use semaprax::diagnostic::Diagnostic;
use semaprax::project::{
    with_authenticated_project, FlatOwnedRecordApiDescriptor, ProjectNativeRustPackageMode,
    ProjectNpmBuild, MAX_PROJECT_NPM_BUILD_BYTES,
};

static SERIAL: AtomicU64 = AtomicU64::new(0);
const SENTINEL: &str = "test publisher intentionally refuses publication";
const MANIFEST: &str = "schema = \"semaprax.project.v9\"\nname = \"recipe-identity\"\nversion = \"0.1.0\"\nprofile = \"flat-owned-record-api.v1\"\nentry = \"fixture.app\"\nsources = [\"src/app.spx\", \"src/left.spx\", \"src/right.spx\", \"src/tests.spx\"]\nweb_exports = [\"left.payload\", \"right.payload\"]\ntests = [\"fixture.tests\"]\n";

struct Fixture {
    root: PathBuf,
    files: Vec<(PathBuf, Vec<u8>)>,
}

impl std::ops::Deref for Fixture {
    type Target = Path;
    fn deref(&self) -> &Path {
        &self.root
    }
}

impl Fixture {
    fn cleanup(&self) -> std::io::Result<()> {
        fn plain(path: &Path, directory: bool) -> std::io::Result<()> {
            let metadata = fs::symlink_metadata(path)?;
            let rejected = metadata.file_type().is_symlink()
                || metadata.is_dir() != directory
                || (!directory && !metadata.is_file());
            #[cfg(windows)]
            let rejected = {
                use std::os::windows::fs::MetadataExt as _;
                rejected || metadata.file_attributes() & 0x400 != 0
            };
            if rejected {
                return Err(std::io::Error::other("fixture type drift"));
            }
            Ok(())
        }
        fn names(path: &Path) -> std::io::Result<Vec<std::ffi::OsString>> {
            let mut values = fs::read_dir(path)?
                .map(|row| row.map(|row| row.file_name()))
                .collect::<std::io::Result<Vec<_>>>()?;
            values.sort();
            Ok(values)
        }
        plain(&self.root, true)?;
        plain(&self.root.join("src"), true)?;
        if names(&self.root)? != ["semaprax.toml", "src"].map(std::ffi::OsString::from)
            || names(&self.root.join("src"))?
                != ["app.spx", "left.spx", "right.spx", "tests.spx"].map(std::ffi::OsString::from)
        {
            return Err(std::io::Error::other("fixture inventory drift"));
        }
        for (path, bytes) in &self.files {
            plain(path, false)?;
            if fs::read(path)? != *bytes {
                return Err(std::io::Error::other("fixture content drift"));
            }
        }
        // All exact paths/types/bytes were checked before the first removal.
        for (path, _) in &self.files {
            fs::remove_file(path)?;
        }
        fs::remove_dir(self.root.join("src"))?;
        fs::remove_dir(&self.root)
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = self.cleanup();
    }
}

fn module(side: &str, name: &str, controls: bool) -> String {
    // These are source escapes, deliberately not JSON's \u0008 spelling.
    let suffix = if controls {
        r"\u{8}\u{c}\u{7f}\u{85}"
    } else {
        ""
    };
    let count = if side == "left" { 11 } else { 22 };
    let bytes = format!("    @id(\"{side}.bytes{suffix}\") bytes: Bytes,\n");
    let scalar = format!("    @id(\"{side}.count{suffix}\") count: i64,\n");
    let fields = if side == "left" {
        format!("{bytes}{scalar}")
    } else {
        format!("{scalar}{bytes}")
    };
    let values = if side == "left" {
        format!("bytes: bytes_copy(input), count: {count}")
    } else {
        format!("count: {count}, bytes: bytes_copy(input)")
    };
    format!(
        r#"module fixture.{side};
@id("{side}.Payload{suffix}") record {name} {{
{fields}
}}
@id("{side}.payload") fn payload(input: borrow Slice<u8>) -> {name} {{
    {name} {{ {values} }}
}}
"#
    )
}

fn fixture(left_name: &str, right_name: &str, controls: bool) -> Fixture {
    let root = std::env::temp_dir().join(format!(
        "semaprax-v9-recipe-identity-{}-{}-{}",
        std::process::id(),
        SERIAL.fetch_add(1, Ordering::Relaxed),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    fs::create_dir(&root).unwrap();
    let root = root.canonicalize().unwrap();
    let mut fixture = Fixture {
        root: root.clone(),
        files: Vec::new(),
    };
    fs::create_dir(root.join("src")).unwrap();
    fs::write(root.join("semaprax.toml"), MANIFEST).unwrap();
    fixture
        .files
        .push((root.join("semaprax.toml"), MANIFEST.as_bytes().to_vec()));
    for (path, source) in [
        (
            "app.spx",
            "module fixture.app;\n@id(\"fixture.main\") fn main() -> i64 { 0 }\n".to_owned(),
        ),
        ("left.spx", module("left", left_name, controls)),
        ("right.spx", module("right", right_name, controls)),
        (
            "tests.spx",
            "module fixture.tests;\n@id(\"fixture.tests.main\") fn main() -> i64 { 0 }\n"
                .to_owned(),
        ),
    ] {
        let parsed = semaprax::parse(&source, Path::new(path)).unwrap();
        let canonical = semaprax::format::canonical(&parsed);
        assert_eq!(
            semaprax::format::canonical(&semaprax::parse(&canonical, Path::new(path)).unwrap()),
            canonical
        );
        let path = root.join("src").join(path);
        fs::write(&path, &canonical).unwrap();
        fixture.files.push((path, canonical.into_bytes()));
    }
    fixture
}

fn artifact(envelope: &serde_json::Value, name: &str) -> Vec<u8> {
    let rows = envelope["artifacts"].as_array().unwrap();
    let selected = rows
        .iter()
        .filter(|row| row["path"].as_str() == Some(name))
        .collect::<Vec<_>>();
    assert_eq!(selected.len(), 1);
    let hex = selected[0]["hex"].as_str().unwrap();
    assert_eq!(hex.len() % 2, 0);
    (0..hex.len())
        .step_by(2)
        .map(|index| u8::from_str_radix(&hex[index..index + 2], 16).unwrap())
        .collect()
}

fn observe(root: &Path) -> FlatOwnedRecordApiDescriptor {
    with_authenticated_project(&root.join("semaprax.toml"), |snapshot| {
        snapshot.check()?;
        let descriptor = snapshot.flat_owned_record_api_descriptor()?;
        assert_eq!(descriptor.exports().len(), 2);
        let npm = snapshot.build_npm_inline(MAX_PROJECT_NPM_BUILD_BYTES)?;
        npm.verify().unwrap();
        ProjectNpmBuild::inspect_envelope(npm.envelope(), MAX_PROJECT_NPM_BUILD_BYTES).unwrap();
        let envelope: serde_json::Value = serde_json::from_str(npm.envelope()).unwrap();
        assert_eq!(envelope["schema"], "semaprax.project-npm-build.v8");
        let metadata: serde_json::Value =
            serde_json::from_slice(&artifact(&envelope, "semaprax.api.json")).unwrap();
        assert_eq!(
            metadata["descriptor"].as_str().unwrap().as_bytes(),
            descriptor.canonical_bytes()
        );
        let declarations =
            String::from_utf8(artifact(&envelope, "semaprax.bindings.d.ts")).unwrap();
        for export in descriptor.exports() {
            assert!(declarations.contains(&format!("interface {}", export.record_host_name())));
        }
        let output = root.join("not-published");
        let mut reached = false;
        let failure = snapshot
            .build_rust_with(&output, |plan, supplied_output| {
                reached = true;
                assert_eq!(supplied_output, output);
                assert_eq!(plan.mode(), ProjectNativeRustPackageMode::FlatOwnedRecord);
                assert_eq!(plan.descriptor(), descriptor.canonical_bytes());
                assert_eq!(plan.descriptor_digest(), descriptor.digest());
                assert_eq!(plan.selected(), ["left.payload", "right.payload"]);
                assert!(!plan.provider().is_empty());
                Err(vec![Diagnostic::io("SPX-J114", SENTINEL)])
            })
            .unwrap_err();
        assert!(
            reached,
            "native semantic replay must reach the injected handoff"
        );
        assert_eq!(failure.len(), 1);
        assert_eq!(failure[0].code, "SPX-J114");
        assert_eq!(failure[0].message, SENTINEL);
        assert!(!output.exists());
        Ok(descriptor)
    })
    .unwrap()
}

#[test]
fn distinct_module_records_with_same_display_names_replay_on_both_routes() {
    let descriptor = observe(&fixture("Payload", "Payload", false));
    let left = &descriptor.exports()[0];
    let right = &descriptor.exports()[1];
    assert_eq!(left.record_source_name(), "Payload");
    assert_eq!(right.record_source_name(), "Payload");
    assert_ne!(left.record_id(), right.record_id());
    assert_ne!(left.record_host_name(), right.record_host_name());
    assert_eq!(left.fields()[0].source_name(), "bytes");
    assert_eq!(right.fields()[1].source_name(), "bytes");
    for left in left.fields() {
        let right = right
            .fields()
            .iter()
            .find(|field| field.source_name() == left.source_name())
            .unwrap();
        assert_eq!(left.source_name(), right.source_name());
        assert_ne!(left.stable_id(), right.stable_id());
        assert_ne!(left.host_name(), right.host_name());
        assert_ne!(left.ordinal(), right.ordinal());
    }
    let renamed = observe(&fixture("Payload", "RenamedPayload", false));
    for (before, after) in descriptor.exports().iter().zip(renamed.exports()) {
        assert_eq!(before.stable_id(), after.stable_id());
        assert_eq!(before.record_id(), after.record_id());
        assert_eq!(before.record_host_name(), after.record_host_name());
        assert_eq!(before.rust_method_name(), after.rust_method_name());
    }
    assert_eq!(renamed.exports()[1].record_source_name(), "RenamedPayload");
}

#[test]
fn duplicate_display_names_and_control_identities_combine_without_reinterpretation() {
    let descriptor = observe(&fixture("Payload", "Payload", true));
    let left = &descriptor.exports()[0];
    let right = &descriptor.exports()[1];
    assert_eq!(left.record_source_name(), right.record_source_name());
    assert_ne!(left.record_host_name(), right.record_host_name());
    for export in descriptor.exports() {
        assert!(export
            .record_id()
            .as_str()
            .ends_with("\u{8}\u{c}\u{7f}\u{85}"));
    }
    assert_eq!(descriptor.carrier_plans()[0].owned_field_ordinal, 0);
    assert_eq!(descriptor.carrier_plans()[1].owned_field_ordinal, 1);
}

#[test]
fn unique_display_names_with_control_identities_replay_on_both_routes() {
    let descriptor = observe(&fixture("LeftPayload", "RightPayload", true));
    for export in descriptor.exports() {
        assert!(export
            .record_id()
            .as_str()
            .ends_with("\u{8}\u{c}\u{7f}\u{85}"));
        assert!(export.record_host_name().ends_with("080c7fc285"));
        for field in export.fields() {
            assert!(field
                .stable_id()
                .as_str()
                .ends_with("\u{8}\u{c}\u{7f}\u{85}"));
        }
    }
}
