//! Read-only facts from an actual physical reducer stage. JSON here is inert:
//! no ACK, cleanup permission, owner constructor or restoration path exists.
use super::*;
use crate::resumable_effects::owned_frame::v2::{self, CheckedOwnedAgentWaitBindingV8};
use serde_json::{json, Value as Json};

pub(crate) struct CheckedLiveOwnedReduceStageFactsV8 {
    plan: CheckedOwnedReduceV2,
    allowance: usize,
    consumed: usize,
    effect_settled: u32,
    step: Option<Json>,
    basis: Json,
    operations: Json,
    flags: Vec<u32>,
}
impl CheckedLiveOwnedReduceStageFactsV8 {
    pub(crate) fn allowance(&self) -> usize {
        self.allowance
    }
    pub(crate) fn consumed(&self) -> usize {
        self.consumed
    }
    pub(crate) fn effect_settled(&self) -> u32 {
        self.effect_settled
    }
    pub(crate) fn step(&self) -> Option<&Json> {
        self.step.as_ref()
    }
    pub(crate) fn operations(&self) -> &Json {
        &self.operations
    }
    pub(crate) fn active_flags(&self) -> &[u32] {
        &self.flags
    }
    /// A coordinate is descriptive only. The actual source append producer
    /// must bind it to its real session/ACK; this method cannot attest a row.
    pub(crate) fn cleanup_basis(&self, staged: Option<u32>) -> Result<Json, Diagnostic> {
        let mut basis = self.basis.clone();
        if self.step.is_some() {
            basis["staged"] = json!(staged.ok_or_else(|| rejected(
                "successful Step needs a descriptive staged coordinate"
            ))?);
        } else if staged.is_some() {
            return Err(rejected("failed reducer cannot cite a full staged Step"));
        }
        v2::validate_owned_reduce_cleanup_v8(&self.plan, &basis, &self.operations)
            .map_err(|_| rejected("actual reducer compiler cleanup basis differs"))?;
        Ok(basis)
    }
}

impl StagedExecutedOwnedReduceV2<'_> {
    pub(crate) fn live_stage_facts(
        &self,
        binding: &CheckedOwnedAgentWaitBindingV8,
    ) -> Result<CheckedLiveOwnedReduceStageFactsV8, Diagnostic> {
        let s = &self.staged;
        let (allowance, consumed) = self.observed_fuel();
        if !self.validate_store()
            || s.settlement_started
            || self.inputs.execution.wait().binding() != binding.binding()
            || s.plan.binding() != binding.binding()
            || !s.plan.helper().same_helper(binding.helper())
            || consumed > allowance
            || allowance == 0
            || s.case
                .is_some_and(|i| s.plan.transfers().cases.get(i).is_none())
            || s.case
                .is_some_and(|i| s.transferred > s.plan.transfers().cases[i].fields.len())
            || (s.provisional && (s.case.is_none() || !matches!(s.step, Some(Value::Variant(_)))))
            || (s.failure.is_none() && !s.provisional)
        {
            return Err(rejected("actual reducer stage identity/phase differs"));
        }
        let actions = step::actions(s);
        let flags = step::active_flags(s);
        if !step::pending_matches(s, &actions, &flags) {
            return Err(rejected("actual reducer pending roots/flags differ"));
        }
        let operations = v2::owned_wait_operations_v8(&actions)
            .map_err(|_| rejected("compiler reducer vector differs"))?;
        let flags = flags.iter().map(|f| f.0).collect::<Vec<_>>();
        let status = s.failure.as_ref().map(status).transpose()?;
        let mut basis = if s.provisional {
            let case = &s.plan.transfers().cases[s.case.expect("checked constructor")];
            json!({"kind":if status.is_some(){"provisional_failure"}else{"success"},"constructor":case.constructor.as_str(),"case":case.case.as_str(),"active_flags":flags})
        } else if let Some(i) = s.case {
            let case = &s.plan.transfers().cases[i];
            json!({"kind":"partial_failure","constructor":case.constructor.as_str(),"case":case.case.as_str(),"transfer_prefix":case.fields[..s.transferred].iter().map(|f|f.at.as_str()).collect::<Vec<_>>(),"active_flags":flags})
        } else {
            json!({"kind":"initial_failure"})
        };
        if let Some(status) = status {
            basis["status"] = status;
        }
        let value = if s.failure.is_none() {
            Some(step_value(s)?)
        } else {
            None
        };
        if value.is_none() {
            v2::validate_owned_reduce_cleanup_v8(&s.plan, &basis, &operations)
                .map_err(|_| rejected("actual reducer failure basis differs"))?;
        }
        Ok(CheckedLiveOwnedReduceStageFactsV8 {
            plan: s.plan.clone(),
            allowance,
            consumed,
            effect_settled: self.effect_settled,
            step: value,
            basis,
            operations,
            flags,
        })
    }
}
fn step_value(s: &StagedOwnedReduceV2) -> Result<Json, Diagnostic> {
    let Some(Value::Variant(root)) = &s.step else {
        return Err(rejected("actual full Step missing"));
    };
    let declared = s
        .plan
        .helper()
        .program()
        .declarations
        .case_fields(&root.case)
        .ok_or_else(|| rejected("actual Step case differs"))?;
    let fields=declared.iter().map(|field| {
        let actual=root.fields.get(&field.id).ok_or_else(||rejected("actual Step field missing"))?;
        let value=match actual {
            Value::Bytes(bytes)=>json!({"kind":"bytes","hex":crate::live_invocation::identity::hex(&bytes.bytes)}),
            scalar=>crate::interpreter::resumable::checkpoint::scalar_json(
                &crate::interpreter::resumable::argument_of(scalar).ok_or_else(||rejected("actual Step leaf differs"))?),
        };
        Ok(json!({"identity":field.id.as_str(),"value":value}))
    }).collect::<Result<Vec<_>,Diagnostic>>()?;
    let value =
        json!({"declaration":root.variant.as_str(),"case":root.case.as_str(),"fields":fields});
    v2::validate_owned_reduce_step_v8(&s.plan, &value)
        .map_err(|_| rejected("actual Step schema differs"))?;
    Ok(value)
}
fn status(failure: &OwnedFrameFailure) -> Result<Json, Diagnostic> {
    let (tag, language) = match failure {
        OwnedFrameFailure::Language(s) => (
            "language_failure",
            serde_json::from_str(&s.to_json())
                .map_err(|_| rejected("actual reducer status differs"))?,
        ),
        OwnedFrameFailure::FuelExhausted => ("fuel_exhausted", Json::Null),
        OwnedFrameFailure::HostAbandoned => ("host_abandoned", Json::Null),
        OwnedFrameFailure::AnswerTypeMismatch => ("answer_type_mismatch", Json::Null),
        OwnedFrameFailure::EvaluationRejected => ("evaluation_rejected", Json::Null),
        OwnedFrameFailure::HandlerFailed => ("handler_failed", Json::Null),
        OwnedFrameFailure::CallDepthExceeded => ("call_depth_exceeded", Json::Null),
    };
    Ok(json!({"failure":tag,"language_status":language}))
}
#[cfg(test)]
mod tests;
