# Application JSON codec plan v1

Audience: coding agents, compiler contributors and reviewers.

Status: source implementation; focused execution and repository gates pending.

`semaprax json-codec <project> --plan <plan-json> --output <new-bundle>`
derives a bounded set of existing closed codec profiles against one immutable
held-input Project revision. It returns one reviewable complete checked candidate;
it does not install generated modules or publish a managed generation.
The existing single-codec command and exclusive no-overwrite publication remain
unchanged. No measured turn, token, context or wall-time gain is established.

## Closed plan and fixed bounds

The UTF-8 JSON plan is at most 16384 bytes. Duplicate keys, unknown fields,
trailing JSON, unknown profiles and invalid profile/bound combinations refuse.
The schema is `semaprax.json-codec-plan.v1`; `entries` contains 1..8 objects:

```json
{"schema":"semaprax.json-codec-plan.v1","entries":[
  {"source":"src/request.spx","record":"shiftsim.request","profile":"stream-owned-request.v1"},
  {"source":"src/response.spx","record":"shiftsim.report","profile":"bounded-collection-response.v1","max_string_bytes":16}
]}
```

Optional `expected_project_revision` must equal the exact authenticated authored
Project revision; it makes a stale queued plan refuse. Without it, derivation
binds the revision authenticated by the live invocation. Both modes retain the
ordinary final held-input recheck before returning an artifact to publication.
`source` must be an exact declared Project source path and `record` an explicitly
identified record in that source. Each source path and selected record identity
is unique. Two entries replacing the same module overlap and refuse, including
entries selecting different records or profiles in that module.

The additive `bounded-nested-response.v2` selector requires both bound fields
and binds its distinct generated identities in the existing entry receipt.
Its source contract is [nested response v2](APPLICATION-JSON-NESTED-RESPONSE-V2.md);
the plan envelope, replay and publication authority remain unchanged.

