# Cross-language runnable adapter v2

Status: implemented local-fixture execution extension for five additional
lanes (`c`, `python`, `swift`, `java`, `typescript`); external-language
admission (Zero, NTNT, Aver, Vera, Hale, MoonBit) remains unavailable.

Audience: benchmark operators and reviewers of offline adapter provenance.

This reference defines `benchmark.cross_language.runnable_adapter.v2`
(`benchmarks/cross-language-v1/runnable_adapter_v2.py`). It is a second,
explicitly versioned execution extension alongside
[Cross-language runnable adapter v1](CROSS-LANGUAGE-RUNNABLE-ADAPTER-V1.md),
not a replacement for it: v1's file, schema, and `rust` lane are byte-for-byte
unchanged, remain the sole executor for `rust`, and retain every unavailable
reason and hostile-input guarantee they already had. v2 generalizes the same
admission and containment model — reusing v1's audited primitives directly
rather than re-implementing them — to a data-driven list of bound host tools,
so that admitting a new language does not require a new bespoke Rust-shaped
admission function.

## Why v1's shape does not generalize directly

v1's descriptor hardcodes one toolchain's specific shape: a copied `rustc`
toolchain root, an external linker and link editor passed as `-C` flags, and
a macOS SDK receipt bound through `SDKROOT`/`DEVELOPER_DIR`. Rust genuinely
needs all of this. Empirically re-verifying the other five lanes this round
(direct, closed-environment `env -i LANG=C ... clang|swiftc|javac+java|
python3|node` invocations against a trivial positive and a trivial hostile
program, on this host) showed every one of them builds and runs a fixture
task with **no** linker flag, **no** SDK variables, and **no** environment
beyond `LANG`/`LC_ALL`/`TZ` — a strictly narrower requirement than Rust's.
Reusing v1's fixed-field shape for these lanes would therefore force either
carrying unused Rust-only fields into every new descriptor, or duplicating
v1's ~250 lines of file-acquisition, snapshot, and containment primitives
into a parallel implementation. v2 does neither: it defines a smaller,
uniform descriptor shape and imports v1's primitives (`_read_regular`,
`_write_snapshot_file`, `_admit_host_executable`, `_toolchain_digest`,
`_copy_tree`, `_safe_relative`, `_run_bounded_group`, `_kill_group`,
`SnapshotError`, `_unavailable`, `_sha256`, `_canonical_bytes`, the byte
bounds, and `CLOSED_ENVIRONMENT`) directly, by module reference, so there is
exactly one audited implementation of every security-sensitive primitive.

## Descriptor and admission

A descriptor is canonical UTF-8 JSON (`json.dumps(..., indent=2,
ensure_ascii=True)` plus one LF), at most 65,536 bytes, with exactly:

```json
{
  "schema": "benchmark.cross_language.runnable_adapter.v2",
  "baseline": { "...": "the unchanged baseline_admission v1 descriptor" },
  "execution": {
    "classification": "local_fixture",
    "adapter_id": "python",
    "task_id": "sequence-digest-v1",
    "adapter_inventory_sha256": "sha256:...",
    "receipt": "bounded local receipt text",
    "receipt_sha256": "sha256:...",
    "timeout_seconds": 30,
    "tools": [
      {"placeholder": "python3", "path": "/usr/bin/python3", "sha256": "sha256:..."}
    ],
    "copied_roots": []
  }
}
```

`tools` and `copied_roots` are always both present (the latter empty except
for `typescript`), so every lane shares one uniform descriptor shape rather
than a conditional one. The implementation passes `baseline` unchanged to
`agent.baseline_admission.admit_baseline_descriptor` exactly as v1 does; its
successful result is still `unavailable`, never execution evidence.

`classification` is closed to `local_fixture`, exactly as in v1: any
`external` claim is refused as `external_execution_requires_provisioned_review`.

`adapter_id` must be one of the five lanes v2 admits — `c`, `python`,
`swift`, `java`, `typescript` — via a fixed allowlist
(`V2_ADAPTERS`) the descriptor cannot extend or override. `rust`, `semaprax`,
`semaprax-project`, and the six reserved comparison languages are refused as
`adapter_not_admitted_under_runnable_adapter_v2`; this keeps `rust` exclusively
v1's lane rather than letting two admission paths both claim to execute it.

## Two tool trust shapes

Each lane declares one or more `tools` (single executables), keyed by a
placeholder token, plus (for `typescript` only) one `copied_roots` entry
(a directory tree). The placeholder identity — not caller-supplied metadata —
decides which of two fixed trust shapes applies, so a caller cannot relabel
an untrusted path as trusted through the descriptor:

- **SIP/root-owned executables** (`clang`, `python3`, `swiftc`, `javac`,
  `java`): each is macOS's own `/usr/bin/<tool>`, root-owned with no
  group/other write bit. The executor verifies path, ownership, mode, and
  SHA-256 (`v1._admit_host_executable`, unchanged) and then references the
  *original* absolute path — it is never copied. This is exactly v1's
  existing treatment of the Rust fixture's external linker and link editor,
  reused unchanged: a path only an administrator can alter needs no private
  copy to stay immutable for the run's duration.
- **Copied tools** (`node`, and the `typescript_lib` package tree `tsc`
  loads): each lives under the invoking user's own home directory, so it is
  read, verified against its declared digest, and materialized into a
  private snapshot directory before launch — exactly as v1 already does for
  the entire Rust toolchain root (`v1._toolchain_digest` with a copy
  destination, and a matching single-file read-verify-write for `node`).
  Replacing the original after admission cannot alter what actually runs.

