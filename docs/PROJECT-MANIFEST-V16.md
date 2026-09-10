# Project Manifest v16: Useful Data v2

Status: implemented bounded Project profile; **HOSTED GREEN** under the
[v0.4.0 release baseline](RELEASE-0.4.0-STATUS.md), including the admitted
cross-package roundtrip across Project backends.

Audience: compiler, project-tooling, and standard-library contributors.

Project v16 selects `useful-data.v2`. It combines the existing checked internal
owned-data linking profile with the frozen Useful Data v1 public byte-export
boundary. This permits a library to compose private Reader/Writer functions
while retaining its existing borrowed-slice/scalar exports and contracts.
It does not select Public Owned Data API v1 or relax that profile's
contract-free closure rule.

## Manifest

The flat projection has exactly the same ordered fields as Project v3, with
an authenticated new schema/profile pair:

```toml
schema = "semaprax.project.v16"
name = "json-writer"
version = "0.1.0"
profile = "useful-data.v2"
entry = "json_writer.examples"
sources = ["src/examples.spx", "src/tests.spx", "src/write.spx"]
web_exports = ["json-writer.quoted-length"]
tests = ["json_writer.tests"]
```

The table manifest remains `semaprax.manifest.v1`; its package profile selects
this flat projection. An empty export list selects private entry/test execution
without a public artifact. Schema
v3 with profile v2, or schema v16 with profile v1, is rejected. No command,
capability grant, provider, public nominal type, or generic ABI is added.

## Separate internal and public admission

One immutable authenticated Project snapshot supplies all role projections.
Entry and test execution use the existing owned-data linker, including its
checked nominal identities, ownership, contract evaluation, and independent
cleanup replay. Private dependency records and consuming helpers may therefore
compose behind the public boundary.

When exports are selected, the Web role retains its actual entry-main closure plus the selected export
closures, using the existing conservative reachability rules. Unrelated private
functions are excluded. The complete Web projection must still pass the frozen
Useful Data v1 byte-export admission and emission. Consequently an entry-main
closure that itself exceeds that public emitter's profile is rejected; this
version does not replace an authored entry with an unchecked synthetic body.
With no exports, the retained role is internal only: no public emitter or
descriptor is constructed, an owning entry closure is allowed, and an npm
artifact request fails closed.

Public parameters/results remain exactly those accepted by Useful Data v1.
An owning Reader/Writer export, generic export, effectful export, or other
unsupported signature fails before publication. The original contract checks
and status/result publication order are preserved. Private ownership support
is not a public ownership descriptor.

Legacy Useful Data v1 consumers keep their previous function inventory when
all dependency declarations meet the old profile. For dependency modules with
newer unsupported signatures or types, the linker retains the legacy-compatible
functions and their complete checked callee closure. Unused newer members receive
no target authority; any retained unsupported member still fails the ordinary
v1 linker. Authored module checks and dependency permit/effect checks remain
unchanged. The existing JSON writer sibling consumer exercises this fallback.

## Artifacts and replay

The byte-export Wasm emitter, JavaScript bindings, TypeScript types, and
`semaprax.data-exports.v1` descriptor retain their existing format and carrier
contract. Raw owned tokens and internal nominal records do not cross them.
The existing deterministic npm carrier format commits the exact Project schema
v16 and revision in its identity and digest; replay accepts that schema only
through the same checked Useful Data artifact reconstruction. A v3/v16
cross-pair cannot substitute a different Project identity.

All previously accepted v3 parsing, rendering, descriptors and emitter bytes
remain unchanged. This profile does not extend Project v1's inline scalar-Web
carrier or the Public Owned Data API descriptor.

## Focused verification

`project::admission::tests::useful_data_v2_keeps_public_contract_abi_and_private_owned_calls_separate`
checks canonical v16 parsing, a public callee's retained postcondition, exclusion
of an unrelated private owned function, refusal under the old internal profile,
and rejection of an owning public root and schema/profile confusion.
`profile_admission::project_v16_json_cursor_public_facade_replays_and_executes`
additionally checks deterministic npm reconstruction and envelope replay,
repeated execution of the borrowed-byte facade in Node, absence of the private
owning function from that facade, and refusal to publish an empty export list.
The standard JSON writer cursor corpus additionally exercises private owned
composition on the interpreter, native C11 at `-O0` and `-O2`, and Core Wasm
through authenticated Project snapshots; its decoder/writer cases include a
300-byte decoded string. Six malformed-input, insufficient-capacity, and
forged-cursor contract-rejection cases are retained. Cross-package decode/requote
roundtrip uses the named
`private_json_cursor_roundtrip_executes_across_project_backends` gate on the
interpreter entry and repeated test, native C11 at `-O0` and `-O2`, and repeated
Core Wasm with a strict two-entry arena. The unchanged 16 MiB budget fits.
The implemented release corpus is hosted green; earlier local observations
remain historical witnesses. No broader nominal/public ownership support is
created by that evidence classification.
