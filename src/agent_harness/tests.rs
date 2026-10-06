use super::{compile_agent_payment_graph, verify_agent_payment_graph_bundle, MAX_GRAPH_BYTES};
use crate::agent_definition::compilations_on_this_thread;
use crate::agent_definition::tests::fixture_definition;

fn economic_nonclaims() -> &'static str {
    r#"["no_model_output_payment_authority","no_model_self_approval_or_policy_expansion","no_seed_private_key_credential_or_signing_material_input","no_secret_prompt_trace_evidence_log_or_diagnostic_exposure","no_builtin_network_http_dns_custody_or_chain_authority","no_mainnet_authority","no_wildcard_network_asset_recipient_origin_or_resource","no_token_contract_program_script_swap_bridge_or_unlimited_approval","no_raw_signing_or_signed_transaction_export","no_exactly_once_signing_broadcast_or_payment","no_automatic_uncertain_broadcast_retry","no_guaranteed_confirmation_finality_or_reorg_freedom","no_compromised_wallet_approver_adapter_provider_or_chain_recovery","no_power_loss_durability_without_host_journal_contract","no_cross_process_or_distributed_concurrency_guarantee","no_live_price_exchange_rate_fee_or_cost_accuracy","no_balance_allowance_or_simulation_truth_beyond_adapter","no_human_identity_intent_approval_provenance_or_nonrepudiation","no_signature_attestation_or_custody_provenance","no_tax_accounting_legal_regulatory_sanctions_or_compliance_correctness","no_privacy_data_residency_or_unlinkability_guarantee","no_x402_redirect_ssrf_private_network_or_server_honesty_guarantee_beyond_admitted_adapter_contract","no_automatic_refund_chargeback_replacement_or_fee_bumping","no_wallet_recovery_rotation_backup_or_inheritance","no_general_payment_sdk_or_production_readiness","no_language_graph_cleanup_backend_or_workspace_atomicity_semantics","no_current_agent_runtime_schema_api_or_kat_modification","no_completion_matrix_status_promotion"]"#
}

fn economic_limits() -> &'static str {
    r#"{"max_policy_bytes":1048576,"max_intent_bytes":1048576,"max_invoice_bytes":1048576,"max_snapshot_bytes":1048576,"max_plan_bytes":1048576,"max_simulation_bytes":1048576,"max_approval_request_bytes":1048576,"max_approval_bytes":65536,"max_journal_bytes":8388608,"max_unsigned_transaction_bytes":1048576,"max_signed_transaction_bytes":2097152,"max_broadcast_receipt_bytes":1048576,"max_reconciliation_bytes":1048576,"max_trace_events":1024,"max_trace_bytes":8388608,"max_evidence_bytes":16777216,"max_builder_bytes":67108864,"max_json_depth":16,"max_identifier_bytes":128,"max_memo_bytes":1024,"max_recipients":128,"max_network_policies":16,"max_x402_origins":32,"max_utxos":100,"max_reconciliations":64,"max_elapsed_ms":600000,"max_amount_atomic":1000000000000000000,"max_fee_atomic":1000000000000000,"max_compute_units":200000,"max_confirmation_target":144,"max_concurrency":1,"max_unexpected_authority_calls":0}"#
}

pub(crate) fn economic_policy() -> String {
    concat!(
        "{\"schema\":\"semaprax.economic-agent-policy.v1\",",
        "\"economic_agent_id\":\"fixture.economic\",\"wallet_id\":\"fixture.wallet\",",
        "\"network_policies\":[{\"rail\":\"evm\",\"network\":\"sepolia\",",
        "\"asset\":\"native:eth\",\"recipients\":[\"0x1111111111111111111111111111111111111111\"],",
        "\"max_amount_atomic\":1000000,\"max_fee_atomic\":1000000,",
        "\"max_rolling_24h_atomic\":1000000}],\"x402_origins\":[],",
        "\"limits\":LIMITS,\"nonclaims\":NONCLAIMS}\n"
    )
    .replace("LIMITS", economic_limits())
    .replace("NONCLAIMS", economic_nonclaims())
}

struct Bundle {
    definition: String,
    policy: String,
    agent_graph: String,
    payment_graph: String,
}

fn bundle() -> Bundle {
    let definition = fixture_definition();
    let policy = economic_policy();
    let compiled = compile_agent_payment_graph(&definition, &policy).unwrap();
    Bundle {
        agent_graph: compiled.agent().graph().canonical_json().to_owned(),
        payment_graph: compiled.graph().canonical_json().to_owned(),
        definition,
        policy,
    }
}

