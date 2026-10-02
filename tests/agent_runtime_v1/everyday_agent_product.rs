//! Everyday Agent end-to-end validation product (issue ABI-09A.17).
//!
//! Two independently real slices of one bounded scenario, composed from
//! existing, already-shipped APIs:
//!
//! 1. The source-declared `everyday.agent` Agent
//!    (`examples/everyday-agent-project/src/agent.spx`) driven through the
//!    existing durable checkpoint/crash-recovery machinery
//!    (`semaprax::agent_lifecycle::DurableAgent`), with its one external
//!    read backed by this test's own real `std::fs::read` of the project's
//!    real fixture file. That read stands in for "the host executes the
//!    explicitly selected generated consumer": it is a validation-harness
//!    substitute for a public-generic provider call, not a public-generic
//!    call itself. See `examples/everyday-agent-project/README.md` "Scope
//!    and nonclaims" for exactly what this product does and does not
//!    claim.
//! 2. The real typed-filesystem-and-bounded-JSON workflow
//!    (`everyday.manifest.review-ok`, the project's `[command]`) executed
//!    for real against a scoped, real, temporary directory through the
//!    existing `semaprax::project::with_authenticated_project` /
//!    `execute_filesystem_command` Project route — the same route
//!    `tests/project/standard_library/filesystem_v2.rs` uses for `std.fs`
//!    itself. This proves genuine fs.read, JSON validation/classification,
//!    and a real, bounded, no-clobber fs.write, end to end.

use std::fs;
use std::path::{Path, PathBuf};
#[cfg(unix)]
use std::time::{SystemTime, UNIX_EPOCH};

use semaprax::agent_deployment::{bind_agent_deployment, migrate_agent_definition_v1};
use semaprax::agent_lifecycle::{
    bind_durable_agent, AgentCheckpoint, AgentReadOperation, AuthorizedRequest, CheckpointStore,
    CheckpointStoreError, CrashPoint, DurableAgent, DurableBudget, DurableStatus, LifecycleTask,
    Reconciliation, Retention,
};
use semaprax::agent_runtime::AgentCancellation;
use semaprax::project::{compile_source_agent_declaration, with_authenticated_project};

const AGENT_ID: &str = "everyday.agent";
const DEPLOYMENT_ID: &str = "everyday.deployment.local";

fn project_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("examples/everyday-agent-project")
}

fn agent_module_path() -> PathBuf {
    project_root().join("src/agent.spx")
}

fn agent_module_source() -> String {
    fs::read_to_string(agent_module_path())
        .expect("examples/everyday-agent-project/src/agent.spx is checked in")
}

fn fixture_input_bytes() -> Vec<u8> {
    fs::read(project_root().join("fixtures/input.json"))
        .expect("examples/everyday-agent-project/fixtures/input.json is checked in")
}

fn durable_agent_from_source(source: &str, policy_epoch: u64) -> DurableAgent {
    let checked = semaprax::check(source, "agent.spx").unwrap();
    let declaration = checked
        .agents
        .iter()
        .find(|declaration| declaration.stable_id == AGENT_ID)
        .expect("everyday.agent is declared in agent.spx");
    let compiled_definition = compile_source_agent_declaration(declaration).unwrap();
    let (definition_v2, deployment) = migrate_agent_definition_v1(
        compiled_definition.definition().canonical_source(),
        DEPLOYMENT_ID,
    )
    .unwrap();
    let bound = bind_agent_deployment(&definition_v2, &deployment).unwrap();
    bind_durable_agent(source, "agent.spx", &bound, policy_epoch).unwrap()
}

fn durable_agent(policy_epoch: u64) -> DurableAgent {
    durable_agent_from_source(&agent_module_source(), policy_epoch)
}

fn task() -> LifecycleTask {
    LifecycleTask {
        objective: b"everyday-manifest-review".to_vec(),
        budget: 12,
    }
}

fn proposal(schema_digest: &str, budget: &str, sequence: &str) -> String {
    format!(
        concat!(
            "{{\"schema\":\"semaprax.agent-proposal.v1\",\"agent_id\":\"everyday.agent\",",
            "\"proposal_schema_digest\":\"{digest}\",\"value\":{{\"fields\":{{",
            "\"everyday.agent.type.proposal.budget\":\"{budget}\",",
            "\"everyday.agent.type.proposal.urgent\":false,",
            "\"everyday.agent.type.proposal.sequence\":\"{sequence}\"}}}}}}\n"
        ),
        digest = schema_digest,
        budget = budget,
        sequence = sequence,
    )
}

