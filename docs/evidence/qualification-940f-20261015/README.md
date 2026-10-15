# Focused qualification at 940fcf2

Historical source: `940fcf220e47d1975f757e76f0b3401691db9358` (tree `0a7e853aa6df60acdb838ab2a7a7d6e0dde025a6`). These receipts describe that source, not later corrections.

The unit-first build completed all three stages: library unit-test compilation, production library, and the five selected integration harnesses. Build wall time was 750.955 seconds; maximum observed process-group RSS was 5.82 GiB. No test bodies ran during compilation.

Exactly 136 registered owning runtime checks executed serially: **116 passed, 20 failed**. There were no selected-count mismatches. All individual logs, the complete result receipt, and the frozen manifest are retained. Ten failures concern the existing application builder-budget cases and their codec-frontier witness. The other ten owning failures are being investigated; this archive does not claim they were repaired.

Scoped production library and CLI Clippy failed on three warnings: a redundant field initializer, an unused import, and an unused argument. Its actual log and resource receipt are retained. No full suite or full quality gate ran.

The role-checked actual pin producer completed and emitted all ten wires. Its successful capture does **not** mean the pin consumers passed. Strict read-only validation prepared eleven literal changes, from the prior actual amount 1,428,092 to **1,440,482** (+12,390), plus exact wire/source inverses. That patch is deliberately **unapplied**: the next implementation batch must regenerate its own final pins. All actual wires and the read-only plan are retained here.

These are compiler checks and allocation/budget observations, not fresh agent application benchmarks or evidence of a token advantage over TypeScript. Known application failures remain open. Claude was not used for this qualification.
