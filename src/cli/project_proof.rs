//! Explicit installed proof execution for one exact retained source obligation.
use semaprax::{
    agent_runtime::AgentCancellation,
    assurance_manifest::project::{
        generate_from_snapshot_with_verified_proofs, ProjectAssuranceOptions,
    },
    project::with_authenticated_project,
    proof_export::{
        installed::{HostProfile, InstalledProofTool, Limits, ToolKind},
        installed_project::prove_postcondition,
    },
};
use std::{collections::BTreeMap, path::Path};

pub(crate) fn run(args: &[String]) -> Result<(), u8> {
    let Some(manifest) = args.first() else {
        return usage("a manifest is required");
    };
    let mut values = BTreeMap::new();
    let mut cursor = 1;
    while cursor < args.len() {
        let key = args[cursor].as_str();
        if ![
            "--tool",
            "--executable",
            "--version-line",
            "--host-profile",
            "--source",
            "--declaration",
            "--ensures",
        ]
        .contains(&key)
        {
            return usage("unknown option");
        }
        let Some(value) = args.get(cursor + 1) else {
            return usage("option value missing");
        };
        if values.insert(key, value.as_str()).is_some() {
            return usage("duplicate option");
        }
        cursor += 2;
    }
    if values.len() != 7 {
        return usage("every explicit proof-tool option is required");
    }
    let kind = match values["--tool"] {
        "lean" => ToolKind::Lean,
        "z3" => ToolKind::Z3,
        _ => return usage("tool must be lean or z3"),
    };
    let profile = match values["--host-profile"] {
        "trusted-local" => HostProfile::TrustedLocal,
        "confined" => HostProfile::Confined,
        _ => return usage("unknown host profile"),
    };
    let index: usize = values["--ensures"].parse().map_err(|_| {
        eprintln!("project-proof-check: invalid postcondition index");
        2
    })?;
    let path = Path::new(manifest);
    let cwd = path.parent().ok_or_else(|| {
        eprintln!("project-proof-check: absolute manifest required");
        2
    })?;
    if !path.is_absolute() {
        return usage("absolute manifest required");
    }
    // Validate every option before acquiring any execution capability.
    let tool = InstalledProofTool::open(
        Path::new(values["--executable"]),
        cwd,
        kind,
        values["--version-line"],
        profile,
        Limits::default(),
        AgentCancellation::new(),
    )
    .map_err(|error| {
        eprintln!("{}: {}", error.code, error.message);
        1
    })?;
    let output = with_authenticated_project(path, |snapshot| {
        let revision = snapshot.retain_revision();
        let proof = prove_postcondition(&revision, values["--source"], values["--declaration"], index, &tool)?;
        let assurance = generate_from_snapshot_with_verified_proofs(snapshot, &ProjectAssuranceOptions::default(), &[proof])?;
        Ok(serde_json::json!({
            "schema":"semaprax.installed-project-proof-check.v1", "project_assurance":serde_json::from_str::<serde_json::Value>(&assurance).expect("derived JSON"),
            "toolchain":tool.expected_version(), "host_profile":"trusted_local",
            "limits":{"wall_time_ms":10_000,"version_time_ms":2_000,"input_bytes":65_536,"output_wire_bytes":65_536,"process_runs":16},
            "memory_limit_enforced":false,"filesystem_confined":false,"network_confined":false,
            "application_executed":false,"publication_authority":false,
            "nonclaims":["one_exact_source_postcondition_not_complete_law_coverage","source_proof_not_proved_lowering","trusted_tool_and_translation","no_escaped_descendant_confinement"]
        }).to_string())
    }).map_err(|errors| { for error in errors { eprintln!("{}: {}", error.code, error.message); } 1 })?;
    println!("{output}");
    Ok(())
}

fn usage(message: &str) -> Result<(), u8> {
    eprintln!("project-proof-check: {message}");
    Err(2)
}
