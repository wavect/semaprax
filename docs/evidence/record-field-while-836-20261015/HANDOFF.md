# #836 while-field admission correction — source only

Source basis: `7a87b33115c0cd45b14cac64d7b37e5742503159`.
No Cargo, Rust tests, Clippy, backend execution or benchmark ran in this lane.
The #846 checkpoint e2521dac remains immutable on its separate branch.

The actual 145-round failures archived here distinguish two causes:
owner30/33/34 source admission refuses the four-argument source call using the
three-runtime-child arity; owner31/35 refuse a stale backend source inverse
before executing backend numeric controls. These logs are prior failed evidence,
not results of the corrected source.

Only three production conditions change: HIR source resolution, iterative source
verification and its independent current source oracle count four arguments for
UpdateField. All other Vec operations retain their old arity; HIR remains three
children. The existing lowering independently validates the literal selector,
nominal owned-leaf shape and exact field type, removes the selector, and retains
ordinary argument staging/commit. No backend, owner, cleanup, quota or failure
selection implementation changes. The complete historical borrowed-root and
before-nested receivers remain byte-exact.

One new existing-library test body covers a real three-iteration store, source
checking, HIR validation and exact child count, graph replay, codec replay and
interpreter result, plus too-few/many operands, non-Copy/computed selectors and
wrong scalar type stable refusals. It is authored, not executed. Existing
same-source eight-lane native/Wasm/old-row oracle fixtures remain unchanged.

The existing shared backend support adds a bounded authenticated full-source
adapter before each original table. `backend-input-views.json` protects two
backend inputs and one later coverage input in distinct immutable domains.
Every one of the 13+4 original owners retains its unique old edit and exact
full-original comparison; all 6+2 children retain exact old hashes. No old
packet/archive/assertion is rewritten. This is whole-input authentication,
not replacing an expected hash or selecting the new store as the old oracle.

`current-to-7a87.json` / `source-inverses.json` restore eight complete immediate
inputs including the source helper itself. The helper prepends this stage to
older guards before #831's existing stage. No new Rust owner/module is added;
all changed files already participate in their owning source inventories.

Static verification only:

```
python3 docs/evidence/record-field-while-836-20261015/verify.py
```

Actual result: eight full immediate inverses and all 25 old backend/coverage
owner/child input checks pass. `source-proof.json` records this source-only
result. All old immutable tracked evidence remains unchanged.

The parent must preserve all 26 #836 gates and original ShiftSim15 positive
requirements in the next grouped qualification, plus the one new library test
in `pending-selectors.json`; its five prior failed selectors are already selected.
No backend success, application recovery or #836 closure is claimed.

Future composition: authenticate merged physical bytes into this checkpoint
before the new backend adapter hashes its full inputs, and separately before
older inherited guards enter current-to-7a87. Keep this packet and both original
backend JSONs immutable. The two backend input domains must remain independent.
