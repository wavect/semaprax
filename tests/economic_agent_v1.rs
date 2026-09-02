use std::collections::BTreeMap;
use std::fs;
use std::process::Command;
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};

use semaprax::agent_runtime::{
    Agent, AgentBoundaryProbe, AgentCancellation, AgentHost, AgentProviderAttempt,
    AgentProviderDisposition, AgentProviderSink, AgentProviderUsage, AgentRun, AgentToolResultSink,
};
use semaprax::economic_agent::{
    BitcoinPaymentAdapter, EconomicAdapterDisposition, EconomicAgent, EconomicAgentHost,
    EconomicBoundaryProbe, EconomicBytesSink, EconomicDocumentSink, EconomicJournalLoad,
    EconomicRail, EconomicRollingReservationUpdate, EconomicRunStatus, EvmPaymentAdapter,
    PaymentApprover, PaymentJournal, SolanaPaymentAdapter, WalletCustody, X402InvoiceAdapter,
};
use serde_json::json;
use sha2::{Digest, Sha256};

const EVIDENCE_DOMAIN: &[u8] = b"semaprax.economic-agent.evidence-digest.v1\0";

fn digest(domain: &[u8], bytes: &[u8]) -> String {
    let mut digest = Sha256::new();
    digest.update(domain);
    digest.update(bytes);
    format!(
        "sha256:{:x}",
        semaprax::digest_hex::LowerHex(digest.finalize())
    )
}

fn nonclaims() -> &'static str {
    r#"["no_model_output_payment_authority","no_model_self_approval_or_policy_expansion","no_seed_private_key_credential_or_signing_material_input","no_secret_prompt_trace_evidence_log_or_diagnostic_exposure","no_builtin_network_http_dns_custody_or_chain_authority","no_mainnet_authority","no_wildcard_network_asset_recipient_origin_or_resource","no_token_contract_program_script_swap_bridge_or_unlimited_approval","no_raw_signing_or_signed_transaction_export","no_exactly_once_signing_broadcast_or_payment","no_automatic_uncertain_broadcast_retry","no_guaranteed_confirmation_finality_or_reorg_freedom","no_compromised_wallet_approver_adapter_provider_or_chain_recovery","no_power_loss_durability_without_host_journal_contract","no_cross_process_or_distributed_concurrency_guarantee","no_live_price_exchange_rate_fee_or_cost_accuracy","no_balance_allowance_or_simulation_truth_beyond_adapter","no_human_identity_intent_approval_provenance_or_nonrepudiation","no_signature_attestation_or_custody_provenance","no_tax_accounting_legal_regulatory_sanctions_or_compliance_correctness","no_privacy_data_residency_or_unlinkability_guarantee","no_x402_redirect_ssrf_private_network_or_server_honesty_guarantee_beyond_admitted_adapter_contract","no_automatic_refund_chargeback_replacement_or_fee_bumping","no_wallet_recovery_rotation_backup_or_inheritance","no_general_payment_sdk_or_production_readiness","no_language_graph_cleanup_backend_or_workspace_atomicity_semantics","no_current_agent_runtime_schema_api_or_kat_modification","no_completion_matrix_status_promotion"]"#
}

fn limits() -> &'static str {
    r#"{"max_policy_bytes":1048576,"max_intent_bytes":1048576,"max_invoice_bytes":1048576,"max_snapshot_bytes":1048576,"max_plan_bytes":1048576,"max_simulation_bytes":1048576,"max_approval_request_bytes":1048576,"max_approval_bytes":65536,"max_journal_bytes":8388608,"max_unsigned_transaction_bytes":1048576,"max_signed_transaction_bytes":2097152,"max_broadcast_receipt_bytes":1048576,"max_reconciliation_bytes":1048576,"max_trace_events":1024,"max_trace_bytes":8388608,"max_evidence_bytes":16777216,"max_builder_bytes":67108864,"max_json_depth":16,"max_identifier_bytes":128,"max_memo_bytes":1024,"max_recipients":128,"max_network_policies":16,"max_x402_origins":32,"max_utxos":100,"max_reconciliations":64,"max_elapsed_ms":600000,"max_amount_atomic":1000000000000000000,"max_fee_atomic":1000000000000000,"max_compute_units":200000,"max_confirmation_target":144,"max_concurrency":1,"max_unexpected_authority_calls":0}"#
}

