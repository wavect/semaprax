//! The durable operation journal of one checkpointed lifecycle run.
//!
//! The journal is an ordered record of what a run actually committed to the
//! caller's storage, in a closed vocabulary whose kinds may only advance. Its
//! chain link is recomputed from the entries on every load, so a truncated,
//! reordered, or partially written journal fails to decode.
//!
//! The journal is deliberately **not** authenticated. It carries no key
//! material and no signature, so a party who can rewrite the caller's storage
//! can also recompute the chain. The chain detects accidental truncation,
//! reordering and torn writes; it is not a forgery defence. What makes a
//! forged journal harmless to authority is that no journal entry is ever an
//! input to minting an [`crate::agent_lifecycle::Authorized`]: a resumed run
//! re-runs the validated authorizing transition against a state it recomputes
//! itself, and compares the resulting operation identity against the journal.
//! A journal can therefore withhold or misdescribe an operation; it can never
//! produce authority for one.

use serde_json::{Map, Value};
use sha2::{Digest, Sha256};

use crate::diagnostic::quote_json;

const JOURNAL_DOMAIN: &[u8] = b"semaprax.agent-checkpoint.journal.v1\0";

/// The closed journal vocabulary of one durable run.
///
/// Ranks advance strictly: a journal is admitted only when every entry's rank
/// is greater than its predecessor's, starting from [`JournalEntry::Prefix`].
/// [`JournalEntry::Settled`] and [`JournalEntry::Abandoned`] share a rank, so
/// one journal can never carry both.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(in crate::agent_lifecycle) enum JournalEntry {
    /// The deterministic prefix settled on one state and one proposal. No
    /// external boundary has been approached.
    Prefix {
        state_digest: String,
        proposal_digest: String,
    },
    /// One external operation is about to cross the boundary. This entry is
    /// committed *before* the boundary, so a crash after it leaves the
    /// operation's delivery uncertain rather than unknown to have been tried.
    Intent {
        operation: String,
        granted_budget: i64,
    },
    /// The external operation settled with this observation. Present only
    /// after the runtime, or an explicit host reconciliation, determined the
    /// settled value.
    Settled {
        operation: String,
        observation_digest: String,
        observation: Option<Vec<u8>>,
    },
    /// The host reconciled an uncertain operation to abandonment. Terminal.
    Abandoned { operation: String },
    /// The deterministic reduction published a result.
    Reduced { result_digest: String },
    /// The result was delivered to the caller. Terminal.
    Delivered { result_digest: String },
}

impl JournalEntry {
    /// The strictly advancing rank of one entry kind.
    pub(in crate::agent_lifecycle) const fn rank(&self) -> u8 {
        match self {
            Self::Prefix { .. } => 0,
            Self::Intent { .. } => 1,
            Self::Settled { .. } | Self::Abandoned { .. } => 2,
            Self::Reduced { .. } => 3,
            Self::Delivered { .. } => 4,
        }
    }

    pub(in crate::agent_lifecycle) const fn kind(&self) -> &'static str {
        match self {
            Self::Prefix { .. } => "prefix",
            Self::Intent { .. } => "intent",
            Self::Settled { .. } => "settled",
            Self::Abandoned { .. } => "abandoned",
            Self::Reduced { .. } => "reduced",
            Self::Delivered { .. } => "delivered",
        }
    }

    /// The stable identity of the external operation this entry names.
    pub(in crate::agent_lifecycle) fn operation(&self) -> Option<&str> {
        match self {
            Self::Intent { operation, .. }
            | Self::Settled { operation, .. }
            | Self::Abandoned { operation } => Some(operation),
            _ => None,
        }
    }

    /// The canonical encoding of one entry at one sequence position.
    pub(in crate::agent_lifecycle) fn encode(&self, seq: usize) -> String {
        let head = format!(
            "{{\"seq\":{},\"kind\":{}",
            quote_json(&seq.to_string()),
            quote_json(self.kind())
        );
        let body = match self {
            Self::Prefix {
                state_digest,
                proposal_digest,
            } => format!(
                ",\"state_digest\":{},\"proposal_digest\":{}",
                quote_json(state_digest),
                quote_json(proposal_digest)
            ),
            Self::Intent {
                operation,
                granted_budget,
            } => format!(
                ",\"operation\":{},\"granted_budget\":{}",
                quote_json(operation),
                quote_json(&granted_budget.to_string())
            ),
            Self::Settled {
                operation,
                observation_digest,
                observation,
            } => format!(
                ",\"operation\":{},\"observation_digest\":{},\"observation\":{}",
                quote_json(operation),
                quote_json(observation_digest),
                observation
                    .as_deref()
                    .map_or_else(|| "null".to_owned(), |bytes| quote_json(&hex(bytes)))
            ),
            Self::Abandoned { operation } => format!(",\"operation\":{}", quote_json(operation)),
            Self::Reduced { result_digest } | Self::Delivered { result_digest } => {
                format!(",\"result_digest\":{}", quote_json(result_digest))
            }
        };
        format!("{head}{body}}}")
    }
}

/// Lowercase hexadecimal, the only byte transport the journal admits.
pub(in crate::agent_lifecycle) fn hex(bytes: &[u8]) -> String {
    let mut output = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        output.push_str(&format!("{byte:02x}"));
    }
    output
}