fn verify(definition: &str, policy: &str, agent_graph: &str, payment_graph: &str) -> &'static str {
    match verify_agent_payment_graph_bundle(definition, policy, agent_graph, payment_graph) {
        Ok(()) => "ok",
        Err(diagnostics) => diagnostics[0].code,
    }
}

#[test]
fn one_payment_bundle_verification_compiles_the_definition_once() {
    let bundle = bundle();
    let before = compilations_on_this_thread();
    verify_agent_payment_graph_bundle(
        &bundle.definition,
        &bundle.policy,
        &bundle.agent_graph,
        &bundle.payment_graph,
    )
    .unwrap();
    assert_eq!(compilations_on_this_thread() - before, 1);
}

#[test]
fn a_valid_bundle_keeps_its_exact_bytes_and_digests() {
    let bundle = bundle();
    let compiled = compile_agent_payment_graph(&bundle.definition, &bundle.policy).unwrap();
    // Frozen known answers, independent of the verifier under test.
    assert_eq!(
        compiled.agent().definition().digest(),
        "sha256:82ab9abbeca5e209c36224d9cab3b7b6a7cdffc3b2fce5db73123fa7425965a0"
    );
    assert_eq!(
        compiled.agent().graph().digest(),
        "sha256:0dc7ce1d50d43077042577cf6ac3dcfb5d2a744fb3acd2ca6cea12a6e296ff61"
    );
    assert_eq!(compiled.graph().digest(), PAYMENT_GRAPH_DIGEST);
    assert_eq!(
        compiled.agent().graph().canonical_json(),
        bundle.agent_graph
    );
    assert_eq!(compiled.graph().canonical_json(), bundle.payment_graph);
    assert_eq!(
        verify(
            &bundle.definition,
            &bundle.policy,
            &bundle.agent_graph,
            &bundle.payment_graph
        ),
        "ok"
    );
}

const PAYMENT_GRAPH_DIGEST: &str =
    "sha256:0abdcd0d50cff65993abbc00776e00e6f18063d3691f92d958a5b5a7da2997b9";

#[test]
fn mutated_truncated_oversized_and_noncanonical_agent_graphs_are_rejected() {
    let bundle = bundle();
    let check = |agent_graph: &str| {
        verify(
            &bundle.definition,
            &bundle.policy,
            agent_graph,
            &bundle.payment_graph,
        )
    };
    let mutated = bundle
        .agent_graph
        .replacen("\"single_use\":true", "\"single_use\":false", 1);
    assert_eq!(check(&mutated), "SPX-G503");
    assert_eq!(
        check(&bundle.agent_graph[..bundle.agent_graph.len() - 1]),
        "SPX-G503"
    );
    assert_eq!(check(""), "SPX-G503");
    let noncanonical = bundle.agent_graph.replacen(",", ", ", 1);
    assert_eq!(check(&noncanonical), "SPX-G503");
    let crlf = bundle.agent_graph.replacen('\n', "\r\n", 1);
    assert_eq!(check(&crlf), "SPX-G503");

    // The payment graph's 65,536-byte guard does not cover the agent graph:
    // the agent graph keeps its own 1,572,864-byte bound.
    let mut oversized = bundle.agent_graph.clone();
    oversized.push_str(&" ".repeat(1_572_865 - oversized.len()));
    assert!(oversized.len() > MAX_GRAPH_BYTES);
    assert_eq!(check(&oversized), "SPX-G503");
}

#[test]
fn mutated_truncated_oversized_and_noncanonical_payment_graphs_are_rejected() {
    let bundle = bundle();
    let check = |payment_graph: &str| {
        verify(
            &bundle.definition,
            &bundle.policy,
            &bundle.agent_graph,
            payment_graph,
        )
    };
    let mutated =
        bundle
            .payment_graph
            .replacen("\"approval\":\"injected\"", "\"approval\":\"model\"", 1);
    assert_eq!(check(&mutated), "SPX-G505");
    assert_eq!(
        check(&bundle.payment_graph[..bundle.payment_graph.len() - 1]),
        "SPX-G505"
    );
    assert_eq!(
        check(&bundle.payment_graph.replacen(",", ", ", 1)),
        "SPX-G505"
    );
    assert_eq!(check(&"x".repeat(MAX_GRAPH_BYTES + 1)), "SPX-G505");
}

