//! `semaprax.agent-skill.v1`: the version-matched Agent Skill bundle, and the
//! small, closed, authority-labeled public semantic workflow it publishes.
//!
//! See [`docs/AGENT-SKILL-BUNDLE-V1.md`](../docs/AGENT-SKILL-BUNDLE-V1.md) for
//! the full specification. This module is issue #196's landable slice of the
//! `SEMANTIC-DISCOVERY` package ([`crate::semantic_discovery`], issue #125):
//! it composes existing owners by schema and digest rather than restating
//! their content, exactly as that module's own documentation invites ("a
//! future skill-bundle entry belongs in this same catalog rather than as a
//! second one").
//!
//! [`generate_agent_skill_bundle`] is a pure function of the compiled-in
//! toolchain: unlike [`crate::semantic_discovery::generate_discovery_manifest`]
//! it is not bound to any one source file's revision, because a Skill bundle
//! describes the INSTALLED COMPILER, not one module. It takes no path
//! argument, performs no I/O, and returns byte-identical output on every call
//! against the same build. "Version-matched" here means bound to
//! `CARGO_PKG_VERSION` plus the optional `SEMAPRAX_BUILD_COMMIT`: a version
//! bump without regenerating the committed bundle
//! (`docs/AGENT-SKILL-BUNDLE-V1.json`) is caught by
//! `tests::committed_bundle_is_pinned_and_regenerates_byte_identical`, which
//! is this module's drift gate.
//!
//! [`negotiate_agent_skill_schema`] lets a caller (an SDK/MCP client) assert
//! the exact schema it was built against; it never falls back silently to an
//! older schema on mismatch.
//!
//! This module performs no host effect and grants no authority. Every entry
//! in [`PUBLIC_WORKFLOW`] names an existing top-level CLI command already
//! catalogued in `cli::help::COMMANDS` — the public workflow WRAPS that
//! surface, it does not fork it. `cli::agent::tests::public_workflow_commands_
//! are_all_catalogued` (binary-side, since the CLI catalog only exists in the
//! binary) is the drift gate for that mapping.

use serde_json::Value;
use sha2::{Digest, Sha256};

use crate::bounded_output::with_limit;
use crate::diagnostic::{quote_json, Diagnostic};
use crate::{installed_diagnostics, installed_guidance, semantic_discovery};

macro_rules! bformat {
    ($($argument:tt)*) => {
        crate::bounded_output::budgeted_format(format_args!($($argument)*))
    };
}

/// Schema identity of the version-matched Agent Skill bundle.
pub const AGENT_SKILL_SCHEMA: &str = "semaprax.agent-skill.v1";

/// Bounded output budget: the whole bundle must stay small enough that an
/// agent can hold it in full without paging, matching the spirit of
/// [`crate::semantic_discovery`]'s compactness measurement.
pub const MAX_AGENT_SKILL_BUNDLE_BYTES: usize = 64 * 1024;

const AGENT_SKILL_DOMAIN: &[u8] = b"semaprax.agent-skill.payload.digest.v1\0";

const STDLIB_CATALOG: &str = include_str!("../std/catalog.json");
const STDLIB_CATALOG_DOMAIN: &[u8] = b"semaprax.agent-skill.package-status.digest.v1\0";

fn capacity(message: &'static str) -> Vec<Diagnostic> {
    vec![Diagnostic::io("SPX-G586", message)]
}

fn invalid(message: String) -> Vec<Diagnostic> {
    vec![Diagnostic::io("SPX-G585", message)]
}

fn incompatible_schema(requested: &str) -> Diagnostic {
    Diagnostic::io(
        "SPX-G587",
        format!(
            "requested agent-skill schema `{requested}` does not match the \
             installed `{AGENT_SKILL_SCHEMA}`; there is no compatibility \
             fallback, request the exact schema this compiler serves"
        ),
    )
}

/// The closed, sorted set of authority classes a public workflow verb may
/// declare. Every verb in [`PUBLIC_WORKFLOW`] names exactly one of these, and
/// `tests::authority_classes_cover_exactly_the_used_set` checks the two
/// tables agree.
pub const AUTHORITY_CLASSES: &[&str] = &[
    "candidate_only",
    "publication",
    "read_only",
    "source_write",
    "test_execute",
];

/// The closed, sorted set of execution backends a safe program has
/// equivalent checked behavior on (`AGENTS.md`'s non-negotiable invariant).
pub const TARGET_PROFILES: &[&str] = &["core-wasm", "interpreter", "native-c11"];

/// One entry of the small, stable public command vocabulary: a public verb,
/// its authority class, and the existing canonical CLI command it wraps
/// without forking its semantics.
pub struct PublicWorkflowVerb {
    pub verb: &'static str,
    pub authority_class: &'static str,
    pub cli_command: &'static str,
    pub cli_usage: &'static str,
    pub note: &'static str,
}

