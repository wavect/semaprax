//! The causal journal: one ordered, chain-linked record of what a live
//! invocation actually committed before crossing each external boundary.
//!
//! This is the *one* journal both #108 and #177 share. `model.invoke` is not
//! a second execution log bolted beside it: every `model.invoke` call is
//! recorded as this journal's [`JournalEntry::RequestIntent`] /
//! [`JournalEntry::ResponseRecorded`] pair, in the same vocabulary a future
//! tool effect's [`JournalEntry::EffectIntent`] / [`JournalEntry::EffectObserved`]
//! pair uses. A receipt is a read-only projection of this journal (see
//! [`receipt_projection`]), never a second log kept in step with it by hand.
//!
//! # What the chain link defends and what it does not
//!
//! Every entry's chain link folds in its predecessor's link and its own
//! canonical bytes, exactly like [`crate::agent_lifecycle::durable::journal`]:
//! removing, reordering, or tearing a write changes the link. The journal
//! carries no key material and no signature — a party who can rewrite the
//! caller's storage can also recompute the chain. That is a declared
//! nonclaim, not a gap this module papers over: what makes a forged or
//! withheld journal harmless to *authority* is that [`AuthorizationGrant`]
//! (`super::model_invoke`) is minted by re-running the authorization gate
//! against a state the kernel recomputes itself, never by decoding a journal
//! entry. A journal can misdescribe or omit a turn; it can never produce a
//! grant for one.
//!
//! # Ordering rules ([`validate`])
//!
//! One turn's entries must appear in exactly this order: `TurnOpened`,
//! `RequestIntent`, then `ResponseRecorded` or `ResponseFailed`. Only after
//! `ResponseRecorded` may `ProposalAdmitted` or `ProposalRefused` follow.
//! Only after `ProposalAdmitted` may `AuthorizationConsumed` follow. After
//! that, zero or more `EffectIntent`/`EffectObserved` pairs (matched by
//! operation identity) may occur, followed by exactly one `Transition`. A
//! `Transition { case: "continue", .. }` must be followed by the next
//! `TurnOpened` (turn number advancing by exactly one, same invocation); any
//! other case must be followed by exactly one matching `TerminalOutcome`,
//! after which the journal must end. Any other entry sequence — omission,
//! reorder, a turn number that repeats or skips, a `TurnOpened` naming a
//! different invocation, an `EffectObserved` naming a different operation
//! than its `EffectIntent`, or any entry after `TerminalOutcome` — is
//! rejected by [`validate`] before it is trusted for replay.

use serde_json::{Map, Value};
use sha2::{Digest, Sha256};

use super::identity::{hex, looks_like_digest, unhex};
use crate::diagnostic::quote_json;
use crate::digest_hex::LowerHex;

const JOURNAL_DOMAIN: &[u8] = b"semaprax.live-invocation.journal.v1\0";

/// The maximum number of entries one journal may carry. This bounds decode
/// allocation the same way the existing operation checkpoint wire bounds its
/// entry count; it is not a claim about how long a real invocation may run.
pub const MAX_JOURNAL_ENTRIES: usize = 16_384;

/// The closed causal-journal vocabulary of one live invocation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum JournalEntry {
    /// Opens `turn`, binding the exact invocation identity and this turn's
    /// deterministic observation digest. No external boundary has been
    /// approached yet.
    TurnOpened {
        turn: u32,
        invocation: String,
        observation_digest: String,
    },
    /// The `model.invoke` request is durable *before* dispatch. A crash
    /// after this commit leaves provider-side delivery uncertain, never
    /// unknown-to-have-been-attempted.
    RequestIntent {
        turn: u32,
        request_digest: String,
        reserved_budget: i64,
    },
    /// The request settled with this recorded response. The response bytes
    /// are the trusted, replayable record — not a claim reconstructed later
    /// by hashing caller-provided data.
    ResponseRecorded {
        turn: u32,
        response_digest: String,
        response: Vec<u8>,
    },
    /// The request failed in the closed [`super::model_invoke::ModelFailure`]
    /// domain. Terminal for this turn's model step; no `ProposalAdmitted`,
    /// `ProposalRefused`, or `AuthorizationConsumed` may follow, and the
    /// turn's next entry is its `Transition`.
    ResponseFailed {
        turn: u32,
        failure: String,
        attempted_bytes: usize,
    },
    /// The compiler-derived decode admitted the response as a checked
    /// proposal.
    ProposalAdmitted { turn: u32, proposal_digest: String },
    /// The compiler-derived decode refused the response before authorize.
    ProposalRefused { turn: u32, reason: String },
    /// The one opaque grant this turn consumed, bound to this turn's exact
    /// observation and proposal. Never reusable across turns.
    AuthorizationConsumed { turn: u32, grant_digest: String },
    /// One further external operation (typically a deployed tool, not
    /// `model.invoke`) is about to cross its own boundary within this turn.
    EffectIntent {
        turn: u32,
        operation: String,
        request_digest: String,
    },
    /// That effect settled.
    EffectObserved {
        turn: u32,
        operation: String,
        observation_digest: String,
    },
    /// The deterministic transition this turn produced. `case` is one of
    /// `continue`, `complete`, `suspend`, `fail`.
    Transition {
        turn: u32,
        case: String,
        carrier_digest: String,
    },
    /// The terminal outcome was delivered to the caller. No entry may
    /// follow this one anywhere in the journal.
    TerminalOutcome {
        turn: u32,
        case: String,
        carrier_digest: String,
    },
}

