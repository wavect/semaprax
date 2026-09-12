# source_verify/place.rs

- SourcePlace · struct · L12-L18 — pub(super) struct SourcePlace
- source_place · function · L20-L54 — pub(super) fn source_place(
- check_source_place_availability · function · L56-L119 — pub(super) fn check_source_place_availability(
- overlapping_place_state · function · L121-L146 — pub(super) fn overlapping_place_state(binding: &Binding, requested: &[String]) -> Availability
- path_is_prefix · function · L148-L150 — pub(super) fn path_is_prefix<T: PartialEq>(prefix: &[T], path: &[T]) -> bool
- join_moved_places · function · L152-L177 — pub(super) fn join_moved_places(
- join_definitely_partial · function · L179-L199 — pub(super) fn join_definitely_partial(left: &Binding, right: &Binding) -> HashSet<Vec<String>>
