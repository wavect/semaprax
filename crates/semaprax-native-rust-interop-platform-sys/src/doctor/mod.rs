//! Doctor-only trusted installed-tool probes; not build authority or a sandbox.
use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::time::Duration;

#[cfg(test)]
mod tests;
#[cfg(any(target_os = "linux", target_os = "macos"))]
mod unix;
#[cfg(windows)]
mod windows;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProbeError {
    Invalid,
    Unsupported,
    Spawn,
    Exit,
    OutputLimit,
    Timeout,
    Io,
}

#[derive(Clone, Copy)]
struct Limits {
    output: usize,
    run: Duration,
    settle: Duration,
}

impl Default for Limits {
    fn default() -> Self {
        Self {
            output: 65_536,
            run: Duration::from_secs(10),
            settle: Duration::from_secs(5),
        }
    }
}

#[allow(dead_code)] // Faults are private and some belong to only one OS lane.
#[derive(Clone, Copy, Eq, PartialEq)]
enum Fault {
    Read,
    Wait,
    Deadline,
    Kill,
    Settle,
    Close,
    Spawn,
    Assign,
    Resume,
}

struct Prepared {
    path: PathBuf,
    cwd: PathBuf,
    environment: Vec<(OsString, OsString)>,
    limits: Limits,
    #[cfg(test)]
    fault: Option<Fault>,
}

impl Prepared {
    fn injected(&self, fault: Fault) -> bool {
        #[cfg(test)]
        {
            self.fault == Some(fault)
        }
        #[cfg(not(test))]
        {
            let _ = fault;
            false
        }
    }
}

fn prepare(path: &Path) -> Result<Prepared, ProbeError> {
    if !path.is_absolute() || native_units(path.as_os_str()) > 32_768 {
        return Err(ProbeError::Invalid);
    }
    let cwd = std::env::current_dir().map_err(|_| ProbeError::Invalid)?;
    if native_units(cwd.as_os_str()) > 32_768 {
        return Err(ProbeError::Invalid);
    }
    let environment = sanitized_environment(|key| std::env::var_os(key))?;
    Ok(Prepared {
        path: path.to_owned(),
        cwd,
        environment,
        limits: Limits::default(),
        #[cfg(test)]
        fault: None,
    })
}

fn sanitized_environment(
    mut read: impl FnMut(&str) -> Option<OsString>,
) -> Result<Vec<(OsString, OsString)>, ProbeError> {
    let mut environment = Vec::new();
    // Include the forced row and final block terminator in the same bound as
    // copied rows. These fixed ASCII strings have equal byte/UTF-16 lengths.
    let mut total = "RUSTUP_AUTO_INSTALL".len() + "0".len() + 2 + 1;
    for key in retained_environment_names() {
        if let Some(value) = read(key) {
            let units = native_units(&value);
            if units > 8192 {
                return Err(ProbeError::Invalid);
            }
            total = total
                .checked_add(key.len() + units + 2)
                .ok_or(ProbeError::Invalid)?;
            if total > 32_768 {
                return Err(ProbeError::Invalid);
            }
            environment.push((OsString::from(key), value));
        }
    }
    // Documented rustup control: absent toolchains must not be auto-installed.
    // This does not assert that arbitrary local executables cannot use network.
    environment.push((OsString::from("RUSTUP_AUTO_INSTALL"), OsString::from("0")));
    Ok(environment)
}

fn retained_environment_names() -> &'static [&'static str] {
    #[cfg(windows)]
    {
        &[
            "CARGO_HOME",
            "HOME",
            "RUSTUP_HOME",
            "RUSTUP_TOOLCHAIN",
            "SystemRoot",
            "USERPROFILE",
            "WINDIR",
        ]
    }
    #[cfg(target_os = "macos")]
    {
        &[
            "CARGO_HOME",
            "DEVELOPER_DIR",
            "HOME",
            "RUSTUP_HOME",
            "RUSTUP_TOOLCHAIN",
        ]
    }
    #[cfg(not(any(windows, target_os = "macos")))]
    {
        &["CARGO_HOME", "HOME", "RUSTUP_HOME", "RUSTUP_TOOLCHAIN"]
    }
}

fn native_units(value: &std::ffi::OsStr) -> usize {
    #[cfg(windows)]
    {
        use std::os::windows::ffi::OsStrExt as _;
        value.encode_wide().count()
    }
    #[cfg(not(windows))]
    {
        value.as_encoded_bytes().len()
    }
}

/// Run exactly `--version`, retaining only bounded stdout after owned process
/// settlement. The lexical executable name is not canonicalized or attested.
/// An uncertain OS settlement is fail-stop, not an ordinary diagnostic result.
/// Unix callers must exclude foreign child reapers and concurrent SIGCHLD
/// policy mutation for the invocation; non-default policy is rejected before
/// launch. This cooperative host condition is not arbitrary-code containment.
pub fn probe_version(path: &Path) -> Result<Vec<u8>, ProbeError> {
    let prepared = prepare(path)?;
    #[cfg(any(target_os = "linux", target_os = "macos"))]
    {
        unix::run(&prepared)
    }
    #[cfg(windows)]
    {
        windows::run(&prepared)
    }
    #[cfg(not(any(target_os = "linux", target_os = "macos", windows)))]
    {
        let _ = prepared;
        Err(ProbeError::Unsupported)
    }
}
