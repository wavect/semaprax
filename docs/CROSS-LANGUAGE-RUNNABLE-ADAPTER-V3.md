# Cross-language runnable adapter v3: official TypeScript lane

Status: implemented locally; independent implementation review pending, not full R03 acceptance.
Audience: benchmark adapter implementers, independent reviewers and maintainers.

## 1. Scope and frozen behavior

`benchmark.cross_language.runnable_adapter.v3` admits only the TypeScript
adapter through official Node.js 22.12.0 Darwin arm64 and TypeScript 5.8.3.
It reuses the v1/v2 bounded acquisition, snapshots and existing `run.py`
public/hidden scorer. It does not change either earlier descriptor, their
`local_fixture` classifications, or unavailable-only baseline admission.

This is an official-toolchain conformance lane, not an Agent/model result,
language superiority result, signed release or completion of R03. Other
actually available language lanes still need their own official provenance.
All 182 task/adapter denominator rows remain (13 tasks, 14 adapters), with
explicit unavailable reasons for every unsupported lane. No caller may
provide a subset as the complete comparison inventory.

## 2. Independent approved provenance

Expected identities are reviewed implementation-owned constants, never
descriptor-supplied expected hashes. Authorized artifact paths are inputs;
matching caller-created receipts do not authorize a different subject.

| Fact | Approved value |
| --- | --- |
| Node checksum origin | `https://nodejs.org/dist/v22.12.0/SHASUMS256.txt` |
| Checksum receipt SHA-256 | `4d4dc7ec5755c7b034127c9f13ba83efe9869d2aa11c173d901a551da0f4649f` |
| Node artifact | `node-v22.12.0-darwin-arm64.tar.xz` |
| Node artifact SHA-256 | `0047be0cfda922eb73876f9ef41de361c36b7654c884d13d9b783b0efd1db9aa` |
| TypeScript metadata origin | `https://registry.npmjs.org/typescript/5.8.3` |
| Metadata receipt SHA-256 | `903e88bb6d16ca9419e14722378df6b66de31187f3534fb90a18e9ee2b37582e` |
| TypeScript artifact | `https://registry.npmjs.org/typescript/-/typescript-5.8.3.tgz` |
| TypeScript artifact integrity | `sha512-p1diW6TqL9L07nNxvRMM7hMMw4c5XOo/1ibL4aAIGmSAt9slTE1Xgw5KWuof2uTOvCg9BY7ZRi+GaF+7sfgPeQ==` |

The coordinator assistant retrieved metadata over certificate-verified HTTPS
under the user-authorized task-local provisioning scope. An independent
reviewer verified and approved expected-content facts before archive acquisition.
Retained original metadata bytes plus the approved pins are the origin trust
basis. No release-signature or npm-signature verification is claimed.
The TypeScript receipt must also have exact name `typescript`, version
`5.8.3`, tarball URL, integrity, fileCount 130 and unpackedSize 22867703.
The acquired TypeScript archive additionally observed SHA-256
`72e75dbeb92c2e6eb9a34cb59d74fab5c2ee6f32a0324a89405f6165d5a08374`;
the SHA-512 pin remains its authoritative archive admission check.

No runtime network fetch, package manager, installation script, global
installation or search-path fallback is admitted. Provisioning is separate
from execution and confined to the task-authorized scratch directory.

## 3. Bounded private extraction and source identity

Acquire original archive/receipt files through no-follow regular-file reads
under explicitly authorized held parent directories. Reject acquisition
through substituted ancestors, leaf links, nonregular files, concurrent
changes and bound violations. Authenticate archive bytes before extraction.

Manually scan archives; never use unrestricted archive extraction. Reject
duplicate names, absolute/traversing paths, unsupported entry types, excessive
member counts, oversized members and excessive expanded bytes. Validate names
before materializing anything. Node's only execution payload is the one
regular member `node-v22.12.0-darwin-arm64/bin/node`; unrelated archive
members, including npm links, are not extracted or executed. TypeScript
admits its `package/` regular-file tree and directory entries, with no links
or special files. Reconstruct new private paths with exclusive creation.