fn policy(recipient: &str, rail: &str, network: &str, asset: &str, x402: bool) -> String {
    let origins = if x402 {
        r#"[{"origin":"https://pay.example.com","methods":["POST"],"resources":["/v1/payments"],"settlement_rails":["evm"],"max_amount_atomic":1000000}]"#
    } else {
        "[]"
    };
    format!(
        "{{\"schema\":\"semaprax.economic-agent-policy.v1\",\"economic_agent_id\":\"fixture.economic\",\"wallet_id\":\"fixture.wallet\",\"network_policies\":[{{\"rail\":\"{rail}\",\"network\":\"{network}\",\"asset\":\"{asset}\",\"recipients\":[\"{recipient}\"],\"max_amount_atomic\":1000000,\"max_fee_atomic\":1000000,\"max_rolling_24h_atomic\":1000000}}],\"x402_origins\":{origins},\"limits\":{},\"nonclaims\":{}}}\n",
        limits(),
        nonclaims()
    )
}

fn intent(rail: &str, payment: &str, key: &str) -> String {
    format!(
        "{{\"schema\":\"semaprax.economic-agent-payment-intent.v1\",\"intent_id\":\"fixture.intent.{rail}\",\"wallet_id\":\"fixture.wallet\",\"rail\":\"{rail}\",\"idempotency_key\":\"{key}\",\"created_at_ms\":1700000000000,\"expires_at_ms\":1700000300000,\"memo\":null,\"payment\":{payment}}}\n"
    )
}

fn base58(bytes: &[u8]) -> String {
    const ALPHABET: &[u8] = b"123456789ABCDEFGHJKLMNPQRSTUVWXYZabcdefghijkmnopqrstuvwxyz";
    let zeros = bytes.iter().take_while(|byte| **byte == 0).count();
    let mut digits = Vec::new();
    for byte in bytes.iter().skip(zeros) {
        let mut carry = u32::from(*byte);
        for digit in &mut digits {
            let value = u32::from(*digit) * 256 + carry;
            *digit = (value % 58) as u8;
            carry = value / 58;
        }
        while carry != 0 {
            digits.push((carry % 58) as u8);
            carry /= 58;
        }
    }
    std::iter::repeat_n('1', zeros)
        .chain(
            digits
                .iter()
                .rev()
                .map(|digit| ALPHABET[usize::from(*digit)] as char),
        )
        .collect()
}

fn convert_bits(data: &[u8], from: u32, to: u32) -> Vec<u8> {
    let (mut acc, mut bits) = (0u32, 0u32);
    let mut output = Vec::new();
    for value in data {
        acc = (acc << from) | u32::from(*value);
        bits += from;
        while bits >= to {
            bits -= to;
            output.push(((acc >> bits) & ((1 << to) - 1)) as u8);
        }
    }
    if bits != 0 {
        output.push(((acc << (to - bits)) & ((1 << to) - 1)) as u8);
    }
    output
}

