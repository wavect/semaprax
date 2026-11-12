# CLI Help v5: bounded authoring guidance

Status: implemented source; current-head execution of the owning help harnesses
is pending the grouped verification batch.

Audience: CLI users, coding agents, and compiler contributors.

V5 changes the default `semaprax help language` response. It now prints a
short authoring guide with supported source and Project routes, profile
distinctions, and exact selectors for deeper help. The complete
`AGENT-QUICK-REFERENCE.md` remains available byte-for-byte through
`semaprax help language all`. Existing `topics` and exact topic selectors keep
their behavior. Diagnostics, library catalogs, capability rules, and profile
admission do not change.

The authoring guide is a checked-in document embedded in the compiler. The
default response is bounded to 2,048 bytes and 400 lexical units. Both
standalone and full-capability CLI tests check its exact bytes, the explicit
full-reference bytes, exact topic behavior, usage grammar, and inert execution
in an empty working directory.

The guide distinguishes single-file web source profiles from Project profiles.
It names only the admitted `internal-strings-v1` and `text-toolkit-v1` source
profiles and identifies `source-command-file-text` as a native Project starter
with its declared profile, input, and target. Generated project documentation
remains authoritative for each starter's dependencies and commands.

This is guidance, not an admission mechanism. A profile or capability is
available only when the ordinary compiler route accepts it.

## Explicit fused source usage

Command usage now includes the authored `fused-construction-v1` source-only
run/build opt-in with mandatory `--managed-bytes`. Interpreter fuel remains
explicitly separate from native/Wasm account policy. The source, target and
publication contract is [Fused source profile v1](FUSED-SOURCE-PROFILE-V1.md);
executable qualification is pending. Existing command and help bounds remain.

## Bounded application discovery

`help language author:routes` lists the complete application-route inventory.
Every admitted route appears as its full exact selector, including
`author:selected-file-resource`; descriptions never abbreviate selectors.
Exact `author:stdin-json`, `author:file-text`, and `author:source-web` selectors
give the admitted profile/target, build and test commands, required capabilities,
starter, limitations, and exact selectors for checked library prerequisites.
These routes are not aliases for the existing seventeen language topics.
Unknown routes refuse with exit two and no stdout.

`help language find:<word>:<offset>` searches the embedded generated library
catalog. A word has 1..64 ASCII letters, digits, dots, underscores or hyphens;
matching is case-sensitive substring matching over module, identity, display
name and checked signature. The offset is canonical unsigned decimal. Results
are ordered by stable identity. Each response includes at most 2,048 UTF-8 bytes,
whole declarations with their exact dependency, required profile and signature,
and either complete matching coverage or an exact next-page command. No result
is silently truncated. An entry larger than a page refuses with its exact
`help library` selector. Empty matches, malformed queries and out-of-range
offsets refuse. The fixed page limit is not a runtime-language capacity.

Search uses the existing catalog, not a competing semantic index. No selector
reads a user project, compiles source, writes files or gains a capability.
Focused tests cover deterministic route output, malformed selectors, complete
multi-page traversal without duplicates, exact byte budget and one-short
refusal. Smaller help bytes alone do not establish live-agent savings.

## Typed records, outcomes and JSON response discovery

`author:collection-records` describes the v31 record carrier. The distinct
`author:nested-outcomes` card explains v32 typed success/error carriers, whole
payload moves, direct nominal imports, commas inside match arms, and the closed
owning result boundary. It links the [nested outcome contract](OWNED-NESTED-OUTCOMES-V1.md)
and [native v32 route](PROJECT-V32-NESTED-OUTCOME-COMMAND-V1.md).

`author:record-field-views` distinguishes detached clones from selected-field
reads, Copy loan completion from String/Bytes view lifetimes, and ordinary
out-of-range failure. `author:string-from-utf8` distinguishes the shared Fixed
meter from native execution and keeps escaped JSON handling separate from the
validated unescaped bulk constructor.

