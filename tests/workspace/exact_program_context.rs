//! Exact Program context regressions; authored, not executed locally.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

use semaprax::agent_definition::compile_agent_definition;
use semaprax::diagnostic::Diagnostic;
use semaprax::project::{
    render_project_lock, with_authenticated_project, ExactProgramContext, ImageArtifactKind,
    InterfaceArtifactFacts, ProgramRootV2, SemanticQuery, SemanticTransaction,
    SemanticTransactionRenameDisplayName, SemanticWorkspaceRevision, SemanticWorkspaceService,
    SemanticWorkspaceServiceHistoryQuery, SemanticWorkspaceStructuralDiff,
    MAX_IMAGE_ARTIFACT_BUILD_BYTES,
};
use serde_json::Value;

static SERIAL: AtomicU64 = AtomicU64::new(0);

struct Fixture(PathBuf);

impl Fixture {
    fn new() -> Self {
        let root = std::env::temp_dir().join(format!(
            "spx-exact-program-context-{}-{}",
            std::process::id(),
            SERIAL.fetch_add(1, Ordering::Relaxed),
        ));
        std::fs::create_dir_all(root.join("src")).unwrap();
        let sample = Path::new(env!("CARGO_MANIFEST_DIR")).join("examples/calculator-project");
        for file in [
            "semaprax.toml",
            "src/app.spx",
            "src/core.spx",
            "src/tests.spx",
        ] {
            std::fs::copy(sample.join(file), root.join(file)).unwrap();
        }
        Self(root.canonicalize().unwrap())
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn assert_code<T>(result: Result<T, Vec<Diagnostic>>, code: &str) {
    let errors = result.err().unwrap_or_else(|| panic!("expected {code}"));
    assert!(errors.iter().any(|error| error.code == code), "{errors:?}");
}

#[test]
fn context_exactly_binds_distinct_v1_roots_extensions_and_dual_selector() {
    let fixture = Fixture::new();
    let agent = super::program_root_v2::definition();
    with_authenticated_project(&fixture.0.join("semaprax.toml"), |snapshot| {
        let revision = snapshot.retain_revision();
        let default_workspace = snapshot.canonical_workspace_revision()?;
        let default_workspace_bytes = default_workspace.to_json().to_owned();
        let base_root = default_workspace.program_root()?;
        let base_root_bytes = base_root.to_json().to_owned();
        let lock = render_project_lock(snapshot)?;
        let association = base_root.associate_dependency_lock(
            snapshot,
            base_root.program_root_digest(),
            &lock,
        )?;
        let workspace = SemanticWorkspaceRevision::derive_with_agent_definitions(
            &revision,
            revision.project_revision(),
            &[&agent],
        )?;
        let workspace_root = workspace.program_root()?;
        assert_ne!(
            workspace_root.program_root_digest(),
            base_root.program_root_digest()
        );
        let facts = InterfaceArtifactFacts::derive(
            revision.clone(),
            revision.project_revision(),
            &[ImageArtifactKind::Web],
            MAX_IMAGE_ARTIFACT_BUILD_BYTES,
        )?;
        let v2 = ProgramRootV2::derive(&workspace, &base_root, &facts, &association)?;
        let v2_digest = v2.program_root_v2_digest().to_owned();
        let context = Arc::new(ExactProgramContext::derive(
            revision.clone(),
            revision.project_revision(),
            workspace.clone(),
            workspace.workspace_revision(),
            facts.clone(),
            association.clone(),
            v2,
            &v2_digest,
        )?);
        let other_agent = compile_agent_definition(
            &agent
                .definition()
                .canonical_source()
                .replace("root.agent", "other.agent"),
        )?;
        let other_workspace = SemanticWorkspaceRevision::derive_with_agent_definitions(
            &revision,
            revision.project_revision(),
            &[&other_agent],
        )?;
        let other_v2 = ProgramRootV2::derive(&other_workspace, &base_root, &facts, &association)?;
        assert_ne!(
            other_v2.program_root_v2_digest(),
            context.program_root_v2().program_root_v2_digest()
        );
        assert_eq!(context.base_project_root(), &base_root);
        assert_eq!(context.semantic_workspace_root(), &workspace_root);
        assert_eq!(
            context
                .select(
                    workspace.workspace_revision(),
                    context.program_root_v2().program_root_v2_digest()
                )?
                .to_json(),
            context.program_root_v2().to_json()
        );

        let service = SemanticWorkspaceService::open_exact(Arc::clone(&context))?;
        assert_eq!(
            service.active_generation().program_root_v2(),
            Some(context.program_root_v2())
        );
        let exact_snapshot = service.snapshot_exact(
            workspace.workspace_revision(),
            context.program_root_v2().program_root_v2_digest(),
        )?;
        assert_eq!(
            exact_snapshot.program_root_v2(),
            Some(context.program_root_v2())
        );
        let query = SemanticQuery::symbol(workspace.workspace_revision(), "calculator.add")?;
        let legacy_query = query.execute(&exact_snapshot)?;
        let exact_query = service.query_exact(
            query.to_json().as_bytes(),
            workspace.workspace_revision(),
            context.program_root_v2().program_root_v2_digest(),
        )?;
        assert_eq!(exact_query.to_json(), legacy_query.to_json());
        assert_eq!(
            exact_query.program_root_v2(),
            Some(context.program_root_v2())
        );
        assert_code(
            service.query_exact(
                b"{}",
                workspace.workspace_revision(),
                other_v2.program_root_v2_digest(),
            ),
            "SPX-G555",
        );
        let legacy_replayed_query = SemanticQuery::replay(
            &exact_snapshot,
            query.to_json().as_bytes(),
            exact_query.result_digest(),
            exact_query.to_json().as_bytes(),
        )?;
        assert_eq!(legacy_replayed_query.to_json(), exact_query.to_json());
        assert!(legacy_replayed_query.program_root_v2().is_none());
        let replayed_query = SemanticQuery::replay_exact(
            &exact_snapshot,
            query.to_json().as_bytes(),
            workspace.workspace_revision(),
            context.program_root_v2().program_root_v2_digest(),
            exact_query.result_digest(),
            exact_query.to_json().as_bytes(),
        )?;
        let service_replayed_query = service.replay_query_exact(
            query.to_json().as_bytes(),
            workspace.workspace_revision(),
            context.program_root_v2().program_root_v2_digest(),
            exact_query.result_digest(),
            exact_query.to_json().as_bytes(),
        )?;
        assert_eq!(replayed_query.to_json(), exact_query.to_json());
        assert_eq!(service_replayed_query.to_json(), exact_query.to_json());
        for replayed in [&replayed_query, &service_replayed_query] {
            let replayed_root = replayed
                .program_root_v2()
                .expect("exact query replay retains ProgramRoot v2");
            assert_eq!(
                replayed_root.program_root_v2_digest(),
                context.program_root_v2().program_root_v2_digest()
            );
            for kind in ["interface_artifact_facts", "project_lock_association"] {
                assert_eq!(
                    replayed_root.segment(kind),
                    context.program_root_v2().segment(kind),
                    "exact query replay changed `{kind}` segment identity"
                );
            }
        }
        let reminted = format!("sha256:{}", "0".repeat(64));
        assert_code(
            SemanticQuery::replay_exact(
                &exact_snapshot,
                query.to_json().as_bytes(),
                &reminted,
                context.program_root_v2().program_root_v2_digest(),
                exact_query.result_digest(),
                exact_query.to_json().as_bytes(),
            ),
            "SPX-G555",
        );
        assert_code(
            SemanticQuery::replay_exact(
                &exact_snapshot,
                b"{}",
                workspace.workspace_revision(),
                other_v2.program_root_v2_digest(),
                "not-a-digest",
                b"{}",
            ),
            "SPX-G555",
        );
        assert_code(
            SemanticQuery::replay_exact(
                &exact_snapshot,
                query.to_json().as_bytes(),
                workspace.workspace_revision(),
                &reminted,
                exact_query.result_digest(),
                exact_query.to_json().as_bytes(),
            ),
            "SPX-G555",
        );
        let mutated_result = exact_query
            .to_json()
            .replace("calculator.add", "calculator.sub");
        assert_ne!(mutated_result, exact_query.to_json());
        assert_code(
            SemanticQuery::replay_exact(
                &exact_snapshot,
                query.to_json().as_bytes(),
                workspace.workspace_revision(),
                context.program_root_v2().program_root_v2_digest(),
                exact_query.result_digest(),
                mutated_result.as_bytes(),
            ),
            "SPX-G533",
        );

        let transaction = SemanticTransaction::rename_display_name(
            default_workspace.workspace_revision(),
            SemanticTransactionRenameDisplayName::new("calculator.add", "add", "sum"),
        )?;
        let legacy_transaction = transaction.validate(revision.clone())?;
        let exact_transaction = service.validate_transaction_exact(
            transaction.to_json().as_bytes(),
            workspace.workspace_revision(),
            context.program_root_v2().program_root_v2_digest(),
        )?;
        assert_eq!(exact_transaction.result(), legacy_transaction.result());
        assert_eq!(exact_transaction.evidence(), legacy_transaction.evidence());
        assert_eq!(
            exact_transaction.base_program_root_v2(),
            Some(context.program_root_v2())
        );
        let replayed = SemanticTransaction::replay_exact(
            Arc::clone(&context),
            workspace.workspace_revision(),
            context.program_root_v2().program_root_v2_digest(),
            transaction.to_json().as_bytes(),
            exact_transaction.evidence().as_bytes(),
        )?;
        assert_eq!(replayed.evidence(), legacy_transaction.evidence());
        assert_eq!(
            replayed.base_program_root_v2(),
            Some(context.program_root_v2())
        );
        let service_replay = service.replay_transaction_exact(
            transaction.to_json().as_bytes(),
            exact_transaction.evidence().as_bytes(),
            workspace.workspace_revision(),
            context.program_root_v2().program_root_v2_digest(),
        )?;
        assert_eq!(service_replay.result(), legacy_transaction.result());
        assert_eq!(
            service_replay.base_program_root_v2(),
            Some(context.program_root_v2())
        );

        let history_query =
            SemanticWorkspaceServiceHistoryQuery::new(workspace.workspace_revision(), 0, 8)?;
        let legacy_history = service.history_query(history_query.to_json().as_bytes())?;
        let exact_history = service.history_query_exact(
            history_query.to_json().as_bytes(),
            workspace.workspace_revision(),
            context.program_root_v2().program_root_v2_digest(),
        )?;
        assert_code(
            service.history_query_exact(
                b"{}",
                workspace.workspace_revision(),
                other_v2.program_root_v2_digest(),
            ),
            "SPX-G555",
        );
        assert_eq!(exact_history.to_json(), legacy_history.to_json());
        assert_eq!(
            exact_history.result_digest(),
            legacy_history.result_digest()
        );
        assert_eq!(
            exact_history.program_root_v2(),
            Some(context.program_root_v2())
        );
        assert_eq!(exact_history.history_length(), 1);
        assert_eq!(
            exact_history.items()[0].base_workspace_revision(),
            default_workspace.workspace_revision()
        );
        let transaction_result: Value = serde_json::from_str(exact_transaction.result()).unwrap();
        assert_eq!(
            exact_history.items()[0].base_workspace_revision(),
            transaction_result["base"]["workspace_revision"]
                .as_str()
                .unwrap()
        );
        let exact_history_snapshot = service.history_snapshot_exact(
            workspace.workspace_revision(),
            context.program_root_v2().program_root_v2_digest(),
        )?;
        assert_eq!(
            exact_history_snapshot.program_root_v2(),
            Some(context.program_root_v2())
        );
        assert_eq!(exact_history_snapshot.len(), 1);
        let replayed_history = SemanticWorkspaceServiceHistoryQuery::replay(
            &exact_history_snapshot,
            history_query.to_json().as_bytes(),
            exact_history.result_digest(),
            exact_history.to_json().as_bytes(),
        )?;
        assert_eq!(replayed_history.to_json(), exact_history.to_json());
        assert_eq!(
            replayed_history.program_root_v2(),
            Some(context.program_root_v2())
        );

        let stale = format!("sha256:{}", "0".repeat(64));
        assert_code(
            service.replay_transaction_exact(
                transaction.to_json().as_bytes(),
                exact_transaction.evidence().as_bytes(),
                &stale,
                context.program_root_v2().program_root_v2_digest(),
            ),
            "SPX-G555",
        );
        assert_code(
            service.replay_transaction_exact(
                b"{}",
                b"{}",
                workspace.workspace_revision(),
                other_v2.program_root_v2_digest(),
            ),
            "SPX-G555",
        );
        assert_code(
            service.validate_transaction_exact(
                b"{}",
                workspace.workspace_revision(),
                other_v2.program_root_v2_digest(),
            ),
            "SPX-G555",
        );
        assert_code(
            service.replay_transaction_exact(
                transaction.to_json().as_bytes(),
                exact_transaction.evidence().as_bytes(),
                workspace.workspace_revision(),
                &stale,
            ),
            "SPX-G555",
        );
        let mut mutated_evidence = exact_transaction.evidence().as_bytes().to_vec();
        let last = mutated_evidence.len() - 2;
        mutated_evidence[last] ^= 1;
        assert_code(
            service.replay_transaction_exact(
                transaction.to_json().as_bytes(),
                &mutated_evidence,
                workspace.workspace_revision(),
                context.program_root_v2().program_root_v2_digest(),
            ),
            "SPX-G527",
        );
        assert_eq!(
            service
                .history_snapshot(workspace.workspace_revision())?
                .len(),
            1
        );
        let legacy_diff = SemanticWorkspaceStructuralDiff::derive(
            exact_transaction.candidate(),
            exact_transaction.candidate().candidate_digest(),
        )?;
        let exact_diff = SemanticWorkspaceStructuralDiff::derive_exact(
            exact_transaction.candidate(),
            exact_transaction.candidate().candidate_digest(),
            &context,
            workspace.workspace_revision(),
            context.program_root_v2().program_root_v2_digest(),
        )?;
        assert_eq!(exact_diff.to_json(), legacy_diff.to_json());
        assert_eq!(
            exact_diff.base_program_root_v2(),
            Some(context.program_root_v2())
        );
        assert_code(
            ProgramRootV2::replay(
                &workspace,
                &base_root,
                context.interface_artifact_facts(),
                context.dependency_lock_association(),
                context.program_root_v2().program_root_v2_digest(),
                context.program_root_v2().to_json().trim_end().as_bytes(),
            ),
            "SPX-G550",
        );
        let value: Value = serde_json::from_str(context.to_json()).unwrap();
        assert_eq!(
            value["base_project_root_digest"],
            base_root.program_root_digest()
        );
        assert_eq!(
            value["semantic_workspace_root_digest"],
            workspace_root.program_root_digest()
        );
        assert!(!context.to_json().contains(&lock));

        let assembled = ExactProgramContext::assemble(
            revision.clone(),
            revision.project_revision(),
            workspace.clone(),
            workspace.workspace_revision(),
            facts,
            association,
        )?;
        assert_eq!(assembled.to_json(), context.to_json());
        assert_code(
            context.select(&stale, context.program_root_v2().program_root_v2_digest()),
            "SPX-G555",
        );
        assert_code(
            context.select(workspace.workspace_revision(), &stale),
            "SPX-G555",
        );

        let empty = SemanticWorkspaceRevision::derive(&revision)?;
        let submitted_v2 = assembled.program_root_v2().clone();
        let submitted_v2_digest = submitted_v2.program_root_v2_digest().to_owned();
        assert_code(
            ExactProgramContext::derive(
                revision,
                snapshot.project_revision(),
                empty.clone(),
                empty.workspace_revision(),
                assembled.interface_artifact_facts().clone(),
                assembled.dependency_lock_association().clone(),
                submitted_v2,
                &submitted_v2_digest,
            ),
            "SPX-G554",
        );
        assert_eq!(
            snapshot.canonical_workspace_revision()?.to_json(),
            default_workspace_bytes
        );
        assert_eq!(
            snapshot
                .canonical_workspace_revision()?
                .program_root()?
                .to_json(),
            base_root_bytes
        );
        Ok(())
    })
    .unwrap();
}