fn document(agent: &DurableAgent) -> String {
    proposal(
        agent.lifecycle().proposal_schema().schema().digest(),
        "5",
        "1",
    )
}

/// The single registered external read. It stands in for the host boundary
/// that would otherwise call a generated public-generic consumer; here it
/// performs one real, bounded `std::fs::read` of the project's own fixture,
/// so the Agent's `reduce` stage carries genuinely observed bytes rather
/// than a fixed placeholder.
struct Read {
    value: Vec<u8>,
    fails: bool,
    calls: usize,
}

impl Read {
    fn new() -> Self {
        let value = fixture_input_bytes();
        assert!(
            value.len() <= 65536,
            "fixture must stay inside the product's own bound"
        );
        Self {
            value,
            fails: false,
            calls: 0,
        }
    }

    fn failing() -> Self {
        Self {
            fails: true,
            ..Self::new()
        }
    }
}

impl AgentReadOperation for Read {
    fn read(&mut self, request: &AuthorizedRequest) -> Option<Vec<u8>> {
        self.calls += 1;
        assert_eq!(request.seal(), b"AZ");
        (!self.fails).then(|| self.value.clone())
    }
}

#[derive(Default)]
struct Memory {
    generations: Vec<(u64, String)>,
}

impl CheckpointStore for Memory {
    fn commit(&mut self, generation: u64, document: &str) -> Result<(), CheckpointStoreError> {
        self.generations.push((generation, document.to_owned()));
        Ok(())
    }
}

impl Memory {
    fn active(&self) -> AgentCheckpoint {
        AgentCheckpoint::decode(&self.generations.last().expect("one generation").1).unwrap()
    }
}

#[test]
fn source_declared_agent_is_admitted_as_an_authenticated_project_source() {
    let manifest = project_root().join("semaprax.toml");
    with_authenticated_project(&manifest, |snapshot| {
        let project = snapshot.retain_revision();
        let source = project
            .sources()
            .iter()
            .find(|source| source.path() == "src/agent.spx")
            .expect("the Agent module is an authenticated Project source");
        assert_eq!(source.source(), agent_module_source());
        assert!(project
            .agent_definitions()
            .iter()
            .any(|definition| definition.definition().agent_id() == AGENT_ID));
        Ok(())
    })
    .unwrap();

    let source = agent_module_source();
    let checked = semaprax::check(&source, "agent.spx").unwrap();
    assert!(checked
        .agents
        .iter()
        .any(|declaration| declaration.stable_id == AGENT_ID));
    let lifecycle =
        semaprax::agent_lifecycle::compile_source_agent_lifecycle(&source, "agent.spx", AGENT_ID)
            .unwrap();
    assert_eq!(lifecycle.agent_id(), AGENT_ID);
}

#[test]
fn everyday_agent_durable_run_completes_with_one_real_fixture_read_and_binds_evidence() {
    let agent = durable_agent(7);
    let mut read = Read::new();
    let mut store = Memory::default();
    let run = agent
        .start(
            &task(),
            &document(&agent),
            &mut read,
            DurableBudget::default(),
            Retention::ObservationBytes,
            &AgentCancellation::new(),
            &mut store,
            CrashPoint::Never,
        )
        .unwrap();
    assert_eq!(run.status(), DurableStatus::Completed);
    assert_eq!(run.reason(), "reduce_published_a_result");
    assert_eq!(read.calls, 1, "the real fixture is read exactly once");
    assert_eq!(run.boundary_crossings(), 1);
    assert!(run.result().is_some());

    // Evidence binds identities/digests/counts only; it never carries the
    // observed fixture bytes, the authorization seal, or the task payload.
    assert!(run.evidence().ends_with('\n'));
    assert!(run.evidence().contains("\"status\":\"completed\""));
    assert!(run.evidence_digest().starts_with("sha256:"));
    for absent in ["AZ", "everyday-manifest-review", "everyday-agent-manifest"] {
        assert!(
            !run.evidence().contains(absent),
            "evidence carries `{absent}`, which is not an identity/digest/count"
        );
    }

    // The checkpoint chain is opaque: no seal, no task payload, no proposal
    // payload, and no stage identity leak into the persisted bytes.
    let active = store.active();
    assert!(active.document().ends_with('\n'));
    assert_eq!(
        AgentCheckpoint::decode(active.document()).unwrap().digest(),
        active.digest()
    );
    for absent in ["AZ", "everyday-manifest-review", "everyday.agent.fn."] {
        assert!(!active.document().contains(absent));
    }
}