`author:json-nested-response` documents the encode-only
`bounded-nested-response.v1` selector, both canonical bounds, borrowed APIs,
preflight refusal, and the explicit primitive `Vec<string>` refusal. The
[nested response contract](APPLICATION-JSON-NESTED-RESPONSE-V1.md) owns the
schema and limits. These cards describe authoring contracts; their bytes and
selector assertions do not establish application qualification or efficiency.

`author:static-utf8` explains adjacent `str"..."`, immutable borrowed scope,
exact decoded UTF-8/NUL bytes, named `str_as_bytes` views, and ordinary owning
String fallback. Its independent source/canonical/HIR assertion and CLI selector
checks are owning definitions pending execution. The card preserves the shared
65536-byte admission bound, closed escape/public adapters, and pending
Project/template qualification from the [static literal contract](STATIC-BORROWED-UTF8-LITERALS-V1.md).

`author:vec-constructor` owns typed `vec_of<T>` advice: exact capacity, separate
ordinary push commits, left-to-right items, sticky failure and unchanged resource
bounds. P220 covers syntax/reservation refusal and T318 covers authenticated
expansion/hygiene refusal. The card grants no public/interop profile, ABI or
capability; [Typed Vec Constructors v1](TYPED-VEC-CONSTRUCTORS-V1.md) owns the
authored implementation and pending executable qualification.

## Source-shape corrections

The bounded source guide distinguishes ordinary native builds from the explicit
fused source opt-in, without asserting its pending qualification. Scalar help
names the adjacent static borrowed `str"..."` literal. The full card checks a
borrowed-text helper example and a `SPX-T312` nonzero exit-guard example; the
normal guard arm remains exact `else { 0 }`. A retained private function
signature does not authorize authored type names, cases or constructors without
the corresponding explicit type import. Diagnostic help is regenerated only
through the owning documentation projection, never edited as a pin shortcut.

## Exact NDJSON discovery and repair prerequisites

Exact library lookup for `std.data.ndjson`, its public declaration name, or its
stable identity appends one bounded usage card. It embeds the complete existing
`std/data-ndjson/src/examples.spx` bytes, preserving the package's independently
owned compiler gates. The card names the dependency and owning API profile,
global and per-line bounds, current-input frame checks, structured fault
categories, and the caller's JSON/publication responsibilities. It does not
claim origin authentication for numeric cuts or executable qualification.
Default module inventory and explicit full-catalog output retain their owners.

Unknown-function repair hints enumerate admitted Vec, List, Map/Set, Box and
iterator operation registries, preserving ambiguity refusal and declared-name
precedence. Exact bundled-function repairs include the generated catalog's
required profile with the existing dependency, identity and signature. These
are help-only changes; diagnostic identity, span and compiler admission stay
unchanged. Generated catalogs must be refreshed by their existing producer.

[Scaffold capsule v8](PROJECT-SCAFFOLD-V8.md) carries corrected bounded discovery
and control-flow guidance for newly created projects while preserving earlier
capsule replay bytes.

The same nested-response authoring card now names additive
`bounded-nested-response.v2` String-slot composition, its distinct identities
and existing caps. The v1 String-vector refusal is explicit. Focused v2 source,
Project and backend checks passed. [V2 contract](APPLICATION-JSON-NESTED-RESPONSE-V2.md)

## Search existing authoring cards

`help language author:find:<word>:<offset>` searches the existing route selector,
summary and exact card text. It uses the same word and canonical-offset grammar
as library search, but reports only matching authoring cards. Results sort by
selector (so the word `author` enumerates the full card inventory), include whole selector/summary rows and exact detail commands, and
carry complete coverage or a deterministic next-page command within 2,048 bytes.
The existing exact card owns all profile and prerequisite guidance; a discovery
row alone makes no support claim. Every page separately links library search.
No matches, invalid offsets and entries larger than the page budget refuse.

The inventory is derived from the existing routes and exact-card implementation,
without another semantic catalog or aliases. `core.string.take` names the
existing `string_take` card's compiler identity. Library-only `find:` behavior,
ordering and continuation stay unchanged. This additive search is inert and
does not establish live-agent token savings; executable qualification is pending
the grouped owning CLI checks.