Bounds: Node archive 64 MiB, TypeScript archive 16 MiB, Node scan 10000
members, selected Node binary 128 MiB; TypeScript scan 512 members, each
regular file 16 MiB and total expanded regular bytes 32 MiB. Additionally cap
all decoded tar stream bytes, including skipped member payloads, headers,
extended metadata, padding and trailers: Node 512 MiB and TypeScript 40 MiB.
Enforce this on the decompressor output while scanning, without buffering
skipped payloads; stop at the cap even if an ignored member would be larger.
Member names are at most 4096 UTF-8 bytes and extended metadata per entry at
most 1 MiB, within the same total cap. The approved
TypeScript archive has 130 regular files totaling 22867703 bytes. The observed
selected Node binary has 121482128 bytes and SHA-256
`53dc65febda99ecaafe692de5ec60efdc2f7bd4fb14d1ba8cd30dc2af103953f`.

Materialize Node executable mode 0500, package files 0400 and private runtime
directories 0500 after construction. Bind a complete declaration-ordered
runtime inventory of relative names, sizes, modes and content hashes; recheck
before execution. Read-only modes prevent accidental mutation, not a hostile
same-user administrator; private authority and authenticated rechecks remain
required. Never run provisioned npm or any archive installation hook.

The independently approved expected-source anchor must be the complete
`benchmark.cross_language.official_ts_source_subject.v1` manifest, not caller
hashes or a digest computed from an arbitrary current checkout. The proposed
review subject is exact Git commit
`8e2a1c58324fb17308084e3cef149494259bd585`; its 298-file manifest is 120925 bytes,
SHA-256 `c69695fd5f16917d745ef4c47c1d54438455d541cc4801046dd764e50a0fb83c`,
retained at
`benchmarks/cross-language-v1/provenance/typescript-official-v3-source.json`
and as `source-subject.json` beside the probe receipts. Its origin commit
identifies benchmark input provenance; the actual integrated execution commit
is recorded separately and may contain unrelated later changes. It was derived
from that commit's Git objects, and all selected current files match.
Independent approval of this expected-source manifest is required before code.
The implementation must own the approved manifest bytes/digest; callers may
supply locations but not substitute expected identities. A future corpus
subject requires an independently reviewed successor pin, never silent
re-pinning during execution.

The manifest pins all 182 comparison rows, including declared/implemented
flags and exact blocked reasons. It selects tasks.json, adapters.json, run.py,
v1/v2 adapter helpers, baseline_admission.py and every declared public/hidden
regular file for every language in all 13 tasks, plus every task equivalence
document. File entries are sorted by repository-relative path; comparison
rows follow task declaration order then adapter declaration order. Canonical
manifest JSON uses recursively sorted keys, indent 2, ASCII escaping and one
final LF; its acquisition cap is 128 KiB. Test vectors/oracles
are in these public/hidden files; retained independent review/mutation records
are the task EQUIVALENCE.md sections, not invented separate attestations.
Bind these exact owner task/adapter inventory bytes, scorer bytes, selected
public and hidden trees, oracle/equivalence and independently reviewed records. The actual reviewed documents, not synthetic test
`review:<task>` digests, supply provenance. Preserve task splits and public
candidate/hidden-overlay boundaries. The public and hidden phase directories
are disjoint siblings, never nested or reused. Construct the public phase from
public source only; it never contains the hidden overlay. Construct the hidden
phase separately from unchanged public source plus the authenticated overlay
only after public scoring/leak checks. Each sandbox grants only its current
phase subtree, never their common parent or the other phase. Compiler outputs
stay in that phase. Mutations touch private copies only.

## 4. Platform and runtime dependencies

The initial execution profile is Darwin arm64, observed macOS 26.5.1 build
25F80. Record and require this reviewed host version/build for this first
profile; another host profile requires review and fresh authority probes.
The OS, its protected loader/libraries and sandbox are explicit host trust
assumptions; the archive pin does not authenticate Apple's system runtime.

Observed `/usr/bin/otool -L` imports are exactly:

- `/System/Library/Frameworks/CoreFoundation.framework/Versions/A/CoreFoundation`
- `/usr/lib/libSystem.B.dylib`
- `/usr/lib/libc++.1.dylib`

