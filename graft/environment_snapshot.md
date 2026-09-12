---
covers: []
---
# environment_snapshot.rs

- MAX_ENVIRONMENT_ENTRIES · constant · L10-L10 — pub const MAX_ENVIRONMENT_ENTRIES: usize = 256;
- MAX_ENVIRONMENT_BYTES · constant · L11-L11 — pub const MAX_ENVIRONMENT_BYTES: usize = 65_536;
- EnvironmentSnapshotError · enum · L14-L20 — pub enum EnvironmentSnapshotError
- EnvironmentSnapshot · struct · L23-L27 — pub struct EnvironmentSnapshot
- fmt · function · L30-L36 — fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result
- default · function · L40-L42 — fn default() -> Self
- empty · function · L46-L51 — pub fn empty() -> Self
- from_entries · function · L53-L55 — pub fn from_entries(entries: Vec<(String, String)>) -> Result<Self, EnvironmentSnapshotError>
- from_raw_entries · function · L57-L82 — pub fn from_raw_entries(
- len · function · L84-L86 — pub fn len(&self) -> usize
- is_empty · function · L88-L90 — pub fn is_empty(&self) -> bool
- byte_len · function · L92-L94 — pub fn byte_len(&self) -> usize
- get · function · L96-L103 — pub fn get(&self, index: usize) -> Option<(&str, &str)>
- entries · function · L105-L112 — pub fn entries(&self) -> impl ExactSizeIterator<Item = (&str, &str)> + '_
- raw_entry · function · L115-L119 — pub(crate) fn raw_entry(&self, index: usize) -> Option<(Arc<[u8]>, Arc<[u8]>)>
- canonicalize · function · L121-L171 — fn canonicalize(mut entries: Vec<(String, String)>) -> Result<Self, EnvironmentSnapshotError>
- tests · module · L175-L294 — mod tests
- canonicalizes_byte_order_and_keeps_empty_values · function · L179-L192 — fn canonicalizes_byte_order_and_keeps_empty_values()
- exact_bounds_admit_and_first_excess_rejects · function · L195-L236 — fn exact_bounds_admit_and_first_excess_rejects()
- rejects_raw_utf8_names_values_and_invalid_entries · function · L239-L279 — fn rejects_raw_utf8_names_values_and_invalid_entries()
- empty_and_clones_are_cheap_immutable_views · function · L282-L293 — fn empty_and_clones_are_cheap_immutable_views()
