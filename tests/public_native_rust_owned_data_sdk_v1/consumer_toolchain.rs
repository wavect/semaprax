//! Test-only, opt-in exact compiler pair for the packaged external consumer.
use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::process::Command;

const CARGO_OVERRIDE: &str = "SEMAPRAX_NATIVE_RUST_CONSUMER_CARGO";
const RUSTC_OVERRIDE: &str = "SEMAPRAX_NATIVE_RUST_CONSUMER_RUSTC";

pub(super) struct ConsumerToolchain {
    exact: Option<ExactToolchain>,
}
struct ExactToolchain {
    cargo: PathBuf,
    rustc: PathBuf,
    host: String,
}

impl ConsumerToolchain {
    pub(super) fn from_environment() -> Result<Self, String> {
        let pair = validate_pair(
            std::env::var_os(CARGO_OVERRIDE),
            std::env::var_os(RUSTC_OVERRIDE),
        )?;
        let Some((cargo, rustc)) = pair else {
            return Ok(Self { exact: None });
        };
        let cargo_observation = observe(&cargo)?;
        let rustc_observation = observe(&rustc)?;
        eprintln!(
            "exact packaged consumer Cargo {}:\n{cargo_observation}",
            cargo.display()
        );
        eprintln!(
            "exact packaged consumer rustc {}:\n{rustc_observation}",
            rustc.display()
        );
        let host = validate_versions(&cargo_observation, &rustc_observation)?;
        Ok(Self {
            exact: Some(ExactToolchain { cargo, rustc, host }),
        })
    }

    pub(super) fn cargo_command(&self) -> Command {
        let Some(exact) = &self.exact else {
            return super::native_rust_cargo::cargo_command();
        };
        let mut command = exact_cargo_command(exact);
        super::native_rust_cargo::bind_nested_cargo_linker_path(&mut command);
        command
    }

    pub(super) fn rustc_command(&self) -> Command {
        let Some(exact) = &self.exact else {
            return Command::new("rustc");
        };
        let mut command = Command::new(&exact.rustc);
        command.arg("--target").arg(&exact.host);
        command
    }
}

// Pure construction permits command inspection without provisioning a linker.
// Real dispatch additionally binds the existing platform linker above.
fn exact_cargo_command(exact: &ExactToolchain) -> Command {
    let mut command = Command::new(&exact.cargo);
    command
        .env("RUSTC", &exact.rustc)
        .env("RUSTC_WRAPPER", "")
        .env("RUSTC_WORKSPACE_WRAPPER", "")
        .env("CARGO_BUILD_TARGET", &exact.host);
    command
}

fn validate_pair(
    cargo: Option<OsString>,
    rustc: Option<OsString>,
) -> Result<Option<(PathBuf, PathBuf)>, String> {
    match (cargo, rustc) {
        (None, None) => Ok(None),
        (Some(cargo), Some(rustc)) => {
            let cargo = validate_path(cargo, "cargo")?;
            let rustc = validate_path(rustc, "rustc")?;
            Ok(Some((cargo, rustc)))
        }
        _ => Err("consumer Cargo and rustc overrides must be supplied together".into()),
    }
}

fn validate_path(value: OsString, tool: &str) -> Result<PathBuf, String> {
    let path = PathBuf::from(value);
    if !path.is_absolute() {
        return Err(format!("consumer {tool} must be a nonempty absolute path"));
    }
    let expected = if cfg!(windows) {
        format!("{tool}.exe")
    } else {
        tool.to_owned()
    };
    if path.file_name().and_then(|p| p.to_str()) != Some(expected.as_str()) {
        return Err(format!(
            "consumer {tool} must name the actual {expected} binary"
        ));
    }
    for ancestor in path.ancestors() {
        let metadata = std::fs::symlink_metadata(ancestor)
            .map_err(|e| format!("consumer {tool} path: {e}"))?;
        if metadata.file_type().is_symlink() {
            return Err(format!("consumer {tool} path must not traverse symlinks"));
        }
        if ancestor == path && !metadata.is_file() {
            return Err(format!("consumer {tool} must be a regular file"));
        }
    }
    // Rustup's ordinary proxy directory is not an actual paired toolchain.
    if path
        .parent()
        .and_then(Path::file_name)
        .is_some_and(|p| p == "bin")
        && path
            .parent()
            .and_then(Path::parent)
            .and_then(Path::file_name)
            .is_some_and(|p| p == ".cargo")
    {
        return Err(format!("consumer {tool} must not be a rustup proxy"));
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        let metadata = std::fs::metadata(&path).map_err(|e| e.to_string())?;
        if let Some(parent) = path.parent() {
            if let Ok(proxy) = std::fs::metadata(parent.join("rustup")) {
                if metadata.dev() == proxy.dev() && metadata.ino() == proxy.ino() {
                    return Err(format!("consumer {tool} must not be a rustup proxy"));
                }
            }
        }
    }
    Ok(path)
}

fn observe(path: &Path) -> Result<String, String> {
    let output = Command::new(path)
        .arg("-Vv")
        .output()
        .map_err(|e| format!("observe {}: {e}", path.display()))?;
    if !output.status.success() {
        return Err(format!(
            "{} -Vv failed: {}",
            path.display(),
            String::from_utf8_lossy(&output.stderr)
        ));
    }
    String::from_utf8(output.stdout).map_err(|e| e.to_string())
}

