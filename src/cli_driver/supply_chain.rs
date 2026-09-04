//! Dispatch for the dependency supply-chain commands. These live outside the
//! driver root so adding a lock or resolve surface does not grow a budgeted
//! module.
use super::{cli, report, write_package_resolver_stdout};

pub(super) fn package_lock(args: &[String]) -> Result<(), u8> {
    match cli::package_lock::run(args) {
        Ok(lock) => {
            println!("{lock}");
            Ok(())
        }
        Err(cli::package_lock::PackageLockCliError::Usage(message)) => {
            eprintln!("{message}");
            Err(2)
        }
        Err(cli::package_lock::PackageLockCliError::Domain(errors)) => Err(report(&errors, false)),
    }
}

pub(super) fn project_lock(args: &[String]) -> Result<(), u8> {
    match cli::project_lock::run(args) {
        Ok(output) => {
            print!("{output}");
            Ok(())
        }
        Err(cli::project_lock::ProjectLockCliError::Usage(message)) => {
            eprintln!("{message}");
            Err(2)
        }
        Err(cli::project_lock::ProjectLockCliError::Domain(errors)) => Err(report(&errors, false)),
    }
}

pub(super) fn package_resolve(args: &[String]) -> Result<(), u8> {
    match cli::package_resolver::run(args) {
        Ok(evidence) => {
            write_package_resolver_stdout(&evidence).map_err(|error| report(&[error], false))
        }
        Err(cli::package_resolver::PackageResolverCliError::Usage(message)) => {
            eprintln!("{message}");
            Err(2)
        }
        Err(cli::package_resolver::PackageResolverCliError::Domain(errors)) => {
            Err(report(&errors, false))
        }
    }
}

pub(super) fn resolve(args: &[String]) -> Result<(), u8> {
    match cli::resolve::run(args) {
        Ok(evidence) => {
            println!("{evidence}");
            Ok(())
        }
        Err(cli::resolve::ResolveCliError::Usage(message)) => {
            eprintln!("{message}");
            Err(2)
        }
        Err(cli::resolve::ResolveCliError::Domain(errors)) => Err(report(&errors, false)),
    }
}
