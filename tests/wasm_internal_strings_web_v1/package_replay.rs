//! Independent file/manifest oracle; never calls a package renderer/replayer.
use std::collections::BTreeMap;
use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use semaprax::diagnostic::quote_json;
use sha2::{Digest, Sha256};

pub(super) const INVENTORY: [&str; 8] = [
    "app.wasm",
    "semaprax.js",
    "semaprax.d.ts",
    "semaprax.internal-strings.json",
    "semaprax.manifest.json",
    "package.json",
    "index.html",
    "app.js",
];
pub(super) type Files = BTreeMap<String, Vec<u8>>;
static SERIAL: AtomicU64 = AtomicU64::new(0);

pub(super) fn digest(bytes: &[u8]) -> String {
    format!(
        "sha256:{}",
        Sha256::digest(bytes)
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>()
    )
}

fn regular(path: &Path) -> fs::Metadata {
    let metadata = fs::symlink_metadata(path).unwrap();
    assert!(!metadata.file_type().is_symlink());
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        assert_eq!(metadata.file_attributes() & 0x400, 0);
    }
    metadata
}

pub(super) fn reopen(path: &Path) -> Files {
    let entries = fs::read_dir(path)
        .unwrap()
        .collect::<Result<Vec<_>, _>>()
        .unwrap();
    assert_eq!(entries.len(), INVENTORY.len());
    let mut files = Files::new();
    let mut remaining = 32 * 1024 * 1024usize;
    for entry in entries {
        let name = entry.file_name().into_string().unwrap();
        assert!(INVENTORY.contains(&name.as_str()));
        let metadata = regular(&entry.path());
        assert!(metadata.is_file());
        let length = usize::try_from(metadata.len()).unwrap();
        assert!(length <= remaining);
        if name == "app.wasm" {
            assert!(length <= 16 * 1024 * 1024);
        }
        if name == "semaprax.internal-strings.json" {
            assert!(length <= 1024 * 1024);
        }
        let mut bytes = Vec::new();
        fs::File::open(entry.path())
            .unwrap()
            .take(length as u64 + 1)
            .read_to_end(&mut bytes)
            .unwrap();
        assert_eq!(bytes.len(), length);
        remaining -= length;
        assert!(files.insert(name, bytes).is_none());
    }
    files
}

pub(super) fn replay(files: &Files, source: &str) -> Result<(), String> {
    let mut expected_names = INVENTORY.to_vec();
    expected_names.sort_unstable();
    if files.keys().map(String::as_str).collect::<Vec<_>>() != expected_names {
        return Err("inventory".into());
    }
    let program = semaprax::check(source, "fixture.spx").map_err(|_| "source")?;
    let rows = INVENTORY
        .iter()
        .filter(|name| **name != "semaprax.manifest.json")
        .map(|name| {
            let bytes = &files[*name];
            format!(
                "{{\"path\":{},\"bytes\":{},\"sha256\":{}}}",
                quote_json(name),
                bytes.len(),
                quote_json(&digest(bytes))
            )
        })
        .collect::<Vec<_>>()
        .join(",");
    let expected = format!("{{\"schema\":\"semaprax.web-internal-strings.v1\",\"module\":{},\"source_digest\":{},\"graph_revision\":{},\"compiler_schema\":\"semaprax.wasm-internal-strings.v1\",\"runtime_schema\":\"semaprax.wasm-internal-strings.runtime.v1\",\"capabilities\":[],\"artifacts\":[{rows}]}}\n", quote_json(&program.module), quote_json(&digest(source.as_bytes())), quote_json(&semaprax::graph::revision(&program)));
    if files["semaprax.manifest.json"] != expected.as_bytes() {
        return Err("canonical manifest".into());
    }
    Ok(())
}

pub(super) struct Fixture {
    pub root: PathBuf,
    files: Vec<String>,
    packages: Vec<String>,
}
impl Fixture {
    pub fn new(label: &str) -> Self {
        let root = std::env::temp_dir().join(format!(
            "semaprax-string-web-{label}-{}-{}",
            std::process::id(),
            SERIAL.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&root).unwrap();
        Self {
            root: root.canonicalize().unwrap(),
            files: Vec::new(),
            packages: Vec::new(),
        }
    }
    pub fn write(&mut self, name: &str, bytes: impl AsRef<[u8]>) -> PathBuf {
        assert!(name
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || b"-._".contains(&byte)));
        assert!(!matches!(name, "" | "." | ".."));
        if !self.files.iter().any(|file| file == name) {
            self.files.push(name.to_owned());
        }
        let path = self.root.join(name);
        fs::write(&path, bytes).unwrap();
        path
    }
    pub fn package(&mut self, name: &str) -> PathBuf {
        assert!(name
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-'));
        assert!(!name.is_empty());
        self.packages.push(name.to_owned());
        self.root.join(name)
    }
    pub fn cleanup(self) {
        // Validate the whole bounded tree before removing any successful fixture.
        // Failed assertions deliberately retain everything for investigation.
        let entries = fs::read_dir(&self.root)
            .unwrap()
            .collect::<Result<Vec<_>, _>>()
            .unwrap();
        assert_eq!(entries.len(), self.files.len() + self.packages.len());
        for entry in entries {
            let name = entry.file_name().into_string().unwrap();
            let metadata = regular(&entry.path());
            if self.files.contains(&name) {
                assert!(metadata.is_file());
            } else {
                assert!(self.packages.contains(&name));
                assert!(metadata.is_dir());
                reopen(&entry.path());
            }
        }
        for name in self.packages {
            let path = self.root.join(name);
            for file in INVENTORY {
                fs::remove_file(path.join(file)).unwrap();
            }
            fs::remove_dir(path).unwrap();
        }
        for name in self.files {
            fs::remove_file(self.root.join(name)).unwrap();
        }
        fs::remove_dir(self.root).unwrap();
    }
}