/// The minimal public workflow required by issue #196: inspect, context,
/// propose/preview, impact, apply, repair, rebase, review, test, publish.
/// Sorted by `verb`; `tests::catalog_is_sorted_by_verb` enforces that.
///
/// Every `cli_command` here is an EXISTING top-level command already
/// catalogued in `cli::help::COMMANDS` (see that module's `DISPATCHER_
/// INVENTORY`); this table adds no new command surface and forks no
/// semantics, it only labels an existing command's authority class and
/// gives it one small, memorable public name.
pub const PUBLIC_WORKFLOW: &[PublicWorkflowVerb] = &[
    PublicWorkflowVerb {
        verb: "apply",
        authority_class: "source_write",
        cli_command: "apply-semantic-workspace-change-evidence",
        cli_usage: "semaprax apply-semantic-workspace-change-evidence <root> <proposal.json> <evidence.json>",
        note: "commits one evidence-gated proposal to source; requires a prior propose/preview and its exact evidence",
    },
    PublicWorkflowVerb {
        verb: "context",
        authority_class: "read_only",
        cli_command: "context",
        cli_usage: "semaprax context <file|project> <symbol|stable-id> [--direction forward|reverse|both] [--depth N] [--max-bytes N] [--max-nodes N] [--filters ...]",
        note: "one bounded, targeted question about one declaration; see AGENTS.md's own worked example",
    },
    PublicWorkflowVerb {
        verb: "impact",
        authority_class: "read_only",
        cli_command: "impact",
        cli_usage: "semaprax impact <file> <patch.spatch> [--depth N] [--max-bytes N] [--max-nodes N]",
        note: "bounded blast-radius of one candidate patch before requesting a preview",
    },
    PublicWorkflowVerb {
        verb: "inspect",
        authority_class: "read_only",
        cli_command: "graph",
        cli_usage: "semaprax graph <file>",
        note: "whole-module structural read; reserve for tools that need the complete document, prefer `context` otherwise",
    },
    PublicWorkflowVerb {
        verb: "propose",
        authority_class: "candidate_only",
        cli_command: "change",
        cli_usage: "semaprax change preview <project> <operation> ... [--evidence|--structural-diff]",
        note: "produces a candidate preview/evidence capsule; never writes source and never publishes",
    },
    PublicWorkflowVerb {
        verb: "publish",
        authority_class: "publication",
        cli_command: "project-candidate-git-publish",
        cli_usage: "semaprax project-candidate-git-publish <manifest> <capsule.json> <approved-candidate-digest> <host-policy.json>",
        note: "the one terminal step that publishes an already-approved candidate; requires an explicit prior approval digest",
    },
    PublicWorkflowVerb {
        verb: "rebase",
        authority_class: "candidate_only",
        cli_command: "change",
        cli_usage: "semaprax change rebase <base-project> <operation> ... --onto <onto-project> [--revision digest] [--onto-revision digest]",
        note: "re-targets a pending proposal onto a newer revision; still produces a candidate, never writes source",
    },
    PublicWorkflowVerb {
        verb: "repair",
        authority_class: "source_write",
        cli_command: "repair",
        cli_usage: "semaprax repair <file> <repair-id> --persistent-id <persistent-id>",
        note: "applies one installed, compiler-checked fix plan; writes source",
    },
    PublicWorkflowVerb {
        verb: "review",
        authority_class: "read_only",
        cli_command: "review",
        cli_usage: "semaprax review <file> <patch.spatch>",
        note: "read-only, bound to exact source and patch bytes; source drift fails closed",
    },
    PublicWorkflowVerb {
        verb: "test",
        authority_class: "test_execute",
        cli_command: "test",
        cli_usage: "semaprax test [<dir>|semaprax.toml|--manifest-path path] [--json] [--max-steps N] [--max-bytes N]",
        note: "executes the project's own compiler-checked tests; the only public verb that runs code",
    },
];

fn package_status() -> Result<Value, Vec<Diagnostic>> {
    let catalog: Value = serde_json::from_str(STDLIB_CATALOG)
        .map_err(|_| invalid("standard-library catalog is not valid JSON".to_owned()))?;
    let module_count = catalog["modules"]
        .as_array()
        .ok_or_else(|| invalid("standard-library catalog module inventory is invalid".to_owned()))?
        .len();
    let schema = catalog["schema"]
        .as_str()
        .ok_or_else(|| invalid("standard-library catalog is missing its schema".to_owned()))?
        .to_owned();
    Ok(serde_json::json!({
        "schema": schema,
        "digest": domain_digest(STDLIB_CATALOG_DOMAIN, STDLIB_CATALOG.as_bytes()),
        "module_count": module_count,
    }))
}