fn regtest(program: [u8; 20]) -> String {
    const CHARSET: &[u8] = b"qpzry9x8gf2tvdw0s3jn54khce6mua7l";
    let mut data = vec![0];
    data.extend(convert_bits(&program, 8, 5));
    let mut values = vec![3, 3, 3, 3, 0, 2, 3, 18, 20];
    values.extend_from_slice(&data);
    values.extend([0; 6]);
    let mut polymod = 1u32;
    for value in values {
        let top = polymod >> 25;
        polymod = ((polymod & 0x01ff_ffff) << 5) ^ u32::from(value);
        for (index, generator) in [
            0x3b6a_57b2,
            0x2650_8e6d,
            0x1ea1_19fa,
            0x3d42_33dd,
            0x2a14_62b3,
        ]
        .iter()
        .enumerate()
        {
            if ((top >> index) & 1) != 0 {
                polymod ^= generator;
            }
        }
    }
    polymod ^= 1;
    let mut encoded = String::from("bcrt1");
    for value in data
        .into_iter()
        .chain((0..6).map(|index| ((polymod >> (5 * (5 - index))) & 31) as u8))
    {
        encoded.push(CHARSET[usize::from(value)] as char);
    }
    encoded
}

fn runtime_nonclaims() -> &'static str {
    r#"["no_compiler_determinism_from_model_output","no_model_output_authority","no_provider_identity_provenance_or_quality_truth","no_secret_input_or_secret_leakage_guarantee_for_caller_supplied_content","no_credential_prompt_state_trace_or_diagnostic_exposure","no_ambient_network_filesystem_process_home_or_environment_authority","no_write_apply_mutation_or_target_execution_tool_authority","no_capability_minting_delegation_or_self_approval","no_human_approval_ui_or_policy","no_semantic_prompt_injection_proof","no_forced_cancellation_or_preemption","no_exactly_once_provider_billing_or_retry","no_durable_memory_persistence_recovery_or_resume","no_crash_reboot_or_power_loss_durability","no_distributed_or_parallel_execution","no_model_quality_accuracy_or_completion_guarantee","no_live_price_or_cost_accuracy_guarantee","no_reusable_authorization_token","no_signature_attestation_or_authenticated_provenance","no_wallet_payment_signing_asset_or_economic_authority","no_privacy_compliance_or_data_residency_guarantee","no_general_formal_proof","no_new_language_graph_cleanup_backend_or_runtime_semantics","no_current_schema_api_or_kat_modification"]"#
}

fn runtime_profile() -> String {
    format!(
        "{{\"schema\":\"semaprax.agent-runtime-profile.v1\",\"agent_id\":\"fixture.agent\",\"models\":[{{\"provider_id\":\"fake.local\",\"model_id\":\"fake-basic\",\"locality\":\"local\",\"quality_tier\":\"basic\",\"tokenizer_id\":\"fake.bytes-v1\",\"max_context_tokens\":4096,\"input_usd_microunits_per_million_tokens\":0,\"output_usd_microunits_per_million_tokens\":0,\"capabilities\":[\"text\"]}}],\"tools\":[],\"policy\":{{\"allowed_provider_ids\":[\"fake.local\"],\"allowed_model_ids\":[\"fake-basic\"],\"required_locality\":\"local_only\",\"minimum_quality_tier\":\"basic\",\"required_model_capabilities\":[\"text\"],\"granted_capabilities\":[],\"allowed_tool_ids\":[]}},\"limits\":{{\"max_turns\":1,\"max_provider_attempts\":1,\"max_retries_per_turn\":0,\"max_concurrency\":1,\"max_elapsed_ms\":1000,\"max_provider_request_bytes\":65536,\"max_provider_response_bytes\":4096,\"max_stream_chunks\":4,\"max_total_provider_input_bytes\":131072,\"max_total_provider_output_bytes\":8192,\"max_reported_model_input_tokens\":131072,\"max_reported_model_output_tokens\":8192,\"max_usd_microunits\":0,\"max_tool_calls\":0,\"max_tool_arguments_bytes\":4096,\"max_tool_result_bytes\":4096,\"max_total_tool_bytes\":8192,\"max_retained_state_bytes\":16777216,\"max_trace_events\":64,\"max_trace_bytes\":16777216,\"max_evidence_bytes\":20971520,\"max_builder_bytes\":67108864}},\"nonclaims\":{}}}\n",
        runtime_nonclaims()
    )
}

