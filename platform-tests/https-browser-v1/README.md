# HTTPS Browser v1

This provisioned fixture executes the real generated Project-v13 npm package
in Chromium. The browser loads authenticated Wasm over loopback, supplies only
the explicit deterministic HTTPS fixture-v3 provider, checks exact output,
rejects invocation reuse and tampered Wasm, and proves that no request leaves
the loopback origin.

Build the package into a fresh absolute directory, install the locked browser
dependencies, and run:

```sh
SEMAPRAX_HTTPS_PACKAGE_ROOT=/absolute/generated npm test
```

This gate proves browser execution of the fixture-backed HTTPS capability. It
does not grant ambient `fetch`, public-endpoint authority, or live browser TLS.
