# Provisioned frame-payload browser evidence

Status: authored, unrun; Chromium only, no hosted or public promotion.

This fixture reuses the repository's existing pinned Playwright 1.62.0 under
`platform-tests/wasm-scalar-browser-v1/node_modules`. It installs nothing and
requires an already provisioned Chromium binary. Missing prerequisites fail.

Provision two loopback HTTP directories containing `index.html`, `browser.mjs`,
`corpus-runner.mjs`, the exact canonical `corpus.json`, and the compiler-generated
`generated/` npm inventory. Build one from the canonical frame-payload Project
and the other after changing only the display name of `payload_result`, retaining
`@id("frame.payload-result")`. Never alter the stable ID, signature or corpus.
The repository's Project-v8 acceptance test owns the source rename derivation;
this browser gate consumes host-provisioned artifacts, checks equal export rows
and distinct revision facts, and does not authenticate source derivation itself.

From the repository root, select the gate explicitly:

```sh
SEMAPRAX_FRAME_BROWSER_URLS='{"before":"http://127.0.0.1:4173/","after":"http://127.0.0.1:4174/"}' node platform-tests/wasm-scalar-browser-v1/node_modules/@playwright/test/cli.js test --config platform-tests/frame-payload-browser-v1/playwright.config.mjs
```

No existing hosted workflow selects this new fixture. Both directories and
existing dependency/browser provisioning are host responsibilities. The gate
compares received corpus bytes exactly and executes the shared Node/browser
runner over every case in both packages; it is not multi-engine evidence.
