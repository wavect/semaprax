# Alias layout Drop-order compiler correction

Status: historical source evidence; qualification is limited to the recorded revisions and results.

Audience: compiler contributors and reviewers.

Source basis: d719ea241120655b618ca85a185d5fe5c575a2bf. End the tuple containing the registry-borrowing layout before moving the registry into its own final Drop statement. Destruction order stays actual, expected, layout, view, owner, registry. Every original assertion and preceding owner Drop remains unchanged.

Source-only exact inverse replay passed against the complete immutable Git input. No Cargo, compiler, Rust tests or executable qualification ran. No new selector: the existing retained_same_name_views_lend_actual_provider_payload_in_original_order_and_hir owner exercises this path after compilation.

Integration must add a complete current-to-d719 input view for this changed test file before the existing protected #847 source guard namespace; old JSON tables/hashes/archives remain unchanged. This packet records the full immediate original and the exact one-site inverse; it does not update runtime source routing itself.
