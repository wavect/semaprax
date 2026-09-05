use semaprax::agent_harness::{compile_agent_payment_graph, verify_agent_payment_graph_bundle};
use semaprax::agent_runtime::{AgentCancellation, AgentRunStatus};
use semaprax::economic_agent::{
    BitcoinPaymentAdapter, EconomicAdapterDisposition, EconomicAgentHost, EconomicBoundaryProbe,
    EconomicBytesSink, EconomicDocumentSink, EconomicJournalLoad, EconomicRail,
    EconomicRollingReservationUpdate, EconomicRunStatus, EvmPaymentAdapter, PaymentApprover,
    PaymentJournal, SolanaPaymentAdapter, WalletCustody, X402InvoiceAdapter,
};

use super::agent_definition_v1::definition;
use super::{profile, task, Host};

fn economic_nonclaims() -> &'static str {
    r#"["no_model_output_payment_authority","no_model_self_approval_or_policy_expansion","no_seed_private_key_credential_or_signing_material_input","no_secret_prompt_trace_evidence_log_or_diagnostic_exposure","no_builtin_network_http_dns_custody_or_chain_authority","no_mainnet_authority","no_wildcard_network_asset_recipient_origin_or_resource","no_token_contract_program_script_swap_bridge_or_unlimited_approval","no_raw_signing_or_signed_transaction_export","no_exactly_once_signing_broadcast_or_payment","no_automatic_uncertain_broadcast_retry","no_guaranteed_confirmation_finality_or_reorg_freedom","no_compromised_wallet_approver_adapter_provider_or_chain_recovery","no_power_loss_durability_without_host_journal_contract","no_cross_process_or_distributed_concurrency_guarantee","no_live_price_exchange_rate_fee_or_cost_accuracy","no_balance_allowance_or_simulation_truth_beyond_adapter","no_human_identity_intent_approval_provenance_or_nonrepudiation","no_signature_attestation_or_custody_provenance","no_tax_accounting_legal_regulatory_sanctions_or_compliance_correctness","no_privacy_data_residency_or_unlinkability_guarantee","no_x402_redirect_ssrf_private_network_or_server_honesty_guarantee_beyond_admitted_adapter_contract","no_automatic_refund_chargeback_replacement_or_fee_bumping","no_wallet_recovery_rotation_backup_or_inheritance","no_general_payment_sdk_or_production_readiness","no_language_graph_cleanup_backend_or_workspace_atomicity_semantics","no_current_agent_runtime_schema_api_or_kat_modification","no_completion_matrix_status_promotion"]"#
}

fn economic_limits() -> &'static str {
    r#"{"max_policy_bytes":1048576,"max_intent_bytes":1048576,"max_invoice_bytes":1048576,"max_snapshot_bytes":1048576,"max_plan_bytes":1048576,"max_simulation_bytes":1048576,"max_approval_request_bytes":1048576,"max_approval_bytes":65536,"max_journal_bytes":8388608,"max_unsigned_transaction_bytes":1048576,"max_signed_transaction_bytes":2097152,"max_broadcast_receipt_bytes":1048576,"max_reconciliation_bytes":1048576,"max_trace_events":1024,"max_trace_bytes":8388608,"max_evidence_bytes":16777216,"max_builder_bytes":67108864,"max_json_depth":16,"max_identifier_bytes":128,"max_memo_bytes":1024,"max_recipients":128,"max_network_policies":16,"max_x402_origins":32,"max_utxos":100,"max_reconciliations":64,"max_elapsed_ms":600000,"max_amount_atomic":1000000000000000000,"max_fee_atomic":1000000000000000,"max_compute_units":200000,"max_confirmation_target":144,"max_concurrency":1,"max_unexpected_authority_calls":0}"#
}

