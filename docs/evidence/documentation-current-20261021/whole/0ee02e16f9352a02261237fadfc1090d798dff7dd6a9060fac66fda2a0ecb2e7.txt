# CI merge repair, 2026-10-18

This merge combines local browser capture repair `3a205c841` with main
`e32be37755524c57e7205044935807aab71f2b0b`, preserving whole prior source
packets, source views, and the canonical formatted dispatcher.

The [incoming CI run](https://github.com/wavect/semaprax/actions/runs/39087714126)
reported nine E0659 errors in `ordinary_cross_phase.rs`: two glob imports
exposed different `require_expression` functions. The explicit ancestor import
selects the original receiving helper with its lookup and budget arguments.
All nine calls and every other statement are unchanged; `import-repair.json`
contains the whole-source inverse evidence.

The retained memo accessor receives one local Clippy type-complexity allowance.
Its original tuple type and body remain unchanged. Metadata refreshes retain
complete prior fields and named views and keep every reader cap unchanged.

Browser captures in `browser-graph-pins-8db669070` belong to that exact prior
hosted head. They do not establish current-head browser or full CI success.
Local checks recorded here are source checks only. Full compiler, browser,
quality, and Lean qualification remain the responsibility of actual hosted CI.