fn render_public_workflow() -> String {
    let mut entries = Vec::with_capacity(PUBLIC_WORKFLOW.len());
    for verb in PUBLIC_WORKFLOW {
        entries.push(bformat!(
            "{{\"verb\":{},\"authority_class\":{},\"cli_command\":{},\"cli_usage\":{},\"note\":{}}}",
            quote_json(verb.verb),
            quote_json(verb.authority_class),
            quote_json(verb.cli_command),
            quote_json(verb.cli_usage),
            quote_json(verb.note),
        ));
    }
    format!("[{}]", entries.join(","))
}

fn quoted_list(items: &[&str]) -> String {
    format!(
        "[{}]",
        items
            .iter()
            .map(|item| quote_json(item))
            .collect::<Vec<_>>()
            .join(",")
    )
}

fn domain_digest(domain: &[u8], bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(domain);
    hasher.update((bytes.len() as u64).to_le_bytes());
    hasher.update(bytes);
    format!(
        "sha256:{:x}",
        crate::digest_hex::LowerHex(hasher.finalize())
    )
}

const KNOWN_LIMITATIONS: &[&str] = &[
    "a_static_bundle_carries_no_authority_and_must_not_be_treated_as_a_live_capability_grant",
    "an_operation_or_command_listed_here_must_still_be_reconfirmed_against_the_installed_compiler_before_use",
    "does_not_replace_reading_the_owning_specification_for_the_syntax_protocol_or_command_it_names",
    "package_status_is_the_bundled_standard_library_catalog_not_an_ordinary_package_registry_inventory",
    "public_workflow_wraps_existing_commands_and_does_not_widen_or_narrow_their_admitted_semantics",
];

/// Generate the canonical `semaprax.agent-skill.v1` envelope for the
/// installed compiler.
///
/// Pure and deterministic: no source file, no filesystem beyond the
/// compiled-in `std/catalog.json` bytes, no network, no clock. Byte-identical
/// across repeated calls against the same build (`tests::generation_is_
/// byte_identical_on_repetition`).
pub fn generate_agent_skill_bundle() -> Result<String, Vec<Diagnostic>> {
    let compiler = installed_guidance::compiler()?;
    let query_capabilities = installed_guidance::installed_query_capabilities()?;
    let diagnostic_catalog = installed_diagnostics::installed_diagnostic_catalog()?;
    let package_status = package_status()?;

    let operations = semantic_discovery::render_operations_catalog(
        query_capabilities.schema(),
        query_capabilities.digest(),
        diagnostic_catalog.digest(),
        diagnostic_catalog.code_count(),
    );
    let public_workflow = render_public_workflow();
    let authority_classes = quoted_list(AUTHORITY_CLASSES);
    let target_profiles = quoted_list(TARGET_PROFILES);
    let limitations = quoted_list(KNOWN_LIMITATIONS);

    let (envelope, overflowed) = with_limit(MAX_AGENT_SKILL_BUNDLE_BYTES, || {
        let payload = bformat!(
            "{{\"schema\":\"{}\",\"authority\":false,\"compiler\":{},\
\"authority_classes\":{},\"target_profiles\":{},\"package_status\":{},\
\"discovery\":{{\"schema\":\"{}\",\"operations\":{}}},\
\"public_workflow\":{},\"known_limitations\":{}}}",
            AGENT_SKILL_SCHEMA,
            compiler,
            authority_classes,
            target_profiles,
            package_status,
            semantic_discovery::DISCOVERY_SCHEMA,
            operations,
            public_workflow,
            limitations,
        );
        bformat!(
            "{{\"schema\":\"{}\",\"digest\":{},\"bytes\":{},\"payload\":{}}}",
            AGENT_SKILL_SCHEMA,
            quote_json(&domain_digest(AGENT_SKILL_DOMAIN, payload.as_bytes())),
            payload.len(),
            payload,
        )
    });
    if overflowed {
        return Err(capacity("agent skill bundle exceeds its bounded budget"));
    }
    Ok(envelope)
}

/// Version negotiation for an SDK/MCP client: assert that `requested` names
/// exactly the schema this compiler serves. There is no compatibility
/// fallback to an older schema — a mismatch is always a hard error, never a
/// silent downgrade (one of the failure modes issue #196 explicitly calls
/// out).
pub fn negotiate_agent_skill_schema(requested: &str) -> Result<(), Diagnostic> {
    if requested == AGENT_SKILL_SCHEMA {
        Ok(())
    } else {
        Err(incompatible_schema(requested))
    }
}

#[cfg(test)]
mod tests;