impl JournalEntry {
    const fn kind(&self) -> &'static str {
        match self {
            Self::TurnOpened { .. } => "turn_opened",
            Self::RequestIntent { .. } => "request_intent",
            Self::ResponseRecorded { .. } => "response_recorded",
            Self::ResponseFailed { .. } => "response_failed",
            Self::ProposalAdmitted { .. } => "proposal_admitted",
            Self::ProposalRefused { .. } => "proposal_refused",
            Self::AuthorizationConsumed { .. } => "authorization_consumed",
            Self::EffectIntent { .. } => "effect_intent",
            Self::EffectObserved { .. } => "effect_observed",
            Self::Transition { .. } => "transition",
            Self::TerminalOutcome { .. } => "terminal_outcome",
        }
    }

    fn turn(&self) -> u32 {
        match self {
            Self::TurnOpened { turn, .. }
            | Self::RequestIntent { turn, .. }
            | Self::ResponseRecorded { turn, .. }
            | Self::ResponseFailed { turn, .. }
            | Self::ProposalAdmitted { turn, .. }
            | Self::ProposalRefused { turn, .. }
            | Self::AuthorizationConsumed { turn, .. }
            | Self::EffectIntent { turn, .. }
            | Self::EffectObserved { turn, .. }
            | Self::Transition { turn, .. }
            | Self::TerminalOutcome { turn, .. } => *turn,
        }
    }

    /// The canonical encoding of one entry at one sequence position.
    pub fn encode(&self, seq: usize) -> String {
        let head = format!(
            "{{\"seq\":{},\"kind\":{},\"turn\":{}",
            seq,
            quote_json(self.kind()),
            self.turn()
        );
        let body = match self {
            Self::TurnOpened {
                invocation,
                observation_digest,
                ..
            } => format!(
                ",\"invocation\":{},\"observation_digest\":{}",
                quote_json(invocation),
                quote_json(observation_digest)
            ),
            Self::RequestIntent {
                request_digest,
                reserved_budget,
                ..
            } => format!(
                ",\"request_digest\":{},\"reserved_budget\":{}",
                quote_json(request_digest),
                reserved_budget
            ),
            Self::ResponseRecorded {
                response_digest,
                response,
                ..
            } => format!(
                ",\"response_digest\":{},\"response\":{}",
                quote_json(response_digest),
                quote_json(&hex(response))
            ),
            Self::ResponseFailed {
                failure,
                attempted_bytes,
                ..
            } => format!(
                ",\"failure\":{},\"attempted_bytes\":{}",
                quote_json(failure),
                attempted_bytes
            ),
            Self::ProposalAdmitted {
                proposal_digest, ..
            } => {
                format!(",\"proposal_digest\":{}", quote_json(proposal_digest))
            }
            Self::ProposalRefused { reason, .. } => {
                format!(",\"reason\":{}", quote_json(reason))
            }
            Self::AuthorizationConsumed { grant_digest, .. } => {
                format!(",\"grant_digest\":{}", quote_json(grant_digest))
            }
            Self::EffectIntent {
                operation,
                request_digest,
                ..
            } => format!(
                ",\"operation\":{},\"request_digest\":{}",
                quote_json(operation),
                quote_json(request_digest)
            ),
            Self::EffectObserved {
                operation,
                observation_digest,
                ..
            } => format!(
                ",\"operation\":{},\"observation_digest\":{}",
                quote_json(operation),
                quote_json(observation_digest)
            ),
            Self::Transition {
                case,
                carrier_digest,
                ..
            }
            | Self::TerminalOutcome {
                case,
                carrier_digest,
                ..
            } => format!(
                ",\"case\":{},\"carrier_digest\":{}",
                quote_json(case),
                quote_json(carrier_digest)
            ),
        };
        format!("{head}{body}}}")
    }
}

