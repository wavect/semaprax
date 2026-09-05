//! `semaprax agent inspect <definition.json> [--profile]`: compile one
//! canonical AgentDefinition v1 and print its deterministic AgentGraph v1, or
//! the byte-preserved Agent Runtime Profile v1 projection with `--profile`.
//!
//! Inspection is pure. It grants no provider, tool, filesystem, process,
//! network, or publication authority, and it runs nothing.

use std::path::PathBuf;

use semaprax::agent_definition;
use semaprax::diagnostic::Diagnostic;

const USAGE: &str = "agent accepts exactly `inspect <definition.json> [--profile]`";

pub(crate) enum AgentCommand {
    Inspect { definition: PathBuf, profile: bool },
}

pub(crate) fn parse(args: &[String]) -> Result<AgentCommand, u8> {
    let Some((subcommand, rest)) = args.split_first() else {
        eprintln!("{USAGE}");
        return Err(2);
    };
    if subcommand != "inspect" {
        eprintln!("unknown agent subcommand `{subcommand}`; {USAGE}");
        return Err(2);
    }
    let mut definition = None;
    let mut profile = false;
    for argument in rest {
        match argument.as_str() {
            "--profile" if !profile => profile = true,
            "--profile" => {
                eprintln!("duplicate agent inspect option --profile");
                return Err(2);
            }
            option if option.starts_with('-') => {
                eprintln!("unknown agent inspect option `{option}`");
                return Err(2);
            }
            path if definition.is_none() => definition = Some(PathBuf::from(path)),
            _ => {
                eprintln!("{USAGE}");
                return Err(2);
            }
        }
    }
    let definition = definition.ok_or_else(|| {
        eprintln!("{USAGE}");
        2
    })?;
    Ok(AgentCommand::Inspect {
        definition,
        profile,
    })
}

pub(crate) fn run(command: &AgentCommand) -> Result<String, Vec<Diagnostic>> {
    match command {
        AgentCommand::Inspect {
            definition,
            profile,
        } => {
            let source = std::fs::read_to_string(definition).map_err(|error| {
                vec![Diagnostic::io(
                    "SPX-I001",
                    format!("cannot read {}: {error}", definition.display()),
                )]
            })?;
            let compiled = agent_definition::compile_agent_definition(&source)?;
            Ok(if *profile {
                compiled.runtime_v1_profile().to_owned()
            } else {
                compiled.graph().canonical_json().to_owned()
            })
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn strings(values: &[&str]) -> Vec<String> {
        values.iter().map(|value| (*value).to_owned()).collect()
    }

    #[test]
    fn agent_grammar_is_closed() {
        let AgentCommand::Inspect {
            definition,
            profile,
        } = parse(&strings(&["inspect", "agent.json"])).unwrap();
        assert_eq!(definition, PathBuf::from("agent.json"));
        assert!(!profile);
        let AgentCommand::Inspect { profile, .. } =
            parse(&strings(&["inspect", "--profile", "agent.json"])).unwrap();
        assert!(profile);
        for malformed in [
            &[][..],
            &["inspect"][..],
            &["run", "agent.json"][..],
            &["inspect", "--unknown", "agent.json"][..],
            &["inspect", "agent.json", "extra"][..],
            &["inspect", "agent.json", "--profile", "--profile"][..],
        ] {
            assert!(parse(&strings(malformed)).is_err(), "{malformed:?}");
        }
    }
}
