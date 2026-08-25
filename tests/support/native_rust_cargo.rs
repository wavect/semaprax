use std::process::Command;

const WINDOWS_CARGO_LINKER: &str = "CARGO_TARGET_X86_64_PC_WINDOWS_MSVC_LINKER";

pub fn cargo_command() -> Command {
    let cargo = std::env::var_os("CARGO").unwrap_or_else(|| "cargo".into());
    let mut command = Command::new(cargo);
    bind_nested_cargo_linker_path(&mut command);
    command
}

/// Binds one validated absolute linker pathname for these nested Cargo tests.
/// This does not hold the linker image or its ancestors, attest ancestor
/// reparses, or close a same-path substitution race after this check.
pub fn bind_nested_cargo_linker_path(command: &mut Command) {
    #[cfg(windows)]
    {
        use std::path::{Path, PathBuf};

        let linker = PathBuf::from(
            std::env::var_os("SEMAPRAX_LINKER")
                .expect("nested Windows Cargo requires the authenticated SEMAPRAX_LINKER"),
        );
        let vctools = PathBuf::from(
            std::env::var_os("SEMAPRAX_VCTOOLS")
                .expect("nested Windows Cargo requires the authenticated SEMAPRAX_VCTOOLS"),
        );
        assert!(linker.is_absolute(), "SEMAPRAX_LINKER must be absolute");
        assert!(vctools.is_absolute(), "SEMAPRAX_VCTOOLS must be absolute");
        assert_eq!(
            linker.strip_prefix(&vctools).ok(),
            Some(Path::new(r"bin\Hostx64\x64\link.exe")),
            "SEMAPRAX_LINKER must name the configured x64 MSVC linker path",
        );
        let metadata = std::fs::symlink_metadata(&linker)
            .expect("stat the configured nested-Cargo Windows linker path");
        assert!(
            metadata.is_file() && !metadata.file_type().is_symlink(),
            "nested-Cargo Windows linker path must currently name a regular non-symlink file",
        );
        command.env(WINDOWS_CARGO_LINKER, linker.as_os_str());
        command.env_remove("LINK");
        command.env_remove("_LINK_");
    }

    #[cfg(not(windows))]
    let _ = (command, WINDOWS_CARGO_LINKER);
}
