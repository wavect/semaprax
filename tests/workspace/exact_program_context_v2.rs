//! Additive ExactProgramContext v2 selection and replay regressions.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

use semaprax::diagnostic::Diagnostic;
use semaprax::project::{
    render_project_lock, with_authenticated_project, ExactProgramContext, ExactProgramContextV2,
    ImageArtifactKind, InterfaceArtifactFacts, ProgramRootV2, SemanticQuery, SemanticTransaction,
    SemanticTransactionRenameDisplayName, SemanticWorkspaceRevision, SemanticWorkspaceService,
    SemanticWorkspaceServiceHistoryQuery, EXACT_PROGRAM_CONTEXT_V2_SCHEMA,
    MAX_EXACT_PROGRAM_CONTEXT_V2_BYTES, MAX_IMAGE_ARTIFACT_BUILD_BYTES,
};
use serde_json::Value;
use sha2::{Digest, Sha256};

static SERIAL: AtomicU64 = AtomicU64::new(0);
const CONTEXT_V2_DOMAIN: &[u8] = b"semaprax.exact-program-context.digest.v2\0";

struct Fixture(PathBuf);

impl Fixture {
    fn new() -> Self {
        let root = std::env::temp_dir().join(format!(
            "spx-exact-program-context-v2-{}-{}",
            std::process::id(),
            SERIAL.fetch_add(1, Ordering::Relaxed),
        ));
        std::fs::create_dir_all(root.join("src")).unwrap();
        let source = Path::new(env!("CARGO_MANIFEST_DIR")).join("examples/calculator-project");
        for file in [
            "semaprax.toml",
            "src/app.spx",
            "src/core.spx",
            "src/tests.spx",
        ] {
            std::fs::copy(source.join(file), root.join(file)).unwrap();
        }
        Self(root.canonicalize().unwrap())
    }

