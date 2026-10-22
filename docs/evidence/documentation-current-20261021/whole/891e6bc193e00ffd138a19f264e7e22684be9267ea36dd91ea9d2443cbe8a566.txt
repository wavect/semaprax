# Exact7a87 focused qualification

Status: historical source evidence; qualification is limited to the recorded revisions and results.

Audience: compiler contributors and reviewers.

Execution source: `7a87b33115c0cd45b14cac64d7b37e5742503159`.
Five focused build commands passed; 145 exact selected bodies executed:
**120 PASS, 25 FAIL**. Scoped core and full-CLI production Clippy passed.
No full quality gates, full test suite or live-agent benchmark ran.

`round/` preserves byte-exact receipts, commands, logs, controllers and memory
observations. All25 failures remain disclosed in `closure-audit.json`.
`AUDIT.md` is the original conditional issue audit: #83330/30PASS;
#83522executed=20PASS+2disclosed applicationFAIL. It does not establish closure
without the separately completed actual producer/pin/consumer chain.

`raw-pair-arm/` preserves the one-literal raw #841 arm. Both raw and gzip
fixture owners passed; `pair-final-settlement.json` binds the completed common
owner to the unchanged earlier conditional summary. Gzip reduced unit artifact
bytes by4738944 and wall time by31.74s in this pair, but peak process-group RSS
increased681115648bytes. Build order, cache and ambient memory were uncontrolled.
No demonstrated RAM benefit, application recovery or live token gain is claimed.
Earlier resource-refused attempts remain in their separate existing archive.

Later source changes require their own applicable checks; these receipts are
never reinterpreted as execution at a later commit.
