//! The versioned checkpoint envelope that attaches a route record to the
//! existing durable policy journal. Generation 0 holds the frozen route and
//! binding facts before any adapter exists; every later generation carries
//! the unchanged route plus the policy journal document the kernel committed.

use serde_json::{json, Map, Value};

use super::error::RuntimeRoutingError;
use super::record::RouteRecord;
use crate::agent_lifecycle::{CheckpointStore, CheckpointStoreError};
use crate::model_routing::engine::json;

pub const ENVELOPE_SCHEMA: &str = "semaprax.routed-invocation-checkpoint.v1";
const MAX_ENVELOPE_BYTES: usize = 4 * 1024 * 1024;

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Envelope {
    pub(crate) record: RouteRecord,
    pub(crate) binding: String,
    pub(crate) invocation: String,
    pub(crate) deployment_root: String,
    pub(crate) instance_root: String,
    pub(crate) policy: Option<String>,
}

impl Envelope {
    pub(crate) fn render(&self) -> String {
        json::canonical(&json!({
            "schema": ENVELOPE_SCHEMA,
            "route": self.record.to_json(),
            "route_digest": self.record.digest(),
            "binding": self.binding,
            "invocation": self.invocation,
            "deployment_root": self.deployment_root,
            "instance_root": self.instance_root,
            "policy": self.policy,
        }))
    }

    pub(crate) fn parse(text: &str) -> Result<Self, RuntimeRoutingError> {
        let bad = |why: &str| RuntimeRoutingError::RecordMismatch(why.to_owned());
        if text.len() > MAX_ENVELOPE_BYTES {
            return Err(bad("checkpoint envelope exceeds its byte bound"));
        }
        let value: Value = serde_json::from_str(text).map_err(|_| bad("envelope JSON"))?;
        let m: &Map<String, Value> = value.as_object().ok_or_else(|| bad("envelope object"))?;
        const KEYS: [&str; 8] = [
            "schema",
            "route",
            "route_digest",
            "binding",
            "invocation",
            "deployment_root",
            "instance_root",
            "policy",
        ];
        if m.len() != KEYS.len() || KEYS.iter().any(|k| !m.contains_key(*k)) {
            return Err(bad("envelope members"));
        }
        if m["schema"] != ENVELOPE_SCHEMA {
            return Err(bad("envelope schema"));
        }
        let record = RouteRecord::from_value(&m["route"])?;
        if m["route_digest"] != record.digest().as_str() {
            return Err(bad("route digest"));
        }
        let text_of = |k: &str| m[k].as_str().map(str::to_owned).ok_or_else(|| bad(k));
        let envelope = Self {
            record,
            binding: text_of("binding")?,
            invocation: text_of("invocation")?,
            deployment_root: text_of("deployment_root")?,
            instance_root: text_of("instance_root")?,
            policy: match &m["policy"] {
                Value::Null => None,
                Value::String(s) => Some(s.clone()),
                _ => return Err(bad("policy")),
            },
        };
        if envelope.render() != text {
            return Err(bad("envelope is not canonical"));
        }
        Ok(envelope)
    }
}

/// Wraps the host store: every policy-journal commit is written inside the
/// envelope, so the route can never be separated from the attempts it bound.
pub(crate) struct EnvelopeStore<'a> {
    pub(crate) inner: &'a mut dyn CheckpointStore,
    pub(crate) head: Envelope,
}

impl CheckpointStore for EnvelopeStore<'_> {
    fn commit(&mut self, generation: u64, document: &str) -> Result<(), CheckpointStoreError> {
        let mut next = self.head.clone();
        next.policy = Some(document.to_owned());
        self.inner.commit(generation, &next.render())
    }
}
