//! Output dispatch for the global and scoped help surfaces. The catalog
//! text lives in `cli::help`; this module only prints it so the driver root
//! stays within its module budget.
use super::cli;

pub(super) fn print_help(has_private_host: bool) {
    print!("{}", cli::help::global(has_private_host));
}

pub(super) fn print_scoped_help(command: &str, has_private_host: bool) -> Result<(), u8> {
    let help = cli::help::global(has_private_host);
    if let Some(scoped) = cli::help::scoped(command, has_private_host) {
        print!("{scoped}");
        Ok(())
    } else {
        eprint!(
            "{}",
            cli::help::unknown_diagnostic(command, has_private_host)
        );
        print!("{help}");
        Err(2)
    }
}
