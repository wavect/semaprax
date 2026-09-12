---
covers: []
---
# package_range.rs

- Version · struct · L4-L4 — pub(crate) struct Version(pub(crate) u32, pub(crate) u32, pub(crate) u32);
- Range · enum · L7-L10 — pub(crate) enum Range
- contains · function · L13-L18 — pub(crate) fn contains(self, version: Version) -> bool
- parse_version · function · L21-L42 — pub(crate) fn parse_version(
- parse_range · function · L44-L89 — pub(crate) fn parse_range(
- component · function · L91-L109 — fn component(
- tests · module · L112-L430 — mod tests
- error · function · L115-L117 — fn error(message: String) -> Diagnostic
- version_error · function · L119-L123 — fn version_error(value: &str) -> Diagnostic
- range_error · function · L125-L129 — fn range_error(value: &str) -> Diagnostic
- bounds · function · L131-L136 — fn bounds(text: &str) -> (Version, Version)
- exact_tilde_and_caret_boundaries_match_the_frozen_v1_language · function · L139-L168 — fn exact_tilde_and_caret_boundaries_match_the_frozen_v1_language()
- caret_upper_bound_advances_the_leading_nonzero_component · function · L171-L208 — fn caret_upper_bound_advances_the_leading_nonzero_component()
- tilde_upper_bound_advances_only_the_minor_component · function · L211-L226 — fn tilde_upper_bound_advances_only_the_minor_component()
- exact_range_admits_only_the_named_version · function · L229-L248 — fn exact_range_admits_only_the_named_version()
- component_accepts_only_canonical_unsigned_decimal · function · L251-L301 — fn component_accepts_only_canonical_unsigned_decimal()
- parse_version_requires_exactly_three_dot_separated_components · function · L304-L333 — fn parse_version_requires_exactly_three_dot_separated_components()
- parse_version_enforces_the_frozen_thirty_two_byte_width · function · L336-L362 — fn parse_version_enforces_the_frozen_thirty_two_byte_width()
- parse_range_requires_a_supported_single_byte_operator · function · L365-L397 — fn parse_range_requires_a_supported_single_byte_operator()
- upper_bound_arithmetic_reports_overflow_instead_of_wrapping · function · L400-L421 — fn upper_bound_arithmetic_reports_overflow_instead_of_wrapping()
- version_ordering_compares_major_then_minor_then_patch · function · L424-L429 — fn version_ordering_compares_major_then_minor_then_patch()