#[test]
fn a_recomputed_attacker_digest_cannot_bypass_source_derived_comparison() {
    let bundle = bundle();
    // The attacker edits the agent graph and rebinds the payment graph to the
    // edited graph's correctly recomputed digest. Both remain rejected,
    // because the comparison is against graphs derived from the sources.
    let forged_agent = bundle.agent_graph.replacen(
        "\"model_cannot_mint\":true",
        "\"model_cannot_mint\":false",
        1,
    );
    let honest_digest = super::digest(
        crate::agent_definition::GRAPH_DOMAIN_FOR_TESTS,
        bundle.agent_graph.as_bytes(),
    );
    let forged_digest = super::digest(
        crate::agent_definition::GRAPH_DOMAIN_FOR_TESTS,
        forged_agent.as_bytes(),
    );
    assert!(bundle.payment_graph.contains(&honest_digest));
    let forged_payment = bundle
        .payment_graph
        .replacen(&honest_digest, &forged_digest, 1);
    assert_eq!(
        verify(
            &bundle.definition,
            &bundle.policy,
            &forged_agent,
            &forged_payment
        ),
        "SPX-G503"
    );
    assert_eq!(
        verify(
            &bundle.definition,
            &bundle.policy,
            &bundle.agent_graph,
            &forged_payment
        ),
        "SPX-G505"
    );
}

#[test]
fn a_changed_definition_or_policy_invalidates_the_old_bundle() {
    let bundle = bundle();
    let definition = bundle.definition.replacen(
        "fixture.agent.type.observation",
        "fixture.agent.type.other_observation",
        1,
    );
    assert_eq!(
        verify(
            &definition,
            &bundle.policy,
            &bundle.agent_graph,
            &bundle.payment_graph
        ),
        "SPX-G503"
    );
    let changed = compile_agent_payment_graph(&definition, &bundle.policy).unwrap();
    assert_eq!(
        verify(
            &definition,
            &bundle.policy,
            changed.agent().graph().canonical_json(),
            &bundle.payment_graph
        ),
        "SPX-G505"
    );

    let policy = bundle
        .policy
        .replacen("fixture.economic", "fixture.economic.other", 1);
    assert_eq!(
        verify(
            &bundle.definition,
            &policy,
            &bundle.agent_graph,
            &bundle.payment_graph
        ),
        "SPX-G505"
    );
}

#[test]
fn cross_paired_graphs_are_rejected() {
    let bundle = bundle();
    let other_definition = bundle.definition.replacen(
        "fixture.agent.fn.reduce",
        "fixture.agent.fn.other_reduce",
        1,
    );
    let other = compile_agent_payment_graph(&other_definition, &bundle.policy).unwrap();
    assert_eq!(
        verify(
            &bundle.definition,
            &bundle.policy,
            other.agent().graph().canonical_json(),
            &bundle.payment_graph
        ),
        "SPX-G503"
    );
    assert_eq!(
        verify(
            &bundle.definition,
            &bundle.policy,
            &bundle.agent_graph,
            other.graph().canonical_json()
        ),
        "SPX-G505"
    );
    // The graphs swapped into each other's positions: the agent graph is
    // checked first.
    assert_eq!(
        verify(
            &bundle.definition,
            &bundle.policy,
            &bundle.payment_graph,
            &bundle.agent_graph
        ),
        "SPX-G503"
    );
}

#[test]
fn failure_precedence_is_unchanged_when_several_inputs_are_invalid() {
    let bundle = bundle();
    let bad_definition = bundle
        .definition
        .replacen("\"kind\":\"model\"", "\"kind\":\"effect\"", 1);
    let bad_policy = bundle.policy.replacen("{", "{ ", 1);
    let bad_agent = bundle.agent_graph.replacen(",", ", ", 1);
    let bad_payment = bundle.payment_graph.replacen(",", ", ", 1);
    let oversized_payment = "x".repeat(MAX_GRAPH_BYTES + 1);

    // An oversized payment graph is refused before anything compiles.
    let before = compilations_on_this_thread();
    assert_eq!(
        verify(&bad_definition, &bad_policy, &bad_agent, &oversized_payment),
        "SPX-G505"
    );
    assert_eq!(compilations_on_this_thread(), before);
    // Then the definition, then the policy, then the agent graph, then the
    // payment graph.
    assert_eq!(
        verify(&bad_definition, &bad_policy, &bad_agent, &bad_payment),
        "SPX-G502"
    );
    let policy_code = verify(&bundle.definition, &bad_policy, &bad_agent, &bad_payment);
    assert_ne!(policy_code, "ok");
    assert_ne!(policy_code, "SPX-G503");
    assert_ne!(policy_code, "SPX-G505");
    assert_eq!(
        verify(&bundle.definition, &bundle.policy, &bad_agent, &bad_payment),
        "SPX-G503"
    );
    assert_eq!(
        verify(
            &bundle.definition,
            &bundle.policy,
            &bundle.agent_graph,
            &bad_payment
        ),
        "SPX-G505"
    );
}