`profile` also accepts the four explicitly selected decode-only request v1
profiles in [the codec contract](APPLICATION-JSON-CODECS-V1.md#explicit-decode-only-owned-requests-v1-897).
Their bounds, complete ordinary candidate validation, independent replay and
exact profile receipt remain required. A full-profile bundle cannot be replayed
as a decode-only bundle or vice versa. No existing plan is silently changed.

`profile` uses the existing closed selector spellings; `flat-scalars.v1` explicitly
selects the original default. `max_string_bytes` and `max_array_items` are JSON
integers present only when required by the selected profile. All existing schema,
capability, profile, source and runtime bounds remain authoritative. This route
has at most 64 authored Project sources, at most 2 MiB total authored source and
2 MiB total combined candidate source, and at most 8 MiB compact serialized
bundle bytes. Each derivation retains the existing 65536-byte schema and
131072-byte generated fragment bounds. These are artifact/compiler bounds,
not a change to accepted application input, output or execution budgets.

## Candidate and exact evidence

Every entry reads the same immutable authored revision. No generated output
becomes another entry's input. Entries are canonicalized by source path, so
reordering a valid plan preserves exact bundle bytes. All replacements are
linked together through ordinary complete Project admission, independent HIR
validation, semantic graph construction and unchanged manifest profile and
dependency inventory. Cross-entry or authored/generated declaration collisions,
invalid members and unsupported combined candidates fail before output.

The deterministic compact JSON bundle has schema `semaprax.json-codec-bundle.v1`:

- `authored_project_revision`, `candidate_project_revision` and
  `candidate_workspace_revision` bind the checked revisions.
- `manifest` is the original canonical Project manifest.
- `authored_sources` preserves every exact original authored source byte string,
  source path and SHA-256 separately from the generated candidate inventory.
- `candidate_sources` contains every complete candidate source, sorted by path,
  with its exact SHA-256. Unchanged files remain part of the complete candidate.
- `entries` records source path, record identity, exact profile and optional
  bounds, authored/replacement SHA-256 and sorted newly generated top-level
  declaration identities. Ordinary resolution still validates all nested IDs.

The public Rust API is `project::derive_json_codec_plan(&ProjectRevision,
&[JsonCodecPlanEntry]) -> Result<JsonCodecBundle, Vec<Diagnostic>>` and
`project::verify_json_codec_plan(&ProjectRevision, &[JsonCodecPlanEntry],
&JsonCodecBundle) -> Result<(), Vec<Diagnostic>>`. Replay rederives and compares
all fields and complete bytes. A changed selector, bound, member, source,
receipt or generated inventory invalidates the claim. Evidence grants no
filesystem, compilation exemption or mutation authority.

The internal application starter additionally uses a private unverified draft:
it independently regenerates complete source/receipt bytes, compares all draft
fields, drops the regeneration, then performs one complete ordinary candidate
build and both HIR validations. Candidate Project/workspace revisions are derived
only by that ordinary build. Public derivation and public replay retain their
original checked-candidate boundaries, and the starter's distinct activated and
formatted candidate is still rebuilt and validated afterward. This removes one
repeated pre-activation candidate build on successful starter preparation; it
establishes no measured net allocation, latency, application fit or runtime gain.

The raw CLI encodes the complete checked bundle before creating its private
output file and uses the existing exclusive hard-link publication for one
explicit new destination. Existing files, symlinks and racing destination
creators refuse. Failed members produce no bundle, and failed publication
removes the private temporary. Original files are never overwritten. One
artifact publication does not give atomic visibility to Git, editors or raw
source readers. Installation remains an ordinary reviewed source operation;
any managed integration must acquire the existing lock, replay before staging,
validate one complete immutable generation and pivot `ACTIVE` under the
[Semantic Workspace Transaction v1](SEMANTIC-WORKSPACE-TRANSACTION-V1.md)
authority. This version exposes no managed apply command.

## Owning regressions and adoption

`project standard_library::application_json::plan::` owns paired success,
reordered-plan determinism, exact source/graph revision round trips, mixed-invalid
and duplicate/overlap/bounds/receipt/source drift controls, no-overwrite and
no-partial-output CLI cases. `cli::json_codec::plan::tests::` owns closed bounded
JSON decoding. The existing ShiftSim owning application test now derives and
replays both codecs as one combined candidate before installation and retains
its original full acceptance oracle. The committed `codec-plan.json` in
`examples/shiftsim-typed-record-successor` provides the real one-command setup.
The focused project and CLI owners passed, including complete bundle
publication and the original ShiftSim and Catalog application corpora.

Two local comparisons used the same compiler, the same immutable ShiftSim
bootstrap and the same two selectors. Each paired invocation produced both
generated modules in one reviewable bundle. Two individual invocations
produced byte-identical modules. Both orders preserved every authored file
and produced the same paired bundle. Paired CLI elapsed times were 22.8 and
25.8 seconds, while the corresponding sums for the two individual commands
were 18.0 and 18.1 seconds. These local timings include process startup and
output serialization.

### Private starter replay qualification

The paired flat-scalar fixture runs the complete saved and current plan and
starter drivers. It compares the serialized bundle, every final file and both
revisions, with and without command activation. The pre-activation candidate
build, entry HIR validation and test HIR validation each ran twice in the saved
starter and once in the current starter. Both drivers performed the final
formatted Project build. The public derive-and-verify pair still performed two
independent candidate checks. The refusal controls covered every draft field,
source inventory, reminted hashes, wrong revisions, first diagnostic order
and preservation of the authored files.

The qualification passed eight library checks, six Project integration
checks, five full-host CLI checks, seven held-input and staging checks,
sixteen publication platform checks and scoped production Clippy.

The local macOS arm64 observation measured the complete starter through Drop
on the same two-entry Project. It warmed the immutable renderer tables before
opening the observer. The observer tracks requested bytes and allocates
replacement backing before releasing the old allocation during reallocation.
These measurements are distinct from the Project builder budget.

| Whole starter | Saved | Current |
| --- | ---: | ---: |
| Allocation events including replacements | 3,963,819 | 2,723,696 |
| Reallocations within those events | 543,039 | 365,334 |
| Cumulative requested bytes | 569,311,549 | 384,843,171 |
| Peak live bytes | 8,054,507 | 8,054,224 |
| Live bytes after Drop | 0 | 0 |

On this build the unverified draft and its Result are 120 bytes each. The
checked bundle and its Result are 192 bytes each. A candidate source header
is 48 bytes. Path, source and receipt payloads have separate heap backing.
The draft observation on the same fixture measured 149,940 live bytes after
one draft, 299,880 with both complete drafts, and 149,940 after dropping the
regenerated draft. Deriving and comparing the two drafts requested 12,480,180
bytes cumulatively, peaked at 455,132 live bytes and released all observed
backing. The phase recorded zero candidate builds and zero entry or test HIR
validations. Production replay releases the regenerated draft before candidate
admission.
