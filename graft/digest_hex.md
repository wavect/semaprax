---
covers: []
---
# digest_hex.rs

- LowerHex · struct · L10-L10 — pub struct LowerHex<T>(pub T);
- fmt · function · L13-L25 — fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result
- LOWER · constant · L14-L14 — const LOWER: &[u8; 16] = b"0123456789abcdef";
- tests · module · L29-L92 — mod tests
- render · function · L34-L38 — fn render(digest: &[u8]) -> Result<String, fmt::Error>
- every_byte_renders_both_nibbles_in_order · function · L41-L59 — fn every_byte_renders_both_nibbles_in_order()
- rendering_is_injective_and_fixed_width_over_every_byte_value · function · L62-L77 — fn rendering_is_injective_and_fixed_width_over_every_byte_value()
- digests_that_are_not_thirty_two_bytes_are_refused · function · L80-L91 — fn digests_that_are_not_thirty_two_bytes_are_refused()
