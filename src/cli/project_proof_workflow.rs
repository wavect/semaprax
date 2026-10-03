//! Additive selected-law CLI over the shared checked workflow operation.
use semaprax::{
    agent_runtime::AgentCancellation,
    assurance_manifest::law_set::installed_workflow::{self, Request, SourceGoal, View},
    project::with_selected_law_diagnostics,
    proof_export::installed::{HostProfile, InstalledProofTool, Limits, ToolKind},
};
use std::{collections::BTreeMap, path::Path};

pub(crate) fn run(args: &[String]) -> Result<(), u8> {
    let Some(manifest) = args.first() else {
        return usage("absolute manifest required");
    };
    if !Path::new(manifest).is_absolute() {
        return usage("absolute manifest required");
    }
    let mut options = BTreeMap::new();
    let mut show_values = false;
    let mut cursor = 1;
    while cursor < args.len() {
        let key = args[cursor].as_str();
        if key == "--show-witness-values" {
            if show_values {
                return usage("duplicate option");
            }
            show_values = true;
            cursor += 1;
            continue;
        }
        if !matches!(
            key,
            "--workflow"
                | "--law"
                | "--tool"
                | "--executable"
                | "--version-line"
                | "--host-profile"
                | "--source"
                | "--declaration"
                | "--ensures"
                | "--offset"
                | "--limit"
                | "--max-bytes"
        ) {
            return usage("unknown workflow option");
        }
        let Some(value) = args.get(cursor + 1) else {
            return usage("option value missing");
        };
        if options.insert(key, value.as_str()).is_some() {
            return usage("duplicate option");
        }
        cursor += 2;
    }
    if [
        "--workflow",
        "--law",
        "--tool",
        "--executable",
        "--version-line",
        "--host-profile",
    ]
    .iter()
    .any(|key| !options.contains_key(key))
    {
        return usage("missing workflow selection or proof tool");
    }
    let detail = match options["--workflow"] {
        "summary" => false,
        "detail" => true,
        _ => return usage("workflow must be summary or detail"),
    };
    if show_values && !detail {
        return usage("witness values require detail view");
    }
    let selected_source = ["--source", "--declaration", "--ensures"]
        .iter()
        .filter(|key| options.contains_key(**key))
        .count();
    if selected_source != 0 && selected_source != 3 {
        return usage("source proof requires --source, --declaration and --ensures");
    }
    let source_goal = if selected_source == 3 {
        Some(SourceGoal {
            path: options["--source"],
            declaration: options["--declaration"],
            ensures_index: options["--ensures"].parse().map_err(|_| {
                eprintln!("project-proof-check: invalid postcondition index");
                2
            })?,
        })
    } else {
        None
    };
    let kind = match options["--tool"] {
        "z3" => ToolKind::Z3,
        "lean" => ToolKind::Lean,
        _ => return usage("tool must be z3 or lean"),
    };
    let profile = match options["--host-profile"] {
        "trusted-local" => HostProfile::TrustedLocal,
        "confined" => HostProfile::Confined,
        _ => return usage("unknown host profile"),
    };
    let offset = number(&options, "--offset", 0)?;
    let limit = number(&options, "--limit", 16)?;
    let max_bytes = number(&options, "--max-bytes", 65_536)?;
    let request = Request {
        law_id: options["--law"],
        source_goal,
        view: if detail {
            View::Detail
        } else {
            View::Summary { offset, limit }
        },
        max_bytes,
        show_witness_values: show_values,
        expected_candidate_revision: None,
    };
    let path = Path::new(manifest);
    let outcome = with_selected_law_diagnostics(path, |revision, laws, policy| {
        // Protected intent is authenticated before the process capability is acquired.
        let tool = InstalledProofTool::open(
            Path::new(options["--executable"]),
            path.parent().expect("absolute path has parent"),
            kind,
            options["--version-line"],
            profile,
            Limits::default(),
            AgentCancellation::new(),
        )
        .map_err(|error| vec![error])?;
        installed_workflow::check(revision, laws, policy, &tool, &request)
    })
    .map_err(|errors| {
        for error in errors {
            eprintln!("{}: {}", error.code, error.message);
        }
        1
    })?;
    println!("{}", outcome.document);
    if outcome.accepted {
        Ok(())
    } else {
        Err(1)
    }
}

fn number(options: &BTreeMap<&str, &str>, key: &str, default: usize) -> Result<usize, u8> {
    options.get(key).map_or(Ok(default), |raw| {
        raw.parse().map_err(|_| {
            eprintln!("project-proof-check: invalid {key}");
            2
        })
    })
}

fn usage(message: &str) -> Result<(), u8> {
    eprintln!("project-proof-check: {message}");
    Err(2)
}
