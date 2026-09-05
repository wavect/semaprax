//! `semaprax service`: one persistent canonical semantic workspace over stdio.

use std::path::PathBuf;

use semaprax::diagnostic::Diagnostic;
use semaprax::project::with_authenticated_project;
use semaprax::semantic_service_mcp::serve_semantic_workspace_mcp;
use semaprax::semantic_service_transport::serve_semantic_workspace_stdio;

use super::project::{is_project_manifest, resolve_positional};

pub(crate) struct ServiceOptions {
    manifest: PathBuf,
    mcp: bool,
}

pub(crate) fn parse(args: &[String]) -> Result<ServiceOptions, u8> {
    let (project, mcp) = match args {
        [project] => (project, false),
        [project, flag] if flag == "--mcp" => (project, true),
        _ => return usage(),
    };
    if project.is_empty() || project.starts_with('-') {
        return usage();
    }
    let manifest = resolve_positional(PathBuf::from(project));
    if !is_project_manifest(&manifest) {
        eprintln!("service requires a Project directory or semaprax.toml");
        return Err(2);
    }
    Ok(ServiceOptions { manifest, mcp })
}

pub(crate) fn run(options: ServiceOptions, report: impl Fn(&[Diagnostic]) -> u8) -> Result<(), u8> {
    let stdin = std::io::stdin();
    let stdout = std::io::stdout();
    // End the authenticated loader lifetime before stdio starts. The retained
    // revision is fully owned, so no request or EOF settlement can reopen a
    // startup Project path.
    let revision =
        with_authenticated_project(&options.manifest, |snapshot| Ok(snapshot.retain_revision()))
            .map_err(|errors| report(&errors))?;
    let result = if options.mcp {
        serve_semantic_workspace_mcp(stdin.lock(), stdout.lock(), revision)
    } else {
        serve_semantic_workspace_stdio(stdin.lock(), stdout.lock(), revision)
    };
    result.map_err(|error| {
        report(&[Diagnostic::io(
            "SPX-G548",
            format!("semantic workspace service ended: {error}"),
        )])
    })
}

fn usage<T>() -> Result<T, u8> {
    eprintln!("service requires exactly <project> [--mcp]");
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
        let repository = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .ancestors()
            .find(|root| {
                root.join("tests/project_scalar_wit_interface_v1/semaprax.toml")
                    .is_file()
            })
            .expect("repository fixture root");
        let project = repository.join("tests/project_scalar_wit_interface_v1");
        assert_eq!(
            parse(&[project.to_string_lossy().into_owned()])
                .unwrap()
                .manifest,
            project.join("semaprax.toml")
        );
        let manifest = repository.join("fixtures/semaprax.toml");
        assert_eq!(
            parse(&[manifest.to_string_lossy().into_owned()])
                .unwrap()
                .manifest,
            manifest
        );
        assert!(
            parse(&strings(&["fixtures/semaprax.toml", "--mcp"]))
                .unwrap()
                .mcp
        );
        for malformed in [
            &[][..],
            &[""][..],
            &["--stdio"][..],
            &["module.spx"][..],
            &["--mcp", "fixtures"][..],
            &["fixtures", "--mcp", "extra"][..],
            &["fixtures", "extra"][..],
        ] {
            assert!(parse(&strings(malformed)).is_err(), "{malformed:?}");
        }
    }
}
