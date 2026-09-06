//! The opaque agent checkpoint document.
//!
//! A checkpoint is a self-describing canonical JSON document plus its
//! domain-separated digest. It carries **identities, digests, counters and a
//! journal** — never an authorization, never a state carrier, never a task or
//! proposal payload, and never a credential.
//!
//! # Retention and redaction
//!
//! The only external datum a checkpoint may retain is the settled observation
//! of the single registered read operation, and only under the explicit
//! [`Retention::ObservationBytes`] mode. Caller-supplied inputs — the task
//! objective and the model proposal document — are retained as digests only,
//! at every generation and in every mode, so a secret in a caller input never
//! reaches checkpoint bytes. Under [`Retention::ObservationDigestOnly`] the
//! observation is reduced to its digest as well, and a resumed run then cannot
//! complete without an explicit host reconciliation that reproduces it.
//!
//! # Storage contract
//!
//! Checkpoint bytes are self-verifying: decoding requires the document to
//! reparse under a closed key set, the journal chain to recompute, the program
//! counter to agree with the journal, and the canonical re-rendering to equal
//! the supplied bytes exactly. A partially written generation therefore fails
//! to decode rather than being adopted. Atomicity itself is the caller's
//! [`super::CheckpointStore`] contract; this module supplies the detection.

use serde_json::{Map, Value};
use sha2::{Digest, Sha256};

use crate::diagnostic::{quote_json, Diagnostic};

use super::journal::{self, JournalEntry};

/// Schema identity of the opaque checkpoint document.
pub const CHECKPOINT_SCHEMA: &str = "semaprax.agent-checkpoint.v1";

pub(super) const CHECKPOINT_DOMAIN: &[u8] = b"semaprax.agent-checkpoint.digest.v1\0";
pub(super) const STATE_DOMAIN: &[u8] = b"semaprax.agent-checkpoint.state.v1\0";
pub(super) const PROPOSAL_DOMAIN: &[u8] = b"semaprax.agent-checkpoint.proposal.v1\0";
pub(super) const OBSERVATION_DOMAIN: &[u8] = b"semaprax.agent-checkpoint.observation.v1\0";
pub(super) const RESULT_DOMAIN: &[u8] = b"semaprax.agent-checkpoint.result.v1\0";
pub(super) const SOURCE_DOMAIN: &[u8] = b"semaprax.agent-checkpoint.source.v1\0";
pub(super) const TASK_DOMAIN: &[u8] = b"semaprax.agent-checkpoint.task.v1\0";

const MAX_CHECKPOINT_BYTES: usize = 262_144;
const MAX_JOURNAL_ENTRIES: usize = 8;

/// The nonclaims every checkpoint document republishes.
pub(super) const NONCLAIMS: [&str; 8] = [
    "no_authorization_value_seal_or_credential_in_a_checkpoint",
    "no_checkpoint_integrity_or_authenticity_without_the_callers_storage_contract",
    "no_automatic_retry_of_an_uncertain_external_operation",
    "no_exactly_once_external_execution_or_provider_billing_claim",
    "no_budget_refund_on_resume",
    "no_effect_beyond_the_single_registered_read_operation",
    "no_ambient_filesystem_process_network_home_or_secret_authority",
    "no_cli_surface_in_this_slice",
];

pub(super) fn malformed() -> Diagnostic {
    Diagnostic::io(
        "SPX-G573",
        "AgentCheckpoint bytes are not one exact canonical checkpoint generation".to_owned(),
    )
}

pub(super) fn digest(domain: &[u8], bytes: &[u8]) -> String {
    let mut hash = Sha256::new();
    hash.update(domain);
    hash.update(bytes);
    format!("sha256:{:x}", crate::digest_hex::LowerHex(hash.finalize()))
}

/// Where one durable run has advanced to. Derived from the journal and stored
/// explicitly; a disagreement between the two rejects the checkpoint.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProgramCounter {
    /// The deterministic prefix settled. No external boundary approached.
    Prefix,
    /// An intent is durable and the operation's delivery is uncertain.
    Intent,
    /// The operation settled.
    Settled,
    /// The host reconciled the operation to abandonment. Terminal.
    Abandoned,
    /// The reduction published a result that was not yet delivered.
    Reduced,
    /// The result was delivered. Terminal.
    Delivered,
}