#[test]
fn everyday_agent_refuses_before_the_external_read_when_authorization_is_refused() {
    let agent = durable_agent(7);
    // budget 50 exceeds the state's granted budget (12), so `authorize`
    // refuses and the read effect is never reached.
    let refused_proposal = proposal(
        agent.lifecycle().proposal_schema().schema().digest(),
        "50",
        "1",
    );
    let mut read = Read::new();
    let mut store = Memory::default();
    let run = agent
        .start(
            &task(),
            &refused_proposal,
            &mut read,
            DurableBudget::default(),
            Retention::ObservationBytes,
            &AgentCancellation::new(),
            &mut store,
            CrashPoint::Never,
        )
        .unwrap();
    assert_ne!(run.status(), DurableStatus::Completed);
    assert_eq!(
        read.calls, 0,
        "a refused authorization never reaches the external boundary"
    );
}

#[test]
fn everyday_agent_effect_failure_is_a_typed_terminal_not_a_panic() {
    let agent = durable_agent(7);
    let mut read = Read::failing();
    let mut store = Memory::default();
    let run = agent
        .start(
            &task(),
            &document(&agent),
            &mut read,
            DurableBudget::default(),
            Retention::ObservationBytes,
            &AgentCancellation::new(),
            &mut store,
            CrashPoint::Never,
        )
        .unwrap();
    assert_ne!(run.status(), DurableStatus::Completed);
    assert_eq!(read.calls, 1);
}

/// Crash injection at every one of the durable machine's boundaries, proving
/// a resume never repeats the external read and always reaches the same
/// terminal result as an uncrashed run.
#[test]
fn everyday_agent_crash_injection_never_duplicates_the_real_read() {
    let agent = durable_agent(7);
    let proposal_source = document(&agent);

    let expected_digest = {
        let mut read = Read::new();
        let mut store = Memory::default();
        agent
            .start(
                &task(),
                &proposal_source,
                &mut read,
                DurableBudget::default(),
                Retention::ObservationBytes,
                &AgentCancellation::new(),
                &mut store,
                CrashPoint::Never,
            )
            .unwrap()
            .result_digest()
            .unwrap()
            .to_owned()
    };

    // `AfterIntent` and `AfterEffect` both leave the checkpoint at the
    // `Intent` program counter: once the intent is durable, the runtime can
    // no longer tell whether the boundary was actually crossed before the
    // crash, so a plain resume (`Reconciliation::None`) must not blindly
    // retry the read — it stays `Unknown` until a host reconciliation
    // explicitly settles or abandons it. `BeforeIntent` (intent not yet
    // durable) and `AfterSettlement`/`BeforeDelivery` (already settled) are
    // the two cases where a plain resume alone reaches delivery.
    let uncertain_after_intent = [CrashPoint::AfterIntent, CrashPoint::AfterEffect];

    for crash in [
        CrashPoint::BeforeIntent,
        CrashPoint::AfterIntent,
        CrashPoint::AfterEffect,
        CrashPoint::AfterSettlement,
        CrashPoint::BeforeDelivery,
    ] {
        let mut read = Read::new();
        let mut store = Memory::default();
        let crashed = agent
            .start(
                &task(),
                &proposal_source,
                &mut read,
                DurableBudget::default(),
                Retention::ObservationBytes,
                &AgentCancellation::new(),
                &mut store,
                crash,
            )
            .unwrap();
        assert_ne!(
            crashed.status(),
            DurableStatus::Completed,
            "{crash:?} must not itself deliver a result"
        );
        let calls_at_crash = read.calls;
        assert!(
            calls_at_crash <= 1,
            "{crash:?}: the real read crosses the boundary at most once before any crash"
        );

        let stored = store.active();
        let resumed = agent
            .resume(
                &stored,
                &task(),
                &proposal_source,
                &mut read,
                Reconciliation::None,
                &AgentCancellation::new(),
                &mut store,
            )
            .unwrap();

        if uncertain_after_intent.contains(&crash) {
            // No blind retry: the operation is uncertain and stays that way
            // until the host explicitly reconciles it.
            assert_eq!(resumed.status(), DurableStatus::Unknown, "{crash:?}");
            assert_eq!(
                resumed.reason(),
                "uncertain_delivery_requires_reconciliation",
                "{crash:?}"
            );
            assert_eq!(
                read.calls, calls_at_crash,
                "{crash:?}: a plain resume never re-crosses the boundary"
            );
            assert!(resumed.result().is_none());

            // Only an explicit host reconciliation carrying the real,
            // already-observed bytes may settle it, and it does so without
            // crossing the boundary again.
            let uncertain = store.active();
            let observed = read.value.clone();
            let reconciled = agent
                .resume(
                    &uncertain,
                    &task(),
                    &proposal_source,
                    &mut read,
                    Reconciliation::Settled(&observed),
                    &AgentCancellation::new(),
                    &mut store,
                )
                .unwrap();
            assert_eq!(reconciled.status(), DurableStatus::Completed, "{crash:?}");
            assert_eq!(
                reconciled.result_digest().unwrap(),
                expected_digest,
                "{crash:?}"
            );
            assert_eq!(
                read.calls, calls_at_crash,
                "{crash:?}: reconciliation settles without ever calling the real read again"
            );
        } else {
            assert_eq!(
                resumed.status(),
                DurableStatus::Completed,
                "{crash:?}: resume must reach delivery"
            );
            assert_eq!(
                resumed.result_digest().unwrap(),
                expected_digest,
                "{crash:?}: resume must reach the same result as the uncrashed run"
            );
        }
        assert!(
            read.calls <= 1,
            "{crash:?}: the real fixture read is never repeated across the crash and its resume (saw {} calls)",
            read.calls
        );
    }
}