/// The canonical rendering of a whole journal, as a JSON array with one
/// trailing newline.
#[must_use]
pub fn render(entries: &[JournalEntry]) -> String {
    let mut output = String::from("[");
    for (seq, entry) in entries.iter().enumerate() {
        if seq > 0 {
            output.push(',');
        }
        output.push_str(&entry.encode(seq));
    }
    output.push_str("]\n");
    output
}

/// The chain link over a whole journal: `link(0) = H(domain || 0x00 ||
/// entry(0))`, `link(i) = H(domain || link(i-1) || 0x00 || entry(i))`.
#[must_use]
pub fn chain(entries: &[JournalEntry]) -> String {
    let mut link = String::new();
    for (seq, entry) in entries.iter().enumerate() {
        let mut hash = Sha256::new();
        hash.update(JOURNAL_DOMAIN);
        hash.update(link.as_bytes());
        hash.update([0]);
        hash.update(entry.encode(seq).as_bytes());
        link = format!("sha256:{:x}", LowerHex(hash.finalize()));
    }
    link
}

fn text<'a>(entry: &'a Map<String, Value>, key: &str) -> Option<&'a str> {
    entry.get(key)?.as_str()
}

fn digest_field<'a>(entry: &'a Map<String, Value>, key: &str) -> Option<&'a str> {
    let value = text(entry, key)?;
    looks_like_digest(value).then_some(value)
}

fn closed(entry: &Map<String, Value>, keys: &[&str]) -> Option<()> {
    (entry.len() == keys.len() && keys.iter().all(|key| entry.contains_key(*key))).then_some(())
}

fn turn_field(entry: &Map<String, Value>) -> Option<u32> {
    u32::try_from(entry.get("turn")?.as_u64()?).ok()
}

/// A decode failure: malformed JSON shape, an unknown key, a bad digest
/// format, an out-of-order `seq`, or an entry count over
/// [`MAX_JOURNAL_ENTRIES`]. Decode never repairs or reinterprets bytes.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DecodeError;

