# image_transport/vnext/read_batch.rs

- MAX_FRAMES · constant · L5-L5 — const MAX_FRAMES: usize = 16;
- MAX_WORKERS · constant · L6-L6 — const MAX_WORKERS: usize = 4;
- Read · enum · L8-L15 — pub(super) enum Read
- DetachedRead · enum · L17-L29 — enum DetachedRead<'a>
- handle_read_batch · function · L35-L94 — pub fn handle_read_batch(
- parallel_read_methods · function · L98-L110 — pub fn parallel_read_methods(&self) -> Vec<&'static str>
- ReadContext · struct · L115-L122 — pub(super) struct ReadContext<'a>
- execute · function · L126-L213 — pub(super) fn execute(
- parallel_read · function · L215-L243 — pub(super) fn parallel_read(operation: Operation) -> bool
- prepare_read · function · L245-L300 — pub(super) fn prepare_read(
- parallel_map · function · L304-L345 — fn parallel_map<T: Sync, R: Send>(
- tests · module · L348-L455 — mod tests
- dependency_query_extends_only_the_immutable_batch_subset · function · L353-L407 — fn dependency_query_extends_only_the_immutable_batch_subset()
- scoped_workers_overlap_and_restore_input_order · function · L410-L440 — fn scoped_workers_overlap_and_restore_input_order()
- worker_panic_discards_results_and_joins_other_workers · function · L443-L454 — fn worker_panic_discards_results_and_joins_other_workers()
