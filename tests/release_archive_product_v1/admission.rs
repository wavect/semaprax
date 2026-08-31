use super::command;
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::fs::{self, File};
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Duration;

const VERSION: &str = env!("CARGO_PKG_VERSION");
const SMOKE: &[u8] =
    b"module release.smoke;\n\n@id(\"release.smoke.main\")\nfn main() -> i64 { 42 }\n";
type Pins = BTreeMap<String, (u64, String)>;

pub(super) struct Release {
    pub(super) root: PathBuf,
    pub(super) cli: PathBuf,
    pub(super) daemon: PathBuf,
    commit: String,
    target: &'static str,
    pins: Pins,
}

fn native_target() -> &'static str {
    match (std::env::consts::OS, std::env::consts::ARCH) {
        ("linux", "x86_64") => "x86_64-unknown-linux-gnu",
        ("macos", "aarch64") => "aarch64-apple-darwin",
        ("windows", "x86_64") => "x86_64-pc-windows-msvc",
        _ => panic!("selected archive gate requires an admitted native release host"),
    }
}

fn plain(path: &Path, directory: bool) -> Result<(), String> {
    let metadata = fs::symlink_metadata(path).map_err(|error| error.to_string())?;
    if metadata.file_type().is_symlink()
        || (if directory {
            !metadata.is_dir()
        } else {
            !metadata.is_file()
        })
    {
        return Err(format!(
            "not an ordinary {}: {}",
            if directory { "directory" } else { "file" },
            path.display()
        ));
    }
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt as _;
        if metadata.file_attributes() & 0x400 != 0 {
            return Err("reparse point".into());
        }
    }
    Ok(())
}

fn names(root: &Path, maximum: usize) -> Result<Vec<String>, String> {
    let mut names = fs::read_dir(root)
        .map_err(|e| e.to_string())?
        .take(maximum + 1)
        .map(|row| {
            row.map_err(|e| e.to_string())?
                .file_name()
                .into_string()
                .map_err(|_| "non-Unicode name".into())
        })
        .collect::<Result<Vec<_>, String>>()?;
    if names.len() > maximum {
        return Err("archive directory inventory exceeded limit".into());
    }
    names.sort();
    Ok(names)
}

fn manifest(commit: &str, target: &str) -> String {
    format!("{{\n  \"schema\": \"semaprax.release-artifact.v1\",\n  \"version\": \"{VERSION}\",\n  \"commit\": \"{commit}\",\n  \"target\": \"{target}\",\n  \"maturity\": \"pre-alpha\",\n  \"binaries\": [\"semaprax\", \"semapraxd\"],\n  \"nonclaims\": [\n    \"production-ready\",\n    \"stable language ABI\",\n    \"stable public protocol\",\n    \"safety-critical suitability\"\n  ]\n}}\n")
}

