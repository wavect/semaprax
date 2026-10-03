//! Closed conditional cleanup for Option<String> and Result<String, i64>.
//! The whole-carrier initialized bit is distinct from payload ownership: an
//! inactive case has no Rust table entry and executes no physical finalizer.
use super::*;
use semaprax::cleanup_plan::{CleanupSlot, ConditionalVariantEntry, VariantCaseGuard};

pub(crate) struct Layout {
    pub variant: DeclarationId,
    pub active: DeclarationId,
    pub inactive: DeclarationId,
    pub payload: DeclarationId,
    pub active_tag: u8,
}
impl Layout {
    pub(crate) fn new(
        program: &ResolvedProgram,
        ty: &ResolvedType,
        option: bool,
    ) -> Result<Self, Diagnostic> {
        let ResolvedType::Nominal { declaration, .. } = ty else {
            return Err(sdk_error("native container is not nominal"));
        };
        let cases = program
            .declarations
            .variant_cases(declaration)
            .ok_or_else(|| sdk_error("native container case domain is absent"))?;
        let (active_name, inactive_name, active_tag) = if option {
            ("Some", "None", 1)
        } else {
            ("Ok", "Err", 0)
        };
        let active = cases
            .iter()
            .find(|c| c.name == active_name)
            .ok_or_else(|| sdk_error("native container active case is absent"))?;
        let inactive = cases
            .iter()
            .find(|c| c.name == inactive_name)
            .ok_or_else(|| sdk_error("native container inactive case is absent"))?;
        if cases.len() != 2 || active.fields.len() != 1 {
            return Err(sdk_error("native container case inventory is unsupported"));
        }
        Ok(Self {
            variant: declaration.clone(),
            active: active.id.clone(),
            inactive: inactive.id.clone(),
            payload: active.fields[0].id.clone(),
            active_tag,
        })
    }
    pub(super) fn slot(&self, slot: &CleanupSlot, lifecycle: &DeclarationId) -> bool {
        let FieldLivenessShape::Variant { declaration, cases } = &slot.field_liveness_shape else {
            return false;
        };
        declaration == &self.variant && cases.len()==2 && cases.iter().all(|case| {
            if case.case == self.active {
                case.fields.len()==1 && case.fields[0].field==self.payload && matches!(&case.fields[0].shape, FieldLivenessShape::Leaf { lifecycle: found, .. } if found == lifecycle)
            } else { case.case==self.inactive && case.fields.iter().all(|f| f.shape==FieldLivenessShape::NoDrop) }
        })
    }
    pub(super) fn projected(&self, place: &CleanupPlace) -> bool {
        place.projections.is_empty()
            || place.projections == [self.active.clone(), self.payload.clone()]
    }
    pub(super) fn entry(&self, entry: &ConditionalVariantEntry) -> bool {
        entry.variant == self.variant
            && entry.cases.len() == 2
            && entry.cases.iter().all(|case| {
                if case.case == self.active {
                    case.live_places.len() == 1
                        && case.live_places[0].storage == entry.storage
                        && case.live_places[0].projections
                            == [self.active.clone(), self.payload.clone()]
                } else {
                    case.case == self.inactive && case.live_places.is_empty()
                }
            })
    }
    pub(super) fn finalizer(&self, place: &CleanupPlace, guard: Option<&VariantCaseGuard>) -> bool {
        place.projections == [self.active.clone(), self.payload.clone()]
            && guard.is_some_and(|guard| {
                guard.storage == place.storage
                    && guard.variant == self.variant
                    && guard.case == self.active
            })
    }
}