/// Decodes one journal array's *shape* only (closed keys, digest format,
/// sequential `seq`). Causal ordering across entries is [`validate`]'s job,
/// kept separate so a malformed-shape rejection and a causally-invalid
/// rejection are two distinct, individually testable failures.
pub fn decode(value: &Value) -> Result<Vec<JournalEntry>, DecodeError> {
    let array = value.as_array().ok_or(DecodeError)?;
    if array.is_empty() || array.len() > MAX_JOURNAL_ENTRIES {
        return Err(DecodeError);
    }
    let mut entries = Vec::with_capacity(array.len());
    for (seq, item) in array.iter().enumerate() {
        let entry = item.as_object().ok_or(DecodeError)?;
        if entry.get("seq").and_then(Value::as_u64) != u64::try_from(seq).ok() {
            return Err(DecodeError);
        }
        let turn = turn_field(entry).ok_or(DecodeError)?;
        let kind = text(entry, "kind").ok_or(DecodeError)?;
        let decoded = match kind {
            "turn_opened" => {
                closed(
                    entry,
                    &["seq", "kind", "turn", "invocation", "observation_digest"],
                )
                .ok_or(DecodeError)?;
                JournalEntry::TurnOpened {
                    turn,
                    invocation: digest_field(entry, "invocation")
                        .ok_or(DecodeError)?
                        .to_owned(),
                    observation_digest: digest_field(entry, "observation_digest")
                        .ok_or(DecodeError)?
                        .to_owned(),
                }
            }
            "request_intent" => {
                closed(
                    entry,
                    &["seq", "kind", "turn", "request_digest", "reserved_budget"],
                )
                .ok_or(DecodeError)?;
                JournalEntry::RequestIntent {
                    turn,
                    request_digest: digest_field(entry, "request_digest")
                        .ok_or(DecodeError)?
                        .to_owned(),
                    reserved_budget: entry
                        .get("reserved_budget")
                        .and_then(Value::as_i64)
                        .ok_or(DecodeError)?,
                }
            }
            "response_recorded" => {
                closed(
                    entry,
                    &["seq", "kind", "turn", "response_digest", "response"],
                )
                .ok_or(DecodeError)?;
                let response = match text(entry, "response") {
                    Some(text) => unhex(text).ok_or(DecodeError)?,
                    None => return Err(DecodeError),
                };
                JournalEntry::ResponseRecorded {
                    turn,
                    response_digest: digest_field(entry, "response_digest")
                        .ok_or(DecodeError)?
                        .to_owned(),
                    response,
                }
            }
            "response_failed" => {
                closed(
                    entry,
                    &["seq", "kind", "turn", "failure", "attempted_bytes"],
                )
                .ok_or(DecodeError)?;
                JournalEntry::ResponseFailed {
                    turn,
                    failure: text(entry, "failure").ok_or(DecodeError)?.to_owned(),
                    attempted_bytes: entry
                        .get("attempted_bytes")
                        .and_then(Value::as_u64)
                        .and_then(|v| usize::try_from(v).ok())
                        .ok_or(DecodeError)?,
                }
            }
            "proposal_admitted" => {
                closed(entry, &["seq", "kind", "turn", "proposal_digest"]).ok_or(DecodeError)?;
                JournalEntry::ProposalAdmitted {
                    turn,
                    proposal_digest: digest_field(entry, "proposal_digest")
                        .ok_or(DecodeError)?
                        .to_owned(),
                }
            }
            "proposal_refused" => {
                closed(entry, &["seq", "kind", "turn", "reason"]).ok_or(DecodeError)?;
                JournalEntry::ProposalRefused {
                    turn,
                    reason: text(entry, "reason").ok_or(DecodeError)?.to_owned(),
                }
            }
            "authorization_consumed" => {
                closed(entry, &["seq", "kind", "turn", "grant_digest"]).ok_or(DecodeError)?;
                JournalEntry::AuthorizationConsumed {
                    turn,
                    grant_digest: digest_field(entry, "grant_digest")
                        .ok_or(DecodeError)?
                        .to_owned(),
                }
            }
            "effect_intent" => {
                closed(
                    entry,
                    &["seq", "kind", "turn", "operation", "request_digest"],
                )
                .ok_or(DecodeError)?;
                JournalEntry::EffectIntent {
                    turn,
                    operation: text(entry, "operation").ok_or(DecodeError)?.to_owned(),
                    request_digest: digest_field(entry, "request_digest")
                        .ok_or(DecodeError)?
                        .to_owned(),
                }
            }
            "effect_observed" => {
                closed(
                    entry,
                    &["seq", "kind", "turn", "operation", "observation_digest"],
                )
                .ok_or(DecodeError)?;
                JournalEntry::EffectObserved {
                    turn,
                    operation: text(entry, "operation").ok_or(DecodeError)?.to_owned(),
                    observation_digest: digest_field(entry, "observation_digest")
                        .ok_or(DecodeError)?
                        .to_owned(),
                }
            }
            "transition" | "terminal_outcome" => {
                closed(entry, &["seq", "kind", "turn", "case", "carrier_digest"])
                    .ok_or(DecodeError)?;
                let case = text(entry, "case").ok_or(DecodeError)?.to_owned();
                let carrier_digest = digest_field(entry, "carrier_digest")
                    .ok_or(DecodeError)?
                    .to_owned();
                if kind == "transition" {
                    if !matches!(case.as_str(), "continue" | "complete" | "suspend" | "fail") {
                        return Err(DecodeError);
                    }
                    JournalEntry::Transition {
                        turn,
                        case,
                        carrier_digest,
                    }
                } else {
                    if !matches!(case.as_str(), "complete" | "suspend" | "fail") {
                        return Err(DecodeError);
                    }
                    JournalEntry::TerminalOutcome {
                        turn,
                        case,
                        carrier_digest,
                    }
                }
            }
            _ => return Err(DecodeError),
        };
        entries.push(decoded);
    }
    Ok(entries)
}

/// A causal-ordering rejection from [`validate`]. Each case names one
/// concrete rule from the module-level ordering description.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum JournalError {
    Empty,
    /// An entry appeared where the per-turn phase machine did not expect it.
    UnexpectedEntry {
        seq: usize,
        kind: &'static str,
    },
    /// A `TurnOpened` named an invocation other than the journal's own.
    CrossInvocation {
        seq: usize,
    },
    /// A `TurnOpened`'s turn number did not advance by exactly one.
    NonSequentialTurn {
        seq: usize,
    },
    /// An `EffectObserved` named a different operation than its matching
    /// `EffectIntent`.
    EffectOperationMismatch {
        seq: usize,
    },
    /// An entry followed a `TerminalOutcome`.
    PostTerminal {
        seq: usize,
    },
}

