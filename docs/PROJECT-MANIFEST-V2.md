# Project Manifest v2

Status: versioned bounded reference; the completion matrix owns product status.

Audience: language users, tool authors, and compiler contributors.

Project Manifest v2 is the locally evidenced packaging profile for one Useful
Text Consumer v1 project. Exact-head hosted promotion is pending.

The canonical manifest adds required `version` and
`profile = "useful-text-consumer.v1"` fields to the bounded Project authority.
It retains explicit sorted sources, one entry module, one test module, and
explicit stable-ID export roots. The existing held-file authentication,
single in-memory Phase-A link, drift checks, and stable-ID rename behavior are
unchanged. Project v1 parsing, builds, carriers, and output bytes remain
unchanged.

`semaprax build --manifest-path semaprax.toml --target npm -o <new-directory>`
publishes an exact create-new six-file package:

- `app.wasm`
- `semaprax.js`
- `semaprax.bindings.js`
- `semaprax.bindings.d.ts`
- `semaprax.text-exports.json`
- `package.json`

The pathless `semaprax.project-npm-build.v1` carrier binds the Project schema,
package/version, Project/workspace/graph revisions, exact artifact order,
per-artifact bytes and digests, cumulative byte count, canonical semantic
recipe, and payload digest. Context-free inspection independently replays that
recipe through the real parser, resolver, text planner, and Wasm emitter and
proves compiler consistency, but it does not authenticate self-claimed Project
facts or create publication authority. Only the opaque build prepared by a
retained authenticated Project snapshot carries the trusted context required
before materialization. The opt-in Project daemon can return the same bounded
carrier inline without accepting a path or gaining filesystem, process,
network, npm, registry, or publication authority.

For filesystem publication, Project-v2 default/`web`/`wasm` and explicit
`npm` all select this same six-file text package. The daemon likewise maps its
v2 `web` and `npm` requests to the same pathless text carrier. The legacy
`ProjectSnapshot::build_web_inline` return type remains Project-v1 scalar-only;
v2 library callers use `build_npm_inline` rather than confusing the two carrier
schemas.

The shared Unix npm publisher writes create-new artifacts relative to held
directories. Before reporting success it also reopens the requested parent's
canonical path without following newly substituted components and compares its
filesystem identity with the retained parent. Canonical pathname equality alone
is insufficient: a parent or ancestor can be moved aside and replaced while
the held writes correctly continue into the displaced tree. Such replacement
must report the existing parent-identity error, not success at a different
directory. Pre-existing parent aliases remain admitted when they still bind
the same canonical path and identity.

This final check is an observation, not a permanent pathname lock or atomic
publication guarantee. Failures preserve the completed artifacts or partial
prefix under the held destination, and leave foreign replacement bytes alone;
there is no new cleanup or rollback. The correction changes no artifact,
carrier, descriptor, or Windows route. Real-carrier Unix regression cases in
`src/project/npm/publication/tests.rs` and thread-local test-hook isolation
cases in `hook_tests.rs` are authored but unrun. The hooks exist only in Unix
test builds and do not add production concurrency authority.

Local evidence builds the real config-validator fixture, preserves exports by
stable ID across a display rename, performs offline `npm pack`, installs the
result into a compiler-free consumer with scripts disabled, type-checks its
declarations, and executes it when the required local tools are available.
These are local pack/install tests, not npm-registry publication, registry
compatibility, signing, provenance, dependency resolution, lockfile, or
production-distribution claims.

See [Useful Text Consumer v1](USEFUL-TEXT-CONSUMER-V1.md) for the closed
language and ABI boundary.
