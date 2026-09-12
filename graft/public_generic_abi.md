---
covers: []
---
# public_generic_abi.rs

- boundary_profile · module · L27-L27 — pub mod boundary_profile;
- carrier · module · L28-L28 — pub mod carrier;
- classifier · module · L29-L29 — pub mod classifier;
- descriptor · module · L30-L30 — pub mod descriptor;
- interpreter · module · L31-L31 — pub mod interpreter;
- native · module · L32-L32 — pub mod native;
- wasm · module · L33-L33 — pub mod wasm;
- frame · function · L38-L41 — pub(crate) fn frame(preimage: &mut Vec<u8>, bytes: &[u8])
- read_frame · function · L47-L58 — pub(crate) fn read_frame(input: &[u8], offset: usize, max_len: usize) -> Option<(&[u8], usize)>
- digest · function · L63-L70 — pub(crate) fn digest(domain: &[u8], bytes: &[u8]) -> String
- tests · module · L73-L111 — mod tests
- frame_and_read_frame_round_trip · function · L77-L89 — fn frame_and_read_frame_round_trip()
- read_frame_rejects_truncated_header · function · L92-L94 — fn read_frame_rejects_truncated_header()
- read_frame_rejects_declared_length_past_input · function · L97-L103 — fn read_frame_rejects_declared_length_past_input()
- read_frame_rejects_length_over_max · function · L106-L110 — fn read_frame_rejects_length_over_max()
