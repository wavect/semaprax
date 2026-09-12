# project/candidate/extraction_owned.rs

- validate · function · L5-L213 — pub(super) fn validate(
- correspondence · function · L215-L221 — fn correspondence(owner_capture: bool, message: &'static str) -> Vec<Diagnostic>
- function · function · L223-L233 — fn function<'a>(revision: &'a ProjectRevision, id: &str) -> Result<&'a ResolvedFunction>
- Ids · struct · L236-L240 — struct Ids
- insert · function · L242-L251 — fn insert(&mut self, old: &str, new: &str) -> Result<()>
- root · function · L252-L257 — fn root(&self, old: &ValueId, new: &ValueId) -> Result<()>
- binding · function · L258-L265 — fn binding(&mut self, old: &ResolvedBinding, new: &ResolvedBinding) -> Result<()>
- pattern · function · L266-L318 — fn pattern(
- fields · function · L319-L353 — fn fields(
- pair · function · L356-L524 — fn pair(
- tests · module · L527-L609 — mod tests
- rebuilt_pair_rejects_tampering · function · L534-L598 — fn rebuilt_pair_rejects_tampering(target: &str, change_callee: bool)
- rebuilt_helper_rejects_same_typed_lexical_root_substitution · function · L601-L603 — fn rebuilt_helper_rejects_same_typed_lexical_root_substitution()
- rebuilt_helper_rejects_changed_stable_callee_identity · function · L606-L608 — fn rebuilt_helper_rejects_changed_stable_callee_identity()
