# Owned Data Browser v1

Status: authored, unrun; no current-head browser or hosted promotion claim.

This is the provisioned WP-10 direct-`Bytes` boundary fixture. It imports the
actual generated package, not a host-injected `semapraxOwnedData` global. The
existing Chromium, Firefox and WebKit projects remain selected; each requires
an already provisioned browser and the existing pinned Playwright 1.55.0
dependency. The suite installs nothing and has no retries.

## Provisioning

Build the checked-in `project/semaprax.toml` with the compiler from the commit
under review. It exports exactly `frame.fail-after`, `frame.fail-before`,
`frame.mixed` and `frame.payload`. The two failure functions divide by a caller
argument before or after creating an owned byte value. They are real language
functions, not substituted WebAssembly implementations.

For example, from the repository root, with a prebuilt compiler and an existing
host-owned parent (the `generated` destination must not exist):

```sh
semaprax build platform-tests/owned-data-browser-v1/project/semaprax.toml --target npm -o /absolute/host-owned/generated
```

Serve that generated directory on loopback HTTP, with JavaScript modules served
using a JavaScript MIME type. Its package inventory is exactly:

```text
app.wasm
semaprax.js
semaprax.bindings.js
semaprax.bindings.d.ts
semaprax.api.json
package.json
```

`SEMAPRAX_OWNED_DATA_PACKAGE_URL` is now a **directory URL**, not an HTML page.
It must use `http://127.0.0.1`, end in `/`, and contain no credentials, query or
fragment. Select the fixture explicitly after provisioning:

```sh
SEMAPRAX_OWNED_DATA_PACKAGE_URL='http://127.0.0.1:4173/' node platform-tests/owned-data-browser-v1/node_modules/@playwright/test/cli.js test --config platform-tests/owned-data-browser-v1/playwright.config.mjs
```

Missing URLs, package files, browsers or required browser features fail; they
do not skip the test. The test supplies its own blank document with
[COOP/COEP cross-origin isolation headers](https://developer.mozilla.org/en-US/docs/Web/API/Window/crossOriginIsolated)
so shared-buffer rejection is exercised rather than silently omitted. It
permits requests only to the selected package directory and disables service
workers, following [Playwright's request-interception guidance](https://playwright.dev/docs/network).
This test request policy is not an OS network sandbox.

## Evidence and limits

The browser runner covers empty, binary/NUL/invalid-UTF8 and 65,535/65,536-byte
copies; cumulative UTF-8-plus-byte input bounds in both mixed branches;
65,537-byte input rejection; detached/shared/resizable and wrong-brand values;
constructor/species/accessor hostility; independent retained outputs and
repeated calls; and recovery on the same instance after genuine checked
failures before and after owned staging. It requires shared, resizable and
transferable buffers rather than silently weakening the selected engine's
coverage. A calibrated observer of the real `WebAssembly.instantiate` checks
that tampered Wasm is rejected before engine instantiation.

`tests/owned_data_browser_fixture_v1.rs` separately authenticates the checked-in
Project subject, selected signatures, descriptor and exact six-artifact inline
carrier through the compiler's public APIs. It does not launch browsers or
publish files. The browser gate consumes host-provisioned bytes: metadata and
Wasm consistency are not signatures or proof that a host served this exact
source revision. Provisioning must keep the generated package bound to the
commit under review.

Successful reuse after failure is observable lifecycle evidence, not a full
physical allocation/destruction trace. The 65,537-byte case checks borrowed
input rejection, not a separately generated oversized owned result. Raw
pointer/tag/token faults, native settlement, sanitizers, TypeScript checking,
and exact-head cross-platform promotion retain their separate owning gates.
No compiler, runtime, schema, dependency or hosted workflow changes are made
by this fixture completion.