/// The checkpoint document is self-verifying by construction
/// (`src/agent_lifecycle/durable/checkpoint.rs`'s module doc): decoding
/// requires the closed key set, a journal whose embedded `seq`/rank stays
/// consistent with array order, a recomputed chain link equal to the stored
/// one, and an exact canonical re-render of the supplied bytes. This proves
/// three of the four hostile shapes the issue's "Agent lifecycle" and
/// "Output/evidence" required-tests bullets name
/// ("checkpoint mutation/truncation/reorder/reminting") are rejected before
/// `resume()` is ever reached, using only the existing checkpoint codec —
/// no new host operation or ABI surface.
#[test]
fn everyday_agent_checkpoint_decode_rejects_truncation_reorder_and_injected_bytes() {
    let agent = durable_agent(7);
    let mut read = Read::new();
    let mut store = Memory::default();
    let run = agent
        .start(
            &task(),
            &document(&agent),
            &mut read,
            DurableBudget::default(),
            Retention::ObservationBytes,
            &AgentCancellation::new(),
            &mut store,
            CrashPoint::Never,
        )
        .unwrap();
    assert_eq!(run.status(), DurableStatus::Completed);
    let canonical = store.active().document().to_owned();
    assert!(
        AgentCheckpoint::decode(&canonical).is_ok(),
        "the untouched document must decode"
    );

    // Truncation: a checkpoint store that tore mid-write leaves a prefix of
    // the canonical bytes, which no longer ends in the required terminal LF.
    let truncated = &canonical[..canonical.len() - 2];
    assert!(
        AgentCheckpoint::decode(truncated).is_err(),
        "a truncated checkpoint must not decode"
    );

    // Reordering: swap the first two journal entries' raw bytes in place
    // (not a JSON-value round trip, which would also alphabetize every
    // other key and confound what is actually being tested). Each entry's
    // embedded `seq` travels with it, so after the swap position 0 carries
    // `"seq":"1"`: array position and embedded rank now disagree, which
    // breaks decode before the recomputed chain link is even compared.
    let (start0, end0) = journal_entry_span(&canonical, 0);
    let (start1, end1) = journal_entry_span(&canonical, 1);
    assert!(end0 <= start1, "entry 0 must precede entry 1 in the array");
    let mut reordered = String::new();
    reordered.push_str(&canonical[..start0]);
    reordered.push_str(&canonical[start1..end1]);
    reordered.push_str(&canonical[end0..start1]);
    reordered.push_str(&canonical[start0..end0]);
    reordered.push_str(&canonical[end1..]);
    assert_ne!(reordered, canonical);
    assert!(
        AgentCheckpoint::decode(&reordered).is_err(),
        "a reordered journal must not decode"
    );

    // Byte injection: one extra, structurally-valid-JSON space is not
    // something the canonical renderer ever produces, so the exact
    // re-render check refuses it even though `serde_json` parses it fine.
    let injected = canonical.replacen("\"schema\":", "\"schema\": ", 1);
    assert_ne!(injected, canonical);
    assert!(
        AgentCheckpoint::decode(&injected).is_err(),
        "a byte-for-byte non-canonical (but JSON-valid) document must not decode"
    );
}

