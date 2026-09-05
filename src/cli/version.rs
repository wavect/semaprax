use semaprax::diagnostic::quote_json;

const SCHEMA: &str = "semaprax.version.v1";
const MATURITY: &str = "pre-alpha";
const RUST_MIN: &str = "1.88";
const VERSION: &str = env!("CARGO_PKG_VERSION");
const INVALID_COMMIT: &str =
    "invalid SEMAPRAX_BUILD_COMMIT: expected exactly 40 lowercase hexadecimal characters";

#[derive(Clone, Copy)]
pub(crate) enum Invocation {
    Command,
    Flag,
}

pub(crate) fn render(invocation: Invocation, arguments: &[String]) -> Result<String, String> {
    match (invocation, arguments) {
        (_, []) => render_human_with_commit(option_env!("SEMAPRAX_BUILD_COMMIT")),
        (Invocation::Command, [argument]) if argument == "--json" => {
            render_json_with_commit(option_env!("SEMAPRAX_BUILD_COMMIT"))
        }
        (Invocation::Flag, _) => Err("--version does not accept arguments".to_owned()),
        (Invocation::Command, _) => Err(format!(
            "unexpected version argument `{}`",
            arguments.first().expect("non-empty version arguments")
        )),
    }
}

pub(crate) fn render_human_with_commit(commit: Option<&str>) -> Result<String, String> {
    let commit = validated_commit(commit)?;
    Ok(match commit {
        Some(commit) => format!("semaprax {VERSION} ({commit})\n"),
        None => format!("semaprax {VERSION} (commit unknown)\n"),
    })
}

pub(crate) fn render_json_with_commit(commit: Option<&str>) -> Result<String, String> {
    let commit = validated_commit(commit)?
        .map(quote_json)
        .unwrap_or_else(|| "null".to_owned());
    Ok(format!(
        "{{\"schema\":{},\"version\":{},\"commit\":{},\"maturity\":{},\"rust_min\":{}}}\n",
        quote_json(SCHEMA),
        quote_json(VERSION),
        commit,
        quote_json(MATURITY),
        quote_json(RUST_MIN),
    ))
}

fn validated_commit(commit: Option<&str>) -> Result<Option<&str>, String> {
    let Some(commit) = commit else {
        return Ok(None);
    };
    if commit.len() == 40
        && commit
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        Ok(Some(commit))
    } else {
        Err(INVALID_COMMIT.to_owned())
    }
}

#[cfg(test)]
mod tests {
    use super::{render, Invocation};

    fn argv(tokens: &[&str]) -> Vec<String> {
        tokens.iter().map(|token| (*token).to_owned()).collect()
    }

    /// The empty-argument arm has to be matched before the invocation-specific
    /// arms. If the `Invocation::Flag` arm were moved above it, a bare
    /// `--version` would start reporting "does not accept arguments"; if the
    /// `Invocation::Command` arm were moved above it, a bare `version` would
    /// panic in the `expect` that assumes a non-empty argument slice.
    #[test]
    fn no_arguments_renders_human_output_for_both_invocations() {
        let flag = render(Invocation::Flag, &[]).unwrap();
        let command = render(Invocation::Command, &[]).unwrap();
        assert_eq!(flag, command);
        assert!(flag.starts_with("semaprax "), "{flag}");
        assert!(flag.ends_with('\n'), "{flag}");
    }

    /// `--json` is a `version` subcommand argument only, is matched exactly,
    /// and takes no further arguments. The trailing-argument refusal names
    /// the first argument rather than the offending trailing one.
    #[test]
    fn json_is_admitted_only_as_the_sole_argument_of_the_version_command() {
        assert!(render(Invocation::Command, &argv(&["--json"]))
            .unwrap()
            .starts_with("{\"schema\":"));
        assert_eq!(
            render(Invocation::Flag, &argv(&["--json"])).unwrap_err(),
            "--version does not accept arguments"
        );
        for rejected in [
            &["--JSON"][..],
            &["json"][..],
            &["--json="][..],
            &["--json", "--json"][..],
        ] {
            assert!(
                render(Invocation::Command, &argv(rejected)).is_err(),
                "{rejected:?}"
            );
        }
        assert_eq!(
            render(Invocation::Command, &argv(&["--json", "extra"])).unwrap_err(),
            "unexpected version argument `--json`"
        );
    }
}
