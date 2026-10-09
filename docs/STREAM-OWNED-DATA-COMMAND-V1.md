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
The intrinsic/storage contract is owned by
[Owned Leaf Collections v1](OWNED-LEAF-COLLECTIONS-V1.md).

For source syntax, use `help language author:owned-data`. Spell String as
lowercase `string`; by-value String parameters such as `text: string` already
consume their argument, and `text: own string` is refused. User-declared record
and field declarations each need an explicit stable `@id`. `Vec<string>` is an
owning carrier: pushing a String consumes it, while `vec_into_iter` transfers
the elements to `for own`.

Bytes-bearing construction or deep copy in loop bodies/conditions remains
refused with `SPX-T267`, including `vec_clone_at` on a Bytes-bearing record and
transitive helper calls. Stage allocations and deep copies outside loops;
admitted String-bearing loop operations retain their separate checked rules.

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

The next source tranche admits an ordinary explicitly identified, monomorphic
two-case private outcome. One case has one or two direct `Vec<T>` fields whose
elements independently satisfy the owned-leaf collection contract; the other
has exactly `i64`, `usize`, `i64` fields in that order. Case and field identities
are explicit, and neither case names nor generated origins grant authority.
The outcome is affine; its Vec fields remain whole canonical cleanup leaves.
Source and HIR reconstruct this shape separately, and the original Copy-only
collection outcome is unchanged. V29 and older native profiles refuse even a
header-only or unused function carrying this new outcome. This source tranche
does not admit arbitrary recursive variants, nested Vec values or an owned
record inside a variant. Its owning gates and backend execution remain pending.

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
