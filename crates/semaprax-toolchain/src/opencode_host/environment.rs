//! Host-only OpenCode environment for the explicit #112 process boundary.
//!
//! OpenCode v1.18.27 loads project and global instructions independently of
//! `--pure`. Keep its config and home discovery in a private scratch tree while
//! preserving only the host's XDG data root for existing provider auth.

use std::env;
use std::ffi::OsStr;
use std::fs;
use std::io::{ErrorKind, Read};
use std::path::{Path, PathBuf};
use std::process::Command;

const MAX_AUTH_BYTES: u64 = 1_048_576;
const PRIVATE: &str = ".opencode-host-private";
const REFUSED: &str = "OpenCode host environment is not context-isolated";

struct Sources {
    data_home: PathBuf,
    managed_config: PathBuf,
    managed_preferences: Option<PathBuf>,
}

#[cfg(not(test))]
fn host_data_home() -> Result<PathBuf, &'static str> {
    if let Some(value) = env::var_os("XDG_DATA_HOME") {
        let path = PathBuf::from(value);
        return path.is_absolute().then_some(path).ok_or(REFUSED);
    }
    let home = env::var_os("HOME").map(PathBuf::from).ok_or(REFUSED)?;
    if !home.is_absolute() {
        return Err(REFUSED);
    }
    Ok(home.join(".local/share"))
}

#[cfg(not(test))]
fn sources() -> Result<Sources, &'static str> {
    #[cfg(target_os = "macos")]
    let (managed_config, managed_preferences) = (
        PathBuf::from("/Library/Application Support/opencode"),
        Some(PathBuf::from("/Library/Managed Preferences")),
    );
    #[cfg(not(target_os = "macos"))]
    let (managed_config, managed_preferences) = (PathBuf::from("/etc/opencode"), None);
    Ok(Sources {
        data_home: host_data_home()?,
        managed_config,
        managed_preferences,
    })
}

fn existing(path: &Path) -> Result<bool, &'static str> {
    match fs::symlink_metadata(path) {
        Ok(_) => Ok(true),
        Err(error) if error.kind() == ErrorKind::NotFound => Ok(false),
        Err(_) => Err(REFUSED),
    }
}

fn require_absent(path: &Path) -> Result<(), &'static str> {
    if existing(path)? {
        Err(REFUSED)
    } else {
        Ok(())
    }
}

fn check_managed(sources: &Sources) -> Result<(), &'static str> {
    // ConfigManaged reads these files after the explicit config; inherited
    // instructions arrays concatenate, so an empty explicit array cannot
    // neutralize them. Refuse their presence instead.
    for file in ["opencode.json", "opencode.jsonc"] {
        if existing(&sources.managed_config.join(file))? {
            return Err(REFUSED);
        }
    }
    let Some(root) = &sources.managed_preferences else {
        return Ok(());
    };
    if existing(&root.join("ai.opencode.managed.plist"))? {
        return Err(REFUSED);
    }
    let entries = match fs::read_dir(root) {
        Ok(entries) => entries,
        Err(error) if error.kind() == ErrorKind::NotFound => return Ok(()),
        Err(_) => return Err(REFUSED),
    };
    for entry in entries {
        let entry = entry.map_err(|_| REFUSED)?;
        if existing(&entry.path().join("ai.opencode.managed.plist"))? {
            return Err(REFUSED);
        }
    }
    Ok(())
}

fn check_auth(sources: &Sources) -> Result<(), &'static str> {
    // Auth.all() reads XDG_DATA_HOME/opencode/auth.json. A `wellknown` entry
    // makes Config.loadInstanceState fetch remote config before our policy,
    // which can inject instructions. API/OAuth entries retain fixed-model auth.
    let file = sources.data_home.join("opencode/auth.json");
    let metadata = match fs::metadata(&file) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == ErrorKind::NotFound => return Ok(()),
        Err(_) => return Err(REFUSED),
    };
    if !metadata.is_file() || metadata.len() > MAX_AUTH_BYTES {
        return Err(REFUSED);
    }
    let mut bytes = Vec::new();
    fs::File::open(file)
        .map_err(|_| REFUSED)?
        .take(MAX_AUTH_BYTES + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| REFUSED)?;
    if bytes.len() as u64 > MAX_AUTH_BYTES {
        return Err(REFUSED);
    }
    let auth: serde_json::Value = serde_json::from_slice(&bytes).map_err(|_| REFUSED)?;
    let entries = auth.as_object().ok_or(REFUSED)?;
    for item in entries.values() {
        if !matches!(
            item.get("type").and_then(serde_json::Value::as_str),
            Some("api" | "oauth")
        ) {
            return Err(REFUSED);
        }
    }
    Ok(())
}

