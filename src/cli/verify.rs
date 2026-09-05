//! `semaprax verify`: one entry to every independent evidence verifier.
//!
//! The last operand is always the capsule to replay. Its `schema` selects the
//! verifier; the preceding operands are that verifier's subject and change,
//! exactly as the long-form `verify-*` routes take them. The front adds no
//! verification of its own and grants no authority: it reads the capsule once
//! to select, then hands the same paths to the owning verifier, which re-reads
//! and independently replays them. An unrecognized or unreadable capsule fails
//! closed before any verifier runs.

use std::path::{Path, PathBuf};

use semaprax::diagnostic::{quote_json, Diagnostic};
use semaprax::{
    agent_definition, patch_evidence, semantic_workspace_change, semantic_workspace_operations,
    semantic_workspace_structural_change, workspace_patch_evidence,
};

/// Largest capsule the front reads to select a verifier. Every admitted
/// capsule family caps its own bytes far below this.
const MAX_CAPSULE_BYTES: u64 = 16 * 1024 * 1024;

const USAGE: &str =
    "verify requires exactly <subject> <change> <capsule.json> or <manifest> <image.json>";

/// The admitted capsule schemas, their operand count, and their verifier.
pub(crate) const ROUTES: &[(&str, usize, &str)] = &[
    (
        "semaprax.semantic-patch-evidence.v1",
        3,
        "verify-patch-evidence",
    ),
    (
        "semaprax.semantic-patch-evidence.v2",
        3,
        "verify-patch-evidence-v2",
    ),
    (
        "semaprax.semantic-workspace-patch-evidence.v1",
        3,
        "verify-workspace-patch-evidence",
    ),
    (
        "semaprax.workspace-semantic-change-evidence.v1",
        3,
        "verify-semantic-workspace-change-evidence",
    ),
    (
        "semaprax.workspace-semantic-structural-change-evidence.v1",
        3,
        "verify-semantic-workspace-structural-change-evidence",
    ),
    (
        "semaprax.semantic-workspace-operations-evidence.v1",
        3,
        "verify-semantic-workspace-operations-evidence",
    ),
    ("semaprax.agent-graph.v1", 3, "agent graph bundle"),
    (
        "semaprax.semantic-workspace-image.v1",
        2,
        "project-image-verify",
    ),
];

pub(crate) struct VerifyOptions {
    pub(crate) operands: Vec<PathBuf>,
}

pub(crate) fn parse(args: &[String]) -> Result<VerifyOptions, u8> {
    if !(2..=3).contains(&args.len())
        || args
            .iter()
            .any(|argument| argument.is_empty() || argument.starts_with('-'))
    {
        eprintln!("{USAGE}");
        return Err(2);
    }
    Ok(VerifyOptions {
        operands: args.iter().map(PathBuf::from).collect(),
    })
}

fn capsule_schema(path: &Path) -> Result<String, Diagnostic> {
    let metadata = std::fs::metadata(path).map_err(|error| {
        Diagnostic::io(
            "SPX-V202",
            format!("cannot read capsule {}: {error}", path.display()),
        )
    })?;
    if metadata.len() > MAX_CAPSULE_BYTES {
        return Err(Diagnostic::io(
            "SPX-V202",
            format!(
                "capsule {} is {} bytes; verify reads at most {MAX_CAPSULE_BYTES}",
                path.display(),
                metadata.len()
            ),
        ));
    }
    let text = std::fs::read_to_string(path).map_err(|error| {
        Diagnostic::io(
            "SPX-V202",
            format!("cannot read capsule {}: {error}", path.display()),
        )
    })?;
    let value: serde_json::Value = serde_json::from_str(&text).map_err(|_| {
        Diagnostic::io(
            "SPX-V202",
            format!("capsule {} is not a JSON document", path.display()),
        )
    })?;
    value
        .get("schema")
        .and_then(serde_json::Value::as_str)
        .map(str::to_owned)
        .ok_or_else(|| {
            Diagnostic::io(
                "SPX-V202",
                format!(
                    "capsule {} has no top-level string `schema`",
                    path.display()
                ),
            )
        })
}