impl ProgramCounter {
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Prefix => "prefix",
            Self::Intent => "intent",
            Self::Settled => "settled",
            Self::Abandoned => "abandoned",
            Self::Reduced => "reduced",
            Self::Delivered => "delivered",
        }
    }

    fn parse(name: &str) -> Option<Self> {
        Some(match name {
            "prefix" => Self::Prefix,
            "intent" => Self::Intent,
            "settled" => Self::Settled,
            "abandoned" => Self::Abandoned,
            "reduced" => Self::Reduced,
            "delivered" => Self::Delivered,
            _ => return None,
        })
    }

    pub(super) const fn of(entry: &JournalEntry) -> Self {
        match entry {
            JournalEntry::Prefix { .. } => Self::Prefix,
            JournalEntry::Intent { .. } => Self::Intent,
            JournalEntry::Settled { .. } => Self::Settled,
            JournalEntry::Abandoned { .. } => Self::Abandoned,
            JournalEntry::Reduced { .. } => Self::Reduced,
            JournalEntry::Delivered { .. } => Self::Delivered,
        }
    }
}

/// The retention mode one durable run writes its checkpoints under.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum Retention {
    /// Retain the settled observation's bytes, so a resumed run can complete
    /// without contacting the external boundary again.
    #[default]
    ObservationBytes,
    /// Retain only the settled observation's digest. A resumed run then needs
    /// an explicit host reconciliation that reproduces the same digest.
    ObservationDigestOnly,
}

impl Retention {
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::ObservationBytes => "observation_bytes",
            Self::ObservationDigestOnly => "observation_digest_only",
        }
    }

    fn parse(name: &str) -> Option<Self> {
        Some(match name {
            "observation_bytes" => Self::ObservationBytes,
            "observation_digest_only" => Self::ObservationDigestOnly,
            _ => return None,
        })
    }
}

/// The revision a checkpoint is bound to.
///
/// Every field is an identity or a digest of something the live invocation
/// recomputes for itself. A resumed run compares all nine and refuses a
/// checkpoint that does not match on any of them.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CheckpointBinding {
    pub(super) bound_digest: String,
    pub(super) definition_digest: String,
    pub(super) deployment_digest: String,
    pub(super) source_digest: String,
    pub(super) lifecycle_digest: String,
    pub(super) proposal_schema_digest: String,
    pub(super) state_schema: String,
    pub(super) policy_epoch: u64,
}

impl CheckpointBinding {
    /// The exact drift a mismatch against `live` amounts to, if any.
    ///
    /// Ordered so that each cause is independently reachable: a revoked epoch
    /// reports before any content digest, a deployment substitution before the
    /// bound product it changes, and a pure display rename — which changes the
    /// source and nothing else — reports last as source drift.
    pub(super) fn drift(&self, live: &Self) -> Option<&'static str> {
        if self.policy_epoch != live.policy_epoch {
            return Some("policy_epoch_revoked");
        }
        if self.definition_digest != live.definition_digest {
            return Some("definition_drift");
        }
        if self.deployment_digest != live.deployment_digest {
            return Some("deployment_drift");
        }
        if self.bound_digest != live.bound_digest {
            return Some("bound_deployment_drift");
        }
        if self.state_schema != live.state_schema {
            return Some("state_schema_drift");
        }
        if self.proposal_schema_digest != live.proposal_schema_digest {
            return Some("proposal_schema_drift");
        }
        if self.lifecycle_digest != live.lifecycle_digest {
            return Some("lifecycle_drift");
        }
        if self.source_digest != live.source_digest {
            return Some("source_drift");
        }
        None
    }
}

/// The budget ledgers one durable run carries forward.
///
/// Neither ledger is ever refunded. Re-running the deterministic prefix on
/// resume spends interpreter fuel from the same remaining total, and an effect
/// grant consumed at an intent stays consumed whatever the operation's fate.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CheckpointBudgets {
    pub(super) total_steps_remaining: usize,
    pub(super) max_steps_per_stage: usize,
    pub(super) effect_grants_remaining: usize,
}

/// One opaque, immutable checkpoint generation.
///
/// It has no public constructor and no `Clone`: a checkpoint is produced by a
/// durable run and reconstructed only by [`AgentCheckpoint::decode`] from
/// bytes that reproduce it exactly.
pub struct AgentCheckpoint {
    generation: u64,
    agent_id: String,
    binding: CheckpointBinding,
    counter: ProgramCounter,
    budgets: CheckpointBudgets,
    retention: Retention,
    task_digest: String,
    journal: Vec<JournalEntry>,
    link: String,
    document: String,
    digest: String,
}