fn inspect(root: &Path, commit: &str, target: &str) -> Result<Pins, String> {
    if !root.is_absolute()
        || commit.len() != 40
        || !commit
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
    {
        return Err(
            "absolute root and exactly 40 lowercase hexadecimal commit bytes required".into(),
        );
    }
    plain(root, true)?;
    let cli = format!("semaprax{}", std::env::consts::EXE_SUFFIX);
    let daemon = format!("semapraxd{}", std::env::consts::EXE_SUFFIX);
    let mut top = vec![
        "LICENSE".to_owned(),
        "README.md".into(),
        "release-manifest.json".into(),
        cli.clone(),
        daemon.clone(),
        "smoke".into(),
    ];
    top.sort();
    if names(root, 6)? != top {
        return Err("archive top-level inventory mismatch".into());
    }
    plain(&root.join("smoke"), true)?;
    if names(&root.join("smoke"), 1)? != ["meaning.spx"] {
        return Err("smoke inventory mismatch".into());
    }
    let mut pins = BTreeMap::new();
    for name in [
        cli.as_str(),
        daemon.as_str(),
        "LICENSE",
        "README.md",
        "release-manifest.json",
        "smoke/meaning.spx",
    ] {
        let path = root.join(name);
        plain(&path, false)?;
        let binary = name == cli || name == daemon;
        let maximum = if binary {
            512 * 1024 * 1024
        } else {
            1024 * 1024
        };
        let size = fs::metadata(&path).map_err(|e| e.to_string())?.len();
        if size == 0 || size > maximum {
            return Err(format!("archive file size rejected: {name}"));
        }
        #[cfg(unix)]
        if binary {
            use std::os::unix::fs::PermissionsExt as _;
            if fs::metadata(&path)
                .map_err(|e| e.to_string())?
                .permissions()
                .mode()
                & 0o111
                == 0
            {
                return Err("archive binary is not executable".into());
            }
        }
        let mut reader = File::open(&path)
            .map_err(|e| e.to_string())?
            .take(maximum + 1);
        let mut hash = Sha256::new();
        let mut count = 0u64;
        let mut buffer = [0; 8192];
        loop {
            let read = reader.read(&mut buffer).map_err(|e| e.to_string())?;
            if read == 0 {
                break;
            }
            count += read as u64;
            hash.update(&buffer[..read]);
        }
        if count != size {
            return Err("archive file changed length".into());
        }
        let digest = hash
            .finalize()
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect();
        pins.insert(name.to_owned(), (count, digest));
    }
    for (name, expected) in [
        (
            "release-manifest.json",
            manifest(commit, target).into_bytes(),
        ),
        ("smoke/meaning.spx", SMOKE.to_vec()),
        ("LICENSE", include_bytes!("../../LICENSE").to_vec()),
        ("README.md", include_bytes!("../../README.md").to_vec()),
    ] {
        let mut bytes = Vec::new();
        File::open(root.join(name))
            .map_err(|e| e.to_string())?
            .take(1024 * 1024 + 1)
            .read_to_end(&mut bytes)
            .map_err(|e| e.to_string())?;
        if bytes != expected {
            return Err(format!("archive literal mismatch: {name}"));
        }
    }
    Ok(pins)
}

impl Release {
    pub(super) fn admit() -> Self {
        let root = PathBuf::from(
            std::env::var_os("SEMAPRAX_RELEASE_ROOT").expect("provision SEMAPRAX_RELEASE_ROOT"),
        );
        let commit =
            std::env::var("SEMAPRAX_RELEASE_COMMIT").expect("provision SEMAPRAX_RELEASE_COMMIT");
        let target = native_target();
        let pins = inspect(&root, &commit, target).unwrap();
        let root = root.canonicalize().unwrap();
        assert!(!root.starts_with(
            Path::new(env!("CARGO_MANIFEST_DIR"))
                .canonicalize()
                .unwrap()
        ));
        Self {
            cli: root.join(format!("semaprax{}", std::env::consts::EXE_SUFFIX)),
            daemon: root.join(format!("semapraxd{}", std::env::consts::EXE_SUFFIX)),
            root,
            commit,
            target,
            pins,
        }
    }

    pub(super) fn assert_unchanged(&self) {
        assert_eq!(
            inspect(&self.root, &self.commit, self.target).unwrap(),
            self.pins
        );
    }

    pub(super) fn verify_versions(&self, root: &Path) {
        for (label, arguments, expected) in [
            ("human", vec!["--version"], format!("semaprax {VERSION} ({})\n", self.commit)),
            ("json", vec!["version", "--json"], format!("{{\"schema\":\"semaprax.version.v1\",\"version\":\"{VERSION}\",\"commit\":\"{}\",\"maturity\":\"pre-alpha\",\"rust_min\":\"1.88\"}}\n", self.commit)),
        ] {
            let output = command::run(Command::new(&self.cli).args(arguments).current_dir(root), b"",
                &root.join(format!("version-{label}")), Duration::from_secs(30), 4096, 4096);
            assert!(output.status.success(), "{output:?}");
            assert_eq!(output.stdout, expected.as_bytes());
            assert!(output.stderr.is_empty());
        }
    }
}

#[path = "admission/tests.rs"]
mod tests;
