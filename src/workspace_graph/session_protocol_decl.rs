//! Issue #297 follow-on (R21): declared `.spx` session protocols projected
//! into the Workspace Semantic Graph.
//!
//! `declaration_facts` runs inside `retain_workspace_module`, in the same
//! per-module resolve step that already pairs one module's own `Program`
//! (with its `session_protocols`) with the checked `hir::ResolvedProgram`
//! built from that same program -- the identical pairing
//! `verify_resolved_call_edges` uses for cross-file call binding. The bound
//! facts become part of that module's own retained data
//! (`WorkspaceResolvedModule::session_protocol_facts`), so they flow through
//! the existing pipeline exactly like every other per-module fact: pruned to
//! the entry module's reachable closure by
//! `AuthenticatedWorkspaceGraphBuild::project`, then read once by
//! `render_graph_json` from `WorkspaceGraphProjectionModule`. Nothing here
//! is global, thread-local, or re-rendered; the workspace graph's own digest
//! and byte-budget accounting (both computed over that one render) stay
//! exactly self-consistent.
//!
//! A workspace with no declaring module contributes no facts,
//! `render_graph_json` keeps emitting `semaprax.workspace-semantic-graph.v1`
//! unchanged, and the output is byte-for-byte identical to a build of this
//! module never having existed. A declaring workspace selects
//! `semaprax.workspace-semantic-graph.v2` and gains one trailing
//! `session_protocols` object, mirroring the per-source graph's own
//! `semaprax.graph.v48` gating (`crate::graph::session_protocol_decl`).
//! Every fact is bound to its owning module and path, its `@id` and span,
//! and the checked `via` functions of that same module; legal order is
//! still not authority, and every fact carries `"authority":"none"`.

use crate::ast::Program;
use crate::diagnostic::{quote_json, Diagnostic};
use crate::hir::ResolvedProgram;
use crate::session_protocol::source;

/// Schema selected only when at least one module declares a session
/// protocol; otherwise `render_graph_json` keeps emitting
/// `WORKSPACE_GRAPH_SCHEMA` unchanged.
pub(super) const SCHEMA_V2: &str = "semaprax.workspace-semantic-graph.v2";

/// Bind one module's declared session protocols against its own checked HIR
/// and return one canonical fact per declaration, in source order. A no-op
/// (empty result) for a module with no declarations; otherwise fails closed
/// exactly like the per-source graph when a `via` names a function the
/// checked HIR does not retain.
pub(super) fn declaration_facts(
    program: &Program,
    resolved: &ResolvedProgram,
) -> Result<Vec<String>, Vec<Diagnostic>> {
    if program.session_protocols.is_empty() {
        return Ok(Vec::new());
    }
    source::bind_to_hir(program, resolved).map_err(|error| vec![error])?;
    Ok(program
        .session_protocols
        .iter()
        .map(source::declaration_json)
        .collect())
}

/// `WORKSPACE_GRAPH_SCHEMA` when nothing was recorded, `SCHEMA_V2` otherwise.
pub(super) fn schema(recorded: &[(String, String, String)]) -> &'static str {
    if recorded.is_empty() {
        super::WORKSPACE_GRAPH_SCHEMA
    } else {
        SCHEMA_V2
    }
}

