//! Bounded Economic Agent v1 injected-host API.
//!
//! This safe-Rust state machine has no built-in transport, DNS, filesystem,
//! process, environment, key, custody, journal, or wallet implementation.
//! Caller implementations of the host traits are trusted authorities; all
//! adapter documents and bytes remain untrusted input.

#![forbid(unsafe_code)]
#![allow(
    dead_code,
    clippy::field_reassign_with_default,
    clippy::format_collect,
    clippy::too_many_arguments,
    reason = "private typed and replay internals support the opaque public C surface"
)]

use std::collections::BTreeMap;
use std::fmt;
use std::net::IpAddr;

use serde_json::{Map, Value};
use sha2::{Digest, Sha256};

use crate::agent_runtime::{AgentCancellation, AgentRun, AgentRunStatus};
use crate::bounded_output::{
    active_limit, active_remaining, clear_active_floor, reserve_active, reserve_active_preserving,
    set_active_floor, with_limit_usage,
};
use crate::diagnostic::{quote_json, Diagnostic};

const POLICY_SCHEMA: &str = "semaprax.economic-agent-policy.v1";
const INTENT_SCHEMA: &str = "semaprax.economic-agent-payment-intent.v1";
const INVOICE_SCHEMA: &str = "semaprax.economic-agent-x402-invoice.v1";
const SNAPSHOT_SCHEMA: &str = "semaprax.economic-agent-chain-snapshot.v1";
const PLAN_SCHEMA: &str = "semaprax.economic-agent-payment-plan.v1";
const SIMULATION_SCHEMA: &str = "semaprax.economic-agent-simulation.v1";
const APPROVAL_REQUEST_SCHEMA: &str = "semaprax.economic-agent-approval-request.v1";
const APPROVAL_SCHEMA: &str = "semaprax.economic-agent-approval.v1";
const JOURNAL_SCHEMA: &str = "semaprax.economic-agent-journal.v1";
const BROADCAST_SCHEMA: &str = "semaprax.economic-agent-broadcast-receipt.v1";
const RECONCILIATION_SCHEMA: &str = "semaprax.economic-agent-reconciliation.v1";
const TRACE_SCHEMA: &str = "semaprax.economic-agent-trace.v1";
const EVIDENCE_SCHEMA: &str = "semaprax.economic-agent-evidence.v1";

const POLICY_DOMAIN: &[u8] = b"semaprax.economic-agent.policy-digest.v1\0";
const INTENT_DOMAIN: &[u8] = b"semaprax.economic-agent.payment-intent-digest.v1\0";
const INVOICE_DOMAIN: &[u8] = b"semaprax.economic-agent.x402-invoice-digest.v1\0";
const SNAPSHOT_DOMAIN: &[u8] = b"semaprax.economic-agent.chain-snapshot-digest.v1\0";
const PLAN_DOMAIN: &[u8] = b"semaprax.economic-agent.payment-plan-digest.v1\0";
const SIMULATION_DOMAIN: &[u8] = b"semaprax.economic-agent.simulation-digest.v1\0";
const APPROVAL_REQUEST_DOMAIN: &[u8] = b"semaprax.economic-agent.approval-request-digest.v1\0";
const APPROVAL_DOMAIN: &[u8] = b"semaprax.economic-agent.approval-digest.v1\0";
const JOURNAL_DOMAIN: &[u8] = b"semaprax.economic-agent.journal-digest.v1\0";
const BROADCAST_DOMAIN: &[u8] = b"semaprax.economic-agent.broadcast-receipt-digest.v1\0";
const RECONCILIATION_DOMAIN: &[u8] = b"semaprax.economic-agent.reconciliation-digest.v1\0";
const TRACE_DOMAIN: &[u8] = b"semaprax.economic-agent.trace-digest.v1\0";
const EVIDENCE_DOMAIN: &[u8] = b"semaprax.economic-agent.evidence-digest.v1\0";
const UNSIGNED_DOMAIN: &[u8] = b"semaprax.economic-agent.unsigned-transaction-digest.v1\0";
const SIGNED_DOMAIN: &[u8] = b"semaprax.economic-agent.signed-transaction-digest.v1\0";
const RUN_ID_DOMAIN: &[u8] = b"semaprax.economic-agent.run-id.v1\0";

const MAX_POLICY_BYTES: usize = 1_048_576;
const MAX_INTENT_BYTES: usize = 1_048_576;
const MAX_INVOICE_BYTES: usize = 1_048_576;
const MAX_SNAPSHOT_BYTES: usize = 1_048_576;
const MAX_PLAN_BYTES: usize = 1_048_576;
const MAX_SIMULATION_BYTES: usize = 1_048_576;
const MAX_APPROVAL_REQUEST_BYTES: usize = 1_048_576;
const MAX_APPROVAL_BYTES: usize = 65_536;
const MAX_JOURNAL_BYTES: usize = 8_388_608;
const MAX_UNSIGNED_BYTES: usize = 1_048_576;
const MAX_SIGNED_BYTES: usize = 2_097_152;
const MAX_BROADCAST_BYTES: usize = 1_048_576;
const MAX_RECONCILIATION_BYTES: usize = 1_048_576;
const MAX_TRACE_EVENTS: usize = 1_024;
const MAX_TRACE_BYTES: usize = 8_388_608;
const MAX_EVIDENCE_BYTES: usize = 16_777_216;
const MAX_BUILDER_BYTES: usize = 67_108_864;
const MAX_JSON_DEPTH: usize = 16;
const MAX_IDENTIFIER_BYTES: usize = 128;
const MAX_MEMO_BYTES: usize = 1_024;
const MAX_RECIPIENTS: usize = 128;
const MAX_NETWORK_POLICIES: usize = 16;
const MAX_X402_ORIGINS: usize = 32;
const MAX_UTXOS: usize = 100;

const NONCLAIMS: [&str; 28] = [
    "no_model_output_payment_authority",
    "no_model_self_approval_or_policy_expansion",
    "no_seed_private_key_credential_or_signing_material_input",
    "no_secret_prompt_trace_evidence_log_or_diagnostic_exposure",
    "no_builtin_network_http_dns_custody_or_chain_authority",
    "no_mainnet_authority",
    "no_wildcard_network_asset_recipient_origin_or_resource",
    "no_token_contract_program_script_swap_bridge_or_unlimited_approval",
    "no_raw_signing_or_signed_transaction_export",
    "no_exactly_once_signing_broadcast_or_payment",
    "no_automatic_uncertain_broadcast_retry",
    "no_guaranteed_confirmation_finality_or_reorg_freedom",
    "no_compromised_wallet_approver_adapter_provider_or_chain_recovery",
    "no_power_loss_durability_without_host_journal_contract",
    "no_cross_process_or_distributed_concurrency_guarantee",
    "no_live_price_exchange_rate_fee_or_cost_accuracy",
    "no_balance_allowance_or_simulation_truth_beyond_adapter",
    "no_human_identity_intent_approval_provenance_or_nonrepudiation",
    "no_signature_attestation_or_custody_provenance",
    "no_tax_accounting_legal_regulatory_sanctions_or_compliance_correctness",
    "no_privacy_data_residency_or_unlinkability_guarantee",
    "no_x402_redirect_ssrf_private_network_or_server_honesty_guarantee_beyond_admitted_adapter_contract",
    "no_automatic_refund_chargeback_replacement_or_fee_bumping",
    "no_wallet_recovery_rotation_backup_or_inheritance",
    "no_general_payment_sdk_or_production_readiness",
    "no_language_graph_cleanup_backend_or_workspace_atomicity_semantics",
    "no_current_agent_runtime_schema_api_or_kat_modification",
    "no_completion_matrix_status_promotion",
];

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
/// Outcome reported by an injected Economic Agent authority boundary.
pub enum EconomicAdapterDisposition {
    Succeeded,
    DefinitelyNotStarted,
    FailedUncertain,
    PolicyRejected,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
/// Outcome of the single injected journal load for an operation.
pub enum EconomicJournalLoad {
    Missing,
    Present,
    DefinitelyNotStarted,
    FailedUncertain,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
/// Frozen native-asset settlement rail.
pub enum EconomicRail {
    Evm,
    Solana,
    Bitcoin,
}

/// Opaque rolling-window reservation supplied only to the journal CAS.
pub struct EconomicRollingReservation {
    wallet_id: String,
    rail: EconomicRail,
    network: String,
    asset: String,
    requested_at_ms: u64,
    amount_atomic: u64,
    max_rolling_24h_atomic: u64,
}
impl EconomicRollingReservation {
    /// Bound wallet identifier.
    pub fn wallet_id(&self) -> &str {
        &self.wallet_id
    }
    /// Bound settlement rail.
    pub const fn rail(&self) -> EconomicRail {
        self.rail
    }
    /// Bound test network.
    pub fn network(&self) -> &str {
        &self.network
    }
    /// Bound native asset.
    pub fn asset(&self) -> &str {
        &self.asset
    }
    /// Admitted intent timestamp; the journal owns trusted clock time.
    pub const fn requested_at_ms(&self) -> u64 {
        self.requested_at_ms
    }
    /// Amount reserved in atomic native-asset units.
    pub const fn amount_atomic(&self) -> u64 {
        self.amount_atomic
    }
    /// Policy maximum for the matching rolling 24-hour tuple.
    pub const fn max_rolling_24h_atomic(&self) -> u64 {
        self.max_rolling_24h_atomic
    }
}
/// Atomic rolling-reservation update accompanying a journal CAS.
pub enum EconomicRollingReservationUpdate<'a> {
    Reserve(&'a EconomicRollingReservation),
    Retain,
    Release,
}

impl EconomicRail {
    fn text(self) -> &'static str {
        match self {
            Self::Evm => "evm",
            Self::Solana => "solana",
            Self::Bitcoin => "bitcoin",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
/// Closed terminal status returned by an Economic Agent operation.
pub enum EconomicRunStatus {
    Confirmed,
    Pending,
    Reorged,
    Dropped,
    Rejected,
    Cancelled,
    DeadlineExceeded,
    BudgetExhausted,
    JournalFailed,
    AdapterFailed,
    ApprovalFailed,
    CustodyFailed,
    BroadcastUnknown,
    ReconciliationFailed,
}

impl EconomicRunStatus {
    fn text(self) -> &'static str {
        match self {
            Self::Confirmed => "confirmed",
            Self::Pending => "pending",
            Self::Reorged => "reorged",
            Self::Dropped => "dropped",
            Self::Rejected => "rejected",
            Self::Cancelled => "cancelled",
            Self::DeadlineExceeded => "deadline_exceeded",
            Self::BudgetExhausted => "budget_exhausted",
            Self::JournalFailed => "journal_failed",
            Self::AdapterFailed => "adapter_failed",
            Self::ApprovalFailed => "approval_failed",
            Self::CustodyFailed => "custody_failed",
            Self::BroadcastUnknown => "broadcast_unknown",
            Self::ReconciliationFailed => "reconciliation_failed",
        }
    }
}

/// Push-only bounded sink for canonical adapter documents.
pub struct EconomicDocumentSink {
    bytes: Vec<u8>,
    limit: usize,
    closed: Option<SinkClose>,
    cancellation: AgentCancellation,
    probe: Box<dyn EconomicBoundaryProbe>,
    started_ms: u64,
    deadline_ms: u64,
    builder_limit: u64,
    terminal_floor: usize,
}

#[derive(Clone, Copy)]
enum SinkClose {
    Cancelled,
    Deadline,
    DeclaredLimit,
    Builder,
}

impl EconomicDocumentSink {
    fn new(
        limit: usize,
        cancellation: AgentCancellation,
        probe: Box<dyn EconomicBoundaryProbe>,
        started_ms: u64,
        deadline_ms: u64,
        builder_limit: u64,
        terminal_floor: usize,
    ) -> Self {
        Self {
            bytes: Vec::new(),
            limit,
            closed: None,
            cancellation,
            probe,
            started_ms,
            deadline_ms,
            builder_limit,
            terminal_floor,
        }
    }

    /// Appends one chunk, returning `false` permanently after closure.
    pub fn push(&mut self, chunk: &[u8]) -> bool {
        if self.closed.is_some() {
            return false;
        }
        if self.cancellation.is_cancelled() {
            self.closed = Some(SinkClose::Cancelled);
            return false;
        }
        if self
            .probe
            .elapsed_ms()
            .checked_sub(self.started_ms)
            .is_none_or(|elapsed| elapsed > self.deadline_ms)
        {
            self.closed = Some(SinkClose::Deadline);
            return false;
        }
        let Some(length) = self.bytes.len().checked_add(chunk.len()) else {
            self.closed = Some(SinkClose::DeclaredLimit);
            return false;
        };
        if length > self.limit {
            self.closed = Some(SinkClose::DeclaredLimit);
            return false;
        }
        if !reserve_active_preserving(chunk.len(), self.terminal_floor) {
            self.closed = Some(SinkClose::Builder);
            return false;
        }
        self.bytes.extend_from_slice(chunk);
        true
    }

    fn finish(self, field: &str) -> Result<String, Diagnostic> {
        match self.closed {
            Some(SinkClose::Cancelled) => {
                return Err(info("SPX-I228", "Economic Agent run was cancelled"))
            }
            Some(SinkClose::Deadline) => {
                return Err(info("SPX-I229", "Economic Agent deadline was exceeded"))
            }
            Some(SinkClose::DeclaredLimit) => return Err(g216(field, self.limit as u64)),
            Some(SinkClose::Builder) => return Err(g216("builder_bytes", self.builder_limit)),
            None => {}
        }
        String::from_utf8(self.bytes).map_err(|_| g210(field, "UTF-8"))
    }
}

/// Push-only bounded sink for opaque custody-produced signed bytes.
pub struct EconomicBytesSink {
    bytes: Vec<u8>,
    limit: usize,
    closed: Option<SinkClose>,
    cancellation: AgentCancellation,
    probe: Box<dyn EconomicBoundaryProbe>,
    started_ms: u64,
    deadline_ms: u64,
    builder_limit: u64,
    terminal_floor: usize,
}

impl EconomicBytesSink {
    fn new(
        limit: usize,
        cancellation: AgentCancellation,
        probe: Box<dyn EconomicBoundaryProbe>,
        started_ms: u64,
        deadline_ms: u64,
        builder_limit: u64,
        terminal_floor: usize,
    ) -> Self {
        Self {
            bytes: Vec::new(),
            limit,
            closed: None,
            cancellation,
            probe,
            started_ms,
            deadline_ms,
            builder_limit,
            terminal_floor,
        }
    }

    /// Appends one chunk, returning `false` permanently after closure.
    pub fn push(&mut self, chunk: &[u8]) -> bool {
        if self.closed.is_some() {
            return false;
        }
        if self.cancellation.is_cancelled() {
            self.closed = Some(SinkClose::Cancelled);
            return false;
        }
        if self
            .probe
            .elapsed_ms()
            .checked_sub(self.started_ms)
            .is_none_or(|elapsed| elapsed > self.deadline_ms)
        {
            self.closed = Some(SinkClose::Deadline);
            return false;
        }
        let Some(length) = self.bytes.len().checked_add(chunk.len()) else {
            self.closed = Some(SinkClose::DeclaredLimit);
            return false;
        };
        if length > self.limit {
            self.closed = Some(SinkClose::DeclaredLimit);
            return false;
        }
        if !reserve_active_preserving(chunk.len(), self.terminal_floor) {
            self.closed = Some(SinkClose::Builder);
            return false;
        }
        self.bytes.extend_from_slice(chunk);
        true
    }

    fn finish(self) -> Result<Vec<u8>, Diagnostic> {
        match self.closed {
            Some(SinkClose::Cancelled) => Err(info("SPX-I228", "Economic Agent run was cancelled")),
            Some(SinkClose::Deadline) => {
                Err(info("SPX-I229", "Economic Agent deadline was exceeded"))
            }
            Some(SinkClose::DeclaredLimit) => {
                Err(g216("signed_transaction_bytes", self.limit as u64))
            }
            Some(SinkClose::Builder) => Err(g216("builder_bytes", self.builder_limit)),
            None => Ok(self.bytes),
        }
    }
}

/// Caller-injected durable journal and rolling-window authority.
pub trait PaymentJournal {
    /// Loads the exact journal bound to `idempotency_key` into `sink`.
    fn load(
        &mut self,
        idempotency_key: &str,
        sink: &mut EconomicDocumentSink,
    ) -> EconomicJournalLoad;
    /// Atomically compares the version, writes canonical Journal bytes, and applies rolling policy.
    fn compare_and_swap(
        &mut self,
        idempotency_key: &str,
        expected_version: u64,
        journal: &str,
        rolling: EconomicRollingReservationUpdate<'_>,
    ) -> EconomicAdapterDisposition;
}

/// Caller-injected x402 invoice data adapter; it performs no redirects through this API.
pub trait X402InvoiceAdapter {
    /// Fetches the invoice bound to the admitted origin, method, and resource.
    fn fetch_invoice(
        &mut self,
        origin: &str,
        method: &str,
        resource: &str,
        sink: &mut EconomicDocumentSink,
    ) -> EconomicAdapterDisposition;
}

macro_rules! rail_adapter {
    ($name:ident,$snapshot:ident,$simulate:ident,$broadcast:ident,$reconcile:ident) => {
        /// Caller-injected test-network chain observation and broadcast adapter.
        pub trait $name {
            /// Returns one canonical snapshot for the admitted Intent.
            fn $snapshot(
                &mut self,
                intent: &str,
                sink: &mut EconomicDocumentSink,
            ) -> EconomicAdapterDisposition;
            /// Simulates the core-built unsigned transaction against its canonical Plan.
            fn $simulate(
                &mut self,
                plan: &str,
                unsigned_transaction: &[u8],
                sink: &mut EconomicDocumentSink,
            ) -> EconomicAdapterDisposition;
            /// Broadcasts the exact independently validated signed transaction once.
            fn $broadcast(
                &mut self,
                signed_transaction: &[u8],
                sink: &mut EconomicDocumentSink,
            ) -> EconomicAdapterDisposition;
            /// Returns one reconciliation observation for the bound transaction ID.
            fn $reconcile(
                &mut self,
                transaction_id: &str,
                sink: &mut EconomicDocumentSink,
            ) -> EconomicAdapterDisposition;
        }
    };
}
rail_adapter!(
    EvmPaymentAdapter,
    evm_snapshot,
    evm_simulate,
    evm_broadcast,
    evm_reconcile
);
rail_adapter!(
    SolanaPaymentAdapter,
    solana_snapshot,
    solana_simulate,
    solana_broadcast,
    solana_reconcile
);
rail_adapter!(
    BitcoinPaymentAdapter,
    bitcoin_snapshot,
    bitcoin_simulate,
    bitcoin_broadcast,
    bitcoin_reconcile
);

/// Caller-injected approval authority for an exact canonical Approval Request.
pub trait PaymentApprover {
    /// Returns one canonical Approval document.
    fn approve(
        &mut self,
        approval_request: &str,
        sink: &mut EconomicDocumentSink,
    ) -> EconomicAdapterDisposition;
}

/// Caller-injected opaque signing authority; key material never crosses this API.
pub trait WalletCustody {
    /// Signs exact digest-bound unsigned bytes into the push-only sink.
    fn sign(
        &mut self,
        wallet_id: &str,
        rail: EconomicRail,
        unsigned_transaction_digest: &str,
        unsigned_transaction: &[u8],
        approval_digest: &str,
        sink: &mut EconomicBytesSink,
    ) -> EconomicAdapterDisposition;
}

/// Pure caller-injected monotonic observation for one Economic Agent run.
pub trait EconomicBoundaryProbe {
    /// Returns a nondecreasing local elapsed-millisecond observation.
    fn elapsed_ms(&self) -> u64;
}

/// Complete caller-injected authority set required by [`EconomicAgent`].
pub trait EconomicAgentHost:
    PaymentJournal
    + X402InvoiceAdapter
    + EvmPaymentAdapter
    + SolanaPaymentAdapter
    + BitcoinPaymentAdapter
    + PaymentApprover
    + WalletCustody
{
    /// Creates a pure local elapsed-time probe; this method must not cross an external-effect boundary.
    fn boundary_probe(&self) -> Box<dyn EconomicBoundaryProbe>;
}

#[derive(Clone)]
struct Limits {
    max_policy_bytes: u64,
    max_intent_bytes: u64,
    max_invoice_bytes: u64,
    max_snapshot_bytes: u64,
    max_plan_bytes: u64,
    max_simulation_bytes: u64,
    max_approval_request_bytes: u64,
    max_approval_bytes: u64,
    max_journal_bytes: u64,
    max_unsigned_transaction_bytes: u64,
    max_signed_transaction_bytes: u64,
    max_broadcast_receipt_bytes: u64,
    max_reconciliation_bytes: u64,
    max_trace_events: u64,
    max_trace_bytes: u64,
    max_evidence_bytes: u64,
    max_builder_bytes: u64,
    max_json_depth: u64,
    max_identifier_bytes: u64,
    max_memo_bytes: u64,
    max_recipients: u64,
    max_network_policies: u64,
    max_x402_origins: u64,
    max_utxos: u64,
    max_reconciliations: u64,
    max_elapsed_ms: u64,
    max_amount_atomic: u64,
    max_fee_atomic: u64,
    max_compute_units: u64,
    max_confirmation_target: u64,
    max_concurrency: u64,
    max_unexpected_authority_calls: u64,
}

#[derive(Clone)]
struct NetworkPolicy {
    rail: EconomicRail,
    network: String,
    asset: String,
    recipients: Vec<String>,
    max_amount: u64,
    max_fee: u64,
    max_rolling: u64,
}

#[derive(Clone)]
struct OriginPolicy {
    origin: String,
    methods: Vec<String>,
    resources: Vec<String>,
    rails: Vec<EconomicRail>,
    max_amount: u64,
}

#[derive(Clone)]
struct Policy {
    economic_agent_id: String,
    wallet_id: String,
    networks: Vec<NetworkPolicy>,
    origins: Vec<OriginPolicy>,
    limits: Limits,
    source: String,
    digest: String,
}

#[derive(Clone)]
enum Payment {
    Evm {
        recipient: String,
        amount: u64,
        max_fee: u64,
    },
    Solana {
        recipient: String,
        amount: u64,
        max_fee: u64,
        compute: u64,
        priority: u64,
    },
    Bitcoin {
        recipient: String,
        amount: u64,
        max_fee: u64,
        confirmations: u64,
    },
    X402 {
        origin: String,
        method: String,
        resource: String,
        invoice_digest: String,
        payee: String,
        rail: EconomicRail,
        network: String,
        asset: String,
        amount: u64,
        max_fee: u64,
        invoice_expires: u64,
        nonce: String,
    },
}

#[derive(Clone)]
struct Intent {
    intent_id: String,
    wallet_id: String,
    rail_text: String,
    idempotency_key: String,
    created_at: u64,
    expires_at: u64,
    memo: Option<String>,
    payment: Payment,
    source: String,
    digest: String,
}

impl Intent {
    fn settlement_rail(&self) -> EconomicRail {
        match &self.payment {
            Payment::Evm { .. } => EconomicRail::Evm,
            Payment::Solana { .. } => EconomicRail::Solana,
            Payment::Bitcoin { .. } => EconomicRail::Bitcoin,
            Payment::X402 { rail, .. } => *rail,
        }
    }
    fn recipient(&self) -> &str {
        match &self.payment {
            Payment::Evm { recipient, .. }
            | Payment::Solana { recipient, .. }
            | Payment::Bitcoin { recipient, .. } => recipient,
            Payment::X402 { payee, .. } => payee,
        }
    }
    fn amount(&self) -> u64 {
        match &self.payment {
            Payment::Evm { amount, .. }
            | Payment::Solana { amount, .. }
            | Payment::Bitcoin { amount, .. }
            | Payment::X402 { amount, .. } => *amount,
        }
    }
    fn max_fee(&self) -> u64 {
        match &self.payment {
            Payment::Evm { max_fee, .. }
            | Payment::Solana { max_fee, .. }
            | Payment::Bitcoin { max_fee, .. }
            | Payment::X402 { max_fee, .. } => *max_fee,
        }
    }
    fn network_asset(&self) -> (&str, &str) {
        match &self.payment {
            Payment::Evm { .. } => ("sepolia", "native:eth"),
            Payment::Solana { .. } => ("devnet", "native:sol"),
            Payment::Bitcoin { .. } => ("regtest", "native:btc"),
            Payment::X402 { network, asset, .. } => (network, asset),
        }
    }
}

fn admit_intent(policy: &Policy, intent: &Intent) -> Result<(), Diagnostic> {
    if intent.source.len() > policy.limits.max_intent_bytes as usize {
        return Err(g216("intent_bytes", policy.limits.max_intent_bytes));
    }
    configured_depth(&intent.source, &policy.limits)?;
    if intent.wallet_id != policy.wallet_id {
        return Err(g212("wallet mismatch"));
    }
    let identifier_limit = policy.limits.max_identifier_bytes as usize;
    if [
        intent.intent_id.as_str(),
        intent.wallet_id.as_str(),
        intent.rail_text.as_str(),
        intent.idempotency_key.as_str(),
    ]
    .into_iter()
    .any(|value| value.len() > identifier_limit)
    {
        return Err(g216("identifier_bytes", policy.limits.max_identifier_bytes));
    }
    if intent
        .memo
        .as_ref()
        .is_some_and(|memo| memo.len() > policy.limits.max_memo_bytes as usize)
    {
        return Err(g216("memo_bytes", policy.limits.max_memo_bytes));
    }
    let rail = intent.settlement_rail();
    let (network, asset) = intent.network_asset();
    let Some(network_policy) = policy
        .networks
        .iter()
        .find(|row| row.rail == rail && row.network == network && row.asset == asset)
    else {
        return Err(g212("rail/network/asset not allowed"));
    };
    if !network_policy
        .recipients
        .iter()
        .any(|recipient| recipient == intent.recipient())
    {
        return Err(g212("recipient not allowed"));
    }
    if intent.amount() == 0
        || intent.amount() > network_policy.max_amount
        || intent.amount() > policy.limits.max_amount_atomic
        || intent.max_fee() > network_policy.max_fee
        || intent.max_fee() > policy.limits.max_fee_atomic
    {
        return Err(g212("amount or fee not allowed"));
    }
    match &intent.payment {
        Payment::Solana {
            compute, priority, ..
        } if *compute == 0
            || *compute > policy.limits.max_compute_units
            || *priority > intent.max_fee() =>
        {
            return Err(g212("amount or fee not allowed"))
        }
        Payment::Bitcoin { confirmations, .. }
            if *confirmations == 0 || *confirmations > policy.limits.max_confirmation_target =>
        {
            return Err(g212("amount or fee not allowed"))
        }
        Payment::X402 {
            origin,
            method,
            resource,
            rail,
            nonce,
            ..
        } => {
            if *rail == EconomicRail::Solana && policy.limits.max_compute_units < 200_000 {
                return Err(g212("amount or fee not allowed"));
            }
            if nonce.len() > identifier_limit {
                return Err(g216("identifier_bytes", policy.limits.max_identifier_bytes));
            }
            let Some(row) = policy.origins.iter().find(|row| row.origin == *origin) else {
                return Err(g212("origin/method/resource not allowed"));
            };
            if !row.methods.iter().any(|v| v == method)
                || !row.resources.iter().any(|v| v == resource)
                || !row.rails.contains(rail)
                || intent.amount() > row.max_amount
            {
                return Err(g212("origin/method/resource not allowed"));
            }
        }
        _ => {}
    }
    Ok(())
}

#[derive(Clone)]
struct Doc {
    source: String,
    digest: String,
}
#[derive(Clone)]
struct DocRef {
    digest: String,
    bytes: u64,
}
impl From<&Doc> for DocRef {
    fn from(value: &Doc) -> Self {
        Self {
            digest: value.digest.clone(),
            bytes: value.source.len() as u64,
        }
    }
}

#[derive(Clone, Default)]
struct Usage {
    journal_reads: u64,
    journal_writes: u64,
    invoice_reads: u64,
    snapshot_reads: u64,
    simulations: u64,
    approvals: u64,
    signatures: u64,
    broadcasts: u64,
    reconciliations: u64,
    input_bytes: u64,
    output_bytes: u64,
    elapsed_ms: u64,
}

#[derive(Clone)]
struct Event {
    kind: &'static str,
    rail: Option<EconomicRail>,
    input: Option<String>,
    output: Option<String>,
    status: &'static str,
    usage: Usage,
    authority_uncertain: bool,
}

#[derive(Clone)]
struct Terminal {
    status: EconomicRunStatus,
    transaction_id: Option<String>,
    confirmation: Option<String>,
    code: Option<String>,
    message: Option<String>,
}

#[derive(Clone, Default)]
struct Budget {
    policy_bytes: u64,
    intent_bytes: u64,
    invoice_bytes: u64,
    snapshot_bytes: u64,
    plan_bytes: u64,
    simulation_bytes: u64,
    approval_request_bytes: u64,
    approval_bytes: u64,
    journal_bytes: u64,
    unsigned_bytes: u64,
    signed_bytes: u64,
    broadcast_bytes: u64,
    reconciliation_bytes: u64,
    trace_events: u64,
    trace_bytes: u64,
    evidence_bytes: u64,
    builder_bytes: u64,
    recipients: u64,
    network_policies: u64,
    x402_origins: u64,
    utxos: u64,
    reconciliations: u64,
    elapsed_ms: u64,
    concurrency: u64,
    unexpected_authority_calls: u64,
}

/// Opaque replay-validated Economic Agent result.
pub struct EconomicRun {
    status: EconomicRunStatus,
    transaction_id: Option<String>,
    confirmation_status: Option<String>,
    trace: String,
    trace_digest: String,
    evidence: String,
    evidence_digest: String,
}

impl EconomicRun {
    /// Returns the closed terminal status.
    pub const fn status(&self) -> EconomicRunStatus {
        self.status
    }
    /// Returns the bound transaction ID when present.
    pub fn transaction_id(&self) -> Option<&str> {
        self.transaction_id.as_deref()
    }
    /// Returns the latest confirmation status when present.
    pub fn confirmation_status(&self) -> Option<&str> {
        self.confirmation_status.as_deref()
    }
    /// Returns canonical Trace v1 JSON.
    pub fn trace(&self) -> &str {
        &self.trace
    }
    /// Returns the domain-separated Trace digest.
    pub fn trace_digest(&self) -> &str {
        &self.trace_digest
    }
    /// Returns canonical Evidence v1 JSON.
    pub fn evidence(&self) -> &str {
        &self.evidence
    }
    /// Returns the domain-separated Evidence digest.
    pub fn evidence_digest(&self) -> &str {
        &self.evidence_digest
    }
}

/// Opaque single-concurrency Economic Agent owning its injected host.
pub struct EconomicAgent<H: EconomicAgentHost> {
    policy: Policy,
    retained_policy_bytes: usize,
    host: H,
    cancellation: AgentCancellation,
}

impl<H: EconomicAgentHost> EconomicAgent<H> {
    fn terminal_floor(&self) -> Result<usize, Diagnostic> {
        let lane = terminal_floor(&self.policy.limits)?;
        if active_remaining().is_some_and(|remaining| remaining < lane) {
            return Err(g216("builder_bytes", self.policy.limits.max_builder_bytes));
        }
        if !set_active_floor(lane) {
            return Err(g216("builder_bytes", self.policy.limits.max_builder_bytes));
        }
        Ok(lane)
    }

    /// Parses and retains one canonical Policy before consulting the host.
    pub fn new(
        policy: &str,
        host: H,
        cancellation: AgentCancellation,
    ) -> Result<Self, Vec<Diagnostic>> {
        let (result, overflowed, consumed) = with_limit_usage(MAX_BUILDER_BYTES, || {
            if !reserve_active(policy.len().saturating_mul(MAX_JSON_DEPTH + 2)) {
                return Err(g216("builder_bytes", MAX_BUILDER_BYTES as u64));
            }
            parse_policy(policy)
        });
        if overflowed {
            return Err(vec![g216("builder_bytes", MAX_BUILDER_BYTES as u64)]);
        }
        result
            .map(|policy| Self {
                policy,
                retained_policy_bytes: consumed,
                host,
                cancellation,
            })
            .map_err(|diagnostic| vec![diagnostic])
    }

    /// Executes one canonical Payment Intent proposed by a completed sealed Agent run.
    pub fn execute(&mut self, source: &AgentRun) -> Result<EconomicRun, Vec<Diagnostic>> {
        let binding = source.economic_binding();
        if binding.status != AgentRunStatus::Completed {
            return Err(vec![g212("agent run not completed")]);
        }
        let Some(message) = binding.final_message else {
            return Err(vec![g212("agent run not completed")]);
        };
        if self.cancellation.is_cancelled() {
            return Err(vec![info("SPX-I228", "Economic Agent run was cancelled")]);
        }
        let policy_limit = self.policy.limits.max_builder_bytes as usize;
        let started = self.host.boundary_probe().elapsed_ms();
        let result = with_limit_usage(policy_limit, || {
            if !reserve_active(self.retained_policy_bytes) {
                return Err(g216("builder_bytes", self.policy.limits.max_builder_bytes));
            }
            if !reserve_active(message.len().saturating_mul(MAX_JSON_DEPTH + 2)) {
                return Err(g216("builder_bytes", self.policy.limits.max_builder_bytes));
            }
            let intent = parse_intent(message).and_then(|intent| {
                admit_intent(&self.policy, &intent)?;
                Ok(intent)
            })?;
            self.terminal_floor()?;
            self.execute_bounded(&binding, intent, started)
        });
        match result {
            (Ok(run), false, _) => Ok(run),
            (Err(diagnostic), _, _) => Err(vec![diagnostic]),
            (Ok(_), true, _) => Err(vec![g216(
                "builder_bytes",
                self.policy.limits.max_builder_bytes,
            )]),
        }
    }

    fn execute_bounded(
        &mut self,
        binding: &crate::agent_runtime::EconomicAgentBinding<'_>,
        intent: Intent,
        started: u64,
    ) -> Result<EconomicRun, Diagnostic> {
        let profile_cost = binding.evidence.len();
        if !reserve_active(profile_cost) {
            return Err(g216("builder_bytes", self.policy.limits.max_builder_bytes));
        }
        let economic_run_id = run_id(
            binding.evidence_digest,
            &self.policy.digest,
            &intent.digest,
            &intent.idempotency_key,
        );
        let policy_doc = Doc {
            source: self.policy.source.clone(),
            digest: self.policy.digest.clone(),
        };
        let intent_doc = Doc {
            source: intent.source.clone(),
            digest: intent.digest.clone(),
        };
        let mut journal = Journal {
            idempotency_key: intent.idempotency_key.clone(),
            version: 0,
            policy: policy_doc,
            intent: intent_doc,
            run_id: economic_run_id.clone(),
            state: JournalState::Reserved,
            reserved_amount: intent.amount(),
            reserved_fee: intent.max_fee(),
            plan: None,
            simulation: None,
            approval: None,
            unsigned: None,
            signed: None,
            broadcast: None,
            reconciliation: None,
            updated_at: intent.created_at,
        };
        let mut budget = Budget {
            policy_bytes: self.policy.source.len() as u64,
            intent_bytes: intent.source.len() as u64,
            recipients: self
                .policy
                .networks
                .iter()
                .map(|row| row.recipients.len() as u64)
                .sum(),
            network_policies: self.policy.networks.len() as u64,
            x402_origins: self.policy.origins.len() as u64,
            concurrency: 1,
            ..Budget::default()
        };
        let mut events = Vec::new();
        push_event(
            &mut events,
            event(
                "run_started",
                None,
                Some(binding.evidence_digest.to_owned()),
                Some(self.policy.digest.clone()),
                "started",
                Usage::default(),
            )?,
        )?;
        self.pre_call(started, self.policy.limits.max_journal_bytes as usize)?;
        let mut load_sink = EconomicDocumentSink::new(
            self.policy.limits.max_journal_bytes as usize,
            self.cancellation.clone(),
            self.host.boundary_probe(),
            started,
            self.policy.limits.max_elapsed_ms,
            self.policy.limits.max_builder_bytes,
            self.terminal_floor()?,
        );
        let load = self.host.load(&intent.idempotency_key, &mut load_sink);
        let mut load_usage = Usage::default();
        load_usage.journal_reads = 1;
        let loaded = match load {
            EconomicJournalLoad::Missing => {
                push_event(
                    &mut events,
                    event("journal_loaded", None, None, None, "missing", load_usage)?,
                )?;
                None
            }
            EconomicJournalLoad::Present => {
                let source = match load_sink.finish("journal_bytes") {
                    Ok(source) => source,
                    Err(diagnostic) => {
                        push_event(
                            &mut events,
                            event("journal_loaded", None, None, None, "failed", load_usage)?,
                        )?;
                        return finish_run(
                            &economic_run_id,
                            binding,
                            &self.policy,
                            &intent,
                            None,
                            None,
                            None,
                            None,
                            &journal,
                            None,
                            None,
                            &mut events,
                            diagnostic_terminal(&diagnostic),
                            &mut budget,
                            started,
                        );
                    }
                };
                load_usage.output_bytes = source.len() as u64;
                push_event(
                    &mut events,
                    event(
                        "journal_loaded",
                        None,
                        None,
                        Some(digest(JOURNAL_DOMAIN, source.as_bytes())),
                        "present",
                        load_usage,
                    )?,
                )?;
                let parsed = match parse_journal(&source, &self.policy, &intent, &economic_run_id) {
                    Ok(parsed) => parsed,
                    Err(diagnostic) => {
                        return finish_run(
                            &economic_run_id,
                            binding,
                            &self.policy,
                            &intent,
                            None,
                            None,
                            None,
                            None,
                            &journal,
                            None,
                            None,
                            &mut events,
                            diagnostic_terminal(&diagnostic),
                            &mut budget,
                            started,
                        );
                    }
                };
                budget.journal_bytes = source.len() as u64;
                Some(parsed)
            }
            EconomicJournalLoad::DefinitelyNotStarted | EconomicJournalLoad::FailedUncertain => {
                push_event(
                    &mut events,
                    event("journal_loaded", None, None, None, "failed", load_usage)?,
                )?;
                let terminal =
                    diagnostic_terminal(&info("SPX-I222", "Economic Agent journal adapter failed"));
                return finish_run(
                    &economic_run_id,
                    binding,
                    &self.policy,
                    &intent,
                    None,
                    None,
                    None,
                    None,
                    &journal,
                    None,
                    None,
                    &mut events,
                    terminal,
                    &mut budget,
                    started,
                );
            }
        };
        if let Some(existing) = loaded {
            if existing.version == 1 && existing.state == JournalState::Reserved {
                return self.execute_reserved(
                    binding,
                    intent,
                    economic_run_id,
                    existing,
                    events,
                    budget,
                    started,
                );
            }
            if existing.version < 6 || existing.broadcast.is_none() {
                budget.signed_bytes = existing.signed.as_ref().map_or(0, |value| value.1 as u64);
                return finish_run(
                    &economic_run_id,
                    binding,
                    &self.policy,
                    &intent,
                    None,
                    None,
                    None,
                    None,
                    &existing,
                    None,
                    None,
                    &mut events,
                    diagnostic_terminal(&info("SPX-I222", "Economic Agent journal adapter failed")),
                    &mut budget,
                    self.elapsed_ms(started)?,
                );
            }
            return self.resume_loaded(
                binding,
                intent,
                economic_run_id,
                existing,
                events,
                budget,
                started,
            );
        }
        let (network, asset) = intent.network_asset();
        let max_rolling = self
            .policy
            .networks
            .iter()
            .find(|row| {
                row.rail == intent.settlement_rail() && row.network == network && row.asset == asset
            })
            .ok_or_else(|| g212("rail/network/asset not allowed"))?
            .max_rolling;
        let rolling = EconomicRollingReservation {
            wallet_id: intent.wallet_id.clone(),
            rail: intent.settlement_rail(),
            network: network.to_owned(),
            asset: asset.to_owned(),
            requested_at_ms: intent.created_at,
            amount_atomic: intent.amount(),
            max_rolling_24h_atomic: max_rolling,
        };
        if let Err(diagnostic) = cas_journal(
            &mut self.host,
            &mut journal,
            &mut events,
            &mut budget,
            self.policy.limits.max_journal_bytes,
            EconomicRollingReservationUpdate::Reserve(&rolling),
        ) {
            return finish_run(
                &economic_run_id,
                binding,
                &self.policy,
                &intent,
                None,
                None,
                None,
                None,
                &journal,
                None,
                None,
                &mut events,
                diagnostic_terminal(&diagnostic),
                &mut budget,
                started,
            );
        }
        push_event(
            &mut events,
            event(
                "intent_reserved",
                Some(intent.settlement_rail()),
                Some(intent.digest.clone()),
                Some(journal_digest(&journal)),
                "reserved",
                Usage::default(),
            )?,
        )?;
        self.execute_reserved(
            binding,
            intent,
            economic_run_id,
            journal,
            events,
            budget,
            started,
        )
    }

    fn execute_reserved(
        &mut self,
        binding: &crate::agent_runtime::EconomicAgentBinding<'_>,
        intent: Intent,
        economic_run_id: String,
        mut journal: Journal,
        mut events: Vec<Event>,
        mut budget: Budget,
        started: u64,
    ) -> Result<EconomicRun, Diagnostic> {
        let rail = intent.settlement_rail();
        let (network, _) = intent.network_asset();
        macro_rules! terminal_try {
            ($expression:expr, $invoice:expr, $plan:expr, $simulation:expr, $approval:expr, $broadcast:expr, $reconciliation:expr) => {
                match $expression {
                    Ok(value) => value,
                    Err(diagnostic) => {
                        return self.finish_failure(
                            binding,
                            &intent,
                            &economic_run_id,
                            &journal,
                            $invoice,
                            $plan,
                            $simulation,
                            $approval,
                            $broadcast,
                            $reconciliation,
                            &mut events,
                            &mut budget,
                            diagnostic,
                            started,
                        )
                    }
                }
            };
        }
        let invoice = if let Payment::X402 {
            origin,
            method,
            resource,
            ..
        } = &intent.payment
        {
            terminal_try!(
                self.pre_call(started, self.policy.limits.max_invoice_bytes as usize),
                None,
                None,
                None,
                None,
                None,
                None
            );
            let mut sink = EconomicDocumentSink::new(
                self.policy.limits.max_invoice_bytes as usize,
                self.cancellation.clone(),
                self.host.boundary_probe(),
                started,
                self.policy.limits.max_elapsed_ms,
                self.policy.limits.max_builder_bytes,
                self.terminal_floor()?,
            );
            let disposition = self.host.fetch_invoice(origin, method, resource, &mut sink);
            let mut usage = Usage::default();
            usage.invoice_reads = 1;
            if disposition != EconomicAdapterDisposition::Succeeded {
                push_event(
                    &mut events,
                    event(
                        "invoice_loaded",
                        Some(rail),
                        Some(intent.digest.clone()),
                        None,
                        "failed",
                        usage,
                    )?,
                )?;
                return self.finish_failure(
                    binding,
                    &intent,
                    &economic_run_id,
                    &journal,
                    None,
                    None,
                    None,
                    None,
                    None,
                    None,
                    &mut events,
                    &mut budget,
                    info("SPX-I223", "Economic Agent chain adapter failed"),
                    started,
                );
            }
            let source = terminal_try!(
                sink.finish("invoice_bytes"),
                None,
                None,
                None,
                None,
                None,
                None
            );
            usage.output_bytes = source.len() as u64;
            let parsed = terminal_try!(
                parse_invoice_limited(&source, &intent, &self.policy.limits),
                None,
                None,
                None,
                None,
                None,
                None
            );
            budget.invoice_bytes = source.len() as u64;
            push_event(
                &mut events,
                event(
                    "invoice_loaded",
                    Some(rail),
                    Some(intent.digest.clone()),
                    Some(parsed.doc.digest.clone()),
                    "loaded",
                    usage,
                )?,
            )?;
            Some(parsed)
        } else {
            None
        };
        terminal_try!(
            self.pre_call(started, self.policy.limits.max_snapshot_bytes as usize),
            invoice.as_ref(),
            None,
            None,
            None,
            None,
            None
        );
        let mut snapshot_sink = EconomicDocumentSink::new(
            self.policy.limits.max_snapshot_bytes as usize,
            self.cancellation.clone(),
            self.host.boundary_probe(),
            started,
            self.policy.limits.max_elapsed_ms,
            self.policy.limits.max_builder_bytes,
            self.terminal_floor()?,
        );
        let disposition = match rail {
            EconomicRail::Evm => self.host.evm_snapshot(&intent.source, &mut snapshot_sink),
            EconomicRail::Solana => self
                .host
                .solana_snapshot(&intent.source, &mut snapshot_sink),
            EconomicRail::Bitcoin => self
                .host
                .bitcoin_snapshot(&intent.source, &mut snapshot_sink),
        };
        let mut snapshot_usage = Usage::default();
        snapshot_usage.snapshot_reads = 1;
        if disposition != EconomicAdapterDisposition::Succeeded {
            push_event(
                &mut events,
                event(
                    "snapshot_loaded",
                    Some(rail),
                    Some(intent.digest.clone()),
                    None,
                    "failed",
                    snapshot_usage,
                )?,
            )?;
            return self.finish_failure(
                binding,
                &intent,
                &economic_run_id,
                &journal,
                invoice.as_ref(),
                None,
                None,
                None,
                None,
                None,
                &mut events,
                &mut budget,
                info("SPX-I223", "Economic Agent chain adapter failed"),
                started,
            );
        }
        let snapshot_source = terminal_try!(
            snapshot_sink.finish("snapshot_bytes"),
            invoice.as_ref(),
            None,
            None,
            None,
            None,
            None
        );
        snapshot_usage.output_bytes = snapshot_source.len() as u64;
        let snapshot = terminal_try!(
            parse_snapshot_limited(&snapshot_source, rail, &self.policy.limits),
            invoice.as_ref(),
            None,
            None,
            None,
            None,
            None
        );
        budget.snapshot_bytes = snapshot_source.len() as u64;
        if let SnapshotState::Bitcoin { utxos, .. } = &snapshot.state {
            budget.utxos = utxos.len() as u64;
        }
        push_event(
            &mut events,
            event(
                "snapshot_loaded",
                Some(rail),
                Some(intent.digest.clone()),
                Some(snapshot.doc.digest.clone()),
                "loaded",
                snapshot_usage,
            )?,
        )?;
        let (unsigned, format) = terminal_try!(
            build_unsigned_limited(
                &intent,
                &snapshot,
                self.policy.limits.max_unsigned_transaction_bytes,
                self.policy.limits.max_builder_bytes,
                self.terminal_floor()?,
            ),
            invoice.as_ref(),
            None,
            None,
            None,
            None,
            None
        );
        if unsigned.len() > self.policy.limits.max_unsigned_transaction_bytes as usize {
            return Err(g216(
                "unsigned_transaction_bytes",
                self.policy.limits.max_unsigned_transaction_bytes,
            ));
        }
        let plan = terminal_try!(
            make_plan(
                &economic_run_id,
                binding.run_id,
                binding.evidence,
                binding.evidence_digest,
                &self.policy,
                &intent,
                invoice.as_ref(),
                &snapshot,
                unsigned,
                format,
            ),
            invoice.as_ref(),
            None,
            None,
            None,
            None,
            None
        );
        budget.plan_bytes = plan.doc.source.len() as u64;
        budget.unsigned_bytes = plan.unsigned.len() as u64;
        push_event(
            &mut events,
            event(
                "plan_built",
                Some(rail),
                Some(snapshot.doc.digest.clone()),
                Some(plan.doc.digest.clone()),
                "built",
                Usage::default(),
            )?,
        )?;
        terminal_try!(
            self.pre_call(started, self.policy.limits.max_simulation_bytes as usize),
            invoice.as_ref(),
            Some(&plan),
            None,
            None,
            None,
            None
        );
        let mut simulation_sink = EconomicDocumentSink::new(
            self.policy.limits.max_simulation_bytes as usize,
            self.cancellation.clone(),
            self.host.boundary_probe(),
            started,
            self.policy.limits.max_elapsed_ms,
            self.policy.limits.max_builder_bytes,
            self.terminal_floor()?,
        );
        let disposition = match rail {
            EconomicRail::Evm => {
                self.host
                    .evm_simulate(&plan.doc.source, &plan.unsigned, &mut simulation_sink)
            }
            EconomicRail::Solana => {
                self.host
                    .solana_simulate(&plan.doc.source, &plan.unsigned, &mut simulation_sink)
            }
            EconomicRail::Bitcoin => {
                self.host
                    .bitcoin_simulate(&plan.doc.source, &plan.unsigned, &mut simulation_sink)
            }
        };
        let mut sim_usage = Usage::default();
        sim_usage.simulations = 1;
        if disposition != EconomicAdapterDisposition::Succeeded {
            push_event(
                &mut events,
                event(
                    "simulation_finished",
                    Some(rail),
                    Some(plan.doc.digest.clone()),
                    None,
                    "failed",
                    sim_usage,
                )?,
            )?;
            return self.finish_failure(
                binding,
                &intent,
                &economic_run_id,
                &journal,
                invoice.as_ref(),
                Some(&plan),
                None,
                None,
                None,
                None,
                &mut events,
                &mut budget,
                info("SPX-I223", "Economic Agent chain adapter failed"),
                started,
            );
        }
        let simulation_source = terminal_try!(
            simulation_sink.finish("simulation_bytes"),
            invoice.as_ref(),
            Some(&plan),
            None,
            None,
            None,
            None
        );
        sim_usage.output_bytes = simulation_source.len() as u64;
        let simulation = terminal_try!(
            parse_simulation_limited(&simulation_source, &plan, &intent, &self.policy.limits),
            invoice.as_ref(),
            Some(&plan),
            None,
            None,
            None,
            None
        );
        budget.simulation_bytes = simulation_source.len() as u64;
        push_event(
            &mut events,
            event(
                "simulation_finished",
                Some(rail),
                Some(plan.doc.digest.clone()),
                Some(simulation.doc.digest.clone()),
                "succeeded",
                sim_usage,
            )?,
        )?;
        let mut prepared_journal = terminal_try!(
            clone_journal_bounded(&journal, self.policy.limits.max_builder_bytes),
            invoice.as_ref(),
            Some(&plan),
            Some(&simulation),
            None,
            None,
            None
        );
        prepared_journal.state = JournalState::Prepared;
        prepared_journal.plan = Some(DocRef::from(&plan.doc));
        prepared_journal.simulation = Some(DocRef::from(&simulation.doc));
        prepared_journal.unsigned = Some((
            plan.unsigned_digest.clone(),
            plan.unsigned.len(),
            plan.format,
        ));
        prepared_journal.updated_at = snapshot.observed;
        terminal_try!(
            cas_journal(
                &mut self.host,
                &mut prepared_journal,
                &mut events,
                &mut budget,
                self.policy.limits.max_journal_bytes,
                EconomicRollingReservationUpdate::Retain,
            ),
            invoice.as_ref(),
            Some(&plan),
            Some(&simulation),
            None,
            None,
            None
        );
        journal = prepared_journal;
        let request = terminal_try!(
            make_approval_request(&economic_run_id, &self.policy, &intent, &plan, &simulation),
            invoice.as_ref(),
            Some(&plan),
            Some(&simulation),
            None,
            None,
            None
        );
        budget.approval_request_bytes = request.source.len() as u64;
        terminal_try!(
            self.pre_call(started, self.policy.limits.max_approval_bytes as usize),
            invoice.as_ref(),
            Some(&plan),
            Some(&simulation),
            None,
            None,
            None
        );
        let mut approval_sink = EconomicDocumentSink::new(
            self.policy.limits.max_approval_bytes as usize,
            self.cancellation.clone(),
            self.host.boundary_probe(),
            started,
            self.policy.limits.max_elapsed_ms,
            self.policy.limits.max_builder_bytes,
            self.terminal_floor()?,
        );
        let disposition = self.host.approve(&request.source, &mut approval_sink);
        let mut approval_usage = Usage::default();
        approval_usage.approvals = 1;
        if disposition != EconomicAdapterDisposition::Succeeded {
            push_event(
                &mut events,
                event(
                    "approval_finished",
                    Some(rail),
                    Some(request.digest.clone()),
                    None,
                    "failed",
                    approval_usage,
                )?,
            )?;
            return self.finish_failure(
                binding,
                &intent,
                &economic_run_id,
                &journal,
                invoice.as_ref(),
                Some(&plan),
                Some(&simulation),
                None,
                None,
                None,
                &mut events,
                &mut budget,
                info("SPX-I224", "Economic Agent approval adapter failed"),
                started,
            );
        }
        let approval_source = terminal_try!(
            approval_sink.finish("approval_bytes"),
            invoice.as_ref(),
            Some(&plan),
            Some(&simulation),
            None,
            None,
            None
        );
        approval_usage.output_bytes = approval_source.len() as u64;
        let approval = terminal_try!(
            parse_approval_limited(
                &approval_source,
                &self.policy,
                &intent,
                &plan,
                &simulation,
                &request,
            ),
            invoice.as_ref(),
            Some(&plan),
            Some(&simulation),
            None,
            None,
            None
        );
        budget.approval_bytes = approval_source.len() as u64;
        push_event(
            &mut events,
            event(
                "approval_finished",
                Some(rail),
                Some(request.digest.clone()),
                Some(approval.doc.digest.clone()),
                "approved",
                approval_usage,
            )?,
        )?;
        let mut approved_journal = terminal_try!(
            clone_journal_bounded(&journal, self.policy.limits.max_builder_bytes),
            invoice.as_ref(),
            Some(&plan),
            Some(&simulation),
            Some(&approval),
            None,
            None
        );
        approved_journal.state = JournalState::Approved;
        approved_journal.approval = Some(DocRef::from(&approval.doc));
        approved_journal.updated_at = snapshot.observed;
        terminal_try!(
            cas_journal(
                &mut self.host,
                &mut approved_journal,
                &mut events,
                &mut budget,
                self.policy.limits.max_journal_bytes,
                EconomicRollingReservationUpdate::Retain,
            ),
            invoice.as_ref(),
            Some(&plan),
            Some(&simulation),
            Some(&approval),
            None,
            None
        );
        journal = approved_journal;
        terminal_try!(
            self.pre_call(
                started,
                self.policy.limits.max_signed_transaction_bytes as usize,
            ),
            invoice.as_ref(),
            Some(&plan),
            Some(&simulation),
            Some(&approval),
            None,
            None
        );
        let current_ms = terminal_try!(
            admitted_now_from(&intent, snapshot.observed, self.elapsed_ms(started)?),
            invoice.as_ref(),
            Some(&plan),
            Some(&simulation),
            Some(&approval),
            None,
            None
        );
        if current_ms >= plan.expires || current_ms >= simulation.expires {
            return self.finish_failure(
                binding,
                &intent,
                &economic_run_id,
                &journal,
                invoice.as_ref(),
                Some(&plan),
                Some(&simulation),
                Some(&approval),
                None,
                None,
                &mut events,
                &mut budget,
                g212("expired"),
                started,
            );
        }
        let mut sign_marker = terminal_try!(
            clone_journal_bounded(&journal, self.policy.limits.max_builder_bytes),
            invoice.as_ref(),
            Some(&plan),
            Some(&simulation),
            Some(&approval),
            None,
            None
        );
        terminal_try!(
            cas_journal(
                &mut self.host,
                &mut sign_marker,
                &mut events,
                &mut budget,
                self.policy.limits.max_journal_bytes,
                EconomicRollingReservationUpdate::Retain,
            ),
            invoice.as_ref(),
            Some(&plan),
            Some(&simulation),
            Some(&approval),
            None,
            None
        );
        journal = sign_marker;
        terminal_try!(
            self.pre_call(
                started,
                self.policy.limits.max_signed_transaction_bytes as usize,
            ),
            invoice.as_ref(),
            Some(&plan),
            Some(&simulation),
            Some(&approval),
            None,
            None
        );
        let mut signed_sink = EconomicBytesSink::new(
            self.policy.limits.max_signed_transaction_bytes as usize,
            self.cancellation.clone(),
            self.host.boundary_probe(),
            started,
            self.policy.limits.max_elapsed_ms,
            self.policy.limits.max_builder_bytes,
            self.terminal_floor()?,
        );
        let disposition = self.host.sign(
            &self.policy.wallet_id,
            rail,
            &plan.unsigned_digest,
            &plan.unsigned,
            &approval.doc.digest,
            &mut signed_sink,
        );
        let mut sign_usage = Usage::default();
        sign_usage.signatures = 1;
        if disposition != EconomicAdapterDisposition::Succeeded {
            push_event(
                &mut events,
                event(
                    "transaction_signed",
                    Some(rail),
                    Some(plan.unsigned_digest.clone()),
                    None,
                    "failed",
                    sign_usage,
                )?,
            )?;
            return self.finish_failure(
                binding,
                &intent,
                &economic_run_id,
                &journal,
                invoice.as_ref(),
                Some(&plan),
                Some(&simulation),
                Some(&approval),
                None,
                None,
                &mut events,
                &mut budget,
                info("SPX-I225", "Economic Agent custody adapter failed"),
                started,
            );
        }
        let signed = match signed_sink.finish() {
            Ok(value) => value,
            Err(diagnostic) => {
                push_event(
                    &mut events,
                    event(
                        "transaction_signed",
                        Some(rail),
                        Some(plan.unsigned_digest.clone()),
                        None,
                        "failed",
                        sign_usage,
                    )?,
                )?;
                return self.finish_failure(
                    binding,
                    &intent,
                    &economic_run_id,
                    &journal,
                    invoice.as_ref(),
                    Some(&plan),
                    Some(&simulation),
                    Some(&approval),
                    None,
                    None,
                    &mut events,
                    &mut budget,
                    diagnostic,
                    started,
                );
            }
        };
        if let Err(diagnostic) = verify_signed(rail, &plan.unsigned, &signed) {
            push_event(
                &mut events,
                event(
                    "transaction_signed",
                    Some(rail),
                    Some(plan.unsigned_digest.clone()),
                    None,
                    "failed",
                    sign_usage,
                )?,
            )?;
            return self.finish_failure(
                binding,
                &intent,
                &economic_run_id,
                &journal,
                invoice.as_ref(),
                Some(&plan),
                Some(&simulation),
                Some(&approval),
                None,
                None,
                &mut events,
                &mut budget,
                diagnostic,
                started,
            );
        }
        let signed_digest = digest(SIGNED_DOMAIN, &signed);
        budget.signed_bytes = signed.len() as u64;
        sign_usage.output_bytes = signed.len() as u64;
        push_event(
            &mut events,
            event(
                "transaction_signed",
                Some(rail),
                Some(plan.unsigned_digest.clone()),
                Some(signed_digest.clone()),
                "signed",
                sign_usage,
            )?,
        )?;
        let mut signed_journal = terminal_try!(
            clone_journal_bounded(&journal, self.policy.limits.max_builder_bytes),
            invoice.as_ref(),
            Some(&plan),
            Some(&simulation),
            Some(&approval),
            None,
            None
        );
        signed_journal.state = JournalState::Signed;
        signed_journal.signed = Some((signed_digest.clone(), signed.len()));
        signed_journal.updated_at = snapshot.observed;
        terminal_try!(
            cas_journal(
                &mut self.host,
                &mut signed_journal,
                &mut events,
                &mut budget,
                self.policy.limits.max_journal_bytes,
                EconomicRollingReservationUpdate::Retain,
            ),
            invoice.as_ref(),
            Some(&plan),
            Some(&simulation),
            Some(&approval),
            None,
            None
        );
        journal = signed_journal;
        let current_ms = terminal_try!(
            admitted_now_from(&intent, snapshot.observed, self.elapsed_ms(started)?),
            invoice.as_ref(),
            Some(&plan),
            Some(&simulation),
            Some(&approval),
            None,
            None
        );
        if current_ms >= plan.expires || current_ms >= approval_expires(&approval) {
            return self.finish_failure(
                binding,
                &intent,
                &economic_run_id,
                &journal,
                invoice.as_ref(),
                Some(&plan),
                Some(&simulation),
                Some(&approval),
                None,
                None,
                &mut events,
                &mut budget,
                g212("expired"),
                started,
            );
        }
        let expected_transaction_id = terminal_try!(
            transaction_id(rail, &signed).ok_or_else(g213),
            invoice.as_ref(),
            Some(&plan),
            Some(&simulation),
            Some(&approval),
            None,
            None
        );
        let provisional_source = format!("{{\"schema\":\"{BROADCAST_SCHEMA}\",\"rail\":{},\"network\":{},\"signed_transaction_digest\":{},\"transaction_id\":{},\"disposition\":\"unknown\",\"observed_at_ms\":0}}\n",quote_json(rail.text()),quote_json(network),quote_json(&signed_digest),quote_json(&expected_transaction_id));
        let provisional = terminal_try!(
            parse_provisional_broadcast(
                &provisional_source,
                rail,
                network,
                &signed_digest,
                &expected_transaction_id
            ),
            invoice.as_ref(),
            Some(&plan),
            Some(&simulation),
            Some(&approval),
            None,
            None
        );
        terminal_try!(
            self.pre_call(
                started,
                self.policy.limits.max_broadcast_receipt_bytes as usize,
            ),
            invoice.as_ref(),
            Some(&plan),
            Some(&simulation),
            Some(&approval),
            None,
            None
        );
        let mut broadcast_marker = terminal_try!(
            clone_journal_bounded(&journal, self.policy.limits.max_builder_bytes),
            invoice.as_ref(),
            Some(&plan),
            Some(&simulation),
            Some(&approval),
            Some(&provisional),
            None
        );
        broadcast_marker.state = JournalState::BroadcastUnknown;
        broadcast_marker.broadcast = Some(provisional.doc.clone());
        broadcast_marker.updated_at = 0;
        terminal_try!(
            cas_journal(
                &mut self.host,
                &mut broadcast_marker,
                &mut events,
                &mut budget,
                self.policy.limits.max_journal_bytes,
                EconomicRollingReservationUpdate::Retain,
            ),
            invoice.as_ref(),
            Some(&plan),
            Some(&simulation),
            Some(&approval),
            Some(&provisional),
            None
        );
        journal = broadcast_marker;
        terminal_try!(
            self.pre_call(
                started,
                self.policy.limits.max_broadcast_receipt_bytes as usize,
            ),
            invoice.as_ref(),
            Some(&plan),
            Some(&simulation),
            Some(&approval),
            Some(&provisional),
            None
        );
        let mut broadcast_sink = EconomicDocumentSink::new(
            self.policy.limits.max_broadcast_receipt_bytes as usize,
            self.cancellation.clone(),
            self.host.boundary_probe(),
            started,
            self.policy.limits.max_elapsed_ms,
            self.policy.limits.max_builder_bytes,
            self.terminal_floor()?,
        );
        let disposition = match rail {
            EconomicRail::Evm => self.host.evm_broadcast(&signed, &mut broadcast_sink),
            EconomicRail::Solana => self.host.solana_broadcast(&signed, &mut broadcast_sink),
            EconomicRail::Bitcoin => self.host.bitcoin_broadcast(&signed, &mut broadcast_sink),
        };
        let mut broadcast_usage = Usage::default();
        broadcast_usage.broadcasts = 1;
        if disposition != EconomicAdapterDisposition::Succeeded {
            push_event(
                &mut events,
                event(
                    "broadcast_finished",
                    Some(rail),
                    Some(signed_digest.clone()),
                    Some(provisional.doc.digest.clone()),
                    "unknown",
                    broadcast_usage,
                )?,
            )?;
            return self.finish_failure(
                binding,
                &intent,
                &economic_run_id,
                &journal,
                invoice.as_ref(),
                Some(&plan),
                Some(&simulation),
                Some(&approval),
                Some(&provisional),
                None,
                &mut events,
                &mut budget,
                info("SPX-I226", "Economic Agent broadcast outcome is uncertain"),
                started,
            );
        }
        let broadcast_source = terminal_try!(
            broadcast_sink.finish("broadcast_receipt_bytes"),
            invoice.as_ref(),
            Some(&plan),
            Some(&simulation),
            Some(&approval),
            None,
            None
        );
        broadcast_usage.output_bytes = broadcast_source.len() as u64;
        let broadcast = terminal_try!(
            parse_broadcast_limited(
                &broadcast_source,
                rail,
                network,
                &signed_digest,
                Some(&expected_transaction_id),
                &self.policy.limits,
            ),
            invoice.as_ref(),
            Some(&plan),
            Some(&simulation),
            Some(&approval),
            None,
            None
        );
        budget.signed_bytes = journal.signed.as_ref().map_or(0, |value| value.1 as u64);
        budget.broadcast_bytes = broadcast.doc.source.len() as u64;
        budget.broadcast_bytes = broadcast_source.len() as u64;
        push_event(
            &mut events,
            event(
                "broadcast_finished",
                Some(rail),
                Some(signed_digest),
                Some(broadcast.doc.digest.clone()),
                broadcast.disposition,
                broadcast_usage,
            )?,
        )?;
        if broadcast.disposition == "unknown" {
            let mut outcome_journal = terminal_try!(
                clone_journal_bounded(&journal, self.policy.limits.max_builder_bytes),
                invoice.as_ref(),
                Some(&plan),
                Some(&simulation),
                Some(&approval),
                Some(&broadcast),
                None
            );
            outcome_journal.state = JournalState::BroadcastUnknown;
            outcome_journal.broadcast = Some(broadcast.doc.clone());
            outcome_journal.updated_at = broadcast.observed;
            terminal_try!(
                cas_journal(
                    &mut self.host,
                    &mut outcome_journal,
                    &mut events,
                    &mut budget,
                    self.policy.limits.max_journal_bytes,
                    EconomicRollingReservationUpdate::Retain,
                ),
                invoice.as_ref(),
                Some(&plan),
                Some(&simulation),
                Some(&approval),
                Some(&broadcast),
                None
            );
            journal = outcome_journal;
            return self.finish_failure(
                binding,
                &intent,
                &economic_run_id,
                &journal,
                invoice.as_ref(),
                Some(&plan),
                Some(&simulation),
                Some(&approval),
                Some(&broadcast),
                None,
                &mut events,
                &mut budget,
                info("SPX-I226", "Economic Agent broadcast outcome is uncertain"),
                started,
            );
        }
        if broadcast.disposition == "rejected" {
            let mut outcome_journal = terminal_try!(
                clone_journal_bounded(&journal, self.policy.limits.max_builder_bytes),
                invoice.as_ref(),
                Some(&plan),
                Some(&simulation),
                Some(&approval),
                Some(&broadcast),
                None
            );
            outcome_journal.state = JournalState::Rejected;
            outcome_journal.broadcast = Some(broadcast.doc.clone());
            outcome_journal.updated_at = broadcast.observed;
            terminal_try!(
                cas_journal(
                    &mut self.host,
                    &mut outcome_journal,
                    &mut events,
                    &mut budget,
                    self.policy.limits.max_journal_bytes,
                    EconomicRollingReservationUpdate::Retain,
                ),
                invoice.as_ref(),
                Some(&plan),
                Some(&simulation),
                Some(&approval),
                Some(&broadcast),
                None
            );
            journal = outcome_journal;
            return self.finish_failure(
                binding,
                &intent,
                &economic_run_id,
                &journal,
                invoice.as_ref(),
                Some(&plan),
                Some(&simulation),
                Some(&approval),
                Some(&broadcast),
                None,
                &mut events,
                &mut budget,
                info("SPX-I223", "Economic Agent chain adapter failed"),
                started,
            );
        }
        let mut outcome_journal = terminal_try!(
            clone_journal_bounded(&journal, self.policy.limits.max_builder_bytes),
            invoice.as_ref(),
            Some(&plan),
            Some(&simulation),
            Some(&approval),
            Some(&broadcast),
            None
        );
        outcome_journal.state = if broadcast.disposition == "pending" {
            JournalState::Pending
        } else {
            JournalState::Broadcasted
        };
        outcome_journal.broadcast = Some(broadcast.doc.clone());
        outcome_journal.updated_at = broadcast.observed;
        terminal_try!(
            cas_journal(
                &mut self.host,
                &mut outcome_journal,
                &mut events,
                &mut budget,
                self.policy.limits.max_journal_bytes,
                EconomicRollingReservationUpdate::Retain,
            ),
            invoice.as_ref(),
            Some(&plan),
            Some(&simulation),
            Some(&approval),
            Some(&broadcast),
            None
        );
        journal = outcome_journal;
        self.reconcile_after_broadcast(
            binding,
            &intent,
            &economic_run_id,
            journal,
            invoice.as_ref(),
            &plan,
            &simulation,
            &approval,
            &broadcast,
            events,
            budget,
            started,
        )
    }

    fn pre_call(&self, started: u64, maximum_output: usize) -> Result<(), Diagnostic> {
        if self.cancellation.is_cancelled() {
            return Err(info("SPX-I228", "Economic Agent run was cancelled"));
        }
        if self.elapsed_ms(started)? > self.policy.limits.max_elapsed_ms {
            return Err(info("SPX-I229", "Economic Agent deadline was exceeded"));
        }
        let multiplier = if maximum_output == self.policy.limits.max_journal_bytes as usize {
            2
        } else if maximum_output == self.policy.limits.max_signed_transaction_bytes as usize {
            3
        } else {
            usize::try_from(self.policy.limits.max_json_depth)
                .map_err(|_| g217())?
                .checked_add(3)
                .ok_or_else(g217)?
        };
        let output_lane = maximum_output
            .checked_mul(multiplier)
            .ok_or_else(|| g216("builder_bytes", self.policy.limits.max_builder_bytes))?;
        let required = output_lane
            .checked_add(self.terminal_floor()?)
            .ok_or_else(|| g216("builder_bytes", self.policy.limits.max_builder_bytes))?;
        if active_remaining().is_some_and(|remaining| remaining < required) {
            return Err(g216("builder_bytes", self.policy.limits.max_builder_bytes));
        }
        Ok(())
    }

    fn elapsed_ms(&self, started: u64) -> Result<u64, Diagnostic> {
        self.host
            .boundary_probe()
            .elapsed_ms()
            .checked_sub(started)
            .ok_or_else(|| info("SPX-I229", "Economic Agent deadline was exceeded"))
    }

    #[allow(clippy::too_many_arguments)]
    fn finish_failure(
        &mut self,
        binding: &crate::agent_runtime::EconomicAgentBinding<'_>,
        intent: &Intent,
        economic_run_id: &str,
        journal: &Journal,
        invoice: Option<&Invoice>,
        plan: Option<&Plan>,
        simulation: Option<&Simulation>,
        approval: Option<&Approval>,
        broadcast: Option<&BroadcastReceipt>,
        reconciliation: Option<&Reconciliation>,
        events: &mut Vec<Event>,
        budget: &mut Budget,
        diagnostic: Diagnostic,
        started: u64,
    ) -> Result<EconomicRun, Diagnostic> {
        let mut terminal_diagnostic = diagnostic;
        let mut terminal_journal =
            clone_journal_bounded(journal, self.policy.limits.max_builder_bytes)?;
        let usage = cumulative_usage(events)?;
        let uncertain_journal_cas = events
            .last()
            .is_some_and(|event| event.kind == "journal_committed" && event.authority_uncertain);
        let already_sealed_effect_boundary = (journal.version == 4
            && journal.state == JournalState::Approved)
            || (journal.version == 6 && journal.state == JournalState::BroadcastUnknown)
            || journal.broadcast.is_some();
        let no_signature_or_broadcast_attempt = journal.signed.is_none()
            && journal.broadcast.is_none()
            && usage.signatures == 0
            && usage.broadcasts == 0
            && journal.version <= 3
            && (journal.version == 1 || usage.journal_writes > 0);
        if !uncertain_journal_cas {
            terminal_journal.state = if already_sealed_effect_boundary
                || journal.signed.is_some()
                || journal.broadcast.is_some()
            {
                journal.state
            } else {
                match terminal_diagnostic.code {
                    "SPX-I228" => JournalState::Cancelled,
                    "SPX-G212" | "SPX-G214" => JournalState::Rejected,
                    _ => JournalState::Failed,
                }
            };
        }
        if no_signature_or_broadcast_attempt && !uncertain_journal_cas {
            terminal_journal.updated_at = admitted_now_from(
                intent,
                journal.updated_at,
                self.elapsed_ms(started)
                    .unwrap_or(self.policy.limits.max_elapsed_ms),
            )
            .unwrap_or(intent.expires_at);
        }
        let rolling = if no_signature_or_broadcast_attempt {
            EconomicRollingReservationUpdate::Release
        } else {
            EconomicRollingReservationUpdate::Retain
        };
        if journal.version > 0
            && !uncertain_journal_cas
            && no_signature_or_broadcast_attempt
            && !already_sealed_effect_boundary
            && cas_journal(
                &mut self.host,
                &mut terminal_journal,
                events,
                budget,
                self.policy.limits.max_journal_bytes,
                rolling,
            )
            .is_err()
        {
            terminal_journal =
                clone_journal_bounded(journal, self.policy.limits.max_builder_bytes)?;
            terminal_diagnostic = info("SPX-I222", "Economic Agent journal adapter failed");
        }
        let terminal = diagnostic_terminal(&terminal_diagnostic);
        finish_run(
            economic_run_id,
            binding,
            &self.policy,
            intent,
            invoice,
            plan,
            simulation,
            approval,
            &terminal_journal,
            broadcast,
            reconciliation,
            events,
            terminal,
            budget,
            self.elapsed_ms(started)?,
        )
    }

    #[allow(clippy::too_many_arguments)]
    fn reconcile_after_broadcast(
        &mut self,
        binding: &crate::agent_runtime::EconomicAgentBinding<'_>,
        intent: &Intent,
        economic_run_id: &str,
        mut journal: Journal,
        invoice: Option<&Invoice>,
        plan: &Plan,
        simulation: &Simulation,
        approval: &Approval,
        broadcast: &BroadcastReceipt,
        mut events: Vec<Event>,
        mut budget: Budget,
        started: u64,
    ) -> Result<EconomicRun, Diagnostic> {
        macro_rules! terminal_try {
            ($expression:expr, $reconciliation:expr) => {
                match $expression {
                    Ok(value) => value,
                    Err(diagnostic) => {
                        return self.finish_failure(
                            binding,
                            intent,
                            economic_run_id,
                            &journal,
                            invoice,
                            Some(plan),
                            Some(simulation),
                            Some(approval),
                            Some(broadcast),
                            $reconciliation,
                            &mut events,
                            &mut budget,
                            diagnostic,
                            started,
                        )
                    }
                }
            };
        }
        terminal_try!(
            self.pre_call(
                started,
                self.policy.limits.max_reconciliation_bytes as usize,
            ),
            None
        );
        let (attempts, odd) = terminal_try!(reconciliation_topology(&journal), None);
        if odd || attempts >= self.policy.limits.max_reconciliations {
            return self.finish_failure(
                binding,
                intent,
                economic_run_id,
                &journal,
                invoice,
                Some(plan),
                Some(simulation),
                Some(approval),
                Some(broadcast),
                None,
                &mut events,
                &mut budget,
                g216("reconciliations", self.policy.limits.max_reconciliations),
                started,
            );
        }
        terminal_try!(
            cas_journal(
                &mut self.host,
                &mut journal,
                &mut events,
                &mut budget,
                self.policy.limits.max_journal_bytes,
                EconomicRollingReservationUpdate::Retain,
            ),
            None
        );
        terminal_try!(
            self.pre_call(
                started,
                self.policy.limits.max_reconciliation_bytes as usize,
            ),
            None
        );
        let rail = intent.settlement_rail();
        let (network, _) = intent.network_asset();
        let mut sink = EconomicDocumentSink::new(
            self.policy.limits.max_reconciliation_bytes as usize,
            self.cancellation.clone(),
            self.host.boundary_probe(),
            started,
            self.policy.limits.max_elapsed_ms,
            self.policy.limits.max_builder_bytes,
            self.terminal_floor()?,
        );
        let disposition = match rail {
            EconomicRail::Evm => self
                .host
                .evm_reconcile(&broadcast.transaction_id, &mut sink),
            EconomicRail::Solana => self
                .host
                .solana_reconcile(&broadcast.transaction_id, &mut sink),
            EconomicRail::Bitcoin => self
                .host
                .bitcoin_reconcile(&broadcast.transaction_id, &mut sink),
        };
        let mut usage = Usage::default();
        usage.reconciliations = 1;
        if disposition != EconomicAdapterDisposition::Succeeded {
            push_event(
                &mut events,
                event(
                    "reconciliation_finished",
                    Some(rail),
                    Some(broadcast.doc.digest.clone()),
                    None,
                    "failed",
                    usage,
                )?,
            )?;
            return self.finish_failure(
                binding,
                intent,
                economic_run_id,
                &journal,
                invoice,
                Some(plan),
                Some(simulation),
                Some(approval),
                Some(broadcast),
                None,
                &mut events,
                &mut budget,
                info("SPX-I227", "Economic Agent reconciliation adapter failed"),
                started,
            );
        }
        let source = terminal_try!(sink.finish("reconciliation_bytes"), None);
        usage.output_bytes = source.len() as u64;
        let reconciliation = terminal_try!(
            parse_reconciliation_limited(
                &source,
                rail,
                network,
                &broadcast.transaction_id,
                &self.policy.limits,
            ),
            None
        );
        terminal_try!(validate_confirmation(intent, &reconciliation), None);
        budget.reconciliation_bytes = source.len() as u64;
        budget.reconciliations = attempts.checked_add(1).ok_or_else(g217)?;
        push_event(
            &mut events,
            event(
                "reconciliation_finished",
                Some(rail),
                Some(broadcast.doc.digest.clone()),
                Some(reconciliation.doc.digest.clone()),
                reconciliation.status,
                usage,
            )?,
        )?;
        journal.state = match reconciliation.status {
            "confirmed" => JournalState::Confirmed,
            "reorged" => JournalState::Reorged,
            "dropped" => JournalState::Dropped,
            _ => JournalState::Pending,
        };
        journal.reconciliation = Some(reconciliation.doc.clone());
        journal.updated_at = reconciliation.observed;
        terminal_try!(
            cas_journal(
                &mut self.host,
                &mut journal,
                &mut events,
                &mut budget,
                self.policy.limits.max_journal_bytes,
                EconomicRollingReservationUpdate::Retain,
            ),
            Some(&reconciliation)
        );
        let status = match reconciliation.status {
            "confirmed" => EconomicRunStatus::Confirmed,
            "reorged" => EconomicRunStatus::Reorged,
            "dropped" => EconomicRunStatus::Dropped,
            _ => EconomicRunStatus::Pending,
        };
        let terminal = Terminal {
            status,
            transaction_id: Some(broadcast.transaction_id.clone()),
            confirmation: Some(reconciliation.status.to_owned()),
            code: None,
            message: None,
        };
        finish_run(
            economic_run_id,
            binding,
            &self.policy,
            intent,
            invoice,
            Some(plan),
            Some(simulation),
            Some(approval),
            &journal,
            Some(broadcast),
            Some(&reconciliation),
            &mut events,
            terminal,
            &mut budget,
            self.elapsed_ms(started)?,
        )
    }

    fn resume_loaded(
        &mut self,
        binding: &crate::agent_runtime::EconomicAgentBinding<'_>,
        intent: Intent,
        economic_run_id: String,
        mut journal: Journal,
        mut events: Vec<Event>,
        mut budget: Budget,
        started: u64,
    ) -> Result<EconomicRun, Diagnostic> {
        macro_rules! terminal_try {
            ($expression:expr, $broadcast:expr, $reconciliation:expr) => {
                match $expression {
                    Ok(value) => value,
                    Err(diagnostic) => {
                        return self.finish_failure(
                            binding,
                            &intent,
                            &economic_run_id,
                            &journal,
                            None,
                            None,
                            None,
                            None,
                            $broadcast,
                            $reconciliation,
                            &mut events,
                            &mut budget,
                            diagnostic,
                            started,
                        )
                    }
                }
            };
        }
        let Some(broadcast_doc) = journal.broadcast.as_ref() else {
            return self.finish_failure(
                binding,
                &intent,
                &economic_run_id,
                &journal,
                None,
                None,
                None,
                None,
                None,
                None,
                &mut events,
                &mut budget,
                g215(),
                started,
            );
        };
        let (_, value) = terminal_try!(
            canonical(
                &broadcast_doc.source,
                "broadcast receipt",
                BROADCAST_SCHEMA,
                self.policy.limits.max_broadcast_receipt_bytes as usize,
            ),
            None,
            None
        );
        let row = terminal_try!(
            object(&value, "broadcast receipt", BROADCAST_SCHEMA),
            None,
            None
        );
        let signed_digest = terminal_try!(
            text(
                row,
                "signed_transaction_digest",
                "broadcast receipt",
                BROADCAST_SCHEMA,
            ),
            None,
            None
        );
        let (network, _) = intent.network_asset();
        let broadcast = terminal_try!(
            if broadcast_is_provisional(broadcast_doc) {
                let transaction_id = value["transaction_id"].as_str().ok_or_else(g215);
                transaction_id.and_then(|transaction_id| {
                    parse_provisional_broadcast(
                        &broadcast_doc.source,
                        intent.settlement_rail(),
                        network,
                        signed_digest,
                        transaction_id,
                    )
                })
            } else {
                parse_broadcast(
                    &broadcast_doc.source,
                    intent.settlement_rail(),
                    network,
                    signed_digest,
                    None,
                )
            },
            None,
            None
        );
        budget.signed_bytes = journal.signed.as_ref().map_or(0, |value| value.1 as u64);
        budget.broadcast_bytes = broadcast.doc.source.len() as u64;
        if matches!(
            journal.state,
            JournalState::Confirmed | JournalState::Reorged | JournalState::Dropped
        ) {
            let status = match journal.state {
                JournalState::Confirmed => EconomicRunStatus::Confirmed,
                JournalState::Reorged => EconomicRunStatus::Reorged,
                _ => EconomicRunStatus::Dropped,
            };
            let terminal = Terminal {
                status,
                transaction_id: Some(broadcast.transaction_id.clone()),
                confirmation: Some(journal.state.text().to_owned()),
                code: None,
                message: None,
            };
            return finish_run(
                &economic_run_id,
                binding,
                &self.policy,
                &intent,
                None,
                None,
                None,
                None,
                &journal,
                Some(&broadcast),
                None,
                &mut events,
                terminal,
                &mut budget,
                started,
            );
        }
        let (mut attempts, odd) =
            terminal_try!(reconciliation_topology(&journal), Some(&broadcast), None);
        if odd {
            let mut closed = terminal_try!(
                clone_journal_bounded(&journal, self.policy.limits.max_builder_bytes),
                Some(&broadcast),
                None
            );
            terminal_try!(
                cas_journal(
                    &mut self.host,
                    &mut closed,
                    &mut events,
                    &mut budget,
                    self.policy.limits.max_journal_bytes,
                    EconomicRollingReservationUpdate::Retain,
                ),
                Some(&broadcast),
                None
            );
            journal = closed;
        }
        if attempts >= self.policy.limits.max_reconciliations {
            return self.finish_failure(
                binding,
                &intent,
                &economic_run_id,
                &journal,
                None,
                None,
                None,
                None,
                Some(&broadcast),
                None,
                &mut events,
                &mut budget,
                g216("reconciliations", self.policy.limits.max_reconciliations),
                started,
            );
        }
        terminal_try!(
            self.pre_call(
                started,
                self.policy.limits.max_reconciliation_bytes as usize,
            ),
            Some(&broadcast),
            None
        );
        terminal_try!(
            cas_journal(
                &mut self.host,
                &mut journal,
                &mut events,
                &mut budget,
                self.policy.limits.max_journal_bytes,
                EconomicRollingReservationUpdate::Retain,
            ),
            Some(&broadcast),
            None
        );
        attempts = attempts.checked_add(1).ok_or_else(g217)?;
        terminal_try!(
            self.pre_call(
                started,
                self.policy.limits.max_reconciliation_bytes as usize,
            ),
            Some(&broadcast),
            None
        );
        let mut sink = EconomicDocumentSink::new(
            self.policy.limits.max_reconciliation_bytes as usize,
            self.cancellation.clone(),
            self.host.boundary_probe(),
            started,
            self.policy.limits.max_elapsed_ms,
            self.policy.limits.max_builder_bytes,
            self.terminal_floor()?,
        );
        let disposition = match intent.settlement_rail() {
            EconomicRail::Evm => self
                .host
                .evm_reconcile(&broadcast.transaction_id, &mut sink),
            EconomicRail::Solana => self
                .host
                .solana_reconcile(&broadcast.transaction_id, &mut sink),
            EconomicRail::Bitcoin => self
                .host
                .bitcoin_reconcile(&broadcast.transaction_id, &mut sink),
        };
        if disposition != EconomicAdapterDisposition::Succeeded {
            return self.finish_failure(
                binding,
                &intent,
                &economic_run_id,
                &journal,
                None,
                None,
                None,
                None,
                Some(&broadcast),
                None,
                &mut events,
                &mut budget,
                info("SPX-I227", "Economic Agent reconciliation adapter failed"),
                started,
            );
        }
        let source = terminal_try!(sink.finish("reconciliation_bytes"), Some(&broadcast), None);
        let reconciliation = terminal_try!(
            parse_reconciliation_limited(
                &source,
                intent.settlement_rail(),
                network,
                &broadcast.transaction_id,
                &self.policy.limits,
            ),
            Some(&broadcast),
            None
        );
        terminal_try!(
            validate_confirmation(&intent, &reconciliation),
            Some(&broadcast),
            None
        );
        budget.reconciliation_bytes = source.len() as u64;
        budget.reconciliations = attempts;
        push_event(
            &mut events,
            event(
                "reconciliation_finished",
                Some(intent.settlement_rail()),
                Some(broadcast.doc.digest.clone()),
                Some(reconciliation.doc.digest.clone()),
                reconciliation.status,
                Usage {
                    reconciliations: 1,
                    output_bytes: source.len() as u64,
                    ..Usage::default()
                },
            )?,
        )?;
        let mut next = terminal_try!(
            clone_journal_bounded(&journal, self.policy.limits.max_builder_bytes),
            Some(&broadcast),
            Some(&reconciliation)
        );
        next.state = match reconciliation.status {
            "confirmed" => JournalState::Confirmed,
            "reorged" => JournalState::Reorged,
            "dropped" => JournalState::Dropped,
            _ => JournalState::Pending,
        };
        next.reconciliation = Some(reconciliation.doc.clone());
        next.updated_at = reconciliation.observed;
        terminal_try!(
            cas_journal(
                &mut self.host,
                &mut next,
                &mut events,
                &mut budget,
                self.policy.limits.max_journal_bytes,
                EconomicRollingReservationUpdate::Retain,
            ),
            Some(&broadcast),
            Some(&reconciliation)
        );
        let terminal = Terminal {
            status: match reconciliation.status {
                "confirmed" => EconomicRunStatus::Confirmed,
                "reorged" => EconomicRunStatus::Reorged,
                "dropped" => EconomicRunStatus::Dropped,
                _ => EconomicRunStatus::Pending,
            },
            transaction_id: Some(broadcast.transaction_id.clone()),
            confirmation: Some(reconciliation.status.to_owned()),
            code: None,
            message: None,
        };
        finish_run(
            &economic_run_id,
            binding,
            &self.policy,
            &intent,
            None,
            None,
            None,
            None,
            &next,
            Some(&broadcast),
            Some(&reconciliation),
            &mut events,
            terminal,
            &mut budget,
            started,
        )
    }

    /// Reconciles an idempotency binding using the same sealed Agent source.
    pub fn reconcile(
        &mut self,
        idempotency_key: &str,
        source: &AgentRun,
    ) -> Result<EconomicRun, Vec<Diagnostic>> {
        if !identifier(idempotency_key) {
            return Err(vec![g215()]);
        }
        let binding = source.economic_binding();
        if binding.status != AgentRunStatus::Completed {
            return Err(vec![g212("agent run not completed")]);
        }
        let Some(message) = binding.final_message else {
            return Err(vec![g212("agent run not completed")]);
        };
        let started = self.host.boundary_probe().elapsed_ms();
        let limit = self.policy.limits.max_builder_bytes as usize;
        let (result, overflowed, _) = with_limit_usage(limit, || {
            if !reserve_active(self.retained_policy_bytes) {
                return Err(g216("builder_bytes", self.policy.limits.max_builder_bytes));
            }
            if !reserve_active(message.len().saturating_mul(MAX_JSON_DEPTH + 2)) {
                return Err(g216("builder_bytes", self.policy.limits.max_builder_bytes));
            }
            let intent = parse_intent(message).and_then(|intent| {
                admit_intent(&self.policy, &intent)?;
                Ok(intent)
            })?;
            if intent.idempotency_key != idempotency_key {
                return Err(g215());
            }
            self.terminal_floor()?;
            self.reconcile_bounded(&binding, intent, started)
        });
        if overflowed {
            return Err(vec![g216(
                "builder_bytes",
                self.policy.limits.max_builder_bytes,
            )]);
        }
        result.map_err(|diagnostic| vec![diagnostic])
    }

    fn reconcile_bounded(
        &mut self,
        binding: &crate::agent_runtime::EconomicAgentBinding<'_>,
        intent: Intent,
        started: u64,
    ) -> Result<EconomicRun, Diagnostic> {
        let economic_run_id = run_id(
            binding.evidence_digest,
            &self.policy.digest,
            &intent.digest,
            &intent.idempotency_key,
        );
        let mut journal = Journal {
            idempotency_key: intent.idempotency_key.clone(),
            version: 0,
            policy: Doc {
                source: self.policy.source.clone(),
                digest: self.policy.digest.clone(),
            },
            intent: Doc {
                source: intent.source.clone(),
                digest: intent.digest.clone(),
            },
            run_id: economic_run_id.clone(),
            state: JournalState::Failed,
            reserved_amount: intent.amount(),
            reserved_fee: intent.max_fee(),
            plan: None,
            simulation: None,
            approval: None,
            unsigned: None,
            signed: None,
            broadcast: None,
            reconciliation: None,
            updated_at: intent.created_at,
        };
        let mut events = Vec::new();
        push_event(
            &mut events,
            event(
                "run_started",
                None,
                Some(binding.evidence_digest.to_owned()),
                Some(self.policy.digest.clone()),
                "started",
                Usage::default(),
            )?,
        )?;
        let mut budget = Budget {
            policy_bytes: self.policy.source.len() as u64,
            intent_bytes: intent.source.len() as u64,
            recipients: self
                .policy
                .networks
                .iter()
                .map(|row| row.recipients.len() as u64)
                .sum(),
            network_policies: self.policy.networks.len() as u64,
            x402_origins: self.policy.origins.len() as u64,
            concurrency: 1,
            ..Budget::default()
        };
        self.pre_call(started, self.policy.limits.max_journal_bytes as usize)?;
        let mut sink = EconomicDocumentSink::new(
            self.policy.limits.max_journal_bytes as usize,
            self.cancellation.clone(),
            self.host.boundary_probe(),
            started,
            self.policy.limits.max_elapsed_ms,
            self.policy.limits.max_builder_bytes,
            self.terminal_floor()?,
        );
        let load = self.host.load(&intent.idempotency_key, &mut sink);
        if load != EconomicJournalLoad::Present {
            push_event(
                &mut events,
                event(
                    "journal_loaded",
                    None,
                    None,
                    None,
                    "failed",
                    Usage {
                        journal_reads: 1,
                        ..Usage::default()
                    },
                )?,
            )?;
            return self.finish_failure(
                binding,
                &intent,
                &economic_run_id,
                &journal,
                None,
                None,
                None,
                None,
                None,
                None,
                &mut events,
                &mut budget,
                info("SPX-I222", "Economic Agent journal adapter failed"),
                started,
            );
        }
        let source = match sink.finish("journal_bytes") {
            Ok(source) => source,
            Err(diagnostic) => {
                push_event(
                    &mut events,
                    event(
                        "journal_loaded",
                        None,
                        None,
                        None,
                        "failed",
                        Usage {
                            journal_reads: 1,
                            ..Usage::default()
                        },
                    )?,
                )?;
                return self.finish_failure(
                    binding,
                    &intent,
                    &economic_run_id,
                    &journal,
                    None,
                    None,
                    None,
                    None,
                    None,
                    None,
                    &mut events,
                    &mut budget,
                    diagnostic,
                    started,
                );
            }
        };
        push_event(
            &mut events,
            event(
                "journal_loaded",
                None,
                None,
                Some(digest(JOURNAL_DOMAIN, source.as_bytes())),
                "present",
                Usage {
                    journal_reads: 1,
                    output_bytes: source.len() as u64,
                    ..Usage::default()
                },
            )?,
        )?;
        journal = match parse_journal_classified(&source, &self.policy, &intent, &economic_run_id) {
            Ok(journal) => journal,
            Err(JournalParseFailure::BindingMismatch) => return Err(g215()),
            Err(JournalParseFailure::Diagnostic(diagnostic)) => {
                return self.finish_failure(
                    binding,
                    &intent,
                    &economic_run_id,
                    &journal,
                    None,
                    None,
                    None,
                    None,
                    None,
                    None,
                    &mut events,
                    &mut budget,
                    diagnostic,
                    started,
                );
            }
        };
        budget.journal_bytes = source.len() as u64;
        self.resume_loaded(
            binding,
            intent,
            economic_run_id,
            journal,
            events,
            budget,
            started,
        )
    }
}

fn terminal_floor(limits: &Limits) -> Result<usize, Diagnostic> {
    usize::try_from(limits.max_trace_bytes)
        .ok()
        .and_then(|trace| {
            usize::try_from(limits.max_evidence_bytes)
                .ok()
                .and_then(|evidence| evidence.checked_mul(2))
                .and_then(|evidence| trace.checked_add(evidence))
        })
        .and_then(|value| value.checked_add(4096))
        .ok_or_else(|| g216("builder_bytes", limits.max_builder_bytes))
}

fn digest(domain: &[u8], bytes: &[u8]) -> String {
    let mut hash = Sha256::new();
    hash.update(domain);
    hash.update(bytes);
    format!("sha256:{:x}", crate::digest_hex::LowerHex(hash.finalize()))
}

fn admitted_now_from(intent: &Intent, observed: u64, elapsed: u64) -> Result<u64, Diagnostic> {
    intent
        .created_at
        .max(observed)
        .checked_add(elapsed)
        .ok_or_else(|| g212("expired"))
}

fn confirmation_target(intent: &Intent) -> u64 {
    match &intent.payment {
        Payment::Bitcoin { confirmations, .. } => *confirmations,
        Payment::X402 {
            rail: EconomicRail::Bitcoin,
            ..
        } => 1,
        _ => 0,
    }
}

fn validate_confirmation(
    intent: &Intent,
    reconciliation: &Reconciliation,
) -> Result<(), Diagnostic> {
    let target = confirmation_target(intent);
    if reconciliation.status == "confirmed" && reconciliation.confirmations.unwrap_or(0) < target {
        return Err(g213());
    }
    Ok(())
}

fn g210(document: &str, schema: &str) -> Diagnostic {
    Diagnostic::io(
        "SPX-G210",
        format!("Economic Agent {document} is not canonical {schema} JSON"),
    )
}
fn g211(field: &str) -> Diagnostic {
    Diagnostic::io(
        "SPX-G211",
        format!("Economic Agent policy invariant failed: {field}"),
    )
}
fn g212(reason: &'static str) -> Diagnostic {
    Diagnostic::io(
        "SPX-G212",
        format!("Economic Agent payment intent was rejected: {reason}"),
    )
}
fn g213() -> Diagnostic {
    Diagnostic::io(
        "SPX-G213",
        "Economic Agent prepared transaction or simulation disagrees with the admitted intent",
    )
}
fn g214() -> Diagnostic {
    Diagnostic::io(
        "SPX-G214",
        "Economic Agent approval is absent, expired, rejected, or digest-mismatched",
    )
}
fn g215() -> Diagnostic {
    Diagnostic::io(
        "SPX-G215",
        "Economic Agent journal state or idempotency replay disagrees with the admitted operation",
    )
}
fn g216(field: &str, maximum: u64) -> Diagnostic {
    Diagnostic::io("SPX-G216", format!("{field} exceeds {maximum}"))
}
fn g217() -> Diagnostic {
    Diagnostic::io(
        "SPX-G217",
        "Economic Agent Trace or Evidence disagrees with the replayed state machine",
    )
}
fn info(code: &'static str, message: &'static str) -> Diagnostic {
    Diagnostic::io(code, message)
}

fn canonical<'a>(
    source: &'a str,
    document: &str,
    schema: &str,
    maximum: usize,
) -> Result<(&'a str, Value), Diagnostic> {
    if source.len() > maximum {
        return Err(g216(document_bytes_field(document), maximum as u64));
    }
    let Some(body) = source.strip_suffix('\n') else {
        return Err(g210(document, schema));
    };
    if body.is_empty() || body.contains('\n') || body.contains('\r') || body.starts_with('\u{feff}')
    {
        return Err(g210(document, schema));
    }
    let value: Value = serde_json::from_str(body).map_err(|_| g210(document, schema))?;
    if depth(&value) > MAX_JSON_DEPTH {
        return Err(g216("json_depth", MAX_JSON_DEPTH as u64));
    }
    if value
        .as_object()
        .and_then(|row| row.get("schema"))
        .and_then(Value::as_str)
        != Some(schema)
    {
        return Err(g210(document, schema));
    }
    Ok((body, value))
}
fn canonical_policy_limited<'a>(
    source: &'a str,
    document: &str,
    schema: &str,
    maximum: u64,
    max_depth: u64,
) -> Result<(&'a str, Value), Diagnostic> {
    let (body, value) = canonical(source, document, schema, maximum as usize)?;
    if depth(&value) as u64 > max_depth {
        return Err(g216("json_depth", max_depth));
    }
    Ok((body, value))
}
fn configured_depth(source: &str, limits: &Limits) -> Result<(), Diagnostic> {
    if structural_json_depth(source).ok_or_else(g217)? > limits.max_json_depth {
        return Err(g216("json_depth", limits.max_json_depth));
    }
    Ok(())
}

fn structural_json_depth(source: &str) -> Option<u64> {
    let mut depth = 0_u64;
    let mut maximum = 0_u64;
    let mut quoted = false;
    let mut escaped = false;
    for byte in source.bytes() {
        if quoted {
            if escaped {
                escaped = false;
            } else if byte == b'\\' {
                escaped = true;
            } else if byte == b'"' {
                quoted = false;
            }
            continue;
        }
        match byte {
            b'"' => quoted = true,
            b'{' | b'[' => {
                depth = depth.checked_add(1)?;
                maximum = maximum.max(depth);
            }
            b'}' | b']' => depth = depth.checked_sub(1)?,
            _ => {}
        }
    }
    (!quoted && !escaped && depth == 0).then(|| {
        if source
            .bytes()
            .any(|byte| !byte.is_ascii_whitespace() && !matches!(byte, b'{' | b'}' | b'[' | b']'))
        {
            maximum.saturating_add(1)
        } else {
            maximum
        }
    })
}

fn configured_document_limits(
    source: &str,
    document: &str,
    maximum: u64,
    limits: &Limits,
) -> Result<(), Diagnostic> {
    if source.len() > maximum as usize {
        return Err(g216(document_bytes_field(document), maximum));
    }
    configured_depth(source, limits)
}

fn document_bytes_field(document: &str) -> &'static str {
    match document {
        "policy" => "policy_bytes",
        "payment intent" => "intent_bytes",
        "x402 invoice" => "invoice_bytes",
        "chain snapshot" => "snapshot_bytes",
        "payment plan" => "plan_bytes",
        "simulation" => "simulation_bytes",
        "approval request" => "approval_request_bytes",
        "approval" => "approval_bytes",
        "journal" => "journal_bytes",
        "broadcast receipt" => "broadcast_receipt_bytes",
        "reconciliation" => "reconciliation_bytes",
        "trace" => "trace_bytes",
        "evidence" => "evidence_bytes",
        _ => "builder_bytes",
    }
}

fn depth(value: &Value) -> usize {
    match value {
        Value::Array(v) => 1 + v.iter().map(depth).max().unwrap_or(0),
        Value::Object(v) => 1 + v.values().map(depth).max().unwrap_or(0),
        _ => 1,
    }
}
fn object<'a>(
    value: &'a Value,
    doc: &str,
    schema: &str,
) -> Result<&'a Map<String, Value>, Diagnostic> {
    value.as_object().ok_or_else(|| g210(doc, schema))
}
fn keys(row: &Map<String, Value>, expected: &[&str]) -> bool {
    row.len() == expected.len() && expected.iter().all(|key| row.contains_key(*key))
}
fn text<'a>(
    row: &'a Map<String, Value>,
    key: &str,
    doc: &str,
    schema: &str,
) -> Result<&'a str, Diagnostic> {
    row.get(key)
        .and_then(Value::as_str)
        .ok_or_else(|| g210(doc, schema))
}
fn number(row: &Map<String, Value>, key: &str, doc: &str, schema: &str) -> Result<u64, Diagnostic> {
    row.get(key)
        .and_then(Value::as_u64)
        .ok_or_else(|| g210(doc, schema))
}
fn policy_limit(
    row: &Map<String, Value>,
    key: &str,
    maximum: u64,
    nonzero: bool,
) -> Result<u64, Diagnostic> {
    let value = number(row, key, "policy", POLICY_SCHEMA)?;
    if value > maximum || (nonzero && value == 0) {
        return Err(g211(&format!("limits.{key}")));
    }
    Ok(value)
}
fn identifier(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= MAX_IDENTIFIER_BYTES
        && value.bytes().all(|b| {
            b.is_ascii_lowercase() || b.is_ascii_digit() || matches!(b, b'.' | b'_' | b':' | b'-')
        })
}
fn sorted_unique(values: &[String]) -> bool {
    values.windows(2).all(|w| w[0] < w[1])
}
fn string_array(value: &Value) -> Option<Vec<String>> {
    value
        .as_array()?
        .iter()
        .map(|v| v.as_str().map(str::to_owned))
        .collect()
}
fn string_list(values: &[String]) -> String {
    format!(
        "[{}]",
        values
            .iter()
            .map(|v| quote_json(v))
            .collect::<Vec<_>>()
            .join(",")
    )
}
fn nonclaims_json() -> String {
    format!(
        "[{}]",
        NONCLAIMS
            .iter()
            .map(|v| quote_json(v))
            .collect::<Vec<_>>()
            .join(",")
    )
}

fn rail(value: &str) -> Option<EconomicRail> {
    match value {
        "evm" => Some(EconomicRail::Evm),
        "solana" => Some(EconomicRail::Solana),
        "bitcoin" => Some(EconomicRail::Bitcoin),
        _ => None,
    }
}

fn limits_json(limits: &Limits) -> String {
    let mut output = String::new();
    write_limits(&mut output, limits).expect("String writes cannot fail");
    output
}
fn write_limits<W: fmt::Write>(output: &mut W, limits: &Limits) -> fmt::Result {
    write!(output,"{{\"max_policy_bytes\":{},\"max_intent_bytes\":{},\"max_invoice_bytes\":{},\"max_snapshot_bytes\":{},\"max_plan_bytes\":{},\"max_simulation_bytes\":{},\"max_approval_request_bytes\":{},\"max_approval_bytes\":{},\"max_journal_bytes\":{},\"max_unsigned_transaction_bytes\":{},\"max_signed_transaction_bytes\":{},\"max_broadcast_receipt_bytes\":{},\"max_reconciliation_bytes\":{},\"max_trace_events\":{},\"max_trace_bytes\":{},\"max_evidence_bytes\":{},\"max_builder_bytes\":{},\"max_json_depth\":{},\"max_identifier_bytes\":{},\"max_memo_bytes\":{},\"max_recipients\":{},\"max_network_policies\":{},\"max_x402_origins\":{},\"max_utxos\":{},\"max_reconciliations\":{},\"max_elapsed_ms\":{},\"max_amount_atomic\":{},\"max_fee_atomic\":{},\"max_compute_units\":{},\"max_confirmation_target\":{},\"max_concurrency\":{},\"max_unexpected_authority_calls\":{}}}",limits.max_policy_bytes,limits.max_intent_bytes,limits.max_invoice_bytes,limits.max_snapshot_bytes,limits.max_plan_bytes,limits.max_simulation_bytes,limits.max_approval_request_bytes,limits.max_approval_bytes,limits.max_journal_bytes,limits.max_unsigned_transaction_bytes,limits.max_signed_transaction_bytes,limits.max_broadcast_receipt_bytes,limits.max_reconciliation_bytes,limits.max_trace_events,limits.max_trace_bytes,limits.max_evidence_bytes,limits.max_builder_bytes,limits.max_json_depth,limits.max_identifier_bytes,limits.max_memo_bytes,limits.max_recipients,limits.max_network_policies,limits.max_x402_origins,limits.max_utxos,limits.max_reconciliations,limits.max_elapsed_ms,limits.max_amount_atomic,limits.max_fee_atomic,limits.max_compute_units,limits.max_confirmation_target,limits.max_concurrency,limits.max_unexpected_authority_calls)
}

fn render_policy(policy: &Policy) -> String {
    let mut networks = String::from("[");
    for (index, row) in policy.networks.iter().enumerate() {
        if index > 0 {
            networks.push(',');
        }
        networks.push_str(&format!("{{\"rail\":{},\"network\":{},\"asset\":{},\"recipients\":{},\"max_amount_atomic\":{},\"max_fee_atomic\":{},\"max_rolling_24h_atomic\":{}}}",quote_json(row.rail.text()),quote_json(&row.network),quote_json(&row.asset),string_list(&row.recipients),row.max_amount,row.max_fee,row.max_rolling));
    }
    networks.push(']');
    let mut origins = String::from("[");
    for (index, row) in policy.origins.iter().enumerate() {
        if index > 0 {
            origins.push(',');
        }
        origins.push_str(&format!("{{\"origin\":{},\"methods\":{},\"resources\":{},\"settlement_rails\":{},\"max_amount_atomic\":{}}}",quote_json(&row.origin),string_list(&row.methods),string_list(&row.resources),string_list(&row.rails.iter().map(|r|r.text().to_owned()).collect::<Vec<_>>()),row.max_amount));
    }
    origins.push(']');
    format!("{{\"schema\":\"{POLICY_SCHEMA}\",\"economic_agent_id\":{},\"wallet_id\":{},\"network_policies\":{networks},\"x402_origins\":{origins},\"limits\":{},\"nonclaims\":{}}}\n",quote_json(&policy.economic_agent_id),quote_json(&policy.wallet_id),limits_json(&policy.limits),nonclaims_json())
}

fn parse_policy(source: &str) -> Result<Policy, Diagnostic> {
    let (_, value) = canonical(source, "policy", POLICY_SCHEMA, MAX_POLICY_BYTES)?;
    let top = object(&value, "policy", POLICY_SCHEMA)?;
    if !keys(
        top,
        &[
            "schema",
            "economic_agent_id",
            "wallet_id",
            "network_policies",
            "x402_origins",
            "limits",
            "nonclaims",
        ],
    ) {
        return Err(g210("policy", POLICY_SCHEMA));
    }
    let economic_agent_id = text(top, "economic_agent_id", "policy", POLICY_SCHEMA)?.to_owned();
    let wallet_id = text(top, "wallet_id", "policy", POLICY_SCHEMA)?.to_owned();
    if !identifier(&economic_agent_id) || !identifier(&wallet_id) {
        return Err(g211("identifiers"));
    }
    let rows = top["network_policies"]
        .as_array()
        .ok_or_else(|| g210("policy", POLICY_SCHEMA))?;
    if rows.is_empty() || rows.len() > MAX_NETWORK_POLICIES {
        return Err(g211("network_policies"));
    }
    let mut networks = Vec::new();
    for value in rows {
        let row = object(value, "policy", POLICY_SCHEMA)?;
        if !keys(
            row,
            &[
                "rail",
                "network",
                "asset",
                "recipients",
                "max_amount_atomic",
                "max_fee_atomic",
                "max_rolling_24h_atomic",
            ],
        ) {
            return Err(g210("policy", POLICY_SCHEMA));
        }
        let rail = rail(text(row, "rail", "policy", POLICY_SCHEMA)?)
            .ok_or_else(|| g211("network_policies.rail"))?;
        let network = text(row, "network", "policy", POLICY_SCHEMA)?.to_owned();
        let asset = text(row, "asset", "policy", POLICY_SCHEMA)?.to_owned();
        if (rail, network.as_str(), asset.as_str()) != (EconomicRail::Evm, "sepolia", "native:eth")
            && (rail, network.as_str(), asset.as_str())
                != (EconomicRail::Solana, "devnet", "native:sol")
            && (rail, network.as_str(), asset.as_str())
                != (EconomicRail::Bitcoin, "regtest", "native:btc")
        {
            return Err(g211("network_policies.network"));
        }
        let recipients =
            string_array(&row["recipients"]).ok_or_else(|| g210("policy", POLICY_SCHEMA))?;
        if recipients.is_empty()
            || recipients.len() > MAX_RECIPIENTS
            || !sorted_unique(&recipients)
            || recipients.iter().any(|v| !valid_recipient(rail, v))
        {
            return Err(g211("network_policies.recipients"));
        }
        let max_amount = number(row, "max_amount_atomic", "policy", POLICY_SCHEMA)?;
        let max_fee = number(row, "max_fee_atomic", "policy", POLICY_SCHEMA)?;
        let max_rolling = number(row, "max_rolling_24h_atomic", "policy", POLICY_SCHEMA)?;
        if max_amount == 0
            || max_amount > 1_000_000_000_000_000_000
            || max_fee > 1_000_000_000_000_000
            || max_rolling < max_amount
        {
            return Err(g211("network_policies.limits"));
        }
        networks.push(NetworkPolicy {
            rail,
            network,
            asset,
            recipients,
            max_amount,
            max_fee,
            max_rolling,
        });
    }
    if !networks.windows(2).all(|w| {
        (w[0].rail.text(), w[0].network.as_str(), w[0].asset.as_str())
            < (w[1].rail.text(), w[1].network.as_str(), w[1].asset.as_str())
    }) {
        return Err(g211("network_policies.order"));
    }
    let origin_rows = top["x402_origins"]
        .as_array()
        .ok_or_else(|| g210("policy", POLICY_SCHEMA))?;
    if origin_rows.len() > MAX_X402_ORIGINS {
        return Err(g211("x402_origins"));
    }
    let mut origins = Vec::new();
    for value in origin_rows {
        let row = object(value, "policy", POLICY_SCHEMA)?;
        if !keys(
            row,
            &[
                "origin",
                "methods",
                "resources",
                "settlement_rails",
                "max_amount_atomic",
            ],
        ) {
            return Err(g210("policy", POLICY_SCHEMA));
        }
        let origin = text(row, "origin", "policy", POLICY_SCHEMA)?.to_owned();
        if !valid_origin(&origin) {
            return Err(g211("x402_origins.origin"));
        }
        let methods = string_array(&row["methods"]).ok_or_else(|| g210("policy", POLICY_SCHEMA))?;
        let resources =
            string_array(&row["resources"]).ok_or_else(|| g210("policy", POLICY_SCHEMA))?;
        let rail_text =
            string_array(&row["settlement_rails"]).ok_or_else(|| g210("policy", POLICY_SCHEMA))?;
        let rails: Vec<_> = rail_text
            .iter()
            .map(|v| rail(v).ok_or_else(|| g211("x402_origins.settlement_rails")))
            .collect::<Result<_, _>>()?;
        let max_amount = number(row, "max_amount_atomic", "policy", POLICY_SCHEMA)?;
        if methods.is_empty()
            || !sorted_unique(&methods)
            || methods.iter().any(|v| v != "GET" && v != "POST")
            || resources.is_empty()
            || !sorted_unique(&resources)
            || resources.iter().any(|v| !valid_resource(v))
            || rails.is_empty()
            || !rails.windows(2).all(|w| w[0].text() < w[1].text())
            || max_amount == 0
            || max_amount > 1_000_000_000_000_000_000
        {
            return Err(g211("x402_origins"));
        }
        origins.push(OriginPolicy {
            origin,
            methods,
            resources,
            rails,
            max_amount,
        });
    }
    if !origins.windows(2).all(|w| w[0].origin < w[1].origin) {
        return Err(g211("x402_origins.order"));
    }
    let limits = top["limits"]
        .as_object()
        .ok_or_else(|| g210("policy", POLICY_SCHEMA))?;
    let expected_limit_keys = [
        "max_policy_bytes",
        "max_intent_bytes",
        "max_invoice_bytes",
        "max_snapshot_bytes",
        "max_plan_bytes",
        "max_simulation_bytes",
        "max_approval_request_bytes",
        "max_approval_bytes",
        "max_journal_bytes",
        "max_unsigned_transaction_bytes",
        "max_signed_transaction_bytes",
        "max_broadcast_receipt_bytes",
        "max_reconciliation_bytes",
        "max_trace_events",
        "max_trace_bytes",
        "max_evidence_bytes",
        "max_builder_bytes",
        "max_json_depth",
        "max_identifier_bytes",
        "max_memo_bytes",
        "max_recipients",
        "max_network_policies",
        "max_x402_origins",
        "max_utxos",
        "max_reconciliations",
        "max_elapsed_ms",
        "max_amount_atomic",
        "max_fee_atomic",
        "max_compute_units",
        "max_confirmation_target",
        "max_concurrency",
        "max_unexpected_authority_calls",
    ];
    if !keys(limits, &expected_limit_keys) {
        return Err(g211("limits"));
    }
    let limits = Limits {
        max_policy_bytes: policy_limit(limits, "max_policy_bytes", MAX_POLICY_BYTES as u64, true)?,
        max_intent_bytes: policy_limit(limits, "max_intent_bytes", MAX_INTENT_BYTES as u64, true)?,
        max_invoice_bytes: policy_limit(
            limits,
            "max_invoice_bytes",
            MAX_INVOICE_BYTES as u64,
            true,
        )?,
        max_snapshot_bytes: policy_limit(
            limits,
            "max_snapshot_bytes",
            MAX_SNAPSHOT_BYTES as u64,
            true,
        )?,
        max_plan_bytes: policy_limit(limits, "max_plan_bytes", MAX_PLAN_BYTES as u64, true)?,
        max_simulation_bytes: policy_limit(
            limits,
            "max_simulation_bytes",
            MAX_SIMULATION_BYTES as u64,
            true,
        )?,
        max_approval_request_bytes: policy_limit(
            limits,
            "max_approval_request_bytes",
            MAX_APPROVAL_REQUEST_BYTES as u64,
            true,
        )?,
        max_approval_bytes: policy_limit(
            limits,
            "max_approval_bytes",
            MAX_APPROVAL_BYTES as u64,
            true,
        )?,
        max_journal_bytes: policy_limit(
            limits,
            "max_journal_bytes",
            MAX_JOURNAL_BYTES as u64,
            true,
        )?,
        max_unsigned_transaction_bytes: policy_limit(
            limits,
            "max_unsigned_transaction_bytes",
            MAX_UNSIGNED_BYTES as u64,
            true,
        )?,
        max_signed_transaction_bytes: policy_limit(
            limits,
            "max_signed_transaction_bytes",
            MAX_SIGNED_BYTES as u64,
            true,
        )?,
        max_broadcast_receipt_bytes: policy_limit(
            limits,
            "max_broadcast_receipt_bytes",
            MAX_BROADCAST_BYTES as u64,
            true,
        )?,
        max_reconciliation_bytes: policy_limit(
            limits,
            "max_reconciliation_bytes",
            MAX_RECONCILIATION_BYTES as u64,
            true,
        )?,
        max_trace_events: policy_limit(limits, "max_trace_events", MAX_TRACE_EVENTS as u64, true)?,
        max_trace_bytes: policy_limit(limits, "max_trace_bytes", MAX_TRACE_BYTES as u64, true)?,
        max_evidence_bytes: policy_limit(
            limits,
            "max_evidence_bytes",
            MAX_EVIDENCE_BYTES as u64,
            true,
        )?,
        max_builder_bytes: policy_limit(
            limits,
            "max_builder_bytes",
            MAX_BUILDER_BYTES as u64,
            true,
        )?,
        max_json_depth: policy_limit(limits, "max_json_depth", MAX_JSON_DEPTH as u64, true)?,
        max_identifier_bytes: policy_limit(
            limits,
            "max_identifier_bytes",
            MAX_IDENTIFIER_BYTES as u64,
            true,
        )?,
        max_memo_bytes: policy_limit(limits, "max_memo_bytes", MAX_MEMO_BYTES as u64, true)?,
        max_recipients: policy_limit(limits, "max_recipients", MAX_RECIPIENTS as u64, true)?,
        max_network_policies: policy_limit(
            limits,
            "max_network_policies",
            MAX_NETWORK_POLICIES as u64,
            true,
        )?,
        max_x402_origins: policy_limit(limits, "max_x402_origins", MAX_X402_ORIGINS as u64, false)?,
        max_utxos: policy_limit(limits, "max_utxos", MAX_UTXOS as u64, true)?,
        max_reconciliations: policy_limit(limits, "max_reconciliations", 64, true)?,
        max_elapsed_ms: policy_limit(limits, "max_elapsed_ms", 600_000, true)?,
        max_amount_atomic: policy_limit(
            limits,
            "max_amount_atomic",
            1_000_000_000_000_000_000,
            true,
        )?,
        max_fee_atomic: policy_limit(limits, "max_fee_atomic", 1_000_000_000_000_000, true)?,
        max_compute_units: policy_limit(limits, "max_compute_units", 200_000, true)?,
        max_confirmation_target: policy_limit(limits, "max_confirmation_target", 144, true)?,
        max_concurrency: policy_limit(limits, "max_concurrency", 1, true)?,
        max_unexpected_authority_calls: policy_limit(
            limits,
            "max_unexpected_authority_calls",
            0,
            false,
        )?,
    };
    if limits.max_concurrency != 1 || limits.max_unexpected_authority_calls != 0 {
        return Err(g211("limits"));
    }
    if economic_agent_id.len() > limits.max_identifier_bytes as usize
        || wallet_id.len() > limits.max_identifier_bytes as usize
    {
        return Err(g211("limits.max_identifier_bytes"));
    }
    if networks.len() > limits.max_network_policies as usize
        || origins.len() > limits.max_x402_origins as usize
        || networks.iter().any(|network| {
            network.recipients.len() > limits.max_recipients as usize
                || network.max_amount > limits.max_amount_atomic
                || network.max_fee > limits.max_fee_atomic
                || network.max_rolling > limits.max_amount_atomic
        })
        || origins
            .iter()
            .any(|origin| origin.max_amount > limits.max_amount_atomic)
    {
        return Err(g211("limits"));
    }
    let claims = string_array(&top["nonclaims"]).ok_or_else(|| g210("policy", POLICY_SCHEMA))?;
    if claims
        != NONCLAIMS
            .iter()
            .map(|v| (*v).to_owned())
            .collect::<Vec<_>>()
    {
        return Err(g211("nonclaims"));
    }
    let mut policy = Policy {
        economic_agent_id,
        wallet_id,
        networks,
        origins,
        limits,
        source: source.to_owned(),
        digest: digest(POLICY_DOMAIN, source.as_bytes()),
    };
    if render_policy(&policy) != source {
        return Err(g210("policy", POLICY_SCHEMA));
    }
    if source.len() > policy.limits.max_policy_bytes as usize {
        return Err(g216("policy_bytes", policy.limits.max_policy_bytes));
    }
    let policy_value: Value =
        serde_json::from_str(source.trim_end()).map_err(|_| g210("policy", POLICY_SCHEMA))?;
    if depth(&policy_value) as u64 > policy.limits.max_json_depth {
        return Err(g216("json_depth", policy.limits.max_json_depth));
    }
    policy.source = source.to_owned();
    Ok(policy)
}

fn valid_recipient(rail: EconomicRail, value: &str) -> bool {
    match rail {
        EconomicRail::Evm => {
            value.len() == 42
                && value.starts_with("0x")
                && value[2..]
                    .bytes()
                    .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
        }
        EconomicRail::Solana => decode_base58_32(value).is_some(),
        EconomicRail::Bitcoin => decode_regtest_p2wpkh(value).is_some(),
    }
}
fn valid_origin(value: &str) -> bool {
    let Some(host) = value.strip_prefix("https://") else {
        return false;
    };
    !host.is_empty()
        && !host.contains(['/', ':', '@', '#', '?', '[', ']'])
        && host.parse::<IpAddr>().is_err()
        && host != "localhost"
        && !host.ends_with(".localhost")
        && !host.ends_with(".local")
        && host.bytes().all(|byte| {
            byte.is_ascii_lowercase() || byte.is_ascii_digit() || matches!(byte, b'.' | b'-')
        })
        && host
            .split('.')
            .all(|part| !part.is_empty() && !part.starts_with('-') && !part.ends_with('-'))
}

fn valid_resource(value: &str) -> bool {
    if !value.starts_with('/') || value.starts_with("//") || value.contains(['?', '#', '\\']) {
        return false;
    }
    if value
        .split('/')
        .any(|segment| matches!(segment, "." | ".."))
    {
        return false;
    }
    let bytes = value.as_bytes();
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index] != b'%' {
            index += 1;
            continue;
        }
        let Some(pair) = bytes.get(index + 1..index + 3) else {
            return false;
        };
        let Some(high) = (pair[0] as char).to_digit(16) else {
            return false;
        };
        let Some(low) = (pair[1] as char).to_digit(16) else {
            return false;
        };
        if matches!(((high << 4) | low) as u8, b'.' | b'/' | b'\\') {
            return false;
        }
        index += 3;
    }
    true
}

const BASE58_ALPHABET: &[u8] = b"123456789ABCDEFGHJKLMNPQRSTUVWXYZabcdefghijkmnopqrstuvwxyz";

fn encode_base58(bytes: &[u8]) -> String {
    if bytes.is_empty() {
        return String::new();
    }
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
    let mut encoded = String::with_capacity(zeros + digits.len());
    encoded.extend(std::iter::repeat_n('1', zeros));
    for digit in digits.iter().rev() {
        encoded.push(BASE58_ALPHABET[usize::from(*digit)] as char);
    }
    encoded
}

fn decode_base58_32(value: &str) -> Option<[u8; 32]> {
    if value.is_empty() {
        return None;
    }
    let mut output = [0u8; 32];
    for byte in value.bytes() {
        let digit = BASE58_ALPHABET
            .iter()
            .position(|candidate| *candidate == byte)? as u32;
        let mut carry = digit;
        for slot in output.iter_mut().rev() {
            let expanded = u32::from(*slot) * 58 + carry;
            *slot = expanded as u8;
            carry = expanded >> 8;
        }
        if carry != 0 {
            return None;
        }
    }
    (encode_base58(&output) == value).then_some(output)
}
fn decode_regtest_p2wpkh(value: &str) -> Option<Vec<u8>> {
    if value.to_ascii_lowercase() != value || !value.starts_with("bcrt1q") {
        return None;
    }
    let position = value.rfind('1')?;
    let hrp = &value[..position];
    if hrp != "bcrt" {
        return None;
    }
    let charset = b"qpzry9x8gf2tvdw0s3jn54khce6mua7l";
    let data: Vec<u8> = value[position + 1..]
        .bytes()
        .map(|b| charset.iter().position(|v| *v == b).map(|n| n as u8))
        .collect::<Option<_>>()?;
    if data.len() < 7 || !bech32_verify(hrp, &data) {
        return None;
    }
    let payload = &data[..data.len() - 6];
    if payload.first() != Some(&0) {
        return None;
    }
    let program = convert_bits(&payload[1..], 5, 8, false)?;
    if program.len() != 20 {
        return None;
    }
    let mut script = vec![0x00, 0x14];
    script.extend(program);
    Some(script)
}
fn bech32_verify(hrp: &str, data: &[u8]) -> bool {
    let mut values = Vec::new();
    for b in hrp.bytes() {
        values.push(b >> 5);
    }
    values.push(0);
    for b in hrp.bytes() {
        values.push(b & 31);
    }
    values.extend_from_slice(data);
    let mut chk = 1u32;
    for v in values {
        let top = chk >> 25;
        chk = ((chk & 0x1ffffff) << 5) ^ u32::from(v);
        for (index, g) in [0x3b6a57b2, 0x26508e6d, 0x1ea119fa, 0x3d4233dd, 0x2a1462b3]
            .iter()
            .enumerate()
        {
            if ((top >> index) & 1) != 0 {
                chk ^= *g;
            }
        }
    }
    chk == 1
}
fn convert_bits(data: &[u8], from: u32, to: u32, pad: bool) -> Option<Vec<u8>> {
    let mut acc = 0u32;
    let mut bits = 0u32;
    let maxv = (1u32 << to) - 1;
    let mut out = Vec::new();
    for value in data {
        if (u32::from(*value) >> from) != 0 {
            return None;
        }
        acc = (acc << from) | u32::from(*value);
        bits += from;
        while bits >= to {
            bits -= to;
            out.push(((acc >> bits) & maxv) as u8);
        }
    }
    if pad {
        if bits > 0 {
            out.push(((acc << (to - bits)) & maxv) as u8);
        }
    } else if bits >= from || ((acc << (to - bits)) & maxv) != 0 {
        return None;
    }
    Some(out)
}

fn render_intent(intent: &Intent) -> String {
    let memo = intent
        .memo
        .as_ref()
        .map_or_else(|| "null".to_owned(), |v| quote_json(v));
    let payment=match &intent.payment{
        Payment::Evm{recipient,amount,max_fee}=>format!("{{\"kind\":\"evm\",\"network\":\"sepolia\",\"asset\":\"native:eth\",\"recipient\":{},\"amount_atomic\":{amount},\"max_fee_atomic\":{max_fee}}}",quote_json(recipient)),
        Payment::Solana{recipient,amount,max_fee,compute,priority}=>format!("{{\"kind\":\"solana\",\"network\":\"devnet\",\"asset\":\"native:sol\",\"recipient\":{},\"amount_atomic\":{amount},\"max_fee_atomic\":{max_fee},\"max_compute_units\":{compute},\"max_priority_fee_atomic\":{priority}}}",quote_json(recipient)),
        Payment::Bitcoin{recipient,amount,max_fee,confirmations}=>format!("{{\"kind\":\"bitcoin\",\"network\":\"regtest\",\"asset\":\"native:btc\",\"recipient\":{},\"amount_atomic\":{amount},\"max_fee_atomic\":{max_fee},\"confirmation_target\":{confirmations}}}",quote_json(recipient)),
        Payment::X402{origin,method,resource,invoice_digest,payee,rail,network,asset,amount,max_fee,invoice_expires,nonce}=>format!("{{\"kind\":\"x402\",\"origin\":{},\"method\":{},\"resource\":{},\"invoice_digest\":{},\"payee\":{},\"settlement_rail\":{},\"network\":{},\"asset\":{},\"amount_atomic\":{amount},\"max_fee_atomic\":{max_fee},\"invoice_expires_at_ms\":{invoice_expires},\"invoice_nonce\":{}}}",quote_json(origin),quote_json(method),quote_json(resource),quote_json(invoice_digest),quote_json(payee),quote_json(rail.text()),quote_json(network),quote_json(asset),quote_json(nonce)),
    };
    format!("{{\"schema\":\"{INTENT_SCHEMA}\",\"intent_id\":{},\"wallet_id\":{},\"rail\":{},\"idempotency_key\":{},\"created_at_ms\":{},\"expires_at_ms\":{},\"memo\":{memo},\"payment\":{payment}}}\n",quote_json(&intent.intent_id),quote_json(&intent.wallet_id),quote_json(&intent.rail_text),quote_json(&intent.idempotency_key),intent.created_at,intent.expires_at)
}

fn parse_intent(source: &str) -> Result<Intent, Diagnostic> {
    let (_, value) = canonical(source, "payment intent", INTENT_SCHEMA, MAX_INTENT_BYTES)?;
    let top = object(&value, "payment intent", INTENT_SCHEMA)?;
    if !keys(
        top,
        &[
            "schema",
            "intent_id",
            "wallet_id",
            "rail",
            "idempotency_key",
            "created_at_ms",
            "expires_at_ms",
            "memo",
            "payment",
        ],
    ) {
        return Err(g210("payment intent", INTENT_SCHEMA));
    }
    let intent_id = text(top, "intent_id", "payment intent", INTENT_SCHEMA)?.to_owned();
    let wallet_id = text(top, "wallet_id", "payment intent", INTENT_SCHEMA)?.to_owned();
    let rail_text = text(top, "rail", "payment intent", INTENT_SCHEMA)?.to_owned();
    let idempotency_key = text(top, "idempotency_key", "payment intent", INTENT_SCHEMA)?.to_owned();
    if !identifier(&intent_id) || !identifier(&wallet_id) || !identifier(&idempotency_key) {
        return Err(g210("payment intent", INTENT_SCHEMA));
    }
    let created_at = number(top, "created_at_ms", "payment intent", INTENT_SCHEMA)?;
    let expires_at = number(top, "expires_at_ms", "payment intent", INTENT_SCHEMA)?;
    if expires_at <= created_at || expires_at - created_at > 600_000 {
        return Err(g212("expired"));
    }
    let memo = if top["memo"].is_null() {
        None
    } else {
        Some(
            top["memo"]
                .as_str()
                .ok_or_else(|| g210("payment intent", INTENT_SCHEMA))?
                .to_owned(),
        )
    };
    if memo.as_ref().is_some_and(|v| v.len() > MAX_MEMO_BYTES) {
        return Err(g216("memo_bytes", MAX_MEMO_BYTES as u64));
    }
    let row = object(&top["payment"], "payment intent", INTENT_SCHEMA)?;
    let kind = text(row, "kind", "payment intent", INTENT_SCHEMA)?;
    let payment = match kind {
        "evm" => {
            if !keys(
                row,
                &[
                    "kind",
                    "network",
                    "asset",
                    "recipient",
                    "amount_atomic",
                    "max_fee_atomic",
                ],
            ) || text(row, "network", "payment intent", INTENT_SCHEMA)? != "sepolia"
                || text(row, "asset", "payment intent", INTENT_SCHEMA)? != "native:eth"
                || rail_text != "evm"
            {
                return Err(g210("payment intent", INTENT_SCHEMA));
            }
            Payment::Evm {
                recipient: text(row, "recipient", "payment intent", INTENT_SCHEMA)?.to_owned(),
                amount: number(row, "amount_atomic", "payment intent", INTENT_SCHEMA)?,
                max_fee: number(row, "max_fee_atomic", "payment intent", INTENT_SCHEMA)?,
            }
        }
        "solana" => {
            if !keys(
                row,
                &[
                    "kind",
                    "network",
                    "asset",
                    "recipient",
                    "amount_atomic",
                    "max_fee_atomic",
                    "max_compute_units",
                    "max_priority_fee_atomic",
                ],
            ) || text(row, "network", "payment intent", INTENT_SCHEMA)? != "devnet"
                || text(row, "asset", "payment intent", INTENT_SCHEMA)? != "native:sol"
                || rail_text != "solana"
            {
                return Err(g210("payment intent", INTENT_SCHEMA));
            }
            Payment::Solana {
                recipient: text(row, "recipient", "payment intent", INTENT_SCHEMA)?.to_owned(),
                amount: number(row, "amount_atomic", "payment intent", INTENT_SCHEMA)?,
                max_fee: number(row, "max_fee_atomic", "payment intent", INTENT_SCHEMA)?,
                compute: number(row, "max_compute_units", "payment intent", INTENT_SCHEMA)?,
                priority: number(
                    row,
                    "max_priority_fee_atomic",
                    "payment intent",
                    INTENT_SCHEMA,
                )?,
            }
        }
        "bitcoin" => {
            if !keys(
                row,
                &[
                    "kind",
                    "network",
                    "asset",
                    "recipient",
                    "amount_atomic",
                    "max_fee_atomic",
                    "confirmation_target",
                ],
            ) || text(row, "network", "payment intent", INTENT_SCHEMA)? != "regtest"
                || text(row, "asset", "payment intent", INTENT_SCHEMA)? != "native:btc"
                || rail_text != "bitcoin"
            {
                return Err(g210("payment intent", INTENT_SCHEMA));
            }
            Payment::Bitcoin {
                recipient: text(row, "recipient", "payment intent", INTENT_SCHEMA)?.to_owned(),
                amount: number(row, "amount_atomic", "payment intent", INTENT_SCHEMA)?,
                max_fee: number(row, "max_fee_atomic", "payment intent", INTENT_SCHEMA)?,
                confirmations: number(row, "confirmation_target", "payment intent", INTENT_SCHEMA)?,
            }
        }
        "x402" => {
            if !keys(
                row,
                &[
                    "kind",
                    "origin",
                    "method",
                    "resource",
                    "invoice_digest",
                    "payee",
                    "settlement_rail",
                    "network",
                    "asset",
                    "amount_atomic",
                    "max_fee_atomic",
                    "invoice_expires_at_ms",
                    "invoice_nonce",
                ],
            ) || rail_text != "x402"
            {
                return Err(g210("payment intent", INTENT_SCHEMA));
            }
            Payment::X402 {
                origin: text(row, "origin", "payment intent", INTENT_SCHEMA)?.to_owned(),
                method: text(row, "method", "payment intent", INTENT_SCHEMA)?.to_owned(),
                resource: text(row, "resource", "payment intent", INTENT_SCHEMA)?.to_owned(),
                invoice_digest: text(row, "invoice_digest", "payment intent", INTENT_SCHEMA)?
                    .to_owned(),
                payee: text(row, "payee", "payment intent", INTENT_SCHEMA)?.to_owned(),
                rail: rail(text(
                    row,
                    "settlement_rail",
                    "payment intent",
                    INTENT_SCHEMA,
                )?)
                .ok_or_else(|| g210("payment intent", INTENT_SCHEMA))?,
                network: text(row, "network", "payment intent", INTENT_SCHEMA)?.to_owned(),
                asset: text(row, "asset", "payment intent", INTENT_SCHEMA)?.to_owned(),
                amount: number(row, "amount_atomic", "payment intent", INTENT_SCHEMA)?,
                max_fee: number(row, "max_fee_atomic", "payment intent", INTENT_SCHEMA)?,
                invoice_expires: number(
                    row,
                    "invoice_expires_at_ms",
                    "payment intent",
                    INTENT_SCHEMA,
                )?,
                nonce: text(row, "invoice_nonce", "payment intent", INTENT_SCHEMA)?.to_owned(),
            }
        }
        _ => return Err(g210("payment intent", INTENT_SCHEMA)),
    };
    let intent = Intent {
        intent_id,
        wallet_id,
        rail_text,
        idempotency_key,
        created_at,
        expires_at,
        memo,
        payment,
        source: source.to_owned(),
        digest: digest(INTENT_DOMAIN, source.as_bytes()),
    };
    if render_intent(&intent) != source {
        return Err(g210("payment intent", INTENT_SCHEMA));
    }
    Ok(intent)
}

#[derive(Clone)]
struct Utxo {
    txid: String,
    vout: u64,
    value: u64,
    script: String,
    confirmations: u64,
}
#[derive(Clone)]
enum SnapshotState {
    Evm {
        from: String,
        nonce: u64,
        base_fee: u64,
        priority: u64,
        gas: u64,
    },
    Solana {
        payer: String,
        blockhash: String,
        last_height: u64,
        fee: u64,
    },
    Bitcoin {
        wallet_script: String,
        height: u64,
        fee_rate: u64,
        utxos: Vec<Utxo>,
    },
}
#[derive(Clone)]
struct Snapshot {
    rail: EconomicRail,
    observed: u64,
    expires: u64,
    state: SnapshotState,
    doc: Doc,
}

fn render_snapshot(snapshot: &Snapshot) -> String {
    let(network,state)=match &snapshot.state{
    SnapshotState::Evm{from,nonce,base_fee,priority,gas}=>("sepolia",format!("{{\"chain_id\":11155111,\"from\":{},\"nonce\":{nonce},\"base_fee_per_gas\":{base_fee},\"max_priority_fee_per_gas\":{priority},\"gas_limit\":{gas}}}",quote_json(from))),
    SnapshotState::Solana{payer,blockhash,last_height,fee}=>("devnet",format!("{{\"fee_payer\":{},\"recent_blockhash\":{},\"last_valid_block_height\":{last_height},\"lamports_per_signature\":{fee}}}",quote_json(payer),quote_json(blockhash))),
    SnapshotState::Bitcoin{wallet_script,height,fee_rate,utxos}=>{let mut rows=String::from("[");for(index,u)in utxos.iter().enumerate(){if index>0{rows.push(',');}rows.push_str(&format!("{{\"txid\":{},\"vout\":{},\"value_atomic\":{},\"script_pubkey\":{},\"confirmations\":{}}}",quote_json(&u.txid),u.vout,u.value,quote_json(&u.script),u.confirmations));}rows.push(']');("regtest",format!("{{\"wallet_script_pubkey\":{},\"height\":{height},\"fee_rate_sat_vbyte\":{fee_rate},\"utxos\":{rows}}}",quote_json(wallet_script)))} };
    format!("{{\"schema\":\"{SNAPSHOT_SCHEMA}\",\"rail\":{},\"network\":{},\"observed_at_ms\":{},\"expires_at_ms\":{},\"state\":{state}}}\n",quote_json(snapshot.rail.text()),quote_json(network),snapshot.observed,snapshot.expires)
}

fn parse_snapshot(source: &str, expected: EconomicRail) -> Result<Snapshot, Diagnostic> {
    let (_, value) = canonical(
        source,
        "chain snapshot",
        SNAPSHOT_SCHEMA,
        MAX_SNAPSHOT_BYTES,
    )?;
    let top = object(&value, "chain snapshot", SNAPSHOT_SCHEMA)?;
    if !keys(
        top,
        &[
            "schema",
            "rail",
            "network",
            "observed_at_ms",
            "expires_at_ms",
            "state",
        ],
    ) {
        return Err(g210("chain snapshot", SNAPSHOT_SCHEMA));
    }
    let parsed = rail(text(top, "rail", "chain snapshot", SNAPSHOT_SCHEMA)?).ok_or_else(g213)?;
    if parsed != expected {
        return Err(g213());
    }
    let observed = number(top, "observed_at_ms", "chain snapshot", SNAPSHOT_SCHEMA)?;
    let expires = number(top, "expires_at_ms", "chain snapshot", SNAPSHOT_SCHEMA)?;
    if expires <= observed || expires - observed > 600_000 {
        return Err(g213());
    }
    let row = object(&top["state"], "chain snapshot", SNAPSHOT_SCHEMA)?;
    let state = match parsed {
        EconomicRail::Evm => {
            if text(top, "network", "chain snapshot", SNAPSHOT_SCHEMA)? != "sepolia"
                || !keys(
                    row,
                    &[
                        "chain_id",
                        "from",
                        "nonce",
                        "base_fee_per_gas",
                        "max_priority_fee_per_gas",
                        "gas_limit",
                    ],
                )
                || number(row, "chain_id", "chain snapshot", SNAPSHOT_SCHEMA)? != 11155111
            {
                return Err(g213());
            }
            let from = text(row, "from", "chain snapshot", SNAPSHOT_SCHEMA)?.to_owned();
            if !valid_recipient(parsed, &from) {
                return Err(g213());
            }
            let gas = number(row, "gas_limit", "chain snapshot", SNAPSHOT_SCHEMA)?;
            if gas != 21000 {
                return Err(g213());
            }
            SnapshotState::Evm {
                from,
                nonce: number(row, "nonce", "chain snapshot", SNAPSHOT_SCHEMA)?,
                base_fee: number(row, "base_fee_per_gas", "chain snapshot", SNAPSHOT_SCHEMA)?,
                priority: number(
                    row,
                    "max_priority_fee_per_gas",
                    "chain snapshot",
                    SNAPSHOT_SCHEMA,
                )?,
                gas,
            }
        }
        EconomicRail::Solana => {
            if text(top, "network", "chain snapshot", SNAPSHOT_SCHEMA)? != "devnet"
                || !keys(
                    row,
                    &[
                        "fee_payer",
                        "recent_blockhash",
                        "last_valid_block_height",
                        "lamports_per_signature",
                    ],
                )
            {
                return Err(g213());
            }
            let payer = text(row, "fee_payer", "chain snapshot", SNAPSHOT_SCHEMA)?.to_owned();
            let blockhash =
                text(row, "recent_blockhash", "chain snapshot", SNAPSHOT_SCHEMA)?.to_owned();
            if decode_base58_32(&payer).is_none() || decode_base58_32(&blockhash).is_none() {
                return Err(g213());
            }
            SnapshotState::Solana {
                payer,
                blockhash,
                last_height: number(
                    row,
                    "last_valid_block_height",
                    "chain snapshot",
                    SNAPSHOT_SCHEMA,
                )?,
                fee: number(
                    row,
                    "lamports_per_signature",
                    "chain snapshot",
                    SNAPSHOT_SCHEMA,
                )?,
            }
        }
        EconomicRail::Bitcoin => {
            if text(top, "network", "chain snapshot", SNAPSHOT_SCHEMA)? != "regtest"
                || !keys(
                    row,
                    &[
                        "wallet_script_pubkey",
                        "height",
                        "fee_rate_sat_vbyte",
                        "utxos",
                    ],
                )
            {
                return Err(g213());
            }
            let wallet_script = text(
                row,
                "wallet_script_pubkey",
                "chain snapshot",
                SNAPSHOT_SCHEMA,
            )?
            .to_owned();
            if !valid_script(&wallet_script) {
                return Err(g213());
            }
            let values = row["utxos"].as_array().ok_or_else(g213)?;
            if values.is_empty() || values.len() > MAX_UTXOS {
                return Err(g216("utxos", MAX_UTXOS as u64));
            }
            let mut utxos = Vec::new();
            for value in values {
                let u = object(value, "chain snapshot", SNAPSHOT_SCHEMA)?;
                if !keys(
                    u,
                    &[
                        "txid",
                        "vout",
                        "value_atomic",
                        "script_pubkey",
                        "confirmations",
                    ],
                ) {
                    return Err(g210("chain snapshot", SNAPSHOT_SCHEMA));
                }
                let txid = text(u, "txid", "chain snapshot", SNAPSHOT_SCHEMA)?.to_owned();
                let script =
                    text(u, "script_pubkey", "chain snapshot", SNAPSHOT_SCHEMA)?.to_owned();
                if !lower_hex(&txid, 64) || !valid_script(&script) {
                    return Err(g213());
                }
                utxos.push(Utxo {
                    txid,
                    vout: number(u, "vout", "chain snapshot", SNAPSHOT_SCHEMA)?,
                    value: number(u, "value_atomic", "chain snapshot", SNAPSHOT_SCHEMA)?,
                    script,
                    confirmations: number(u, "confirmations", "chain snapshot", SNAPSHOT_SCHEMA)?,
                });
            }
            if !utxos
                .windows(2)
                .all(|w| (w[0].txid.as_str(), w[0].vout) < (w[1].txid.as_str(), w[1].vout))
                || utxos
                    .iter()
                    .any(|u| u.confirmations == 0 || u.script != wallet_script)
            {
                return Err(g213());
            }
            SnapshotState::Bitcoin {
                wallet_script,
                height: number(row, "height", "chain snapshot", SNAPSHOT_SCHEMA)?,
                fee_rate: number(row, "fee_rate_sat_vbyte", "chain snapshot", SNAPSHOT_SCHEMA)?,
                utxos,
            }
        }
    };
    let mut snapshot = Snapshot {
        rail: parsed,
        observed,
        expires,
        state,
        doc: Doc {
            source: source.to_owned(),
            digest: digest(SNAPSHOT_DOMAIN, source.as_bytes()),
        },
    };
    if render_snapshot(&snapshot) != source {
        return Err(g210("chain snapshot", SNAPSHOT_SCHEMA));
    }
    snapshot.doc.source = source.to_owned();
    Ok(snapshot)
}
fn parse_snapshot_limited(
    source: &str,
    expected: EconomicRail,
    limits: &Limits,
) -> Result<Snapshot, Diagnostic> {
    configured_document_limits(source, "chain snapshot", limits.max_snapshot_bytes, limits)?;
    reserve_parse_sidecar(source, limits)?;
    let snapshot = parse_snapshot(source, expected)?;
    if let SnapshotState::Bitcoin { utxos, .. } = &snapshot.state {
        if utxos.len() > limits.max_utxos as usize {
            return Err(g216("utxos", limits.max_utxos));
        }
    }
    Ok(snapshot)
}
fn reserve_parse_sidecar(source: &str, limits: &Limits) -> Result<(), Diagnostic> {
    let multiplier = usize::try_from(limits.max_json_depth)
        .map_err(|_| g217())?
        .checked_add(2)
        .ok_or_else(g217)?;
    let sidecar = source.len().checked_mul(multiplier).ok_or_else(g217)?;
    if !reserve_active_preserving(sidecar, terminal_floor(limits)?) {
        return Err(g216("builder_bytes", limits.max_builder_bytes));
    }
    Ok(())
}
fn lower_hex(value: &str, n: usize) -> bool {
    value.len() == n
        && value
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}
fn valid_script(value: &str) -> bool {
    value.len() == 44 && value.starts_with("0014") && lower_hex(value, 44)
}
fn hex_bytes(value: &str) -> Option<Vec<u8>> {
    if value.len() % 2 != 0 {
        return None;
    }
    value
        .as_bytes()
        .chunks(2)
        .map(|c| u8::from_str_radix(std::str::from_utf8(c).ok()?, 16).ok())
        .collect()
}

fn rlp_bytes(value: &[u8]) -> Vec<u8> {
    if value.len() == 1 && value[0] < 0x80 {
        return value.to_vec();
    }
    if value.len() < 56 {
        let mut out = vec![0x80 + value.len() as u8];
        out.extend_from_slice(value);
        out
    } else {
        let len = (value.len() as u64).to_be_bytes();
        let first = len.iter().position(|b| *b != 0).unwrap_or(7);
        let mut out = vec![0xb7 + (8 - first) as u8];
        out.extend_from_slice(&len[first..]);
        out.extend_from_slice(value);
        out
    }
}
fn rlp_u64(value: u64) -> Vec<u8> {
    if value == 0 {
        return vec![0x80];
    }
    let bytes = value.to_be_bytes();
    rlp_bytes(&bytes[bytes.iter().position(|b| *b != 0).unwrap_or(7)..])
}
fn rlp_list(items: &[Vec<u8>]) -> Vec<u8> {
    let payload = items.concat();
    if payload.len() < 56 {
        let mut out = vec![0xc0 + payload.len() as u8];
        out.extend(payload);
        out
    } else {
        let len = (payload.len() as u64).to_be_bytes();
        let first = len.iter().position(|b| *b != 0).unwrap_or(7);
        let mut out = vec![0xf7 + (8 - first) as u8];
        out.extend_from_slice(&len[first..]);
        out.extend(payload);
        out
    }
}
fn shortvec(mut value: usize) -> Vec<u8> {
    let mut out = Vec::new();
    loop {
        let mut byte = (value & 0x7f) as u8;
        value >>= 7;
        if value > 0 {
            byte |= 0x80;
        }
        out.push(byte);
        if value == 0 {
            return out;
        }
    }
}

fn build_unsigned(
    intent: &Intent,
    snapshot: &Snapshot,
) -> Result<(Vec<u8>, &'static str), Diagnostic> {
    build_unsigned_limited(
        intent,
        snapshot,
        MAX_UNSIGNED_BYTES as u64,
        MAX_BUILDER_BYTES as u64,
        0,
    )
}
fn build_unsigned_limited(
    intent: &Intent,
    snapshot: &Snapshot,
    unsigned_max: u64,
    builder_max: u64,
    terminal_floor: usize,
) -> Result<(Vec<u8>, &'static str), Diagnostic> {
    let bytes = match (&intent.payment, &snapshot.state) {
        (
            Payment::Evm {
                recipient,
                amount,
                max_fee,
            },
            SnapshotState::Evm {
                nonce,
                base_fee,
                priority,
                gas,
                ..
            },
        ) => {
            let per_gas = base_fee
                .checked_mul(2)
                .and_then(|v| v.checked_add(*priority))
                .ok_or_else(g213)?;
            let total = per_gas.checked_mul(21000).ok_or_else(g213)?;
            if total > *max_fee || *gas != 21000 {
                return Err(g213());
            }
            let to = hex_bytes(&recipient[2..]).ok_or_else(g213)?;
            let mut out = vec![0x02];
            out.extend(rlp_list(&[
                rlp_u64(11155111),
                rlp_u64(*nonce),
                rlp_u64(*priority),
                rlp_u64(per_gas),
                rlp_u64(21000),
                rlp_bytes(&to),
                rlp_u64(*amount),
                rlp_bytes(&[]),
                rlp_list(&[]),
            ]));
            out
        }
        (
            Payment::Solana {
                recipient,
                amount,
                max_fee,
                compute,
                priority,
            },
            SnapshotState::Solana {
                payer,
                blockhash,
                fee,
                ..
            },
        ) => {
            if *compute == 0 || *compute > 200000 {
                return Err(g216("compute_units", 200000));
            }
            let price = priority
                .checked_mul(1_000_000)
                .map(|v| v / compute)
                .ok_or_else(g213)?;
            let priority_fee = compute
                .checked_mul(price)
                .and_then(|v| v.checked_add(999999))
                .map(|v| v / 1000000)
                .ok_or_else(g213)?;
            if priority_fee > *priority
                || fee.checked_add(priority_fee).ok_or_else(g213)? > *max_fee
            {
                return Err(g213());
            }
            let payer = decode_base58_32(payer).ok_or_else(g213)?;
            let recipient = decode_base58_32(recipient).ok_or_else(g213)?;
            let system = decode_base58_32("11111111111111111111111111111111").ok_or_else(g213)?;
            let compute_program =
                decode_base58_32("ComputeBudget111111111111111111111111111111").ok_or_else(g213)?;
            let blockhash = decode_base58_32(blockhash).ok_or_else(g213)?;
            let mut out = vec![0x80, 1, 0, 2];
            out.extend(shortvec(4));
            out.extend(payer);
            out.extend(recipient);
            out.extend(compute_program);
            out.extend(system);
            out.extend(blockhash);
            out.extend(shortvec(3));
            out.push(2);
            out.extend(shortvec(0));
            out.extend(shortvec(5));
            out.push(2);
            out.extend_from_slice(&(*compute as u32).to_le_bytes());
            out.push(2);
            out.extend(shortvec(0));
            out.extend(shortvec(9));
            out.push(3);
            out.extend_from_slice(&price.to_le_bytes());
            out.push(3);
            out.extend(shortvec(2));
            out.extend([0, 1]);
            out.extend(shortvec(12));
            out.extend_from_slice(&2u32.to_le_bytes());
            out.extend_from_slice(&amount.to_le_bytes());
            out.extend(shortvec(0));
            out
        }
        (
            Payment::Bitcoin {
                recipient,
                amount,
                max_fee,
                ..
            },
            SnapshotState::Bitcoin {
                height,
                fee_rate,
                utxos,
                wallet_script,
            },
        ) => build_psbt(
            utxos,
            wallet_script,
            recipient,
            *amount,
            *max_fee,
            *fee_rate,
            *height,
        )?,
        (Payment::X402 { rail, .. }, _) => {
            let mut clone = intent.clone();
            clone.payment = match rail {
                EconomicRail::Evm => {
                    if let Payment::X402 {
                        payee,
                        amount,
                        max_fee,
                        ..
                    } = &intent.payment
                    {
                        Payment::Evm {
                            recipient: payee.clone(),
                            amount: *amount,
                            max_fee: *max_fee,
                        }
                    } else {
                        unreachable!()
                    }
                }
                EconomicRail::Solana => {
                    if let Payment::X402 {
                        payee,
                        amount,
                        max_fee,
                        ..
                    } = &intent.payment
                    {
                        Payment::Solana {
                            recipient: payee.clone(),
                            amount: *amount,
                            max_fee: *max_fee,
                            compute: 200_000,
                            priority: 0,
                        }
                    } else {
                        unreachable!()
                    }
                }
                EconomicRail::Bitcoin => {
                    if let Payment::X402 {
                        payee,
                        amount,
                        max_fee,
                        ..
                    } = &intent.payment
                    {
                        Payment::Bitcoin {
                            recipient: payee.clone(),
                            amount: *amount,
                            max_fee: *max_fee,
                            confirmations: 1,
                        }
                    } else {
                        unreachable!()
                    }
                }
            };
            return build_unsigned_limited(
                &clone,
                snapshot,
                unsigned_max,
                builder_max,
                terminal_floor,
            );
        }
        _ => return Err(g213()),
    };
    if bytes.len() as u64 > unsigned_max {
        return Err(g216("unsigned_transaction_bytes", unsigned_max));
    }
    if !reserve_active_preserving(bytes.len(), terminal_floor) {
        return Err(g216("builder_bytes", builder_max));
    }
    let format = match snapshot.rail {
        EconomicRail::Evm => "eip1559-unsigned-v1",
        EconomicRail::Solana => "solana-message-v0",
        EconomicRail::Bitcoin => "psbt-v2",
    };
    Ok((bytes, format))
}

fn build_psbt(
    utxos: &[Utxo],
    wallet_script: &str,
    recipient: &str,
    amount: u64,
    max_fee: u64,
    fee_rate: u64,
    height: u64,
) -> Result<Vec<u8>, Diagnostic> {
    let mut selected = Vec::new();
    let mut total = 0u64;
    for u in utxos {
        selected.push(u);
        total = total.checked_add(u.value).ok_or_else(g213)?;
        let estimate = 10 + selected.len() as u64 * 68 + 2 * 31;
        let fee = estimate.checked_mul(fee_rate).ok_or_else(g213)?;
        if total >= amount.saturating_add(fee) {
            break;
        }
    }
    let estimate = 10 + selected.len() as u64 * 68 + 2 * 31;
    let mut fee = estimate.checked_mul(fee_rate).ok_or_else(g213)?;
    if fee > max_fee || total < amount.saturating_add(fee) {
        return Err(g213());
    }
    let mut change = total - amount - fee;
    if change < 546 {
        fee = fee.checked_add(change).ok_or_else(g213)?;
        change = 0;
    }
    if fee > max_fee {
        return Err(g213());
    }
    let recipient_script = decode_regtest_p2wpkh(recipient).ok_or_else(g213)?;
    let change_script = hex_bytes(wallet_script).ok_or_else(g213)?;
    let mut outputs = vec![(recipient_script, amount)];
    if change > 0 {
        outputs.push((change_script, change));
    }
    outputs.sort_by(|a, b| (a.1, a.0.as_slice()).cmp(&(b.1, b.0.as_slice())));
    let mut out = b"psbt\xff".to_vec();
    psbt_pair(&mut out, &[0x02], &2u32.to_le_bytes());
    psbt_pair(&mut out, &[0x03], &(height as u32).to_le_bytes());
    psbt_pair(&mut out, &[0x04], &compact_size(selected.len()));
    psbt_pair(&mut out, &[0x05], &compact_size(outputs.len()));
    psbt_pair(&mut out, &[0x06], &[0]);
    psbt_pair(&mut out, &[0xfb], &2u32.to_le_bytes());
    out.push(0);
    for u in selected {
        let script = hex_bytes(&u.script).ok_or_else(g213)?;
        let mut witness = u.value.to_le_bytes().to_vec();
        witness.extend(compact_size(script.len()));
        witness.extend(script);
        psbt_pair(&mut out, &[0x01], &witness);
        psbt_pair(&mut out, &[0x03], &1u32.to_le_bytes());
        let mut txid = hex_bytes(&u.txid).ok_or_else(g213)?;
        txid.reverse();
        psbt_pair(&mut out, &[0x0e], &txid);
        psbt_pair(&mut out, &[0x0f], &(u.vout as u32).to_le_bytes());
        psbt_pair(&mut out, &[0x10], &0xffff_ffffu32.to_le_bytes());
        out.push(0);
    }
    for (script, value) in outputs {
        psbt_pair(&mut out, &[0x03], &value.to_le_bytes());
        psbt_pair(&mut out, &[0x04], &script);
        out.push(0);
    }
    Ok(out)
}
fn psbt_pair(out: &mut Vec<u8>, key: &[u8], value: &[u8]) {
    out.extend(shortvec(key.len()));
    out.extend(key);
    out.extend(shortvec(value.len()));
    out.extend(value);
}
fn rlp_header(bytes: &[u8]) -> Option<(bool, usize, usize)> {
    let first = *bytes.first()?;
    match first {
        0x00..=0x7f => Some((false, 0, 1)),
        0x80..=0xb7 => {
            let len = (first - 0x80) as usize;
            (bytes.len() > len && !(len == 1 && bytes[1] < 0x80)).then_some((false, 1, len))
        }
        0xb8..=0xbf => {
            let n = (first - 0xb7) as usize;
            if bytes.len() < 1 + n || bytes[1] == 0 {
                return None;
            }
            let len = bytes[1..1 + n].iter().try_fold(0usize, |value, byte| {
                value.checked_mul(256)?.checked_add(*byte as usize)
            })?;
            (len >= 56 && bytes.len() >= 1 + n + len).then_some((false, 1 + n, len))
        }
        0xc0..=0xf7 => {
            let len = (first - 0xc0) as usize;
            (bytes.len() > len).then_some((true, 1, len))
        }
        0xf8..=0xff => {
            let n = (first - 0xf7) as usize;
            if bytes.len() < 1 + n || bytes[1] == 0 {
                return None;
            }
            let len = bytes[1..1 + n].iter().try_fold(0usize, |value, byte| {
                value.checked_mul(256)?.checked_add(*byte as usize)
            })?;
            (len >= 56 && bytes.len() >= 1 + n + len).then_some((true, 1 + n, len))
        }
    }
}
fn rlp_list_items(bytes: &[u8]) -> Option<Vec<&[u8]>> {
    let (list, header, len) = rlp_header(bytes)?;
    if !list || header + len != bytes.len() {
        return None;
    }
    let mut body = &bytes[header..];
    let mut out = Vec::new();
    while !body.is_empty() {
        let (_, item_header, item_len) = rlp_header(body)?;
        let total = item_header.checked_add(item_len)?;
        out.push(&body[..total]);
        body = &body[total..];
    }
    Some(out)
}
fn rlp_scalar(item: &[u8]) -> Option<&[u8]> {
    let (list, header, len) = rlp_header(item)?;
    (!list && header + len == item.len()).then_some(&item[header..])
}
fn valid_secp_scalar(value: &[u8], low_s: bool) -> bool {
    const ORDER: [u8; 32] = [
        0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xfe, 0xba, 0xae, 0xdc, 0xe6, 0xaf, 0x48, 0xa0,
        0x3b, 0xbf, 0xd2, 0x5e, 0x8c, 0xd0, 0x36, 0x41, 0x41, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x00,
    ];
    const HALF: [u8; 32] = [
        0x7f, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0x5d, 0x57, 0x6e, 0x73, 0x57, 0xa4, 0x50,
        0x1d, 0xdf, 0xe9, 0x2f, 0x46, 0x68, 0x1b, 0x20, 0xa0, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x00,
    ];
    if value.is_empty() || value.len() > 32 || value.first() == Some(&0) {
        return false;
    }
    let mut padded = [0u8; 32];
    padded[32 - value.len()..].copy_from_slice(value);
    padded < ORDER && (!low_s || padded <= HALF)
}
fn verify_evm_signed(unsigned: &[u8], signed: &[u8]) -> bool {
    if unsigned.first() != Some(&2) || signed.first() != Some(&2) {
        return false;
    }
    let Some(unsigned_items) = rlp_list_items(&unsigned[1..]) else {
        return false;
    };
    let Some(signed_items) = rlp_list_items(&signed[1..]) else {
        return false;
    };
    if unsigned_items.len() != 9
        || signed_items.len() != 12
        || unsigned_items
            .iter()
            .zip(&signed_items[..9])
            .any(|(a, b)| a != b)
    {
        return false;
    }
    let Some(parity) = rlp_scalar(signed_items[9]) else {
        return false;
    };
    if !matches!(parity, [] | [1]) {
        return false;
    }
    let Some(r) = rlp_scalar(signed_items[10]) else {
        return false;
    };
    let Some(s) = rlp_scalar(signed_items[11]) else {
        return false;
    };
    valid_secp_scalar(r, false) && valid_secp_scalar(s, true)
}
fn take<'a>(bytes: &mut &'a [u8], length: usize) -> Option<&'a [u8]> {
    if bytes.len() < length {
        return None;
    }
    let (value, rest) = bytes.split_at(length);
    *bytes = rest;
    Some(value)
}
fn read_compact(bytes: &mut &[u8]) -> Option<u64> {
    let first = *take(bytes, 1)?.first()?;
    match first {
        0..=0xfc => Some(first as u64),
        0xfd => {
            let value = u16::from_le_bytes(take(bytes, 2)?.try_into().ok()?) as u64;
            (value >= 0xfd).then_some(value)
        }
        0xfe => {
            let value = u32::from_le_bytes(take(bytes, 4)?.try_into().ok()?) as u64;
            (value > u16::MAX as u64).then_some(value)
        }
        0xff => {
            let value = u64::from_le_bytes(take(bytes, 8)?.try_into().ok()?);
            (value > u32::MAX as u64).then_some(value)
        }
    }
}
#[derive(Eq, PartialEq)]
struct BtcInput {
    txid: [u8; 32],
    vout: u32,
    sequence: u32,
}
#[derive(Eq, PartialEq)]
struct BtcOutput {
    value: u64,
    script: Vec<u8>,
}
struct BtcTemplate {
    locktime: u32,
    inputs: Vec<BtcInput>,
    outputs: Vec<BtcOutput>,
}
fn psbt_map(bytes: &mut &[u8]) -> Option<Vec<(Vec<u8>, Vec<u8>)>> {
    let mut entries = Vec::new();
    let mut previous: Option<Vec<u8>> = None;
    loop {
        let key_len = read_compact(bytes)? as usize;
        if key_len == 0 {
            return Some(entries);
        }
        let key = take(bytes, key_len)?.to_vec();
        if previous.as_ref().is_some_and(|value| value >= &key) {
            return None;
        }
        previous = Some(key.clone());
        let value_len = read_compact(bytes)? as usize;
        let value = take(bytes, value_len)?.to_vec();
        entries.push((key, value));
    }
}
fn parse_psbt_template(unsigned: &[u8]) -> Option<BtcTemplate> {
    let mut bytes = unsigned;
    if take(&mut bytes, 5)? != b"psbt\xff" {
        return None;
    }
    let globals = psbt_map(&mut bytes)?;
    let get = |key: u8| {
        globals
            .iter()
            .find(|(candidate, _)| candidate.as_slice() == [key])
            .map(|(_, value)| value.as_slice())
    };
    if get(0xfb)? != 2u32.to_le_bytes() || get(0x02)? != 2i32.to_le_bytes() || get(0x06)? != [0] {
        return None;
    }
    let locktime = u32::from_le_bytes(get(0x03)?.try_into().ok()?);
    let mut count_bytes = get(0x04)?;
    let input_count = read_compact(&mut count_bytes)? as usize;
    if !count_bytes.is_empty() || input_count > 100 {
        return None;
    }
    let mut count_bytes = get(0x05)?;
    let output_count = read_compact(&mut count_bytes)? as usize;
    if !count_bytes.is_empty() {
        return None;
    }
    let mut inputs = Vec::new();
    for _ in 0..input_count {
        let map = psbt_map(&mut bytes)?;
        let get = |key: u8| {
            map.iter()
                .find(|(candidate, _)| candidate.as_slice() == [key])
                .map(|(_, value)| value.as_slice())
        };
        let txid = get(0x0e)?.try_into().ok()?;
        let vout = u32::from_le_bytes(get(0x0f)?.try_into().ok()?);
        let sequence = u32::from_le_bytes(get(0x10)?.try_into().ok()?);
        if sequence != 0xffff_ffff || get(0x03)? != 1u32.to_le_bytes() || get(0x01).is_none() {
            return None;
        }
        inputs.push(BtcInput {
            txid,
            vout,
            sequence,
        });
    }
    let mut outputs = Vec::new();
    for _ in 0..output_count {
        let map = psbt_map(&mut bytes)?;
        let get = |key: u8| {
            map.iter()
                .find(|(candidate, _)| candidate.as_slice() == [key])
                .map(|(_, value)| value.as_slice())
        };
        outputs.push(BtcOutput {
            value: u64::from_le_bytes(get(0x03)?.try_into().ok()?),
            script: get(0x04)?.to_vec(),
        });
    }
    if !bytes.is_empty() {
        return None;
    }
    Some(BtcTemplate {
        locktime,
        inputs,
        outputs,
    })
}
fn valid_der_signature(value: &[u8]) -> bool {
    if value.len() < 9
        || value.last() != Some(&1)
        || value[0] != 0x30
        || value[1] as usize + 3 != value.len()
    {
        return false;
    }
    let body = &value[2..value.len() - 1];
    if body.first() != Some(&2) || body.len() < 2 {
        return false;
    }
    let rlen = body[1] as usize;
    if body.len() < 2 + rlen + 2 || rlen == 0 {
        return false;
    }
    let r = &body[2..2 + rlen];
    let rest = &body[2 + rlen..];
    if rest.first() != Some(&2) || rest.len() < 2 || rest.len() != 2 + rest[1] as usize {
        return false;
    }
    let s = &rest[2..];
    fn integer(bytes: &[u8]) -> Option<&[u8]> {
        if bytes.is_empty() || bytes[0] & 0x80 != 0 {
            return None;
        }
        if bytes.len() > 1 && bytes[0] == 0 && bytes[1] & 0x80 == 0 {
            return None;
        }
        Some(if bytes[0] == 0 { &bytes[1..] } else { bytes })
    }
    let Some(r) = integer(r) else { return false };
    let Some(s) = integer(s) else { return false };
    valid_secp_scalar(r, false) && valid_secp_scalar(s, true)
}
fn verify_bitcoin_signed(unsigned: &[u8], signed: &[u8]) -> bool {
    let Some(template) = parse_psbt_template(unsigned) else {
        return false;
    };
    let mut bytes = signed;
    if take(&mut bytes, 4) != Some(&2i32.to_le_bytes()) || take(&mut bytes, 2) != Some(&[0, 1]) {
        return false;
    }
    let Some(input_count) = read_compact(&mut bytes).and_then(|value| usize::try_from(value).ok())
    else {
        return false;
    };
    if input_count != template.inputs.len() {
        return false;
    }
    for expected in &template.inputs {
        let Some(txid) = take(&mut bytes, 32) else {
            return false;
        };
        let Some(vout) = take(&mut bytes, 4)
            .and_then(|value| value.try_into().ok())
            .map(u32::from_le_bytes)
        else {
            return false;
        };
        let Some(script_len) =
            read_compact(&mut bytes).and_then(|value| usize::try_from(value).ok())
        else {
            return false;
        };
        if script_len != 0 || take(&mut bytes, script_len).is_none() {
            return false;
        }
        let Some(sequence) = take(&mut bytes, 4)
            .and_then(|value| value.try_into().ok())
            .map(u32::from_le_bytes)
        else {
            return false;
        };
        if txid != expected.txid || vout != expected.vout || sequence != expected.sequence {
            return false;
        }
    }
    let Some(output_count) = read_compact(&mut bytes).and_then(|value| usize::try_from(value).ok())
    else {
        return false;
    };
    if output_count != template.outputs.len() {
        return false;
    }
    for expected in &template.outputs {
        let Some(value) = take(&mut bytes, 8)
            .and_then(|value| value.try_into().ok())
            .map(u64::from_le_bytes)
        else {
            return false;
        };
        let Some(script_len) =
            read_compact(&mut bytes).and_then(|value| usize::try_from(value).ok())
        else {
            return false;
        };
        let Some(script) = take(&mut bytes, script_len) else {
            return false;
        };
        if value != expected.value || script != expected.script {
            return false;
        }
    }
    for _ in &template.inputs {
        if read_compact(&mut bytes) != Some(2) {
            return false;
        }
        let Some(sig_len) = read_compact(&mut bytes).and_then(|value| usize::try_from(value).ok())
        else {
            return false;
        };
        let Some(signature) = take(&mut bytes, sig_len) else {
            return false;
        };
        if !valid_der_signature(signature) {
            return false;
        }
        if read_compact(&mut bytes) != Some(33) {
            return false;
        }
        let Some(pubkey) = take(&mut bytes, 33) else {
            return false;
        };
        if !matches!(pubkey.first(), Some(2 | 3)) {
            return false;
        }
    }
    take(&mut bytes, 4) == Some(&template.locktime.to_le_bytes()) && bytes.is_empty()
}
fn verify_signed(rail: EconomicRail, unsigned: &[u8], signed: &[u8]) -> Result<(), Diagnostic> {
    let valid = match rail {
        EconomicRail::Solana => {
            signed.len() == 1 + 64 + unsigned.len()
                && signed.first() == Some(&1)
                && signed[1..65].iter().any(|byte| *byte != 0)
                && &signed[65..] == unsigned
        }
        EconomicRail::Evm => verify_evm_signed(unsigned, signed),
        EconomicRail::Bitcoin => verify_bitcoin_signed(unsigned, signed),
    };
    if valid {
        Ok(())
    } else {
        Err(g213())
    }
}

fn keccak_f(state: &mut [u64; 25]) {
    const R: [u32; 25] = [
        0, 1, 62, 28, 27, 36, 44, 6, 55, 20, 3, 10, 43, 25, 39, 41, 45, 15, 21, 8, 18, 2, 61, 56,
        14,
    ];
    const RC: [u64; 24] = [
        0x0000000000000001,
        0x0000000000008082,
        0x800000000000808a,
        0x8000000080008000,
        0x000000000000808b,
        0x0000000080000001,
        0x8000000080008081,
        0x8000000000008009,
        0x000000000000008a,
        0x0000000000000088,
        0x0000000080008009,
        0x000000008000000a,
        0x000000008000808b,
        0x800000000000008b,
        0x8000000000008089,
        0x8000000000008003,
        0x8000000000008002,
        0x8000000000000080,
        0x000000000000800a,
        0x800000008000000a,
        0x8000000080008081,
        0x8000000000008080,
        0x0000000080000001,
        0x8000000080008008,
    ];
    for rc in RC {
        let mut c = [0u64; 5];
        for x in 0..5 {
            c[x] = state[x] ^ state[x + 5] ^ state[x + 10] ^ state[x + 15] ^ state[x + 20];
        }
        let mut d = [0u64; 5];
        for x in 0..5 {
            d[x] = c[(x + 4) % 5] ^ c[(x + 1) % 5].rotate_left(1);
        }
        for y in 0..5 {
            for x in 0..5 {
                state[x + 5 * y] ^= d[x];
            }
        }
        let mut b = [0u64; 25];
        for y in 0..5 {
            for x in 0..5 {
                b[y % 5 + 5 * ((2 * x + 3 * y) % 5)] = state[x + 5 * y].rotate_left(R[x + 5 * y]);
            }
        }
        for y in 0..5 {
            for x in 0..5 {
                state[x + 5 * y] =
                    b[x + 5 * y] ^ ((!b[(x + 1) % 5 + 5 * y]) & b[(x + 2) % 5 + 5 * y]);
            }
        }
        state[0] ^= rc;
    }
}
fn keccak256(bytes: &[u8]) -> [u8; 32] {
    let mut state = [0u64; 25];
    let mut chunks = bytes.chunks_exact(136);
    for chunk in &mut chunks {
        for (index, word) in chunk.chunks_exact(8).enumerate() {
            state[index] ^= u64::from_le_bytes(word.try_into().unwrap_or([0; 8]));
        }
        keccak_f(&mut state);
    }
    let remainder = chunks.remainder();
    let mut block = [0u8; 136];
    block[..remainder.len()].copy_from_slice(remainder);
    block[remainder.len()] = 0x01;
    block[135] |= 0x80;
    for (index, word) in block.chunks_exact(8).enumerate() {
        state[index] ^= u64::from_le_bytes(word.try_into().unwrap_or([0; 8]));
    }
    keccak_f(&mut state);
    let mut output = [0u8; 32];
    for (index, word) in state[..4].iter().enumerate() {
        output[index * 8..index * 8 + 8].copy_from_slice(&word.to_le_bytes());
    }
    output
}
fn bitcoin_stripped(signed: &[u8]) -> Option<Vec<u8>> {
    let mut bytes = signed;
    let version = take(&mut bytes, 4)?;
    if take(&mut bytes, 2)? != [0, 1] {
        return None;
    }
    let input_count = read_compact(&mut bytes)?;
    let mut stripped = version.to_vec();
    stripped.extend(compact_size(input_count.try_into().ok()?));
    for _ in 0..input_count {
        let txid = take(&mut bytes, 32)?;
        let vout = take(&mut bytes, 4)?;
        let script_len = read_compact(&mut bytes)?;
        let script = take(&mut bytes, script_len.try_into().ok()?)?;
        let sequence = take(&mut bytes, 4)?;
        stripped.extend(txid);
        stripped.extend(vout);
        stripped.extend(compact_size(script.len()));
        stripped.extend(script);
        stripped.extend(sequence);
    }
    let output_count = read_compact(&mut bytes)?;
    stripped.extend(compact_size(output_count.try_into().ok()?));
    for _ in 0..output_count {
        let value = take(&mut bytes, 8)?;
        let script_len = read_compact(&mut bytes)?;
        let script = take(&mut bytes, script_len.try_into().ok()?)?;
        stripped.extend(value);
        stripped.extend(compact_size(script.len()));
        stripped.extend(script);
    }
    for _ in 0..input_count {
        let items = read_compact(&mut bytes)?;
        for _ in 0..items {
            let len = read_compact(&mut bytes)?;
            take(&mut bytes, len.try_into().ok()?)?;
        }
    }
    let locktime = take(&mut bytes, 4)?;
    if !bytes.is_empty() {
        return None;
    }
    stripped.extend(locktime);
    Some(stripped)
}
fn transaction_id(rail: EconomicRail, signed: &[u8]) -> Option<String> {
    match rail {
        EconomicRail::Evm => Some(format!(
            "0x{}",
            keccak256(signed)
                .iter()
                .map(|byte| format!("{byte:02x}"))
                .collect::<String>()
        )),
        EconomicRail::Solana => {
            (signed.len() >= 65 && signed[0] == 1).then(|| encode_base58(&signed[1..65]))
        }
        EconomicRail::Bitcoin => {
            let stripped = bitcoin_stripped(signed)?;
            let first = Sha256::digest(&stripped);
            let second = Sha256::digest(first);
            Some(
                second
                    .iter()
                    .rev()
                    .map(|byte| format!("{byte:02x}"))
                    .collect(),
            )
        }
    }
}

fn compact_size(value: usize) -> Vec<u8> {
    if value < 0xfd {
        vec![value as u8]
    } else if value <= 0xffff {
        let mut out = vec![0xfd];
        out.extend_from_slice(&(value as u16).to_le_bytes());
        out
    } else {
        let mut out = vec![0xfe];
        out.extend_from_slice(&(value as u32).to_le_bytes());
        out
    }
}

fn verify_unsigned(intent: &Intent, snapshot: &Snapshot, bytes: &[u8]) -> Result<(), Diagnostic> {
    let (expected, _) = build_unsigned(intent, snapshot)?;
    if expected == bytes {
        Ok(())
    } else {
        Err(g213())
    }
}

fn doc_ref(schema: &str, doc: &Doc) -> String {
    format!(
        "{{\"schema\":{},\"digest\":{},\"bytes\":{}}}",
        quote_json(schema),
        quote_json(&doc.digest),
        doc.source.len()
    )
}
fn agent_ref(run_id: &str, evidence: &str, digest_value: &str) -> String {
    format!("{{\"schema\":\"semaprax.agent-runtime-evidence.v1\",\"digest\":{},\"bytes\":{},\"run_id\":{}}}",quote_json(digest_value),evidence.len(),quote_json(run_id))
}
fn ref_matches(value: &Value, schema: &str, doc: &Doc) -> bool {
    let Some(row) = value.as_object() else {
        return false;
    };
    keys(row, &["schema", "digest", "bytes"])
        && row.get("schema").and_then(Value::as_str) == Some(schema)
        && row.get("digest").and_then(Value::as_str) == Some(doc.digest.as_str())
        && row.get("bytes").and_then(Value::as_u64) == u64::try_from(doc.source.len()).ok()
}
fn ref_identity_matches(value: &Value, schema: &str, digest_value: &str, bytes: usize) -> bool {
    let Some(row) = value.as_object() else {
        return false;
    };
    keys(row, &["schema", "digest", "bytes"])
        && row.get("schema").and_then(Value::as_str) == Some(schema)
        && row.get("digest").and_then(Value::as_str) == Some(digest_value)
        && row.get("bytes").and_then(Value::as_u64) == u64::try_from(bytes).ok()
}
fn unsigned_ref(bytes: &[u8], format: &str) -> String {
    format!(
        "{{\"digest\":{},\"bytes\":{},\"format\":{}}}",
        quote_json(&digest(UNSIGNED_DOMAIN, bytes)),
        bytes.len(),
        quote_json(format)
    )
}

#[derive(Clone)]
struct Invoice {
    origin: String,
    method: String,
    resource: String,
    invoice_id: String,
    payee: String,
    rail: EconomicRail,
    network: String,
    asset: String,
    amount: u64,
    max_fee: u64,
    expires: u64,
    nonce: String,
    idempotency: String,
    doc: Doc,
}
fn render_invoice(i: &Invoice) -> String {
    format!("{{\"schema\":\"{INVOICE_SCHEMA}\",\"origin\":{},\"method\":{},\"resource\":{},\"invoice_id\":{},\"payee\":{},\"settlement_rail\":{},\"network\":{},\"asset\":{},\"amount_atomic\":{},\"max_fee_atomic\":{},\"expires_at_ms\":{},\"nonce\":{},\"idempotency_key\":{}}}\n",quote_json(&i.origin),quote_json(&i.method),quote_json(&i.resource),quote_json(&i.invoice_id),quote_json(&i.payee),quote_json(i.rail.text()),quote_json(&i.network),quote_json(&i.asset),i.amount,i.max_fee,i.expires,quote_json(&i.nonce),quote_json(&i.idempotency))
}
fn parse_invoice(source: &str, intent: &Intent) -> Result<Invoice, Diagnostic> {
    let (_, value) = canonical(source, "x402 invoice", INVOICE_SCHEMA, MAX_INVOICE_BYTES)?;
    let row = object(&value, "x402 invoice", INVOICE_SCHEMA)?;
    if !keys(
        row,
        &[
            "schema",
            "origin",
            "method",
            "resource",
            "invoice_id",
            "payee",
            "settlement_rail",
            "network",
            "asset",
            "amount_atomic",
            "max_fee_atomic",
            "expires_at_ms",
            "nonce",
            "idempotency_key",
        ],
    ) {
        return Err(g210("x402 invoice", INVOICE_SCHEMA));
    }
    let i = Invoice {
        origin: text(row, "origin", "x402 invoice", INVOICE_SCHEMA)?.to_owned(),
        method: text(row, "method", "x402 invoice", INVOICE_SCHEMA)?.to_owned(),
        resource: text(row, "resource", "x402 invoice", INVOICE_SCHEMA)?.to_owned(),
        invoice_id: text(row, "invoice_id", "x402 invoice", INVOICE_SCHEMA)?.to_owned(),
        payee: text(row, "payee", "x402 invoice", INVOICE_SCHEMA)?.to_owned(),
        rail: rail(text(
            row,
            "settlement_rail",
            "x402 invoice",
            INVOICE_SCHEMA,
        )?)
        .ok_or_else(g213)?,
        network: text(row, "network", "x402 invoice", INVOICE_SCHEMA)?.to_owned(),
        asset: text(row, "asset", "x402 invoice", INVOICE_SCHEMA)?.to_owned(),
        amount: number(row, "amount_atomic", "x402 invoice", INVOICE_SCHEMA)?,
        max_fee: number(row, "max_fee_atomic", "x402 invoice", INVOICE_SCHEMA)?,
        expires: number(row, "expires_at_ms", "x402 invoice", INVOICE_SCHEMA)?,
        nonce: text(row, "nonce", "x402 invoice", INVOICE_SCHEMA)?.to_owned(),
        idempotency: text(row, "idempotency_key", "x402 invoice", INVOICE_SCHEMA)?.to_owned(),
        doc: Doc {
            source: source.to_owned(),
            digest: digest(INVOICE_DOMAIN, source.as_bytes()),
        },
    };
    if render_invoice(&i) != source {
        return Err(g210("x402 invoice", INVOICE_SCHEMA));
    }
    if let Payment::X402 {
        origin,
        method,
        resource,
        invoice_digest,
        payee,
        rail,
        network,
        asset,
        amount,
        max_fee,
        invoice_expires,
        nonce,
    } = &intent.payment
    {
        if i.origin != *origin
            || i.method != *method
            || i.resource != *resource
            || i.doc.digest != *invoice_digest
            || i.payee != *payee
            || i.rail != *rail
            || i.network != *network
            || i.asset != *asset
            || i.amount != *amount
            || i.max_fee != *max_fee
            || i.expires != *invoice_expires
            || i.nonce != *nonce
            || i.idempotency != intent.idempotency_key
        {
            return Err(g213());
        }
    } else {
        return Err(g213());
    }
    Ok(i)
}
fn parse_invoice_limited(
    source: &str,
    intent: &Intent,
    limits: &Limits,
) -> Result<Invoice, Diagnostic> {
    configured_document_limits(source, "x402 invoice", limits.max_invoice_bytes, limits)?;
    reserve_parse_sidecar(source, limits)?;
    let invoice = parse_invoice(source, intent)?;
    if [
        invoice.invoice_id.as_str(),
        invoice.nonce.as_str(),
        invoice.idempotency.as_str(),
    ]
    .into_iter()
    .any(|value| value.len() > limits.max_identifier_bytes as usize)
    {
        return Err(g216("identifier_bytes", limits.max_identifier_bytes));
    }
    Ok(invoice)
}

#[derive(Clone)]
struct Plan {
    doc: Doc,
    unsigned: Vec<u8>,
    unsigned_digest: String,
    format: &'static str,
    observed: u64,
    expires: u64,
    utxos: u64,
}
fn make_plan(
    run_id: &str,
    agent_run_id: &str,
    agent_evidence: &str,
    agent_digest: &str,
    policy: &Policy,
    intent: &Intent,
    invoice: Option<&Invoice>,
    snapshot: &Snapshot,
    unsigned: Vec<u8>,
    format: &'static str,
) -> Result<Plan, Diagnostic> {
    if snapshot.observed < intent.created_at
        || snapshot.observed >= intent.expires_at
        || invoice.is_some_and(|value| snapshot.observed >= value.expires)
    {
        return Err(g212("expired"));
    }
    let unsigned_digest = digest(UNSIGNED_DOMAIN, &unsigned);
    let expires = snapshot
        .expires
        .min(intent.expires_at)
        .min(invoice.map_or(u64::MAX, |value| value.expires));
    if expires <= snapshot.observed {
        return Err(g212("expired"));
    }
    let mut count = CountSink::default();
    write_plan(
        &mut count,
        run_id,
        agent_run_id,
        agent_evidence,
        agent_digest,
        policy,
        intent,
        invoice,
        snapshot,
        &unsigned,
        format,
        &unsigned_digest,
        expires,
    )
    .map_err(|_| g217())?;
    if count.0 > policy.limits.max_plan_bytes as usize {
        return Err(g216("plan_bytes", policy.limits.max_plan_bytes));
    }
    let mut source = String::with_capacity(count.0);
    write_plan(
        &mut source,
        run_id,
        agent_run_id,
        agent_evidence,
        agent_digest,
        policy,
        intent,
        invoice,
        snapshot,
        &unsigned,
        format,
        &unsigned_digest,
        expires,
    )
    .map_err(|_| g217())?;
    let doc = Doc {
        digest: digest(PLAN_DOMAIN, source.as_bytes()),
        source,
    };
    let utxos = match &snapshot.state {
        SnapshotState::Bitcoin { utxos, .. } => utxos.len() as u64,
        _ => 0,
    };
    Ok(Plan {
        doc,
        unsigned,
        unsigned_digest,
        format,
        observed: snapshot.observed,
        expires,
        utxos,
    })
}

#[allow(clippy::too_many_arguments)]
fn write_plan<W: fmt::Write>(
    output: &mut W,
    run_id: &str,
    agent_run_id: &str,
    agent_evidence: &str,
    agent_digest: &str,
    policy: &Policy,
    intent: &Intent,
    invoice: Option<&Invoice>,
    snapshot: &Snapshot,
    unsigned: &[u8],
    format: &str,
    unsigned_digest: &str,
    expires: u64,
) -> fmt::Result {
    output.write_str("{\"schema\":")?;
    write_json(output, PLAN_SCHEMA)?;
    output.write_str(",\"run_id\":")?;
    write_json(output, run_id)?;
    output.write_str(
        ",\"source_agent_evidence\":{\"schema\":\"semaprax.agent-runtime-evidence.v1\",\"digest\":",
    )?;
    write_json(output, agent_digest)?;
    write!(output, ",\"bytes\":{},\"run_id\":", agent_evidence.len())?;
    write_json(output, agent_run_id)?;
    output.write_char('}')?;
    output.write_str(",\"policy\":")?;
    write_doc_reference(output, POLICY_SCHEMA, &policy.digest, policy.source.len())?;
    output.write_str(",\"intent\":")?;
    write_doc_reference(output, INTENT_SCHEMA, &intent.digest, intent.source.len())?;
    output.write_str(",\"x402_invoice\":")?;
    write_optional_reference(output, INVOICE_SCHEMA, invoice.map(|v| &v.doc))?;
    output.write_str(",\"chain_snapshot\":")?;
    write_doc_reference(
        output,
        SNAPSHOT_SCHEMA,
        &snapshot.doc.digest,
        snapshot.doc.source.len(),
    )?;
    output.write_str(",\"rail\":")?;
    write_json(output, intent.settlement_rail().text())?;
    let (network, asset) = intent.network_asset();
    output.write_str(",\"network\":")?;
    write_json(output, network)?;
    output.write_str(",\"asset\":")?;
    write_json(output, asset)?;
    output.write_str(",\"wallet_id\":")?;
    write_json(output, &intent.wallet_id)?;
    output.write_str(",\"recipient\":")?;
    write_json(output, intent.recipient())?;
    write!(
        output,
        ",\"amount_atomic\":{},\"max_fee_atomic\":{}",
        intent.amount(),
        intent.max_fee()
    )?;
    output.write_str(",\"unsigned_transaction\":{\"digest\":")?;
    write_json(output, unsigned_digest)?;
    write!(output, ",\"bytes\":{},\"format\":", unsigned.len())?;
    write_json(output, format)?;
    writeln!(output, "}},\"expires_at_ms\":{expires}}}")
}

#[derive(Clone)]
struct Simulation {
    doc: Doc,
    fee: u64,
    expires: u64,
}
fn parse_simulation(source: &str, plan: &Plan, intent: &Intent) -> Result<Simulation, Diagnostic> {
    let (_, value) = canonical(
        source,
        "simulation",
        SIMULATION_SCHEMA,
        MAX_SIMULATION_BYTES,
    )?;
    let row = object(&value, "simulation", SIMULATION_SCHEMA)?;
    if !keys(
        row,
        &[
            "schema",
            "plan",
            "success",
            "fee_atomic",
            "balance_before_atomic",
            "balance_after_atomic",
            "allowance_atomic",
            "units",
            "expires_at_ms",
        ],
    ) {
        return Err(g210("simulation", SIMULATION_SCHEMA));
    }
    if !ref_matches(&row["plan"], PLAN_SCHEMA, &plan.doc) || row["success"].as_bool() != Some(true)
    {
        return Err(g213());
    }
    let fee = number(row, "fee_atomic", "simulation", SIMULATION_SCHEMA)?;
    if fee > intent.max_fee() {
        return Err(g213());
    }
    let before = number(
        row,
        "balance_before_atomic",
        "simulation",
        SIMULATION_SCHEMA,
    )?;
    let after = number(row, "balance_after_atomic", "simulation", SIMULATION_SCHEMA)?;
    if after
        .checked_add(intent.amount())
        .and_then(|value| value.checked_add(fee))
        != Some(before)
    {
        return Err(g213());
    }
    if intent.settlement_rail() == EconomicRail::Evm && row["allowance_atomic"].as_u64() != Some(0)
    {
        return Err(g213());
    }
    if intent.settlement_rail() != EconomicRail::Evm && !row["allowance_atomic"].is_null() {
        return Err(g213());
    }
    let units = number(row, "units", "simulation", SIMULATION_SCHEMA)?;
    if intent.settlement_rail() == EconomicRail::Evm && units != 21_000 {
        return Err(g213());
    }
    if let Payment::Solana { compute, .. } = &intent.payment {
        if units != *compute {
            return Err(g213());
        }
    }
    let expires = number(row, "expires_at_ms", "simulation", SIMULATION_SCHEMA)?;
    if expires <= plan.observed || expires > plan.expires {
        return Err(g213());
    }
    let plan_ref = doc_ref(PLAN_SCHEMA, &plan.doc);
    let canonical_source=format!("{{\"schema\":\"{SIMULATION_SCHEMA}\",\"plan\":{},\"success\":true,\"fee_atomic\":{fee},\"balance_before_atomic\":{before},\"balance_after_atomic\":{after},\"allowance_atomic\":{},\"units\":{units},\"expires_at_ms\":{expires}}}\n",plan_ref,if intent.settlement_rail()==EconomicRail::Evm{"0"}else{"null"});
    if canonical_source != source {
        return Err(g210("simulation", SIMULATION_SCHEMA));
    }
    Ok(Simulation {
        doc: Doc {
            source: source.to_owned(),
            digest: digest(SIMULATION_DOMAIN, source.as_bytes()),
        },
        fee,
        expires,
    })
}
fn parse_simulation_limited(
    source: &str,
    plan: &Plan,
    intent: &Intent,
    limits: &Limits,
) -> Result<Simulation, Diagnostic> {
    configured_document_limits(source, "simulation", limits.max_simulation_bytes, limits)?;
    reserve_parse_sidecar(source, limits)?;
    parse_simulation(source, plan, intent)
}

fn make_approval_request(
    run_id: &str,
    policy: &Policy,
    intent: &Intent,
    plan: &Plan,
    simulation: &Simulation,
) -> Result<Doc, Diagnostic> {
    let mut count = CountSink::default();
    write_approval_request(&mut count, run_id, policy, intent, plan, simulation)
        .map_err(|_| g217())?;
    if count.0 > policy.limits.max_approval_request_bytes as usize {
        return Err(g216(
            "approval_request_bytes",
            policy.limits.max_approval_request_bytes,
        ));
    }
    let mut source = String::with_capacity(count.0);
    write_approval_request(&mut source, run_id, policy, intent, plan, simulation)
        .map_err(|_| g217())?;
    canonical_policy_limited(
        &source,
        "approval request",
        APPROVAL_REQUEST_SCHEMA,
        policy.limits.max_approval_request_bytes,
        policy.limits.max_json_depth,
    )?;
    Ok(Doc {
        digest: digest(APPROVAL_REQUEST_DOMAIN, source.as_bytes()),
        source,
    })
}
fn write_approval_request<W: fmt::Write>(
    output: &mut W,
    run_id: &str,
    policy: &Policy,
    intent: &Intent,
    plan: &Plan,
    simulation: &Simulation,
) -> fmt::Result {
    output.write_str("{\"schema\":")?;
    write_json(output, APPROVAL_REQUEST_SCHEMA)?;
    output.write_str(",\"run_id\":")?;
    write_json(output, run_id)?;
    output.write_str(",\"wallet_id\":")?;
    write_json(output, &intent.wallet_id)?;
    output.write_str(",\"rail\":")?;
    write_json(output, intent.settlement_rail().text())?;
    let (network, asset) = intent.network_asset();
    output.write_str(",\"network\":")?;
    write_json(output, network)?;
    output.write_str(",\"asset\":")?;
    write_json(output, asset)?;
    output.write_str(",\"recipient\":")?;
    write_json(output, intent.recipient())?;
    write!(
        output,
        ",\"amount_atomic\":{},\"max_fee_atomic\":{}",
        intent.amount(),
        intent.max_fee()
    )?;
    let x402 = match &intent.payment {
        Payment::X402 {
            origin,
            method,
            resource,
            ..
        } => Some((origin.as_str(), method.as_str(), resource.as_str())),
        _ => None,
    };
    output.write_str(",\"origin\":")?;
    write_optional_json(output, x402.map(|v| v.0))?;
    output.write_str(",\"method\":")?;
    write_optional_json(output, x402.map(|v| v.1))?;
    output.write_str(",\"resource\":")?;
    write_optional_json(output, x402.map(|v| v.2))?;
    output.write_str(",\"policy\":")?;
    write_doc_reference(output, POLICY_SCHEMA, &policy.digest, policy.source.len())?;
    output.write_str(",\"intent\":")?;
    write_doc_reference(output, INTENT_SCHEMA, &intent.digest, intent.source.len())?;
    output.write_str(",\"plan\":")?;
    write_doc_reference(output, PLAN_SCHEMA, &plan.doc.digest, plan.doc.source.len())?;
    output.write_str(",\"simulation\":")?;
    write_doc_reference(
        output,
        SIMULATION_SCHEMA,
        &simulation.doc.digest,
        simulation.doc.source.len(),
    )?;
    writeln!(output, ",\"expires_at_ms\":{}}}", simulation.expires)
}

#[derive(Clone)]
struct Approval {
    doc: Doc,
}
fn parse_approval(
    source: &str,
    policy: &Policy,
    intent: &Intent,
    plan: &Plan,
    simulation: &Simulation,
    request: &Doc,
) -> Result<Approval, Diagnostic> {
    let (_, value) = canonical(source, "approval", APPROVAL_SCHEMA, MAX_APPROVAL_BYTES)?;
    let row = object(&value, "approval", APPROVAL_SCHEMA)?;
    if !keys(
        row,
        &[
            "schema",
            "approval_id",
            "approver_id",
            "policy",
            "intent",
            "plan",
            "simulation",
            "approval_request",
            "decision",
            "approved_amount_atomic",
            "approved_fee_atomic",
            "expires_at_ms",
        ],
    ) {
        return Err(g210("approval", APPROVAL_SCHEMA));
    }
    let approval_expires = number(row, "expires_at_ms", "approval", APPROVAL_SCHEMA)?;
    let approval_id = text(row, "approval_id", "approval", APPROVAL_SCHEMA)?;
    let approver_id = text(row, "approver_id", "approval", APPROVAL_SCHEMA)?;
    if approval_id.len() > policy.limits.max_identifier_bytes as usize
        || approver_id.len() > policy.limits.max_identifier_bytes as usize
    {
        return Err(g216("identifier_bytes", policy.limits.max_identifier_bytes));
    }
    if !identifier(approval_id)
        || !identifier(approver_id)
        || text(row, "decision", "approval", APPROVAL_SCHEMA)? != "approved"
        || number(row, "approved_amount_atomic", "approval", APPROVAL_SCHEMA)? != intent.amount()
        || number(row, "approved_fee_atomic", "approval", APPROVAL_SCHEMA)? != intent.max_fee()
        || approval_expires <= plan.observed
        || approval_expires > simulation.expires
    {
        return Err(g214());
    }
    let refs = [
        (
            "policy",
            POLICY_SCHEMA,
            Doc {
                source: policy.source.clone(),
                digest: policy.digest.clone(),
            },
        ),
        (
            "intent",
            INTENT_SCHEMA,
            Doc {
                source: intent.source.clone(),
                digest: intent.digest.clone(),
            },
        ),
        ("plan", PLAN_SCHEMA, plan.doc.clone()),
        ("simulation", SIMULATION_SCHEMA, simulation.doc.clone()),
        ("approval_request", APPROVAL_REQUEST_SCHEMA, request.clone()),
    ];
    if refs
        .iter()
        .any(|(key, schema, doc)| !ref_matches(&row[*key], schema, doc))
    {
        return Err(g214());
    }
    let canonical_source=format!("{{\"schema\":\"{APPROVAL_SCHEMA}\",\"approval_id\":{},\"approver_id\":{},\"policy\":{},\"intent\":{},\"plan\":{},\"simulation\":{},\"approval_request\":{},\"decision\":\"approved\",\"approved_amount_atomic\":{},\"approved_fee_atomic\":{},\"expires_at_ms\":{}}}\n",quote_json(text(row,"approval_id","approval",APPROVAL_SCHEMA)?),quote_json(text(row,"approver_id","approval",APPROVAL_SCHEMA)?),doc_ref(POLICY_SCHEMA,&Doc{source:policy.source.clone(),digest:policy.digest.clone()}),doc_ref(INTENT_SCHEMA,&Doc{source:intent.source.clone(),digest:intent.digest.clone()}),doc_ref(PLAN_SCHEMA,&plan.doc),doc_ref(SIMULATION_SCHEMA,&simulation.doc),doc_ref(APPROVAL_REQUEST_SCHEMA,request),intent.amount(),intent.max_fee(),number(row,"expires_at_ms","approval",APPROVAL_SCHEMA)?);
    if canonical_source != source {
        return Err(g210("approval", APPROVAL_SCHEMA));
    }
    Ok(Approval {
        doc: Doc {
            source: source.to_owned(),
            digest: digest(APPROVAL_DOMAIN, source.as_bytes()),
        },
    })
}
fn parse_approval_limited(
    source: &str,
    policy: &Policy,
    intent: &Intent,
    plan: &Plan,
    simulation: &Simulation,
    request: &Doc,
) -> Result<Approval, Diagnostic> {
    configured_document_limits(
        source,
        "approval",
        policy.limits.max_approval_bytes,
        &policy.limits,
    )?;
    reserve_parse_sidecar(source, &policy.limits)?;
    parse_approval(source, policy, intent, plan, simulation, request)
}
fn approval_expires(approval: &Approval) -> u64 {
    serde_json::from_str::<Value>(approval.doc.source.trim_end())
        .ok()
        .and_then(|value| value.get("expires_at_ms").and_then(Value::as_u64))
        .unwrap_or(0)
}

#[derive(Clone, Copy, Eq, PartialEq)]
enum JournalState {
    Reserved,
    Prepared,
    Approved,
    Signed,
    BroadcastUnknown,
    Broadcasted,
    Pending,
    Confirmed,
    Reorged,
    Dropped,
    Rejected,
    Cancelled,
    Failed,
}

impl JournalState {
    fn text(self) -> &'static str {
        match self {
            Self::Reserved => "reserved",
            Self::Prepared => "prepared",
            Self::Approved => "approved",
            Self::Signed => "signed",
            Self::BroadcastUnknown => "broadcast_unknown",
            Self::Broadcasted => "broadcasted",
            Self::Pending => "pending",
            Self::Confirmed => "confirmed",
            Self::Reorged => "reorged",
            Self::Dropped => "dropped",
            Self::Rejected => "rejected",
            Self::Cancelled => "cancelled",
            Self::Failed => "failed",
        }
    }
    fn parse(value: &str) -> Option<Self> {
        Some(match value {
            "reserved" => Self::Reserved,
            "prepared" => Self::Prepared,
            "approved" => Self::Approved,
            "signed" => Self::Signed,
            "broadcast_unknown" => Self::BroadcastUnknown,
            "broadcasted" => Self::Broadcasted,
            "pending" => Self::Pending,
            "confirmed" => Self::Confirmed,
            "reorged" => Self::Reorged,
            "dropped" => Self::Dropped,
            "rejected" => Self::Rejected,
            "cancelled" => Self::Cancelled,
            "failed" => Self::Failed,
            _ => return None,
        })
    }
}

#[derive(Clone)]
struct Journal {
    idempotency_key: String,
    version: u64,
    policy: Doc,
    intent: Doc,
    run_id: String,
    state: JournalState,
    reserved_amount: u64,
    reserved_fee: u64,
    plan: Option<DocRef>,
    simulation: Option<DocRef>,
    approval: Option<DocRef>,
    unsigned: Option<(String, usize, &'static str)>,
    signed: Option<(String, usize)>,
    broadcast: Option<Doc>,
    reconciliation: Option<Doc>,
    updated_at: u64,
}
fn journal_owned_bytes(journal: &Journal) -> Result<usize, Diagnostic> {
    let mut total = 0usize;
    let mut add = |value: usize| -> Result<(), Diagnostic> {
        total = total.checked_add(value).ok_or_else(g217)?;
        Ok(())
    };
    for value in [
        journal.idempotency_key.len(),
        journal.policy.source.len(),
        journal.policy.digest.len(),
        journal.intent.source.len(),
        journal.intent.digest.len(),
        journal.run_id.len(),
    ] {
        add(value)?;
    }
    for value in [&journal.plan, &journal.simulation, &journal.approval]
        .into_iter()
        .flatten()
    {
        add(value.digest.len())?;
    }
    if let Some((digest, _, _)) = &journal.unsigned {
        add(digest.len())?;
    }
    if let Some((digest, _)) = &journal.signed {
        add(digest.len())?;
    }
    for value in [&journal.broadcast, &journal.reconciliation]
        .into_iter()
        .flatten()
    {
        add(value.source.len())?;
        add(value.digest.len())?;
    }
    Ok(total)
}
fn clone_journal_bounded(journal: &Journal, builder_max: u64) -> Result<Journal, Diagnostic> {
    let bytes = journal_owned_bytes(journal)?;
    if active_remaining().is_some_and(|remaining| bytes > remaining) || !reserve_active(bytes) {
        return Err(g216("builder_bytes", builder_max));
    }
    Ok(journal.clone())
}

fn optional_ref(schema: &str, doc: Option<&Doc>) -> String {
    doc.map_or_else(|| "null".to_owned(), |value| doc_ref(schema, value))
}
fn optional_typed_ref(schema: &str, value: Option<&DocRef>) -> String {
    value.map_or_else(
        || "null".to_owned(),
        |value| {
            format!(
                "{{\"schema\":{},\"digest\":{},\"bytes\":{}}}",
                quote_json(schema),
                quote_json(&value.digest),
                value.bytes
            )
        },
    )
}
fn optional_capsule(schema: &str, doc: Option<&Doc>) -> String {
    doc.map_or_else(
        || "null".to_owned(),
        |value| {
            format!(
                "{{\"schema\":{},\"digest\":{},\"bytes\":{},\"document\":{}}}",
                quote_json(schema),
                quote_json(&value.digest),
                value.source.len(),
                quote_json(&value.source)
            )
        },
    )
}
fn optional_unsigned(value: Option<&(String, usize, &'static str)>) -> String {
    value.map_or_else(
        || "null".to_owned(),
        |(digest_value, bytes, format)| {
            format!(
                "{{\"digest\":{},\"bytes\":{bytes},\"format\":{}}}",
                quote_json(digest_value),
                quote_json(format)
            )
        },
    )
}
fn optional_signed(value: Option<&(String, usize)>) -> String {
    value.map_or_else(
        || "null".to_owned(),
        |(digest_value, bytes)| {
            format!(
                "{{\"digest\":{},\"bytes\":{bytes}}}",
                quote_json(digest_value)
            )
        },
    )
}
fn render_journal(journal: &Journal) -> String {
    let mut count = CountSink::default();
    write_journal(&mut count, journal).expect("journal count cannot fail");
    let mut output = String::with_capacity(count.0);
    write_journal(&mut output, journal).expect("String writes cannot fail");
    output
}
fn write_optional_journal_ref<W: fmt::Write>(
    output: &mut W,
    schema: &str,
    value: Option<&DocRef>,
) -> fmt::Result {
    match value {
        Some(value) => write_doc_reference(output, schema, &value.digest, value.bytes as usize),
        None => output.write_str("null"),
    }
}
fn write_capsule<W: fmt::Write>(output: &mut W, schema: &str, value: Option<&Doc>) -> fmt::Result {
    match value {
        Some(value) => {
            output.write_str("{\"schema\":")?;
            write_json(output, schema)?;
            output.write_str(",\"digest\":")?;
            write_json(output, &value.digest)?;
            write!(output, ",\"bytes\":{},\"document\":", value.source.len())?;
            write_json(output, &value.source)?;
            output.write_char('}')
        }
        None => output.write_str("null"),
    }
}
fn write_journal<W: fmt::Write>(output: &mut W, journal: &Journal) -> fmt::Result {
    output.write_str("{\"schema\":\"")?;
    output.write_str(JOURNAL_SCHEMA)?;
    output.write_str("\",\"idempotency_key\":")?;
    write_json(output, &journal.idempotency_key)?;
    write!(output, ",\"version\":{},\"policy\":", journal.version)?;
    write_doc_reference(
        output,
        POLICY_SCHEMA,
        &journal.policy.digest,
        journal.policy.source.len(),
    )?;
    output.write_str(",\"intent\":")?;
    write_doc_reference(
        output,
        INTENT_SCHEMA,
        &journal.intent.digest,
        journal.intent.source.len(),
    )?;
    output.write_str(",\"run_id\":")?;
    write_json(output, &journal.run_id)?;
    output.write_str(",\"state\":")?;
    write_json(output, journal.state.text())?;
    write!(
        output,
        ",\"reserved_amount_atomic\":{},\"reserved_fee_atomic\":{},\"plan\":",
        journal.reserved_amount, journal.reserved_fee
    )?;
    write_optional_journal_ref(output, PLAN_SCHEMA, journal.plan.as_ref())?;
    output.write_str(",\"simulation\":")?;
    write_optional_journal_ref(output, SIMULATION_SCHEMA, journal.simulation.as_ref())?;
    output.write_str(",\"approval\":")?;
    write_optional_journal_ref(output, APPROVAL_SCHEMA, journal.approval.as_ref())?;
    output.write_str(",\"unsigned_transaction\":")?;
    match journal.unsigned.as_ref() {
        Some((digest_value, bytes, format)) => {
            output.write_str("{\"digest\":")?;
            write_json(output, digest_value)?;
            write!(output, ",\"bytes\":{bytes},\"format\":")?;
            write_json(output, format)?;
            output.write_char('}')?;
        }
        None => output.write_str("null")?,
    }
    output.write_str(",\"signed_transaction\":")?;
    match journal.signed.as_ref() {
        Some((digest_value, bytes)) => {
            output.write_str("{\"digest\":")?;
            write_json(output, digest_value)?;
            write!(output, ",\"bytes\":{bytes}}}")?;
        }
        None => output.write_str("null")?,
    }
    output.write_str(",\"broadcast\":")?;
    write_capsule(output, BROADCAST_SCHEMA, journal.broadcast.as_ref())?;
    output.write_str(",\"reconciliation\":")?;
    write_capsule(
        output,
        RECONCILIATION_SCHEMA,
        journal.reconciliation.as_ref(),
    )?;
    writeln!(output, ",\"updated_at_ms\":{}}}", journal.updated_at)
}

#[derive(Clone)]
struct BroadcastReceipt {
    doc: Doc,
    transaction_id: String,
    disposition: &'static str,
    observed: u64,
}
fn parse_broadcast(
    source: &str,
    rail: EconomicRail,
    network: &str,
    signed_digest: &str,
    expected_transaction_id: Option<&str>,
) -> Result<BroadcastReceipt, Diagnostic> {
    parse_broadcast_mode(
        source,
        rail,
        network,
        signed_digest,
        expected_transaction_id,
        false,
    )
}
fn parse_broadcast_limited(
    source: &str,
    rail: EconomicRail,
    network: &str,
    signed_digest: &str,
    expected_transaction_id: Option<&str>,
    limits: &Limits,
) -> Result<BroadcastReceipt, Diagnostic> {
    configured_document_limits(
        source,
        "broadcast receipt",
        limits.max_broadcast_receipt_bytes,
        limits,
    )?;
    reserve_parse_sidecar(source, limits)?;
    parse_broadcast(
        source,
        rail,
        network,
        signed_digest,
        expected_transaction_id,
    )
}
fn parse_provisional_broadcast(
    source: &str,
    rail: EconomicRail,
    network: &str,
    signed_digest: &str,
    expected_transaction_id: &str,
) -> Result<BroadcastReceipt, Diagnostic> {
    parse_broadcast_mode(
        source,
        rail,
        network,
        signed_digest,
        Some(expected_transaction_id),
        true,
    )
}
fn parse_broadcast_mode(
    source: &str,
    rail: EconomicRail,
    network: &str,
    signed_digest: &str,
    expected_transaction_id: Option<&str>,
    provisional: bool,
) -> Result<BroadcastReceipt, Diagnostic> {
    let (_, value) = canonical(
        source,
        "broadcast receipt",
        BROADCAST_SCHEMA,
        MAX_BROADCAST_BYTES,
    )?;
    let row = object(&value, "broadcast receipt", BROADCAST_SCHEMA)?;
    if !keys(
        row,
        &[
            "schema",
            "rail",
            "network",
            "signed_transaction_digest",
            "transaction_id",
            "disposition",
            "observed_at_ms",
        ],
    ) || text(row, "rail", "broadcast receipt", BROADCAST_SCHEMA)? != rail.text()
        || text(row, "network", "broadcast receipt", BROADCAST_SCHEMA)? != network
        || text(
            row,
            "signed_transaction_digest",
            "broadcast receipt",
            BROADCAST_SCHEMA,
        )? != signed_digest
    {
        return Err(g213());
    }
    let transaction_id =
        text(row, "transaction_id", "broadcast receipt", BROADCAST_SCHEMA)?.to_owned();
    if expected_transaction_id.is_some_and(|expected| expected != transaction_id) {
        return Err(g213());
    }
    let disposition = match text(row, "disposition", "broadcast receipt", BROADCAST_SCHEMA)? {
        "accepted" => "accepted",
        "pending" => "pending",
        "unknown" => "unknown",
        "rejected" => "rejected",
        _ => return Err(g210("broadcast receipt", BROADCAST_SCHEMA)),
    };
    let observed = number(row, "observed_at_ms", "broadcast receipt", BROADCAST_SCHEMA)?;
    if provisional {
        if disposition != "unknown" || observed != 0 {
            return Err(g213());
        }
    } else if observed == 0 {
        return Err(g213());
    }
    let canonical_source=format!("{{\"schema\":\"{BROADCAST_SCHEMA}\",\"rail\":{},\"network\":{},\"signed_transaction_digest\":{},\"transaction_id\":{},\"disposition\":{},\"observed_at_ms\":{observed}}}\n",quote_json(rail.text()),quote_json(network),quote_json(signed_digest),quote_json(&transaction_id),quote_json(disposition));
    if canonical_source != source {
        return Err(g210("broadcast receipt", BROADCAST_SCHEMA));
    }
    Ok(BroadcastReceipt {
        doc: Doc {
            source: source.to_owned(),
            digest: digest(BROADCAST_DOMAIN, source.as_bytes()),
        },
        transaction_id,
        disposition,
        observed,
    })
}

#[derive(Clone)]
struct Reconciliation {
    doc: Doc,
    status: &'static str,
    transaction_id: String,
    observed: u64,
    confirmations: Option<u64>,
}
fn nullable_u64(value: &Value) -> Option<Option<u64>> {
    if value.is_null() {
        Some(None)
    } else {
        value.as_u64().map(Some)
    }
}
fn nullable_text(value: &Value) -> Option<Option<String>> {
    if value.is_null() {
        Some(None)
    } else {
        value.as_str().map(|text| Some(text.to_owned()))
    }
}
fn parse_reconciliation(
    source: &str,
    rail: EconomicRail,
    network: &str,
    transaction_id: &str,
) -> Result<Reconciliation, Diagnostic> {
    parse_reconciliation_with_identifier_limit(
        source,
        rail,
        network,
        transaction_id,
        MAX_IDENTIFIER_BYTES as u64,
    )
}

fn parse_reconciliation_with_identifier_limit(
    source: &str,
    rail: EconomicRail,
    network: &str,
    transaction_id: &str,
    max_identifier_bytes: u64,
) -> Result<Reconciliation, Diagnostic> {
    let (_, value) = canonical(
        source,
        "reconciliation",
        RECONCILIATION_SCHEMA,
        MAX_RECONCILIATION_BYTES,
    )?;
    let row = object(&value, "reconciliation", RECONCILIATION_SCHEMA)?;
    if !keys(
        row,
        &[
            "schema",
            "rail",
            "network",
            "transaction_id",
            "status",
            "observed_at_ms",
            "observed_height",
            "confirmations",
            "canonical_block_id",
        ],
    ) || text(row, "rail", "reconciliation", RECONCILIATION_SCHEMA)? != rail.text()
        || text(row, "network", "reconciliation", RECONCILIATION_SCHEMA)? != network
        || text(
            row,
            "transaction_id",
            "reconciliation",
            RECONCILIATION_SCHEMA,
        )? != transaction_id
    {
        return Err(g215());
    }
    let status = match text(row, "status", "reconciliation", RECONCILIATION_SCHEMA)? {
        "pending" => "pending",
        "confirmed" => "confirmed",
        "reorged" => "reorged",
        "dropped" => "dropped",
        _ => return Err(g210("reconciliation", RECONCILIATION_SCHEMA)),
    };
    let observed = number(
        row,
        "observed_at_ms",
        "reconciliation",
        RECONCILIATION_SCHEMA,
    )?;
    let height = nullable_u64(&row["observed_height"])
        .ok_or_else(|| g210("reconciliation", RECONCILIATION_SCHEMA))?;
    let confirmations = nullable_u64(&row["confirmations"])
        .ok_or_else(|| g210("reconciliation", RECONCILIATION_SCHEMA))?;
    let block = nullable_text(&row["canonical_block_id"])
        .ok_or_else(|| g210("reconciliation", RECONCILIATION_SCHEMA))?;
    if transaction_id.len() > max_identifier_bytes as usize
        || block
            .as_deref()
            .is_some_and(|value| value.len() > max_identifier_bytes as usize)
    {
        return Err(g216("identifier_bytes", max_identifier_bytes));
    }
    if status == "confirmed" && (height.is_none() || confirmations.is_none() || block.is_none()) {
        return Err(g215());
    }
    let canonical_source=format!("{{\"schema\":\"{RECONCILIATION_SCHEMA}\",\"rail\":{},\"network\":{},\"transaction_id\":{},\"status\":{},\"observed_at_ms\":{observed},\"observed_height\":{},\"confirmations\":{},\"canonical_block_id\":{}}}\n",quote_json(rail.text()),quote_json(network),quote_json(transaction_id),quote_json(status),height.map_or_else(||"null".to_owned(),|v|v.to_string()),confirmations.map_or_else(||"null".to_owned(),|v|v.to_string()),block.as_deref().map_or_else(||"null".to_owned(),quote_json));
    if canonical_source != source {
        return Err(g210("reconciliation", RECONCILIATION_SCHEMA));
    }
    Ok(Reconciliation {
        doc: Doc {
            source: source.to_owned(),
            digest: digest(RECONCILIATION_DOMAIN, source.as_bytes()),
        },
        status,
        transaction_id: transaction_id.to_owned(),
        observed,
        confirmations,
    })
}
fn parse_reconciliation_limited(
    source: &str,
    rail: EconomicRail,
    network: &str,
    transaction_id: &str,
    limits: &Limits,
) -> Result<Reconciliation, Diagnostic> {
    configured_document_limits(
        source,
        "reconciliation",
        limits.max_reconciliation_bytes,
        limits,
    )?;
    reserve_parse_sidecar(source, limits)?;
    parse_reconciliation_with_identifier_limit(
        source,
        rail,
        network,
        transaction_id,
        limits.max_identifier_bytes,
    )
}

fn capsule_doc(
    value: &Value,
    schema: &str,
    domain: &[u8],
    maximum: usize,
    max_depth: u64,
    document: &str,
) -> Result<Option<Doc>, Diagnostic> {
    if value.is_null() {
        return Ok(None);
    }
    let row = object(value, "journal", JOURNAL_SCHEMA)?;
    if !keys(row, &["schema", "digest", "bytes", "document"]) {
        return Err(g215());
    }
    let source = text(row, "document", "journal", JOURNAL_SCHEMA)?.to_owned();
    let sidecar = source
        .len()
        .checked_mul(
            usize::try_from(max_depth)
                .map_err(|_| g217())?
                .checked_add(2)
                .ok_or_else(g217)?,
        )
        .ok_or_else(g217)?;
    if active_remaining().is_some_and(|remaining| sidecar > remaining) || !reserve_active(sidecar) {
        return Err(g216(
            "builder_bytes",
            active_limit().unwrap_or(MAX_BUILDER_BYTES) as u64,
        ));
    }
    if source.len() > maximum
        || row.get("schema").and_then(Value::as_str) != Some(schema)
        || row.get("bytes").and_then(Value::as_u64) != u64::try_from(source.len()).ok()
    {
        return Err(g215());
    }
    canonical_policy_limited(&source, document, schema, maximum as u64, max_depth)?;
    let digest_value = digest(domain, source.as_bytes());
    if row.get("digest").and_then(Value::as_str) != Some(digest_value.as_str()) {
        return Err(g215());
    }
    Ok(Some(Doc {
        source,
        digest: digest_value,
    }))
}
fn generic_ref_doc(
    value: &Value,
    schema: &str,
    maximum: u64,
) -> Result<Option<DocRef>, Diagnostic> {
    if value.is_null() {
        return Ok(None);
    }
    let row = object(value, "journal", JOURNAL_SCHEMA)?;
    if !keys(row, &["schema", "digest", "bytes"])
        || row.get("schema").and_then(Value::as_str) != Some(schema)
    {
        return Err(g215());
    }
    let digest_value = text(row, "digest", "journal", JOURNAL_SCHEMA)?.to_owned();
    let bytes = number(row, "bytes", "journal", JOURNAL_SCHEMA)?;
    if bytes > maximum
        || !digest_value.starts_with("sha256:")
        || digest_value.len() != 71
        || !digest_value[7..]
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        return Err(g215());
    }
    Ok(Some(DocRef {
        bytes,
        digest: digest_value,
    }))
}
fn unsigned_journal_ref(
    value: &Value,
    maximum: u64,
) -> Result<Option<(String, usize, &'static str)>, Diagnostic> {
    if value.is_null() {
        return Ok(None);
    }
    let row = object(value, "journal", JOURNAL_SCHEMA)?;
    if !keys(row, &["digest", "bytes", "format"]) {
        return Err(g215());
    }
    let digest_value = text(row, "digest", "journal", JOURNAL_SCHEMA)?.to_owned();
    let bytes = number(row, "bytes", "journal", JOURNAL_SCHEMA)?;
    let format = match text(row, "format", "journal", JOURNAL_SCHEMA)? {
        "eip1559-unsigned-v1" => "eip1559-unsigned-v1",
        "solana-message-v0" => "solana-message-v0",
        "psbt-v2" => "psbt-v2",
        _ => return Err(g215()),
    };
    if bytes > maximum || digest_value.len() != 71 || !digest_value.starts_with("sha256:") {
        return Err(g215());
    }
    Ok(Some((digest_value, bytes as usize, format)))
}
fn signed_journal_ref(value: &Value, maximum: u64) -> Result<Option<(String, usize)>, Diagnostic> {
    if value.is_null() {
        return Ok(None);
    }
    let row = object(value, "journal", JOURNAL_SCHEMA)?;
    if !keys(row, &["digest", "bytes"]) {
        return Err(g215());
    }
    let digest_value = text(row, "digest", "journal", JOURNAL_SCHEMA)?.to_owned();
    let bytes = number(row, "bytes", "journal", JOURNAL_SCHEMA)?;
    if bytes > maximum || digest_value.len() != 71 || !digest_value.starts_with("sha256:") {
        return Err(g215());
    }
    Ok(Some((digest_value, bytes as usize)))
}
enum JournalParseFailure {
    BindingMismatch,
    Diagnostic(Diagnostic),
}

impl From<Diagnostic> for JournalParseFailure {
    fn from(diagnostic: Diagnostic) -> Self {
        Self::Diagnostic(diagnostic)
    }
}

fn parse_journal(
    source: &str,
    policy: &Policy,
    intent: &Intent,
    run_id: &str,
) -> Result<Journal, Diagnostic> {
    parse_journal_classified(source, policy, intent, run_id).map_err(|failure| match failure {
        JournalParseFailure::BindingMismatch => g215(),
        JournalParseFailure::Diagnostic(diagnostic) => diagnostic,
    })
}

fn parse_journal_classified(
    source: &str,
    policy: &Policy,
    intent: &Intent,
    run_id: &str,
) -> Result<Journal, JournalParseFailure> {
    configured_document_limits(
        source,
        "journal",
        policy.limits.max_journal_bytes,
        &policy.limits,
    )?;
    let sidecar = source.len().checked_mul(2).ok_or_else(g217)?;
    if active_remaining().is_some_and(|remaining| sidecar > remaining) || !reserve_active(sidecar) {
        return Err(g216("builder_bytes", policy.limits.max_builder_bytes).into());
    }
    let (_, value) = canonical(
        source,
        "journal",
        JOURNAL_SCHEMA,
        policy.limits.max_journal_bytes as usize,
    )?;
    let row = object(&value, "journal", JOURNAL_SCHEMA)?;
    if !keys(
        row,
        &[
            "schema",
            "idempotency_key",
            "version",
            "policy",
            "intent",
            "run_id",
            "state",
            "reserved_amount_atomic",
            "reserved_fee_atomic",
            "plan",
            "simulation",
            "approval",
            "unsigned_transaction",
            "signed_transaction",
            "broadcast",
            "reconciliation",
            "updated_at_ms",
        ],
    ) {
        return Err(g215().into());
    }
    let policy_doc = Doc {
        source: policy.source.clone(),
        digest: policy.digest.clone(),
    };
    let intent_doc = Doc {
        source: intent.source.clone(),
        digest: intent.digest.clone(),
    };
    if text(row, "idempotency_key", "journal", JOURNAL_SCHEMA)? != intent.idempotency_key
        || text(row, "run_id", "journal", JOURNAL_SCHEMA)? != run_id
        || !ref_matches(&row["policy"], POLICY_SCHEMA, &policy_doc)
        || !ref_matches(&row["intent"], INTENT_SCHEMA, &intent_doc)
    {
        return Err(JournalParseFailure::BindingMismatch);
    }
    let broadcast = capsule_doc(
        &row["broadcast"],
        BROADCAST_SCHEMA,
        BROADCAST_DOMAIN,
        policy.limits.max_broadcast_receipt_bytes as usize,
        policy.limits.max_json_depth,
        "broadcast receipt",
    )?;
    let reconciliation = capsule_doc(
        &row["reconciliation"],
        RECONCILIATION_SCHEMA,
        RECONCILIATION_DOMAIN,
        policy.limits.max_reconciliation_bytes as usize,
        policy.limits.max_json_depth,
        "reconciliation",
    )?;
    let journal = Journal {
        idempotency_key: intent.idempotency_key.clone(),
        version: number(row, "version", "journal", JOURNAL_SCHEMA)?,
        policy: policy_doc,
        intent: intent_doc,
        run_id: run_id.to_owned(),
        state: JournalState::parse(text(row, "state", "journal", JOURNAL_SCHEMA)?)
            .ok_or_else(g215)?,
        reserved_amount: number(row, "reserved_amount_atomic", "journal", JOURNAL_SCHEMA)?,
        reserved_fee: number(row, "reserved_fee_atomic", "journal", JOURNAL_SCHEMA)?,
        plan: generic_ref_doc(&row["plan"], PLAN_SCHEMA, policy.limits.max_plan_bytes)?,
        simulation: generic_ref_doc(
            &row["simulation"],
            SIMULATION_SCHEMA,
            policy.limits.max_simulation_bytes,
        )?,
        approval: generic_ref_doc(
            &row["approval"],
            APPROVAL_SCHEMA,
            policy.limits.max_approval_bytes,
        )?,
        unsigned: unsigned_journal_ref(
            &row["unsigned_transaction"],
            policy.limits.max_unsigned_transaction_bytes,
        )?,
        signed: signed_journal_ref(
            &row["signed_transaction"],
            policy.limits.max_signed_transaction_bytes,
        )?,
        broadcast,
        reconciliation,
        updated_at: number(row, "updated_at_ms", "journal", JOURNAL_SCHEMA)?,
    };
    if journal.reserved_amount != intent.amount() || journal.reserved_fee != intent.max_fee() {
        return Err(g215().into());
    }
    let prepared =
        journal.plan.is_some() && journal.simulation.is_some() && journal.unsigned.is_some();
    let approved = prepared && journal.approval.is_some();
    let signed = approved && journal.signed.is_some();
    let broadcasted = signed && journal.broadcast.is_some();
    let reserved_prefix = journal.plan.is_none()
        && journal.simulation.is_none()
        && journal.approval.is_none()
        && journal.unsigned.is_none()
        && journal.signed.is_none()
        && journal.broadcast.is_none()
        && journal.reconciliation.is_none();
    let prepared_prefix = prepared
        && journal.approval.is_none()
        && journal.signed.is_none()
        && journal.broadcast.is_none()
        && journal.reconciliation.is_none();
    let approved_prefix = approved
        && journal.signed.is_none()
        && journal.broadcast.is_none()
        && journal.reconciliation.is_none();
    let valid_shape = match journal.state {
        JournalState::Reserved => reserved_prefix,
        JournalState::Prepared => prepared_prefix,
        JournalState::Approved => approved_prefix,
        JournalState::Signed => {
            signed && journal.broadcast.is_none() && journal.reconciliation.is_none()
        }
        JournalState::BroadcastUnknown | JournalState::Broadcasted => {
            broadcasted && journal.reconciliation.is_none()
        }
        JournalState::Pending => broadcasted,
        JournalState::Confirmed | JournalState::Reorged | JournalState::Dropped => {
            broadcasted && journal.reconciliation.is_some()
        }
        JournalState::Rejected => {
            (reserved_prefix || prepared_prefix || approved_prefix)
                || (broadcasted && journal.reconciliation.is_none())
        }
        JournalState::Cancelled | JournalState::Failed => {
            reserved_prefix || prepared_prefix || approved_prefix
        }
    };
    let version_shape = match journal.state {
        JournalState::Reserved => journal.version == 1,
        JournalState::Prepared => journal.version == 2,
        JournalState::Approved => matches!(journal.version, 3 | 4),
        JournalState::Signed => journal.version == 5,
        JournalState::BroadcastUnknown => journal.version >= 6,
        JournalState::Broadcasted
        | JournalState::Pending
        | JournalState::Confirmed
        | JournalState::Reorged
        | JournalState::Dropped
        | JournalState::Rejected => journal.version >= 7,
        JournalState::Cancelled | JournalState::Failed => journal.version >= 2,
    };
    if !valid_shape || !version_shape {
        return Err(g215().into());
    }
    if let Some(broadcast_doc) = journal.broadcast.as_ref() {
        let signed_digest = journal.signed.as_ref().ok_or_else(g215)?.0.as_str();
        let (network, _) = intent.network_asset();
        let provisional = broadcast_is_provisional(broadcast_doc);
        let base = if provisional { 6 } else { 7 };
        let offset = journal.version.checked_sub(base).ok_or_else(g215)?;
        let attempts = offset.checked_add(1).ok_or_else(g215)? / 2;
        let odd = offset % 2 == 1;
        if attempts > policy.limits.max_reconciliations
            || (journal.reconciliation.is_some() && odd)
            || (matches!(
                journal.state,
                JournalState::Confirmed | JournalState::Reorged | JournalState::Dropped
            ) && (journal.reconciliation.is_none() || attempts == 0 || odd))
        {
            return Err(g215().into());
        }
        let broadcast = if provisional {
            let value: Value =
                serde_json::from_str(broadcast_doc.source.trim_end()).map_err(|_| g215())?;
            let transaction_id = value["transaction_id"].as_str().ok_or_else(g215)?;
            parse_provisional_broadcast(
                &broadcast_doc.source,
                intent.settlement_rail(),
                network,
                signed_digest,
                transaction_id,
            )?
        } else {
            parse_broadcast(
                &broadcast_doc.source,
                intent.settlement_rail(),
                network,
                signed_digest,
                None,
            )?
        };
        let allowed_disposition = match journal.state {
            JournalState::BroadcastUnknown => broadcast.disposition == "unknown",
            JournalState::Broadcasted => broadcast.disposition == "accepted",
            JournalState::Pending if journal.reconciliation.is_none() => {
                broadcast.disposition == "pending"
            }
            JournalState::Pending => matches!(broadcast.disposition, "accepted" | "pending"),
            JournalState::Confirmed | JournalState::Reorged | JournalState::Dropped => {
                matches!(broadcast.disposition, "accepted" | "pending")
            }
            JournalState::Rejected => broadcast.disposition == "rejected",
            _ => false,
        };
        if !allowed_disposition {
            return Err(g215().into());
        }
        if let Some(reconciliation_doc) = journal.reconciliation.as_ref() {
            let reconciliation = parse_reconciliation(
                &reconciliation_doc.source,
                intent.settlement_rail(),
                network,
                &broadcast.transaction_id,
            )?;
            validate_confirmation(intent, &reconciliation)?;
            let status_matches = match journal.state {
                JournalState::Pending => reconciliation.status == "pending",
                JournalState::Confirmed => reconciliation.status == "confirmed",
                JournalState::Reorged => reconciliation.status == "reorged",
                JournalState::Dropped => reconciliation.status == "dropped",
                _ => false,
            };
            if reconciliation.observed < broadcast.observed
                || reconciliation.observed != journal.updated_at
                || !status_matches
            {
                return Err(g215().into());
            }
        } else if journal.updated_at != broadcast.observed
            && !(journal.state == JournalState::BroadcastUnknown
                && provisional
                && broadcast.disposition == "unknown"
                && broadcast.observed == 0)
        {
            return Err(g215().into());
        }
    }
    Ok(journal)
}

fn broadcast_is_provisional(document: &Doc) -> bool {
    serde_json::from_str::<Value>(document.source.trim_end())
        .ok()
        .is_some_and(|value| {
            value["disposition"].as_str() == Some("unknown")
                && value["observed_at_ms"].as_u64() == Some(0)
        })
}

fn reconciliation_topology(journal: &Journal) -> Result<(u64, bool), Diagnostic> {
    let broadcast = journal.broadcast.as_ref().ok_or_else(g215)?;
    let base = if broadcast_is_provisional(broadcast) {
        6
    } else {
        7
    };
    let offset = journal.version.checked_sub(base).ok_or_else(g215)?;
    let attempts = offset.checked_add(1).ok_or_else(g215)? / 2;
    Ok((attempts, offset % 2 == 1))
}

#[derive(Default)]
struct CountSink(usize);
impl fmt::Write for CountSink {
    fn write_str(&mut self, value: &str) -> fmt::Result {
        self.0 = self.0.checked_add(value.len()).ok_or(fmt::Error)?;
        Ok(())
    }
}

struct MatchSink<'a> {
    expected: &'a [u8],
    offset: usize,
}

struct DigestSink {
    hash: Sha256,
    bytes: usize,
}
impl DigestSink {
    fn new(domain: &[u8]) -> Self {
        let mut hash = Sha256::new();
        hash.update(domain);
        Self { hash, bytes: 0 }
    }
    fn finish(self) -> (String, usize) {
        (
            format!(
                "sha256:{:x}",
                crate::digest_hex::LowerHex(self.hash.finalize())
            ),
            self.bytes,
        )
    }
}
impl fmt::Write for DigestSink {
    fn write_str(&mut self, value: &str) -> fmt::Result {
        self.bytes = self.bytes.checked_add(value.len()).ok_or(fmt::Error)?;
        self.hash.update(value.as_bytes());
        Ok(())
    }
}
impl fmt::Write for MatchSink<'_> {
    fn write_str(&mut self, value: &str) -> fmt::Result {
        let end = self.offset.checked_add(value.len()).ok_or(fmt::Error)?;
        if self.expected.get(self.offset..end) != Some(value.as_bytes()) {
            return Err(fmt::Error);
        }
        self.offset = end;
        Ok(())
    }
}

fn write_json<W: fmt::Write>(output: &mut W, value: &str) -> fmt::Result {
    output.write_char('"')?;
    for character in value.chars() {
        match character {
            '"' => output.write_str("\\\"")?,
            '\\' => output.write_str("\\\\")?,
            '\n' => output.write_str("\\n")?,
            '\r' => output.write_str("\\r")?,
            '\t' => output.write_str("\\t")?,
            value if value.is_control() => write!(output, "\\u{:04x}", value as u32)?,
            value => output.write_char(value)?,
        }
    }
    output.write_char('"')
}
fn write_optional_json<W: fmt::Write>(output: &mut W, value: Option<&str>) -> fmt::Result {
    match value {
        Some(value) => write_json(output, value),
        None => output.write_str("null"),
    }
}
fn write_usage<W: fmt::Write>(output: &mut W, usage: &Usage) -> fmt::Result {
    write!(output,"{{\"journal_reads\":{},\"journal_writes\":{},\"invoice_reads\":{},\"snapshot_reads\":{},\"simulations\":{},\"approvals\":{},\"signatures\":{},\"broadcasts\":{},\"reconciliations\":{},\"input_bytes\":{},\"output_bytes\":{},\"elapsed_ms\":{}}}",usage.journal_reads,usage.journal_writes,usage.invoice_reads,usage.snapshot_reads,usage.simulations,usage.approvals,usage.signatures,usage.broadcasts,usage.reconciliations,usage.input_bytes,usage.output_bytes,usage.elapsed_ms)
}
fn write_event<W: fmt::Write>(output: &mut W, index: usize, event: &Event) -> fmt::Result {
    write!(output, "{{\"index\":{index},\"kind\":")?;
    write_json(output, event.kind)?;
    output.write_str(",\"rail\":")?;
    write_optional_json(output, event.rail.map(EconomicRail::text))?;
    output.write_str(",\"input_digest\":")?;
    write_optional_json(output, event.input.as_deref())?;
    output.write_str(",\"output_digest\":")?;
    write_optional_json(output, event.output.as_deref())?;
    output.write_str(",\"status\":")?;
    write_json(output, event.status)?;
    output.write_str(",\"usage\":")?;
    write_usage(output, &event.usage)?;
    output.write_char('}')
}
fn write_result<W: fmt::Write>(output: &mut W, terminal: &Terminal) -> fmt::Result {
    output.write_str("{\"status\":")?;
    write_json(output, terminal.status.text())?;
    output.write_str(",\"transaction_id\":")?;
    write_optional_json(output, terminal.transaction_id.as_deref())?;
    output.write_str(",\"confirmation_status\":")?;
    write_optional_json(output, terminal.confirmation.as_deref())?;
    output.write_str(",\"code\":")?;
    write_optional_json(output, terminal.code.as_deref())?;
    output.write_str(",\"message\":")?;
    write_optional_json(output, terminal.message.as_deref())?;
    output.write_char('}')
}
fn write_nonclaims<W: fmt::Write>(output: &mut W) -> fmt::Result {
    output.write_char('[')?;
    for (index, value) in NONCLAIMS.iter().enumerate() {
        if index > 0 {
            output.write_char(',')?;
        }
        write_json(output, value)?;
    }
    output.write_char(']')
}
fn write_trace<W: fmt::Write>(
    output: &mut W,
    run_id: &str,
    source_agent_digest: &str,
    policy: &Policy,
    intent: &Intent,
    events: &[Event],
    terminal: &Terminal,
) -> fmt::Result {
    output.write_str("{\"schema\":\"")?;
    output.write_str(TRACE_SCHEMA)?;
    output.write_str("\",\"run_id\":")?;
    write_json(output, run_id)?;
    output.write_str(",\"source_agent_evidence_digest\":")?;
    write_json(output, source_agent_digest)?;
    output.write_str(",\"policy_digest\":")?;
    write_json(output, &policy.digest)?;
    output.write_str(",\"intent_digest\":")?;
    write_json(output, &intent.digest)?;
    output.write_str(",\"events\":[")?;
    for (index, event) in events.iter().enumerate() {
        if index > 0 {
            output.write_char(',')?;
        }
        write_event(output, index, event)?;
    }
    output.write_str("],\"result\":")?;
    write_result(output, terminal)?;
    output.write_str(",\"nonclaims\":")?;
    write_nonclaims(output)?;
    output.write_str("}\n")
}

fn usage_json(usage: &Usage) -> String {
    let mut output = String::new();
    write_usage(&mut output, usage).expect("String writes cannot fail");
    output
}
fn event_json(index: usize, event: &Event) -> String {
    let mut output = String::new();
    write_event(&mut output, index, event).expect("String writes cannot fail");
    output
}
fn result_json(terminal: &Terminal) -> String {
    let mut output = String::new();
    write_result(&mut output, terminal).expect("String writes cannot fail");
    output
}
fn render_trace(
    run_id: &str,
    source_agent_digest: &str,
    policy: &Policy,
    intent: &Intent,
    events: &[Event],
    terminal: &Terminal,
) -> Result<Doc, Diagnostic> {
    if events.len() > policy.limits.max_trace_events as usize {
        return Err(g216("trace_events", policy.limits.max_trace_events));
    }
    let mut count = CountSink::default();
    write_trace(
        &mut count,
        run_id,
        source_agent_digest,
        policy,
        intent,
        events,
        terminal,
    )
    .map_err(|_| g217())?;
    if count.0 > policy.limits.max_trace_bytes as usize {
        return Err(g216("trace_bytes", policy.limits.max_trace_bytes));
    }
    if !reserve_active(count.0) {
        return Err(g216("builder_bytes", policy.limits.max_builder_bytes));
    }
    let mut source = String::with_capacity(count.0);
    write_trace(
        &mut source,
        run_id,
        source_agent_digest,
        policy,
        intent,
        events,
        terminal,
    )
    .map_err(|_| g217())?;
    if source.len() != count.0 {
        return Err(g217());
    }
    Ok(Doc {
        digest: digest(TRACE_DOMAIN, source.as_bytes()),
        source,
    })
}
fn limits_evidence_json(l: &Limits) -> String {
    limits_json(l)
}
fn budget_json(b: &Budget) -> String {
    let mut output = String::new();
    write_budget(&mut output, b).expect("String writes cannot fail");
    output
}
fn write_budget<W: fmt::Write>(output: &mut W, b: &Budget) -> fmt::Result {
    write!(output,"{{\"used_policy_bytes\":{},\"used_intent_bytes\":{},\"used_invoice_bytes\":{},\"used_snapshot_bytes\":{},\"used_plan_bytes\":{},\"used_simulation_bytes\":{},\"used_approval_request_bytes\":{},\"used_approval_bytes\":{},\"used_journal_bytes\":{},\"used_unsigned_transaction_bytes\":{},\"used_signed_transaction_bytes\":{},\"used_broadcast_receipt_bytes\":{},\"used_reconciliation_bytes\":{},\"used_trace_events\":{},\"used_trace_bytes\":{},\"used_evidence_bytes\":{},\"used_builder_bytes\":{},\"used_recipients\":{},\"used_network_policies\":{},\"used_x402_origins\":{},\"used_utxos\":{},\"used_reconciliations\":{},\"used_elapsed_ms\":{},\"used_concurrency\":{},\"used_unexpected_authority_calls\":{}}}",b.policy_bytes,b.intent_bytes,b.invoice_bytes,b.snapshot_bytes,b.plan_bytes,b.simulation_bytes,b.approval_request_bytes,b.approval_bytes,b.journal_bytes,b.unsigned_bytes,b.signed_bytes,b.broadcast_bytes,b.reconciliation_bytes,b.trace_events,b.trace_bytes,b.evidence_bytes,b.builder_bytes,b.recipients,b.network_policies,b.x402_origins,b.utxos,b.reconciliations,b.elapsed_ms,b.concurrency,b.unexpected_authority_calls)
}
struct EvidenceParts<'a> {
    run_id: &'a str,
    agent_run_id: &'a str,
    agent_evidence: &'a str,
    agent_digest: &'a str,
    policy: &'a Policy,
    intent: &'a Intent,
    invoice: Option<&'a Invoice>,
    plan: Option<&'a Plan>,
    simulation: Option<&'a Simulation>,
    approval: Option<&'a Approval>,
    journal: &'a Journal,
    broadcast: Option<&'a BroadcastReceipt>,
    reconciliation: Option<&'a Reconciliation>,
    trace: &'a Doc,
    terminal: &'a Terminal,
    budget: &'a mut Budget,
}

fn write_doc_reference<W: fmt::Write>(
    output: &mut W,
    schema: &str,
    digest_value: &str,
    bytes: usize,
) -> fmt::Result {
    output.write_str("{\"schema\":")?;
    write_json(output, schema)?;
    output.write_str(",\"digest\":")?;
    write_json(output, digest_value)?;
    write!(output, ",\"bytes\":{bytes}}}")
}

fn write_optional_reference<W: fmt::Write>(
    output: &mut W,
    schema: &str,
    document: Option<&Doc>,
) -> fmt::Result {
    match document {
        Some(document) => {
            write_doc_reference(output, schema, &document.digest, document.source.len())
        }
        None => output.write_str("null"),
    }
}

fn write_evidence<W: fmt::Write>(
    output: &mut W,
    parts: &EvidenceParts<'_>,
    journal_digest: &str,
    journal_bytes: usize,
) -> fmt::Result {
    output.write_str("{\"schema\":\"")?;
    output.write_str(EVIDENCE_SCHEMA)?;
    output.write_str("\",\"run_id\":")?;
    write_json(output, parts.run_id)?;
    output.write_str(
        ",\"source_agent\":{\"schema\":\"semaprax.agent-runtime-evidence.v1\",\"digest\":",
    )?;
    write_json(output, parts.agent_digest)?;
    write!(
        output,
        ",\"bytes\":{},\"run_id\":",
        parts.agent_evidence.len()
    )?;
    write_json(output, parts.agent_run_id)?;
    output.write_str("},\"policy\":")?;
    write_doc_reference(
        output,
        POLICY_SCHEMA,
        &parts.policy.digest,
        parts.policy.source.len(),
    )?;
    output.write_str(",\"intent\":")?;
    write_doc_reference(
        output,
        INTENT_SCHEMA,
        &parts.intent.digest,
        parts.intent.source.len(),
    )?;
    output.write_str(",\"x402_invoice\":")?;
    write_optional_reference(output, INVOICE_SCHEMA, parts.invoice.map(|v| &v.doc))?;
    output.write_str(",\"plan\":")?;
    write_optional_reference(output, PLAN_SCHEMA, parts.plan.map(|v| &v.doc))?;
    output.write_str(",\"simulation\":")?;
    write_optional_reference(output, SIMULATION_SCHEMA, parts.simulation.map(|v| &v.doc))?;
    output.write_str(",\"approval\":")?;
    write_optional_reference(output, APPROVAL_SCHEMA, parts.approval.map(|v| &v.doc))?;
    output.write_str(",\"journal\":")?;
    write_doc_reference(output, JOURNAL_SCHEMA, journal_digest, journal_bytes)?;
    output.write_str(",\"broadcast\":")?;
    write_optional_reference(output, BROADCAST_SCHEMA, parts.broadcast.map(|v| &v.doc))?;
    output.write_str(",\"reconciliation\":")?;
    write_optional_reference(
        output,
        RECONCILIATION_SCHEMA,
        parts.reconciliation.map(|v| &v.doc),
    )?;
    output.write_str(",\"trace\":{\"schema\":")?;
    write_json(output, TRACE_SCHEMA)?;
    output.write_str(",\"digest\":")?;
    write_json(output, &parts.trace.digest)?;
    write!(
        output,
        ",\"bytes\":{},\"document\":",
        parts.trace.source.len()
    )?;
    write_json(output, &parts.trace.source)?;
    output.write_str("},\"result\":")?;
    write_result(output, parts.terminal)?;
    output.write_str(",\"limits\":")?;
    write_limits(output, &parts.policy.limits)?;
    output.write_str(",\"budget\":")?;
    write_budget(output, parts.budget)?;
    output.write_str(",\"nonclaims\":")?;
    write_nonclaims(output)?;
    output.write_str("}\n")
}
fn journal_identity(parts: &EvidenceParts<'_>) -> (String, usize) {
    let mut sink = DigestSink::new(JOURNAL_DOMAIN);
    write_journal(&mut sink, parts.journal).expect("journal identity cannot fail");
    sink.finish()
}

fn render_evidence(parts: &mut EvidenceParts<'_>) -> Result<Doc, Diagnostic> {
    let (journal_digest, journal_bytes) = journal_identity(parts);
    let builder_before_evidence = parts
        .policy
        .limits
        .max_builder_bytes
        .checked_sub(active_remaining().ok_or_else(g217)? as u64)
        .ok_or_else(g217)?;
    let mut evidence_bytes = 0;
    let mut builder_bytes = builder_before_evidence;
    let mut converged = false;
    for _ in 0..24 {
        parts.budget.evidence_bytes = evidence_bytes;
        parts.budget.builder_bytes = builder_bytes;
        let mut count = CountSink::default();
        write_evidence(&mut count, parts, &journal_digest, journal_bytes).map_err(|_| g217())?;
        let next_evidence = u64::try_from(count.0).map_err(|_| g217())?;
        let next_builder = builder_before_evidence
            .checked_add(next_evidence)
            .ok_or_else(g217)?;
        if next_evidence == evidence_bytes && next_builder == builder_bytes {
            converged = true;
            break;
        }
        evidence_bytes = next_evidence;
        builder_bytes = next_builder;
    }
    if !converged {
        return Err(g217());
    }
    if evidence_bytes > parts.policy.limits.max_evidence_bytes {
        return Err(g216(
            "evidence_bytes",
            parts.policy.limits.max_evidence_bytes,
        ));
    }
    parts.budget.evidence_bytes = evidence_bytes;
    parts.budget.builder_bytes = builder_bytes;
    let evidence_len = usize::try_from(evidence_bytes).map_err(|_| g217())?;
    if !reserve_active(evidence_len) {
        return Err(g216("builder_bytes", parts.policy.limits.max_builder_bytes));
    }
    let mut source = String::with_capacity(evidence_len);
    write_evidence(&mut source, parts, &journal_digest, journal_bytes).map_err(|_| g217())?;
    if source.len() != evidence_len {
        return Err(g217());
    }
    Ok(Doc {
        digest: digest(EVIDENCE_DOMAIN, source.as_bytes()),
        source,
    })
}

fn run_id(
    agent_digest: &str,
    policy_digest: &str,
    intent_digest: &str,
    idempotency: &str,
) -> String {
    let mut hash = Sha256::new();
    hash.update(RUN_ID_DOMAIN);
    hash.update(agent_digest.as_bytes());
    hash.update(policy_digest.as_bytes());
    hash.update(intent_digest.as_bytes());
    hash.update(idempotency.as_bytes());
    format!("sha256:{:x}", crate::digest_hex::LowerHex(hash.finalize()))
}
fn cumulative_usage(events: &[Event]) -> Result<Usage, Diagnostic> {
    let mut total = Usage::default();
    for event in events {
        macro_rules! add {
            ($field:ident) => {
                total.$field = total
                    .$field
                    .checked_add(event.usage.$field)
                    .ok_or_else(g217)?
            };
        }
        add!(journal_reads);
        add!(journal_writes);
        add!(invoice_reads);
        add!(snapshot_reads);
        add!(simulations);
        add!(approvals);
        add!(signatures);
        add!(broadcasts);
        add!(reconciliations);
        add!(input_bytes);
        add!(output_bytes);
        add!(elapsed_ms);
    }
    Ok(total)
}
fn valid_event(kind: &str, status: &str) -> bool {
    match kind {
        "run_started" => status == "started",
        "journal_loaded" => matches!(status, "missing" | "present" | "failed"),
        "intent_reserved" => matches!(status, "reserved" | "failed"),
        "invoice_loaded" | "snapshot_loaded" => matches!(status, "loaded" | "failed"),
        "plan_built" => matches!(status, "built" | "failed"),
        "simulation_finished" => matches!(status, "succeeded" | "rejected" | "failed"),
        "approval_finished" => matches!(status, "approved" | "rejected" | "failed"),
        "transaction_signed" => matches!(status, "signed" | "failed"),
        "broadcast_finished" => matches!(
            status,
            "accepted" | "pending" | "unknown" | "rejected" | "failed"
        ),
        "reconciliation_finished" => matches!(
            status,
            "pending" | "confirmed" | "reorged" | "dropped" | "failed"
        ),
        "journal_committed" => matches!(status, "committed" | "failed"),
        "run_finished" => matches!(
            status,
            "confirmed"
                | "pending"
                | "reorged"
                | "dropped"
                | "rejected"
                | "cancelled"
                | "deadline_exceeded"
                | "budget_exhausted"
                | "journal_failed"
                | "adapter_failed"
                | "approval_failed"
                | "custody_failed"
                | "broadcast_unknown"
                | "reconciliation_failed"
        ),
        _ => false,
    }
}
fn replay_events(events: &[Event], terminal: &Terminal) -> Result<(), Diagnostic> {
    if events
        .first()
        .is_none_or(|event| event.kind != "run_started")
        || events.last().is_none_or(|event| {
            event.kind != "run_finished" || event.status != terminal.status.text()
        })
        || events
            .iter()
            .any(|event| !valid_event(event.kind, event.status))
    {
        return Err(g217());
    }
    let order = [
        "run_started",
        "journal_loaded",
        "intent_reserved",
        "invoice_loaded",
        "snapshot_loaded",
        "plan_built",
        "simulation_finished",
        "approval_finished",
        "transaction_signed",
        "broadcast_finished",
        "reconciliation_finished",
        "run_finished",
    ];
    let mut previous = 0usize;
    let mut counts = BTreeMap::<&str, u64>::new();
    for event in events {
        let count = counts.entry(event.kind).or_default();
        *count = count.checked_add(1).ok_or_else(g217)?;
        if event.kind == "journal_committed" {
            continue;
        }
        let Some(position) = order.iter().position(|kind| *kind == event.kind) else {
            return Err(g217());
        };
        if position < previous {
            return Err(g217());
        }
        previous = position;
    }
    if counts.get("run_started") != Some(&1)
        || counts.get("journal_loaded") != Some(&1)
        || counts.get("run_finished") != Some(&1)
        || counts.get("transaction_signed").copied().unwrap_or(0) > 1
        || counts.get("broadcast_finished").copied().unwrap_or(0) > 1
    {
        return Err(g217());
    }
    for kind in [
        "intent_reserved",
        "invoice_loaded",
        "snapshot_loaded",
        "plan_built",
        "simulation_finished",
        "approval_finished",
        "reconciliation_finished",
    ] {
        if counts.get(kind).copied().unwrap_or(0) > 1 {
            return Err(g217());
        }
    }
    let has = |kind: &str, status: &str| {
        events
            .iter()
            .any(|event| event.kind == kind && event.status == status)
    };
    let successful_fresh_prefix = has("intent_reserved", "reserved")
        && has("snapshot_loaded", "loaded")
        && has("plan_built", "built")
        && has("simulation_finished", "succeeded")
        && has("approval_finished", "approved")
        && has("transaction_signed", "signed");
    let broadcast_terminal = matches!(
        terminal.status,
        EconomicRunStatus::Confirmed
            | EconomicRunStatus::Pending
            | EconomicRunStatus::Reorged
            | EconomicRunStatus::Dropped
            | EconomicRunStatus::BroadcastUnknown
    );
    let fresh_invocation = counts.get("intent_reserved").copied().unwrap_or(0) != 0;
    if broadcast_terminal
        && fresh_invocation
        && (!successful_fresh_prefix
            || !(has("broadcast_finished", "accepted")
                || has("broadcast_finished", "pending")
                || has("broadcast_finished", "unknown")))
    {
        return Err(g217());
    }
    if matches!(
        terminal.status,
        EconomicRunStatus::Confirmed
            | EconomicRunStatus::Pending
            | EconomicRunStatus::Reorged
            | EconomicRunStatus::Dropped
    ) && !events.iter().any(|event| {
        event.kind == "reconciliation_finished"
            && matches!(
                event.status,
                "confirmed" | "pending" | "reorged" | "dropped"
            )
    }) {
        return Err(g217());
    }
    if terminal.status == EconomicRunStatus::BroadcastUnknown
        && ((!has("broadcast_finished", "unknown") && fresh_invocation)
            || counts.get("reconciliation_finished").copied().unwrap_or(0) != 0)
    {
        return Err(g217());
    }
    if let Some(failed) = events.iter().position(|event| {
        event.status == "failed" || matches!(event.status, "rejected" | "unknown")
    }) {
        if events[failed + 1..]
            .iter()
            .any(|event| event.kind != "journal_committed" && event.kind != "run_finished")
        {
            return Err(g217());
        }
    }
    let usage = cumulative_usage(events)?;
    if usage.journal_reads != 1 || usage.signatures > 1 || usage.broadcasts > 1 {
        return Err(g217());
    }
    Ok(())
}
fn diagnostic_terminal(diagnostic: &Diagnostic) -> Terminal {
    let (code, message) = (diagnostic.code, diagnostic.message.as_str());
    let status = match code {
        "SPX-I222" => EconomicRunStatus::JournalFailed,
        "SPX-I224" => EconomicRunStatus::ApprovalFailed,
        "SPX-I225" => EconomicRunStatus::CustodyFailed,
        "SPX-I226" => EconomicRunStatus::BroadcastUnknown,
        "SPX-I227" => EconomicRunStatus::ReconciliationFailed,
        "SPX-I228" => EconomicRunStatus::Cancelled,
        "SPX-I229" => EconomicRunStatus::DeadlineExceeded,
        "SPX-G216" => EconomicRunStatus::BudgetExhausted,
        "SPX-G214" => EconomicRunStatus::ApprovalFailed,
        "SPX-G212" => EconomicRunStatus::Rejected,
        _ => EconomicRunStatus::AdapterFailed,
    };
    Terminal {
        status,
        transaction_id: None,
        confirmation: None,
        code: Some(code.to_owned()),
        message: Some(message.to_owned()),
    }
}

fn replay_bundle(
    evidence: &Doc,
    trace: &Doc,
    parts: &EvidenceParts<'_>,
    events: &[Event],
) -> Result<(), Diagnostic> {
    replay_events(events, parts.terminal)?;
    let mut trace_match = MatchSink {
        expected: trace.source.as_bytes(),
        offset: 0,
    };
    if write_trace(
        &mut trace_match,
        parts.run_id,
        parts.agent_digest,
        parts.policy,
        parts.intent,
        events,
        parts.terminal,
    )
    .is_err()
        || trace_match.offset != trace.source.len()
        || digest(TRACE_DOMAIN, trace.source.as_bytes()) != trace.digest
    {
        return Err(g217());
    }
    let (_journal_digest, journal_bytes) = journal_identity(parts);
    if digest(EVIDENCE_DOMAIN, evidence.source.as_bytes()) != evidence.digest
        || evidence.source.len() as u64 != parts.budget.evidence_bytes
        || trace.source.len() as u64 != parts.budget.trace_bytes
        || events.len() as u64 != parts.budget.trace_events
    {
        return Err(g217());
    }
    let usage = cumulative_usage(events)?;
    let expected_journal_bytes = journal_bytes as u64;
    let expected_recipients = parts
        .policy
        .networks
        .iter()
        .try_fold(0u64, |sum, row| {
            sum.checked_add(row.recipients.len() as u64)
        })
        .ok_or_else(g217)?;
    let expected_utxos = parts.plan.map_or(0, |plan| plan.utxos);
    if usage.journal_reads != 1
        || usage.reconciliations > parts.budget.reconciliations
        || usage.signatures > 1
        || usage.broadcasts > 1
        || parts.budget.policy_bytes != parts.policy.source.len() as u64
        || parts.budget.intent_bytes != parts.intent.source.len() as u64
        || parts.budget.invoice_bytes
            != parts
                .invoice
                .map_or(0, |value| value.doc.source.len() as u64)
        || parts.budget.plan_bytes != parts.plan.map_or(0, |value| value.doc.source.len() as u64)
        || parts.budget.simulation_bytes
            != parts
                .simulation
                .map_or(0, |value| value.doc.source.len() as u64)
        || parts.budget.approval_bytes
            != parts
                .approval
                .map_or(0, |value| value.doc.source.len() as u64)
        || parts.budget.journal_bytes != expected_journal_bytes
        || parts.budget.unsigned_bytes != parts.plan.map_or(0, |value| value.unsigned.len() as u64)
        || parts.budget.signed_bytes
            != parts
                .journal
                .signed
                .as_ref()
                .map_or(0, |value| value.1 as u64)
        || (parts
            .broadcast
            .is_some_and(|broadcast| broadcast.observed != 0)
            && parts.budget.broadcast_bytes
                != parts
                    .broadcast
                    .map_or(0, |value| value.doc.source.len() as u64))
        || parts.budget.reconciliation_bytes
            != parts
                .reconciliation
                .map_or(0, |value| value.doc.source.len() as u64)
        || parts.budget.recipients != expected_recipients
        || parts.budget.network_policies != parts.policy.networks.len() as u64
        || parts.budget.x402_origins != parts.policy.origins.len() as u64
        || parts.budget.utxos != expected_utxos
        || parts.budget.concurrency != 1
        || parts.budget.unexpected_authority_calls != 0
    {
        return Err(g217());
    }
    let (journal_digest, journal_bytes) = journal_identity(parts);
    let mut evidence_match = MatchSink {
        expected: evidence.source.as_bytes(),
        offset: 0,
    };
    if write_evidence(&mut evidence_match, parts, &journal_digest, journal_bytes).is_err()
        || evidence_match.offset != evidence.source.len()
        || digest(EVIDENCE_DOMAIN, evidence.source.as_bytes()) != evidence.digest
    {
        return Err(g217());
    }
    Ok(())
}

fn event(
    kind: &'static str,
    rail: Option<EconomicRail>,
    input: Option<String>,
    output: Option<String>,
    status: &'static str,
    usage: Usage,
) -> Result<Event, Diagnostic> {
    let owned_bytes = input
        .as_ref()
        .map_or(0, String::len)
        .checked_add(output.as_ref().map_or(0, String::len))
        .and_then(|bytes| bytes.checked_add(std::mem::size_of::<Event>()))
        .unwrap_or(usize::MAX);
    if !reserve_active(owned_bytes) {
        return Err(g216(
            "builder_bytes",
            active_limit().unwrap_or(MAX_BUILDER_BYTES) as u64,
        ));
    }
    Ok(Event {
        kind,
        rail,
        input,
        output,
        status,
        usage,
        authority_uncertain: false,
    })
}

fn push_event(events: &mut Vec<Event>, event: Event) -> Result<(), Diagnostic> {
    events.try_reserve(1).map_err(|_| {
        g216(
            "builder_bytes",
            active_limit().unwrap_or(MAX_BUILDER_BYTES) as u64,
        )
    })?;
    events.push(event);
    Ok(())
}
fn journal_digest(journal: &Journal) -> String {
    let mut sink = DigestSink::new(JOURNAL_DOMAIN);
    write_journal(&mut sink, journal).expect("journal digest cannot fail");
    sink.finish().0
}
fn cas_journal<H: PaymentJournal>(
    host: &mut H,
    journal: &mut Journal,
    events: &mut Vec<Event>,
    budget: &mut Budget,
    maximum: u64,
    rolling: EconomicRollingReservationUpdate<'_>,
) -> Result<(), Diagnostic> {
    let expected = journal.version;
    let authenticated_journal_bytes = budget.journal_bytes;
    let mut current_count = CountSink::default();
    write_journal(&mut current_count, journal).map_err(|_| g215())?;
    budget.journal_bytes = u64::try_from(current_count.0).map_err(|_| g217())?;
    let mut prospective =
        clone_journal_bounded(journal, active_limit().unwrap_or(MAX_BUILDER_BYTES) as u64)?;
    prospective.version = prospective.version.checked_add(1).ok_or_else(g215)?;
    let mut count = CountSink::default();
    write_journal(&mut count, &prospective).map_err(|_| g215())?;
    if count.0 > maximum as usize {
        return Err(g216("journal_bytes", maximum));
    }
    if active_remaining().is_some_and(|remaining| count.0 > remaining) || !reserve_active(count.0) {
        return Err(g216(
            "builder_bytes",
            active_limit().unwrap_or(MAX_BUILDER_BYTES) as u64,
        ));
    }
    let mut source = String::with_capacity(count.0);
    write_journal(&mut source, &prospective).map_err(|_| g215())?;
    let mut journal_match = MatchSink {
        expected: source.as_bytes(),
        offset: 0,
    };
    if write_journal(&mut journal_match, &prospective).is_err()
        || journal_match.offset != source.len()
    {
        return Err(g215());
    }
    let prospective_digest = digest(JOURNAL_DOMAIN, source.as_bytes());
    if !reserve_active(
        prospective_digest
            .len()
            .checked_add(std::mem::size_of::<Event>())
            .ok_or_else(g217)?,
    ) {
        return Err(g216(
            "builder_bytes",
            active_limit().unwrap_or(MAX_BUILDER_BYTES) as u64,
        ));
    }
    events.try_reserve(1).map_err(|_| {
        g216(
            "builder_bytes",
            active_limit().unwrap_or(MAX_BUILDER_BYTES) as u64,
        )
    })?;
    let disposition = host.compare_and_swap(&journal.idempotency_key, expected, &source, rolling);
    let mut usage = Usage::default();
    usage.journal_writes = 1;
    usage.input_bytes = source.len() as u64;
    events.push(Event {
        kind: "journal_committed",
        rail: None,
        input: Some(prospective_digest),
        output: None,
        status: if disposition == EconomicAdapterDisposition::Succeeded {
            "committed"
        } else {
            "failed"
        },
        usage,
        authority_uncertain: disposition == EconomicAdapterDisposition::FailedUncertain,
    });
    if disposition == EconomicAdapterDisposition::FailedUncertain {
        debug_assert!(events.last().is_some_and(|event| event.authority_uncertain));
    }
    if disposition == EconomicAdapterDisposition::PolicyRejected {
        budget.journal_bytes = if expected == 0 {
            current_count.0 as u64
        } else {
            authenticated_journal_bytes
        };
        return Err(g212("amount or fee not allowed"));
    }
    if disposition != EconomicAdapterDisposition::Succeeded {
        budget.journal_bytes = if expected == 0 {
            current_count.0 as u64
        } else {
            authenticated_journal_bytes
        };
        return Err(info("SPX-I222", "Economic Agent journal adapter failed"));
    }
    *journal = prospective;
    budget.journal_bytes = source.len() as u64;
    Ok(())
}
fn finish_run(
    run_id: &str,
    binding: &crate::agent_runtime::EconomicAgentBinding<'_>,
    policy: &Policy,
    intent: &Intent,
    invoice: Option<&Invoice>,
    plan: Option<&Plan>,
    simulation: Option<&Simulation>,
    approval: Option<&Approval>,
    journal: &Journal,
    broadcast: Option<&BroadcastReceipt>,
    reconciliation: Option<&Reconciliation>,
    events: &mut Vec<Event>,
    terminal: Terminal,
    budget: &mut Budget,
    elapsed_ms: u64,
) -> Result<EconomicRun, Diagnostic> {
    clear_active_floor();
    push_event(
        events,
        event(
            "run_finished",
            None,
            None,
            None,
            terminal.status.text(),
            Usage::default(),
        )?,
    )?;
    replay_events(events, &terminal)?;
    let usage = cumulative_usage(events)?;
    if usage.journal_reads != 1 || usage.signatures > 1 || usage.broadcasts > 1 {
        return Err(g217());
    }
    budget.trace_events = events.len().try_into().map_err(|_| g217())?;
    budget.elapsed_ms = elapsed_ms.min(policy.limits.max_elapsed_ms);
    let trace = render_trace(
        run_id,
        binding.evidence_digest,
        policy,
        intent,
        events,
        &terminal,
    )?;
    budget.trace_bytes = trace.source.len() as u64;
    let mut parts = EvidenceParts {
        run_id,
        agent_run_id: binding.run_id,
        agent_evidence: binding.evidence,
        agent_digest: binding.evidence_digest,
        policy,
        intent,
        invoice,
        plan,
        simulation,
        approval,
        journal,
        broadcast,
        reconciliation,
        trace: &trace,
        terminal: &terminal,
        budget,
    };
    let evidence = render_evidence(&mut parts)?;
    let exact_builder = policy
        .limits
        .max_builder_bytes
        .checked_sub(active_remaining().ok_or_else(g217)? as u64)
        .ok_or_else(g217)?;
    if exact_builder != parts.budget.builder_bytes {
        return Err(g217());
    }
    replay_bundle(&evidence, &trace, &parts, events)?;
    Ok(EconomicRun {
        status: terminal.status,
        transaction_id: terminal.transaction_id,
        confirmation_status: terminal.confirmation,
        trace: trace.source,
        trace_digest: trace.digest,
        evidence: evidence.source,
        evidence_digest: evidence.digest,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;

    fn limits() -> Limits {
        Limits {
            max_policy_bytes: MAX_POLICY_BYTES as u64,
            max_intent_bytes: MAX_INTENT_BYTES as u64,
            max_invoice_bytes: MAX_INVOICE_BYTES as u64,
            max_snapshot_bytes: MAX_SNAPSHOT_BYTES as u64,
            max_plan_bytes: MAX_PLAN_BYTES as u64,
            max_simulation_bytes: MAX_SIMULATION_BYTES as u64,
            max_approval_request_bytes: MAX_APPROVAL_REQUEST_BYTES as u64,
            max_approval_bytes: MAX_APPROVAL_BYTES as u64,
            max_journal_bytes: MAX_JOURNAL_BYTES as u64,
            max_unsigned_transaction_bytes: MAX_UNSIGNED_BYTES as u64,
            max_signed_transaction_bytes: MAX_SIGNED_BYTES as u64,
            max_broadcast_receipt_bytes: MAX_BROADCAST_BYTES as u64,
            max_reconciliation_bytes: MAX_RECONCILIATION_BYTES as u64,
            max_trace_events: MAX_TRACE_EVENTS as u64,
            max_trace_bytes: MAX_TRACE_BYTES as u64,
            max_evidence_bytes: MAX_EVIDENCE_BYTES as u64,
            max_builder_bytes: MAX_BUILDER_BYTES as u64,
            max_json_depth: MAX_JSON_DEPTH as u64,
            max_identifier_bytes: MAX_IDENTIFIER_BYTES as u64,
            max_memo_bytes: MAX_MEMO_BYTES as u64,
            max_recipients: MAX_RECIPIENTS as u64,
            max_network_policies: MAX_NETWORK_POLICIES as u64,
            max_x402_origins: MAX_X402_ORIGINS as u64,
            max_utxos: MAX_UTXOS as u64,
            max_reconciliations: 64,
            max_elapsed_ms: 600_000,
            max_amount_atomic: 1_000_000_000_000_000_000,
            max_fee_atomic: 1_000_000_000_000_000,
            max_compute_units: 200_000,
            max_confirmation_target: 144,
            max_concurrency: 1,
            max_unexpected_authority_calls: 0,
        }
    }
    fn evm_policy() -> Policy {
        let mut policy = Policy {
            economic_agent_id: "fixture.economic".to_owned(),
            wallet_id: "fixture.wallet".to_owned(),
            networks: vec![NetworkPolicy {
                rail: EconomicRail::Evm,
                network: "sepolia".to_owned(),
                asset: "native:eth".to_owned(),
                recipients: vec!["0x1111111111111111111111111111111111111111".to_owned()],
                max_amount: 1_000_000,
                max_fee: 1_000_000,
                max_rolling: 1_000_000,
            }],
            origins: vec![],
            limits: limits(),
            source: String::new(),
            digest: String::new(),
        };
        policy.source = render_policy(&policy);
        policy.digest = digest(POLICY_DOMAIN, policy.source.as_bytes());
        policy
    }
    fn evm_intent() -> Intent {
        let mut intent = Intent {
            intent_id: "fixture.intent".to_owned(),
            wallet_id: "fixture.wallet".to_owned(),
            rail_text: "evm".to_owned(),
            idempotency_key: "fixture.payment.evm".to_owned(),
            created_at: 1_700_000_000_000,
            expires_at: 1_700_000_300_000,
            memo: None,
            payment: Payment::Evm {
                recipient: "0x1111111111111111111111111111111111111111".to_owned(),
                amount: 10,
                max_fee: 100_000,
            },
            source: String::new(),
            digest: String::new(),
        };
        intent.source = render_intent(&intent);
        intent.digest = digest(INTENT_DOMAIN, intent.source.as_bytes());
        intent
    }

    fn regtest_recipient(program: [u8; 20]) -> String {
        let charset = b"qpzry9x8gf2tvdw0s3jn54khce6mua7l";
        let mut data = vec![0];
        data.extend(convert_bits(&program, 8, 5, true).unwrap());
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
            encoded.push(charset[usize::from(value)] as char);
        }
        assert!(decode_regtest_p2wpkh(&encoded).is_some());
        encoded
    }

    fn rail_fixture(rail: EconomicRail) -> (Policy, Intent) {
        let (network, asset, recipient, payment, rail_text) = match rail {
            EconomicRail::Evm => {
                let recipient = "0x1111111111111111111111111111111111111111".to_owned();
                (
                    "sepolia",
                    "native:eth",
                    recipient.clone(),
                    Payment::Evm {
                        recipient,
                        amount: 10,
                        max_fee: 100_000,
                    },
                    "evm",
                )
            }
            EconomicRail::Solana => {
                let recipient = encode_base58(&[3; 32]);
                (
                    "devnet",
                    "native:sol",
                    recipient.clone(),
                    Payment::Solana {
                        recipient,
                        amount: 10,
                        max_fee: 6_000,
                        compute: 200_000,
                        priority: 1_000,
                    },
                    "solana",
                )
            }
            EconomicRail::Bitcoin => {
                let recipient = regtest_recipient([9; 20]);
                (
                    "regtest",
                    "native:btc",
                    recipient.clone(),
                    Payment::Bitcoin {
                        recipient,
                        amount: 10_000,
                        max_fee: 10_000,
                        confirmations: 1,
                    },
                    "bitcoin",
                )
            }
        };
        let mut policy = Policy {
            economic_agent_id: "fixture.economic".to_owned(),
            wallet_id: "fixture.wallet".to_owned(),
            networks: vec![NetworkPolicy {
                rail,
                network: network.to_owned(),
                asset: asset.to_owned(),
                recipients: vec![recipient],
                max_amount: 1_000_000,
                max_fee: 1_000_000,
                max_rolling: 1_000_000,
            }],
            origins: vec![],
            limits: limits(),
            source: String::new(),
            digest: String::new(),
        };
        policy.source = render_policy(&policy);
        policy.digest = digest(POLICY_DOMAIN, policy.source.as_bytes());
        let mut intent = Intent {
            intent_id: format!("fixture.intent.{rail_text}"),
            wallet_id: "fixture.wallet".to_owned(),
            rail_text: rail_text.to_owned(),
            idempotency_key: format!("fixture.payment.{rail_text}"),
            created_at: 1_700_000_000_000,
            expires_at: 1_700_000_300_000,
            memo: None,
            payment,
            source: String::new(),
            digest: String::new(),
        };
        intent.source = render_intent(&intent);
        intent.digest = digest(INTENT_DOMAIN, intent.source.as_bytes());
        (policy, intent)
    }

    fn x402_fixture() -> (Policy, Intent, Invoice) {
        let (mut policy, mut intent) = rail_fixture(EconomicRail::Evm);
        let invoice = Invoice {
            origin: "https://pay.example.com".to_owned(),
            method: "POST".to_owned(),
            resource: "/v1/payments".to_owned(),
            invoice_id: "fixture.invoice".to_owned(),
            payee: "0x1111111111111111111111111111111111111111".to_owned(),
            rail: EconomicRail::Evm,
            network: "sepolia".to_owned(),
            asset: "native:eth".to_owned(),
            amount: 10,
            max_fee: 100_000,
            expires: intent.expires_at - 1,
            nonce: "fixture.nonce".to_owned(),
            idempotency: "fixture.payment.x402".to_owned(),
            doc: Doc {
                source: String::new(),
                digest: String::new(),
            },
        };
        let invoice_source = render_invoice(&invoice);
        let mut invoice = invoice;
        invoice.doc = Doc {
            digest: digest(INVOICE_DOMAIN, invoice_source.as_bytes()),
            source: invoice_source,
        };
        policy.origins = vec![OriginPolicy {
            origin: invoice.origin.clone(),
            methods: vec![invoice.method.clone()],
            resources: vec![invoice.resource.clone()],
            rails: vec![EconomicRail::Evm],
            max_amount: 1_000_000,
        }];
        policy.source = render_policy(&policy);
        policy.digest = digest(POLICY_DOMAIN, policy.source.as_bytes());
        intent.intent_id = "fixture.intent.x402".to_owned();
        intent.rail_text = "x402".to_owned();
        intent.idempotency_key = invoice.idempotency.clone();
        intent.payment = Payment::X402 {
            origin: invoice.origin.clone(),
            method: invoice.method.clone(),
            resource: invoice.resource.clone(),
            invoice_digest: invoice.doc.digest.clone(),
            payee: invoice.payee.clone(),
            rail: invoice.rail,
            network: invoice.network.clone(),
            asset: invoice.asset.clone(),
            amount: invoice.amount,
            max_fee: invoice.max_fee,
            invoice_expires: invoice.expires,
            nonce: invoice.nonce.clone(),
        };
        intent.source = render_intent(&intent);
        intent.digest = digest(INTENT_DOMAIN, intent.source.as_bytes());
        (policy, intent, invoice)
    }

    #[test]
    fn policy_and_intent_are_exact_canonical_documents() {
        let policy = evm_policy();
        assert_eq!(parse_policy(&policy.source).unwrap().digest, policy.digest);
        let intent = evm_intent();
        assert_eq!(parse_intent(&intent.source).unwrap().digest, intent.digest);
        assert_eq!(
            parse_policy(&policy.source.replace("\n", "\r\n"))
                .err()
                .unwrap()
                .code,
            "SPX-G210"
        );
        let mut over = intent.source.clone();
        over.insert_str(1, "\"extra\":0,");
        assert_eq!(parse_intent(&over).err().unwrap().code, "SPX-G210");
    }

    #[test]
    fn sealed_agent_fixture_binds_exact_canonical_payment_intent() {
        let intent = evm_intent();
        let run = crate::agent_runtime::completed_run_for_economic_test(&intent.source);
        let binding = run.economic_binding();
        assert_eq!(binding.status, AgentRunStatus::Completed);
        assert_eq!(binding.final_message, Some(intent.source.as_str()));
        assert_eq!(
            parse_intent(binding.final_message.unwrap()).unwrap().digest,
            intent.digest
        );
        assert_eq!(binding.evidence_digest, run.evidence_digest());
    }

    #[test]
    fn origin_resource_and_base58_identity_are_fail_closed() {
        for origin in [
            "https://127.0.0.1",
            "https://10.0.0.1",
            "https://localhost",
            "https://wallet.local",
            "https://example.com:443",
            "http://example.com",
        ] {
            assert!(!valid_origin(origin), "{origin}");
        }
        assert!(valid_origin("https://pay.example.com"));
        for path in [
            "//admin",
            "/../admin",
            "/%2e%2e/admin",
            "/a%2Fb",
            "/a%5cb",
            "/a?b",
        ] {
            assert!(!valid_resource(path), "{path}");
        }
        assert!(valid_resource("/v1/payments"));
        let system = "11111111111111111111111111111111";
        assert_eq!(encode_base58(&decode_base58_32(system).unwrap()), system);
        assert!(decode_base58_32(&format!("1{system}")).is_none());
    }

    #[test]
    fn evm_unsigned_and_signed_replay_bind_every_field() {
        let intent = evm_intent();
        let snapshot = Snapshot {
            rail: EconomicRail::Evm,
            observed: intent.created_at + 1,
            expires: intent.expires_at - 1,
            state: SnapshotState::Evm {
                from: "0x2222222222222222222222222222222222222222".to_owned(),
                nonce: 7,
                base_fee: 1,
                priority: 2,
                gas: 21_000,
            },
            doc: Doc {
                source: "snapshot\n".to_owned(),
                digest: "sha256:fixture".to_owned(),
            },
        };
        let (unsigned, format) = build_unsigned(&intent, &snapshot).unwrap();
        assert_eq!(format, "eip1559-unsigned-v1");
        let mut fields = rlp_list_items(&unsigned[1..])
            .unwrap()
            .into_iter()
            .map(<[u8]>::to_vec)
            .collect::<Vec<_>>();
        fields.extend([rlp_u64(1), rlp_u64(1), rlp_u64(1)]);
        let mut signed = vec![2];
        signed.extend(rlp_list(&fields));
        verify_signed(EconomicRail::Evm, &unsigned, &signed).unwrap();
        let last = signed.len() - 1;
        signed[last] = 0;
        assert_eq!(
            verify_signed(EconomicRail::Evm, &unsigned, &signed)
                .unwrap_err()
                .code,
            "SPX-G213"
        );
    }

    #[test]
    fn solana_fee_conversion_and_v0_shape_are_exact() {
        let payer = encode_base58(&[2; 32]);
        let recipient = encode_base58(&[3; 32]);
        let blockhash = encode_base58(&[4; 32]);
        let intent = Intent {
            intent_id: "i".into(),
            wallet_id: "w".into(),
            rail_text: "solana".into(),
            idempotency_key: "k".into(),
            created_at: 1,
            expires_at: 10,
            memo: None,
            payment: Payment::Solana {
                recipient,
                amount: 7,
                max_fee: 6_000,
                compute: 200_000,
                priority: 1_000,
            },
            source: String::new(),
            digest: String::new(),
        };
        let snapshot = Snapshot {
            rail: EconomicRail::Solana,
            observed: 2,
            expires: 9,
            state: SnapshotState::Solana {
                payer,
                blockhash,
                last_height: 5,
                fee: 5_000,
            },
            doc: Doc {
                source: String::new(),
                digest: String::new(),
            },
        };
        let (bytes, format) = build_unsigned(&intent, &snapshot).unwrap();
        assert_eq!(format, "solana-message-v0");
        assert_eq!(&bytes[..4], &[0x80, 1, 0, 2]);
        assert_eq!(bytes.last(), Some(&0));
    }

    #[test]
    fn keccak_and_rail_transaction_id_vectors_are_pinned() {
        let empty = keccak256(b"");
        assert_eq!(
            empty
                .iter()
                .map(|byte| format!("{byte:02x}"))
                .collect::<String>(),
            "c5d2460186f7233c927e7db2dcc703c0e500b653ca82273b7bfad8045d85a470"
        );
        let abc = keccak256(b"abc");
        assert_eq!(
            abc.iter()
                .map(|byte| format!("{byte:02x}"))
                .collect::<String>(),
            "4e03657aea45a94fc7d47ba826c8d667c0d1e6e33a64a036ec44f58fa12d6c45"
        );
        let mut solana = vec![1];
        solana.extend([7u8; 64]);
        assert_eq!(
            transaction_id(EconomicRail::Solana, &solana),
            Some(encode_base58(&[7u8; 64]))
        );
    }

    #[test]
    fn simulation_requires_exact_native_value_conservation() {
        let intent = evm_intent();
        let plan = Plan {
            doc: Doc {
                source: "plan\n".into(),
                digest: "sha256:plan".into(),
            },
            unsigned: vec![],
            unsigned_digest: "sha256:u".into(),
            format: "eip1559-unsigned-v1",
            observed: intent.created_at + 1,
            expires: intent.expires_at - 1,
            utxos: 0,
        };
        let good=format!("{{\"schema\":\"{SIMULATION_SCHEMA}\",\"plan\":{},\"success\":true,\"fee_atomic\":5,\"balance_before_atomic\":115,\"balance_after_atomic\":100,\"allowance_atomic\":0,\"units\":21000,\"expires_at_ms\":{}}}\n",doc_ref(PLAN_SCHEMA,&plan.doc),plan.expires);
        assert!(parse_simulation(&good, &plan, &intent).is_ok());
        let hostile = good.replace(
            "\"balance_before_atomic\":115",
            "\"balance_before_atomic\":116",
        );
        assert_eq!(
            parse_simulation(&hostile, &plan, &intent)
                .err()
                .unwrap()
                .code,
            "SPX-G213"
        );
    }

    type RollingKey = (String, String, String, String);
    type RollingRows = Vec<(String, u64, u64)>;

    struct FixedEconomicProbe(u64);
    impl EconomicBoundaryProbe for FixedEconomicProbe {
        fn elapsed_ms(&self) -> u64 {
            self.0
        }
    }

    struct FullHost {
        journals: BTreeMap<String, String>,
        calls: Vec<&'static str>,
        intent: Intent,
        invoice: Option<Invoice>,
        broadcast_disposition: EconomicAdapterDisposition,
        reconciliation_status: &'static str,
        trusted_now_ms: u64,
        elapsed_ms: u64,
        rolling: BTreeMap<RollingKey, RollingRows>,
        malformed_simulation: bool,
        documents: BTreeMap<&'static str, String>,
        cas_fault: Option<(usize, EconomicAdapterDisposition, bool)>,
        cancel_after_version: Option<(u64, AgentCancellation)>,
        elapsed_after_version: Option<(u64, u64)>,
        rolling_updates: Vec<&'static str>,
    }

    impl FullHost {
        fn new(intent: Intent) -> Self {
            Self {
                journals: BTreeMap::new(),
                calls: vec![],
                intent,
                invoice: None,
                broadcast_disposition: EconomicAdapterDisposition::Succeeded,
                reconciliation_status: "confirmed",
                trusted_now_ms: 1_700_000_000_000,
                elapsed_ms: 0,
                rolling: BTreeMap::new(),
                malformed_simulation: false,
                documents: BTreeMap::new(),
                cas_fault: None,
                cancel_after_version: None,
                elapsed_after_version: None,
                rolling_updates: vec![],
            }
        }

        fn with_invoice(intent: Intent, invoice: Invoice) -> Self {
            Self {
                invoice: Some(invoice),
                ..Self::new(intent)
            }
        }

        fn record_call(&mut self, call: &'static str) {
            self.calls.push(call);
            if let Some(directory) = std::env::var_os("SEMAPRAX_ECONOMIC_DURABLE_DIR") {
                use std::io::Write as _;
                let path = std::path::PathBuf::from(directory).join("calls");
                let mut file = std::fs::OpenOptions::new()
                    .create(true)
                    .append(true)
                    .open(path)
                    .unwrap();
                writeln!(file, "{call}").unwrap();
            }
        }

        fn stop_after_effect_if_requested(&self, stage: &str) {
            if std::env::var("SEMAPRAX_ECONOMIC_KILL_STAGE").as_deref() != Ok(stage) {
                return;
            }
            let directory = std::path::PathBuf::from(
                std::env::var_os("SEMAPRAX_ECONOMIC_DURABLE_DIR").unwrap(),
            );
            std::fs::write(directory.join("ready"), stage.as_bytes()).unwrap();
            loop {
                std::thread::sleep(std::time::Duration::from_millis(20));
            }
        }

        fn simulation(&mut self, plan: &str, sink: &mut EconomicDocumentSink) {
            self.record_call("simulate");
            if self.malformed_simulation {
                assert!(sink.push(b"{\"secret\":\"economic-secret-sentinel\"}\n"));
                return;
            }
            let value: Value = serde_json::from_str(plan.trim_end()).unwrap();
            let plan_doc = Doc {
                source: plan.to_owned(),
                digest: digest(PLAN_DOMAIN, plan.as_bytes()),
            };
            let amount = value["amount_atomic"].as_u64().unwrap();
            let fee = match self.intent.settlement_rail() {
                EconomicRail::Evm => 63_000,
                EconomicRail::Solana => 6_000,
                EconomicRail::Bitcoin => 10_000,
            };
            let after = 1_000_000;
            let expires = value["expires_at_ms"].as_u64().unwrap();
            let units = match self.intent.settlement_rail() {
                EconomicRail::Evm => 21_000,
                EconomicRail::Solana => 200_000,
                EconomicRail::Bitcoin => 1,
            };
            let allowance = if self.intent.settlement_rail() == EconomicRail::Evm {
                "0"
            } else {
                "null"
            };
            let simulation = format!(
                "{{\"schema\":\"{SIMULATION_SCHEMA}\",\"plan\":{},\"success\":true,\"fee_atomic\":{fee},\"balance_before_atomic\":{},\"balance_after_atomic\":{after},\"allowance_atomic\":{allowance},\"units\":{units},\"expires_at_ms\":{expires}}}\n",
                doc_ref(PLAN_SCHEMA, &plan_doc),
                after + amount + fee,
            );
            self.documents.insert("plan", plan.to_owned());
            self.documents.insert("simulation", simulation.clone());
            assert!(sink.push(simulation.as_bytes()));
        }

        fn broadcast(&mut self, signed: &[u8], sink: &mut EconomicDocumentSink) {
            self.record_call("broadcast");
            let rail = self.intent.settlement_rail();
            let (network, _) = self.intent.network_asset();
            let signed_digest = digest(SIGNED_DOMAIN, signed);
            let txid = transaction_id(rail, signed).unwrap();
            let disposition =
                if self.broadcast_disposition == EconomicAdapterDisposition::FailedUncertain {
                    "unknown"
                } else {
                    "accepted"
                };
            let source = format!(
                "{{\"schema\":\"{BROADCAST_SCHEMA}\",\"rail\":{},\"network\":{},\"signed_transaction_digest\":{},\"transaction_id\":{},\"disposition\":{},\"observed_at_ms\":{}}}\n",
                quote_json(rail.text()),
                quote_json(network),
                quote_json(&signed_digest),
                quote_json(&txid),
                quote_json(disposition),
                self.intent.created_at + 2,
            );
            self.documents.insert("broadcast", source.clone());
            assert!(sink.push(source.as_bytes()));
            self.stop_after_effect_if_requested("broadcast_effect");
        }

        fn reconciliation(&mut self, transaction_id: &str, sink: &mut EconomicDocumentSink) {
            self.record_call("reconcile");
            let rail = self.intent.settlement_rail();
            let (network, _) = self.intent.network_asset();
            let (height, confirmations, block) = if self.reconciliation_status == "confirmed" {
                ("1", "1", quote_json("fixture.block"))
            } else {
                ("null", "null", "null".to_owned())
            };
            let source = format!(
                "{{\"schema\":\"{RECONCILIATION_SCHEMA}\",\"rail\":{},\"network\":{},\"transaction_id\":{},\"status\":{},\"observed_at_ms\":{},\"observed_height\":{height},\"confirmations\":{confirmations},\"canonical_block_id\":{block}}}\n",
                quote_json(rail.text()),
                quote_json(network),
                quote_json(transaction_id),
                quote_json(self.reconciliation_status),
                self.intent.created_at + 3,
            );
            self.documents.insert("reconciliation", source.clone());
            assert!(sink.push(source.as_bytes()));
        }
    }

    impl EconomicAgentHost for FullHost {
        fn boundary_probe(&self) -> Box<dyn EconomicBoundaryProbe> {
            Box::new(FixedEconomicProbe(self.elapsed_ms))
        }
    }

    impl PaymentJournal for FullHost {
        fn load(
            &mut self,
            idempotency_key: &str,
            sink: &mut EconomicDocumentSink,
        ) -> EconomicJournalLoad {
            self.record_call("load");
            if !self.journals.contains_key(idempotency_key) {
                if let Some(directory) = std::env::var_os("SEMAPRAX_ECONOMIC_DURABLE_DIR") {
                    let path = std::path::PathBuf::from(directory).join("journal");
                    if let Ok(source) = std::fs::read_to_string(path) {
                        self.journals.insert(idempotency_key.to_owned(), source);
                    }
                }
            }
            match self.journals.get(idempotency_key) {
                Some(source) => {
                    assert!(sink.push(source.as_bytes()));
                    EconomicJournalLoad::Present
                }
                None => EconomicJournalLoad::Missing,
            }
        }

        fn compare_and_swap(
            &mut self,
            idempotency_key: &str,
            expected_version: u64,
            journal: &str,
            rolling: EconomicRollingReservationUpdate<'_>,
        ) -> EconomicAdapterDisposition {
            self.record_call("cas");
            self.rolling_updates.push(match rolling {
                EconomicRollingReservationUpdate::Reserve(_) => "reserve",
                EconomicRollingReservationUpdate::Retain => "retain",
                EconomicRollingReservationUpdate::Release => "release",
            });
            let cas_ordinal = self.calls.iter().filter(|call| **call == "cas").count();
            let fault = self
                .cas_fault
                .filter(|(ordinal, _, _)| *ordinal == cas_ordinal);
            if let Some((_, disposition, false)) = fault {
                return disposition;
            }
            let actual = self
                .journals
                .get(idempotency_key)
                .and_then(|source| serde_json::from_str::<Value>(source.trim_end()).ok())
                .and_then(|value| value["version"].as_u64())
                .unwrap_or(0);
            if actual != expected_version {
                return EconomicAdapterDisposition::FailedUncertain;
            }
            if expected_version == 0 {
                let EconomicRollingReservationUpdate::Reserve(reservation) = rolling else {
                    return EconomicAdapterDisposition::FailedUncertain;
                };
                assert_eq!(reservation.wallet_id(), "fixture.wallet");
                assert_eq!(reservation.rail(), self.intent.settlement_rail());
                let (network, asset) = self.intent.network_asset();
                assert_eq!(reservation.network(), network);
                assert_eq!(reservation.asset(), asset);
                assert_eq!(reservation.requested_at_ms(), self.intent.created_at);
                assert_eq!(reservation.amount_atomic(), self.intent.amount());
                assert_eq!(reservation.max_rolling_24h_atomic(), 1_000_000);
                let key = (
                    reservation.wallet_id().to_owned(),
                    reservation.rail().text().to_owned(),
                    reservation.network().to_owned(),
                    reservation.asset().to_owned(),
                );
                let rows = self.rolling.entry(key).or_default();
                rows.retain(|(_, admitted_at, _)| {
                    self.trusted_now_ms.saturating_sub(*admitted_at) < 86_400_000
                });
                let Some(total) = rows
                    .iter()
                    .try_fold(reservation.amount_atomic(), |sum, (_, _, amount)| {
                        sum.checked_add(*amount)
                    })
                else {
                    return EconomicAdapterDisposition::PolicyRejected;
                };
                if total > reservation.max_rolling_24h_atomic() {
                    return EconomicAdapterDisposition::PolicyRejected;
                }
                rows.push((
                    idempotency_key.to_owned(),
                    self.trusted_now_ms,
                    reservation.amount_atomic(),
                ));
            }
            self.journals
                .insert(idempotency_key.to_owned(), journal.to_owned());
            self.documents.insert("journal", journal.to_owned());
            let committed_version = serde_json::from_str::<Value>(journal.trim_end()).unwrap()
                ["version"]
                .as_u64()
                .unwrap();
            if self
                .cancel_after_version
                .as_ref()
                .is_some_and(|(version, _)| *version == committed_version)
            {
                self.cancel_after_version.as_ref().unwrap().1.cancel();
            }
            if let Some((version, elapsed)) = self.elapsed_after_version {
                if version == committed_version {
                    self.elapsed_ms = elapsed;
                }
            }
            if let Some(directory) = std::env::var_os("SEMAPRAX_ECONOMIC_DURABLE_DIR") {
                let directory = std::path::PathBuf::from(directory);
                std::fs::write(directory.join("journal"), journal).unwrap();
                if let Some(stage) = std::env::var_os("SEMAPRAX_ECONOMIC_KILL_STAGE") {
                    let value: Value = serde_json::from_str(journal.trim_end()).unwrap();
                    let version = value["version"].as_u64().unwrap();
                    let state = value["state"].as_str().unwrap();
                    let matches = match stage.to_str().unwrap() {
                        "v4" => version == 4,
                        "v5" => version == 5,
                        "v6" => version == 6,
                        "odd" => version >= 7 && version % 2 == 1 && state != "approved",
                        "even" => version >= 8 && version % 2 == 0,
                        _ => false,
                    };
                    if matches {
                        std::fs::write(directory.join("ready"), stage.to_string_lossy().as_bytes())
                            .unwrap();
                        loop {
                            std::thread::sleep(std::time::Duration::from_millis(20));
                        }
                    }
                }
            }
            fault.map_or(
                EconomicAdapterDisposition::Succeeded,
                |(_, disposition, _)| disposition,
            )
        }
    }

    impl X402InvoiceAdapter for FullHost {
        fn fetch_invoice(
            &mut self,
            origin: &str,
            method: &str,
            resource: &str,
            sink: &mut EconomicDocumentSink,
        ) -> EconomicAdapterDisposition {
            self.record_call("invoice");
            let Some(invoice) = &self.invoice else {
                return EconomicAdapterDisposition::DefinitelyNotStarted;
            };
            assert_eq!(
                (origin, method, resource),
                (&*invoice.origin, &*invoice.method, &*invoice.resource)
            );
            assert!(sink.push(invoice.doc.source.as_bytes()));
            EconomicAdapterDisposition::Succeeded
        }
    }

    impl EvmPaymentAdapter for FullHost {
        fn evm_snapshot(
            &mut self,
            _: &str,
            sink: &mut EconomicDocumentSink,
        ) -> EconomicAdapterDisposition {
            self.record_call("snapshot");
            let snapshot = Snapshot {
                rail: EconomicRail::Evm,
                observed: self.intent.created_at + 1,
                expires: self.intent.expires_at - 1,
                state: SnapshotState::Evm {
                    from: "0x2222222222222222222222222222222222222222".to_owned(),
                    nonce: 7,
                    base_fee: 1,
                    priority: 2,
                    gas: 21_000,
                },
                doc: Doc {
                    source: String::new(),
                    digest: String::new(),
                },
            };
            let source = render_snapshot(&snapshot);
            self.documents.insert("snapshot", source.clone());
            assert!(sink.push(source.as_bytes()));
            EconomicAdapterDisposition::Succeeded
        }

        fn evm_simulate(
            &mut self,
            plan: &str,
            _: &[u8],
            sink: &mut EconomicDocumentSink,
        ) -> EconomicAdapterDisposition {
            self.simulation(plan, sink);
            EconomicAdapterDisposition::Succeeded
        }

        fn evm_broadcast(
            &mut self,
            signed: &[u8],
            sink: &mut EconomicDocumentSink,
        ) -> EconomicAdapterDisposition {
            self.broadcast(signed, sink);
            self.broadcast_disposition
        }

        fn evm_reconcile(
            &mut self,
            transaction_id: &str,
            sink: &mut EconomicDocumentSink,
        ) -> EconomicAdapterDisposition {
            self.reconciliation(transaction_id, sink);
            EconomicAdapterDisposition::Succeeded
        }
    }

    impl SolanaPaymentAdapter for FullHost {
        fn solana_snapshot(
            &mut self,
            _: &str,
            sink: &mut EconomicDocumentSink,
        ) -> EconomicAdapterDisposition {
            self.record_call("snapshot");
            let snapshot = Snapshot {
                rail: EconomicRail::Solana,
                observed: self.intent.created_at + 1,
                expires: self.intent.expires_at - 1,
                state: SnapshotState::Solana {
                    payer: encode_base58(&[2; 32]),
                    blockhash: encode_base58(&[4; 32]),
                    last_height: 5,
                    fee: 5_000,
                },
                doc: Doc {
                    source: String::new(),
                    digest: String::new(),
                },
            };
            let source = render_snapshot(&snapshot);
            self.documents.insert("snapshot", source.clone());
            assert!(sink.push(source.as_bytes()));
            EconomicAdapterDisposition::Succeeded
        }

        fn solana_simulate(
            &mut self,
            plan: &str,
            _: &[u8],
            sink: &mut EconomicDocumentSink,
        ) -> EconomicAdapterDisposition {
            self.simulation(plan, sink);
            EconomicAdapterDisposition::Succeeded
        }

        fn solana_broadcast(
            &mut self,
            signed: &[u8],
            sink: &mut EconomicDocumentSink,
        ) -> EconomicAdapterDisposition {
            self.broadcast(signed, sink);
            self.broadcast_disposition
        }

        fn solana_reconcile(
            &mut self,
            transaction_id: &str,
            sink: &mut EconomicDocumentSink,
        ) -> EconomicAdapterDisposition {
            self.reconciliation(transaction_id, sink);
            EconomicAdapterDisposition::Succeeded
        }
    }

    impl BitcoinPaymentAdapter for FullHost {
        fn bitcoin_snapshot(
            &mut self,
            _: &str,
            sink: &mut EconomicDocumentSink,
        ) -> EconomicAdapterDisposition {
            self.record_call("snapshot");
            let snapshot = Snapshot {
                rail: EconomicRail::Bitcoin,
                observed: self.intent.created_at + 1,
                expires: self.intent.expires_at - 1,
                state: SnapshotState::Bitcoin {
                    wallet_script: format!("0014{}", "11".repeat(20)),
                    height: 100,
                    fee_rate: 1,
                    utxos: vec![Utxo {
                        txid: format!("{}01", "00".repeat(31)),
                        vout: 0,
                        value: 100_000,
                        script: format!("0014{}", "11".repeat(20)),
                        confirmations: 1,
                    }],
                },
                doc: Doc {
                    source: String::new(),
                    digest: String::new(),
                },
            };
            let source = render_snapshot(&snapshot);
            self.documents.insert("snapshot", source.clone());
            assert!(sink.push(source.as_bytes()));
            EconomicAdapterDisposition::Succeeded
        }

        fn bitcoin_simulate(
            &mut self,
            plan: &str,
            _: &[u8],
            sink: &mut EconomicDocumentSink,
        ) -> EconomicAdapterDisposition {
            self.simulation(plan, sink);
            EconomicAdapterDisposition::Succeeded
        }

        fn bitcoin_broadcast(
            &mut self,
            signed: &[u8],
            sink: &mut EconomicDocumentSink,
        ) -> EconomicAdapterDisposition {
            self.broadcast(signed, sink);
            self.broadcast_disposition
        }

        fn bitcoin_reconcile(
            &mut self,
            transaction_id: &str,
            sink: &mut EconomicDocumentSink,
        ) -> EconomicAdapterDisposition {
            self.reconciliation(transaction_id, sink);
            EconomicAdapterDisposition::Succeeded
        }
    }

    impl PaymentApprover for FullHost {
        fn approve(
            &mut self,
            request: &str,
            sink: &mut EconomicDocumentSink,
        ) -> EconomicAdapterDisposition {
            self.record_call("approve");
            let value: Value = serde_json::from_str(request.trim_end()).unwrap();
            let ref_text = |name: &str| {
                let row = value[name].as_object().unwrap();
                format!(
                    "{{\"schema\":{},\"digest\":{},\"bytes\":{}}}",
                    quote_json(row["schema"].as_str().unwrap()),
                    quote_json(row["digest"].as_str().unwrap()),
                    row["bytes"].as_u64().unwrap(),
                )
            };
            let request_doc = Doc {
                source: request.to_owned(),
                digest: digest(APPROVAL_REQUEST_DOMAIN, request.as_bytes()),
            };
            let source = format!(
                "{{\"schema\":\"{APPROVAL_SCHEMA}\",\"approval_id\":\"fixture.approval\",\"approver_id\":\"fixture.approver\",\"policy\":{},\"intent\":{},\"plan\":{},\"simulation\":{},\"approval_request\":{},\"decision\":\"approved\",\"approved_amount_atomic\":{},\"approved_fee_atomic\":{},\"expires_at_ms\":{}}}\n",
                ref_text("policy"), ref_text("intent"), ref_text("plan"), ref_text("simulation"),
                doc_ref(APPROVAL_REQUEST_SCHEMA, &request_doc),
                value["amount_atomic"].as_u64().unwrap(), value["max_fee_atomic"].as_u64().unwrap(), value["expires_at_ms"].as_u64().unwrap(),
            );
            self.documents
                .insert("approval_request", request.to_owned());
            self.documents.insert("approval", source.clone());
            assert!(sink.push(source.as_bytes()));
            EconomicAdapterDisposition::Succeeded
        }
    }

    impl WalletCustody for FullHost {
        fn sign(
            &mut self,
            _: &str,
            _: EconomicRail,
            _: &str,
            unsigned: &[u8],
            _: &str,
            sink: &mut EconomicBytesSink,
        ) -> EconomicAdapterDisposition {
            self.record_call("sign");
            let signed = match self.intent.settlement_rail() {
                EconomicRail::Evm => {
                    let mut fields = rlp_list_items(&unsigned[1..])
                        .unwrap()
                        .into_iter()
                        .map(<[u8]>::to_vec)
                        .collect::<Vec<_>>();
                    fields.extend([rlp_u64(1), rlp_u64(1), rlp_u64(1)]);
                    let mut signed = vec![2];
                    signed.extend(rlp_list(&fields));
                    signed
                }
                EconomicRail::Solana => {
                    let mut signed = vec![1];
                    signed.extend([7; 64]);
                    signed.extend(unsigned);
                    signed
                }
                EconomicRail::Bitcoin => {
                    let template = parse_psbt_template(unsigned).unwrap();
                    let mut signed = 2i32.to_le_bytes().to_vec();
                    signed.extend([0, 1]);
                    signed.extend(compact_size(template.inputs.len()));
                    for input in &template.inputs {
                        signed.extend(input.txid);
                        signed.extend(input.vout.to_le_bytes());
                        signed.push(0);
                        signed.extend(input.sequence.to_le_bytes());
                    }
                    signed.extend(compact_size(template.outputs.len()));
                    for output in &template.outputs {
                        signed.extend(output.value.to_le_bytes());
                        signed.extend(compact_size(output.script.len()));
                        signed.extend(&output.script);
                    }
                    let signature = [0x30, 0x06, 0x02, 0x01, 0x01, 0x02, 0x01, 0x01, 0x01];
                    for _ in &template.inputs {
                        signed.push(2);
                        signed.extend(compact_size(signature.len()));
                        signed.extend(signature);
                        signed.push(33);
                        signed.extend([2]);
                        signed.extend([1; 32]);
                    }
                    signed.extend(template.locktime.to_le_bytes());
                    signed
                }
            };
            assert!(sink.push(&signed));
            self.stop_after_effect_if_requested("sign_effect");
            EconomicAdapterDisposition::Succeeded
        }
    }

    #[test]
    fn full_evm_authority_route_is_ordered_and_self_replayed() {
        let policy = evm_policy();
        let intent = evm_intent();
        let source = crate::agent_runtime::completed_run_for_economic_test(&intent.source);
        let mut agent = EconomicAgent::new(
            &policy.source,
            FullHost::new(intent),
            AgentCancellation::new(),
        )
        .unwrap();
        let run = agent.execute(&source).unwrap();
        assert_eq!(run.status(), EconomicRunStatus::Confirmed);
        assert!(run.transaction_id().is_some());
        assert_eq!(run.confirmation_status(), Some("confirmed"));
        assert!(run.trace().ends_with('\n'));
        assert!(run.evidence().contains("\"used_builder_bytes\":"));
        assert_eq!(
            run.trace_digest(),
            "sha256:ce7bec5f627a6d48990573353370dc0953203153f0db2ab60a6101cc9a5146d0"
        );
        assert_eq!(
            run.evidence_digest(),
            digest(EVIDENCE_DOMAIN, run.evidence().as_bytes())
        );
        assert_eq!(
            agent.host.calls,
            [
                "load",
                "cas",
                "snapshot",
                "simulate",
                "cas",
                "approve",
                "cas",
                "cas",
                "sign",
                "cas",
                "cas",
                "broadcast",
                "cas",
                "cas",
                "reconcile",
                "cas"
            ]
        );
    }

    #[test]
    fn solana_bitcoin_and_x402_routes_are_chain_distinct_and_self_replayed() {
        for rail in [EconomicRail::Solana, EconomicRail::Bitcoin] {
            let (policy, intent) = rail_fixture(rail);
            let source = crate::agent_runtime::completed_run_for_economic_test(&intent.source);
            let mut agent = EconomicAgent::new(
                &policy.source,
                FullHost::new(intent),
                AgentCancellation::new(),
            )
            .unwrap();
            let run = agent.execute(&source).unwrap();
            assert_eq!(run.status(), EconomicRunStatus::Confirmed, "{rail:?}");
            assert_eq!(run.confirmation_status(), Some("confirmed"));
            assert!(run.transaction_id().is_some());
            assert!(run.trace().ends_with('\n'));
            assert!(run.evidence().ends_with('\n'));
            assert_eq!(
                agent.host.calls,
                [
                    "load",
                    "cas",
                    "snapshot",
                    "simulate",
                    "cas",
                    "approve",
                    "cas",
                    "cas",
                    "sign",
                    "cas",
                    "cas",
                    "broadcast",
                    "cas",
                    "cas",
                    "reconcile",
                    "cas"
                ]
            );
        }

        let (policy, intent, invoice) = x402_fixture();
        let source = crate::agent_runtime::completed_run_for_economic_test(&intent.source);
        let mut agent = EconomicAgent::new(
            &policy.source,
            FullHost::with_invoice(intent, invoice),
            AgentCancellation::new(),
        )
        .unwrap();
        let run = agent.execute(&source).unwrap();
        assert_eq!(run.status(), EconomicRunStatus::Confirmed);
        assert_eq!(run.confirmation_status(), Some("confirmed"));
        assert_eq!(agent.host.calls[2], "invoice");
        assert_eq!(
            agent
                .host
                .calls
                .iter()
                .filter(|call| **call == "invoice")
                .count(),
            1
        );
    }

    #[test]
    fn uncertain_broadcast_is_never_retried_and_restart_reconciles_retained_capsule() {
        let (policy, intent) = rail_fixture(EconomicRail::Evm);
        let source = crate::agent_runtime::completed_run_for_economic_test(&intent.source);
        let mut host = FullHost::new(intent.clone());
        host.broadcast_disposition = EconomicAdapterDisposition::FailedUncertain;
        let mut first = EconomicAgent::new(&policy.source, host, AgentCancellation::new()).unwrap();
        let first_run = first.execute(&source).unwrap();
        assert_eq!(first_run.status(), EconomicRunStatus::BroadcastUnknown);
        assert_eq!(
            first
                .host
                .calls
                .iter()
                .filter(|call| **call == "broadcast")
                .count(),
            1
        );
        let journals = std::mem::take(&mut first.host.journals);
        let mut restart_host = FullHost::new(intent.clone());
        restart_host.journals = journals;
        let mut restart =
            EconomicAgent::new(&policy.source, restart_host, AgentCancellation::new()).unwrap();
        let reconciled = restart.reconcile(&intent.idempotency_key, &source).unwrap();
        assert_eq!(reconciled.status(), EconomicRunStatus::Confirmed);
        assert_eq!(
            restart
                .host
                .calls
                .iter()
                .filter(|call| **call == "broadcast")
                .count(),
            0
        );
        assert_eq!(
            restart
                .host
                .calls
                .iter()
                .filter(|call| **call == "sign")
                .count(),
            0
        );
        assert_eq!(restart.host.calls, ["load", "cas", "reconcile", "cas"]);
    }

    #[test]
    fn rolling_window_uses_trusted_admission_time_and_expires_at_exact_24h() {
        let (_, mut intent) = rail_fixture(EconomicRail::Evm);
        intent.payment = Payment::Evm {
            recipient: "0x1111111111111111111111111111111111111111".to_owned(),
            amount: 600_000,
            max_fee: 100_000,
        };
        intent.source = render_intent(&intent);
        intent.digest = digest(INTENT_DOMAIN, intent.source.as_bytes());
        let mut host = FullHost::new(intent.clone());
        host.trusted_now_ms = intent.created_at + 300_000;
        let reservation = EconomicRollingReservation {
            wallet_id: "fixture.wallet".to_owned(),
            rail: EconomicRail::Evm,
            network: "sepolia".to_owned(),
            asset: "native:eth".to_owned(),
            requested_at_ms: intent.created_at,
            amount_atomic: intent.amount(),
            max_rolling_24h_atomic: 1_000_000,
        };
        assert_eq!(
            host.compare_and_swap(
                "rolling.first",
                0,
                "{\"version\":1}\n",
                EconomicRollingReservationUpdate::Reserve(&reservation),
            ),
            EconomicAdapterDisposition::Succeeded
        );
        let inventory = host.journals.clone();
        host.trusted_now_ms += 86_399_999;
        assert_eq!(
            host.compare_and_swap(
                "rolling.second",
                0,
                "{\"version\":1}\n",
                EconomicRollingReservationUpdate::Reserve(&reservation),
            ),
            EconomicAdapterDisposition::PolicyRejected
        );
        assert_eq!(host.journals, inventory);
        host.trusted_now_ms += 1;
        assert_eq!(
            host.compare_and_swap(
                "rolling.second",
                0,
                "{\"version\":1}\n",
                EconomicRollingReservationUpdate::Reserve(&reservation),
            ),
            EconomicAdapterDisposition::Succeeded
        );
        assert_eq!(host.rolling.values().flatten().count(), 1);
        assert_eq!(
            host.rolling.values().next().unwrap()[0].1,
            intent.created_at + 300_000 + 86_400_000
        );
    }

    #[test]
    fn rolling_window_distinct_keys_race_to_one_atomic_winner() {
        let (_, mut intent) = rail_fixture(EconomicRail::Evm);
        intent.payment = Payment::Evm {
            recipient: "0x1111111111111111111111111111111111111111".to_owned(),
            amount: 600_000,
            max_fee: 100_000,
        };
        let host = std::sync::Arc::new(std::sync::Mutex::new(FullHost::new(intent.clone())));
        let barrier = std::sync::Arc::new(std::sync::Barrier::new(3));
        let mut workers = Vec::new();
        for key in ["rolling.race.a", "rolling.race.b"] {
            let host = std::sync::Arc::clone(&host);
            let barrier = std::sync::Arc::clone(&barrier);
            let intent = intent.clone();
            workers.push(std::thread::spawn(move || {
                let reservation = EconomicRollingReservation {
                    wallet_id: "fixture.wallet".to_owned(),
                    rail: EconomicRail::Evm,
                    network: "sepolia".to_owned(),
                    asset: "native:eth".to_owned(),
                    requested_at_ms: intent.created_at,
                    amount_atomic: intent.amount(),
                    max_rolling_24h_atomic: 1_000_000,
                };
                barrier.wait();
                host.lock().unwrap().compare_and_swap(
                    key,
                    0,
                    "{\"version\":1}\n",
                    EconomicRollingReservationUpdate::Reserve(&reservation),
                )
            }));
        }
        barrier.wait();
        let dispositions = workers
            .into_iter()
            .map(|worker| worker.join().unwrap())
            .collect::<Vec<_>>();
        assert_eq!(
            dispositions
                .iter()
                .filter(|value| **value == EconomicAdapterDisposition::Succeeded)
                .count(),
            1
        );
        assert_eq!(
            dispositions
                .iter()
                .filter(|value| **value == EconomicAdapterDisposition::PolicyRejected)
                .count(),
            1
        );
    }

    #[test]
    fn malformed_post_effect_adapter_output_is_terminal_replayable_and_secret_free() {
        let (policy, intent) = rail_fixture(EconomicRail::Evm);
        let source = crate::agent_runtime::completed_run_for_economic_test(&intent.source);
        let mut host = FullHost::new(intent);
        host.malformed_simulation = true;
        let mut agent = EconomicAgent::new(&policy.source, host, AgentCancellation::new()).unwrap();
        let run = agent.execute(&source).unwrap();
        assert_eq!(run.status(), EconomicRunStatus::AdapterFailed);
        assert!(run.trace().contains("SPX-G210"));
        assert!(run.evidence().contains("SPX-G210"));
        assert!(!run.trace().contains("economic-secret-sentinel"));
        assert!(!run.evidence().contains("economic-secret-sentinel"));
        assert_eq!(
            agent.host.calls,
            ["load", "cas", "snapshot", "simulate", "cas"]
        );
    }

    #[test]
    fn pre_effect_cancellation_is_diagnostic_only_and_invokes_no_authority() {
        let (policy, intent) = rail_fixture(EconomicRail::Evm);
        let source = crate::agent_runtime::completed_run_for_economic_test(&intent.source);
        let cancellation = AgentCancellation::new();
        cancellation.cancel();
        let mut agent =
            EconomicAgent::new(&policy.source, FullHost::new(intent), cancellation).unwrap();
        let diagnostics = match agent.execute(&source) {
            Ok(_) => panic!("pre-effect cancellation unexpectedly returned Evidence"),
            Err(diagnostics) => diagnostics,
        };
        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].code, "SPX-I228");
        assert_eq!(diagnostics[0].message, "Economic Agent run was cancelled");
        assert!(agent.host.calls.is_empty());
        assert!(agent.host.journals.is_empty());
    }

    #[test]
    fn cancellation_and_deadline_after_durable_markers_block_the_next_effect() {
        for version in [4, 6] {
            let (policy, intent) = rail_fixture(EconomicRail::Evm);
            let source = crate::agent_runtime::completed_run_for_economic_test(&intent.source);
            let cancellation = AgentCancellation::new();
            let mut host = FullHost::new(intent);
            host.cancel_after_version = Some((version, cancellation.clone()));
            let mut agent = EconomicAgent::new(&policy.source, host, cancellation).unwrap();
            let run = agent.execute(&source).unwrap();
            assert_eq!(run.status(), EconomicRunStatus::Cancelled);
            if version == 4 {
                assert!(!agent.host.calls.contains(&"sign"));
            } else {
                assert!(!agent.host.calls.contains(&"broadcast"));
            }
        }
        let (policy, intent) = rail_fixture(EconomicRail::Evm);
        let source = crate::agent_runtime::completed_run_for_economic_test(&intent.source);
        let mut host = FullHost::new(intent);
        host.elapsed_after_version = Some((4, policy.limits.max_elapsed_ms + 1));
        let mut agent = EconomicAgent::new(&policy.source, host, AgentCancellation::new()).unwrap();
        let run = agent.execute(&source).unwrap();
        assert_eq!(run.status(), EconomicRunStatus::DeadlineExceeded);
        assert!(!agent.host.calls.contains(&"sign"));
    }

    #[test]
    fn chain_documents_reject_key_order_schema_reference_and_identity_mutations() {
        let intent = evm_intent();
        let mut snapshot = Snapshot {
            rail: EconomicRail::Evm,
            observed: intent.created_at + 1,
            expires: intent.expires_at - 1,
            state: SnapshotState::Evm {
                from: "0x2222222222222222222222222222222222222222".to_owned(),
                nonce: 7,
                base_fee: 1,
                priority: 2,
                gas: 21_000,
            },
            doc: Doc {
                source: String::new(),
                digest: String::new(),
            },
        };
        snapshot.doc.source = render_snapshot(&snapshot);
        snapshot.doc.digest = digest(SNAPSHOT_DOMAIN, snapshot.doc.source.as_bytes());
        assert!(parse_snapshot(&snapshot.doc.source, EconomicRail::Evm).is_ok());
        for hostile in [
            snapshot.doc.source.replace(
                "\"schema\":\"semaprax.economic-agent-chain-snapshot.v1\",\"rail\":\"evm\"",
                "\"rail\":\"evm\",\"schema\":\"semaprax.economic-agent-chain-snapshot.v1\"",
            ),
            snapshot
                .doc
                .source
                .replace("\"network\":\"sepolia\"", "\"network\":\"devnet\""),
        ] {
            assert!(parse_snapshot(&hostile, EconomicRail::Evm).is_err());
        }
        let mutated = parse_snapshot(
            &snapshot.doc.source.replace("\"nonce\":7", "\"nonce\":8"),
            EconomicRail::Evm,
        )
        .unwrap();
        assert!(matches!(mutated.state, SnapshotState::Evm { nonce: 8, .. }));

        let (unsigned, _) = build_unsigned(&intent, &snapshot).unwrap();
        let mut fields = rlp_list_items(&unsigned[1..])
            .unwrap()
            .into_iter()
            .map(<[u8]>::to_vec)
            .collect::<Vec<_>>();
        fields.extend([rlp_u64(1), rlp_u64(1), rlp_u64(1)]);
        let mut signed = vec![2];
        signed.extend(rlp_list(&fields));
        let signed_digest = digest(SIGNED_DOMAIN, &signed);
        let txid = transaction_id(EconomicRail::Evm, &signed).unwrap();
        let broadcast = format!(
            "{{\"schema\":\"{BROADCAST_SCHEMA}\",\"rail\":\"evm\",\"network\":\"sepolia\",\"signed_transaction_digest\":{},\"transaction_id\":{},\"disposition\":\"accepted\",\"observed_at_ms\":{}}}\n",
            quote_json(&signed_digest),
            quote_json(&txid),
            intent.created_at + 2,
        );
        assert!(parse_broadcast(
            &broadcast,
            EconomicRail::Evm,
            "sepolia",
            &signed_digest,
            Some(&txid)
        )
        .is_ok());
        for hostile in [
            broadcast.replace(
                &txid,
                "0x0000000000000000000000000000000000000000000000000000000000000000",
            ),
            broadcast.replace(
                &signed_digest,
                "sha256:0000000000000000000000000000000000000000000000000000000000000000",
            ),
        ] {
            assert!(parse_broadcast(
                &hostile,
                EconomicRail::Evm,
                "sepolia",
                &signed_digest,
                Some(&txid)
            )
            .is_err());
        }
        assert_eq!(
            parse_broadcast(
                &broadcast.replace(
                    "\"disposition\":\"accepted\"",
                    "\"disposition\":\"rejected\""
                ),
                EconomicRail::Evm,
                "sepolia",
                &signed_digest,
                Some(&txid)
            )
            .unwrap()
            .disposition,
            "rejected"
        );

        let reconciliation = format!(
            "{{\"schema\":\"{RECONCILIATION_SCHEMA}\",\"rail\":\"evm\",\"network\":\"sepolia\",\"transaction_id\":{},\"status\":\"confirmed\",\"observed_at_ms\":{},\"observed_height\":1,\"confirmations\":1,\"canonical_block_id\":\"fixture.block\"}}\n",
            quote_json(&txid),
            intent.created_at + 3,
        );
        assert!(parse_reconciliation(&reconciliation, EconomicRail::Evm, "sepolia", &txid).is_ok());
        for hostile in [
            reconciliation.replace("\"confirmations\":1", "\"confirmations\":null"),
            reconciliation.replace("\"status\":\"confirmed\"", "\"status\":\"unknown\""),
            reconciliation.replace("\"network\":\"sepolia\"", "\"network\":\"devnet\""),
        ] {
            assert!(parse_reconciliation(&hostile, EconomicRail::Evm, "sepolia", &txid).is_err());
        }
    }

    #[test]
    fn configured_child_limits_are_exact_and_lower_than_global_caps() {
        let (mut policy, intent, invoice) = x402_fixture();
        policy.limits.max_intent_bytes = intent.source.len() as u64;
        assert!(admit_intent(&policy, &intent).is_ok());
        policy.limits.max_intent_bytes -= 1;
        let diagnostic = admit_intent(&policy, &intent).unwrap_err();
        assert_eq!(diagnostic.code, "SPX-G216");
        assert_eq!(
            diagnostic.message,
            format!("intent_bytes exceeds {}", intent.source.len() - 1)
        );

        policy.limits.max_intent_bytes = intent.source.len() as u64;
        policy.limits.max_identifier_bytes = intent.idempotency_key.len() as u64;
        assert!(admit_intent(&policy, &intent).is_ok());
        policy.limits.max_identifier_bytes -= 1;
        assert_eq!(admit_intent(&policy, &intent).unwrap_err().code, "SPX-G216");

        policy.limits.max_identifier_bytes = MAX_IDENTIFIER_BYTES as u64;
        let intent_depth = depth(&serde_json::from_str::<Value>(intent.source.trim_end()).unwrap());
        policy.limits.max_json_depth = intent_depth as u64;
        assert!(admit_intent(&policy, &intent).is_ok());
        policy.limits.max_json_depth -= 1;
        assert_eq!(admit_intent(&policy, &intent).unwrap_err().code, "SPX-G216");

        let mut invoice_limits = limits();
        invoice_limits.max_invoice_bytes = invoice.doc.source.len() as u64;
        assert!(parse_invoice_limited(&invoice.doc.source, &intent, &invoice_limits).is_ok());
        invoice_limits.max_invoice_bytes -= 1;
        let diagnostic = match parse_invoice_limited(&invoice.doc.source, &intent, &invoice_limits)
        {
            Ok(_) => panic!("over-limit invoice was admitted"),
            Err(diagnostic) => diagnostic,
        };
        assert_eq!(diagnostic.code, "SPX-G216");

        let mut snapshot = Snapshot {
            rail: EconomicRail::Bitcoin,
            observed: intent.created_at + 1,
            expires: intent.expires_at - 1,
            state: SnapshotState::Bitcoin {
                wallet_script: format!("0014{}", "11".repeat(20)),
                height: 100,
                fee_rate: 1,
                utxos: vec![
                    Utxo {
                        txid: format!("{}01", "00".repeat(31)),
                        vout: 0,
                        value: 100_000,
                        script: format!("0014{}", "11".repeat(20)),
                        confirmations: 1,
                    },
                    Utxo {
                        txid: format!("{}02", "00".repeat(31)),
                        vout: 0,
                        value: 100_000,
                        script: format!("0014{}", "11".repeat(20)),
                        confirmations: 1,
                    },
                ],
            },
            doc: Doc {
                source: String::new(),
                digest: String::new(),
            },
        };
        snapshot.doc.source = render_snapshot(&snapshot);
        snapshot.doc.digest = digest(SNAPSHOT_DOMAIN, snapshot.doc.source.as_bytes());
        let mut snapshot_limits = limits();
        snapshot_limits.max_snapshot_bytes = snapshot.doc.source.len() as u64;
        snapshot_limits.max_utxos = 2;
        assert!(parse_snapshot_limited(
            &snapshot.doc.source,
            EconomicRail::Bitcoin,
            &snapshot_limits
        )
        .is_ok());
        snapshot_limits.max_utxos = 1;
        let diagnostic = match parse_snapshot_limited(
            &snapshot.doc.source,
            EconomicRail::Bitcoin,
            &snapshot_limits,
        ) {
            Ok(_) => panic!("over-limit UTXO set was admitted"),
            Err(diagnostic) => diagnostic,
        };
        assert_eq!(diagnostic.code, "SPX-G216");
        assert_eq!(diagnostic.message, "utxos exceeds 1");
        snapshot_limits.max_utxos = 2;
        snapshot_limits.max_snapshot_bytes -= 1;
        let diagnostic = match parse_snapshot_limited(
            &snapshot.doc.source,
            EconomicRail::Bitcoin,
            &snapshot_limits,
        ) {
            Ok(_) => panic!("over-limit snapshot was admitted"),
            Err(diagnostic) => diagnostic,
        };
        assert_eq!(diagnostic.code, "SPX-G216");
    }

    #[test]
    fn thirteen_document_x402_raw_sha_and_domain_digest_ledger_is_pinned() {
        let (policy, intent, invoice) = x402_fixture();
        let source = crate::agent_runtime::completed_run_for_economic_test(&intent.source);
        let mut agent = EconomicAgent::new(
            &policy.source,
            FullHost::with_invoice(intent.clone(), invoice.clone()),
            AgentCancellation::new(),
        )
        .unwrap();
        let run = agent.execute(&source).unwrap();
        let mut documents = vec![
            ("policy", policy.source.as_str(), POLICY_DOMAIN),
            ("intent", intent.source.as_str(), INTENT_DOMAIN),
            ("invoice", invoice.doc.source.as_str(), INVOICE_DOMAIN),
        ];
        for (name, domain) in [
            ("snapshot", SNAPSHOT_DOMAIN),
            ("plan", PLAN_DOMAIN),
            ("simulation", SIMULATION_DOMAIN),
            ("approval_request", APPROVAL_REQUEST_DOMAIN),
            ("approval", APPROVAL_DOMAIN),
            ("journal", JOURNAL_DOMAIN),
            ("broadcast", BROADCAST_DOMAIN),
            ("reconciliation", RECONCILIATION_DOMAIN),
        ] {
            documents.push((name, agent.host.documents[name].as_str(), domain));
        }
        documents.extend([
            ("trace", run.trace(), TRACE_DOMAIN),
            ("evidence", run.evidence(), EVIDENCE_DOMAIN),
        ]);
        assert_eq!(documents.len(), 13);
        let ledger = documents
            .iter()
            .map(|(name, source, domain)| {
                let raw = Sha256::digest(source.as_bytes());
                format!(
                    "{name}|{}|{}|{}",
                    raw.iter()
                        .map(|byte| format!("{byte:02x}"))
                        .collect::<String>(),
                    digest(domain, source.as_bytes()),
                    source.len()
                )
            })
            .collect::<Vec<_>>();
        assert_eq!(ledger, [
            "policy|57ce4d3844f49c9102eb1a2c17f1946305c623e587d4f52a31744ba96ff6114a|sha256:ee623062817928e0088f24b8215705f9aad8e19a52861db6d6051679889c0b53|2987",
            "intent|bfe0695c7e2a5bdfd545b264fb79777cfdadaa449d9089c59753ae3739e36d86|sha256:2a13c2a14cfafba6b4087e647de9e5609c8bb65ddad25c305aa8f5bc28091e2c|670",
            "invoice|38b5b00511f2e461f8df0fe1a830e89109376c893e3c52cea5d23a8d36d8733b|sha256:24cb1025c6beb2a081a05ab504f7d7f6cbb37b27e003da35cbbea003a52ac095|417",
            "snapshot|d005d0f573f337d804d80b8489b63a9f6b03099837b230af69e18a4692b4b9eb|sha256:4123e22e7449e4bbcef812af71337f2e3e5390b4cce20e59f7080b74eeb727d0|309",
            "plan|75418fad0967fa4791d9f146f6997af67b3e76f51dcb2320bfbe2211814bde45|sha256:81dad7aa8e82bdf8ef7b02e2b5a94b899715c86e7d82895c7cdb18c5e7ed28d8|1391",
            "simulation|b3508d24fd29028a9fad89703ba72ade9f4e620eec30f0dd2017b711b96db483|sha256:3a1ac9d741be20bf0d5e35a78e475369a5ccad5c3662bbc5f5365f123df81f1d|369",
            "approval_request|fc932dcef1eb518ba05f463df9e3dd7193ce96df408edb60f3aaa9214a3f19b9|sha256:0833d896f4be4e4feb08d0558e43e7512d589a6c96c368c6ce73ef3a8435adf1|1056",
            "approval|48f716162ae5ec67b28303c5e5c09b641a16a1711b7b83bd4d16a1be6094a56c|sha256:63f3e81facdc9e0ece43b28cbd47310b1a7898662cd4afe3abaae24d06eab8db|1022",
            "journal|f65a8f115c405b086d9a6edb1366a594c87b9f295be8739ea2a56724297f69c9|sha256:9f3d1f568a090c280cfe645536e912d4eb0c18c740bb7309d434ebfd1d1cb169|2394",
            "broadcast|51479da80d60c4e4c363302963010a6675278e53958ce563dafb2892da3c537f|sha256:64882648d5bac5fb58a7408d38e5fa737ba314d27decba34e2106c2310c651a6|335",
            "reconciliation|b1b449375018c27465332384d67205438bea8a2660d3144c417e5de5d5198ba1|sha256:e26e7655d758b53867228950de241c3265675f18687110550fc89ecdc46f2b4a|301",
            "trace|a388543ab6c1a57a0b7798fbd0c5d721bb33c0ab7f7123f3bb8f24c4c965db58|sha256:f28c44894b93948068381bb9047fedade3b855cdd1831a992466a08fa97f6f11|11023",
            "evidence|2d4d4164476bd4fdd037f138b264d0a72728b125d6819baae87da165242788b0|sha256:9dd80e5a13aaaa02b5b854cee0f68870ac22dfdabca14e79d32857ae35980cc6|17399",
        ]);
        for (name, source, domain) in documents {
            let mut mutated = source.as_bytes().to_vec();
            let index = mutated.iter().position(|byte| *byte == b'v').unwrap();
            mutated[index] = b'w';
            assert_ne!(
                Sha256::digest(source.as_bytes()),
                Sha256::digest(&mutated),
                "{name}"
            );
            assert_ne!(
                digest(domain, source.as_bytes()),
                digest(domain, &mutated),
                "{name}"
            );
        }
    }

    #[test]
    fn journal_uncertainty_never_retries_in_process_and_reload_governs_persistence() {
        for persisted in [false, true] {
            let (policy, intent) = rail_fixture(EconomicRail::Evm);
            let source = crate::agent_runtime::completed_run_for_economic_test(&intent.source);
            let mut host = FullHost::new(intent.clone());
            host.cas_fault = Some((2, EconomicAdapterDisposition::FailedUncertain, persisted));
            let mut agent =
                EconomicAgent::new(&policy.source, host, AgentCancellation::new()).unwrap();
            let run = agent.execute(&source).unwrap();
            assert_eq!(run.status(), EconomicRunStatus::JournalFailed);
            assert_eq!(
                agent
                    .host
                    .calls
                    .iter()
                    .filter(|call| **call == "cas")
                    .count(),
                2
            );
            assert_eq!(
                agent
                    .host
                    .calls
                    .iter()
                    .filter(|call| **call == "approve")
                    .count(),
                0
            );
            let journals = std::mem::take(&mut agent.host.journals);
            let retained_version =
                serde_json::from_str::<Value>(journals[&intent.idempotency_key].trim_end())
                    .unwrap()["version"]
                    .as_u64()
                    .unwrap();
            assert_eq!(retained_version, if persisted { 2 } else { 1 });

            let mut restart_host = FullHost::new(intent.clone());
            restart_host.journals = journals;
            let mut restart =
                EconomicAgent::new(&policy.source, restart_host, AgentCancellation::new()).unwrap();
            let restarted = restart.execute(&source).unwrap();
            assert_eq!(
                restarted.status(),
                if persisted {
                    EconomicRunStatus::JournalFailed
                } else {
                    EconomicRunStatus::Confirmed
                }
            );
            assert_eq!(restart.host.calls.contains(&"snapshot"), !persisted);
            if persisted {
                assert_eq!(restart.host.calls, ["load"]);
                assert!(restart.host.rolling_updates.is_empty());
            } else {
                assert!(restart
                    .host
                    .rolling_updates
                    .iter()
                    .all(|update| *update == "retain"));
            }
        }

        let (policy, intent) = rail_fixture(EconomicRail::Evm);
        let source = crate::agent_runtime::completed_run_for_economic_test(&intent.source);
        let mut host = FullHost::new(intent);
        host.cas_fault = Some((1, EconomicAdapterDisposition::DefinitelyNotStarted, false));
        let mut agent = EconomicAgent::new(&policy.source, host, AgentCancellation::new()).unwrap();
        let run = agent.execute(&source).unwrap();
        assert_eq!(run.status(), EconomicRunStatus::JournalFailed);
        assert_eq!(agent.host.calls, ["load", "cas"]);
        assert!(agent.host.journals.is_empty());
        assert!(agent.host.rolling.values().all(Vec::is_empty));
    }

    #[test]
    fn economic_process_kill_markers_never_repeat_sign_or_broadcast() {
        const ROLE: &str = "SEMAPRAX_ECONOMIC_KILL_ROLE";
        const DIRECTORY: &str = "SEMAPRAX_ECONOMIC_DURABLE_DIR";
        const STAGE: &str = "SEMAPRAX_ECONOMIC_KILL_STAGE";
        if std::env::var_os(ROLE).is_some() {
            let (policy, intent) = rail_fixture(EconomicRail::Evm);
            let source = crate::agent_runtime::completed_run_for_economic_test(&intent.source);
            let mut agent = EconomicAgent::new(
                &policy.source,
                FullHost::new(intent),
                AgentCancellation::new(),
            )
            .unwrap();
            let result = agent.execute(&source);
            if std::env::var_os(STAGE).is_none() {
                assert!(result.is_ok());
            }
            return;
        }
        let executable = std::env::current_exe().unwrap();
        for stage in [
            "v4",
            "sign_effect",
            "v5",
            "v6",
            "broadcast_effect",
            "odd",
            "even",
        ] {
            let directory = std::env::temp_dir().join(format!(
                "semaprax-economic-kill-{}-{stage}",
                std::process::id()
            ));
            std::fs::create_dir(&directory).unwrap();
            let mut child = std::process::Command::new(&executable)
                .args([
                    "economic_agent::tests::economic_process_kill_markers_never_repeat_sign_or_broadcast",
                    "--exact",
                    "--nocapture",
                ])
                .env(ROLE, "child")
                .env(DIRECTORY, &directory)
                .env(STAGE, stage)
                .spawn()
                .unwrap();
            let ready = directory.join("ready");
            let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
            while !ready.exists() && std::time::Instant::now() < deadline {
                std::thread::sleep(std::time::Duration::from_millis(20));
            }
            assert!(ready.exists(), "child did not reach {stage}");
            child.kill().unwrap();
            let _ = child.wait().unwrap();
            let status = std::process::Command::new(&executable)
                .args([
                    "economic_agent::tests::economic_process_kill_markers_never_repeat_sign_or_broadcast",
                    "--exact",
                    "--nocapture",
                ])
                .env(ROLE, "resume")
                .env(DIRECTORY, &directory)
                .status()
                .unwrap();
            assert!(status.success(), "resume failed at {stage}");
            let calls = std::fs::read_to_string(directory.join("calls")).unwrap();
            let sign_calls = calls.lines().filter(|call| *call == "sign").count();
            let broadcast_calls = calls.lines().filter(|call| *call == "broadcast").count();
            assert!(sign_calls <= 1);
            assert!(broadcast_calls <= 1);
            if stage == "sign_effect" {
                assert_eq!(sign_calls, 1);
                assert_eq!(broadcast_calls, 0);
                let journal = std::fs::read_to_string(directory.join("journal")).unwrap();
                let value: Value = serde_json::from_str(journal.trim_end()).unwrap();
                assert_eq!(value["version"], 4);
                assert_eq!(value["state"], "approved");
            }
            if stage == "broadcast_effect" {
                assert_eq!(sign_calls, 1);
                assert_eq!(broadcast_calls, 1);
                let journal = std::fs::read_to_string(directory.join("journal")).unwrap();
                let value: Value = serde_json::from_str(journal.trim_end()).unwrap();
                assert_eq!(value["state"], "confirmed");
                assert!(value["version"].as_u64().unwrap() >= 8);
            }
            for name in ["ready", "journal", "calls"] {
                let path = directory.join(name);
                if path.exists() {
                    std::fs::remove_file(path).unwrap();
                }
            }
            std::fs::remove_dir(directory).unwrap();
        }
    }

    #[test]
    fn reconciliation_authority_is_durably_bounded_at_exact_sixty_four() {
        let (policy, intent) = rail_fixture(EconomicRail::Evm);
        let idempotency = intent.idempotency_key.clone();
        let source = crate::agent_runtime::completed_run_for_economic_test(&intent.source);
        let mut host = FullHost::new(intent);
        host.reconciliation_status = "pending";
        let mut agent = EconomicAgent::new(&policy.source, host, AgentCancellation::new()).unwrap();
        let first = agent.execute(&source).unwrap();
        assert_eq!(first.status(), EconomicRunStatus::Pending);
        for _ in 1..64 {
            let observation = agent.reconcile(&idempotency, &source).unwrap();
            assert_eq!(observation.status(), EconomicRunStatus::Pending);
        }
        let calls = agent
            .host
            .calls
            .iter()
            .filter(|call| **call == "reconcile")
            .count();
        assert_eq!(calls, 64);
        let exhausted = agent.reconcile(&idempotency, &source).unwrap();
        assert_eq!(exhausted.status(), EconomicRunStatus::BudgetExhausted);
        assert_eq!(
            agent
                .host
                .calls
                .iter()
                .filter(|call| **call == "reconcile")
                .count(),
            64
        );
    }
}