/// The fourth hostile shape the same bullet names, "reminting", is a real,
/// documented boundary this product does **not** claim to cross: the
/// checkpoint's own `NONCLAIMS` state "no checkpoint integrity or
/// authenticity without the caller's storage contract". Only the journal's
/// chain link and the digests `resume()` separately re-derives (state,
/// proposal, operation identity) or compares against the *live* agent
/// (`CheckpointBinding::drift`) are actually authenticated.
/// `budgets.effect_grants_remaining` is neither, so a store an adversary can
/// rewrite can mint a self-consistent higher grant and `decode()` alone
/// accepts it — proving the gap is real rather than asserting it away.
#[test]
fn everyday_agent_checkpoint_decode_does_not_authenticate_a_self_consistent_budget_remint() {
    let agent = durable_agent(7);
    let mut read = Read::new();
    let mut store = Memory::default();
    agent
        .start(
            &task(),
            &document(&agent),
            &mut read,
            DurableBudget::default(),
            Retention::ObservationBytes,
            &AgentCancellation::new(),
            &mut store,
            CrashPoint::BeforeIntent,
        )
        .unwrap();
    let canonical = store.active().document().to_owned();
    let original_grants = AgentCheckpoint::decode(&canonical)
        .unwrap()
        .effect_grants_remaining();

    // A single targeted field mutation, done as raw-byte substitution (not
    // a JSON-value round trip) so nothing else about the canonical bytes
    // changes.
    let marker = format!("\"effect_grants_remaining\":\"{original_grants}\"");
    assert!(canonical.contains(&marker), "grants field must be present");
    let reminted_grants = original_grants + 1000;
    let reminted = canonical.replacen(
        &marker,
        &format!("\"effect_grants_remaining\":\"{reminted_grants}\""),
        1,
    );
    assert_ne!(reminted, canonical);

    let decoded = AgentCheckpoint::decode(&reminted)
        .expect("a self-consistent budget remint is not a codec-detectable corruption");
    assert_eq!(decoded.effect_grants_remaining(), reminted_grants);
}

/// Locates journal entry `seq`'s exact byte span (its opening `{` through
/// its closing `}`, inclusive) inside a rendered checkpoint document.
/// Journal entries are flat objects (every field is a string or a
/// JSON-quoted scalar; see `JournalEntry::encode`), so the first `}` after
/// the entry's start marker is always that entry's own close.
fn journal_entry_span(document: &str, seq: usize) -> (usize, usize) {
    let marker = format!("{{\"seq\":\"{seq}\",\"kind\":");
    let start = document
        .find(&marker)
        .unwrap_or_else(|| panic!("journal entry {seq} not found in {document}"));
    let close = document[start..]
        .find('}')
        .unwrap_or_else(|| panic!("journal entry {seq} has no closing brace"));
    (start, start + close + 1)
}