#[derive(Clone)]
struct RuntimeProbe;
impl AgentBoundaryProbe for RuntimeProbe {
    fn policy_epoch(&self) -> u64 {
        1
    }
    fn elapsed_ms(&self) -> u64 {
        0
    }
}

struct RuntimeHost {
    message: String,
}
impl AgentHost for RuntimeHost {
    fn policy_epoch(&self) -> u64 {
        1
    }
    fn elapsed_ms(&self) -> u64 {
        0
    }
    fn boundary_probe(&self) -> Box<dyn AgentBoundaryProbe> {
        Box::new(RuntimeProbe)
    }
    fn tokenize(&mut self, _: &str, request: &str) -> Option<u64> {
        Some(request.len() as u64)
    }
    fn attempt_provider(
        &mut self,
        _: &str,
        _: &str,
        request: &str,
        _: u64,
        sink: &mut AgentProviderSink,
    ) -> AgentProviderAttempt {
        let response = format!("{{\"schema\":\"semaprax.agent-runtime-action.v1\",\"kind\":\"final\",\"message\":{}}}\n", serde_json::to_string(&self.message).unwrap());
        assert!(sink.push(response.as_bytes()));
        AgentProviderAttempt::new(
            AgentProviderDisposition::Succeeded,
            AgentProviderUsage::new(request.len() as u64, response.len() as u64, 0),
        )
    }
    fn invoke_tool(&mut self, _: &str, _: &str, _: &str, _: &mut AgentToolResultSink) -> bool {
        panic!("no tool authority")
    }
}

fn sealed(message: String) -> AgentRun {
    let task = format!("{{\"schema\":\"semaprax.agent-runtime-task.v1\",\"nonce\":\"{}\",\"objective\":\"Return the supplied canonical intent.\",\"context\":[]}}\n", "0".repeat(64));
    Agent::new(
        &runtime_profile(),
        RuntimeHost { message },
        AgentCancellation::new(),
    )
    .unwrap()
    .run(&task)
    .unwrap()
}

#[derive(Clone)]
struct Probe(Arc<AtomicU64>);
impl EconomicBoundaryProbe for Probe {
    fn elapsed_ms(&self) -> u64 {
        self.0.load(Ordering::Acquire)
    }
}

struct Host {
    calls: Arc<AtomicUsize>,
    sequence: Arc<Mutex<Vec<&'static str>>>,
    elapsed: Arc<AtomicU64>,
    expected_rail: EconomicRail,
    expected_key: String,
    journals: BTreeMap<String, String>,
}

impl Host {
    fn new(
        rail: EconomicRail,
        key: &str,
    ) -> (Self, Arc<AtomicUsize>, Arc<Mutex<Vec<&'static str>>>) {
        let calls = Arc::new(AtomicUsize::new(0));
        let sequence = Arc::new(Mutex::new(Vec::new()));
        (
            Self {
                calls: calls.clone(),
                sequence: sequence.clone(),
                elapsed: Arc::new(AtomicU64::new(0)),
                expected_rail: rail,
                expected_key: key.into(),
                journals: BTreeMap::new(),
            },
            calls,
            sequence,
        )
    }
    fn record(&self, name: &'static str) {
        self.calls.fetch_add(1, Ordering::AcqRel);
        self.sequence.lock().unwrap().push(name);
    }
    fn fail(&self, name: &'static str) -> EconomicAdapterDisposition {
        self.record(name);
        EconomicAdapterDisposition::DefinitelyNotStarted
    }
}