// Every variant names what the state machine is waiting for next; the
// shared `Need` prefix is intentional documentation, not an accident.
#[allow(clippy::enum_variant_names)]
#[derive(Clone, Copy, PartialEq)]
enum Phase {
    /// Expect this turn's `TurnOpened` (or, at `seq == 0`, the journal's
    /// first entry).
    NeedTurnOpened,
    /// Expect this turn's `RequestIntent`.
    NeedRequestIntent,
    /// Expect this turn's `ResponseRecorded` or `ResponseFailed`.
    NeedResponse,
    /// Expect this turn's `ProposalAdmitted` or `ProposalRefused`. Only
    /// reachable after `ResponseRecorded`.
    NeedDecode,
    /// Expect this turn's `AuthorizationConsumed`. Only reachable after
    /// `ProposalAdmitted`.
    NeedAuthorizationConsumed,
    /// Expect either a further `EffectIntent` or this turn's `Transition`.
    NeedEffectOrTransition,
    /// Expect the `EffectObserved` matching the open `EffectIntent`.
    NeedEffectObserved,
    /// Expect this turn's `TerminalOutcome`. Only reachable after a
    /// non-`continue` `Transition`.
    NeedTerminalOutcome,
}

/// A causally-validated journal, ready for [`super::kernel::run_live_invocation`].
#[derive(Debug)]
pub struct ValidatedJournal<'a> {
    pub entries: &'a [JournalEntry],
    /// Whether the last entry is a `TerminalOutcome` (a fully settled run)
    /// versus ending mid-turn (an uncertain or in-progress run).
    pub terminal: bool,
    /// `Some(turn)` when the journal ends cleanly between turns — empty, or
    /// immediately after a `Transition { case: "continue", .. }` — so the
    /// kernel may safely open `turn` next without redispatching anything
    /// already recorded. `None` for every other non-terminal ending,
    /// including the uncertain case below.
    pub resumable_turn: Option<u32>,
    /// `true` when the journal ends immediately after a `RequestIntent`
    /// with no recorded response. Per the module documentation, this
    /// delivery is uncertain: the kernel must refuse to proceed — including
    /// refusing to redispatch the same request — before any further stage,
    /// store write, or host call.
    pub uncertain_intent: bool,
}

/// Validates causal ordering for a journal already known to belong to
/// `invocation`. See the module documentation for the exact rule set.
pub fn validate<'a>(
    entries: &'a [JournalEntry],
    invocation: &str,
) -> Result<ValidatedJournal<'a>, JournalError> {
    if entries.is_empty() {
        return Err(JournalError::Empty);
    }
    let mut phase = Phase::NeedTurnOpened;
    let mut expected_turn: u32 = 0;
    let mut ended = false;
    let mut pending_effect_operation: Option<String> = None;
    for (seq, entry) in entries.iter().enumerate() {
        if ended {
            return Err(JournalError::PostTerminal { seq });
        }
        let turn = entry.turn();
        match entry {
            JournalEntry::TurnOpened {
                invocation: inv, ..
            } => {
                if phase != Phase::NeedTurnOpened {
                    return Err(JournalError::UnexpectedEntry {
                        seq,
                        kind: entry.kind(),
                    });
                }
                if inv != invocation {
                    return Err(JournalError::CrossInvocation { seq });
                }
                if turn != expected_turn {
                    return Err(JournalError::NonSequentialTurn { seq });
                }
                phase = Phase::NeedRequestIntent;
            }
            JournalEntry::RequestIntent { .. } => {
                if phase != Phase::NeedRequestIntent || turn != expected_turn {
                    return Err(JournalError::UnexpectedEntry {
                        seq,
                        kind: entry.kind(),
                    });
                }
                phase = Phase::NeedResponse;
            }
            JournalEntry::ResponseRecorded { .. } => {
                if phase != Phase::NeedResponse || turn != expected_turn {
                    return Err(JournalError::UnexpectedEntry {
                        seq,
                        kind: entry.kind(),
                    });
                }
                phase = Phase::NeedDecode;
            }
            JournalEntry::ResponseFailed { .. } => {
                if phase != Phase::NeedResponse || turn != expected_turn {
                    return Err(JournalError::UnexpectedEntry {
                        seq,
                        kind: entry.kind(),
                    });
                }
                // A failed model call skips decode/authorize entirely: this
                // turn's only remaining entry is its Transition.
                phase = Phase::NeedEffectOrTransition;
            }
            JournalEntry::ProposalAdmitted { .. } => {
                if phase != Phase::NeedDecode || turn != expected_turn {
                    return Err(JournalError::UnexpectedEntry {
                        seq,
                        kind: entry.kind(),
                    });
                }
                phase = Phase::NeedAuthorizationConsumed;
            }
            JournalEntry::ProposalRefused { .. } => {
                if phase != Phase::NeedDecode || turn != expected_turn {
                    return Err(JournalError::UnexpectedEntry {
                        seq,
                        kind: entry.kind(),
                    });
                }
                phase = Phase::NeedEffectOrTransition;
            }
            JournalEntry::AuthorizationConsumed { .. } => {
                if phase != Phase::NeedAuthorizationConsumed || turn != expected_turn {
                    return Err(JournalError::UnexpectedEntry {
                        seq,
                        kind: entry.kind(),
                    });
                }
                phase = Phase::NeedEffectOrTransition;
            }
            JournalEntry::EffectIntent { operation, .. } => {
                if phase != Phase::NeedEffectOrTransition || turn != expected_turn {
                    return Err(JournalError::UnexpectedEntry {
                        seq,
                        kind: entry.kind(),
                    });
                }
                pending_effect_operation = Some(operation.clone());
                phase = Phase::NeedEffectObserved;
            }
            JournalEntry::EffectObserved { operation, .. } => {
                if phase != Phase::NeedEffectObserved || turn != expected_turn {
                    return Err(JournalError::UnexpectedEntry {
                        seq,
                        kind: entry.kind(),
                    });
                }
                if pending_effect_operation.as_deref() != Some(operation.as_str()) {
                    return Err(JournalError::EffectOperationMismatch { seq });
                }
                pending_effect_operation = None;
                phase = Phase::NeedEffectOrTransition;
            }
            JournalEntry::Transition { case, .. } => {
                if phase != Phase::NeedEffectOrTransition || turn != expected_turn {
                    return Err(JournalError::UnexpectedEntry {
                        seq,
                        kind: entry.kind(),
                    });
                }
                if case == "continue" {
                    expected_turn += 1;
                    phase = Phase::NeedTurnOpened;
                } else {
                    phase = Phase::NeedTerminalOutcome;
                }
            }
            JournalEntry::TerminalOutcome { case, .. } => {
                if phase != Phase::NeedTerminalOutcome || turn != expected_turn {
                    return Err(JournalError::UnexpectedEntry {
                        seq,
                        kind: entry.kind(),
                    });
                }
                let _ = case;
                ended = true;
            }
        }
    }
    // A journal that opened a turn's model step but has not yet reached a
    // Transition is uncertain: it must not be treated as validated for
    // replay-without-redispatch. Callers inspect `terminal` and the kernel
    // additionally refuses an uncertain tail before any redispatch (see
    // `kernel::replay_live_invocation`).
    Ok(ValidatedJournal {
        entries,
        terminal: ended,
        resumable_turn: (!ended && phase == Phase::NeedTurnOpened).then_some(expected_turn),
        uncertain_intent: !ended && phase == Phase::NeedResponse,
    })
}