/// A revoked policy epoch (the live agent now bound to a different epoch
/// than the one a checkpoint durably recorded) is caught by
/// `CheckpointBinding::drift`, the same mechanism `resume()` uses for every
/// other binding field, before any further stage runs and before the
/// external boundary is ever reconsidered.
#[test]
fn everyday_agent_resume_refuses_a_revoked_policy_epoch() {
    let agent = durable_agent(7);
    let proposal_source = document(&agent);
    let mut read = Read::new();
    let mut store = Memory::default();
    agent
        .start(
            &task(),
            &proposal_source,
            &mut read,
            DurableBudget::default(),
            Retention::ObservationBytes,
            &AgentCancellation::new(),
            &mut store,
            CrashPoint::BeforeIntent,
        )
        .unwrap();
    let stuck = store.active();
    let calls_before_resume = read.calls;

    // Same source, same compiled definition/deployment, a different policy
    // epoch: models a live authority revoking the epoch this checkpoint was
    // durably bound under.
    let rebound = durable_agent(8);
    let mut store2 = Memory::default();
    let resumed = rebound
        .resume(
            &stuck,
            &task(),
            &proposal_source,
            &mut read,
            Reconciliation::None,
            &AgentCancellation::new(),
            &mut store2,
        )
        .unwrap();
    assert_eq!(resumed.status(), DurableStatus::Stale);
    assert_eq!(resumed.reason(), "policy_epoch_revoked");
    assert_eq!(
        read.calls, calls_before_resume,
        "a checkpoint refused for a revoked epoch never re-crosses the external boundary"
    );
}

/// ProgramRoot drift, in its narrowest honest form: a pure display rename
/// (a trailing comment appended to `agent.spx`, no identity, signature,
/// type, decision shape, or stage-graph change) changes the raw module
/// bytes `bind_durable_agent`'s `source_digest` hashes directly, while
/// every other `CheckpointBinding` field is recompiled from resolved,
/// `@id`-keyed semantic content (`compile_agent_lifecycle`'s
/// `render_lifecycle`, `compile_source_agent_declaration`'s canonical
/// definition) and so does not move. `CheckpointBinding::drift` checks
/// `source_digest` last, precisely so a display-only rename is
/// distinguishable from every other kind of drift it names.
#[test]
fn everyday_agent_resume_reports_a_display_only_rename_as_source_drift() {
    let original_source = agent_module_source();
    let agent = durable_agent_from_source(&original_source, 7);
    let proposal_source = document(&agent);
    let mut read = Read::new();
    let mut store = Memory::default();
    agent
        .start(
            &task(),
            &proposal_source,
            &mut read,
            DurableBudget::default(),
            Retention::ObservationBytes,
            &AgentCancellation::new(),
            &mut store,
            CrashPoint::BeforeIntent,
        )
        .unwrap();
    let stuck = store.active();
    let calls_before_resume = read.calls;

    // Comment-only edit: no `@id`, signature, type, or stage-graph byte
    // changes, only a trailing line a canonical formatter would keep as a
    // display-only rename.
    let renamed_source =
        format!("{original_source}// display-only rename evidence for ABI-09A.17\n");
    assert_ne!(renamed_source, original_source);
    let renamed = durable_agent_from_source(&renamed_source, 7);

    let mut store2 = Memory::default();
    let resumed = renamed
        .resume(
            &stuck,
            &task(),
            &proposal_source,
            &mut read,
            Reconciliation::None,
            &AgentCancellation::new(),
            &mut store2,
        )
        .unwrap();
    assert_eq!(resumed.status(), DurableStatus::Stale);
    assert_eq!(
        resumed.reason(),
        "source_drift",
        "a comment-only edit must be the *last*-checked drift, not an earlier one"
    );
    assert_eq!(
        read.calls, calls_before_resume,
        "a checkpoint refused for source drift never re-crosses the external boundary"
    );
}

