# Project Agent Workflow v1

Status: locally evidenced additive Project Agent Transport v4; exact-head
hosted promotion pending.

## Scope

`semapraxd --stdio --allow-project-workflow` binds one Project Manifest v1 at
startup and reports `semaprax.agent-transport.v4`. It preserves the v2 and v3
profiles and completes one bounded calculator maintenance workflow:

1. inspect the authenticated Project through snapshot, check, graph, context,
   and test;
2. derive one display rename for one explicit-ID scalar function already
   selected by `web_exports`;
3. preview the exact candidate with Project-bound structural Impact and a
   fixed-section Review;
4. apply through the existing single-file A0 authority and reload the exact
   candidate Project;
5. rebuild the refreshed Project as one deterministic inline Web carrier.

This is not a generic patch/change/build daemon. The request cannot select a
root, source path, source or patch bytes, evidence, output destination, tool,
environment, process, or native/Rust target.

## Profiles and compatibility

- Default `semaprax.agent-transport.v2` remains read-only and byte-preserved.
- `--allow-project-rename` retains the v3 method set and wire behavior.
- `--allow-project-workflow` selects v4. The two opt-in flags are mutually
  exclusive so one startup invocation has one unambiguous authority profile.

V4 retains the common v2/v3 read-only methods, but deliberately does not expose
the v3 `rename/preview` or `rename/apply` shortcuts. It replaces them with the
staged `rename/derive`, `change/preview`, `change/apply`, `impact`, and `review`
methods, and adds `build`. The separate v3 profile remains byte-preserved.
Every semantic method requires the exact current `project_revision` and
`workspace_revision`.

## State machine

```text
configured --workspace/open--> open
open --rename/derive--> derived
derived --change/preview--> prepared
prepared --change/apply--> applying --> refreshed open
```

`rename/derive` retains a plan only after its complete response fits.
`change/preview` advances only after the combined preview/Impact/Review response
fits. Wrong digests, notifications, malformed requests, and bounded ordinary
errors do not consume the retained plan. `build` is admitted only in `open`, so
it cannot run while a change is pending or after terminal uncertainty.

Held-input drift is absorbing. A0 commit followed by an inexact or failed
reload remains terminal `SPX-J110`; v4 performs no later build or recovery
action. Success and uncertainty responses are still bounded before A0 acquires
effect authority.

## Derivation, Impact, and Review

`semaprax.project-rename-derivation.v1` binds the authenticated base revisions,
stable ID, display-name transition, compiler-derived canonical Patch-v1 digest,
source path, and the completely validated candidate revisions, Project graph
digest, and preview digest. It carries no commit authority.

`semaprax.project-change-impact.v1` binds the exact derivation and preview,
base/candidate Project and Workspace revisions, base/candidate Project graph
digests, and both typed reverse structural closures under
`semaprax.project-semantic-impact.v1`. The nested Project Impact schema has its
own digest domain and never claims managed-Workspace provenance. For this one
display rename the report proves stable identity/export selection and unchanged
call-edge meaning while recording the source-projection change and rebuild
requirement.

`semaprax.project-change-review.v1` embeds that complete Impact and emits the
fixed `behavior`, `api_identity`, `security_authority`, `memory_ownership`,
`target_artifact`, `migration`, and `unsafe` sections. Its verdict is limited to
the admitted display-rename profile. It is not a general security,
compatibility, ownership, or target-execution audit and grants no approval or
commit authority.

## Inline Web build

`build { target: "web" }` renders exactly the same prepared seven-artifact
Project Web package as ordinary publication, but returns
`semaprax.project-web-build.v1` inline. Artifacts are in fixed order and carry
their path, decoded byte length, raw SHA-256, and lowercase hexadecimal bytes.
The carrier binds Project/Workspace revisions, entry module, cumulative decoded
bytes, the caller's non-widenable cap, and a payload digest.

`ProjectWebBuild::verify` and `verify_envelope` independently replay schema and
key admission, fixed inventory/order, canonical lowercase decoding, per-file
length and digest, cumulative bounds, payload digest, manifest binding, and
canonical bytes. The daemon writes no files, launches no process, and creates
no cache. A caller may materialize a verified carrier under its own authority.
This proves transport integrity and Project binding, not compiler provenance,
target execution, or external compatibility. Native and Rust builds remain
outside the daemon.

## Evidence and nonclaims

The focused local gate is:

```sh
cargo test --locked -p semaprax --all-features \
  --test project_agent_workflow_v1 -- --test-threads=1 --nocapture
```

It runs the real daemon through open, silent notification, derivation, stale
digest rejection, pending-state build rejection, candidate preview, standalone
Impact and Review, A0 apply, refreshed inline Web build, independent carrier
comparison, materialization, Node stable-ID calls, Project test, and shutdown.
Project semantic and inline-carrier unit gates separately cover deterministic
typed reverse closure, exact bounds/minus-one, every artifact digest, hostile
carrier mutations, and no managed-Workspace schema confusion.

General/multi-file change, import-alias/identity operations, request-selected
patches or outputs, native/Rust daemon builds, persistent indexing, network
service, concurrency, recovery, exactly-once delivery, target execution, and
hosted promotion remain open. Completion totals remain 56 Partial/0 Missing.