impl EconomicAgentHost for Host {
    fn boundary_probe(&self) -> Box<dyn EconomicBoundaryProbe> {
        Box::new(Probe(self.elapsed.clone()))
    }
}
impl PaymentJournal for Host {
    fn load(&mut self, key: &str, sink: &mut EconomicDocumentSink) -> EconomicJournalLoad {
        assert_eq!(key, self.expected_key);
        self.record("load");
        match self.journals.get(key) {
            Some(journal) => {
                assert!(sink.push(journal.as_bytes()));
                EconomicJournalLoad::Present
            }
            None => EconomicJournalLoad::Missing,
        }
    }
    fn compare_and_swap(
        &mut self,
        key: &str,
        _: u64,
        journal: &str,
        update: EconomicRollingReservationUpdate<'_>,
    ) -> EconomicAdapterDisposition {
        if let EconomicRollingReservationUpdate::Reserve(row) = update {
            assert_eq!(row.rail(), self.expected_rail);
            assert_eq!(row.wallet_id(), "fixture.wallet");
        }
        self.record("cas");
        self.journals.insert(key.to_owned(), journal.to_owned());
        EconomicAdapterDisposition::Succeeded
    }
}
impl X402InvoiceAdapter for Host {
    fn fetch_invoice(
        &mut self,
        _: &str,
        _: &str,
        _: &str,
        _: &mut EconomicDocumentSink,
    ) -> EconomicAdapterDisposition {
        self.fail("invoice")
    }
}
macro_rules! fail_rail {
    ($trait:ident,$snap:ident,$sim:ident,$broadcast:ident,$reconcile:ident) => {
        impl $trait for Host {
            fn $snap(
                &mut self,
                _: &str,
                _: &mut EconomicDocumentSink,
            ) -> EconomicAdapterDisposition {
                self.fail(stringify!($snap))
            }
            fn $sim(
                &mut self,
                _: &str,
                _: &[u8],
                _: &mut EconomicDocumentSink,
            ) -> EconomicAdapterDisposition {
                self.fail(stringify!($sim))
            }
            fn $broadcast(
                &mut self,
                _: &[u8],
                _: &mut EconomicDocumentSink,
            ) -> EconomicAdapterDisposition {
                self.fail(stringify!($broadcast))
            }
            fn $reconcile(
                &mut self,
                _: &str,
                _: &mut EconomicDocumentSink,
            ) -> EconomicAdapterDisposition {
                self.fail(stringify!($reconcile))
            }
        }
    };
}
fail_rail!(
    EvmPaymentAdapter,
    evm_snapshot,
    evm_simulate,
    evm_broadcast,
    evm_reconcile
);
fail_rail!(
    SolanaPaymentAdapter,
    solana_snapshot,
    solana_simulate,
    solana_broadcast,
    solana_reconcile
);
fail_rail!(
    BitcoinPaymentAdapter,
    bitcoin_snapshot,
    bitcoin_simulate,
    bitcoin_broadcast,
    bitcoin_reconcile
);
impl PaymentApprover for Host {
    fn approve(&mut self, _: &str, _: &mut EconomicDocumentSink) -> EconomicAdapterDisposition {
        self.fail("approve")
    }
}
impl WalletCustody for Host {
    fn sign(
        &mut self,
        _: &str,
        _: EconomicRail,
        _: &str,
        _: &[u8],
        _: &str,
        _: &mut EconomicBytesSink,
    ) -> EconomicAdapterDisposition {
        self.fail("sign")
    }
}

