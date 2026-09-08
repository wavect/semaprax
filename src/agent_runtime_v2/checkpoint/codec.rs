//! Closed deterministic JSON. Canonical byte equality rejects duplicate keys.
use super::*;
pub(super) fn keys(v: &Value, expected: &[&str]) -> Result<(), Diagnostic> {
    let map = v.as_object().ok_or_else(|| rejected("object"))?;
    if map.len() != expected.len() || !expected.iter().all(|key| map.contains_key(*key)) {
        return Err(rejected("keys"));
    }
    Ok(())
}
pub(super) fn text<'a>(v: &'a Value, key: &str) -> Result<&'a str, Diagnostic> {
    v[key].as_str().ok_or_else(|| rejected(key))
}
fn number(v: &Value, key: &str) -> Result<u64, Diagnostic> {
    v[key].as_u64().ok_or_else(|| rejected(key))
}
pub(super) fn usage(v: CheckpointUsage) -> Value {
    json!({"calls":v.calls,"argument_bytes":v.argument_bytes,"result_bytes":v.result_bytes,"reserved_fuel":v.reserved_fuel})
}
fn read_usage(v: &Value) -> Result<CheckpointUsage, Diagnostic> {
    keys(
        v,
        &["calls", "argument_bytes", "result_bytes", "reserved_fuel"],
    )?;
    Ok(CheckpointUsage {
        calls: number(v, "calls")?,
        argument_bytes: number(v, "argument_bytes")?,
        result_bytes: number(v, "result_bytes")?,
        reserved_fuel: number(v, "reserved_fuel")?,
    })
}
pub(super) fn binding(j: &OperationCheckpoint) -> Value {
    json!({"identity":{"execution_revision":j.identity.execution_revision,"invocation":j.identity.invocation,"registry":j.identity.registry,"program_root":j.identity.program_root},"limits":{"calls":j.limits.calls,"argument_bytes":j.limits.argument_bytes,"result_bytes":j.limits.result_bytes,"total_bytes":j.limits.total_bytes,"reserved_fuel":j.limits.reserved_fuel}})
}
fn context(c: &EffectContext) -> Result<Value, Diagnostic> {
    if c.turn >= 4096
        || !value::identifier(&c.operation)
        || !value::identifier(&c.effect)
        || !hash_valid(&c.authorization_binding)
        || c.proposal.len() > 262144
        || !matches!(c.state, RetainedValue::Record(_))
    {
        return Err(rejected("effect.context"));
    }
    Ok(
        json!({"turn":c.turn,"operation":c.operation,"effect":c.effect,"authorization_binding":c.authorization_binding,"state":value::encode(&c.state)?,"proposal":c.proposal,"arguments":value::fields(&c.arguments)?}),
    )
}
fn read_context(v: &Value) -> Result<EffectContext, Diagnostic> {
    keys(
        v,
        &[
            "turn",
            "operation",
            "effect",
            "authorization_binding",
            "state",
            "proposal",
            "arguments",
        ],
    )?;
    let c = EffectContext {
        turn: number(v, "turn")?,
        operation: text(v, "operation")?.to_owned(),
        effect: text(v, "effect")?.to_owned(),
        authorization_binding: text(v, "authorization_binding")?.to_owned(),
        state: value::decode(&v["state"])?,
        proposal: text(v, "proposal")?.to_owned(),
        arguments: value::decode_fields(&v["arguments"])?,
    };
    context(&c)?;
    Ok(c)
}
pub(super) fn event(e: &JournalEvent) -> Result<Value, Diagnostic> {
    Ok(match e {
        JournalEvent::StageReservation { turn, stage, fuel } => {
            json!({"kind":"stage_reservation","turn":turn,"stage":stage,"fuel":fuel})
        }
        JournalEvent::Intent(c) => json!({"kind":"intent","context":context(c)?}),
        JournalEvent::Observed {
            context: c,
            result,
            failure,
        } => {
            json!({"kind":"observed","context":context(c)?,"result":value::fields(result)?,"failure":failure})
        }
        JournalEvent::Transition {
            context: c,
            transition,
            value: v,
        } => {
            json!({"kind":"transition","context":context(c)?,"transition":transition,"value":value::encode(v)?})
        }
    })
}
fn read_event(v: &Value) -> Result<JournalEvent, Diagnostic> {
    Ok(match text(v, "kind")? {
        "stage_reservation" => {
            keys(v, &["kind", "turn", "stage", "fuel"])?;
            JournalEvent::StageReservation {
                turn: number(v, "turn")?,
                stage: text(v, "stage")?.to_owned(),
                fuel: number(v, "fuel")?,
            }
        }
        "intent" => {
            keys(v, &["kind", "context"])?;
            JournalEvent::Intent(read_context(&v["context"])?)
        }
        "observed" => {
            keys(v, &["kind", "context", "result", "failure"])?;
            JournalEvent::Observed {
                context: read_context(&v["context"])?,
                result: value::decode_fields(&v["result"])?,
                failure: if v["failure"].is_null() {
                    None
                } else {
                    Some(text(v, "failure")?.to_owned())
                },
            }
        }
        "transition" => {
            keys(v, &["kind", "context", "transition", "value"])?;
            JournalEvent::Transition {
                context: read_context(&v["context"])?,
                transition: text(v, "transition")?.to_owned(),
                value: value::decode(&v["value"])?,
            }
        }
        _ => return Err(rejected("event.kind")),
    })
}
pub(super) fn encode(j: &OperationCheckpoint) -> String {
    let entries:Vec<_> = j.entries.iter().map(|e| json!({"generation":e.generation,"prior_digest":e.prior_digest,"usage":usage(e.usage),"event":event(&e.event).expect("validated checkpoint event"),"digest":e.digest})).collect();
    format!(
        "{}\n",
        json!({"schema":CHECKPOINT_SCHEMA,"binding":binding(j),"generation":j.generation(),"digest":j.digest(),"entries":entries})
    )
}
pub(super) fn decode(
    document: &str,
    expected: &CheckpointIdentity,
) -> Result<OperationCheckpoint, Diagnostic> {
    if document.len() > MAX_BYTES {
        return Err(rejected("bytes"));
    }
    let v: Value = serde_json::from_str(document).map_err(|_| rejected("json"))?;
    keys(
        &v,
        &["schema", "binding", "generation", "digest", "entries"],
    )?;
    if text(&v, "schema")? != CHECKPOINT_SCHEMA {
        return Err(rejected("schema"));
    }
    let b = &v["binding"];
    keys(b, &["identity", "limits"])?;
    let i = &b["identity"];
    keys(
        i,
        &[
            "execution_revision",
            "invocation",
            "registry",
            "program_root",
        ],
    )?;
    let identity = CheckpointIdentity {
        execution_revision: text(i, "execution_revision")?.to_owned(),
        invocation: text(i, "invocation")?.to_owned(),
        registry: text(i, "registry")?.to_owned(),
        program_root: text(i, "program_root")?.to_owned(),
    };
    if &identity != expected {
        return Err(rejected("identity.substitution"));
    }
    let l = &b["limits"];
    keys(
        l,
        &[
            "calls",
            "argument_bytes",
            "result_bytes",
            "total_bytes",
            "reserved_fuel",
        ],
    )?;
    let limits = CheckpointLimits {
        calls: number(l, "calls")?,
        argument_bytes: number(l, "argument_bytes")?,
        result_bytes: number(l, "result_bytes")?,
        total_bytes: number(l, "total_bytes")?,
        reserved_fuel: number(l, "reserved_fuel")?,
    };
    let mut journal = OperationCheckpoint::new(identity, limits)?;
    let entries = v["entries"].as_array().ok_or_else(|| rejected("entries"))?;
    if entries.len() > MAX_ENTRIES {
        return Err(rejected("entries"));
    }
    for row in entries {
        keys(
            row,
            &["generation", "prior_digest", "usage", "event", "digest"],
        )?;
        let entry = journal.make_entry(read_event(&row["event"])?, read_usage(&row["usage"])?)?;
        if number(row, "generation")? != entry.generation
            || text(row, "prior_digest")? != entry.prior_digest
            || text(row, "digest")? != entry.digest
        {
            return Err(rejected("generation.chain"));
        }
        journal.entries.push(entry);
    }
    if number(&v, "generation")? != journal.generation()
        || text(&v, "digest")? != journal.digest()
        || encode(&journal) != document
    {
        return Err(rejected("canonical"));
    }
    Ok(journal)
}
