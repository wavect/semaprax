mod cli_driver;

fn main() -> std::process::ExitCode {
    cli_driver::main_with_host(None)
}
