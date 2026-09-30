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
/// Schema selected only when at least one module has a function that opts
/// into endpoint typestate checking with `follows` (issue #297 follow-on,
/// R21) -- always a strict additional selection over [`SCHEMA_V2`], since a
/// `follows` clause names a protocol declared in the same module
/// (`SPX-K107` refuses anything else), so a workspace with at least one
/// `follows` binding already selected `SCHEMA_V2`.
pub(super) const SCHEMA_V3: &str = "semaprax.workspace-semantic-graph.v3";

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

/// Issue #297 follow-on (R21): every function's `follows` clause of `program`
/// as one canonical fact each, bound first against this same program's own
/// declarations (`source::bind_follows`) -- the workspace-level analogue of
/// `declaration_facts`'s own HIR binding. A no-op (empty result) for a
/// program with no `follows` clause.
pub(super) fn follows_facts(program: &Program) -> Result<Vec<String>, Vec<Diagnostic>> {
    if !program
        .functions
        .iter()
        .any(|function| function.follows.is_some())
    {
        return Ok(Vec::new());
    }
    source::bind_follows(program).map_err(|error| vec![error])?;
    Ok(program
        .functions
        .iter()
        .filter_map(source::follows_json)
        .collect())
}

/// `WORKSPACE_GRAPH_SCHEMA` when nothing was recorded, `SCHEMA_V2` when only
/// declarations were, `SCHEMA_V3` when at least one `follows` binding was.
pub(super) fn schema(
    recorded: &[(String, String, String)],
    follows_recorded: &[(String, String, String)],
) -> &'static str {
    if !follows_recorded.is_empty() {
        SCHEMA_V3
    } else if recorded.is_empty() {
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

/// The trailing `,"session_protocol_follows":{...}` fragment, or an empty
/// string when no `follows` binding was recorded -- so a workspace with no
/// `follows` clause anywhere (including a declaration-only, `SCHEMA_V2`
/// workspace) is byte-for-byte unaffected by this fact's existence.
/// `base_schema` names [`SCHEMA_V2`], the schema this section's own facts
/// extend, mirroring `render_trailing`'s own base-schema field one layer
/// down.
pub(super) fn render_follows_trailing(recorded: &[(String, String, String)]) -> String {
    if recorded.is_empty() {
        return String::new();
    }
    let bindings = recorded
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
        ",\"session_protocol_follows\":{{\"base_schema\":{},\"authority\":\"none\",\"bindings\":[{}]}}",
        quote_json(SCHEMA_V2),
        bindings
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

    // Issue #297 follow-on: semantic-workspace operations (rename, change)
    // build a candidate source set through the same two entry points these
    // tests call directly. A `via` clause binds by persistent `@id`, so a
    // display-name-only rename of its realizer must never break the binding,
    // and a change that removes the realizer entirely must never be silently
    // admitted -- both candidate builds replay the ordinary source checks
    // (`SPX-K104`) that already refuse a dangling `via`, and this pins that
    // fact at the exact entry points rename/change use, not only at
    // `declaration_facts` in isolation (the case above).

    /// A second, unrelated source: `build_owned` requires 2..32 files
    /// (`SPX-G170`), and this pair's own declaring module is the only source
    /// these two tests care about.
    fn companion_source() -> WorkspaceSource {
        source(
            "b/other.spx",
            "module session_protocol.fixture.other;\n\n\
             @id(\"fixture.session.other_main\")\nfn main() -> i64 { 0 }\n",
        )
    }

    #[test]
    fn a_via_bound_functions_display_rename_is_admitted_by_operations_and_change_candidate_builds()
    {
        let renamed = DECLARED_A
            .replace("fn begin() -> i64 { 1 }", "fn begin_v2() -> i64 { 1 }")
            .replace("begin() + commit()", "begin_v2() + commit()");
        assert_ne!(renamed, DECLARED_A);
        let sources = vec![source("a/declared.spx", &renamed), companion_source()];
        super::super::build_owned_retaining_sources_for_operations(
            sources.clone(),
            16 * 1024 * 1024,
            4 * 1024 * 1024,
        )
        .expect("a display-name rename of a via-bound function must not break its binding");
        super::super::build_owned_retaining_sources_for_change(sources, 4 * 1024 * 1024)
            .expect("a display-name rename of a via-bound function must not break its binding");
    }

    #[test]
    fn removing_a_via_bound_function_is_refused_with_a_stable_diagnostic_by_operations_and_change_candidate_builds(
    ) {
        let without_begin = DECLARED_A
            .replace(
                "@id(\"fixture.session.begin\")\nfn begin() -> i64 { 1 }\n\n",
                "",
            )
            .replace("begin() + commit()", "commit()");
        assert_ne!(without_begin, DECLARED_A);
        let sources = vec![source("a/declared.spx", &without_begin), companion_source()];
        // `WorkspaceGraphBuild`/`WorkspaceSource` are not `Debug`, so
        // `unwrap_err` cannot be used here; match directly instead.
        let operations_error = match super::super::build_owned_retaining_sources_for_operations(
            sources.clone(),
            16 * 1024 * 1024,
            4 * 1024 * 1024,
        ) {
            Err(errors) => errors,
            Ok(_) => panic!("a dangling via binding must not be silently admitted"),
        };
        assert!(operations_error
            .iter()
            .any(|error| error.code == "SPX-K104"));
        let change_error = match super::super::build_owned_retaining_sources_for_change(
            sources,
            4 * 1024 * 1024,
        ) {
            Err(errors) => errors,
            Ok(_) => panic!("a dangling via binding must not be silently admitted"),
        };
        assert!(change_error.iter().any(|error| error.code == "SPX-K104"));
    }

    // Issue #297 follow-on (R21): endpoint typestate `follows` bindings.

    const FOLLOWS_A: &str = include_str!("../session_protocol/tests/fixtures/follows.spx");
    const FOLLOWS_ENTRY: &str = "module session_protocol.fixture.follows_entry;\n\n\
use function @id(\"fixture.follows.main\") from session_protocol.fixture.follows as follows_main;\n\n\
@id(\"fixture.follows.entry_main\")\nfn main() -> i64 { follows_main() }\n";

    fn without_follows(source: &str) -> String {
        source.replacen(
            "\n    follows session protocol \"fixture.follows.protocol\"\n",
            "\n",
            1,
        )
    }

    fn snapshot_of_follows(declaring: &str) -> WorkspaceSemanticGraph {
        let sources = vec![
            source("a/follows.spx", declaring),
            source("b/follows_entry.spx", FOLLOWS_ENTRY),
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
                .project("session_protocol.fixture.follows_entry")
                .unwrap(),
        )
        .expect("authenticated projection must render")
    }

    #[test]
    fn a_follows_using_workspace_selects_v3_and_carries_a_module_bound_binding() {
        let graph = snapshot_of_follows(FOLLOWS_A);
        let value: serde_json::Value = serde_json::from_str(graph.to_json()).unwrap();
        assert_eq!(value["schema"], SCHEMA_V3);
        // Still carries the base v2 declaration fact this binding names.
        assert_eq!(
            value["session_protocols"]["declarations"][0]["stable_id"],
            "fixture.follows.protocol"
        );
        assert_eq!(value["session_protocol_follows"]["base_schema"], SCHEMA_V2);
        assert_eq!(value["session_protocol_follows"]["authority"], "none");
        let bindings = value["session_protocol_follows"]["bindings"]
            .as_array()
            .unwrap();
        assert_eq!(bindings.len(), 1);
        let fact = &bindings[0];
        assert_eq!(fact["module"], "session_protocol.fixture.follows");
        assert_eq!(fact["path"], "a/follows.spx");
        assert_eq!(fact["function"], "fixture.follows.main");
        assert_eq!(fact["protocol"], "fixture.follows.protocol");
        assert_eq!(fact["result"], "typestate_checked");
        assert_eq!(fact["authority"], "none");
    }

    #[test]
    fn a_declaring_workspace_without_a_follows_clause_keeps_v2_with_no_follows_key() {
        let graph = snapshot_of_follows(&without_follows(FOLLOWS_A));
        let value: serde_json::Value = serde_json::from_str(graph.to_json()).unwrap();
        assert_eq!(value["schema"], SCHEMA_V2);
        assert!(value.get("session_protocol_follows").is_none());
    }

    #[test]
    fn follows_facts_refuses_a_binding_naming_no_declared_protocol() {
        let source = "module x;\n\n\
@id(\"x.f\")\nfn f() -> i64\n    follows session protocol \"x.missing\"\n{ 0 }\n";
        let program = crate::parse(source, "x.spx").unwrap();
        let error = follows_facts(&program).unwrap_err();
        assert_eq!(error[0].code, "SPX-K107");
    }
}
