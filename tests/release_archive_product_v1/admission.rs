use super::command;
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::fs::{self, File};
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Duration;

const VERSION: &str = env!("CARGO_PKG_VERSION");
const SMOKE: &[u8] = b"module app;\n\n@id(\"app.main\")\nfn main() -> i64 { 42 }\n";
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
        ("linux", "aarch64") => "aarch64-unknown-linux-gnu",
        ("macos", "aarch64") => "aarch64-apple-darwin",
        ("macos", "x86_64") => "x86_64-apple-darwin",
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

/// The archive README is rendered from the packaged template, not copied from
/// the repository README.
pub(super) fn readme(target: &str) -> Vec<u8> {
    #[cfg(windows)]
    let template = include_str!("../../packaging/archive/README.windows.md");
    #[cfg(not(windows))]
    let template = include_str!("../../packaging/archive/README.unix.md");
    template
        .replace("\r\n", "\n")
        .replace("{{TAG}}", &format!("v{VERSION}"))
        .replace("{{VERSION}}", VERSION)
        .replace("{{TARGET}}", target)
        .into_bytes()
}

/// The fixed manifest fields as the packager writes them before it appends the
/// per-build `runtime` object (see [`manifest_with_runtime`]).
fn manifest(commit: &str, target: &str) -> String {
    format!("{{\n  \"schema\": \"semaprax.release-artifact.v1\",\n  \"version\": \"{VERSION}\",\n  \"commit\": \"{commit}\",\n  \"target\": \"{target}\",\n  \"maturity\": \"beta\",\n  \"binaries\": [\"semaprax\", \"semapraxd\"],\n  \"nonclaims\": [\n    \"production-ready\",\n    \"stable language ABI\",\n    \"stable public protocol\",\n    \"safety-critical suitability\"\n  ]\n}}\n")
}

/// The fixed manifest with a `runtime` record spliced in as the packager does.
#[cfg(test)]
pub(super) fn manifest_with_runtime(commit: &str, target: &str, runtime: &str) -> String {
    let fixed = manifest(commit, target);
    let head = fixed.strip_suffix("\n}\n").unwrap();
    format!("{head},\n  \"runtime\": {runtime}\n}}\n")
}