fn private_dir(sandbox: &Path) -> Result<PathBuf, &'static str> {
    if !sandbox.is_absolute() {
        return Err(REFUSED);
    }
    let root = sandbox.join(PRIVATE);
    match fs::create_dir(&root) {
        Ok(()) => {}
        Err(error) if error.kind() == ErrorKind::AlreadyExists => {
            let metadata = fs::symlink_metadata(&root).map_err(|_| REFUSED)?;
            if !metadata.is_dir() || metadata.file_type().is_symlink() {
                return Err(REFUSED);
            }
        }
        Err(_) => return Err(REFUSED),
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&root, fs::Permissions::from_mode(0o700)).map_err(|_| REFUSED)?;
    }
    for name in ["home", "config", "cache", "state", "tmp", "db"] {
        let path = root.join(name);
        match fs::create_dir(&path) {
            Ok(()) => {}
            Err(error) if error.kind() == ErrorKind::AlreadyExists => {
                let metadata = fs::symlink_metadata(&path).map_err(|_| REFUSED)?;
                if !metadata.is_dir() || metadata.file_type().is_symlink() {
                    return Err(REFUSED);
                }
            }
            Err(_) => return Err(REFUSED),
        }
    }
    let global_config = root.join("config/opencode");
    fs::create_dir_all(&global_config).map_err(|_| REFUSED)?;
    let metadata = fs::symlink_metadata(&global_config).map_err(|_| REFUSED)?;
    if !metadata.is_dir() || metadata.file_type().is_symlink() {
        return Err(REFUSED);
    }
    // OpenCode scans these even when project config discovery is disabled.
    // A previous invocation must not leave new implicit prompt/config files.
    for file in [
        "AGENTS.md",
        "config.json",
        "opencode.json",
        "opencode.jsonc",
    ] {
        require_absent(&global_config.join(file))?;
    }
    for dir in ["agents", "modes", "commands", "plugins", "plugin", "skills"] {
        require_absent(&global_config.join(dir))?;
    }
    require_absent(&root.join("home/.opencode"))?;
    require_absent(&root.join("home/.claude"))?;
    Ok(root)
}

fn configure_with_sources(
    command: &mut Command,
    sandbox: &Path,
    sources: &Sources,
) -> Result<(), &'static str> {
    check_managed(sources)?;
    check_auth(sources)?;
    let root = private_dir(sandbox)?;
    // The scratch database contains sessions for run -> export, but no host
    // active account. In v1.18.27 that removes active-org remote config loading.
    command.env_clear();
    command.env(
        "PATH",
        OsStr::new("/opt/homebrew/bin:/usr/local/bin:/usr/bin:/bin:/usr/sbin:/sbin"),
    );
    command.env("LANG", "en_US.UTF-8");
    command.env("HOME", root.join("home"));
    command.env("XDG_CONFIG_HOME", root.join("config"));
    command.env("XDG_CACHE_HOME", root.join("cache"));
    command.env("XDG_STATE_HOME", root.join("state"));
    command.env("XDG_DATA_HOME", &sources.data_home);
    command.env("TMPDIR", root.join("tmp"));
    command.env("OPENCODE_CONFIG", sandbox.join("opencode.json"));
    command.env("OPENCODE_CONFIG_DIR", root.join("config/opencode"));
    command.env("OPENCODE_DB", root.join("db/opencode.db"));
    command.env("OPENCODE_DISABLE_PROJECT_CONFIG", "1");
    command.env("OPENCODE_DISABLE_CLAUDE_CODE", "1");
    command.env("OPENCODE_DISABLE_EXTERNAL_SKILLS", "1");
    command.env("OPENCODE_DISABLE_DEFAULT_PLUGINS", "1");
    command.env("OPENCODE_PURE", "1");
    command.env("OPENCODE_DISABLE_AUTOUPDATE", "1");
    Ok(())
}

