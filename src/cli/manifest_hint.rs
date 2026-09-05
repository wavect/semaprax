//! The hint for running a project command outside a project.
//!
//! `check`, `run`, `test`, and `build` default to `./semaprax.toml` when they
//! receive no input. Outside a project that surfaces as `SPX-J102` naming a
//! manifest the user never typed, which is correct but leaves a newcomer or an
//! agent without a next step. When the manifest was that bare default and the
//! file is absent, the diagnostic gains a hint naming the three admitted
//! inputs. An explicitly named manifest is the caller's and stays unchanged.

use std::path::Path;

use semaprax::diagnostic::Diagnostic;

use super::project::DEFAULT_MANIFEST;

pub(crate) const MISSING_MANIFEST_HELP: &str = "no `semaprax.toml` in the current directory: pass a \
                                               `.spx` file, a project directory, or run from inside a \
                                               project";

pub(crate) fn hint_missing_manifest(
    errors: Vec<Diagnostic>,
    manifest_path: &Path,
) -> Vec<Diagnostic> {
    let is_default_manifest =
        manifest_path.file_name().and_then(|name| name.to_str()) == Some(DEFAULT_MANIFEST) && {
            // Bare `semaprax.toml` (relative) or absolute `…/semaprax.toml` where parent is current_dir
            if manifest_path == Path::new(DEFAULT_MANIFEST) {
                true
            } else if let Ok(current) = std::env::current_dir() {
                manifest_path.parent() == Some(current.as_path())
            } else {
                false
            }
        };
    if !is_default_manifest || manifest_path.exists() {
        return errors;
    }
    errors
        .into_iter()
        .map(|diagnostic| {
            if diagnostic.code == "SPX-J102" && diagnostic.help.is_none() {
                diagnostic.with_help(MISSING_MANIFEST_HELP)
            } else {
                diagnostic
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn missing() -> Vec<Diagnostic> {
        vec![
            Diagnostic::io("SPX-J102", "cannot inspect declared Project v1 manifest"),
            Diagnostic::io("SPX-J100", "unrelated"),
        ]
    }

    #[test]
    fn only_the_bare_default_manifest_gains_the_hint() {
        let hinted = hint_missing_manifest(missing(), Path::new(DEFAULT_MANIFEST));
        assert_eq!(hinted[0].help.as_deref(), Some(MISSING_MANIFEST_HELP));
        assert_eq!(hinted[1].help, None);

        let explicit = hint_missing_manifest(missing(), &PathBuf::from("fixtures/semaprax.toml"));
        assert!(explicit.iter().all(|diagnostic| diagnostic.help.is_none()));

        let directory =
            hint_missing_manifest(missing(), &Path::new("missing-dir").join(DEFAULT_MANIFEST));
        assert!(directory.iter().all(|diagnostic| diagnostic.help.is_none()));
    }
}
