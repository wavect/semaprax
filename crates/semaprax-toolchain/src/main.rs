#[path = "../../../src/cli_driver.rs"]
mod driver;
mod new_project;

static HOST: driver::PrivateHost = driver::PrivateHost {
    new_project: |arguments| {
        new_project::run(arguments).map_err(|error| (error.to_string(), error.exit_code()))
    },
    build_rust: semaprax_toolchain::build_rust,
    source_live: semaprax_toolchain::source_live_cli::run,
    source_agent_dev: Some(semaprax_toolchain::source_live_cli::run_hot_reload_migration),
    #[cfg(unix)]
    harness: semaprax_toolchain::harness_cli::run,
    #[cfg(windows)]
    harness: harness_unavailable,
    native_authority_check: semaprax_toolchain::rich_native_cli::run,
    // No private override: signed release material uses the same pure,
    // built-in offline Sigstore verifier as the public executable.
    offline_release_verifier: None,
    #[cfg(windows)]
    build_owned_npm: semaprax_toolchain::build_owned_npm,
};

#[cfg(windows)]
fn harness_unavailable(_arguments: &[String]) -> u8 {
    eprintln!("harness: the provider host requires Unix process controls");
    2
}

fn main() -> std::process::ExitCode {
    driver::main_with_host(Some(&HOST))
}
