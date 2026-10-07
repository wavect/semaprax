# Webapp Browser v1

This local Chromium smoke exercises the generated TeamDesk v2 UI through a real
browser: first-account setup and sign-in, task creation, a permitted workflow
transition, a status-filtered list, and CSV download. It also fails on browser
JavaScript page errors. The server and its disposable data directory stay on
loopback; the test removes the data directory when it exits.

Generate the app with the compiler under test, then install the pinned test
dependency and Chromium once:

```sh
SEMAPRAX_BIN=/absolute/path/to/semaprax
OUT_DIR="$(mktemp -d)/teamdesk"
"$SEMAPRAX_BIN" webapp benchmarks/webapp-tokens-v2/semaprax/teamdesk.spx -o "$OUT_DIR"
cd platform-tests/webapp-browser-v1
npm ci
npx playwright install chromium
SEMAPRAX_WEBAPP_ROOT="$OUT_DIR" npm test
```

The browser smoke checks the UI against a fresh local fixture. It does not
exercise hosted deployment, external services, or production data.