fn version_host<'a>(observation: &'a str, tool: &str) -> Result<&'a str, String> {
    let mut lines = observation.lines();
    let first = lines
        .next()
        .ok_or_else(|| format!("missing {tool} version"))?;
    if !first.starts_with(&format!("{tool} 1.85.0 (")) {
        return Err(format!("consumer {tool} must be exactly 1.85.0"));
    }
    let field = |key: &str| -> Result<&'a str, String> {
        let values: Vec<_> = observation
            .lines()
            .filter_map(|l| l.strip_prefix(key))
            .collect();
        if values.len() != 1 || values[0].is_empty() {
            return Err(format!("missing/duplicate {tool} {key}"));
        }
        Ok(values[0])
    };
    if field("release: ")? != "1.85.0" {
        return Err(format!("consumer {tool} release must be exactly 1.85.0"));
    }
    let host = field("host: ")?;
    if !host
        .bytes()
        .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
        || !host.contains('-')
    {
        return Err(format!("invalid {tool} host"));
    }
    Ok(host)
}

fn validate_versions(cargo: &str, rustc: &str) -> Result<String, String> {
    let cargo_host = version_host(cargo, "cargo")?;
    let rustc_host = version_host(rustc, "rustc")?;
    if cargo_host != rustc_host {
        return Err("consumer Cargo/rustc host mismatch".into());
    }
    Ok(rustc_host.into())
}

#[cfg(test)]
mod tests {
    use super::*;
    fn observation(tool: &str, release: &str, host: &str) -> String {
        format!("{tool} {release} (abcdef 2025-02-20)\nrelease: {release}\nhost: {host}\n")
    }
    #[test]
    fn absent_pair_preserves_default_and_invalid_paths_refuse_without_execution() {
        assert!(validate_pair(None, None).unwrap().is_none());
        assert!(validate_pair(Some("cargo".into()), None).is_err());
        assert!(validate_pair(None, Some("rustc".into())).is_err());
        assert!(validate_pair(Some("".into()), Some("".into())).is_err());
        assert!(validate_pair(Some("cargo".into()), Some("rustc".into())).is_err());
        let fixture = super::super::Fixture::new("consumer-toolchain-config");
        let directory = fixture.0.canonicalize().unwrap();
        let cargo = directory.join(if cfg!(windows) { "cargo.exe" } else { "cargo" });
        let rustc = directory.join(if cfg!(windows) { "rustc.exe" } else { "rustc" });
        assert!(validate_pair(Some(cargo.clone().into()), Some(rustc.clone().into())).is_err());
        assert!(validate_path(directory.join("rustup").into(), "cargo").is_err());
        std::fs::create_dir(&cargo).unwrap();
        assert!(validate_pair(Some(cargo.clone().into()), Some(rustc.clone().into())).is_err());
        std::fs::remove_dir(&cargo).unwrap();
        std::fs::write(&cargo, b"inert data, never executed").unwrap();
        std::fs::write(&rustc, b"inert data, never executed").unwrap();
        assert!(
            validate_pair(Some(cargo.clone().into()), Some(rustc.clone().into()))
                .unwrap()
                .is_some()
        );
        #[cfg(unix)]
        {
            std::fs::remove_file(&rustc).unwrap();
            std::os::unix::fs::symlink(&cargo, &rustc).unwrap();
            assert!(validate_pair(Some(cargo.clone().into()), Some(rustc.clone().into())).is_err());
            std::fs::remove_file(&rustc).unwrap();
            std::fs::hard_link(&cargo, directory.join("rustup")).unwrap();
            assert!(validate_path(cargo.into(), "cargo").is_err());
        }
    }
    #[test]
    fn exact_version_and_matching_host_are_required() {
        let host = "aarch64-apple-darwin";
        let cargo = observation("cargo", "1.85.0", host);
        let rustc = observation("rustc", "1.85.0", host);
        assert_eq!(validate_versions(&cargo, &rustc).unwrap(), host);
        for release in ["1.85.1", "1.85.0-nightly", "1.98.0"] {
            assert!(validate_versions(&observation("cargo", release, host), &rustc).is_err());
            assert!(validate_versions(&cargo, &observation("rustc", release, host)).is_err());
        }
        assert!(validate_versions(
            &cargo,
            &observation("rustc", "1.85.0", "x86_64-unknown-linux-gnu")
        )
        .is_err());
        assert!(
            validate_versions(&cargo.replace("release: 1.85.0", "release: 1.85.1"), &rustc)
                .is_err()
        );
        assert!(validate_versions(&format!("{cargo}host: {host}\n"), &rustc).is_err());
    }
    #[test]
    fn commands_pin_program_compiler_empty_wrappers_and_host_target() {
        let exact = ConsumerToolchain {
            exact: Some(ExactToolchain {
                cargo: PathBuf::from("/exact/cargo"),
                rustc: PathBuf::from("/exact/rustc"),
                host: "aarch64-apple-darwin".into(),
            }),
        };
        let command = exact_cargo_command(exact.exact.as_ref().unwrap());
        assert_eq!(command.get_program(), "/exact/cargo");
        let env: std::collections::BTreeMap<_, _> = command.get_envs().collect();
        assert_eq!(
            env[std::ffi::OsStr::new("RUSTC")],
            Some(std::ffi::OsStr::new("/exact/rustc"))
        );
        for name in ["RUSTC_WRAPPER", "RUSTC_WORKSPACE_WRAPPER"] {
            assert_eq!(
                env[std::ffi::OsStr::new(name)],
                Some(std::ffi::OsStr::new(""))
            );
        }
        assert_eq!(
            env[std::ffi::OsStr::new("CARGO_BUILD_TARGET")],
            Some(std::ffi::OsStr::new("aarch64-apple-darwin"))
        );
        let rustc = exact.rustc_command();
        assert_eq!(rustc.get_program(), "/exact/rustc");
        assert_eq!(
            rustc.get_args().collect::<Vec<_>>(),
            ["--target", "aarch64-apple-darwin"]
        );
    }
}
