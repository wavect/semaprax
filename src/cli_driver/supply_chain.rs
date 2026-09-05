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
        Err(cli::project_lock::ProjectLockCliError::Breaking(report)) => {
            print!("{report}");
            Err(1)
        }
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

#[cfg(test)]
mod tests {
    use super::{package_lock, package_resolve, project_lock, resolve};

    fn argv(tokens: &[&str]) -> Vec<String> {
        tokens.iter().map(|token| (*token).to_owned()).collect()
    }

    /// Every supply-chain dispatcher must map a usage refusal onto exit 2 and
    /// never onto the generic failure code a domain diagnostic uses. These
    /// arguments all fail in the command's own argument grammar, so no
    /// filesystem, network, or lock authority is reached.
    #[test]
    fn usage_refusals_exit_with_two_on_every_supply_chain_route() {
        assert_eq!(package_lock(&argv(&["--verbose"])), Err(2));
        assert_eq!(project_lock(&argv(&["--verbose"])), Err(2));
        assert_eq!(package_resolve(&argv(&["--verbose"])), Err(2));
        assert_eq!(resolve(&argv(&["--verbose"])), Err(2));
        // A missing required option is a usage refusal too, not a domain error.
        assert_eq!(package_lock(&[]), Err(2));
        assert_eq!(package_resolve(&[]), Err(2));
        assert_eq!(resolve(&argv(&["--cache", "cache"])), Err(2));
        assert_eq!(resolve(&argv(&["--target", "native64"])), Err(2));
    }

    /// `package-lock` is order dependent by construction: `--max-bytes` must
    /// come last. Placing a subject after it is a usage refusal rather than a
    /// silently reordered parse.
    #[test]
    fn package_lock_requires_subjects_before_the_byte_budget() {
        assert_eq!(
            package_lock(&argv(&["--max-bytes", "65536", "subject.spx"])),
            Err(2)
        );
        assert_eq!(
            package_lock(&argv(&["--max-bytes", "65536", "--max-bytes", "65536"])),
            Err(2)
        );
        assert_eq!(package_lock(&argv(&["--max-bytes"])), Err(2));
    }

    /// `lock` admits at most one mode flag; two of them must not silently
    /// last-win into a mode that writes.
    #[test]
    fn project_lock_refuses_two_mode_flags() {
        assert_eq!(project_lock(&argv(&["--write", "--verify"])), Err(2));
        assert_eq!(project_lock(&argv(&["--compare"])), Err(2));
        assert_eq!(project_lock(&argv(&["--compare-interface"])), Err(2));
    }

    /// `resolve` admits at most one of `--write` and `--verify`, and requires
    /// a canonical decimal byte budget.
    #[test]
    fn resolve_refuses_two_mode_flags_and_a_non_canonical_byte_budget() {
        let base = ["--target", "native64", "--cache", "cache"];
        for tail in [
            vec!["--write", "--verify"],
            vec!["--max-bytes", "0x10"],
            vec!["--max-bytes", "065536"],
            vec!["--max-bytes", "-1"],
            vec!["--max-bytes"],
        ] {
            let mut tokens = base.to_vec();
            tokens.extend_from_slice(&tail);
            assert_eq!(resolve(&argv(&tokens)), Err(2), "{tail:?}");
        }
    }
}
