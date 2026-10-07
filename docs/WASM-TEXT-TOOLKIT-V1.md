# Wasm Text Toolkit v1

This additive profile owns checked Text Toolkit v1 lowering, borrowed-to-owned
String conversion, numeric conversions, bytewise String comparison, and Copy
or owned String variant matching. Existing internal String v1 and Copy Variant
String Settlement v1 selectors, imports, descriptors, and host bytes remain
frozen. The additive Toolkit selector also admits the closed String Collections
profiles through checked collection imports and a bounded generated host arena.

## Entry points and authority

`wasm::internal_strings::emit_text_toolkit_module` selects 1–32 explicitly
identified scalar exports. Public parameters/results remain `i64` or `bool`;
internal checked calls may transport the admitted Copy scalars, String, borrowed
`str`, Copy variants and records, and independently admitted owned String
variants, typed Map/Set and legacy Map values, and bounded owned text records.
Own/Borrow parameters and aggregate result pointers use the
same validated internal ABI as aggregate Wasm. Source verification and independent
HIR/cleanup/layout replay precede admission and emission.

`build_toolkit_web_from_source` and single-source
`build --target web --profile text-toolkit-v1 --export <id>` publish the fixed
fresh-directory inventory after rechecking the bounded source snapshot. The
compiler descriptor is `semaprax.wasm-text-toolkit.v1`, the trusted runtime is
`semaprax.wasm-text-toolkit.runtime.v1`, and the package manifest is
`semaprax.web-text-toolkit.v1`. Generated TypeScript includes normalized Text,
conversion, collection, and filesystem outcomes. The browser UI authenticates the descriptor
before constructing controls, and the runtime authenticates the exact module
bytes and closed import/export inventory before instantiation.

`instantiate(bytes, {fileReadText: {read(pathBytes, maximum)}})` supplies optional
read-only authority. The callback receives a copied relative path and a 65536
byte maximum; it returns `{ok:true, bytes:Uint8Array}` or
`{ok:false, code:1..7}`. It must be synchronous. No ambient filesystem, browser
storage, working directory, process, or network operation supplies this authority.
An absent provider selects filesystem `AUTHORITY_DENIED` before validating the
path. Source `fs.read` effect/permit checks still apply. An invalid provider
shape/status poisons the instance rather than acquiring a semantic failure code.

## Checked transport

The standalone ten-import arena prefix is unchanged. The additive profile
appends numeric text constructors and a comparator, followed by only the text
operations selected by the exact checked function closure, in catalog order.
When that closure uses collections, two exact `env` imports follow the Toolkit
imports: `spx_collection_checked_v2` and `spx_collection_drop_v2`. A closure
without collections gains neither import nor a collection arena.
The aggregate profile appends its selected checked text imports after the
existing optional String group; later host groups use that dynamic count.

Each checked text import accepts its left-to-right evaluated operands and one
aligned, frame-bounded output pointer. It returns status zero or an operation's
closed failure set. The compiler authenticates that set, executes the exact
canonical operation-failure cleanup vector, and branches with the selected
status. Cleanup cannot replace it. A result is loaded only after status zero.
`string_to_i64` produces a zeroed 16-byte presence/i64 packet; emission validates
its exact compiler-owned `Option<i64>` layout and converts presence to the
actual declaration-order tag. It never assumes authored tag ordinals.

Wire statuses 21–22 normalize to `semaprax.convert.v1` codes 1–2, statuses 23–25
to `semaprax.text.v1` codes 1–3, and 65–71 to
`semaprax.filesystem.v1` codes 1–7. They are private adapter transport, not new
source failure domains. Unsupported or forged status values fail stop.
Collection statuses 26–29 retain `semaprax.map.v1` codes 1–4; statuses 30–33
retain `semaprax.map.v2` codes 1–4. Checked collection results publish only after
status zero, and use the same independently replayed operation-failure cleanup.
Collection arena capacity refusal uses the existing status 11 transport and
the causes `collection_owners`, `collection_bytes`, or `collection_tokens`.

Slice checks signed byte ranges before UTF-8 boundaries. Find permits byte
starts inside a code point and an empty needle matches exactly at `from`.
ByteAt returns the unsigned byte as `i64`. Trim removes only the six specified
ASCII whitespace bytes. Decimal parsing accepts an optional minus followed by
one or more ASCII digits, rejects overflow and all trailing text, and permits
`-9223372036854775808`. Comparison uses unsigned UTF-8 bytes, including NUL,
with prefix-before-extension ordering. Borrowed-to-owned conversion copies.
File reads preserve the existing path grammar and operation/reservation/file
bounds and validate the complete file as UTF-8 before publishing an owner.

## Ownership, budgets, and gates

The standalone arena authenticates positive opaque tokens and exact extents;
String views retain the compiler-proved owner's token. Aggregate carriers use
the separate tagged owned byte arena. The representations are never exchanged.
The existing derived owner/stack limits, expression/function/literal/module
work bounds, allocation refusal cause, and explicit per-owner finalizers apply.
The host never bulk-clears an arena to hide a missing finalizer. Failed entries
must settle every live owner before returning their normalized outcome.
Optional `maxOwnedCollections` is 1 through the descriptor's derived owner
capacity (default: its minimum with 16); `maxOwnedCollectionBytes` is 1 through
8388608 (default: 8388608). Smaller valid quotas select capacity outcomes and
canonical cleanup; malformed configuration fails before ordinary execution.

Condition and match child scopes compose the independently replayed canonical
cleanup plan. Condition temporaries settle before either Bool outcome; lazy
operands remain lazy. Existing String selectors refuse condition allocations
that require this additive profile. Allocation refusal retains the arena's
existing capacity outcome and generated cleanup sweep.

Executable gates are `tests/language/wasm_text_toolkit_v1.rs` and
`wasm::internal_strings::tests::toolkit`. They cover canonical source/graph and
artifact round trips, byte/NUL/Unicode and strict parse behavior, owned variant
calls/matches, String relational operators, repeated success/failure entries,
condition failures and lazy false-first behavior, fresh Web publication,
missing/invalid file authority, invalid UTF-8, over-bound output, forged provider
status, forged module bytes, and malformed ownership/cleanup proof controls.
Generated-package Node execution additionally exercises legacy and typed maps,
sets, record transport, removal, deterministic iteration, and repeated owner
settlement. This gate does not claim browser execution.
These gates were authored before combined-batch verification; no passing run is
claimed by this implementation record.