fn unhex(text: &str) -> Option<Vec<u8>> {
    if !text.len().is_multiple_of(2) {
        return None;
    }
    let raw = text.as_bytes();
    let mut output = Vec::with_capacity(text.len() / 2);
    for pair in raw.chunks(2) {
        let mut byte = 0u8;
        for digit in pair {
            let value = match digit {
                b'0'..=b'9' => digit - b'0',
                b'a'..=b'f' => digit - b'a' + 10,
                _ => return None,
            };
            byte = byte * 16 + value;
        }
        output.push(byte);
    }
    Some(output)
}

/// The canonical rendering of a whole journal, as a JSON array.
pub(in crate::agent_lifecycle) fn render(entries: &[JournalEntry]) -> String {
    let mut output = String::from("[");
    for (seq, entry) in entries.iter().enumerate() {
        if seq > 0 {
            output.push(',');
        }
        output.push_str(&entry.encode(seq));
    }
    output.push(']');
    output
}

/// The chain link over a whole journal.
///
/// `link(0) = H(domain || 0x00 || entry(0))` and
/// `link(i) = H(domain || link(i-1) || 0x00 || entry(i))`, so removing,
/// appending in the middle, or transposing any entry changes the link.
pub(in crate::agent_lifecycle) fn chain(entries: &[JournalEntry]) -> String {
    let mut link = String::new();
    for (seq, entry) in entries.iter().enumerate() {
        let mut hash = Sha256::new();
        hash.update(JOURNAL_DOMAIN);
        hash.update(link.as_bytes());
        hash.update([0]);
        hash.update(entry.encode(seq).as_bytes());
        link = format!("sha256:{:x}", crate::digest_hex::LowerHex(hash.finalize()));
    }
    link
}

fn text<'a>(entry: &'a Map<String, Value>, key: &str) -> Option<&'a str> {
    entry.get(key)?.as_str()
}

fn digest_text<'a>(entry: &'a Map<String, Value>, key: &str) -> Option<&'a str> {
    let value = text(entry, key)?;
    (value.len() == "sha256:".len() + 64
        && value.starts_with("sha256:")
        && value[7..]
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte)))
    .then_some(value)
}

/// Decodes one journal array. Rejects an unknown key, an out-of-order
/// sequence number, a non-advancing rank, an empty journal, and any entry that
/// does not open with the deterministic prefix.
pub(in crate::agent_lifecycle) fn decode(value: &Value, limit: usize) -> Option<Vec<JournalEntry>> {
    let array = value.as_array()?;
    if array.is_empty() || array.len() > limit {
        return None;
    }
    let mut entries = Vec::with_capacity(array.len());
    let mut previous: Option<u8> = None;
    for (seq, item) in array.iter().enumerate() {
        let entry = item.as_object()?;
        if text(entry, "seq")? != seq.to_string() {
            return None;
        }
        let kind = text(entry, "kind")?;
        let decoded = match kind {
            "prefix" => {
                closed(entry, &["seq", "kind", "state_digest", "proposal_digest"])?;
                JournalEntry::Prefix {
                    state_digest: digest_text(entry, "state_digest")?.to_owned(),
                    proposal_digest: digest_text(entry, "proposal_digest")?.to_owned(),
                }
            }
            "intent" => {
                closed(entry, &["seq", "kind", "operation", "granted_budget"])?;
                JournalEntry::Intent {
                    operation: digest_text(entry, "operation")?.to_owned(),
                    granted_budget: text(entry, "granted_budget")?.parse().ok()?,
                }
            }
            "settled" => {
                closed(
                    entry,
                    &[
                        "seq",
                        "kind",
                        "operation",
                        "observation_digest",
                        "observation",
                    ],
                )?;
                let observation = match entry.get("observation")? {
                    Value::Null => None,
                    Value::String(text) => Some(unhex(text)?),
                    _ => return None,
                };
                JournalEntry::Settled {
                    operation: digest_text(entry, "operation")?.to_owned(),
                    observation_digest: digest_text(entry, "observation_digest")?.to_owned(),
                    observation,
                }
            }
            "abandoned" => {
                closed(entry, &["seq", "kind", "operation"])?;
                JournalEntry::Abandoned {
                    operation: digest_text(entry, "operation")?.to_owned(),
                }
            }
            "reduced" => {
                closed(entry, &["seq", "kind", "result_digest"])?;
                JournalEntry::Reduced {
                    result_digest: digest_text(entry, "result_digest")?.to_owned(),
                }
            }
            "delivered" => {
                closed(entry, &["seq", "kind", "result_digest"])?;
                JournalEntry::Delivered {
                    result_digest: digest_text(entry, "result_digest")?.to_owned(),
                }
            }
            _ => return None,
        };
        let rank = decoded.rank();
        match previous {
            None if rank != 0 => return None,
            Some(earlier) if rank <= earlier => return None,
            _ => {}
        }
        previous = Some(rank);
        entries.push(decoded);
    }
    Some(entries)
}

fn closed(entry: &Map<String, Value>, keys: &[&str]) -> Option<()> {
    (entry.len() == keys.len() && keys.iter().all(|key| entry.contains_key(*key))).then_some(())
}
