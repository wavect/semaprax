//! Issue #297 follow-on (R21): declared `.spx` session protocols projected
//! into the Workspace Semantic Graph.
//!
//! Recording happens inline in `build_owned_inner`'s per-module resolve loop,
//! where one module's own `Program` (with its `session_protocols`) and the
//! checked `hir::ResolvedProgram` built from that same program are both in
//! scope -- the identical pairing `verify_resolved_call_edges` already uses
//! for cross-file call binding. A thread-local carries the bound facts from
//! there to `render_graph_json`'s single rendering pass, so the workspace
//! graph's own digest and byte-budget accounting (both computed over that one
//! render) stay exactly self-consistent; nothing here re-renders or
//! double-hashes.
//!
//! A workspace with no declaring module records nothing, `render_graph_json`
//! keeps emitting `semaprax.workspace-semantic-graph.v1` unchanged, and the
//! output is byte-for-byte identical to a build of this module never having
//! existed. A declaring workspace selects `semaprax.workspace-semantic-graph.v2`
//! and gains one trailing `session_protocols` object, mirroring the
//! per-source graph's own `semaprax.graph.v48` gating
//! (`crate::graph::session_protocol_decl`). Every fact is bound to its owning
//! module and path, its `@id` and span, and the checked `via` functions of
//! that same module; legal order is still not authority, and every fact
//! carries `"authority":"none"`.

use std::cell::RefCell;

use crate::ast::Program;
use crate::diagnostic::{quote_json, Diagnostic};
use crate::hir::ResolvedProgram;
use crate::session_protocol::source;

/// Schema selected only when at least one module declares a session
/// protocol; otherwise `render_graph_json` keeps emitting
/// `WORKSPACE_GRAPH_SCHEMA` unchanged.
pub(super) const SCHEMA_V2: &str = "semaprax.workspace-semantic-graph.v2";

type Recorded = Vec<(String, String, String)>;

thread_local! {
    static RECORDED: RefCell<Recorded> = const { RefCell::new(Vec::new()) };
}

/// Clear any facts left by a prior build on this thread. Called once at the
/// top of `build_owned_inner`, the single funnel every builder entry point
/// shares, so a reused test-harness thread never carries a previous
/// workspace's declarations into this one.
pub(super) fn reset() {
    RECORDED.with(|cell| cell.borrow_mut().clear());
}

/// Bind and record one module's declared session protocols, in the same
/// per-module loop that already pairs `program` with its checked `resolved`
/// for `verify_resolved_call_edges`. A no-op for a module with no
/// declarations; otherwise fails closed exactly like the per-source graph
/// when a `via` names a function the checked HIR does not retain.
pub(super) fn record(program: &Program, resolved: &ResolvedProgram) -> Result<(), Vec<Diagnostic>> {
    if program.session_protocols.is_empty() {
        return Ok(());
    }
    source::bind_to_hir(program, resolved).map_err(|error| vec![error])?;
    RECORDED.with(|cell| {
        let mut recorded = cell.borrow_mut();
        for declaration in &program.session_protocols {
            recorded.push((
                program.module.clone(),
                program.path.clone(),
                source::declaration_json(declaration),
            ));
        }
    });
    Ok(())
}

/// A stable clone of everything recorded for the build in progress.
/// `render_graph_json` reads this once per rendering pass; the digest
/// fixed-point loop calls that function repeatedly with a placeholder and
/// then the final digest, and the recorded facts are already frozen before
/// that loop starts, so every read within one build is identical.
pub(super) fn recorded() -> Recorded {
    RECORDED.with(|cell| cell.borrow().clone())
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
    const ENTRY: &str = "module session_protocol.fixture.entry;\n\n\
@id(\"fixture.session.entry_main\")\nfn main() -> i64 { 0 }\n";

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
    fn recording_refuses_a_via_absent_from_checked_hir() {
        let program = crate::check(DECLARED_A, "declared.spx").unwrap();
        let mut resolved = crate::hir::resolve(&program).unwrap();
        resolved
            .functions
            .retain(|function| function.id.as_str() != "fixture.session.commit");
        let error = record(&program, &resolved).unwrap_err();
        assert_eq!(error[0].code, "SPX-K104");
    }
}
