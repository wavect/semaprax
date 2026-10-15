# OPT811 residual component census correction

Status: retained evidence; the recorded scope and qualification below apply.

Audience: compiler contributors, tool authors, and reviewers.

Base f9aadf17cbca430480f8bcf40afbcb58cd9a0711. Source-only; no Cargo/build/test/Clippy executed.

The earlier invocation-local feedback correction and missing ordinary/conditional/string-move terms already exist at this base. They are not newly attributed to the historical pin mismatch. The frozen bf29 owner failed at the unchanged pin with [9224,443313] versus [9224,132070]; neither value establishes allocator highwater or a measured old/new allocation regression.

This batch exposes three distinct readings: standalone builder component maximum across in-scope and out-of-scope builders; separate component-maxima overlap sum; and sampled phase-paired census. Current staged backing and its retained maximum are distinct. A builder sample pairs only with current staged and attached backing. After the builder returns, staging pairs with the still-held immutable ModuleIndex instead, including the no-builder/empty-helper path. No previous builder maximum feeds this phase sample. The legacy numeric reader remains the original outside-scope maximum versus compatibility component sum. Named overlap accessors return None after invalid/overflow observations, sticky until reset. The staged vector multiplication is checked.

The model retains historical identity-length and tree allowances/shared aliases. Sampled phase data is not a proven complete physical peak: unsampled constructor/finished-plan windows remain outside this telemetry. The new actual allocator observation separately measures current replacement-plan/scratch acquisition and complete receiving Drop; preexisting input HIR/attached owners are outside that observation. No offsets, credits, old/current physical equality, app-fit claim, or pin change.

The complete independently compiled legacy receiver is unchanged. Original hostile corpus, pin, semantic assertions, budgets and all original controls are untouched; exact source control seals full legacy and original tests prefix and removes only the literal cfg(test) held-index hook from the complete helper source. Complete original source archives and hashes are in source-provenance.json. Existing source reader already includes these Rust children; no new Rust child joins. Parent owns current artifact-hash refresh. Four additive owners plus ten retained owning selectors are in pending-selectors.json (14 total); all unexecuted.

Actual Claude author and independent reviews completed, tools disabled, on source-only packets. Author identified compatibility-reader and invalid-observation hazards; the final review identified a held-only phase can exceed the component sum and a clear-only retention test can be vacuous. Corrections use a smaller vector and explicit held-only control. Final review also suggested moving ordinary_index publication/check order, but this would change production semantics and is not implemented: seed_entry contains no capacity note, and the held snapshot plus later builder notes cover the sampled route. Full preexisting code/order stays exact. Broader legacy arithmetic/model completeness remains open, not claimed fixed.

Static verification: rustfmt leaf-only, diff check, unchanged full legacy, unchanged original test prefix, exact full helper inverse, and source registration census. Owning producer must later execute and regenerate any changed pin through its ordinary producer; never hand-edit the numeric expectation.