`otool -l` found no `LC_RPATH`. Admission must check the authenticated binary's
architecture and complete load commands, refusing added, relative, local,
`@rpath`/`@loader_path` dependencies or executable search fallback. Runtime
reads under `/System/Library` and `/usr/lib` cover transitive protected loader
data/shared caches as an explicit OS authority, not arbitrary user files.

## 5. Execution authority and exact commands

The trusted Python harness may read only the authorized repository inputs,
approved provenance files and private runtime; create private scratch; start
the authenticated Node through the admitted sandbox; and deliver evidence to
one explicit caller-authorized output location. It receives no model/provider,
signing, publication or global-install authority. It does not inherit PATH,
home, startup, SDK or dynamic-loader injection environment values.

Every Node launch, including version probes, compilation, scoring and hostile
controls, is wrapped by root-owned `/usr/bin/sandbox-exec`, authenticated with
the existing host-executable acquisition policy and recorded in the host
receipt. No unavailable sandbox or failed authority preflight may fall back
to an unconfined Node execution.

The generated SBPL profile starts `(version 1)(deny default)` and allows only:

- `process-exec` for the authenticated private Node literal. No `process-fork`
  allowance: even spawning the same Node must fail.
- `file-read*` for the Node literal, read-only TypeScript runtime subtree,
  current phase subtree, `/usr/lib`, `/System/Library`, and literal `/`.
  The literal `/` permits root-directory data/metadata, not its descendants.
- `file-read-metadata` for exact ancestor directory literals needed for
  Node's realpath traversal; no subtree or global metadata allowance.
- `file-write*` within the current phase scratch subtree only.
- `file-read*` and `file-write*` for literal `/dev/null` only.
- `sysctl-read` for exactly `kern.ostype`, `kern.osrelease`, `kern.version`,
  `kern.hostname`, `hw.machine`, supplying Node's OS-information operation.

There is no general `file-read*`, process, mach-lookup, system-info, sysctl,
network, home, unrelated-source, or external-output allowance. Denied optional
startup operations remain denied. Paths must be escaped as literals, derived
from authenticated private authority; a caller cannot inject profile syntax.

Commands are arrays with no shell. Version probes are private Node
`--version` and private Node plus `typescript/bin/tsc --version`. Compilation
uses that Node and compiler entry with the existing adapter's exact
`--strict --target ES2020 --module commonjs index.ts`; execution uses the same
Node with `index.js`. The closed environment is exactly LANG=C, LC_ALL=C,
TZ=UTC. Cwd is the isolated phase directory. Reuse the existing scorer's
public/hidden success and leak checks rather than introducing another oracle.

Reuse descriptor 64 KiB, process output 64 KiB, source file 1 MiB, source
total 8 MiB, toolchain 512 MiB, result 256 KiB and per-task deadline 120
seconds. Deadline covers version/build/public/hidden phases together; kill
the bounded process group on timeout/output violation. No shell/process
search is authorized. Before work, reserve bounded scratch/evidence capacity;
source/task bounds do not authorize unbounded archive expansion.

## 6. Evidence and executable acceptance

The v3 result distinguishes official-toolchain conformance from local fixtures
and unavailable observations. Deliver original provenance receipt bytes,
archive and runtime inventories, exact source/oracle/review identities,
effective cwd/env/argv, host/sandbox identity, stdout/stderr/status and emitted
JavaScript artifacts for each phase. Preserve failed observations. Limit the
delivered immutable bundle to 8 MiB and result metadata to 256 KiB; refuse
oversized delivery, never truncate into success. Scratch builds are removed
after delivery; original provisioned runtime is separately reusable only
under unchanged authenticated authority. No giant build cache is retained.

Implementation gates must execute all 13 TypeScript positives and a documented
task-specific wrong candidate for each task. Prove the selected mutation target
exists and the required public/hidden divergence from its equivalence review;
a missing target, compile-only failure or vacuous assertion is not success.
Bind all observations to the exact checkout and input snapshots. Preserve all
182 denominator rows and unsupported reasons.