/// Apply the same private context to `run` and `export` before each spawn.
/// No credential bytes enter command arguments, prompt, receipts, or errors.
pub(super) fn configure_command(command: &mut Command, sandbox: &Path) -> Result<(), &'static str> {
    #[cfg(not(test))]
    let sources = sources()?;
    // Local executable fixtures exercise the same environment construction
    // without reading the developer's credential store or managed settings.
    #[cfg(test)]
    let sources = Sources {
        data_home: sandbox.join("fixture-host-data"),
        managed_config: sandbox.join("fixture-managed-config"),
        managed_preferences: None,
    };
    configure_with_sources(command, sandbox, &sources)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};

    static NEXT: AtomicUsize = AtomicUsize::new(0);

    fn fixture() -> (PathBuf, PathBuf, Sources) {
        let root = env::temp_dir().join(format!(
            "semaprax-opencode-env-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&root).unwrap();
        let sandbox = root.join("sandbox");
        let data_home = root.join("host-data");
        fs::create_dir(&sandbox).unwrap();
        fs::create_dir_all(data_home.join("opencode")).unwrap();
        let managed_config = root.join("managed-config");
        let managed_preferences = root.join("managed-preferences");
        fs::create_dir(&managed_config).unwrap();
        fs::create_dir(&managed_preferences).unwrap();
        let sources = Sources {
            data_home,
            managed_config,
            managed_preferences: Some(managed_preferences),
        };
        (root, sandbox, sources)
    }

    fn value(command: &Command, key: &str) -> Option<String> {
        command
            .get_envs()
            .find(|(name, _)| name == &OsStr::new(key))
            .and_then(|(_, value)| value)
            .map(|value| value.to_string_lossy().into_owned())
    }

    #[test]
    fn explicit_config_and_auth_path_survive_both_invocations_without_secret_env() {
        let (root, sandbox, sources) = fixture();
        let secret = "HOST_AUTH_SECRET_SENTINEL";
        fs::write(
            sources.data_home.join("opencode/auth.json"),
            format!(r#"{{"opencode":{{"type":"api","key":"{secret}"}}}}"#),
        )
        .unwrap();
        for _ in 0..2 {
            let mut command = Command::new("/bin/true");
            command.env("OPENCODE_CONFIG_CONTENT", "UNTRUSTED");
            command.env("OPENCODE_PERMISSION", "UNTRUSTED");
            configure_with_sources(&mut command, &sandbox, &sources).unwrap();
            assert_eq!(
                value(&command, "XDG_DATA_HOME"),
                Some(sources.data_home.display().to_string())
            );
            assert_eq!(
                value(&command, "OPENCODE_CONFIG"),
                Some(sandbox.join("opencode.json").display().to_string())
            );
            assert_eq!(
                value(&command, "OPENCODE_DISABLE_PROJECT_CONFIG").as_deref(),
                Some("1")
            );
            assert_eq!(
                value(&command, "OPENCODE_DISABLE_DEFAULT_PLUGINS").as_deref(),
                Some("1")
            );
            assert!(value(&command, "OPENCODE_DB")
                .unwrap()
                .starts_with(sandbox.to_str().unwrap()));
            assert!(command
                .get_envs()
                .all(|(_, value)| !value.unwrap().to_string_lossy().contains(secret)));
            assert!(value(&command, "OPENCODE_CONFIG_CONTENT").is_none());
            assert!(value(&command, "OPENCODE_PERMISSION").is_none());
        }
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn refuses_remote_auth_and_managed_instructions_without_leaking_values() {
        let (root, sandbox, sources) = fixture();
        let auth = sources.data_home.join("opencode/auth.json");
        fs::write(&auth, r#"{"remote":{"type":"wellknown","token":"SECRET"}}"#).unwrap();
        let mut command = Command::new("/bin/true");
        assert_eq!(
            configure_with_sources(&mut command, &sandbox, &sources),
            Err(REFUSED)
        );
        fs::write(&auth, r#"{"opencode":{"type":"api","key":"SECRET"}}"#).unwrap();
        fs::write(
            sources.managed_config.join("opencode.json"),
            r#"{"instructions":["SECRET"]}"#,
        )
        .unwrap();
        assert_eq!(
            configure_with_sources(&mut command, &sandbox, &sources),
            Err(REFUSED)
        );
        fs::remove_file(sources.managed_config.join("opencode.json")).unwrap();
        let user = sources
            .managed_preferences
            .as_ref()
            .unwrap()
            .join("someone");
        fs::create_dir(&user).unwrap();
        fs::write(user.join("ai.opencode.managed.plist"), "SECRET").unwrap();
        assert_eq!(
            configure_with_sources(&mut command, &sandbox, &sources),
            Err(REFUSED)
        );
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn refuses_scratch_instruction_on_repeated_invocation() {
        let (root, sandbox, sources) = fixture();
        let mut command = Command::new("/bin/true");
        configure_with_sources(&mut command, &sandbox, &sources).unwrap();
        fs::write(
            sandbox.join(PRIVATE).join("config/opencode/AGENTS.md"),
            "UNTRUSTED",
        )
        .unwrap();
        assert_eq!(
            configure_with_sources(&mut command, &sandbox, &sources),
            Err(REFUSED)
        );
        fs::remove_dir_all(root).unwrap();
    }
}