    fn context(&self) -> Arc<ExactProgramContext> {
        with_authenticated_project(&self.0.join("semaprax.toml"), |snapshot| {
            let revision = snapshot.retain_revision();
            let default_workspace = snapshot.canonical_workspace_revision()?;
            let base_root = default_workspace.program_root()?;
            let lock = render_project_lock(snapshot)?;
            let association = base_root.associate_dependency_lock(
                snapshot,
                base_root.program_root_digest(),
                &lock,
            )?;
            let workspace = SemanticWorkspaceRevision::derive_with_agent_definitions(
                &revision,
                revision.project_revision(),
                &[&super::program_root_v2::definition()],
            )?;
            let interface = InterfaceArtifactFacts::derive(
                revision.clone(),
                revision.project_revision(),
                &[ImageArtifactKind::Web],
                MAX_IMAGE_ARTIFACT_BUILD_BYTES,
            )?;
            let v2 = ProgramRootV2::derive(&workspace, &base_root, &interface, &association)?;
            let v2_digest = v2.program_root_v2_digest().to_owned();
            ExactProgramContext::derive(
                revision.clone(),
                revision.project_revision(),
                workspace.clone(),
                workspace.workspace_revision(),
                interface,
                association,
                v2,
                &v2_digest,
            )
            .map(Arc::new)
        })
        .unwrap()
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

fn canonical(mut value: Value) -> String {
    value.sort_all_objects();
    serde_json::to_string(&value).unwrap() + "\n"
}

fn remint(value: &mut Value) -> String {
    value.as_object_mut().unwrap().remove("context_v2_digest");
    let identity = canonical(value.clone());
    let mut digest = Sha256::new();
    digest.update(CONTEXT_V2_DOMAIN);
    digest.update((identity.len() as u64).to_le_bytes());
    digest.update(identity.as_bytes());
    value["context_v2_digest"] = Value::String(format!(
        "sha256:{:x}",
        semaprax::digest_hex::LowerHex(digest.finalize())
    ));
    canonical(value.clone())
}

#[test]
fn v2_replays_v1_facts_and_v3_without_changing_prior_bytes() {
    let context_v1 = Fixture::new().context();
    let v1_bytes = context_v1.to_json().to_owned();
    let root_v1_bytes = context_v1.semantic_workspace_root().to_json().to_owned();
    let root_v2_bytes = context_v1.program_root_v2().to_json().to_owned();
    let context_v2 = ExactProgramContextV2::assemble(Arc::clone(&context_v1)).unwrap();

    assert_eq!(context_v2.exact_program_context_v1().to_json(), v1_bytes);
    assert_eq!(
        context_v2.semantic_workspace_program_root_v1().to_json(),
        root_v1_bytes
    );
    assert_eq!(context_v2.program_root_v2().to_json(), root_v2_bytes);
    assert_eq!(
        context_v2
            .program_root_v3()
            .segment("contracts_and_tests_facts")
            .unwrap()
            .node_digest(),
        context_v2.contracts_and_tests_facts().facts_digest()
    );
    assert_eq!(
        context_v2
            .select(
                context_v1.semantic_workspace().workspace_revision(),
                context_v2.program_root_v3().program_root_v3_digest(),
            )
            .unwrap(),
        context_v2.program_root_v3()
    );
    let value: Value = serde_json::from_str(context_v2.to_json()).unwrap();
    assert_eq!(value["schema"], EXACT_PROGRAM_CONTEXT_V2_SCHEMA);

    let replayed = ExactProgramContextV2::replay(
        Arc::clone(context_v2.exact_program_context_v1_arc()),
        context_v2.contracts_and_tests_facts().clone(),
        context_v2.program_root_v3().clone(),
        context_v1.semantic_workspace().workspace_revision(),
        context_v2.program_root_v3().program_root_v3_digest(),
        context_v2.context_v2_digest(),
        context_v2.to_json().as_bytes(),
    )
    .unwrap();
    assert_eq!(replayed.to_json(), context_v2.to_json());
    assert_eq!(context_v1.to_json(), v1_bytes);
    assert_eq!(context_v1.program_root_v2().to_json(), root_v2_bytes);
}

#[test]
fn v2_fails_closed_on_selectors_malformed_reminted_and_over_bound_bytes() {
    let context_v1 = Fixture::new().context();
    let context_v2 = ExactProgramContextV2::assemble(Arc::clone(&context_v1)).unwrap();
    let stale = format!("sha256:{}", "0".repeat(64));

    assert_code(
        ExactProgramContextV2::replay(
            Arc::clone(context_v2.exact_program_context_v1_arc()),
            context_v2.contracts_and_tests_facts().clone(),
            context_v2.program_root_v3().clone(),
            &stale,
            context_v2.program_root_v3().program_root_v3_digest(),
            "not-a-digest",
            b"{}",
        ),
        "SPX-G577",
    );
    assert_code(
        context_v2.select(context_v1.semantic_workspace().workspace_revision(), &stale),
        "SPX-G577",
    );

    let mut malformed: Value = serde_json::from_str(context_v2.to_json()).unwrap();
    malformed["unknown"] = Value::Bool(false);
    let malformed = canonical(malformed);
    assert_code(
        ExactProgramContextV2::replay(
            Arc::clone(context_v2.exact_program_context_v1_arc()),
            context_v2.contracts_and_tests_facts().clone(),
            context_v2.program_root_v3().clone(),
            context_v1.semantic_workspace().workspace_revision(),
            context_v2.program_root_v3().program_root_v3_digest(),
            context_v2.context_v2_digest(),
            malformed.as_bytes(),
        ),
        "SPX-G576",
    );

    let mut forged: Value = serde_json::from_str(context_v2.to_json()).unwrap();
    forged["contracts_and_tests_facts_digest"] = Value::String(stale);
    let forged = remint(&mut forged);
    let forged_value: Value = serde_json::from_str(&forged).unwrap();
    let forged_digest = forged_value["context_v2_digest"].as_str().unwrap();
    assert_code(
        ExactProgramContextV2::replay(
            Arc::clone(context_v2.exact_program_context_v1_arc()),
            context_v2.contracts_and_tests_facts().clone(),
            context_v2.program_root_v3().clone(),
            context_v1.semantic_workspace().workspace_revision(),
            context_v2.program_root_v3().program_root_v3_digest(),
            forged_digest,
            forged.as_bytes(),
        ),
        "SPX-G577",
    );

    let over_bound = vec![b' '; MAX_EXACT_PROGRAM_CONTEXT_V2_BYTES + 1];
    assert_code(
        ExactProgramContextV2::replay(
            Arc::clone(context_v2.exact_program_context_v1_arc()),
            context_v2.contracts_and_tests_facts().clone(),
            context_v2.program_root_v3().clone(),
            context_v1.semantic_workspace().workspace_revision(),
            context_v2.program_root_v3().program_root_v3_digest(),
            context_v2.context_v2_digest(),
            &over_bound,
        ),
        "SPX-G576",
    );
}

#[test]
fn v2_service_query_transaction_and_history_preserve_v1_wires_and_root_precedence() {
    let first = Fixture::new();
    let context_v1 = first.context();
    let context_v1_bytes = context_v1.to_json().to_owned();
    let context_v2 = Arc::new(ExactProgramContextV2::assemble(Arc::clone(&context_v1)).unwrap());
    let workspace = context_v1.semantic_workspace().workspace_revision();
    let v2_digest = context_v1.program_root_v2().program_root_v2_digest();
    let v3_digest = context_v2.program_root_v3().program_root_v3_digest();
    let service = SemanticWorkspaceService::open_exact_v2(Arc::clone(&context_v2)).unwrap();

    assert_eq!(
        service.active_generation().program_root_v2(),
        Some(context_v2.program_root_v2())
    );
    assert_eq!(
        service.active_generation().program_root_v3(),
        Some(context_v2.program_root_v3())
    );
    let old_snapshot = service.snapshot_exact(workspace, v2_digest).unwrap();
    assert_eq!(
        old_snapshot.program_root_v2(),
        Some(context_v2.program_root_v2())
    );
    assert!(old_snapshot.program_root_v3().is_none());
    let snapshot = service.snapshot_exact_v2(workspace, v3_digest).unwrap();

    let query = SemanticQuery::symbol(workspace, "calculator.add").unwrap();
    let ordinary = service.query(query.to_json().as_bytes()).unwrap();
    let old_exact = service
        .query_exact(query.to_json().as_bytes(), workspace, v2_digest)
        .unwrap();
    let exact = service
        .query_exact_v2(query.to_json().as_bytes(), workspace, v3_digest)
        .unwrap();
    assert_eq!(ordinary.to_json(), exact.to_json());
    assert_eq!(old_exact.to_json(), exact.to_json());
    assert!(ordinary.program_root_v2().is_none());
    assert!(ordinary.program_root_v3().is_none());
    assert_eq!(
        old_exact.program_root_v2(),
        Some(context_v2.program_root_v2())
    );
    assert!(old_exact.program_root_v3().is_none());
    assert_eq!(exact.program_root_v2(), Some(context_v2.program_root_v2()));
    assert_eq!(exact.program_root_v3(), Some(context_v2.program_root_v3()));
    let replayed = SemanticQuery::replay_exact_v2(
        &snapshot,
        query.to_json().as_bytes(),
        workspace,
        v3_digest,
        exact.result_digest(),
        exact.to_json().as_bytes(),
    )
    .unwrap();
    let replayed_service = service
        .replay_query_exact_v2(
            query.to_json().as_bytes(),
            workspace,
            v3_digest,
            exact.result_digest(),
            exact.to_json().as_bytes(),
        )
        .unwrap();
    for result in [&replayed, &replayed_service] {
        assert_eq!(result.program_root_v2(), Some(context_v2.program_root_v2()));
        assert_eq!(result.program_root_v3(), Some(context_v2.program_root_v3()));
    }

    let transaction = SemanticTransaction::rename_display_name(
        context_v2.base_program_root_v1().workspace_revision(),
        SemanticTransactionRenameDisplayName::new("calculator.add", "add", "sum"),
    )
    .unwrap();
    let legacy = transaction
        .validate(Arc::clone(context_v1.revision()))
        .unwrap();
    let artifacts = service
        .validate_transaction_exact_v2(transaction.to_json().as_bytes(), workspace, v3_digest)
        .unwrap();
    assert_eq!(artifacts.evidence(), legacy.evidence());
    assert_eq!(
        artifacts.base_program_root_v2(),
        Some(context_v2.program_root_v2())
    );
    assert_eq!(
        artifacts.base_program_root_v3(),
        Some(context_v2.program_root_v3())
    );
    let replayed_transaction = service
        .replay_transaction_exact_v2(
            transaction.to_json().as_bytes(),
            artifacts.evidence().as_bytes(),
            workspace,
            v3_digest,
        )
        .unwrap();
    assert_eq!(
        replayed_transaction.base_program_root_v2(),
        Some(context_v2.program_root_v2())
    );
    assert_eq!(
        replayed_transaction.base_program_root_v3(),
        Some(context_v2.program_root_v3())
    );

    let history_query = SemanticWorkspaceServiceHistoryQuery::new(workspace, 0, 8).unwrap();
    let ordinary_history = service
        .history_query(history_query.to_json().as_bytes())
        .unwrap();
    let old_history = service
        .history_query_exact(history_query.to_json().as_bytes(), workspace, v2_digest)
        .unwrap();
    let history = service
        .history_query_exact_v2(history_query.to_json().as_bytes(), workspace, v3_digest)
        .unwrap();
    assert_eq!(ordinary_history.to_json(), history.to_json());
    assert_eq!(old_history.to_json(), history.to_json());
    assert!(ordinary_history.program_root_v2().is_none());
    assert!(ordinary_history.program_root_v3().is_none());
    assert_eq!(
        old_history.program_root_v2(),
        Some(context_v2.program_root_v2())
    );
    assert!(old_history.program_root_v3().is_none());
    assert_eq!(
        history.program_root_v2(),
        Some(context_v2.program_root_v2())
    );
    assert_eq!(
        history.program_root_v3(),
        Some(context_v2.program_root_v3())
    );
    assert_eq!(history.history_length(), 1);
    assert_eq!(
        history.items()[0].base_workspace_revision(),
        context_v2.base_program_root_v1().workspace_revision()
    );

    let second = Fixture::new();
    let second_source_path = second.0.join("src/core.spx");
    let second_source = std::fs::read_to_string(&second_source_path)
        .unwrap()
        .replacen("left + right", "left - right", 1);
    std::fs::write(second_source_path, second_source).unwrap();
    let second_context = ExactProgramContextV2::assemble(second.context()).unwrap();
    assert_ne!(
        second_context.program_root_v3().program_root_v3_digest(),
        v3_digest
    );
    let stale = format!("sha256:{}", "0".repeat(64));
    for bad_v3 in [
        stale.as_str(),
        second_context.program_root_v3().program_root_v3_digest(),
    ] {
        assert_code(
            service.validate_transaction_exact_v2(b"{}", workspace, bad_v3),
            "SPX-G577",
        );
    }
    assert_code(
        service.replay_query_exact_v2(b"{}", workspace, &stale, "not-a-digest", b"{}"),
        "SPX-G577",
    );
    assert_eq!(
        service
            .history_snapshot_exact_v2(workspace, v3_digest)
            .unwrap()
            .len(),
        1
    );
    assert_eq!(context_v1.to_json(), context_v1_bytes);
}