impl AgentCheckpoint {
    pub(super) fn seal(
        generation: u64,
        agent_id: &str,
        binding: CheckpointBinding,
        budgets: CheckpointBudgets,
        retention: Retention,
        task_digest: &str,
        journal: Vec<JournalEntry>,
    ) -> Result<Self, Vec<Diagnostic>> {
        let Some(last) = journal.last() else {
            return Err(vec![malformed()]);
        };
        let counter = ProgramCounter::of(last);
        let link = journal::chain(&journal);
        let document = render(
            generation,
            agent_id,
            &binding,
            counter,
            &budgets,
            retention,
            task_digest,
            &journal,
            &link,
        );
        if document.len() > MAX_CHECKPOINT_BYTES {
            return Err(vec![malformed()]);
        }
        Ok(Self {
            generation,
            agent_id: agent_id.to_owned(),
            binding,
            counter,
            budgets,
            retention,
            task_digest: task_digest.to_owned(),
            journal,
            link,
            digest: digest(CHECKPOINT_DOMAIN, document.as_bytes()),
            document,
        })
    }

    /// Reconstructs one checkpoint from stored bytes.
    ///
    /// Recovery never adopts a partial generation: the bytes must reparse
    /// under the closed key set, the journal must decode with strictly
    /// advancing ranks, the recomputed chain link must equal the stored one,
    /// the stored program counter must agree with the journal's last entry,
    /// and the canonical re-rendering must equal the supplied bytes exactly.
    pub fn decode(document: &str) -> Result<Self, Vec<Diagnostic>> {
        if document.len() > MAX_CHECKPOINT_BYTES || !document.ends_with('\n') {
            return Err(vec![malformed()]);
        }
        let parsed = parse(&document[..document.len() - 1]).ok_or_else(|| vec![malformed()])?;
        let sealed = Self::seal(
            parsed.generation,
            &parsed.agent_id,
            parsed.binding,
            parsed.budgets,
            parsed.retention,
            &parsed.task_digest,
            parsed.journal,
        )?;
        if sealed.document != document
            || sealed.link != parsed.link
            || sealed.counter != parsed.counter
        {
            return Err(vec![malformed()]);
        }
        Ok(sealed)
    }

    /// The canonical checkpoint document, including its terminal LF.
    #[must_use]
    pub fn document(&self) -> &str {
        &self.document
    }

    /// The domain-separated checkpoint digest.
    #[must_use]
    pub fn digest(&self) -> &str {
        &self.digest
    }

    /// The monotonically increasing generation this checkpoint published.
    #[must_use]
    pub const fn generation(&self) -> u64 {
        self.generation
    }

    #[must_use]
    pub fn agent_id(&self) -> &str {
        &self.agent_id
    }

    /// How far the durable run advanced.
    #[must_use]
    pub const fn program_counter(&self) -> ProgramCounter {
        self.counter
    }

    /// The retention mode the writing run declared.
    #[must_use]
    pub const fn retention(&self) -> Retention {
        self.retention
    }

    /// The stable identity of the external operation this checkpoint names,
    /// once an intent exists. It is an identifier, never a capability: no API
    /// accepts one in place of an authorization.
    #[must_use]
    pub fn operation_identity(&self) -> Option<&str> {
        self.journal.iter().rev().find_map(JournalEntry::operation)
    }

    /// The head of the journal's hash chain.
    #[must_use]
    pub fn journal_link(&self) -> &str {
        &self.link
    }

    /// How many effect grants remain. Never increases across a resume.
    #[must_use]
    pub const fn effect_grants_remaining(&self) -> usize {
        self.budgets.effect_grants_remaining
    }

    /// How much interpreter fuel remains for the whole durable run.
    #[must_use]
    pub const fn total_steps_remaining(&self) -> usize {
        self.budgets.total_steps_remaining
    }

    pub(super) const fn binding(&self) -> &CheckpointBinding {
        &self.binding
    }

    pub(super) const fn budgets(&self) -> CheckpointBudgets {
        self.budgets
    }

    pub(super) fn task_digest(&self) -> &str {
        &self.task_digest
    }

    pub(super) fn journal(&self) -> &[JournalEntry] {
        &self.journal
    }
}

struct Parsed {
    generation: u64,
    agent_id: String,
    binding: CheckpointBinding,
    counter: ProgramCounter,
    budgets: CheckpointBudgets,
    retention: Retention,
    task_digest: String,
    journal: Vec<JournalEntry>,
    link: String,
}