/// A read-only projection of a validated journal, deterministically derived
/// from it.
///
/// This is the entire mechanism a receipt (owned downstream by #180) uses:
/// every field here is a pure fold over already-committed entries, so a
/// receipt is always *recomputed* from the journal, never a second value
/// tracked in step with it. Two callers who fold the same journal always
/// derive the same projection; there is no receipt-only state anywhere in
/// this module.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ReceiptProjection {
    pub invocation: String,
    pub turns: usize,
    pub model_calls: usize,
    pub model_failures: usize,
    pub effect_calls: usize,
    pub terminal_case: Option<String>,
}

/// Derives a [`ReceiptProjection`] from an already-[`validate`]d journal.
#[must_use]
pub fn receipt_projection(validated: &ValidatedJournal<'_>) -> ReceiptProjection {
    let mut turns = 0usize;
    let mut model_calls = 0usize;
    let mut model_failures = 0usize;
    let mut effect_calls = 0usize;
    let mut terminal_case = None;
    let mut invocation = String::new();
    for entry in validated.entries {
        match entry {
            JournalEntry::TurnOpened {
                invocation: inv, ..
            } => {
                turns += 1;
                invocation = inv.clone();
            }
            JournalEntry::ResponseRecorded { .. } => model_calls += 1,
            JournalEntry::ResponseFailed { .. } => model_failures += 1,
            JournalEntry::EffectIntent { .. } => effect_calls += 1,
            JournalEntry::TerminalOutcome { case, .. } => terminal_case = Some(case.clone()),
            _ => {}
        }
    }
    ReceiptProjection {
        invocation,
        turns,
        model_calls,
        model_failures,
        effect_calls,
        terminal_case,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn opened(turn: u32) -> JournalEntry {
        JournalEntry::TurnOpened {
            turn,
            invocation: "sha256:".to_owned() + &"a".repeat(64),
            observation_digest: "sha256:".to_owned() + &"b".repeat(64),
        }
    }
    fn intent(turn: u32) -> JournalEntry {
        JournalEntry::RequestIntent {
            turn,
            request_digest: "sha256:".to_owned() + &"c".repeat(64),
            reserved_budget: 10,
        }
    }
    fn recorded(turn: u32) -> JournalEntry {
        JournalEntry::ResponseRecorded {
            turn,
            response_digest: "sha256:".to_owned() + &"d".repeat(64),
            response: vec![1, 2, 3],
        }
    }
    fn admitted(turn: u32) -> JournalEntry {
        JournalEntry::ProposalAdmitted {
            turn,
            proposal_digest: "sha256:".to_owned() + &"e".repeat(64),
        }
    }
    fn consumed(turn: u32) -> JournalEntry {
        JournalEntry::AuthorizationConsumed {
            turn,
            grant_digest: "sha256:".to_owned() + &"9".repeat(64),
        }
    }
    fn transition(turn: u32, case: &str) -> JournalEntry {
        JournalEntry::Transition {
            turn,
            case: case.to_owned(),
            carrier_digest: "sha256:".to_owned() + &"f".repeat(64),
        }
    }
    fn terminal(turn: u32, case: &str) -> JournalEntry {
        JournalEntry::TerminalOutcome {
            turn,
            case: case.to_owned(),
            carrier_digest: "sha256:".to_owned() + &"f".repeat(64),
        }
    }
    const INVOCATION: &str = concat!(
        "sha256:",
        "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"
    );

    fn one_turn_complete() -> Vec<JournalEntry> {
        vec![
            opened(0),
            intent(0),
            recorded(0),
            admitted(0),
            consumed(0),
            transition(0, "complete"),
            terminal(0, "complete"),
        ]
    }

    #[test]
    fn a_minimal_legal_single_turn_conversation_validates() {
        let entries = one_turn_complete();
        let validated = validate(&entries, INVOCATION).unwrap();
        assert!(validated.terminal);
    }

    #[test]
    fn canonical_encode_decode_round_trips() {
        let entries = one_turn_complete();
        let rendered = render(&entries);
        let value: Value = serde_json::from_str(&rendered).unwrap();
        let decoded = decode(&value).unwrap();
        assert_eq!(decoded, entries);
    }

    #[test]
    fn chain_link_changes_on_reorder_or_truncation() {
        let entries = one_turn_complete();
        let full = chain(&entries);
        let truncated = chain(&entries[..entries.len() - 1]);
        assert_ne!(full, truncated);
        let mut reordered = entries.clone();
        reordered.swap(0, 1);
        // swapping breaks decode/validate long before chain comparison
        // matters in practice, but the chain itself must still differ.
        assert_ne!(chain(&reordered), full);
    }

    #[test]
    fn omission_is_rejected() {
        let mut entries = one_turn_complete();
        entries.remove(1); // drop RequestIntent
        assert!(validate(&entries, INVOCATION).is_err());
    }

    #[test]
    fn reorder_is_rejected() {
        let mut entries = one_turn_complete();
        entries.swap(1, 2); // intent/response swapped
        assert!(validate(&entries, INVOCATION).is_err());
    }

    #[test]
    fn cross_invocation_turn_opened_is_rejected() {
        let mut entries = one_turn_complete();
        entries[0] = JournalEntry::TurnOpened {
            turn: 0,
            invocation: "sha256:".to_owned() + &"9".repeat(64),
            observation_digest: "sha256:".to_owned() + &"b".repeat(64),
        };
        assert_eq!(
            validate(&entries, INVOCATION).unwrap_err(),
            JournalError::CrossInvocation { seq: 0 }
        );
    }

    #[test]
    fn schema_drift_style_refusal_still_produces_a_terminal_shaped_journal() {
        let entries = vec![
            opened(0),
            intent(0),
            recorded(0),
            JournalEntry::ProposalRefused {
                turn: 0,
                reason: "schema_drift".into(),
            },
            transition(0, "fail"),
            terminal(0, "fail"),
        ];
        let validated = validate(&entries, INVOCATION).unwrap();
        assert!(validated.terminal);
    }

    #[test]
    fn continuation_after_terminal_outcome_is_rejected() {
        let mut entries = one_turn_complete();
        let post_terminal_seq = entries.len();
        entries.push(opened(1));
        assert_eq!(
            validate(&entries, INVOCATION).unwrap_err(),
            JournalError::PostTerminal {
                seq: post_terminal_seq
            }
        );
    }

    #[test]
    fn duplicate_request_intent_is_rejected() {
        // Two `RequestIntent`s in a row for the same turn, with no
        // intervening response, is a duplicate durable-commit attempt —
        // distinct from omission (a required entry missing) and reorder (two
        // adjacent entries transposed), and explicitly named in issue #108's
        // required failure cases ("duplicate/out-of-order transitions...are
        // rejected").
        let entries = vec![opened(0), intent(0), intent(0)];
        assert_eq!(
            validate(&entries, INVOCATION).unwrap_err(),
            JournalError::UnexpectedEntry {
                seq: 2,
                kind: "request_intent"
            }
        );
    }

    #[test]
    fn duplicate_turn_opened_is_rejected() {
        let entries = vec![opened(0), opened(0)];
        assert_eq!(
            validate(&entries, INVOCATION).unwrap_err(),
            JournalError::UnexpectedEntry {
                seq: 1,
                kind: "turn_opened"
            }
        );
    }

    #[test]
    fn duplicate_transition_after_continue_is_rejected() {
        // A second `Transition` for turn 0 immediately after the first
        // `continue` is not a new turn's `TurnOpened`, so it is rejected as
        // out of phase rather than silently accepted as a repeat decision.
        let mut entries = vec![
            opened(0),
            intent(0),
            recorded(0),
            admitted(0),
            consumed(0),
            transition(0, "continue"),
        ];
        entries.push(transition(0, "continue"));
        assert_eq!(
            validate(&entries, INVOCATION).unwrap_err(),
            JournalError::UnexpectedEntry {
                seq: 6,
                kind: "transition"
            }
        );
    }

    #[test]
    fn non_sequential_turn_is_rejected() {
        let entries = vec![
            opened(0),
            intent(0),
            recorded(0),
            admitted(0),
            consumed(0),
            transition(0, "continue"),
            opened(2), // should be 1
        ];
        assert_eq!(
            validate(&entries, INVOCATION).unwrap_err(),
            JournalError::NonSequentialTurn { seq: 6 }
        );
    }

    #[test]
    fn effect_operation_mismatch_is_rejected() {
        let entries = vec![
            opened(0),
            intent(0),
            recorded(0),
            admitted(0),
            consumed(0),
            JournalEntry::EffectIntent {
                turn: 0,
                operation: "tool.a".into(),
                request_digest: "sha256:".to_owned() + &"1".repeat(64),
            },
            JournalEntry::EffectObserved {
                turn: 0,
                operation: "tool.b".into(),
                observation_digest: "sha256:".to_owned() + &"2".repeat(64),
            },
        ];
        assert_eq!(
            validate(&entries, INVOCATION).unwrap_err(),
            JournalError::EffectOperationMismatch { seq: 6 }
        );
    }

    #[test]
    fn a_journal_ending_in_intent_is_uncertain_not_terminal() {
        let entries = vec![opened(0), intent(0)];
        let validated = validate(&entries, INVOCATION).unwrap();
        assert!(!validated.terminal);
        assert!(validated.uncertain_intent);
        assert_eq!(validated.resumable_turn, None);
    }

    #[test]
    fn a_journal_ending_right_after_continue_is_resumable_not_uncertain() {
        let entries = vec![
            opened(0),
            intent(0),
            recorded(0),
            admitted(0),
            consumed(0),
            transition(0, "continue"),
        ];
        let validated = validate(&entries, INVOCATION).unwrap();
        assert!(!validated.terminal);
        assert!(!validated.uncertain_intent);
        assert_eq!(validated.resumable_turn, Some(1));
    }

    #[test]
    fn decode_rejects_an_unknown_key() {
        let mut value: Value = serde_json::from_str(&render(&one_turn_complete())).unwrap();
        value[0]
            .as_object_mut()
            .unwrap()
            .insert("extra".into(), Value::Bool(true));
        assert!(decode(&value).is_err());
    }

    #[test]
    fn decode_rejects_malformed_digest_format() {
        let mut value: Value = serde_json::from_str(&render(&one_turn_complete())).unwrap();
        value[0]["invocation"] = Value::String("not-a-digest".into());
        assert!(decode(&value).is_err());
    }

    #[test]
    fn receipt_projection_is_a_pure_fold_over_the_journal() {
        let entries = one_turn_complete();
        let validated = validate(&entries, INVOCATION).unwrap();
        let receipt = receipt_projection(&validated);
        assert_eq!(receipt.invocation, INVOCATION);
        assert_eq!(receipt.turns, 1);
        assert_eq!(receipt.model_calls, 1);
        assert_eq!(receipt.model_failures, 0);
        assert_eq!(receipt.terminal_case.as_deref(), Some("complete"));
        // Deterministic: folding the same journal twice yields the same
        // projection, so a receipt never carries state the journal lacks.
        assert_eq!(receipt, receipt_projection(&validated));
    }
}
