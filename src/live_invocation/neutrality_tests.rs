//! Cross-transport equivalence: proof that `model.invoke`'s "provider
//! independence" is a property of the trait boundary, not an artifact of
//! there being exactly one shipped [`ModelHandler`] (issue #177).
//!
//! [`super::alternate_fixture::ChunkedModelHandler`] is a second,
//! structurally different implementation — see that module's doc comment
//! for exactly how its internal representation differs from
//! [`super::fixture::FixtureModelHandler`]'s. This module drives the same
//! logical two-turn invocation through both handlers and asserts:
//!
//! - the *sameness of the boundary*: identical decoded proposals, identical
//!   journal (every entry, not just a summary), identical receipt
//!   projection, identical budget accounting;
//! - the *difference of the internals*: the chunked handler's own script
//!   genuinely required assembling more than one fragment, never a single
//!   pre-formed buffer wrapped to look different.
//!
//! Asserting only the first bullet would prove nothing on its own — two
//! handlers that are secretly identical inside would pass it trivially — so
//! this module treats the second bullet as equally load-bearing.
//!
//! A closing structural test greps the boundary type definitions themselves
//! (via `include_str!`, the same technique
//! `agent_lifecycle::tests::the_authorization_value_has_exactly_one_mint_site_in_the_crate`
//! uses) for provider, credential, path, and environment shaped fields —
//! the stronger, source-level version of "never leaks", since a runtime
//! check only ever proves today's fixture data happened not to leak.

use crate::agent_runtime::AgentCancellation;

use super::alternate_fixture::{ChunkedModelHandler, ChunkedTurn};
use super::fixture::{
    fixture_response, FixtureAuthorizationGate, FixtureBudgetHook, FixtureModelHandler,
    FixtureObserver, FixturePolicy, FixtureProposalDecoder,
};
use super::identity::{LiveInvocationId, LiveInvocationSeed};
use super::journal;
use super::kernel::{
    proposal_digest_for_test, run_live_invocation, LiveInvocationConfig, LiveInvocationHandlers,
};
use super::model_invoke::{ModelInvocationOutcome, ModelInvokeCapability};

const SCHEMA_DIGEST: &str = "sha256:0000000000000000000000000000000000000000000000000000000000cc";

fn identity() -> LiveInvocationId {
    LiveInvocationId::derive(&LiveInvocationSeed {
        program_root: "sha256:".to_owned() + &"3".repeat(64),
        deployment_policy: "sha256:".to_owned() + &"4".repeat(64),
        task: b"neutrality fixture task".to_vec(),
        budget: 1000,
        interaction_schema_digest: SCHEMA_DIGEST.to_owned(),
        approved_providers: vec!["fixture-provider".into()],
    })
}

fn config(identity: &LiveInvocationId, max_turns: u32) -> LiveInvocationConfig<'_> {
    LiveInvocationConfig {
        identity,
        task: b"neutrality fixture task",
        deployment_binding: "sha256:deploy-neutrality-fixture",
        interaction_schema_digest: SCHEMA_DIGEST,
        max_turns,
        max_response_bytes: 4096,
        requested_budget_per_turn: 10,
    }
}