/// The trailing `,"session_protocols":{...}` fragment, or an empty string
/// when nothing was recorded -- so a protocol-free workspace's rendered
/// bytes are unaffected by this module's existence.
pub(super) fn render_trailing(recorded: &[(String, String, String)]) -> String {
    if recorded.is_empty() {
        return String::new();
    }
    let declarations = recorded
        .iter()
        .map(|(module, path, fact)| {
            format!(
                "{{\"module\":{},\"path\":{},{}",
                quote_json(module),
                quote_json(path),
                &fact[1..]
            )
        })
        .collect::<Vec<_>>()
        .join(",");
    format!(
        ",\"session_protocols\":{{\"base_schema\":{},\"authority\":\"none\",\"declarations\":[{}]}}",
        quote_json(super::WORKSPACE_GRAPH_SCHEMA),
        declarations
    )
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use super::super::{
        build_owned, AuthenticatedSourceFact, AuthenticatedWorkspaceGraphBuild,
        AuthenticatedWorkspaceStorageUsage, WorkspaceSemanticGraph, WorkspaceSource,
    };
    use super::*;

    const DECLARED_A: &str = include_str!("../session_protocol/tests/fixtures/declared.spx");
    /// Imports the declaring module's own `begin` function, so the entry
    /// module's reachable closure actually includes the declaring module --
    /// the Workspace Semantic Graph prunes every fact, session-protocol or
    /// otherwise, to that closure.
    const ENTRY: &str = "module session_protocol.fixture.entry;\n\n\
use function @id(\"fixture.session.begin\") from session_protocol.fixture.declared as begin;\n\n\
@id(\"fixture.session.entry_main\")\nfn main() -> i64 { begin() }\n";

    fn without_declaration(source: &str) -> String {
        let start = source.find("@id(\"fixture.session.transaction\")").unwrap();
        source[..start].to_owned()
    }

    fn source(path: &str, text: &str) -> WorkspaceSource {
        let program = crate::parse(text, std::path::Path::new(path)).expect("fixture parses");
        WorkspaceSource {
            path: path.to_owned(),
            source: crate::format::canonical(&program),
        }
    }

    fn snapshot_of(declaring: &str) -> WorkspaceSemanticGraph {
        let sources = vec![
            source("a/declared.spx", declaring),
            source("b/entry.spx", ENTRY),
        ];
        let source_facts = sources
            .iter()
            .map(|source| {
                (
                    source.path.clone(),
                    AuthenticatedSourceFact {
                        path: source.path.clone(),
                        source_graph_schema: "semaprax.semantic-graph.v14".to_owned(),
                        source_revision: format!("revision:{}", source.path),
                        source_digest: format!("sha256:{:064x}", source.source.len()),
                    },
                )
            })
            .collect::<BTreeMap<_, _>>();
        let authenticated = AuthenticatedWorkspaceGraphBuild {
            workspace_revision: "sha256:workspace".to_owned(),
            sources: source_facts,
            storage: AuthenticatedWorkspaceStorageUsage {
                manifest_bytes: 1,
                retained_generations: 1,
                staging_attempts: 1,
                unexpected_inventory_entries: 0,
            },
            graph: build_owned(sources).expect("fixture workspace must validate"),
        };
        super::super::render_semantic_graph(
            authenticated
                .project("session_protocol.fixture.entry")
                .unwrap(),
        )
        .expect("authenticated projection must render")
    }

    #[test]
    fn a_declaring_workspace_selects_v2_and_carries_a_module_bound_fact() {
        let graph = snapshot_of(DECLARED_A);
        let value: serde_json::Value = serde_json::from_str(graph.to_json()).unwrap();
        assert_eq!(value["schema"], SCHEMA_V2);
        assert_eq!(
            value["session_protocols"]["base_schema"],
            "semaprax.workspace-semantic-graph.v1"
        );
        assert_eq!(value["session_protocols"]["authority"], "none");
        let declarations = value["session_protocols"]["declarations"]
            .as_array()
            .unwrap();
        assert_eq!(declarations.len(), 1);
        let fact = &declarations[0];
        assert_eq!(fact["module"], "session_protocol.fixture.declared");
        assert_eq!(fact["path"], "a/declared.spx");
        assert_eq!(fact["stable_id"], "fixture.session.transaction");
        assert_eq!(fact["authority"], "none");
    }

    #[test]
    fn a_protocol_free_workspace_keeps_v1_with_no_session_protocols_key() {
        let graph = snapshot_of(&without_declaration(DECLARED_A));
        let value: serde_json::Value = serde_json::from_str(graph.to_json()).unwrap();
        assert_eq!(value["schema"], "semaprax.workspace-semantic-graph.v1");
        assert!(value.get("session_protocols").is_none());
    }

    #[test]
    fn rendering_is_deterministic_across_independent_builds() {
        let first = snapshot_of(DECLARED_A);
        let second = snapshot_of(DECLARED_A);
        assert_eq!(first.to_json(), second.to_json());
        assert_eq!(first.graph_digest(), second.graph_digest());
    }

    #[test]
    fn a_mutated_declaration_changes_the_fact_and_the_digest() {
        let base = snapshot_of(DECLARED_A);
        let mutated_source = DECLARED_A.replace(
            "on Idle misuse: fail Unit -> Failed;",
            "on Idle misuse: cancel Unit -> Failed;",
        );
        let mutated = snapshot_of(&mutated_source);
        assert_ne!(base.to_json(), mutated.to_json());
        assert_ne!(base.graph_digest(), mutated.graph_digest());
    }

    #[test]
    fn an_unreachable_declaring_module_contributes_no_fact() {
        // The declaring module is never imported by the entry module here,
        // so it is pruned from the reachable closure before rendering --
        // exactly like every other per-module fact (`declarations`, `edges`)
        // this graph already prunes to the entry's reachable modules.
        let sources = vec![
            source("a/declared.spx", DECLARED_A),
            source(
                "b/entry.spx",
                "module session_protocol.fixture.entry;\n\n\
                 @id(\"fixture.session.entry_main\")\nfn main() -> i64 { 0 }\n",
            ),
        ];
        let source_facts = sources
            .iter()
            .map(|source| {
                (
                    source.path.clone(),
                    AuthenticatedSourceFact {
                        path: source.path.clone(),
                        source_graph_schema: "semaprax.semantic-graph.v14".to_owned(),
                        source_revision: format!("revision:{}", source.path),
                        source_digest: format!("sha256:{:064x}", source.source.len()),
                    },
                )
            })
            .collect::<BTreeMap<_, _>>();
        let authenticated = AuthenticatedWorkspaceGraphBuild {
            workspace_revision: "sha256:workspace".to_owned(),
            sources: source_facts,
            storage: AuthenticatedWorkspaceStorageUsage {
                manifest_bytes: 1,
                retained_generations: 1,
                staging_attempts: 1,
                unexpected_inventory_entries: 0,
            },
            graph: build_owned(sources).expect("fixture workspace must validate"),
        };
        let graph = super::super::render_semantic_graph(
            authenticated
                .project("session_protocol.fixture.entry")
                .unwrap(),
        )
        .expect("authenticated projection must render");
        let value: serde_json::Value = serde_json::from_str(graph.to_json()).unwrap();
        assert_eq!(value["schema"], "semaprax.workspace-semantic-graph.v1");
        assert!(value.get("session_protocols").is_none());
    }

    #[test]
    fn declaration_facts_refuses_a_via_absent_from_checked_hir() {
        let program = crate::check(DECLARED_A, "declared.spx").unwrap();
        let mut resolved = crate::hir::resolve(&program).unwrap();
        resolved
            .functions
            .retain(|function| function.id.as_str() != "fixture.session.commit");
        let error = declaration_facts(&program, &resolved).unwrap_err();
        assert_eq!(error[0].code, "SPX-K104");
    }
}
