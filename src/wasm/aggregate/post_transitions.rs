//! Apply replay-authenticated non-variant cleanup transitions after a Wasm value.
use super::*;

impl Emitter<'_> {
    pub(super) fn apply_post_transitions(
        &mut self,
        expression: &ExpressionId,
        value: &Value,
    ) -> Result<(), Diagnostic> {
        let transitions = self
            .cleanup_plan
            .blocks
            .iter()
            .flat_map(|block| &block.transitions)
            .filter(|transition| match transition {
                crate::cleanup_plan::CleanupTransition::Initialize { at, .. }
                | crate::cleanup_plan::CleanupTransition::InitializeVariant { at, .. }
                | crate::cleanup_plan::CleanupTransition::Transfer { at, .. }
                | crate::cleanup_plan::CleanupTransition::Renew { at, .. }
                | crate::cleanup_plan::CleanupTransition::TransferVariant { at, .. } => {
                    at == expression
                }
                _ => false,
            })
            .cloned()
            .collect::<Vec<_>>();
        let mut completed_variant_destinations = std::collections::BTreeSet::new();
        for transition in &transitions {
            match transition {
                crate::cleanup_plan::CleanupTransition::Initialize { destination, .. } => {
                    if !self.set_variant_storage_flags_from_value(destination, value)? {
                        self.set_storage_flag(destination, true)?;
                    }
                }
                crate::cleanup_plan::CleanupTransition::InitializeVariant {
                    destination,
                    variant,
                    ..
                } => {
                    let layout = variant_layout(self.variant_layouts, value_type(value))?;
                    if layout.variant != *variant
                        || !self.set_variant_storage_flags_from_value(destination, value)?
                    {
                        return Err(error(
                            "conditional variant initialization disagrees with carrier",
                        ));
                    }
                }
                crate::cleanup_plan::CleanupTransition::Transfer {
                    source,
                    destination,
                    ..
                }
                | crate::cleanup_plan::CleanupTransition::Renew {
                    source,
                    destination,
                    ..
                } => {
                    if let Some(local) = self
                        .plan
                        .cleanup_call_argument_carriers
                        .get(&destination.storage)
                        .copied()
                    {
                        if *value_type(value) != ResolvedType::Bytes
                            && !crate::cleanup::is_owned_bounded_vec_type(value_type(value))
                            && !crate::cleanup::is_owned_bounded_box_type(value_type(value))
                        {
                            return Err(error(
                                "owned call epoch requires an exact Bytes or bounded Vec carrier",
                            ));
                        }
                        self.get_scalar(value);
                        self.output.push(0x21);
                        write_u32(self.output, local);
                    }
                    self.set_storage_flag(source, false)?;
                    if !self.set_variant_storage_flags_from_value(destination, value)? {
                        self.set_storage_flag(destination, true)?;
                    }
                }
                crate::cleanup_plan::CleanupTransition::TransferVariant {
                    source: _,
                    destination,
                    variant,
                    ..
                } => {
                    let key = (destination.clone(), variant.clone());
                    if completed_variant_destinations.insert(key) {
                        let sources = transitions
                            .iter()
                            .filter_map(|candidate| match candidate {
                                crate::cleanup_plan::CleanupTransition::TransferVariant {
                                    source,
                                    destination: candidate_destination,
                                    variant: candidate_variant,
                                    ..
                                } if candidate_destination == destination
                                    && candidate_variant == variant =>
                                {
                                    Some(source.clone())
                                }
                                _ => None,
                            })
                            .collect::<Vec<_>>();
                        self.apply_variant_transfer_group(&sources, destination, variant, value)?;
                    }
                }
                crate::cleanup_plan::CleanupTransition::AuthenticateVariantCase { .. } => {}
                _ => unreachable!("filtered byte cleanup transition"),
            }
        }
        Ok(())
    }
}