If the shared baseline gate refuses, this extension preserves its named
unavailable reason rather than replacing it with a second error taxonomy,
exactly as v1 already does.

## Exact fixture execution

For an admitted fixture, the extension snapshots the descriptor, inventories,
selected public/hidden trees, equivalence input, every bound tool (and, for
`typescript`, the copied `typescript` package root), rewrites `adapters.json`'s
declared `version_command`/`build_command`/`run_command` to reference only
the bound, private paths (`_snapshot_adapter`/`_materialize_command` — the
same "rewrite one already-declared command, never accept caller-selected
argv" discipline v1's `_snapshot_adapter` already applies to `rustc`), and
launches the unchanged `run.py` under the identical hardened POSIX profile
v1 uses:

```text
<current-python> benchmarks/cross-language-v1/run.py
  --hardened-posix --execution-deadline-monotonic <deadline>
  --execution-output-bytes 65536
  --root <private-snapshot-root>
  --tasks benchmarks/cross-language-v1/tasks.json
  --adapters benchmarks/cross-language-v1/adapters.json
  --only <admitted-task-id>
  --language <admitted-adapter-id>
  --output <exclusive-temporary-result.json>
```

The subprocess environment is `LANG=C`, `LC_ALL=C`, `TZ=UTC` only — strictly
narrower than v1's, which additionally passes `SDKROOT`/`DEVELOPER_DIR` for
Rust's external linker. None of the five v2 lanes needs those variables (see
above); passing fewer keys than a lane could possibly use is not a claim that
a lane needing more would be safe under this same narrow profile. A successful
fixture is reported only as `fixture_ok`, exactly as in v1: never `ok` in a
cross-language comparison, and it cannot admit Zero, NTNT, Aver, Vera, Hale,
or MoonBit.

## Local provenance, not official upstream provenance

`clang`/`swiftc`/`javac`/`java`/`python3` are this host's locally installed
Xcode Command Line Tools and system Java; `node` and the `typescript` package
are this host's locally installed npm-distributed toolchain (TypeScript
5.8.3 via a local pnpm global install; Node.js 22.12.0 via `nvm`). Offline,
this repository cannot re-derive or verify an official upstream release
artifact digest for any of them the way, for example, a package registry's
published checksum would. Every lane here is therefore recorded honestly as
**local-host provenance** — the exact path, owner, mode, and byte digest
actually observed and bound on this machine — and is not described as an
authenticated official release, matching this suite's existing
`classification: local_fixture` framing (as opposed to `external`).

## Newly ported task

`sequence-digest-v1` was the pilot task ported to `c`, `python`, `swift`, and
`java` at introduction (`typescript` already had every task's ports before
this extension; that round only wired its *execution*, which no
runnable-adapter contract had done before). A follow-on round (issue #284)
ported the remaining eleven tasks in the canonical inventory to these same
four languages, and a later addition (`iterative-repair-workflow-v1`,
issue #298) shipped with its own `c`/`python`/`swift`/`java` ports from the
start, so every one of the 13 tasks now has a `c`/`python`/`swift`/`java`
port. Each port was independently authored against its own task's
`EQUIVALENCE.md` contract and the existing Rust/TypeScript reference ports —
same inputs, same functions/predicates, same hidden vectors — rather than
transliterated line-by-line, and each was confirmed, before being committed,
to (a) pass every public and hidden vector under its official toolchain
invocation and (b) fail under a deliberately mutated (wrong) candidate
specific to that task, so the hidden vectors are proven non-vacuous rather
than merely present. See each task's own `EQUIVALENCE.md` ("C, Python, Swift,
and Java ports" section) for its specific mutation and observed divergence,
and `sequence-digest-v1/EQUIVALENCE.md`'s "Independent review of the four
newer ports" section for the pilot round's review record. C, Python, and Java
keep the Rust/TypeScript candidate/entry split where a task has one (via
`#include`, `import`, and javac's same-directory auto-discovery
respectively, since this suite's C and Java adapters compile only the
declared entry file); Swift's fixed single-file `swiftc main.swift`
invocation admits no such split, so a Swift port with a candidate/entry
split repeats its implementation verbatim in both the public and hidden
`main.swift`.

Each new adapter row is marked `implemented: true` in `adapters.json` because
its toolchain is genuinely wired and provably executes a real canonical task
end to end. This was verified directly (`run.py --language c --language
python --language swift --language java`, no `--only`, against the full
12-task inventory): all 48 (task, language) pairs report `ok`; none report
`blocked`, `failed`, or `drifted`.

## Required future external evidence

Unchanged from v1: an external-language extension (for Zero, NTNT, Aver,
Vera, Hale, or MoonBit) still requires a maintainer-reviewed versioned
successor supplying independently reviewed port bytes and equivalence/oracle
digests, the official immutable source and revision already accepted by
baseline admission, an offline-provisioned toolchain artifact and
installation receipt whose bytes authenticate the declared digests, and a
runner mapping to that toolchain's documented bounded argv, retaining all
twelve task rows and all six currently blocked adapter denominators unless
their support decision is independently changed. Nothing in v2 relaxes this;
`c`, `python`, `swift`, `java`, and (now-executed) `typescript` are still
local fixtures of already-present host toolchains, not new evidence about any
of the six reserved lanes.