fn unrecognized(path: &Path, schema: &str, operands: usize) -> Diagnostic {
    let admitted = ROUTES
        .iter()
        .filter(|(_, count, _)| *count == operands)
        .map(|(schema, _, _)| format!("`{schema}`"))
        .collect::<Vec<_>>()
        .join(", ");
    Diagnostic::io(
        "SPX-V201",
        format!(
            "capsule {} declares `{schema}`, which no verifier taking {operands} operands admits; admitted: {admitted}",
            path.display()
        ),
    )
}

/// Select the verifier by the capsule schema and run it. The receipt the
/// verifier prints is returned unchanged.
pub(crate) fn run(
    options: &VerifyOptions,
    image_verify: impl Fn(&Path, &Path) -> Result<String, Vec<Diagnostic>>,
) -> Result<String, Vec<Diagnostic>> {
    let operands = &options.operands;
    let capsule = operands.last().expect("parse admits two or three operands");
    let schema = capsule_schema(capsule).map_err(|error| vec![error])?;
    let route = ROUTES
        .iter()
        .find(|(admitted, count, _)| *admitted == schema && *count == operands.len())
        .ok_or_else(|| vec![unrecognized(capsule, &schema, operands.len())])?;
    match route.2 {
        "verify-patch-evidence" => patch_evidence::verify(&operands[0], &operands[1], capsule),
        "verify-patch-evidence-v2" => {
            patch_evidence::verify_v2(&operands[0], &operands[1], capsule)
        }
        "verify-workspace-patch-evidence" => {
            workspace_patch_evidence::verify(&operands[0], &operands[1], capsule)
        }
        "verify-semantic-workspace-change-evidence" => {
            semantic_workspace_change::verify(&operands[0], &operands[1], capsule)
        }
        "verify-semantic-workspace-structural-change-evidence" => {
            semantic_workspace_structural_change::verify(&operands[0], &operands[1], capsule)
        }
        "verify-semantic-workspace-operations-evidence" => {
            semantic_workspace_operations::verify(&operands[0], &operands[1], capsule)
        }
        "agent graph bundle" => agent_bundle(&operands[0], &operands[1], capsule),
        "project-image-verify" => image_verify(&operands[0], capsule),
        _ => unreachable!("closed verify route table"),
    }
}

fn read(path: &Path) -> Result<String, Vec<Diagnostic>> {
    std::fs::read_to_string(path).map_err(|error| {
        vec![Diagnostic::io(
            "SPX-V202",
            format!("cannot read {}: {error}", path.display()),
        )]
    })
}

/// Independently recompile the definition and compare the supplied profile
/// and graph bytes, then print a receipt naming the verified identities.
fn agent_bundle(
    definition: &Path,
    profile: &Path,
    graph: &Path,
) -> Result<String, Vec<Diagnostic>> {
    let definition_source = read(definition)?;
    let profile_source = read(profile)?;
    let graph_source = read(graph)?;
    agent_definition::verify_agent_graph_bundle(
        &definition_source,
        &profile_source,
        &graph_source,
    )?;
    let compiled = agent_definition::compile_agent_definition(&definition_source)?;
    Ok(format!(
        "{{\"schema\":\"semaprax.agent-graph-verification.v1\",\"agent_id\":{},\"definition_digest\":{},\"graph_digest\":{},\"verified\":true,\"authority\":false}}\n",
        quote_json(compiled.definition().agent_id()),
        quote_json(compiled.definition().digest()),
        quote_json(compiled.graph().digest())
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn strings(values: &[&str]) -> Vec<String> {
        values.iter().map(|value| (*value).to_owned()).collect()
    }

    #[test]
    fn verify_grammar_is_closed() {
        assert_eq!(parse(&strings(&["a", "b"])).unwrap().operands.len(), 2);
        assert_eq!(parse(&strings(&["a", "b", "c"])).unwrap().operands.len(), 3);
        for malformed in [
            &[][..],
            &["a"][..],
            &["a", "b", "c", "d"][..],
            &["a", "--json", "c"][..],
            &["a", "", "c"][..],
        ] {
            assert!(parse(&strings(malformed)).is_err(), "{malformed:?}");
        }
    }

    #[test]
    fn route_table_is_closed_and_unique() {
        let mut schemas: Vec<_> = ROUTES.iter().map(|(schema, _, _)| *schema).collect();
        schemas.sort_unstable();
        schemas.dedup();
        assert_eq!(schemas.len(), ROUTES.len());
        assert!(ROUTES.iter().all(|(_, count, _)| (2..=3).contains(count)));
    }
}
