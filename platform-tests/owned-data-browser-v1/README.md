# Owned Data Browser v1

Status: scoped local compiler and non-pinned Chromium evidence; the pinned
three-browser gate remains unrun, with no hosted promotion claim.

This is the provisioned WP-10 direct-`Bytes` and owned-variant boundary fixture.
It imports two actual generated packages, not a host-injected
`semapraxOwnedData` global. The
existing Chromium, Firefox and WebKit projects remain selected; each requires
an already provisioned browser and the existing pinned Playwright 1.55.0
dependency. The suite installs nothing and has no retries.

## Provisioning

Build the checked-in `project/semaprax.toml` with the compiler from the commit
under review. It exports exactly `frame.fail-after`, `frame.fail-before`,
`frame.mixed` and `frame.payload`. The two failure functions divide by a caller
argument before or after creating an owned byte value. They are real language
functions, not substituted WebAssembly implementations.

Also build `variant-project/semaprax.toml`, whose separate package
`owned-data-browser-variants` exports exactly `frame.maybe` and `frame.result`.
Both functions initialize Bytes unconditionally before selecting `Some`/`None`
or `Ok`/`Err(-7)`. The original Project is unchanged: its local owned-buffer loan
and the new owned variants must not be combined into one module, because the
current Workspace Graph rejects that combination with `SPX-G410`. Moving its
payload call across modules would instead violate `SPX-G172` for an owned
return import. Separate Projects preserve both checks and admission boundaries.

For example, from the repository root, with a prebuilt source-installed full
toolchain and an existing host-owned parent (the `generated` destination must
not exist; replace the example absolute path with that parent's path):

```sh
semaprax-full build platform-tests/owned-data-browser-v1/project/semaprax.toml --target npm -o /absolute/host-owned/generated
```

Provision the distinct variant package into another fresh destination:

```sh
semaprax-full build platform-tests/owned-data-browser-v1/variant-project/semaprax.toml --target npm -o /absolute/host-owned/generated-variants
```

Windows Project-v8 npm publication requires this full host; the standalone
compiler rejects it with `SPX-W120`. Release archives expose the full CLI as
`semaprax`, so use that name instead when provisioning from an archive.

Serve both generated directories on the same loopback HTTP origin, with
JavaScript modules served using a JavaScript MIME type. Each package inventory
is exactly:

```text
app.wasm
semaprax.js
semaprax.bindings.js
semaprax.bindings.d.ts
semaprax.api.json
package.json
```

`SEMAPRAX_OWNED_DATA_PACKAGE_URL` is now a **directory URL**, not an HTML page.
Both it and the mandatory `SEMAPRAX_OWNED_DATA_VARIANT_PACKAGE_URL` must use
`http://127.0.0.1`, end in `/`, and contain no credentials, query or fragment.
Use distinct directories on the same origin. Select the fixture explicitly
after provisioning:

```sh
SEMAPRAX_OWNED_DATA_PACKAGE_URL='http://127.0.0.1:4173/base/' SEMAPRAX_OWNED_DATA_VARIANT_PACKAGE_URL='http://127.0.0.1:4173/variants/' node platform-tests/owned-data-browser-v1/node_modules/@playwright/test/cli.js test --config platform-tests/owned-data-browser-v1/playwright.config.mjs
```

Missing URLs, package files, browsers or required browser features fail; they
do not skip the test. The test supplies its own blank document with
[COOP/COEP cross-origin isolation headers](https://developer.mozilla.org/en-US/docs/Web/API/Window/crossOriginIsolated)
so shared-buffer rejection is exercised rather than silently omitted. It
permits requests only to the exact twelve package artifact URLs and disables service
workers, following [Playwright's request-interception guidance](https://playwright.dev/docs/network).
This test request policy is not an OS network sandbox.

## Evidence and limits

Local validation on 2026-08-31 ran both compiler-fixture tests on macOS
(Rust 1.98) and isolated offline Linux (Rust 1.88): two passes per host. The
actual full CLI published both six-file packages. A temporary host-owned runner
then used byte-identical checked-in test/config files with cached Playwright
**1.62.0** and Chromium **151.0.7922.34**: one selected Chromium test passed.
It served only the twelve snapshotted artifact URLs, selected Chromium explicitly,
and checked package, test/config and Project input bytes again after execution.
The local binding and generated packages are not repository changes. No downloads
were performed. This is **not** the pinned Playwright 1.55.0 gate: its dependency
and Firefox/WebKit binaries were unavailable. The three-project configuration,
dependency pin, zero retries and mandatory capability checks remain unchanged.

The browser runner covers empty, binary/NUL/invalid-UTF8 and 65,535/65,536-byte
copies; cumulative UTF-8-plus-byte input bounds in both mixed branches;
65,537-byte input rejection; detached/shared/resizable and wrong-brand values;
constructor/species/accessor hostility; independent retained outputs and
repeated calls; and recovery on the same instance after genuine checked
failures before and after owned staging. It requires shared, resizable and
transferable buffers rather than silently weakening the selected engine's
coverage. A calibrated observer of the real `WebAssembly.instantiate` checks
that both packages' tampered Wasm is rejected before engine instantiation.

Additional controls admit fixed nonzero-offset views, including an empty view
and a 65,536-byte view inside a larger backing store. Eight resizable-view
rejections exercise fixed and length-tracking views before shrinking, while
partially/fully out of bounds, and after regrowth; effective lengths are checked
before exact-diagnostic rejection. The separate variant facade executes 96
active/inactive/recovery calls and four oversized-input rejection/recovery pairs.
It retains 68 independent outputs, including empty results, and checks them
after input mutation and creation of another base facade. `None` and frozen
`Err(-7n)` are successful language results, not checked-call failures.

`tests/owned_data/browser_fixture.rs` separately authenticates the checked-in
Project subjects, selected signatures, descriptors and exact six-artifact inline
carriers through the compiler's public APIs. Typed HIR binds variant staging to
the unconditional Bytes initializer. It does not launch browsers or
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
