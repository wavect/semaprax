//! Dispatch for the retention-metadata and project draft/candidate
//! persistence commands. These live outside the driver root so adding a
//! persistence surface does not grow a budgeted module.
use std::path::Path;

use super::{cli, report};

pub(super) fn retention_metadata_plan(command: &str, args: &[String]) -> Result<(), u8> {
    if args.len() != 9
        || args[1..]
            .iter()
            .any(|argument| argument.is_empty() || argument.starts_with('-'))
    {
        eprintln!("{command} requires its exact positional operands; see --help");
        return Err(2);
    }
    let no_previous = args[6] == "none";
    if no_previous != (args[7] == "none") || (no_previous && args[8] != "none") {
        eprintln!("{command} requires a previous checkpoint file and selector together");
        return Err(2);
    }
    let output = cli::retention_metadata::plan(cli::retention_metadata::PlanOptions {
        inventory: Path::new(&args[1]),
        sequence: &args[2],
        max_subjects: &args[3],
        max_bytes: &args[4],
        protected_generations: &args[5],
        previous_checkpoint: (!no_previous).then(|| Path::new(&args[6])),
        expected_previous: (!no_previous).then_some(args[7].as_str()),
        expected_previous_predecessor: (args[8] != "none").then_some(args[8].as_str()),
    })
    .map_err(|errors| report(&errors, false))?;
    print!("{output}");
    Ok(())
}

pub(super) fn retention_metadata_persist_or_load(command: &str, args: &[String]) -> Result<(), u8> {
    let arity = if command == "retention-metadata-persist" {
        7
    } else {
        5
    };
    if args.len() != arity
        || args[1..]
            .iter()
            .any(|argument| argument.is_empty() || argument.starts_with('-'))
    {
        eprintln!("{command} requires its exact positional operands; see --help");
        return Err(2);
    }
    let previous = (args[4] != "none").then_some(args[4].as_str());
    let output = if command == "retention-metadata-persist" {
        cli::retention_metadata::persist(
            Path::new(&args[1]),
            Path::new(&args[2]),
            &args[3],
            previous,
            Path::new(&args[5]),
            &args[6],
        )
    } else {
        let previous = (args[3] != "none").then_some(args[3].as_str());
        cli::retention_metadata::load(Path::new(&args[1]), &args[2], previous, &args[4])
    }
    .map_err(|errors| report(&errors, false))?;
    print!("{output}");
    Ok(())
}

pub(super) fn project_draft_persist_or_load(command: &str, args: &[String]) -> Result<(), u8> {
    if args.len() != 4
        || args[1..]
            .iter()
            .any(|argument| argument.is_empty() || argument.starts_with('-'))
    {
        let operands = if command == "project-draft-persist" {
            "<manifest> <draft-capsule.json> <store-root>"
        } else {
            "<store-root> <archive-digest> <draft-digest>"
        };
        eprintln!("{command} requires exactly {operands}");
        return Err(2);
    }
    let output = if command == "project-draft-persist" {
        cli::draft_archive::persist(
            Path::new(&args[1]),
            Path::new(&args[2]),
            Path::new(&args[3]),
        )
    } else {
        cli::draft_archive::load(Path::new(&args[1]), &args[2], &args[3])
    }
    .map_err(|errors| report(&errors, false))?;
    print!("{output}");
    Ok(())
}

pub(super) fn project_candidate_persist_or_load(command: &str, args: &[String]) -> Result<(), u8> {
    if args.len() != 4
        || args[1..]
            .iter()
            .any(|argument| argument.is_empty() || argument.starts_with('-'))
    {
        let operands = if command == "project-candidate-persist" {
            "<manifest> <capsule.json> <store-root>"
        } else {
            "<store-root> <archive-digest> <candidate-digest>"
        };
        eprintln!("{command} requires exactly {operands}");
        return Err(2);
    }
    let output = if command == "project-candidate-persist" {
        cli::candidate_archive::persist(
            Path::new(&args[1]),
            Path::new(&args[2]),
            Path::new(&args[3]),
        )
    } else {
        cli::candidate_archive::load(Path::new(&args[1]), &args[2], &args[3])
    }
    .map_err(|errors| report(&errors, false))?;
    print!("{output}");
    Ok(())
}