#[test]
fn public_all_rails_and_x402_dispatch_are_replayable_and_authority_injected() {
    let sol = base58(&[3; 32]);
    let btc = regtest([9; 20]);
    let rows = [
        (
            EconomicRail::Evm,
            "evm",
            "sepolia",
            "native:eth",
            "0x1111111111111111111111111111111111111111",
            r#"{"kind":"evm","network":"sepolia","asset":"native:eth","recipient":"0x1111111111111111111111111111111111111111","amount_atomic":10,"max_fee_atomic":100000}"#,
            "fixture.payment.evm",
            false,
        ),
        (
            EconomicRail::Solana,
            "solana",
            "devnet",
            "native:sol",
            &sol,
            &format!(
                r#"{{"kind":"solana","network":"devnet","asset":"native:sol","recipient":"{sol}","amount_atomic":10,"max_fee_atomic":6000,"max_compute_units":200000,"max_priority_fee_atomic":1000}}"#
            ),
            "fixture.payment.solana",
            false,
        ),
        (
            EconomicRail::Bitcoin,
            "bitcoin",
            "regtest",
            "native:btc",
            &btc,
            &format!(
                r#"{{"kind":"bitcoin","network":"regtest","asset":"native:btc","recipient":"{btc}","amount_atomic":10000,"max_fee_atomic":10000,"confirmation_target":1}}"#
            ),
            "fixture.payment.bitcoin",
            false,
        ),
    ];
    for (rail, text, network, asset, recipient, payment, key, x402) in rows {
        let source = sealed(intent(text, payment, key));
        let (host, calls, sequence) = Host::new(rail, key);
        let mut agent = EconomicAgent::new(
            &policy(recipient, text, network, asset, x402),
            host,
            AgentCancellation::new(),
        )
        .unwrap();
        let run = agent.execute(&source).unwrap();
        assert_eq!(run.status(), EconomicRunStatus::AdapterFailed);
        assert_eq!(
            run.evidence_digest(),
            digest(EVIDENCE_DOMAIN, run.evidence().as_bytes())
        );
        assert!(run.trace().ends_with('\n') && run.evidence().ends_with('\n'));
        assert!(calls.load(Ordering::Acquire) >= 2);
        assert_eq!(
            &*sequence.lock().unwrap(),
            &[
                "load",
                "cas",
                match rail {
                    EconomicRail::Evm => "evm_snapshot",
                    EconomicRail::Solana => "solana_snapshot",
                    EconomicRail::Bitcoin => "bitcoin_snapshot",
                },
                "cas"
            ]
        );
    }

    let invoice_digest = digest(
        b"semaprax.economic-agent.x402-invoice-digest.v1\0",
        b"unavailable\n",
    );
    let x402_payment = format!(
        r#"{{"kind":"x402","origin":"https://pay.example.com","method":"POST","resource":"/v1/payments","invoice_digest":"{invoice_digest}","payee":"0x1111111111111111111111111111111111111111","settlement_rail":"evm","network":"sepolia","asset":"native:eth","amount_atomic":10,"max_fee_atomic":100000,"invoice_expires_at_ms":1700000299999,"invoice_nonce":"fixture.nonce"}}"#
    );
    let key = "fixture.payment.x402";
    let source = sealed(intent("x402", &x402_payment, key));
    let (host, _, sequence) = Host::new(EconomicRail::Evm, key);
    let mut agent = EconomicAgent::new(
        &policy(
            "0x1111111111111111111111111111111111111111",
            "evm",
            "sepolia",
            "native:eth",
            true,
        ),
        host,
        AgentCancellation::new(),
    )
    .unwrap();
    assert_eq!(
        agent.execute(&source).unwrap().status(),
        EconomicRunStatus::AdapterFailed
    );
    assert_eq!(
        &*sequence.lock().unwrap(),
        &["load", "cas", "invoice", "cas"]
    );

    let evm_payment = r#"{"kind":"evm","network":"sepolia","asset":"native:eth","recipient":"0x1111111111111111111111111111111111111111","amount_atomic":10,"max_fee_atomic":100000}"#;
    let key = "fixture.payment.reconcile";
    let source = sealed(intent("evm", evm_payment, key));
    let (host, _, sequence) = Host::new(EconomicRail::Evm, key);
    let mut agent = EconomicAgent::new(
        &policy(
            "0x1111111111111111111111111111111111111111",
            "evm",
            "sepolia",
            "native:eth",
            false,
        ),
        host,
        AgentCancellation::new(),
    )
    .unwrap();
    assert_eq!(
        agent.execute(&source).unwrap().status(),
        EconomicRunStatus::AdapterFailed
    );
    sequence.lock().unwrap().clear();
    assert_eq!(
        agent.reconcile(key, &source).unwrap().status(),
        EconomicRunStatus::AdapterFailed
    );
    let calls = sequence.lock().unwrap().clone();
    assert_eq!(calls, ["load"]);
    assert!(!calls.contains(&"sign") && !calls.contains(&"evm_broadcast"));
    let substituted = sealed(intent(
        "evm",
        &evm_payment.replace("\"amount_atomic\":10", "\"amount_atomic\":11"),
        key,
    ));
    sequence.lock().unwrap().clear();
    let error = match agent.reconcile(key, &substituted) {
        Ok(_) => panic!("substituted AgentRun admitted"),
        Err(error) => error,
    };
    assert_eq!(error[0].code, "SPX-G215");
    assert_eq!(&*sequence.lock().unwrap(), &["load"]);
}

