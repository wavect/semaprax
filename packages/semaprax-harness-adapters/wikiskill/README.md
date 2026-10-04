# WikiSkill evolution bridge (experimental, HN-15)

Adapter for the host's `skill.evolve/v1` capability (see
`docs/HARNESS-EVOLUTION-V1.md`). It is not a provider descriptor: the host
invokes it only for an explicit evolution experiment. `adapter.py` currently
performs the preflight and reports `unavailable` with the exact missing piece;
the real WikiSkill call path is not implemented because no audited
implementation can run against a local model endpoint (audit in the doc).
