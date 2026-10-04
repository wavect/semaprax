#!/usr/bin/env python3
"""Deterministic libtest-shaped output with one failure buried in repetition.

`--counter FILE` appends one execution to FILE (ground truth for "exactly one
host execution"). Any other argument is ignored, so a profile can tag the run.
"""
import sys

args = sys.argv[1:]
if "--counter" in args:
    path = args[args.index("--counter") + 1]
    try:
        n = int(open(path).read().strip() or 0)
    except OSError:
        n = 0
    open(path, "w").write(str(n + 1))

TOTAL, BAD = 320, 173
print(f"running {TOTAL} tests")
for i in range(TOTAL):
    status = "FAILED" if i == BAD else "ok"
    print(f"test ledger_suite::case_{i:03d} ... {status}")
    if i % 40 == 39:
        print("note: using fixed-point arithmetic profile (repeated notice)", file=sys.stderr)
print()
print("failures:")
print(f"---- ledger_suite::case_{BAD:03d} stdout ----")
print(f"thread 'ledger_suite::case_{BAD:03d}' panicked at src/lib.spx:6:5:")
print("assertion failed: ledger.line_total(3, 4) == 12 (left: 7, right: 12)")
print()
print(f"test result: FAILED. {TOTAL - 1} passed; 1 failed; 0 ignored")
sys.exit(1)