#[test]
fn public_pre_effect_cancellation_caps_and_source_binding_fail_closed() {
    let payment = r#"{"kind":"evm","network":"sepolia","asset":"native:eth","recipient":"0x1111111111111111111111111111111111111111","amount_atomic":10,"max_fee_atomic":100000}"#;
    let message = intent("evm", payment, "fixture.payment.evm");
    let source = sealed(message);
    let cancellation = AgentCancellation::new();
    cancellation.cancel();
    let (host, calls, _) = Host::new(EconomicRail::Evm, "fixture.payment.evm");
    let mut agent = EconomicAgent::new(
        &policy(
            "0x1111111111111111111111111111111111111111",
            "evm",
            "sepolia",
            "native:eth",
            false,
        ),
        host,
        cancellation,
    )
    .unwrap();
    let error = match agent.execute(&source) {
        Ok(_) => panic!("cancelled execution produced evidence"),
        Err(error) => error,
    };
    assert_eq!(error[0].code, "SPX-I228");
    assert_eq!(calls.load(Ordering::Acquire), 0);

    let (host, calls, _) = Host::new(EconomicRail::Evm, "fixture.payment.evm");
    let error = EconomicAgent::new("{}\n", host, AgentCancellation::new())
        .err()
        .expect("malformed policy admitted");
    assert_eq!(error[0].code, "SPX-G210");
    assert_eq!(calls.load(Ordering::Acquire), 0);
}

#[test]
fn public_surface_traits_and_closed_status_domains_are_exhaustive() {
    let dispositions = [
        EconomicAdapterDisposition::Succeeded,
        EconomicAdapterDisposition::DefinitelyNotStarted,
        EconomicAdapterDisposition::FailedUncertain,
        EconomicAdapterDisposition::PolicyRejected,
    ];
    assert_eq!(dispositions.len(), 4);
    let loads = [
        EconomicJournalLoad::Missing,
        EconomicJournalLoad::Present,
        EconomicJournalLoad::DefinitelyNotStarted,
        EconomicJournalLoad::FailedUncertain,
    ];
    assert_eq!(loads.len(), 4);
    let rails = [
        EconomicRail::Evm,
        EconomicRail::Solana,
        EconomicRail::Bitcoin,
    ];
    assert_eq!(rails.len(), 3);
    let statuses = [
        EconomicRunStatus::Confirmed,
        EconomicRunStatus::Pending,
        EconomicRunStatus::Reorged,
        EconomicRunStatus::Dropped,
        EconomicRunStatus::Rejected,
        EconomicRunStatus::Cancelled,
        EconomicRunStatus::DeadlineExceeded,
        EconomicRunStatus::BudgetExhausted,
        EconomicRunStatus::JournalFailed,
        EconomicRunStatus::AdapterFailed,
        EconomicRunStatus::ApprovalFailed,
        EconomicRunStatus::CustodyFailed,
        EconomicRunStatus::BroadcastUnknown,
        EconomicRunStatus::ReconciliationFailed,
    ];
    assert_eq!(statuses.len(), 14);
    assert_eq!(json!({"surface":"opaque"})["surface"], "opaque");
}