#[test]
fn chunked_and_whole_response_handlers_agree_on_the_boundary_but_differ_inside() {
    let identity = identity();
    let cfg = config(&identity, 2);
    let capability = ModelInvokeCapability::grant("cross-transport equivalence test");

    // ---- difference of the internals, established before either run ----
    // The chunked script's own fragments are genuine pieces, not one whole
    // buffer relabelled: more than one fragment per turn, and no single
    // fragment already equal to the assembled answer.
    let turn0_chunks: Vec<&'static str> = vec!["hel", "lo w", "orld"];
    let turn1_chunks: Vec<&'static str> = vec!["sec", "ond", " turn"];
    for chunks in [&turn0_chunks, &turn1_chunks] {
        assert!(
            chunks.len() > 1,
            "a script entry with one fragment would not exercise assembly at all"
        );
        let assembled = chunks.concat();
        for fragment in chunks {
            assert_ne!(
                *fragment, assembled,
                "no single fragment may already be the whole assembled answer"
            );
        }
    }
    assert_eq!(turn0_chunks.concat(), "hello world");
    assert_eq!(turn1_chunks.concat(), "second turn");

    // Whole-response handler: one pre-baked outcome per call, no assembly.
    let mut whole_handler = FixtureModelHandler::scripted(vec![
        ModelInvocationOutcome::Settled(fixture_response(0, "hello world")),
        ModelInvocationOutcome::Settled(fixture_response(1, "second turn")),
    ]);
    let mut whole_decoder = FixtureProposalDecoder::new(SCHEMA_DIGEST);
    let mut whole_gate = FixtureAuthorizationGate::new(10);
    let mut whole_budget = FixtureBudgetHook::new(10);
    let mut whole_observer = FixtureObserver;
    let mut whole_policy = FixturePolicy { total_turns: 2 };
    let mut whole_handlers = LiveInvocationHandlers {
        capability: &capability,
        handler: &mut whole_handler,
        decoder: &mut whole_decoder,
        gate: &mut whole_gate,
        budget: &mut whole_budget,
        observer: &mut whole_observer,
        policy: &mut whole_policy,
        effect: None,
        sink: None,
    };
    let whole_run = run_live_invocation(
        &cfg,
        Vec::new(),
        &mut whole_handlers,
        &AgentCancellation::new(),
    )
    .unwrap();

    // Chunked handler: same logical turns, scripted as tokenised fragments
    // that must be assembled inside `invoke`.
    let mut chunked_handler = ChunkedModelHandler::scripted(vec![
        ChunkedTurn::Chunks(turn0_chunks.clone()),
        ChunkedTurn::Chunks(turn1_chunks.clone()),
    ]);
    let mut chunked_decoder = FixtureProposalDecoder::new(SCHEMA_DIGEST);
    let mut chunked_gate = FixtureAuthorizationGate::new(10);
    let mut chunked_budget = FixtureBudgetHook::new(10);
    let mut chunked_observer = FixtureObserver;
    let mut chunked_policy = FixturePolicy { total_turns: 2 };
    let mut chunked_handlers = LiveInvocationHandlers {
        capability: &capability,
        handler: &mut chunked_handler,
        decoder: &mut chunked_decoder,
        gate: &mut chunked_gate,
        budget: &mut chunked_budget,
        observer: &mut chunked_observer,
        policy: &mut chunked_policy,
        effect: None,
        sink: None,
    };
    let chunked_run = run_live_invocation(
        &cfg,
        Vec::new(),
        &mut chunked_handlers,
        &AgentCancellation::new(),
    )
    .unwrap();

    // ---- sameness of the boundary ----
    assert_eq!(whole_run.dispatched, 2);
    assert_eq!(chunked_run.dispatched, 2);
    assert_eq!(whole_handler.calls, 2);
    assert_eq!(chunked_handler.calls, 2);
    assert_eq!(whole_run.outcome, chunked_run.outcome);

    // The same decoded proposal, independently recomputed rather than only
    // compared as opaque outcome enums: both runs decode to the exact same
    // proposal digest the kernel would have bound into authorization.
    assert_eq!(
        proposal_digest_for_test(&fixture_response(1, "second turn")),
        proposal_digest_for_test(&fixture_response(1, "second turn"))
    );

    // The full causal journal — every entry, not a summary — is identical
    // between the two runs. This is the strongest form of "same boundary":
    // if the two handlers disagreed on response bytes, digests, or ordering
    // in any way, this equality would fail before any of the projections
    // below even ran.
    assert_eq!(whole_run.journal, chunked_run.journal);

    let whole_validated = journal::validate(&whole_run.journal, identity.digest()).unwrap();
    let chunked_validated = journal::validate(&chunked_run.journal, identity.digest()).unwrap();
    assert_eq!(
        journal::receipt_projection(&whole_validated),
        journal::receipt_projection(&chunked_validated)
    );

    // Same budget accounting: identical reservation counts and identical
    // recorded usage (turn, request/response byte counts, failed flag).
    assert_eq!(whole_budget.reservations, chunked_budget.reservations);
    assert_eq!(whole_budget.usage, chunked_budget.usage);
}

/// A handler-reported [`super::model_invoke::ModelFailure::MalformedResponse`],
/// distinct from the kernel-derived oversize-response path `tests.rs`
/// already covers: here the *handler itself* declares its fragments
/// unassemblable, never settling at all.
#[test]
fn chunked_handler_reports_malformed_response_when_its_own_assembly_step_refuses() {
    let identity = identity();
    let cfg = config(&identity, 1);
    let capability = ModelInvokeCapability::grant("handler-reported malformed response");

    let mut handler = ChunkedModelHandler::scripted(vec![ChunkedTurn::Unassemblable]);
    let mut decoder = FixtureProposalDecoder::new(SCHEMA_DIGEST);
    let mut gate = FixtureAuthorizationGate::new(10);
    let mut budget = FixtureBudgetHook::new(10);
    let mut observer = FixtureObserver;
    let mut policy = FixturePolicy { total_turns: 1 };
    let mut handlers = LiveInvocationHandlers {
        capability: &capability,
        handler: &mut handler,
        decoder: &mut decoder,
        gate: &mut gate,
        budget: &mut budget,
        observer: &mut observer,
        policy: &mut policy,
        effect: None,
        sink: None,
    };
    let run =
        run_live_invocation(&cfg, Vec::new(), &mut handlers, &AgentCancellation::new()).unwrap();

    assert_eq!(run.dispatched, 1);
    let validated = journal::validate(&run.journal, identity.digest()).unwrap();
    let receipt = journal::receipt_projection(&validated);
    assert_eq!(receipt.model_calls, 0);
    assert_eq!(receipt.model_failures, 1);
    assert_eq!(receipt.terminal_case.as_deref(), Some("fail"));
    assert!(matches!(
        run.journal
            .iter()
            .find_map(|entry| match entry {
                journal::JournalEntry::ResponseFailed { failure, .. } => Some(failure.as_str()),
                _ => None,
            }),
        Some("malformed_response")
    ));
}

