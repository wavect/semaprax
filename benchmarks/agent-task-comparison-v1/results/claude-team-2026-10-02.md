# Coding-agent pilot: bounded Claude Team cohort

All **36/36 scheduled trials** ran genuinely: 18 task/lane/repetition positions for each of two pinned models. No trial was retried, and no earlier aborted cohort supplied a completion record.

| Model | Trials | Turn-limit failures | Incomplete at byte limit | Full oracle passes | Reported API-equivalent cost |
|---|---:|---:|---:|---:|---:|
| `claude-haiku-4-5-20251001` | 18 | 18 | 0 | 0 | $0.396215 |
| `claude-sonnet-5-5` | 18 | 0 | 18 | 0 | $1.053706 |

**Result:** 0 trials passed every acceptance oracle under this profile. These are bounded technical outcomes; they do not establish relative capability or superiority. Total reported API-equivalent cost: **$1.449921**, with 0 unknown-cost trials. The ledger retained $9 in nonrefundable reservations. These figures are not subscription invoice charges, and no mathematical provider-framing dollar bound is claimed.

## Frozen experiment

- Runner: `df6e6d72a49e917810319c1482c1ebb5a56c7a15`.
- Protocol SHA-256: `2378f87a01a61baf087efb7a161ece8f8c9cfe6bd530b3b3337625b9b7b0573f`.
- Claude Code 2.1.286; installed SEMAPRAX 0.7.0 compiler, with separate exact binary/revision pins in the result record.
- 120 seconds per trial, eight CLI tool-turn setting, at most nine actual guarded requests; 32 KiB per request, 64 KiB total request bytes, and 512 requested output tokens per request.
- Original tasks, source/graph lanes, drift interventions, ownership/refusal checks and acceptance oracles retained. The guard stops excess delivery before forwarding; actual request receipts bound dispatch, while CLI turn counts remain observed metadata.
- Review policy: explicit user waiver, with technical verdicts retained and no human review or human timing claimed.

## Evidence

[Machine-readable result](claude-team-2026-10-02.json) binds every task/model/trial to output, candidate, MCP wire and receipt hashes, reported usage, terminal classification and oracle verdict. Native output and candidate archives are retained privately in packet `issue321-native-cohort08-20261002`; they contain no copied auth home or credentials.
The complete cohort audit hashes to `1d72af73cd8f1401c99b530ceb8fce55e2ced8fead1bb4c69140b4b174a86963`; the independent raw-receipt/ledger linkage audit hashes to `a229f614ff82e15a8494a2bcf7080468e2a65a0f4bbf125b0c8d35d4cc47337e`. Both passed for all 36 records. Evidence is local operator-held evidence, not cryptographic provider attestation or hosted execution.

Earlier cohorts remain separate aborted packets. They are excluded from this result.
