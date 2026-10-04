---
name: derived-spx-t250-repair-reference
description: "Inject SPX-T250 pattern reference into string concatenation repair tasks"
---
> Derived skill, not an official snapshot. Parent `ponytail` sha256:34dc9057a6584057a458112f2c7a3758ed90526f90cff03d21f3ca293b9e0ec7; scope `compiler-diagnostic/SPX-T250`; promotion is a separate explicit action.

## When to Apply

Use this skill on any task involving SEMAPRAX error [SPX-T250] (string concatenation). Tasks manifest as diagnostic repairs where the agent must replace `a + b` syntax with the correct function call.

## When NOT to Apply

Do not apply to non-SPX-T250 diagnostics, or to tasks that already reference the pattern in task.md or environment.

## Instructions

### Task Detection

1. Check task.md or task context for error code `SPX-T250`.
2. Confirm the error message contains `operator \`+\` does not support string operands`.

### Pattern Injection

Before solving the repair:

1. Read `<tools>/wikiskill-work/wikiskill-t250-r4/wikiskill/evolve/wiki/patterns/spx-t250-string-concat.md`.
2. Acknowledge the pattern explicitly: e.g., "The pattern for SPX-T250 is to replace `a + b` with `string_concat(a, b)`."
3. Apply the pattern uniformly across all operand types (literals, variables, mixed).
4. For chained concatenation (`a + b + c`), nest calls: `string_concat(string_concat(a, b), c)`.

### Success Criterion

Agent must cite the pattern before proposing a repair, and repairs must use `string_concat()` consistently.

