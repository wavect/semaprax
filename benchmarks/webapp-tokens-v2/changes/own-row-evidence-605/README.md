# Own-row permission evidence for issue #605

The external API checker passed against fresh disposable servers for both
arms on 2026-10-07. Each Agent could read and update its own Task, Expense,
and Leave; a second Agent's writes were denied, and its Expense was hidden.
These are HTTP API observations, not browser UI evidence.

The SEMAPRAX server was generated from
`benchmarks/webapp-tokens-v2/semaprax/teamdesk.spx` (SHA-256
`93b7c95f058b12d09b4799c15a36a1ebfe1cab38a5a595a2381618c112da4bc9`) using
the frozen binary `/Users/kevin/.codex/benchmark-binaries/semaprax-2f2ec1acf`
(SHA-256 `a8527bee158f6b6d1822e106b7022a354900a81a0bd57f4c888cfb9b823e2b77`).
The binary reported `{"schema":"semaprax.version.v1","version":"0.9.0","commit":null,"maturity":"beta","rust_min":"1.88"}`. The generated `server.mjs` SHA-256 was
`45afb870336005a3aedf2e6c15cbf4ad07dba8ad86136eddd7d01dac11a043af`.
The frozen binary's own hash is recorded because its version record has no
commit identity.

The TypeScript arm used Node `v24.3.0` and npm `11.4.2`; it ran the checked-in
server directly with a fresh empty data directory. The server SHA-256 was
`dd6d5c02df14d55e64d0a90ed7d7e1a218bbd4c6e7c7002c63bcbc00f5383232`, and the
shared schema SHA-256 was
`cb0df1a384ab680ef7dd0702b68b23fc248ded860b63c88d5c753842707bd0e6`.

The checker SHA-256 for both runs was
`3d5eda0cd47d0ab2d195f211cfd1c38573f8aad815f69154fec574f272f8275d`. It maps
the `TimeEntry` and `TicketReply` route names to `time_entry` and
`ticket_reply` for SEMAPRAX; TypeScript retains its lowercased routes
`timeentry` and `ticketreply`. Both route sets were exercised in the initial
empty-database checks.

- SEMAPRAX: 79 HTTP observations passed; raw output is in
  [semaprax-permission-api.log](semaprax-permission-api.log). Its SHA-256 is
  `e9b852b0c662ee3ce7b9d39890ac4c3b7abfaa75c5de0f6e0bf6b1f20cf9d0a6`.
- TypeScript: 57 HTTP observations passed; raw output is in
  [typescript-permission-api.log](typescript-permission-api.log). Its SHA-256
  is `a165c3a15e1644d4f6a0248a7475d970a8a2a64d1037020645bf205de14f63bc`.

The built-in generated SEMAPRAX `--self-test` was also captured in
[semaprax-builtin-selftest.log](semaprax-builtin-selftest.log) (SHA-256
`509d0ff8f769dc550782548e2277deaa3effcf32d62888fc165c70bbdbbc5ccc`). It
still reports `Agent ... writes: none` because its permission fixture tests
rows owned by another account only. The API passes therefore establish the
own-row behavior, while the requested built-in self-test evidence still needs
an owning self-test fixture change and a corresponding regression.
