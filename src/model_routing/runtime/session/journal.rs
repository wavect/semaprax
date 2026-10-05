//! The route-session journal (`semaprax.runtime-route-session.v1`): route
//! transitions, handoff digests, turn terminals and child reservations.

use serde_json::{json, Map, Value};

use super::super::error::RuntimeRoutingError;
use super::super::record::RouteRecord;
use super::handoff::{hex, unhex};
use crate::model_routing::engine::json;

pub const SESSION_SCHEMA: &str = "semaprax.runtime-route-session.v1";
pub(crate) const MAX_ENTRIES: usize = 512;

/// Why a turn's route was chosen. Transport failover never appears here: it
/// stays inside one invocation's ordered provider policy.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RouteReason {
    Initial,
    Reroute,
    /// The configured bounded no-progress rule restricted the candidates.
    Escalation,
    /// An authorized specialist was named for this turn.
    Specialist,
}

impl RouteReason {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Initial => "initial",
            Self::Reroute => "reroute",
            Self::Escalation => "escalation",
            Self::Specialist => "specialist",
        }
    }
    fn parse(s: &str) -> Option<Self> {
        [
            Self::Initial,
            Self::Reroute,
            Self::Escalation,
            Self::Specialist,
        ]
        .into_iter()
        .find(|r| r.as_str() == s)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum Entry {
    Opened {
        session: String,
        caller: Option<String>,
        depth: u32,
        max_depth: u32,
        ceiling: i64,
        deadline: Option<i64>,
        profile_set: String,
        policy: String,
        instructions: String,
        acceptance: String,
        lineage: Vec<String>,
        allowed: Option<Vec<String>>,
    },
    Routed {
        turn: u32,
        reason: RouteReason,
        from: Option<String>,
        to: String,
        record: RouteRecord,
        handoff: String,
        reserved: i64,
    },
    Settled {
        turn: u32,
        response: String,
        state: Vec<u8>,
        progressed: bool,
        complete: bool,
        tool_results: Vec<String>,
    },
    /// The host refused the settled output: no re-route boundary exists.
    Unaccepted {
        turn: u32,
        why: String,
    },
    /// An uncertain dispatch or external effect: reconciliation only.
    Uncertain {
        turn: u32,
        kind: String,
    },
    /// Refused or cancelled before a settled response; no silent fallback.
    Failed {
        turn: u32,
        why: String,
    },
    ChildReserved {
        child: String,
        specialist: String,
        profile: String,
        depth: u32,
        amount: i64,
    },
    ChildSettled {
        child: String,
        spent: i64,
    },
}

fn strs(v: &Value) -> Option<Vec<String>> {
    v.as_array()?
        .iter()
        .map(|s| s.as_str().map(str::to_owned))
        .collect()
}

impl Entry {
    fn to_json(&self) -> Value {
        match self {
            Self::Opened {
                session,
                caller,
                depth,
                max_depth,
                ceiling,
                deadline,
                profile_set,
                policy,
                instructions,
                acceptance,
                lineage,
                allowed,
            } => json!({"kind": "opened", "session": session, "caller": caller, "depth": depth,
                "max_depth": max_depth, "ceiling": ceiling, "deadline": deadline,
                "profile_set": profile_set, "policy": policy, "instructions": instructions,
                "acceptance": acceptance, "lineage": lineage, "allowed": allowed}),
            Self::Routed {
                turn,
                reason,
                from,
                to,
                record,
                handoff,
                reserved,
            } => json!({"kind": "routed", "turn": turn, "reason": reason.as_str(), "from": from,
                "to": to, "record": record.to_json(), "handoff": handoff, "reserved": reserved}),
            Self::Settled {
                turn,
                response,
                state,
                progressed,
                complete,
                tool_results,
            } => json!({"kind": "settled", "turn": turn, "response": response,
                "state": hex(state), "progressed": progressed, "complete": complete,
                "tool_results": tool_results}),
            Self::Unaccepted { turn, why } => {
                json!({"kind": "unaccepted", "turn": turn, "why": why})
            }
            Self::Uncertain { turn, kind } => {
                json!({"kind": "uncertain", "turn": turn, "effect": kind})
            }
            Self::Failed { turn, why } => json!({"kind": "failed", "turn": turn, "why": why}),
            Self::ChildReserved {
                child,
                specialist,
                profile,
                depth,
                amount,
            } => json!({"kind": "child_reserved", "child": child, "specialist": specialist,
                "profile": profile, "depth": depth, "amount": amount}),
            Self::ChildSettled { child, spent } => {
                json!({"kind": "child_settled", "child": child, "spent": spent})
            }
        }
    }

    fn from_json(v: &Value) -> Option<Self> {
        let m: &Map<String, Value> = v.as_object()?;
        let s = |k: &str| m.get(k)?.as_str().map(str::to_owned);
        let u = |k: &str| m.get(k)?.as_u64().and_then(|n| u32::try_from(n).ok());
        let i = |k: &str| m.get(k)?.as_i64();
        let b = |k: &str| m.get(k)?.as_bool();
        let opt = |k: &str| match m.get(k)? {
            Value::Null => Some(None),
            Value::String(x) => Some(Some(x.clone())),
            _ => None,
        };
        Some(match m.get("kind")?.as_str()? {
            "opened" => Self::Opened {
                session: s("session")?,
                caller: opt("caller")?,
                depth: u("depth")?,
                max_depth: u("max_depth")?,
                ceiling: i("ceiling")?,
                deadline: match m.get("deadline")? {
                    Value::Null => None,
                    x => Some(x.as_i64()?),
                },
                profile_set: s("profile_set")?,
                policy: s("policy")?,
                instructions: s("instructions")?,
                acceptance: s("acceptance")?,
                lineage: strs(m.get("lineage")?)?,
                allowed: match m.get("allowed")? {
                    Value::Null => None,
                    x => Some(strs(x)?),
                },
            },
            "routed" => Self::Routed {
                turn: u("turn")?,
                reason: RouteReason::parse(&s("reason")?)?,
                from: opt("from")?,
                to: s("to")?,
                record: RouteRecord::from_value(m.get("record")?).ok()?,
                handoff: s("handoff")?,
                reserved: i("reserved")?,
            },
            "settled" => Self::Settled {
                turn: u("turn")?,
                response: s("response")?,
                state: unhex(&s("state")?)?,
                progressed: b("progressed")?,
                complete: b("complete")?,
                tool_results: strs(m.get("tool_results")?)?,
            },
            "unaccepted" => Self::Unaccepted {
                turn: u("turn")?,
                why: s("why")?,
            },
            "uncertain" => Self::Uncertain {
                turn: u("turn")?,
                kind: s("effect")?,
            },
            "failed" => Self::Failed {
                turn: u("turn")?,
                why: s("why")?,
            },
            "child_reserved" => Self::ChildReserved {
                child: s("child")?,
                specialist: s("specialist")?,
                profile: s("profile")?,
                depth: u("depth")?,
                amount: i("amount")?,
            },
            "child_settled" => Self::ChildSettled {
                child: s("child")?,
                spent: i("spent")?,
            },
            _ => return None,
        })
    }
}

pub(crate) fn render(entries: &[Entry]) -> String {
    json::canonical(&json!({
        "schema": SESSION_SCHEMA,
        "entries": entries.iter().map(Entry::to_json).collect::<Vec<_>>(),
    }))
}

pub(crate) fn parse(text: &str) -> Result<Vec<Entry>, RuntimeRoutingError> {
    let bad = |why: &str| RuntimeRoutingError::RecordMismatch(format!("route session: {why}"));
    let v: Value = serde_json::from_str(text).map_err(|_| bad("JSON"))?;
    if v.get("schema") != Some(&json!(SESSION_SCHEMA)) || v.as_object().map(Map::len) != Some(2) {
        return Err(bad("schema"));
    }
    let entries: Vec<Entry> = v["entries"]
        .as_array()
        .ok_or_else(|| bad("entries"))?
        .iter()
        .map(Entry::from_json)
        .collect::<Option<_>>()
        .ok_or_else(|| bad("entry"))?;
    if entries.is_empty()
        || entries.len() > MAX_ENTRIES
        || !matches!(entries[0], Entry::Opened { .. })
        || render(&entries) != text
    {
        return Err(bad("shape"));
    }
    Ok(entries)
}
