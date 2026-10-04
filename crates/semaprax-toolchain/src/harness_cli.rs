//! `semaprax-full harness <verb>`: builds the explicit harness environment and
//! prints the verb's outcome. The full toolchain is itself the compiler
//! service unless `--compiler` or `$SEMAPRAX_COMPILER` names another.

use semaprax_harness::cli::{run as run_harness, Environment};

/// Returns the process exit code; stdout/stderr are written here.
pub fn run(arguments: &[String]) -> u8 {
    let mut env = Environment::from_process();
    if env.compiler.is_none() {
        env.compiler = std::env::current_exe().ok();
    }
    let outcome = run_harness(arguments, &env);
    print!("{}", outcome.stdout);
    eprint!("{}", outcome.stderr);
    u8::try_from(outcome.code).unwrap_or(1)
}
