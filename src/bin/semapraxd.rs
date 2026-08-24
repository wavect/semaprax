fn main() {
    if let Err(message) = semaprax::project_transport::run_from_args(std::env::args_os()) {
        eprintln!("{message}");
        std::process::exit(1);
    }
}
