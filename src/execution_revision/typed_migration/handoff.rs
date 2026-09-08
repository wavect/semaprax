//! Closed wire codec for a checked migration seed. Decoding carries no root
//! authority; the runtime wrapper independently authenticates every binding.

use super::*;
use crate::agent_runtime_v2::checkpoint::{
    decode_retained_value, encode_retained_value, CheckpointUsage,
};
use crate::interpreter::retained_call::RetainedValue;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};

const SCHEMA: &str = "semaprax.agent-migration-handoff.v1";
const MAX_BYTES: usize = 1_048_576;
const MAX_ITERATIONS: usize = 4_096;
const MAX_STAGES: usize = 12_289;
#[cfg(test)]
#[path = "handoff/tests.rs"]
mod tests;

pub(super) struct Handoff {
    pub(super) migration: String,
    pub(super) value: RetainedValue,
    pub(super) usage: CheckpointUsage,
    pub(super) iterations: usize,
    pub(super) stages: usize,
    pub(super) max_reserved_fuel: u64,
}

impl Handoff {
    pub(super) fn from_seed(seed: &MigrationSeed) -> Result<Self> {
        let result = Self {
            migration: seed.binding.canonical_json().to_owned(),
            value: seed.value.clone(),
            usage: seed.usage,
            iterations: seed.iterations,
            stages: seed.stages,
            max_reserved_fuel: seed.max_reserved_fuel,
        };
        result.checked()?;
        if result.canonical_json().len() > MAX_BYTES {
            return Err(refused("migration.handoff.bytes"));
        }
        Ok(result)
    }

    pub(super) fn canonical_json(&self) -> String {
        let value = encode_retained_value(&self.value).expect("checked migration seed value");
        canonical(json!({
            "schema": SCHEMA,
            "migration": serde_json::from_str::<Value>(&self.migration)
                .expect("checked execution root JSON"),
            "value": value,
            "usage": usage_json(self.usage),
            "iterations": self.iterations.to_string(),
            "stages": self.stages.to_string(),
            "max_reserved_fuel": self.max_reserved_fuel.to_string(),
        }))
    }

    pub(super) fn digest(&self) -> String {
        let bytes = self.canonical_json();
        let mut hash = Sha256::new();
        hash.update(b"semaprax.agent-migration-handoff.v1\0");
        hash.update((bytes.len() as u64).to_le_bytes());
        hash.update(bytes.as_bytes());
        format!("sha256:{:x}", crate::digest_hex::LowerHex(hash.finalize()))
    }

    pub(super) fn decode(document: &str, expected_digest: &str) -> Result<Self> {
        if document.len() > MAX_BYTES || !digest_valid(expected_digest) {
            return Err(refused("migration.handoff.bounds"));
        }
        let value: Value =
            serde_json::from_str(document).map_err(|_| refused("migration.handoff.json"))?;
        keys(
            &value,
            &[
                "schema",
                "migration",
                "value",
                "usage",
                "iterations",
                "stages",
                "max_reserved_fuel",
            ],
        )?;
        if text(&value, "schema")? != SCHEMA {
            return Err(refused("migration.handoff.schema"));
        }
        let migration = canonical(
            value
                .get("migration")
                .ok_or_else(|| refused("migration.handoff.migration"))?
                .clone(),
        );
        let handoff = Self {
            migration,
            value: decode_retained_value(&value["value"])
                .map_err(|_| refused("migration.handoff.value"))?,
            usage: decode_usage(&value["usage"])?,
            iterations: number(&value, "iterations")?,
            stages: number(&value, "stages")?,
            max_reserved_fuel: number_u64(&value, "max_reserved_fuel")?,
        };
        handoff.checked()?;
        if handoff.canonical_json() != document || handoff.digest() != expected_digest {
            return Err(refused("migration.handoff.integrity"));
        }
        Ok(handoff)
    }

    fn checked(&self) -> Result<()> {
        if self.migration.len() > MAX_BYTES
            || self.iterations > MAX_ITERATIONS
            || self.stages > MAX_STAGES
        {
            return Err(refused("migration.handoff.bounds"));
        }
        let migration: Value = serde_json::from_str(&self.migration)
            .map_err(|_| refused("migration.handoff.migration"))?;
        if canonical(migration) != self.migration {
            return Err(refused("migration.handoff.migration"));
        }
        if !matches!(self.value, RetainedValue::Record(_)) {
            return Err(refused("migration.handoff.value.kind"));
        }
        if self.usage.reserved_fuel > self.max_reserved_fuel
            || self
                .usage
                .argument_bytes
                .checked_add(self.usage.result_bytes)
                .is_none()
        {
            return Err(refused("migration.handoff.usage"));
        }
        let encoded =
            encode_retained_value(&self.value).map_err(|_| refused("migration.handoff.value"))?;
        if serde_json::to_string(&encoded)
            .map_err(|_| refused("migration.handoff.value"))?
            .len()
            > 262_144
        {
            return Err(refused("migration.handoff.value"));
        }
        Ok(())
    }
}

fn canonical(value: Value) -> String {
    format!(
        "{}\n",
        serde_json::to_string(&value).expect("closed handoff JSON")
    )
}
fn usage_json(usage: CheckpointUsage) -> Value {
    json!({"calls":usage.calls.to_string(),"argument_bytes":usage.argument_bytes.to_string(),"result_bytes":usage.result_bytes.to_string(),"reserved_fuel":usage.reserved_fuel.to_string()})
}
fn keys(value: &Value, expected: &[&str]) -> Result<()> {
    let Some(object) = value.as_object() else {
        return Err(refused("migration.handoff.object"));
    };
    if object.len() != expected.len() || expected.iter().any(|key| !object.contains_key(*key)) {
        return Err(refused("migration.handoff.keys"));
    };
    Ok(())
}
fn text<'a>(value: &'a Value, key: &str) -> Result<&'a str> {
    value[key]
        .as_str()
        .ok_or_else(|| refused("migration.handoff.text"))
}
fn number(value: &Value, key: &str) -> Result<usize> {
    text(value, key)?
        .parse()
        .map_err(|_| refused("migration.handoff.integer"))
}
fn number_u64(value: &Value, key: &str) -> Result<u64> {
    text(value, key)?
        .parse()
        .map_err(|_| refused("migration.handoff.integer"))
}
fn decode_usage(value: &Value) -> Result<CheckpointUsage> {
    keys(
        value,
        &["calls", "argument_bytes", "result_bytes", "reserved_fuel"],
    )?;
    Ok(CheckpointUsage {
        calls: number_u64(value, "calls")?,
        argument_bytes: number_u64(value, "argument_bytes")?,
        result_bytes: number_u64(value, "result_bytes")?,
        reserved_fuel: number_u64(value, "reserved_fuel")?,
    })
}
fn digest_valid(value: &str) -> bool {
    value.len() == 71
        && value.starts_with("sha256:")
        && value[7..]
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}
