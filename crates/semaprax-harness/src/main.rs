//! Standalone harness host binary; `semaprax harness` forwards to the same code.

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let outcome = semaprax_harness::cli::run(&args, &semaprax_harness::cli::Environment::from_process());
    print!("{}", outcome.stdout);
    eprint!("{}", outcome.stderr);
    std::process::exit(outcome.code);
}
