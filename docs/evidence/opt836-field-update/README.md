# OPT836 source-only handoff

Audience: integration maintainers and reviewers.

Status: authored, all executable gates pending. Immediate prior complete file
bytes come from immutable e3eb476838442b58c02ccc815aae4bbdfebfd50e.
`manifest.json` binds every prior/current file, all unchanged frozen independent
receiver inventories, the exact additive source patch and its inverse.
New files have no prior archive. Patches use zero context; replay with
`git apply --unidiff-zero`. Backend lane records remain separate.

No Cargo/build/test/Clippy/benchmark/Claude command ran in this authoring lane.
Historical global metadata is deliberately uncomposed; parent integration owns
one additive composition after both lanes arrive. No old source-locked gate is
weakened, skipped, repinned or described as passing here.
