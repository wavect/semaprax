# TeamDesk Enterprise independent acceptance v1

This supplements the **launch interface**, not application requirements.
[SPEC.md](../SPEC.md) remains byte-for-byte frozen. The gate owns its expected
fields, rules, workflow edges, permissions, and computed results; it never
loads a candidate manifest, schema module, predicates, or self-test oracle.
A candidate's successful self-test is recorded but cannot qualify the app.

Each campaign seed receives the same SPEC and this launch protocol before a
model runs. Both arms provide `build.sh`, `test.sh`, and `run.sh` in the candidate
root. The scripts run from that root. `build.sh` creates runnable artifacts;
`test.sh` performs the candidate's own checks. Both exit zero on success and
run without server processes left behind. `run.sh` stays in the foreground,
starts the real API and browser app, and emits a JSON line after readiness:

```json
{"api_base_url":"http://127.0.0.1:41001/","ui_base_url":"http://127.0.0.1:41002/"}
```

The server listens only on the supplied `TEAMDESK_HOST=127.0.0.1` and
`TEAMDESK_PORT`; a separate UI server may use `TEAMDESK_UI_PORT`. Both URLs
have `/` as their path. The gate allocates these ports and preserves them on
restart. All durable application state belongs in `TEAMDESK_DATA_DIR`.
`SEMAPRAX_BIN` is an explicitly supplied already-built compiler. SIGTERM stops
the foreground process group gracefully; the gate waits before restarting.
No compiler build, model, external service, or production authority is needed.
Fresh data and an empty evidence directory are required for every invocation.

## Public API adaptation

The two existing reference protocols differ only in spelling/envelopes. They
retain ordinary application authentication and authorization:

| Operation | SEMAPRAX projection | TypeScript reference |
| --- | --- | --- |
| Entity route | `/api/time_entry` (snake case) | `/api/timeentry` (lowercase) |
| First Admin | documented `--setup`: create Team, then Admin Member | `POST /api/setup` with name/email/password |
| Sign in | `POST /api/session`, login/email value and password | same route, email and password |
| Current member | `GET /api/session` | `GET /api/me` |
| Sign out | `DELETE /api/session` | same |
| CSRF | GET `/api/session/csrf`; retain cookie and send `x-csrf-token` on mutations | ordinary session cookie; no reference CSRF endpoint |
| Audit entry and affected row | `id` is the affected row; actor/time `by` / `at` | `id` is the audit event; affected row `row_id`, then `record_id`; actor/time `member_id` / `time` |

Entity CRUD, CSV `?format=csv`, search `?q=`, enumeration filter query fields,
and `/<id>/history` are the public interface. JSON int fields are **numeric
signed64 values**, including exact endpoints. The external JSON decoder keeps
numeric source lexemes, rather than hiding precision loss through JS Number.
A forbidden read is hidden and 404; a forbidden write remains 403 according to
SPEC (an unreadable target may already be 404). Mutation errors return an
`errors` array with every violated rule. Passwords never appear in rows.
These paths add no migration, deployment, or other application feature.

Readiness probes use the documented current-member route, accepting a JSON
object with 200 or unauthenticated 401; setup endpoints are not health probes.
Audit changed-field values may be a two-item `[old,new]` pair or an object
with exactly `old` and `new` fields. Their values and every audit obligation
remain independently checked. CSV headers are decoded as CSV, including
quoted columns and an optional leading UTF-8 BOM. Records must have consistent
width and required columns, and quoted fields preserve commas, quotes and
newlines. These representation normalizations apply identically to both arms.


The browser uses ordinary visible controls: sign-in labels email/password;
sign-in label capitalization and surrounding whitespace are immaterial;
entity navigation; field labels; New/Edit/Create-or-Save/Delete/Cancel;
search, enum filters, table column sorting, Prev/Next, CSV, and history.
An enum filter may expose its field as the control's accessible name with a
separate `All` clear option, or retain the legacy field-qualified
`field: all` clear option; an unrelated `All` control is not that field's filter.
Create, Edit, and Delete may be links or buttons when their accessible action names are unique. Form field lookup is scoped to the active form and uses that field's exact accessible label; navigation text does not satisfy a form field.
SEM uses hash routes; the TS reference uses pathname routes. Selectors permit
both existing captions (`+ New Task` or `New Task`, `Next ›` or `Next`,
`CSV` or `Export CSV`). An integer editor may use a numeric input or an
exact decimal text input with `inputmode=numeric`; physical request/readback
checks remain authoritative for integer values. Direct page checks open a fresh
document; actual link checks await the destination's visible heading to avoid
inspecting an earlier SPA route while asynchronous fetches are still running. The
gate does not require a CSS framework, candidate test ids, or a supplied
manifest. Chromium interacts with the real UI and records download bytes and
a screenshot. Browser authoring witnesses use values expressible through the permitted
single-line string editor, including quotes and UTF-8; arbitrary multiline
string preservation is independently checked through API and CSV.
Client validation is checked by observing **zero entity
mutations** before the displayed error, rather than reading client code.