/// Check the manifest bytes: the fixed fields are byte-exact, and the trailing
/// `runtime` record is a closed object consistent with the target.
fn manifest_check(bytes: &[u8], commit: &str, target: &str) -> Result<(), String> {
    let text = std::str::from_utf8(bytes).map_err(|e| e.to_string())?;
    let fixed = manifest(commit, target);
    let head = fixed
        .strip_suffix("\n}\n")
        .ok_or("manifest template shape")?;
    let tail = text
        .strip_prefix(head)
        .and_then(|rest| rest.strip_prefix(",\n  \"runtime\": "))
        .and_then(|rest| rest.strip_suffix("\n}\n"))
        .ok_or("manifest fixed fields or trailing runtime object mismatch")?;
    // serde_json keeps the last of duplicated keys, so a repeated key would
    // otherwise pass the key-set comparison below; count them in the text.
    for key in [
        "cpu",
        "dynamic_libraries",
        "libc_family",
        "min_libc_version",
        "min_os_version",
        "os",
    ] {
        if tail.matches(&format!("\"{key}\":")).count() != 1 {
            return Err(format!("runtime key {key:?} missing or duplicated"));
        }
    }
    let runtime: serde_json::Value = serde_json::from_str(tail).map_err(|e| e.to_string())?;
    let object = runtime.as_object().ok_or("runtime is not an object")?;
    let mut keys = object.keys().map(String::as_str).collect::<Vec<_>>();
    keys.sort_unstable();
    if keys
        != [
            "cpu",
            "dynamic_libraries",
            "libc_family",
            "min_libc_version",
            "min_os_version",
            "os",
        ]
    {
        return Err("runtime keys mismatch (or duplicated)".into());
    }
    let (os, cpu, family) = match target {
        "x86_64-unknown-linux-gnu" => ("linux", "x86_64", "glibc"),
        "aarch64-unknown-linux-gnu" => ("linux", "aarch64", "glibc"),
        "aarch64-apple-darwin" => ("macos", "aarch64", "libsystem"),
        "x86_64-apple-darwin" => ("macos", "x86_64", "libsystem"),
        "x86_64-pc-windows-msvc" => ("windows", "x86_64", "ucrt"),
        _ => return Err("unknown target".into()),
    };
    if object["os"] != os || object["cpu"] != cpu || object["libc_family"] != family {
        return Err("runtime os/cpu/libc_family inconsistent with target".into());
    }
    let version = |key: &str| object[key].as_str().filter(|v| !v.is_empty()).is_some();
    let valid_version = |key: &str| {
        version(key)
            && object[key]
                .as_str()
                .unwrap()
                .bytes()
                .all(|b| b.is_ascii_digit() || b == b'.')
    };
    let version_rules = if os == "linux" {
        valid_version("min_libc_version") && object["min_os_version"].is_null()
    } else {
        valid_version("min_os_version") && object["min_libc_version"].is_null()
    };
    if !version_rules {
        return Err("runtime min version fields inconsistent with target".into());
    }
    match &object["dynamic_libraries"] {
        serde_json::Value::Array(libraries) => {
            if libraries.len() > 64
                || !libraries
                    .iter()
                    .all(|name| name.as_str().is_some_and(|n| !n.is_empty()))
            {
                return Err("dynamic_libraries entries rejected".into());
            }
        }
        // Windows records null when dumpbin was unavailable (not inspected).
        serde_json::Value::Null if os == "windows" => {}
        _ => return Err("dynamic_libraries must be an array".into()),
    }
    Ok(())
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
        ("smoke/meaning.spx", SMOKE.to_vec()),
        ("LICENSE", include_bytes!("../../LICENSE").to_vec()),
        ("README.md", readme(target)),
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
    let mut manifest_bytes = Vec::new();
    File::open(root.join("release-manifest.json"))
        .map_err(|e| e.to_string())?
        .take(1024 * 1024 + 1)
        .read_to_end(&mut manifest_bytes)
        .map_err(|e| e.to_string())?;
    manifest_check(&manifest_bytes, commit, target)?;
    Ok(pins)
}

fn sha256_file(path: &Path) -> String {
    Sha256::digest(fs::read(path).unwrap())
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
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

    /// Decisive negative control: a damaged copy of the real executable must
    /// not pass the version contract that the genuine one just passed.
    pub(super) fn assert_damaged_executable_rejected(&self, root: &Path) {
        let damaged = root.join("damaged");
        fs::create_dir(&damaged).unwrap();
        let wrong = damaged.join(format!("semaprax{}", std::env::consts::EXE_SUFFIX));
        let mut bytes = fs::read(&self.cli).unwrap();
        assert!(bytes.len() > 4096);
        // Wipe the image header and truncate the tail: no loader can map this.
        bytes[..64].fill(0xff);
        bytes.truncate(bytes.len() / 2);
        fs::write(&wrong, &bytes).unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt as _;
            fs::set_permissions(&wrong, fs::Permissions::from_mode(0o700)).unwrap();
        }
        let expected = format!("semaprax {VERSION} ({})\n", self.commit);
        let outcome = command::attempt(
            Command::new(&wrong).arg("--version").current_dir(&damaged),
            b"",
            &damaged.join("capture"),
            Duration::from_secs(30),
            4096,
            4096,
        );
        if let Ok(output) = outcome {
            assert!(
                !output.status.success() || output.stdout != expected.as_bytes(),
                "damaged executable was accepted: {output:?}"
            );
        }
        assert_ne!(
            sha256_file(&wrong),
            self.pins[&format!("semaprax{}", std::env::consts::EXE_SUFFIX)].1,
            "damaged copy must differ from the pinned executable"
        );
    }

    pub(super) fn verify_versions(&self, root: &Path) {
        for (label, arguments, expected) in [
            ("human", vec!["--version"], format!("semaprax {VERSION} ({})\n", self.commit)),
            ("json", vec!["version", "--json"], format!("{{\"schema\":\"semaprax.version.v1\",\"version\":\"{VERSION}\",\"commit\":\"{}\",\"maturity\":\"beta\",\"rust_min\":\"1.88\"}}\n", self.commit)),
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
