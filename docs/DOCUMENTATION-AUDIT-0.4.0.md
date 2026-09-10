# v0.4.0 full documentation audit

Status: completed directory-wide reconciliation of the accepted **HOSTED GREEN** release baseline.

Audience: maintainers, contributors, documentation readers and coding agents.

Audit date: 2026-09-10.

## Scope and authoritative subject

The audit accounts for **all 485 pre-existing tracked files under `docs/`**:
**394 Markdown pages and 91 other files**. The
[per-file inventory](audit/v0.4.0-inventory.tsv) records the disposition of every
file, including unchanged pages, compiler-embedded guidance, generated catalogs,
retained evidence, historical decisions and non-Markdown assets. These files
were inspected from the exact tracked-source snapshot
`3d34cff7aa984bbb0e1de02d3140d1368399b575`; all 3,271 repository blob identities
in that snapshot were checked against its tracked tree before the audit.

The implementation subject remains release `v0.4.0`, commit
`dfc15e2ddc818fa97744b5a9d69fd6108dd6a321`. The
[release baseline](RELEASE-0.4.0-STATUS.md) records the maintainer-accepted
**HOSTED GREEN** classification. The release-note publication problem is not
an outstanding implementation or conformance task. A documentation update does
not create a new compiler version or provide evidence for later code changes.

Concurrent documentation changes were reconciled rather than discarded. In
particular, current Status/Audience metadata and the already-pushed Agent,
generic, collection, library/I/O and roadmap corrections are retained or
refined within the same release scope. The audit does not change application
code, tests, executable examples, frozen protocol formats or target admission.

## Corrections closed

**Implementation versus authoring history.** Current references no longer use
obsolete writing-session statements such as unexecuted regression cases or
pending first hosted validation to describe implemented release behavior.
Headers and body paragraphs agree on the current baseline. Exact older run
IDs, measurements and local case counts retain their historical subjects.
Release acceptance is not substituted for an unexecuted comparative model
trial, a newly claimed physical device, or an explicitly unselected aggregate.

**Semantic workspaces and candidate tooling.** Candidate constructors,
recovery and archive stores, typed holes and drafts, schema/client discovery,
analysis and coverage reports, environment/consumer declarations, source
review, semantic deltas, rebase/merge, publication and session integration now
state their implemented scope and evidence consistently. A tested report that
says runtime or external-consumer evidence is not inspected still means exactly
that. Compiler regression evidence does not turn its output into a live-world
observation or an approval.

**Current language and runtime.** The algebraic-data RFC and programme ledger
now account for admitted owned Result propagation, nonidentity forwarding,
argument inference, nested reconstruction, multi-owner records, authored
generic variants, compiler collections, callable values, scalar captures and
iterator operations. Their additive specifications remain authoritative; older
wire/layout/cleanup profiles have not been widened by rewriting an overview.
Linked Agent migration and durable recovery are implemented additions, while
native/Wasm Agent-stage execution, general providers and wider public support
remain separate.

**Library and installed toolchain.** The library inventory agrees with the 34
bundled packages: nine core, sixteen portable, three alloc, three hosted, one
agent and two test packages. The complete required module set is not relabeled
as implemented. Installation and release guidance now reflect the shared
`doctor` dispatch in both CLI binaries while preserving fail-closed unavailable
production-profile acquisition. Private Rust packaging and full-host
publication boundaries remain distinct.

**Caches and future work.** Authenticated cross-process checked-HIR reuse is
already implemented by [Persistent Semantic Cache v1](PERSISTENT-SEMANTIC-CACHE-V1.md).
It is no longer described as entirely missing. General incremental checking,
broader cache compatibility, measured performance and complete session recovery
remain their own requirements. Current roadmap and completion descriptions
start from the released mechanisms instead of repeating obsolete first steps.

**Navigation and implementation ownership.** Repository links, chapter anchors,
source/test ownership references and metadata were checked across the whole
set. Stale test-harness paths were corrected to the actual projection, Wasm
nesting, native-package and collector owners. Body links to root-level Markdown were made explicit repository links rather
than nonexistent book chapters; the mdBook catalog retains local source paths
for its included chapters. The source-verified tour and installed guidance
retain their exact committed text and examples.

## Preservation and evidence rules

The audit preserves every pre-existing non-Markdown file, all retained evidence
under `docs/evidence/`, historical decision and execution records, the
changelog archive, and compiler-embedded guidance/catalog bytes. Every existing
fenced example remains byte-identical to the inspected snapshot. No test,
expected diagnostic, fixture, schema identity or known-answer artifact is
weakened to make documentation validation succeed.

Historical Phase 0 evidence v2 remains the reviewed 86-row aggregate at its
original subject. The separately specified v3 aggregate has no newly fabricated
execution bundle. The reserved external Zero comparison lane and unobserved
model trials remain unexecuted. The provisioned Linux doctor record retains its
actual earlier failed attempts and host requirements. These are scoped evidence
records, not an unexecuted backlog for the accepted v0.4.0 implementation.

Private profiles stay private, generated packages stay unpublished unless their
own publication decision changes, simulator coverage is not physical-device
coverage, and source/HIR admission is not runtime execution. Future checks for
changed code remain mandatory. The full product objective remains governed by
the [completion matrix](COMPLETION-MATRIX.md), not by the number of pages edited.

## Reproducible checks

Run the read-only consistency and preservation check from the repository:

```sh
python3 docs/audit/verify_v040.py --baseline 3d34cff7aa984bbb0e1de02d3140d1368399b575
```

The checker validates H1 titles, Status/Audience in the first twelve lines,
exactly one catalog entry per page, local repository targets, chapter anchors,
all sixteen source-cited language-tour excerpts, unchanged fenced examples,
and byte preservation of the protected baseline files. It is standard-library
Python and creates no compiler, runtime, network or publication authority.
External website availability and every possible host/runtime environment are
not inferred from repository link checks.

The publication check additionally executes the **four original source-only
documentation tests** from `tests/documentation.rs` without changing their
assertions, and builds the documentation with the repository-pinned **mdBook
0.5.4**. Those checks validate documentation; they are not a rerun of the full
compiler/runtime test suite. The validation record below identifies the actual
hosted audit execution rather than reusing a historical compiler CI run.

## Hosted validation record

[Audit run 34500651681](https://github.com/wavect/semaprax/actions/runs/34500651681) executed the checks against the final audited documentation prepared from `bb156e1602b88e2812762c9aea2c6c13b4d8b423`.

The directory check passed for **395 Markdown pages**, **2892 local links**, **95 chapter anchors**, and all **16 source-cited tour excerpts**. Metadata and catalog coverage passed; **108 protected files** and all pre-existing fenced examples remained unchanged.

The **four original source-only documentation tests passed**, and the complete **mdBook 0.5.4 build passed**. The final report was then included in a repeated check and book build. The full compiler/runtime suite was not rerun by this documentation-only audit; its accepted v0.4.0 HOSTED GREEN baseline remains separate.