Hostile gates cover artifact/receipt/runtime/source/review substitution,
unsafe archive entries, excessive expansion, dependency drift, environment
injection, timeout/output bounds, malformed scoring results, hidden leakage,
unavailable confinement and absent tools. Real authority gates must prove
unrelated file read/write, `/usr/bin/true` spawn, same-Node spawn and reachable
loopback connection fail. The trusted host proves the listener reachable;
ECONNREFUSED against an absent listener is not a network-denial observation.

Owning selector:

```sh
SPX_R03_V3_PROVENANCE=/absolute/authorized/provenance-directory \
  python3 -m unittest discover -s benchmarks/cross-language-v1 \
  -p test_runnable_adapter_v3.py -v
```

It requires the actual approved artifacts and the admitted host; missing tools
fail rather than silently skip. The implemented route is `OfficialSession`
with fixed corpus task IDs and the implementation-owned mutant inventory,
never caller-provided source or caller-expected pins. Its CLI is:

```sh
python3 benchmarks/cross-language-v1/runnable_adapter_v3.py \
  --provenance-directory /absolute/authorized/provenance-directory \
  --output /absolute/authorized/existing-parent/new-evidence.json
```

The CLI scores all 13 positives. Tests additionally score all 13 fixed mutants.
Bundle JSON has `result`, `source_manifest`, `artifacts`; artifacts carry exact
bytes in Base64 with sizes/hashes. Command metadata references exact policy and
stdout/stderr artifacts instead of duplicating their bytes. Canonical JSON
uses sorted keys, indent 2, ASCII escaping and one LF. Exclusive delivery never
overwrites evidence. The exact execution head, worktree dirty flag and v3
implementation hashes are recorded separately from the immutable corpus origin.

A local implementation-source run passed **45/45** tests: 13 positives, 13
runtime mutants and 19 hostile/provenance/scoring controls. It retained 26
scored rows and the complete 182-row denominator inventory. The earlier failed
runs and their evidence remain in task-private scratch. This is a dirty-tree
implementation witness; integration must obtain fresh evidence for the clean
accepted execution commit. Independent implementation review remains required.
The existing runnable-adapter and cross-language documentation harnesses must
also remain green. No new top-level Rust harness is needed.

## 7. Observed design probes; not acceptance

Task-private probes on the observed host authenticated the two archives and
selected 131 regular runtime files. A smoke TypeScript source compiled and its
generated JavaScript ran under the narrow profile. It was a host-authored
constant fixture, not arbitrary external source or the 13-task acceptance run.

Failure probes retained root-directory denial/abort, missing `/dev/null`
tracing-loop abort, missing ancestor metadata EPERM and OS-information aborts.
Omission probes showed all five listed sysctls necessary for Node's
OS-information call; `kern.osversion` was unnecessary and remains denied.
The final same-profile controls observed EPERM for forbidden canary reads and
writes, both child-process launches and reachable-loopback connection.

Retained scratch receipts under
`/Users/kevin/Documents/ChatGPT/v070-locks/r03-provenance/r15-runtime/`:

| File | SHA-256 |
| --- | --- |
| `source-subject.json` | `c69695fd5f16917d745ef4c47c1d54438455d541cc4801046dd764e50a0fb83c` |
| `runtime-host-facts.json` | `c5f1d1bcd2015f53af9ceeed7ffd17fed7c44a4a72eb07438baef30dd71c4315` |
| `reachable-listener-probe.json` | `11869bb1931fd2196a6170ec2f33079bac47deddfbb55ffd9e87e888516d2295` |
| `runtime-inventory.json` | `89de79738e48e08cf093832ff8dedc0d51ec457a37ac622c445573674b4c11ad` |
| `accepted-authority-probes.json` | `20e865a1ec348c69adcd2dd14f278fde6664134b6cbaa9478d484d5dc4182c01` |
| `accepted-authority.sb` | `078d6f4d86fb862b4911f1f465dd749eef50ec6b376222f5fcb2ffb09a676126` |
| `sysctl-probes.json` | `d22b6bf070164d0a09f474635f2872bbfa53dd4077551298eec73ac951c4e096` |

These are design witnesses, not fresh canonical scoring, hosted verification,
complete R03 evidence or a signed/public release. Independent design/source-pin review approved this contract before the
implementation lease. These initial probes still do not replace the owning
implementation selector or integrated acceptance evidence.