#[test]
fn external_consumer_surface_is_opaque_and_has_no_cli_or_ambient_authority() {
    let root = std::env::temp_dir().join(format!(
        "semaprax-economic-surface-{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    fs::create_dir_all(root.join("src")).unwrap();
    let manifest_root = env!("CARGO_MANIFEST_DIR").replace('\\', "\\\\");
    fs::write(root.join("Cargo.toml"), format!("[package]\nname=\"economic-surface-lock\"\nversion=\"0.0.0\"\nedition=\"2021\"\n[workspace]\n[dependencies]\nsemaprax={{path=\"{manifest_root}\",default-features=false}}\n")).unwrap();
    fs::write(root.join("src/main.rs"), r#"use semaprax::economic_agent::{EconomicAgent,EconomicRun,EconomicDocumentSink,EconomicBytesSink,EconomicRollingReservation,parse_policy,replay_bundle,Policy,Intent};
fn clone<T: Clone>() {} fn debug<T: std::fmt::Debug>() {}
fn reject<H: semaprax::economic_agent::EconomicAgentHost>() { clone::<EconomicAgent<H>>(); debug::<EconomicAgent<H>>(); }
fn main() { clone::<EconomicRun>(); debug::<EconomicRun>(); let _=EconomicDocumentSink::new(); let _=EconomicBytesSink::new(); let _=EconomicRollingReservation{wallet_id:String::new()}; let _=parse_policy; let _=replay_bundle; let _=std::mem::size_of::<Policy>(); let _=std::mem::size_of::<Intent>(); }
"#).unwrap();
    let checked = Command::new(std::env::var_os("CARGO").unwrap_or_else(|| "cargo".into()))
        .args(["check", "--offline", "--manifest-path"])
        .arg(root.join("Cargo.toml"))
        .env("CARGO_TARGET_DIR", root.join("target"))
        .output()
        .unwrap();
    assert!(!checked.status.success());
    let stderr = String::from_utf8_lossy(&checked.stderr);
    for name in ["parse_policy", "replay_bundle", "Clone", "Debug", "private"] {
        assert!(stderr.contains(name), "missing `{name}` in:\n{stderr}");
    }
    let cli = Command::new(env!("CARGO_BIN_EXE_semaprax"))
        .output()
        .unwrap();
    let cli_text = format!(
        "{}{}",
        String::from_utf8_lossy(&cli.stdout),
        String::from_utf8_lossy(&cli.stderr)
    );
    assert!(!cli_text.contains("economic-agent"));
    let source = include_str!("../src/economic_agent.rs");
    for forbidden in [
        "std::net::TcpStream",
        "std::net::UdpSocket",
        "reqwest::",
        "std::fs::",
        "std::process::Command",
        "std::env::var(",
    ] {
        assert!(
            !source[..source.find("#[cfg(test)]").unwrap()].contains(forbidden),
            "ambient authority `{forbidden}`"
        );
        for submodule in [
            include_str!("../src/economic_agent/address.rs"),
            include_str!("../src/economic_agent/agent_core.rs"),
            include_str!("../src/economic_agent/agent_execute.rs"),
            include_str!("../src/economic_agent/agent_reconcile.rs"),
            include_str!("../src/economic_agent/documents.rs"),
            include_str!("../src/economic_agent/evidence.rs"),
            include_str!("../src/economic_agent/intent.rs"),
            include_str!("../src/economic_agent/journal.rs"),
            include_str!("../src/economic_agent/policy.rs"),
            include_str!("../src/economic_agent/replay.rs"),
            include_str!("../src/economic_agent/snapshot.rs"),
            include_str!("../src/economic_agent/transaction.rs"),
            include_str!("../src/economic_agent/validate.rs"),
        ] {
            assert!(
                !submodule.contains(forbidden),
                "ambient authority `{forbidden}`"
            );
        }
    }
    fs::remove_dir_all(root).unwrap();
}
