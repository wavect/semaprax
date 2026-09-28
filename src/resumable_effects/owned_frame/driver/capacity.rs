//! Decreasing outstanding closures, measured by the ordinary canonical renderer.
//! Placeholder references/counters have maximum wire width; immutable payloads
//! and compiler vectors are exact. Alternative terminal paths are never summed.
use super::*;
use crate::cleanup_plan::StatusProducer;
use crate::conformance::NormalizedStatus;
use crate::hir::ResolvedType;

pub(super) fn remaining(
    state: &State,
    candidate: &Record,
    key: &SourceCheckpointKey,
    historical_resume_pending: bool,
) -> Result<Vec<Vec<Record>>, Error> {
    let mut next = state.clone();
    next.apply(key, candidate)?;
    if next.phase == Phase::Claimed {
        return Ok(vec![vec![]]);
    }
    let plan = &next.plan;
    let argument = next.argument()?.clone();
    let input = codec::decode_input(plan, &argument)?;
    let yields = plan.function().yields.as_ref().ok_or(Error::Binding)?;
    let request = maximum_scalar(&yields.request_type)?;
    let answer = codec::scalar(&maximum_scalar(&yields.response_type)?)?;
    let checkpoint = checkpoint::encode(
        plan,
        key,
        &next.scope,
        &input,
        next.argument_digest()?,
        &request,
        &next.generation,
        u64::MAX,
        u64::MAX,
    )?;
    let digest = "sha256:".to_owned() + &"f".repeat(64);
    let counter = u64::MAX;
    let make = |kind, fields| Record::new(kind, fields);
    let commit = make(
        Kind::ArgumentCommitted,
        json!({"argument_digest":digest,"storage":codec::storage(&plan.liveness().storage)?,"leaf_flags":codec::leaf_flags(plan)}),
    )?;
    let start = make(
        Kind::StartReserved,
        json!({"causal_sequence":counter,"reservation":counter,"reserved_total":counter}),
    )?;
    let yielded = make(
        Kind::Yielded,
        json!({"causal_sequence":counter,"checkpoint":String::from_utf8(checkpoint).map_err(|_|Error::Malformed)?,"checkpoint_digest":digest,"consumed_steps":counter}),
    )?;
    let dispatch = make(
        Kind::Dispatched,
        json!({"yielded_sequence":counter,"checkpoint_digest":digest,"request_digest":digest}),
    )?;
    let answered = make(
        Kind::Answered,
        json!({"dispatched_sequence":counter,"answer":answer,"answer_digest":digest}),
    )?;
    let resume = make(
        Kind::ResumeReserved,
        json!({"answered_sequence":counter,"reservation":counter,"reserved_total":counter}),
    )?;
    let completed = make(
        Kind::Completed,
        json!({"causal_sequence":counter,"result":argument,"result_digest":digest,"pending_cleanup":codec::operations(&plan.liveness().completion_cleanup)?,"consumed_steps":counter}),
    )?;
    let cleanup = make(
        Kind::CleanupStarted,
        json!({"terminal_sequence":counter,"cleanup_digest":digest}),
    )?;
    let claimed = make(
        Kind::ResultClaimed,
        json!({"completed_sequence":counter,"cleanup_settled_sequence":counter,"result_digest":digest}),
    )?;
    let success_pending = codec::operations(&plan.liveness().completion_cleanup)?;
    let success_settled = receipt(&success_pending)?;
    let mut cleanup_pending = codec::operations(&plan.liveness().failure_cleanup)?;
    let result_pending = codec::operations(&plan.liveness().result_disposal)?;
    if codec::canonical(&result_pending).len() > codec::canonical(&cleanup_pending).len() {
        cleanup_pending = result_pending;
    }
    let mut language = Value::Null;
    for source in &plan.function().cleanup_plan.status_sources {
        let statuses = match &source.producer {
            StatusProducer::ContractFalse { phase, .. } => vec![NormalizedStatus::contract(*phase)],
            StatusProducer::CheckedArithmetic {
                normalized_cases, ..
            } => normalized_cases
                .iter()
                .map(|case| NormalizedStatus::arithmetic(*case))
                .collect(),
            StatusProducer::PropagatedCall { .. } => vec![],
        };
        for status in statuses {
            let value = codec::parse(status.to_json().as_bytes(), codec::MAX_CARRIER)?;
            if codec::canonical(&value).len() > codec::canonical(&language).len() {
                language = value;
            }
        }
    }
    let failed = make(
        Kind::Failed,
        json!({"causal_sequence":counter,"failure":"answer_type_mismatch","language_status":language,"pending_cleanup":cleanup_pending,"consumed_steps":counter}),
    )?;
    let failure_settled = receipt(&cleanup_pending)?;
    // Replay validation ACK replaces its already emitted reservation. It does
    // not reserve the original checkpoint or old dispatch/answer a second time.
    let replay=next.replay.as_ref().map(|(_,basis)|make(Kind::ReplayValidated,json!({"reservation_sequence":counter,"basis":max_counters(basis),"consumed_steps":counter}))).transpose()?;
    let mut rows = match next.phase {
        Phase::Created => vec![
            commit, start, yielded, dispatch, answered, resume, completed,
        ],
        Phase::Committed | Phase::Starting => {
            vec![start, yielded, dispatch, answered, resume, completed]
        }
        Phase::Yielded => vec![dispatch, answered, resume, completed],
        Phase::Dispatched => vec![answered, resume, completed],
        Phase::Answered | Phase::Resuming => vec![resume, completed],
        Phase::Completed => vec![],
        Phase::Failed => {
            return Ok(vec![vec![
                cleanup,
                receipt(&next.terminal().ok_or(Error::Binding)?.fields["pending_cleanup"])?,
            ]])
        }
        Phase::CleanupStarted => {
            let terminal = next.terminal().ok_or(Error::Binding)?;
            let mut branch = vec![receipt(&terminal.fields["pending_cleanup"])?];
            if terminal.kind == Kind::Completed {
                branch.push(claimed);
            }
            return Ok(vec![branch]);
        }
        Phase::CleanupSettled => {
            return Ok(vec![
                if next.terminal().is_some_and(|r| r.kind == Kind::Completed) {
                    vec![claimed]
                } else {
                    vec![]
                },
            ])
        }
        Phase::Empty | Phase::Claimed => return Err(Error::Binding),
    };
    // Original in-progress evaluation needs no second original reservation;
    // interrupted recovery does, after the charged historical validation ACK.
    if next.replay.is_none()
        && !next.replay_ready
        && matches!(next.phase, Phase::Starting | Phase::Resuming)
    {
        rows.remove(0);
    }
    if let Some(replay) = replay {
        rows.insert(0, replay);
    }
    if historical_resume_pending {
        let basis = max_counters(&next.basis()?);
        let reservation = make(
            Kind::ReplayReserved,
            json!({"basis":basis,"reservation":counter,"reserved_total":counter}),
        )?;
        let validated = make(
            Kind::ReplayValidated,
            json!({"reservation_sequence":counter,"basis":basis,"consumed_steps":counter}),
        )?;
        let at = usize::from(next.replay.is_some());
        rows.splice(at..at, [reservation, validated]);
    }
    let mut branches = Vec::new();
    for cut in 0..rows.len() {
        let mut branch = rows[..cut].to_vec();
        branch.extend([failed.clone(), cleanup.clone(), failure_settled.clone()]);
        branches.push(branch);
    }
    rows.extend([cleanup, success_settled, claimed]);
    branches.push(rows);
    Ok(branches)
}
fn receipt(pending: &Value) -> Result<Record, Error> {
    let operations = pending
        .as_array()
        .ok_or(Error::Binding)?
        .iter()
        .map(|operation| json!({"operation":operation,"outcome":"completed"}))
        .collect::<Vec<_>>();
    Record::new(
        Kind::CleanupSettled,
        json!({"cleanup_started_sequence":u64::MAX,"receipt":{"kind":"observed","settlement":"completed","operations":operations}}),
    )
}
fn maximum_scalar(ty: &ResolvedType) -> Result<ArgumentValue, Error> {
    Ok(match ty {
        ResolvedType::I64 => ArgumentValue::Int(i64::MIN),
        ResolvedType::I32 => ArgumentValue::Int32(i32::MIN),
        ResolvedType::U8 => ArgumentValue::Uint8(u8::MAX),
        ResolvedType::Usize => ArgumentValue::Usize(u32::MAX as u64),
        ResolvedType::Char => ArgumentValue::Char(0x10ffff),
        ResolvedType::F32 => ArgumentValue::Float32(u32::MAX),
        ResolvedType::F64 => ArgumentValue::Float64(u64::MAX),
        ResolvedType::Bool => ArgumentValue::Bool(false),
        _ => return Err(Error::Binding),
    })
}
fn max_counters(value: &Value) -> Value {
    match value {
        Value::Number(_) => json!(u64::MAX),
        Value::Object(map) => Value::Object(
            map.iter()
                .map(|(key, value)| (key.clone(), max_counters(value)))
                .collect(),
        ),
        _ => value.clone(),
    }
}
