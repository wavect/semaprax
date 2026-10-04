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
    native_authority_check: semaprax_toolchain::rich_native_cli::run,
    // No private override: signed release material uses the same pure,
    // built-in offline Sigstore verifier as the public executable.
    offline_release_verifier: None,
    #[cfg(windows)]
    build_owned_npm: semaprax_toolchain::build_owned_npm,
};

fn main() -> std::process::ExitCode {
    driver::main_with_host(Some(&HOST))
}
