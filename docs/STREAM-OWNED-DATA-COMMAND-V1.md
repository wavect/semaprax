# Stream Owned Data Command v1

Status: source implementation; focused current-head execution is pending.
This document defines Project v30 routing, not qualifying runtime evidence.

The table manifest explicitly selects
`profile = "language-command-io.owned-data.v1"` and semantic schema
`semaprax.project.v30`. Its command input is `argv-utf8+stdin-stream.v1`.
The exact sorted grants remain `process.args.read`, `process.stderr.write`,
`process.stdin.read`, and `process.stdout.write`. The authored entry and selected
command remain separate explicit stable-ID roots with exact `fn() -> i64` ABIs.
Neither exposes a collection or nominal carrier through a public boundary.

This selection inherits v29's authenticated private helpers, codec outcomes,
stdin epochs, native adapters, quotas and cleanup rules. It additionally admits
the independently checked owned-leaf collection runtime: String and flat
owned-leaf record vectors, deep cloning, replacement, reservation, ordering and
consuming iteration. Ordinary source/HIR derive the element shapes from exact
declarations and field types; names, generated IDs, schema-only declarations and
source provenance grant no runtime authority. The collection owner and each
transferred element retain ordinary affine cleanup and staged-call boundaries.
The intrinsic/storage contract is owned by the collection specification.

Workspace routing constructs the ordinary owned-data closure, then independently
validates v30's nominal and function signatures, command roots and operation
profile. Native emission repeats the selected-program validation. Pure entry
and test evaluation use the same authenticated private signature set and supply
no stdin or command provider. Prepared admission, retained revision interfaces,
lock facts and authority-free execution report replay bind the exact v30 schema.

V27 and v29 retain their frozen carrier and operation sets. Every verified module
function is scanned before reachability cropping, so an unused scalar helper
cannot hide new owned collection runtime operations under an older profile.
Logical schema declarations alone remain distinct from executable carriers.
No existing capacity, byte/work budget, capability inventory or public ABI is
increased. Selected Web/Wasm/npm command targets retain the inherited native-only
refusal before artifact publication; ordinary pure collection backend gates do
not establish a streaming host implementation on those targets.

The owning Project selectors are under
`project::tests::stdin_stream_command::owned_data`:

- `v30_owned_collections_execute_project_tests_prepared_and_native_stream_command`
- `v30_selection_does_not_widen_frozen_stream_profiles_or_root_abi`
- `frozen_stream_profile_rejects_owned_collection_uses_in_an_unused_scalar_helper`

They compose the ordinary owned-leaf corpus with imported helper identities,
pure/prepared test execution, streamed native output, repeated source-bound
graph reconstruction, hostile command ABI, and frozen-profile refusal. The
native stream fixture does not substitute for the collection's independent
interpreter/native/Wasm corpus or allocation/cleanup hostile gates.
