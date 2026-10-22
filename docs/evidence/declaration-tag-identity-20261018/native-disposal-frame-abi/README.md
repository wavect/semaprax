# Native resolved-disposal frame layout witness

Status: historical source evidence; qualification is limited to the recorded revisions and results.

Audience: compiler contributors and reviewers.

Source-only correction after the c273 focused build reached the native SDK builder and refused its historical 56-byte frame assertion. No Cargo, compiler, or runtime gate was executed for this change.

The complete thirteen-variant disposal frame and all teardown/capacity code remain unchanged. A layout-only copy retains the genuine 530073 DeclarationId, complete ResolvedType and recursive cleanup shape definitions. Its full historical frame must still equal 56 bytes. A separate complete frame witness uses the actual current payload types and must equal the shipping frame in size and alignment while remaining inside the historical envelope. No new current numeric size is asserted, no padding is introduced, and no fee is credited. Box and Vec alternatives witness their headers, without claiming the layout of old pointed-to HIR values.

The existing preallocated/depth-bounded disposal owner retains the historical 56-byte control and checks actual current equality/envelope after all its original disposal/completion/capacity assertions. Complete old test and producer sources and exact before/after records are retained in this directory. The original capacity and phase-A size-of calculations are byte-exact.

The bounded scan of fixed-size assertions in this crate found no second active by-value DeclarationId-dependent frame: SelectedClosureFrame owns only String/Vec<String>/usize, and both old local C frame assertions are under cfg(any()) with borrowed HIR payloads. This is a source audit, not compiler qualification or a CPU-performance claim.
