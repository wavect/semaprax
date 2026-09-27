//! Compiler-owned Copy projection of an already decoded Proposal.
use super::*;
use crate::hir::DeclarationId;
use crate::interpreter::resumable::ResumableChannelValue;
use crate::interpreter::ArgumentValue;
use shape::{FieldRow, Representation};

impl CompiledAgentProposalSchema {
    pub(crate) fn model_wait_carrier(
        &self,
        decoded: &DecodedProposal,
    ) -> Option<ResumableChannelValue> {
        if decoded.agent_id() != self.schema.agent_id
            || decoded.proposal_schema_digest() != self.schema.digest
            || decoded.case().is_some()
        {
            return None;
        }
        let Shape::Record { fields } = &self.shape else {
            return None;
        };
        if fields.len() != decoded.fields().len() {
            return None;
        }
        let fields = fields
            .iter()
            .zip(decoded.fields())
            .map(|(expected, actual)| {
                (expected.stable_id == actual.stable_id())
                    .then(|| copy_scalar(expected, actual.value()))?
            })
            .collect::<Option<Vec<_>>>()?;
        Some(ResumableChannelValue::Record {
            declaration: DeclarationId::new(&self.schema.proposal_type_id),
            fields,
        })
    }
}

fn copy_scalar(field: &FieldRow, value: &ProposalValue) -> Option<ArgumentValue> {
    Some(match (field.representation, value) {
        (Representation::Bool, ProposalValue::Bool(v)) => ArgumentValue::Bool(*v),
        (Representation::I32, ProposalValue::Signed(v)) => {
            ArgumentValue::Int32((*v).try_into().ok()?)
        }
        (Representation::I64, ProposalValue::Signed(v)) => ArgumentValue::Int(*v),
        (Representation::U8, ProposalValue::Unsigned(v)) => {
            ArgumentValue::Uint8((*v).try_into().ok()?)
        }
        (Representation::U64, ProposalValue::Unsigned(v)) => ArgumentValue::Usize(*v),
        _ => return None,
    })
}
