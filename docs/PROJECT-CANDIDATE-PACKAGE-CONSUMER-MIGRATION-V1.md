# Project Candidate Package Consumer Migration v1

Status: **Partial; library implementation and regressions execute locally in
the `project_candidate` harness.** This is a read-only proposal artifact for
two narrow lanes.

Audience: compiler contributors, package tooling authors, and reviewers.

`ProjectCandidate::package_consumer_migration` accepts an exact authenticated
baseline package corpus through `CandidatePackageSignatureConflictInput`, plus
caller-supplied candidate-era provider Report v2 and Resolver inputs. The
candidate must contain exactly one `change_function_signature` intention for
the selected provider.

The original lane remains one through eight appended `i64`, `i32`, `char`,
`u8`, `usize`, `f32`, `f64`, or `bool` parameters with the ordinary closed
literal defaults admitted by signature evolution. The additive owner-view lane
admits exactly one existing compiler-authenticated intention that replaces the
whole sole `own Bytes` parameter with one `borrow Slice<u8>` parameter. The
result must remain the same Copy value and effects, requires, and ensures must
all remain empty. Owning `string` to `borrow str`, multiple mappings, retained
owners, projections, temporary owner expressions, compatibility adapters,
owned results, and every other signature form remain refused.

The package-source evidence selector admits Copy parameters plus the exact
built-in pair needed to authenticate this boundary: `own Bytes` and
`borrow Slice<u8>`. Results remain by-value package scalars, which include the
`usize` a byte-length provider returns; owning `string`, `borrow str`, authored
nominal types, effects, capabilities, and type imports remain outside the
package-source profile. Existing Copy-only reports retain their interface bytes
and digests.

The compared base and candidate signatures are the exact retained checked HIR
of each Project revision. A provider consumed only across packages is outside
its own Project's entry and test closures, which retain linked call closures
rather than every checked declaration, so the route then relinks that
revision's exact retained canonical sources and reads the same validated HIR
the revision was admitted from. It never reconstructs meaning from source text
or from the submitted corpus.

The append lane reports and authenticates the consumer call inventory. A
provider-local call site is still rewritten, so the reconstructed provider
keeps byte-equalling the candidate source, but it is a provider-local fact and
is excluded from the affected and migrated counts. The owner-view lane refuses
provider-local call sites outright.

The method first regenerates the baseline signature-conflict report. It
requires at least one stable-ID-bound affected cross-package call and exact
provider, corpus, import, and call facts from the verified baseline capsule. It
independently parses every baseline package source and rejects noncanonical
bytes. For the owner-view lane, the retained base Project signature and
candidate signature must prove the exact sole parameter transition. Every
affected argument must be a bare variable root; the exact call inventory must
exclude provider-local and unauthenticated sites.

The compiler's existing revision-authenticated owner-view migration rewrites
the provider's sole checked `bytes_as_slice(owner)` use to the borrowed
parameter. At each consumer it stages original arguments left to right, then
derives `bytes_as_slice` from the staged owner and passes only the view. The
owner stays in the caller's staged scope through the call and ordinary checked
cleanup is rebuilt during package admission. No projection, copy, lifetime, or
second owner is synthesized.

The reconstructed provider source must byte-equal the final candidate source.
The method replaces only its package report with the explicitly supplied
candidate-era report, generates a new complete package source capsule, then
invokes Candidate Package Consumer Replay over all proposed sources. Success is
emitted only when replay authenticates the exact candidate provider and the
same complete cross-package call inventory.

The report contains canonical proposed consumer sources and before/after
domain digests, the regenerated candidate capsule's exact digest, byte count,
source-set digest and link binding, the nested replay, exact revisions, change,
provider, target, lane, counts and false authority flags. Capsule bytes remain
regenerable rather than duplicated. The report is capped at 16,777,216 bytes
and has schema
`semaprax.project-candidate-package-consumer-migration.v1`.

The output is a deterministic proposal, not a write or patch application.
`compatibility` remains `not_assessed`. It does not discover installed,
workspace, generated, deployed or runtime consumers; execute a tool, target or
model; acquire packages; publish source; or establish API, ABI, behavioral or
deployment compatibility. Generated-artifact provenance is explicitly
`not_supplied_or_inferred`; the route does not guess that a corpus source was
generated.

`SPX-G509` rejects candidate, corpus, report, and replay binding failures.
`SPX-G510` is the closed refusal for unsupported changes, temporary or
projected owners, empty or mismatched affected-call inventories, and
reconstructions outside the two lanes. `SPX-G511` owns final report capacity.
Existing package capsule, Resolver, graph, signature, ownership, cleanup, and
stale diagnostics remain authoritative.

The regressions in `tests/project_candidate/package_consumer_replay.rs` retain
the Copy append case and add the whole-Bytes case. They construct real
two-package baseline and candidate-era evidence, assert the canonical caller
stages the owner before deriving its view, require independent candidate-era
package replay, and reject a temporary `bytes_copy(...)` owner argument. They
now execute in the ordinary `project_candidate` harness; no runtime, target, or
publication gate is executed by them.
