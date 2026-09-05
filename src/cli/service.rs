//! `semaprax service`: one persistent canonical semantic workspace over stdio.

use std::path::PathBuf;

use semaprax::diagnostic::Diagnostic;
use semaprax::project::with_authenticated_project;
use semaprax::semantic_service_transport::serve_semantic_workspace_stdio;

use super::project::{is_project_manifest, resolve_positional};

pub(crate) struct ServiceOptions {
    manifest: PathBuf,
}

pub(crate) fn parse(args: &[String]) -> Result<ServiceOptions, u8> {
    let [project] = args else {
        return usage();
    };
    if project.is_empty() || project.starts_with('-') {
        return usage();
    }
    let manifest = resolve_positional(PathBuf::from(project));
    if !is_project_manifest(&manifest) {
        eprintln!("service requires a Project directory or semaprax.toml");
        return Err(2);
    }
    Ok(ServiceOptions { manifest })
}

pub(crate) fn run(options: ServiceOptions, report: impl Fn(&[Diagnostic]) -> u8) -> Result<(), u8> {
    let stdin = std::io::stdin();
    let stdout = std::io::stdout();
    with_authenticated_project(&options.manifest, |snapshot| {
        serve_semantic_workspace_stdio(stdin.lock(), stdout.lock(), snapshot.retain_revision())
            .map_err(|error| {
                vec![Diagnostic::io(
                    "SPX-G548",
                    format!("semantic workspace service ended: {error}"),
                )]
            })
    })
    .map_err(|errors| report(&errors))
}

fn usage<T>() -> Result<T, u8> {
    eprintln!("service requires exactly <project>");
    Err(2)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn strings(values: &[&str]) -> Vec<String> {
        values.iter().map(|value| (*value).to_owned()).collect()
    }

    #[test]
    fn grammar_is_closed() {
        assert_eq!(
            parse(&strings(&["tests/project_scalar_wit_interface_v1"]))
                .unwrap()
                .manifest,
            std::env::current_dir()
                .unwrap()
                .join("tests/project_scalar_wit_interface_v1/semaprax.toml")
        );
        assert_eq!(
            parse(&strings(&["fixtures/semaprax.toml"]))
                .unwrap()
                .manifest,
            PathBuf::from("fixtures/semaprax.toml")
        );
        for malformed in [
            &[][..],
            &[""][..],
            &["--stdio"][..],
            &["module.spx"][..],
            &["fixtures", "extra"][..],
        ] {
            assert!(parse(&strings(malformed)).is_err(), "{malformed:?}");
        }
    }
}