## Password evidence limits

A transparent test-only Node observer invokes actual scrypt/PBKDF2 functions
with their original operands and results. It records salts, derived keys and
work parameters outside the data directory. The gate verifies distinct
persisted salted keys, absence of the known plaintext password, password
change behavior, and no password in API rows. Automatic qualification supports
observed scrypt at ordinary default work (N>=16384,r>=8,p>=1) or PBKDF2
SHA256/SHA512 with >=100000 iterations, salt>=16 bytes and key>=32 bytes.
These are sufficient supported evidence recipes, **not a requirement that a
candidate use these APIs or algorithms**. Different strong KDFs (including
bcrypt/Argon2/WebCrypto), parameters, or storage encodings require an independent
proof audit. Lack of observer evidence is `unverified`, not a claimed SPEC
failure, and never silently accepted. Binary SQLite files and JSON files are
both permitted; evidence examines bytes, not a prescribed storage schema.
The ledger contains derived secret material for disposable test accounts and
must be kept private with the other evidence and discarded with it.

## Commands and evidence

Use Node24 or later. Playwright1.62.0 and its matching Chromium are pinned:

```sh
npm ci --prefix benchmarks/webapp-tokens-v2/acceptance
npm exec --prefix benchmarks/webapp-tokens-v2/acceptance -- playwright install chromium
node --test benchmarks/webapp-tokens-v2/acceptance/gate.test.mjs
node benchmarks/webapp-tokens-v2/acceptance/run.mjs \
  --arm typescript --candidate /absolute/candidate --output /absolute/fresh-evidence
```

For SEM, additionally supply `--compiler /absolute/semaprax
--compiler-source-sha <exact40hex-source-sha>`. A pinned installed Playwright
package elsewhere may be selected with `--playwright-root /absolute/package`.

Qualification of the unchanged references uses an external adapter:

```sh
node benchmarks/webapp-tokens-v2/acceptance/prepare-reference.mjs \
  --arm typescript --candidate /absolute/new-ts-candidate
node benchmarks/webapp-tokens-v2/acceptance/prepare-reference.mjs \
  --arm semaprax --candidate /absolute/new-sem-candidate
```

The TS adapter builds the pinned reference and supplies a static SPA server
with a transparent API proxy. The SEM adapter invokes the supplied compiler's
`webapp` projection. The adapter adds launch scripts, not application behavior.
Build/typecheck and candidate self-test run before the independent gate.

`report.json` contains exact SPEC, gate, pre/post-candidate file digests,
compiler binary/source identity when supplied, Node version, launch endpoints,
every passed/failed/unverified case, missing obligations, password evidence,
and timestamps. `process.log` retains child stdout/stderr. Missing checks and
unverified obligations fail qualification. Anonymous checks cover every entity
CRUD method, row history, CSV, audit, and browser entity/dashboard routes.
Positive CRUD changes every stored field and reads it back; foreign-id type
failures cannot substitute for reference existence checks. Both numeric
endpoints and quoted integer output are checked independently. Cases continue after ordinary
assertion failures; fatal bootstrap/browser failures produce explicit missing
cases. A physical browser failure is not converted into an API-only pass. Reference
DOM compatibility assumptions (tables for list extraction and native select/
input controls) are adapter limits, not extra SPEC requirements. Unsupported
semantic layouts are unverified and require a reviewed browser adapter; they
are never silently accepted or called a functional SPEC failure. The oracle
expectations remain fixed when an adapter changes. Detail/dashboard/reference
checks use visible field text, counts, and actual row links rather than dt/dd
or h2/h3 tags. Numeric and string sorting compare the complete independent
order for every stored/computed column across all25-row pages in both directions.
Equal sort keys may retain any order; complete row membership and monotonic
field values are checked independently. Read-only computed mutations are
refused or ignored, and computed values are observed in the detail UI.
There is no live-agent gain claim in this gate preparation.

The gate starts one application plus one Chromium process and issues hundreds
of bounded local requests. Each build/check is bounded180s, startup30s,
requests10s, browser actions10s, and server lifetime45min. Install/build costs
are recorded outside model token cost; qualification wall time is retained.
No service/API/model cost is incurred. A matched campaign must keep the full
failed-run denominator and separately retain raw Codex events/usage/cache data.
