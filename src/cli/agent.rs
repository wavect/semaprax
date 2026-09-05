//! `semaprax agent inspect|run|replay`: the admitted agent lifecycle verbs.
//!
//! `inspect` compiles a canonical AgentDefinition v1 and prints its
//! deterministic AgentGraph v1, or the Agent Runtime Profile v1 projection with
//! `--profile`. `run` executes one task through the definition's derived
//! profile against a caller-supplied transcript of provider responses and tool
//! results, and `replay` re-runs that transcript and requires the recomputed
//! evidence to equal a supplied capsule byte for byte. Every verb is a pure
//! function of its input documents: no provider, tool, filesystem, process,
//! network, approval, or publication authority is granted, and `resume` and
//! `reconcile` stay unadmitted because the runtime claims no durable memory.

use std::path::{Path, PathBuf};

use semaprax::agent_definition;
use semaprax::agent_transcript;
use semaprax::diagnostic::Diagnostic;

const USAGE: &str = "agent accepts exactly `inspect <definition.json> [--profile]`, `run <definition.json> <task.json> <transcript.json> [--evidence|--trace]`, or `replay <definition.json> <task.json> <transcript.json> <evidence.json>`; `resume` and `reconcile` are not admitted";

pub(crate) enum RunOutput {
    Receipt,
    Evidence,
    Trace,
}

pub(crate) enum AgentCommand {
    Inspect {
        definition: PathBuf,
        profile: bool,
    },
    Run {
        definition: PathBuf,
        task: PathBuf,
        transcript: PathBuf,
        output: RunOutput,
    },
    Replay {
        definition: PathBuf,
        task: PathBuf,
        transcript: PathBuf,
        evidence: PathBuf,
    },
}

fn usage() -> u8 {
    eprintln!("{USAGE}");
    2
}

fn operand(value: &str) -> Result<PathBuf, u8> {
    if value.is_empty() || value.starts_with('-') {
        return Err(usage());
    }
    Ok(PathBuf::from(value))
}

pub(crate) fn parse(args: &[String]) -> Result<AgentCommand, u8> {
    let Some((subcommand, rest)) = args.split_first() else {
        return Err(usage());
    };
    match subcommand.as_str() {
        "inspect" => {
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
                    _ => return Err(usage()),
                }
            }
            Ok(AgentCommand::Inspect {
                definition: definition.ok_or_else(usage)?,
                profile,
            })
        }
        "run" => {
            let (paths, options): (Vec<&String>, Vec<&String>) =
                rest.iter().partition(|argument| !argument.starts_with('-'));
            if paths.len() != 3 || options.len() > 1 {
                return Err(usage());
            }
            let output = match options.first().map(|option| option.as_str()) {
                None => RunOutput::Receipt,
                Some("--evidence") => RunOutput::Evidence,
                Some("--trace") => RunOutput::Trace,
                Some(option) => {
                    eprintln!("unknown agent run option `{option}`");
                    return Err(2);
                }
            };
            Ok(AgentCommand::Run {
                definition: operand(paths[0])?,
                task: operand(paths[1])?,
                transcript: operand(paths[2])?,
                output,
            })
        }
        "replay" => match rest {
            [definition, task, transcript, evidence] => Ok(AgentCommand::Replay {
                definition: operand(definition)?,
                task: operand(task)?,
                transcript: operand(transcript)?,
                evidence: operand(evidence)?,
            }),
            _ => Err(usage()),
        },
        other => {
            eprintln!("unknown agent subcommand `{other}`; {USAGE}");
            Err(2)
        }
    }
}

fn read(path: &Path) -> Result<String, Vec<Diagnostic>> {
    std::fs::read_to_string(path).map_err(|error| {
        vec![Diagnostic::io(
            "SPX-I001",
            format!("cannot read {}: {error}", path.display()),
        )]
    })
}

pub(crate) fn run(command: &AgentCommand) -> Result<String, Vec<Diagnostic>> {
    match command {
        AgentCommand::Inspect {
            definition,
            profile,
        } => {
            let compiled = agent_definition::compile_agent_definition(&read(definition)?)?;
            Ok(if *profile {
                compiled.runtime_v1_profile().to_owned()
            } else {
                compiled.graph().canonical_json().to_owned()
            })
        }
        AgentCommand::Run {
            definition,
            task,
            transcript,
            output,
        } => {
            let scripted =
                agent_transcript::run(&read(definition)?, &read(task)?, &read(transcript)?)?;
            Ok(match output {
                RunOutput::Receipt => agent_transcript::run_receipt(&scripted),
                RunOutput::Evidence => scripted.run.evidence().to_owned(),
                RunOutput::Trace => scripted.run.trace().to_owned(),
            })
        }
        AgentCommand::Replay {
            definition,
            task,
            transcript,
            evidence,
        } => agent_transcript::replay(
            &read(definition)?,
            &read(task)?,
            &read(transcript)?,
            &read(evidence)?,
        ),
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
        } = parse(&strings(&["inspect", "agent.json"])).unwrap()
        else {
            panic!("inspect")
        };
        assert_eq!(definition, PathBuf::from("agent.json"));
        assert!(!profile);
        let AgentCommand::Inspect { profile, .. } =
            parse(&strings(&["inspect", "--profile", "agent.json"])).unwrap()
        else {
            panic!("inspect")
        };
        assert!(profile);
        let AgentCommand::Run { output, .. } =
            parse(&strings(&["run", "a.json", "t.json", "x.json"])).unwrap()
        else {
            panic!("run")
        };
        assert!(matches!(output, RunOutput::Receipt));
        let AgentCommand::Run {
            output, transcript, ..
        } = parse(&strings(&[
            "run",
            "--evidence",
            "a.json",
            "t.json",
            "x.json",
        ]))
        .unwrap()
        else {
            panic!("run")
        };
        assert!(matches!(output, RunOutput::Evidence));
        assert_eq!(transcript, PathBuf::from("x.json"));
        assert!(matches!(
            parse(&strings(&["run", "a.json", "t.json", "x.json", "--trace"])).unwrap(),
            AgentCommand::Run {
                output: RunOutput::Trace,
                ..
            }
        ));
        assert!(matches!(
            parse(&strings(&[
                "replay", "a.json", "t.json", "x.json", "e.json"
            ]))
            .unwrap(),
            AgentCommand::Replay { .. }
        ));
        for malformed in [
            &[][..],
            &["inspect"][..],
            &["resume", "agent.json"][..],
            &["reconcile", "agent.json"][..],
            &["inspect", "--unknown", "agent.json"][..],
            &["inspect", "agent.json", "extra"][..],
            &["inspect", "agent.json", "--profile", "--profile"][..],
            &["run", "a.json", "t.json"][..],
            &["run", "a.json", "t.json", "x.json", "y.json"][..],
            &["run", "a.json", "t.json", "x.json", "--evidence", "--trace"][..],
            &["run", "a.json", "t.json", "x.json", "--json"][..],
            &["replay", "a.json", "t.json", "x.json"][..],
            &["replay", "a.json", "t.json", "x.json", "--evidence"][..],
        ] {
            assert!(parse(&strings(malformed)).is_err(), "{malformed:?}");
        }
    }
}
