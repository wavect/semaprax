# f813 + held-auth source custody successor

Status: historical source evidence; qualification is limited to the recorded revisions and results.

Audience: compiler contributors and reviewers.

Source-only evidence. No Cargo, build, Rust tests, Clippy, Node, Graft CLI or
runtime qualification was executed. Production source basis is
`0ccaeef55025e50dd7566273b1e70bae8f3e3dfa`; four existing test-only helpers have
complete immediate inverses in `source-inverses.json`. The final source hashes
are in `receipt.json`. These helper changes require the parent's focused gates.

All 101 prior active packets / 904 rows remain immutable at their existing paths.
`composition-proof.json` names each exact active manifest and its SHA. The shared
`current-to-prior.json` layer has 38 complete physical-current-to-e4b source
joins. It transports all those old packet inputs to their prior domain without
copying the large historical manifests or deleting any of their 3,372 hooks and
317 existing joins. The inherited verifier re-executes all old joins, original
capacity bounds, 53 receivers, 38 starter bindings, 32+6 starter patch paths and
66 838/839 patch paths in that explicit prior domain. This is a layered current
proof, not a claim that the prior-stage Rust sources are physically unchanged.
The 53 independent originals remain 46 byte-unchanged and 7 fully joined.

The protected held-auth JSON retains SHA
`96ecd68d1a6367d8f3fe871d9fc5e14ebca69a02a9d3d263977936a65d844936`.
Its 26 complete current/previous rows are authenticated independently, including
null predecessors for genuinely new files. Its literal Rust test now projects
physical current to the exact e6ee stage before every original after-hash and
full-source assertion, and passes raw current into `previous()` once. No old
runtime JSON, old proof, script, archive or historical hook was rewritten.

Fresh main's immutable source_reflows.json has 85 rows. Physical inputs first
project to exact f813, then all original main inverse/hash checks execute.
Both newest presence records authenticate after that CI projection. The old
88884 runtime JSON and old Source36 aliases remain separate immutable domains.
The actual Source36 helper chain is physical -> held predecessor -> pre-ours
2b5 -> unchanged old ours 6e -> unchanged four named old views -> original36
inverses. Source36 b.rs adds only named input wrappers for latest main's changed
literal_format and concat_funded sources; every original undo/assertion remains.

838's original protected four views and all 28 old patch/archive files remain
unchanged. One explicit epsilon-test projection bridges latest main formatting
before that domain. All 7+7 original literal patch forward/inverse pairs pass the
source-only mirror. Every one of main's 367 archived merge inputs / 123 paths
matches its independent Git revision, SHA256 and Git blob ID. No graph,
changelog, production Rust, SPX, Cargo or CI guard was edited in this successor.

Run from repository root (refresh expects the exact source basis above):

```sh
python3 docs/evidence/source-composition-f813-held-20261015/refresh.py
python3 docs/evidence/source-composition-f813-held-20261015/verify_source36.py
python3 docs/evidence/source-composition-f813-held-20261015/verify_838.py
python3 docs/evidence/source-composition-f813-held-20261015/verify_inherited.py
```

Actual successful command outputs are the four execution logs. The first
Source36/838 failures exposed the literal input drift and remain in their first
logs. Two initial inherited-runner refusals were virtual-filesystem adapter
errors (physical Git diff, then unexpanded Git pathspec), not old-gate passes;
both refusal logs remain. The final inherited runner preserves every original
assertion and translates only the explicitly named prior source domain. It
writes its result to a new path, never over the old proof. Git cache peaks stay
below 16 MiB; the inherited virtual-file bridge retains no additional cache.

No new Rust module or test selector was introduced. Retain all existing parent
focused selectors, including held-auth, Source36 presence/projection and 838
source guards. This evidence does not establish 838/839 completion, physical
budget fit, application recovery or runtime success.
