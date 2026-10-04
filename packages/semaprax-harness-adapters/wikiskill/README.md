# WikiSkill evolution bridge (experimental, HN-15)

Process bridge for the host's `skill.evolve/v1` capability (see
`docs/HARNESS-EVOLUTION-V1.md`). It is not a provider descriptor: the host runs it only for an
explicit evolution experiment. `adapter.py` drives the pinned community implementation
`ashutoshsinghpr7/wikiskill` 0.1.5 (commit `cdc291c4296e30488a5922860dc596763704ede4`, MIT) through its
CLI against a metering `claude` shim and reports `unavailable` when the executable or the shim is
missing. `family.py` builds one compiler-repair task family and its experiment spec from the real
compiler. The 2026-10-04 run and its limits are in the doc and in
`benchmarks/harness/2026-10-04-wikiskill/`.