fn economic_policy() -> String {
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

fn payment_intent() -> String {
    concat!(
        "{\"schema\":\"semaprax.economic-agent-payment-intent.v1\",",
        "\"intent_id\":\"fixture.intent.evm\",\"wallet_id\":\"fixture.wallet\",",
        "\"rail\":\"evm\",\"idempotency_key\":\"fixture.payment.evm\",",
        "\"created_at_ms\":1700000000000,\"expires_at_ms\":1700000300000,",
        "\"memo\":null,\"payment\":{\"kind\":\"evm\",\"network\":\"sepolia\",",
        "\"asset\":\"native:eth\",",
        "\"recipient\":\"0x1111111111111111111111111111111111111111\",",
        "\"amount_atomic\":10,\"max_fee_atomic\":100000}}\n"
    )
    .to_owned()
}

struct Probe;

impl EconomicBoundaryProbe for Probe {
    fn elapsed_ms(&self) -> u64 {
        1_700_000_000_001
    }
}

#[derive(Default)]
struct PaymentHost {
    calls: Vec<&'static str>,
}

impl EconomicAgentHost for PaymentHost {
    fn boundary_probe(&self) -> Box<dyn EconomicBoundaryProbe> {
        Box::new(Probe)
    }
}

impl PaymentJournal for PaymentHost {
    fn load(&mut self, _: &str, _: &mut EconomicDocumentSink) -> EconomicJournalLoad {
        self.calls.push("load");
        EconomicJournalLoad::Missing
    }

    fn compare_and_swap(
        &mut self,
        _: &str,
        _: u64,
        _: &str,
        _: EconomicRollingReservationUpdate<'_>,
    ) -> EconomicAdapterDisposition {
        self.calls.push("cas");
        EconomicAdapterDisposition::Succeeded
    }
}

impl X402InvoiceAdapter for PaymentHost {
    fn fetch_invoice(
        &mut self,
        _: &str,
        _: &str,
        _: &str,
        _: &mut EconomicDocumentSink,
    ) -> EconomicAdapterDisposition {
        panic!("non-x402 intent requested an invoice")
    }
}

macro_rules! rejecting_rail {
    ($trait:ident, $snapshot:ident, $simulate:ident, $broadcast:ident, $reconcile:ident) => {
        impl $trait for PaymentHost {
            fn $snapshot(
                &mut self,
                _: &str,
                _: &mut EconomicDocumentSink,
            ) -> EconomicAdapterDisposition {
                self.calls.push(stringify!($snapshot));
                EconomicAdapterDisposition::DefinitelyNotStarted
            }
            fn $simulate(
                &mut self,
                _: &str,
                _: &[u8],
                _: &mut EconomicDocumentSink,
            ) -> EconomicAdapterDisposition {
                panic!("simulation followed a rejected snapshot")
            }
            fn $broadcast(
                &mut self,
                _: &[u8],
                _: &mut EconomicDocumentSink,
            ) -> EconomicAdapterDisposition {
                panic!("broadcast followed a rejected snapshot")
            }
            fn $reconcile(
                &mut self,
                _: &str,
                _: &mut EconomicDocumentSink,
            ) -> EconomicAdapterDisposition {
                panic!("reconciliation followed a rejected snapshot")
            }
        }
    };
}

rejecting_rail!(
    EvmPaymentAdapter,
    evm_snapshot,
    evm_simulate,
    evm_broadcast,
    evm_reconcile
);
rejecting_rail!(
    SolanaPaymentAdapter,
    solana_snapshot,
    solana_simulate,
    solana_broadcast,
    solana_reconcile
);
rejecting_rail!(
    BitcoinPaymentAdapter,
    bitcoin_snapshot,
    bitcoin_simulate,
    bitcoin_broadcast,
    bitcoin_reconcile
);

impl PaymentApprover for PaymentHost {
    fn approve(&mut self, _: &str, _: &mut EconomicDocumentSink) -> EconomicAdapterDisposition {
        panic!("approval followed a rejected snapshot")
    }
}

impl WalletCustody for PaymentHost {
    fn sign(
        &mut self,
        _: &str,
        _: EconomicRail,
        _: &str,
        _: &[u8],
        _: &str,
        _: &mut EconomicBytesSink,
    ) -> EconomicAdapterDisposition {
        panic!("custody followed a rejected snapshot")
    }
}

#[test]
fn payment_graph_replays_and_drives_the_authority_separated_harness() {
    let definition = definition(&profile());
    let policy = economic_policy();
    let compiled = compile_agent_payment_graph(&definition, &policy).unwrap();
    let repeated = compile_agent_payment_graph(&definition, &policy).unwrap();

    assert_eq!(
        compiled.graph().canonical_json(),
        repeated.graph().canonical_json()
    );
    assert_eq!(compiled.graph().digest(), repeated.graph().digest());
    assert!(compiled
        .graph()
        .canonical_json()
        .contains("\"economic_evidence_binds_agent_evidence\":true"));
    assert!(compiled
        .graph()
        .canonical_json()
        .contains("\"custody\":\"injected_opaque\""));
    verify_agent_payment_graph_bundle(
        &definition,
        &policy,
        compiled.agent().graph().canonical_json(),
        compiled.graph().canonical_json(),
    )
    .unwrap();

    let mut harness = compiled
        .instantiate(
            Host::new().with_final_message(payment_intent()),
            PaymentHost::default(),
            AgentCancellation::new(),
        )
        .unwrap();
    let run = harness.run_payment(&task()).unwrap();
    assert_eq!(run.agent_run().status(), AgentRunStatus::Completed);
    assert_eq!(
        run.economic_run().status(),
        EconomicRunStatus::AdapterFailed
    );
    assert_eq!(
        run.agent_definition_digest(),
        compiled.agent().definition().digest()
    );
    assert_eq!(run.agent_graph_digest(), compiled.agent().graph().digest());
    assert_eq!(run.payment_graph_digest(), compiled.graph().digest());

    let tampered = compiled.graph().canonical_json().replacen(
        "\"approval\":\"injected\"",
        "\"approval\":\"model\"",
        1,
    );
    let error = verify_agent_payment_graph_bundle(
        &definition,
        &policy,
        compiled.agent().graph().canonical_json(),
        &tampered,
    )
    .unwrap_err();
    assert_eq!(error[0].code, "SPX-G505");

    let harness_source = include_str!("../../src/agent_harness.rs");
    for forbidden in [
        "std::net::",
        "reqwest::",
        "std::fs::",
        "std::process::Command",
        "std::env::var(",
    ] {
        assert!(
            !harness_source.contains(forbidden),
            "ambient authority `{forbidden}`"
        );
    }
}
