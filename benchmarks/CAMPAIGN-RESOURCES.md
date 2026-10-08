# Local matched campaign resource policy

The Codex LogLens, ShiftSim and TeamDesk adapters apply the same disk checks to
both language arms and to the separately reported calibration request. Before
each attempt, every output and Codex rollout volume must have at least 5 GiB
free. A 1 MiB written reserve supports emergency receipts. Monitoring runs every
five seconds throughout model work, acceptance and candidate archival.

This floor cannot guarantee space on a shared machine. A sampled low-space
event, retained stderr disk error, or runner I/O failure records a resource
incident and stops subsequent paid attempts. The runner does not restart the
attempt or change its acceptance result. Original costs remain in the attempted
task denominator; unreconciled or missing usage and billing remain unknown.

`resource-receipts/<arm>-<number>.json` retains the row independently of
`results.json`, including an emergency row after an I/O failure. A `prepared`
receipt alone does not prove a paid launch: inspect the task-owned transcript.
The reserve is released at the first incident or on completion. Receipts and
raw transcripts are evidence and must be preserved; reproducible build caches
may be removed only after confirming they have no live users.

Resource-contaminated, unmonitored historical, and incomplete campaigns cannot
support a clean comparative headline. Acceptance counts and known conditional
costs may still be reported with those qualifications. All source closures
include the resource policy implementation, and ShiftSim now snapshots its
entire local runtime closure before calibration.

The 2026-10-08 incident and offline recovery are recorded in
`live-campaign-resource-incident-20261008.json` and
`live-campaign-interrupted-recovery-20261008.json`. Their three missing paid
attempts were retained without model or acceptance reruns. Original results and
raw logs were not overwritten. Recovered complete input/output usage, cost,
wall time, and final application acceptance remain unknown.
