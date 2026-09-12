# assurance_manifest/obligation.rs

- ObligationKind · enum · L12-L29 — pub enum ObligationKind
- ALL · constant · L32-L42 — pub const ALL: [Self; 9] = [
- token · function · L45-L57 — pub const fn token(self) -> &'static str
- from_token · function · L61-L63 — pub fn from_token(token: &str) -> Option<Self>
- obligation_id · function · L72-L80 — pub fn obligation_id(kind: ObligationKind, declaration_id: &str, locator: &str) -> String
- MethodRecord · struct · L88-L102 — pub struct MethodRecord
- new · function · L110-L130 — pub fn new(
- Obligation · struct · L139-L144 — pub struct Obligation
- new · function · L148-L157 — pub fn new(kind: ObligationKind, declaration_id: impl Into<String>, locator: &str) -> Self
- with_method · function · L160-L163 — pub fn with_method(mut self, method: MethodRecord) -> Self
- AssumptionRecord · struct · L170-L180 — pub struct AssumptionRecord
- ExternalRecords · struct · L191-L194 — pub struct ExternalRecords
- tests · module · L197-L232 — mod tests
- kind_tokens_round_trip_through_from_token · function · L201-L206 — fn kind_tokens_round_trip_through_from_token()
- obligation_id_length_prefixing_prevents_boundary_aliasing · function · L209-L217 — fn obligation_id_length_prefixing_prevents_boundary_aliasing()
- obligation_id_is_stable_for_identical_inputs · function · L220-L224 — fn obligation_id_is_stable_for_identical_inputs()
- obligation_id_changes_with_locator_index · function · L227-L231 — fn obligation_id_changes_with_locator_index()