/// Only the `#[cfg(unix)]` module below calls this, so it must carry the same
/// gate: without it the Windows build of this harness fails with
/// `function \`scoped_report_fixture\` is never used` under `-D warnings`.
#[cfg(unix)]
fn scoped_report_fixture(label: &str) -> PathBuf {
    let root = std::env::temp_dir().join(format!(
        "semaprax-everyday-agent-product-{label}-{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    fs::create_dir_all(root.join("fixtures")).unwrap();
    root
}

/// The real typed-filesystem-and-bounded-JSON slice, executed for real
/// against a scoped temporary directory through the same Project route
/// `std.fs` itself is exercised with
/// (`tests/project/standard_library/filesystem_v2.rs`).
#[cfg(unix)]
mod real_filesystem_and_json {
    use super::*;
    use semaprax::filesystem_provider::{FileAccess, ScopedFileProvider};
    use semaprax::interpreter::CommandEvaluationOutcome;

    fn manifest_path() -> PathBuf {
        project_root().join("semaprax.toml")
    }

    #[test]
    fn review_ok_reads_the_real_manifest_and_writes_the_real_bounded_report() {
        let root = scoped_report_fixture("happy-path");
        fs::write(root.join("fixtures/input.json"), fixture_input_bytes()).unwrap();
        let mut provider = ScopedFileProvider::open(&root, FileAccess::ReadWrite).unwrap();
        let outcome = semaprax::project::with_authenticated_project(&manifest_path(), |snapshot| {
            snapshot.execute_filesystem_command(&mut provider, 1_000_000)
        })
        .unwrap();
        assert!(
            matches!(
                outcome.outcome,
                CommandEvaluationOutcome::ReturnedBool(true)
            ),
            "{outcome:?}"
        );
        drop(provider);
        let report = fs::read(root.join("fixtures/report.json")).unwrap();
        // The fixture's three records are id=m1/tag=ok/payload=alpha,
        // id=m2/tag=warn/payload=beta, id=m3/tag=ok/payload=gamma: two
        // "ok" tags accepted, one "warn" tag rejected.
        assert_eq!(report, b"{\"a\":2}");
        fs::remove_dir_all(&root).unwrap();
    }

    #[test]
    fn review_ok_refuses_a_wrong_schema_and_writes_nothing() {
        let root = scoped_report_fixture("wrong-schema");
        let tampered = String::from_utf8(fixture_input_bytes())
            .unwrap()
            .replace("everyday-agent-manifest.v1", "not-the-right-schema");
        fs::write(root.join("fixtures/input.json"), tampered).unwrap();
        let mut provider = ScopedFileProvider::open(&root, FileAccess::ReadWrite).unwrap();
        let outcome = semaprax::project::with_authenticated_project(&manifest_path(), |snapshot| {
            snapshot.execute_filesystem_command(&mut provider, 1_000_000)
        })
        .unwrap();
        assert!(
            matches!(
                outcome.outcome,
                CommandEvaluationOutcome::ReturnedBool(false)
            ),
            "{outcome:?}"
        );
        drop(provider);
        assert!(!root.join("fixtures/report.json").exists());
        fs::remove_dir_all(&root).unwrap();
    }

    #[test]
    fn review_ok_refuses_a_wrong_record_count_and_writes_nothing() {
        let root = scoped_report_fixture("wrong-record-count");
        let tampered = String::from_utf8(fixture_input_bytes())
            .unwrap()
            .replace("\"record_count\":3", "\"record_count\":9");
        fs::write(root.join("fixtures/input.json"), tampered).unwrap();
        let mut provider = ScopedFileProvider::open(&root, FileAccess::ReadWrite).unwrap();
        let outcome = semaprax::project::with_authenticated_project(&manifest_path(), |snapshot| {
            snapshot.execute_filesystem_command(&mut provider, 1_000_000)
        })
        .unwrap();
        assert!(
            matches!(
                outcome.outcome,
                CommandEvaluationOutcome::ReturnedBool(false)
            ),
            "{outcome:?}"
        );
        drop(provider);
        assert!(!root.join("fixtures/report.json").exists());
        fs::remove_dir_all(&root).unwrap();
    }

    #[test]
    fn review_ok_never_clobbers_an_existing_report() {
        let root = scoped_report_fixture("no-clobber");
        fs::write(root.join("fixtures/input.json"), fixture_input_bytes()).unwrap();
        fs::write(root.join("fixtures/report.json"), b"{\"a\":9}").unwrap();
        let mut provider = ScopedFileProvider::open(&root, FileAccess::ReadWrite).unwrap();
        let outcome = semaprax::project::with_authenticated_project(&manifest_path(), |snapshot| {
            snapshot.execute_filesystem_command(&mut provider, 1_000_000)
        })
        .unwrap();
        drop(provider);
        // Whatever the exact typed no-clobber outcome, the pre-existing
        // report is never silently replaced by this run.
        assert_eq!(
            fs::read(root.join("fixtures/report.json")).unwrap(),
            b"{\"a\":9}",
            "{outcome:?}"
        );
        fs::remove_dir_all(&root).unwrap();
    }
}