#[allow(clippy::too_many_arguments)]
fn render(
    generation: u64,
    agent_id: &str,
    binding: &CheckpointBinding,
    counter: ProgramCounter,
    budgets: &CheckpointBudgets,
    retention: Retention,
    task_digest: &str,
    journal: &[JournalEntry],
    link: &str,
) -> String {
    let mut output = format!(
        "{{\"schema\":{},\"generation\":{},\"agent_id\":{},\"binding\":{{\"bound_digest\":{},\"definition_digest\":{},\"deployment_digest\":{},\"source_digest\":{},\"lifecycle_digest\":{},\"proposal_schema_digest\":{},\"state_schema\":{},\"policy_epoch\":{}}},\"program_counter\":{},\"budgets\":{{\"total_steps_remaining\":{},\"max_steps_per_stage\":{},\"effect_grants_remaining\":{}}},\"retention\":{},\"task_digest\":{},\"journal\":{},\"journal_link\":{},\"nonclaims\":[",
        quote_json(CHECKPOINT_SCHEMA),
        quote_json(&generation.to_string()),
        quote_json(agent_id),
        quote_json(&binding.bound_digest),
        quote_json(&binding.definition_digest),
        quote_json(&binding.deployment_digest),
        quote_json(&binding.source_digest),
        quote_json(&binding.lifecycle_digest),
        quote_json(&binding.proposal_schema_digest),
        quote_json(&binding.state_schema),
        quote_json(&binding.policy_epoch.to_string()),
        quote_json(counter.name()),
        quote_json(&budgets.total_steps_remaining.to_string()),
        quote_json(&budgets.max_steps_per_stage.to_string()),
        quote_json(&budgets.effect_grants_remaining.to_string()),
        quote_json(retention.name()),
        quote_json(task_digest),
        journal::render(journal),
        quote_json(link)
    );
    for (index, nonclaim) in NONCLAIMS.iter().enumerate() {
        if index > 0 {
            output.push(',');
        }
        output.push_str(&quote_json(nonclaim));
    }
    output.push_str("]}\n");
    output
}

fn text<'a>(map: &'a Map<String, Value>, key: &str) -> Option<&'a str> {
    map.get(key)?.as_str()
}

fn closed(map: &Map<String, Value>, keys: &[&str]) -> Option<()> {
    (map.len() == keys.len() && keys.iter().all(|key| map.contains_key(*key))).then_some(())
}

fn parse(body: &str) -> Option<Parsed> {
    let value: Value = serde_json::from_str(body).ok()?;
    let map = value.as_object()?;
    closed(
        map,
        &[
            "schema",
            "generation",
            "agent_id",
            "binding",
            "program_counter",
            "budgets",
            "retention",
            "task_digest",
            "journal",
            "journal_link",
            "nonclaims",
        ],
    )?;
    if text(map, "schema")? != CHECKPOINT_SCHEMA {
        return None;
    }
    let claims = map.get("nonclaims")?.as_array()?;
    if claims.len() != NONCLAIMS.len()
        || claims
            .iter()
            .zip(NONCLAIMS)
            .any(|(claim, expected)| claim.as_str() != Some(expected))
    {
        return None;
    }
    let binding = map.get("binding")?.as_object()?;
    closed(
        binding,
        &[
            "bound_digest",
            "definition_digest",
            "deployment_digest",
            "source_digest",
            "lifecycle_digest",
            "proposal_schema_digest",
            "state_schema",
            "policy_epoch",
        ],
    )?;
    let budgets = map.get("budgets")?.as_object()?;
    closed(
        budgets,
        &[
            "total_steps_remaining",
            "max_steps_per_stage",
            "effect_grants_remaining",
        ],
    )?;
    Some(Parsed {
        generation: text(map, "generation")?.parse().ok()?,
        agent_id: text(map, "agent_id")?.to_owned(),
        binding: CheckpointBinding {
            bound_digest: text(binding, "bound_digest")?.to_owned(),
            definition_digest: text(binding, "definition_digest")?.to_owned(),
            deployment_digest: text(binding, "deployment_digest")?.to_owned(),
            source_digest: text(binding, "source_digest")?.to_owned(),
            lifecycle_digest: text(binding, "lifecycle_digest")?.to_owned(),
            proposal_schema_digest: text(binding, "proposal_schema_digest")?.to_owned(),
            state_schema: text(binding, "state_schema")?.to_owned(),
            policy_epoch: text(binding, "policy_epoch")?.parse().ok()?,
        },
        counter: ProgramCounter::parse(text(map, "program_counter")?)?,
        budgets: CheckpointBudgets {
            total_steps_remaining: text(budgets, "total_steps_remaining")?.parse().ok()?,
            max_steps_per_stage: text(budgets, "max_steps_per_stage")?.parse().ok()?,
            effect_grants_remaining: text(budgets, "effect_grants_remaining")?.parse().ok()?,
        },
        retention: Retention::parse(text(map, "retention")?)?,
        task_digest: text(map, "task_digest")?.to_owned(),
        journal: journal::decode(map.get("journal")?, MAX_JOURNAL_ENTRIES)?,
        link: text(map, "journal_link")?.to_owned(),
    })
}
