//! Private composition root for an already-admitted callable-v3 image.
//!
//! This is intentionally not re-exported and performs no library admission.
//! It only connects the loader's exact leaf pin to the host receipt/ledger
//! authority after both independent descriptor and loader gates succeeded.

#![forbid(unsafe_code)]
#![allow(
    dead_code,
    reason = "callable-v3 public admission remains closed by SPX-B104"
)]

use semaprax_native_loader::NativeSettlementModuleLease;

use crate::callable_wire_v3::RecoveryIdentity;
use crate::descriptor_v3::Descriptor;
use crate::receipt_authority::ReceiptAuthority;
use crate::settlement_ledger::{
    CommittedResult, SettlementLedger, SettlementLedgerError, SettlementTransaction,
};

/// Private exact-instance runtime. The outer loader pin is last, while every
/// frame/cache/quarantine owns its own explicit retain through the ledger.
pub(crate) struct PrivateSettlementHostV3 {
    ledger: SettlementLedger<NativeSettlementModuleLease>,
    module_lease: NativeSettlementModuleLease,
}

impl PrivateSettlementHostV3 {
    pub(crate) fn from_admitted(
        module_lease: NativeSettlementModuleLease,
        expected_descriptor: &[u8],
    ) -> Result<Self, SettlementLedgerError> {
        let descriptor = parse_exact_admitted_descriptor(expected_descriptor, |candidate| {
            module_lease.descriptor_matches(candidate)
        })?;
        let loader = module_lease.capacities();
        if loader.request() != descriptor.capacities.request as usize
            || loader.execute_response() != descriptor.capacities.execute_response as usize
            || loader.frame() != descriptor.capacities.frame as usize
            || loader.decision() != descriptor.capacities.decision as usize
            || loader.candidate_receipt() != descriptor.capacities.candidate_receipt as usize
        {
            return Err(SettlementLedgerError::CapacityExhausted);
        }
        let instance_nonce = std::num::NonZeroU64::new(module_lease.instance_id().get())
            .expect("loader instance identities are structurally nonzero");
        let authority = ReceiptAuthority::from_os(instance_nonce)?;
        let ledger = SettlementLedger::try_new(module_lease.retain(), descriptor, authority)?;
        Ok(Self {
            ledger,
            module_lease,
        })
    }

    pub(crate) fn reserve(
        &self,
    ) -> Result<SettlementTransaction<'_, NativeSettlementModuleLease>, SettlementLedgerError> {
        self.ledger.reserve()
    }

    pub(crate) fn replay_committed(
        &mut self,
        identity: RecoveryIdentity,
        candidate_bytes: &[u8],
    ) -> Result<CommittedResult, SettlementLedgerError> {
        self.ledger.replay_committed(identity, candidate_bytes)
    }

    pub(crate) fn is_poisoned(&self) -> bool {
        self.ledger.is_poisoned()
    }

    pub(crate) fn is_draining(&self) -> bool {
        self.ledger.is_draining()
    }

    pub(crate) fn module_instance_id(&self) -> semaprax_native_loader::ModuleInstanceId {
        self.module_lease.instance_id()
    }
}

fn parse_exact_admitted_descriptor(
    expected_descriptor: &[u8],
    mut admitted_matches: impl FnMut(&[u8]) -> bool,
) -> Result<Descriptor, SettlementLedgerError> {
    if !admitted_matches(expected_descriptor) {
        return Err(SettlementLedgerError::DescriptorMismatch);
    }
    Descriptor::parse(expected_descriptor).map_err(|_| SettlementLedgerError::DescriptorMismatch)
}

#[cfg(test)]
mod tests {
    use semaprax::codegen::emit_native_callable_v3_descriptor;
    use semaprax::hir::DeclarationId;
    use semaprax::owned_resource_corpus::build_owned_resource_corpus_v1;

    use super::*;

    #[test]
    fn canonical_same_capacity_descriptor_substitution_fails_exact_binding() {
        let corpus = build_owned_resource_corpus_v1().unwrap();
        let image_a = emit_native_callable_v3_descriptor(
            &corpus.program,
            &DeclarationId::new("token.discard"),
        )
        .unwrap();
        let image_b = emit_native_callable_v3_descriptor(
            &corpus.program,
            &DeclarationId::new("token.identity"),
        )
        .unwrap();
        let parsed_a = Descriptor::parse(image_a.bytes()).unwrap();
        let parsed_b = Descriptor::parse(image_b.bytes()).unwrap();
        assert_eq!(parsed_a.capacities.request, parsed_b.capacities.request);
        assert_eq!(
            parsed_a.capacities.execute_response,
            parsed_b.capacities.execute_response
        );
        assert_eq!(parsed_a.capacities.frame, parsed_b.capacities.frame);
        assert_eq!(parsed_a.capacities.decision, parsed_b.capacities.decision);
        assert_eq!(
            parsed_a.capacities.candidate_receipt,
            parsed_b.capacities.candidate_receipt
        );
        assert_ne!(image_a.bytes(), image_b.bytes());
        assert_eq!(
            parse_exact_admitted_descriptor(image_b.bytes(), |candidate| {
                candidate == image_a.bytes()
            }),
            Err(SettlementLedgerError::DescriptorMismatch)
        );
    }
}