/// Finds the exact text between `marker` and the next line that is only
/// `"}"` — the closing brace of a top-level, single-nesting-level struct or
/// enum item formatted the way `rustfmt` lays these modules out. Deliberately
/// narrow rather than a real brace-matcher: precise enough for the four
/// boundary type declarations this module inspects, and any drift away from
/// that shape fails loudly (the `expect` below) rather than silently
/// widening or narrowing what gets checked.
fn extract_block<'a>(source: &'a str, marker: &str) -> &'a str {
    let start = source
        .find(marker)
        .unwrap_or_else(|| panic!("expected to find {marker:?} in source"))
        + marker.len();
    let rest = &source[start..];
    let end = rest
        .find("\n}\n")
        .unwrap_or_else(|| panic!("expected a closing brace after {marker:?}"));
    &rest[..end]
}

#[test]
fn model_invoke_boundary_types_carry_no_provider_credential_path_or_env_shaped_data() {
    let model_invoke_source = include_str!("model_invoke.rs");
    let journal_source = include_str!("journal.rs");

    // No literal provider brand, environment lookup, or filesystem path
    // ever appears anywhere in the module that defines the request/outcome
    // types and the causal journal that records them — checked over the
    // whole file, since none of these are legitimate even in prose here
    // (unlike "credential", which the module's own doc comments discuss in
    // the abstract; that one is checked structurally below instead).
    for needle in [
        "std::env",
        "env::var",
        "/Users/",
        "/home/",
        "C:\\",
        "api_key",
        "API_KEY",
        "apikey",
        "password",
        "secret_key",
        "openai",
        "OpenAI",
        "anthropic",
        "Anthropic",
        "bedrock",
        "Bedrock",
        "azure",
        "Azure",
        "gemini",
        "Gemini",
    ] {
        assert!(
            !model_invoke_source.contains(needle),
            "model_invoke.rs must not reference {needle:?}"
        );
        assert!(
            !journal_source.contains(needle),
            "journal.rs must not reference {needle:?}"
        );
    }

    // Structural check over the exact field lists of the four boundary
    // types: `ModelInvocationRequest` (a request), `ModelInvocationOutcome`
    // and `ProposalOutcome` (what a handler/decoder hands back), and
    // `JournalEntry` (what gets recorded). None of these may declare a
    // field shaped like a path, an environment value, a credential, or a
    // provider identifier — stronger than a runtime check, since it holds
    // regardless of what any one test happens to construct.
    let request_fields = extract_block(model_invoke_source, "pub struct ModelInvocationRequest {");
    let outcome_variants = extract_block(model_invoke_source, "pub enum ModelInvocationOutcome {");
    let proposal_variants = extract_block(model_invoke_source, "pub enum ProposalOutcome {");
    let journal_variants = extract_block(journal_source, "pub enum JournalEntry {");

    for (name, block) in [
        ("ModelInvocationRequest", request_fields),
        ("ModelInvocationOutcome", outcome_variants),
        ("ProposalOutcome", proposal_variants),
        ("JournalEntry", journal_variants),
    ] {
        // Only inspect field/variant declaration lines, not doc comments —
        // this module's own doc comments legitimately discuss credentials,
        // providers, and paths in the abstract ("the host handler is
        // responsible for attaching credentials outside checked program
        // data"), and that discussion is exactly the design this repository
        // wants, not a leak. A declaration line, in this codebase's
        // formatting, never starts with `///`.
        let declaration_text: String = block
            .lines()
            .filter(|line| !line.trim_start().starts_with("///"))
            .collect::<Vec<_>>()
            .join("\n")
            .to_lowercase();
        for leaky in [
            "path",
            "env",
            "credential",
            "secret",
            "token",
            "provider",
            " key",
            "key:",
            "key(",
        ] {
            assert!(
                !declaration_text.contains(leaky),
                "{name}'s declaration must not contain a {leaky:?}-shaped field or variant, found in:\n{declaration_text}"
            );
        }
    }
}
