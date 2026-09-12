# assurance_manifest/delta.rs

- DELTA_SCHEMA · constant · L15-L15 — pub(super) const DELTA_SCHEMA: &str = "semaprax.assurance-manifest-delta.v1";
- payload_of · function · L17-L21 — fn payload_of(envelope: &str) -> Value
- Entry · struct · L23-L26 — struct Entry
- entries · function · L28-L50 — fn entries(payload: &Value) -> BTreeMap<String, Entry>
- review_by_map · function · L52-L60 — fn review_by_map(payload: &Value) -> BTreeMap<String, Option<String>>
- quote · function · L62-L64 — fn quote(value: &str) -> String
- delta · function · L75-L169 — pub fn delta(
- tests · module · L172-L248 — mod tests
- fixture_sha256 · function · L177-L179 — fn fixture_sha256() -> String
- envelope · function · L181-L191 — fn envelope(obligations: &[Obligation]) -> String
- identical_manifests_have_an_empty_delta · function · L194-L205 — fn identical_manifests_have_an_empty_delta()
- added_and_removed_are_detected · function · L208-L220 — fn added_and_removed_are_detected()
- a_dominance_increase_is_strengthened_not_reclassified · function · L223-L233 — fn a_dominance_increase_is_strengthened_not_reclassified()
- an_incomparable_change_is_reclassified_not_forced_either_way · function · L236-L247 — fn an_incomparable_change_is_reclassified_not_forced_either_way()
