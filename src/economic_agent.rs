//! Bounded Economic Agent v1 injected-host API.
//!
//! This safe-Rust state machine has no built-in transport, DNS, filesystem,
//! process, environment, key, custody, journal, or wallet implementation.
//! Caller implementations of the host traits are trusted authorities; all
//! adapter documents and bytes remain untrusted input.
//!
//! This file owns the shared vocabulary: schema and domain-separation
//! constants, the public host traits and opaque handles, and the private
//! policy, intent, budget, event, and terminal types every stage reads. The
//! submodules divide the work by concern:
//!
//! - `validate`, `policy`, `address`, `intent`, `snapshot` — bounded
//!   parsing, rendering, and admission of the input documents.
//! - `transaction` — unsigned transaction construction and signed-byte
//!   verification per rail.
//! - `documents`, `journal` — the invoice, plan, simulation, approval,
//!   broadcast, and reconciliation documents, and the journal that records
//!   them.
//! - `evidence`, `replay` — trace and evidence rendering, replay of a
//!   rendered run, and run finalization.
//! - `agent_core`, `agent_execute`, `agent_reconcile` — the driver
//!   itself, split into construction and shared guards, the forward execution
//!   path, and the reconciliation and resume paths.

#![forbid(unsafe_code)]
#![allow(
    dead_code,
    clippy::field_reassign_with_default,
    clippy::format_collect,
    clippy::too_many_arguments,
    reason = "private typed and replay internals support the opaque public C surface"
)]

mod address;
mod agent_core;
mod agent_execute;
mod agent_reconcile;
mod documents;
mod evidence;
mod intent;
mod journal;
mod policy;
mod replay;
mod snapshot;
mod transaction;
mod validate;

use crate::agent_runtime::AgentCancellation;
use crate::bounded_output::{reserve_active, reserve_active_preserving, with_limit_usage};
use crate::diagnostic::Diagnostic;

use self::validate::{g210, g216, info};

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

fn admit_policy_source(source: &str) -> Result<(Policy, usize), Vec<Diagnostic>> {
    let (result, overflowed, consumed) = with_limit_usage(MAX_BUILDER_BYTES, || {
        if !reserve_active(source.len().saturating_mul(MAX_JSON_DEPTH + 2)) {
            return Err(g216("builder_bytes", MAX_BUILDER_BYTES as u64));
        }
        policy::parse_policy(source)
    });
    if overflowed {
        return Err(vec![g216("builder_bytes", MAX_BUILDER_BYTES as u64)]);
    }
    result
        .map(|policy| (policy, consumed))
        .map_err(|diagnostic| vec![diagnostic])
}

/// Validates one canonical Economic Agent Policy and returns its
/// domain-separated digest without consulting a host or acquiring authority.
pub fn economic_agent_policy_digest(source: &str) -> Result<String, Vec<Diagnostic>> {
    admit_policy_source(source).map(|(policy, _)| policy.digest)
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

// The unit tests reach submodule internals through this module. This block
// stays last so the ambient-authority scan in `tests/economic_agent_v1.rs`,
// which stops at the first `#[cfg(test)]`, still covers every line above.
#[cfg(test)]
use {
    self::address::{convert_bits, decode_base58_32, decode_regtest_p2wpkh, encode_base58},
    self::documents::{
        doc_ref, parse_invoice_limited, parse_simulation, render_invoice, Invoice, Plan,
    },
    self::intent::{admit_intent, parse_intent, render_intent},
    self::journal::{parse_broadcast, parse_reconciliation},
    self::policy::{parse_policy, render_policy, valid_origin, valid_resource},
    self::snapshot::{
        parse_snapshot, parse_snapshot_limited, render_snapshot, rlp_list, rlp_u64, Snapshot,
        SnapshotState, Utxo,
    },
    self::transaction::{
        build_unsigned, compact_size, keccak256, parse_psbt_template, rlp_list_items,
        transaction_id, verify_signed,
    },
    self::validate::{depth, digest},
    crate::agent_runtime::AgentRunStatus,
    crate::diagnostic::quote_json,
    serde_json::Value,
    sha2::{Digest, Sha256},
};

#[cfg(test)]
mod tests;
