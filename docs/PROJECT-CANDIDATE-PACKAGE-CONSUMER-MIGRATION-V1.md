# Project Candidate Package Consumer Migration v1

Status: **Partial; library implementation and regression sources authored and
unrun.** This is a read-only proposal artifact for one narrow Copy-scalar lane.

Audience: compiler contributors, package tooling authors, and reviewers.

`ProjectCandidate::package_consumer_migration` accepts an exact authenticated
baseline package corpus through `CandidatePackageSignatureConflictInput`, plus
caller-supplied candidate-era provider report and Resolver inputs. The candidate
must contain exactly one `change_function_signature` intention for the selected
provider. V1 admits only one through eight appended `i64`, `i32`, `char`, `u8`,
`usize`, `f32`, `f64`, or `bool` parameters with the ordinary closed literal
defaults already admitted by signature evolution.

The method first regenerates the existing baseline signature-conflict report.
It requires at least one stable-ID-bound affected call. It independently parses
every baseline package source and rejects noncanonical bytes. The compiler's
existing signature migration rewrites the exact provider and all bound calls,
preserving left-to-right argument staging and inserting only the authenticated
Copy defaults. V1 refuses provider-local calls or any call inventory not equal
to the package graph's exact affected cross-package sites.

The reconstructed provider source must byte-equal the final candidate source.
The method replaces only its package report with the explicitly supplied
candidate-era report, generates a new complete package source capsule, then
invokes Candidate Package Consumer Replay over the proposed corpus. A success
is emitted only when that replay authenticates the candidate provider and the
same complete call inventory. The report contains canonical proposed consumer
sources and before/after domain digests, the regenerated candidate capsule's
exact digest/byte count/source-set/link binding (the capsule bytes remain
regenerable rather than duplicated), the nested replay, exact revisions/change/
provider/target, counts and false authority flags. It is capped at 16,777,216
bytes and has schema
`semaprax.project-candidate-package-consumer-migration.v1`.

The output is a deterministic proposal, not a write or patch application.
`compatibility` remains `not_assessed`. It does not discover installed,
workspace, generated, deployed or runtime consumers; execute a tool, target or
model; acquire packages; publish source; or establish API, ABI, behavioral or
deployment compatibility. Generated-artifact provenance is explicitly
`not_supplied_or_inferred`; V1 does not guess that a corpus source was generated.
Such a source needs a later composition with exact generated-file provenance
and delta evidence before it can be called a generated-consumer migration.

`SPX-G509` rejects candidate/corpus/report binding failures. `SPX-G510` is the
closed refusal for unsupported changes, empty or mismatched affected-call
inventories, and reconstructions outside the narrow lane. `SPX-G511` owns final
report capacity. Existing package capsule, Resolver, graph, signature and stale
diagnostics remain authoritative.

The authored, unrun regression in
`tests/project_candidate/package_consumer_replay.rs` constructs a real two
package baseline and candidate-era evidence, requests an appended `i64`
default, asserts both stable-ID-bound callers become canonical `answer(0)`
calls, and checks the independently replayed candidate-era call count. It also
authors stale-candidate and ownership-changing refusal cases. No test target,
package build, runtime, target, or quality gate was executed.
