# project/admission/mod.rs

- flat_record · module · L8-L8 — mod flat_record;
- legacy · module · L9-L9 — mod legacy;
- native_callback · module · L10-L10 — mod native_callback;
- nested_record · module · L11-L11 — mod nested_record;
- owned · module · L12-L12 — mod owned;
- tests · module · L15-L15 — mod tests;
- PreparedProjectAdmission · enum · L30-L55 — pub(super) enum PreparedProjectAdmission
- profile · function · L58-L79 — pub(super) fn profile(&self) -> ProjectProfile
- owned_descriptor · function · L81-L88 — pub(super) fn owned_descriptor(&self) -> Option<&PublicApiDescriptor>
- flat_record_descriptor · function · L90-L95 — pub(super) fn flat_record_descriptor(&self) -> Option<&FlatOwnedRecordApiDescriptor>
- nested_record_descriptor · function · L97-L102 — pub(super) fn nested_record_descriptor(&self) -> Option<&NestedOwnedRecordApiDescriptor>
- scalar_wit_descriptor · function · L104-L109 — pub(super) fn scalar_wit_descriptor(&self) -> Option<&ScalarWitInterfaceArtifactV1>
- prepare · function · L115-L221 — pub(super) fn prepare(
