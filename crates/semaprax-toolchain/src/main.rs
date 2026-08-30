mod doctor;
#[path = "../../../src/cli_driver.rs"]
mod driver;
mod new_project;

static HOST: driver::PrivateHost = driver::PrivateHost {
    doctor: |arguments| {
        doctor::run(arguments)
            .map(|outcome| (outcome.output, outcome.exit_code))
            .map_err(|error| error.to_string())
    },
    new_project: |arguments| {
        new_project::run(arguments).map_err(|error| (error.to_string(), error.exit_code()))
    },
    build_rust: semaprax_toolchain::build_rust,
};

fn main() -> std::process::ExitCode {
    driver::main_with_host(Some(&HOST))
}
