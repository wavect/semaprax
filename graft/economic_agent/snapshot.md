# economic_agent/snapshot.rs

- Utxo · struct · L17-L23 — pub(super) struct Utxo
- SnapshotState · enum · L25-L45 — pub(super) enum SnapshotState
- Snapshot · struct · L47-L53 — pub(super) struct Snapshot
- render_snapshot · function · L55-L61 — pub(super) fn render_snapshot(snapshot: &Snapshot) -> String
- parse_snapshot · function · L63-L259 — pub(super) fn parse_snapshot(source: &str, expected: EconomicRail) -> Result<Snapshot, Diagnostic>
- parse_snapshot_limited · function · L260-L274 — pub(super) fn parse_snapshot_limited(
- reserve_parse_sidecar · function · L275-L285 — pub(super) fn reserve_parse_sidecar(source: &str, limits: &Limits) -> Result<(), Diagnostic>
- lower_hex · function · L286-L291 — pub(super) fn lower_hex(value: &str, n: usize) -> bool
- valid_script · function · L292-L294 — pub(super) fn valid_script(value: &str) -> bool
- hex_bytes · function · L295-L304 — pub(super) fn hex_bytes(value: &str) -> Option<Vec<u8>>
- rlp_bytes · function · L306-L322 — pub(super) fn rlp_bytes(value: &[u8]) -> Vec<u8>
- rlp_u64 · function · L323-L329 — pub(super) fn rlp_u64(value: u64) -> Vec<u8>
- rlp_list · function · L330-L344 — pub(super) fn rlp_list(items: &[Vec<u8>]) -> Vec<u8>
- shortvec · function · L345-L358 — pub(super) fn shortvec(mut value: usize) -> Vec<u8>
