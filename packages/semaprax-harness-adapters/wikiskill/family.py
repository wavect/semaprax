#!/usr/bin/env python3
"""Build the HN-15 task family and experiment spec from the REAL compiler.

Family `compiler-diagnostic/SPX-P106`: a program written with a habit from another
language (`else if`, `return`) is rejected with SPX-P106; the repair is one line.
Every broken and repaired module is checked by the real `semaprax` compiler while
building (a task whose fix does not verify is never emitted). The traces carry only
public compiler/check outcomes; no model output, reasoning or credentials.

usage: family.py <compiler> <out-dir> <spec-id> [--env KEY=VALUE ...]
Writes <out-dir>/traces.jsonl and <out-dir>/experiment.json.
"""
import json
import os
import subprocess
import sys
import tempfile

FAMILIES = {"p106": ("compiler-diagnostic/SPX-P106", "SPX-P106"), "t250": ("compiler-diagnostic/SPX-T250", "SPX-T250")}
FAMILY, CODE = FAMILIES["p106"]

# (id, split, setup lines, broken line, repaired line)
TASKS = [
    ("p106-t1", "train", ["let x = 4;"], "if x > 5 { 1 } else if x > 2 { 2 } else { 3 }",
     "if x > 5 { 1 } else { if x > 2 { 2 } else { 3 } }"),
    ("p106-t2", "train", ["let a = 3;", "let b = 4;"], "return a * b;", "a * b"),
    ("p106-t3", "train", ["let n = 7;"], "if n < 0 { 0 - 1 } else if n == 0 { 0 } else { 1 }",
     "if n < 0 { 0 - 1 } else { if n == 0 { 0 } else { 1 } }"),
    ("p106-t4", "train", ["let x = 4;"], "if x > 5 { return 1; } else { 2 }", "if x > 5 { 1 } else { 2 }"),
    ("p106-t5", "train", ["let x = 9;"], "return x + 1;", "x + 1"),
    ("p106-t6", "train", ["let x = 8;"], "if x > 9 { 4 } else if x > 6 { 3 } else if x > 3 { 2 } else { 1 }",
     "if x > 9 { 4 } else { if x > 6 { 3 } else { if x > 3 { 2 } else { 1 } } }"),
    ("p106-v1", "validation", ["let m = 12;"], "if m > 20 { 7 } else if m > 10 { 8 } else { 9 }",
     "if m > 20 { 7 } else { if m > 10 { 8 } else { 9 } }"),
    ("p106-v2", "validation", ["let p = 5;", "let q = 6;"], "return p + q;", "p + q"),
    ("p106-v3", "validation", ["let k = 3;"], "if k > 1 { return 10; } else { 20 }", "if k > 1 { 10 } else { 20 }"),
    ("p106-x1", "test", ["let h = 15;"], "if h > 30 { 1 } else if h > 14 { 2 } else { 3 }",
     "if h > 30 { 1 } else { if h > 14 { 2 } else { 3 } }"),
    ("p106-x2", "test", ["let w = 2;", "let z = 21;"], "return w * z;", "w * z"),
    ("p106-x3", "test", ["let g = 6;"], "if g > 5 { return 100; } else { 200 }", "if g > 5 { 100 } else { 200 }"),
]

# extra historical in-family sessions (train-side only; none repeats a held-out answer)
HISTORY = [
    ("hist-1", "if s > 50 { 1 } else if s > 25 { 2 } else { 3 }"),
    ("hist-2", "return s - 1;"),
    ("hist-3", "if s > 0 { return 5; } else { 6 }"),
]
HISTORY_FIX = {
    "hist-1": "if s > 50 { 1 } else { if s > 25 { 2 } else { 3 } }",
    "hist-2": "s - 1",
    "hist-3": "if s > 0 { 5 } else { 6 }",
}

T250 = [
    ("t250-t1", "train", ["let a = \"ab\";"], 'let s = "ab" + "cd";', 'let s = string_concat("ab", "cd");'),
    ("t250-t2", "train", ["let a = \"foo\";", "let b = \"bar\";"], "let s = a + b;", "let s = string_concat(a, b);"),
    ("t250-t3", "train", [], 'let s = "a" + "b" + "c";', 'let s = string_concat(string_concat("a", "b"), "c");'),
    ("t250-t4", "train", ['let a = "hi";'], 'let s = a + "!";', 'let s = string_concat(a, "!");'),
    ("t250-t5", "train", ['let inner = "xy";'], 'let s = "(" + inner + ")";', 'let s = string_concat(string_concat("(", inner), ")");'),
    ("t250-t6", "train", [], 'let s = "n=" + "42";', 'let s = string_concat("n=", "42");'),
    ("t250-v1", "validation", [], 'let s = "up" + "down";', 'let s = string_concat("up", "down");'),
    ("t250-v2", "validation", ['let l = "left";', 'let r = "right";'], "let s = l + r;", "let s = string_concat(l, r);"),
    ("t250-v3", "validation", [], 'let s = "1" + "2" + "3";', 'let s = string_concat(string_concat("1", "2"), "3");'),
    ("t250-x1", "test", [], 'let s = "key" + "val";', 'let s = string_concat("key", "val");'),
    ("t250-x2", "test", ['let p = "pre";'], 'let s = p + "fix";', 'let s = string_concat(p, "fix");'),
    ("t250-x3", "test", ['let m = "mid";'], 'let s = "<" + m + ">";', 'let s = string_concat(string_concat("<", m), ">");'),
]
T250_HISTORY = [("hist-1", 'let s = "q" + "r";', 'let s = string_concat("q", "r");'),
                ("hist-2", 'let s = "[" + "]";', 'let s = string_concat("[", "]");'),
                ("hist-3", 'let s = "t" + "u" + "v";', 'let s = string_concat(string_concat("t", "u"), "v");')]

PROMPT = (
    "The SEMAPRAX compiler rejects the module below.\n\n"
    "```\n{src}```\n\nCompiler diagnostic:\n{diag}\n\n"
    "Write to `answer.txt` exactly one line: the repaired replacement for the one offending "
    "line of main's body, trimmed (no indentation; keep a trailing `;` only if the repaired line "
    "is a statement), written the way `semaprax fmt` prints it (`if` and `else` stay on one "
    "line). The file must contain nothing else: no code fence, no explanation."
)


def module(i, setup, line):
    tail = ["string_len(s)"] if CODE == "SPX-T250" else []
    body = "\n".join("    " + s for s in setup + [line] + tail)
    return f'module app.t{i};\n\n@id("app.main")\nfn main() -> i64\n{{\n{body}\n}}\n'


def check(compiler, src):
    with tempfile.TemporaryDirectory(dir="/private/tmp/claude-501/hp-tools/wikiskill-work") as d:
        p = os.path.join(os.path.realpath(d), "app.spx")
        open(p, "w").write(src)
        r = subprocess.run([compiler, "check", p], capture_output=True, text=True, cwd=d)
        fmt = subprocess.run([compiler, "fmt", p, "--check"], capture_output=True, text=True, cwd=d)
        out = (r.stdout + r.stderr).replace(p, "app.spx")
        return r.returncode, out, fmt.returncode


def first_error(out):
    for line in out.splitlines():
        if line.startswith("error["):
            return line.strip()
    return ""


def code_of(err):
    return err[err.index("[") + 1: err.index("]")]


def main():
    global FAMILY, CODE, TASKS, HISTORY, HISTORY_FIX
    compiler, out_dir, spec_id = sys.argv[1:4]
    key = os.environ.get("FAMILY_KEY", "p106")
    FAMILY, CODE = FAMILIES[key]
    if key == "t250":
        TASKS = T250
        HISTORY = [(h, bad) for h, bad, _ in T250_HISTORY]
        HISTORY_FIX = {h: fix for h, _, fix in T250_HISTORY}
    env = dict(a.split("=", 1) for a in sys.argv[4:] if a != "--env")
    os.makedirs(out_dir, exist_ok=True)
    version = subprocess.run([compiler, "--version"], capture_output=True, text=True).stdout.strip()
    tasks, records = [], []
    for i, (tid, split, setup, bad, fixed) in enumerate(TASKS, 1):
        src_bad, src_ok = module(i, setup, bad), module(i, setup, fixed)
        rc, out, _ = check(compiler, src_bad)
        err = first_error(out)
        assert rc != 0 and code_of(err) == CODE, (tid, out)
        rc2, out2, fmt_rc = check(compiler, src_ok)
        assert rc2 == 0 and fmt_rc == 0, (tid, out2)
        tasks.append({"id": tid, "split": split, "expected": fixed,
                      "prompt": PROMPT.format(src=src_bad, diag=err)})
        if split == "train":
            records += [
                {"schema": "semaprax.evolution-trace.v1", "task_id": tid, "family": FAMILY, "kind": "check",
                 "tool": "semaprax check", "outcome": "error", "diagnostic_code": CODE,
                 "message": err, "attempt": 1},
                {"schema": "semaprax.evolution-trace.v1", "task_id": tid, "family": FAMILY, "kind": "repair",
                 "tool": "semaprax check", "outcome": "ok", "attempt": 2,
                 "message": "repaired module verified and canonical", "repair": f"{bad}  =>  {fixed}"},
            ]
    for j, (hid, bad) in enumerate(HISTORY, 1):
        src = module(50 + j, (["let s = 30;"] if CODE == "SPX-P106" else []), bad)
        rc, out, _ = check(compiler, src)
        err = first_error(out)
        assert rc != 0 and code_of(err) == CODE, (hid, out)
        fixed = HISTORY_FIX[hid]
        rc2, out2, fmt_rc = check(compiler, module(50 + j, (["let s = 30;"] if CODE == "SPX-P106" else []), fixed))
        assert rc2 == 0 and fmt_rc == 0, (hid, out2)
        records += [
            {"schema": "semaprax.evolution-trace.v1", "task_id": hid, "family": FAMILY, "kind": "check",
             "tool": "semaprax check", "outcome": "error", "diagnostic_code": CODE,
             "message": err, "attempt": 1},
            {"schema": "semaprax.evolution-trace.v1", "task_id": hid, "family": FAMILY, "kind": "repair",
             "tool": "semaprax check", "outcome": "ok", "attempt": 2,
             "message": "repaired module verified and canonical", "repair": f"{bad}  =>  {fixed}"},
        ]
    traces = os.path.join(out_dir, "traces.jsonl")
    with open(traces, "w") as f:
        for r in records:
            f.write(json.dumps(r, sort_keys=True) + "\n")
    spec = {
        "schema": "semaprax.evolution-experiment.v1", "id": spec_id, "family": FAMILY,
        "adapter": {"command": [env["WIKISKILL_PYTHON"], env["ADAPTER"]], "env": {
            k: env[k] for k in ("HOME", "USER", "LOGNAME", "PATH", "WIKISKILL_BIN", "WIKISKILL_WORK_ROOT", "WIKISKILL_SHIM_DIR",
                                "WIKISKILL_SRC", "WIKISKILL_COMMIT", "WIKISKILL_MAX_TURNS", "WIKISKILL_RESUME", "WIKISKILL_WIKI_FROM") if k in env}},
        "workspace_root": env["WIKISKILL_WORK_ROOT"],
        "consent": {"traces": [traces], "retention": "keep"},
        "parent": {"name": "ponytail", "dir": env["PARENT_DIR"]},
        "protected": [p for p in env.get("PROTECTED", "").split(",") if p],
        "tasks": tasks,
        "caps": {"max_iterations": 1, "max_model_calls": 60, "max_seconds": 3300},
        "gate": {"max_test_regression": 0, "max_skill_bytes": 4096},
        "min_traces": 2,
    }
    with open(os.path.join(out_dir, "experiment.json"), "w") as f:
        json.dump(spec, f, indent=1)
    print(json.dumps({"compiler": version, "tasks": len(tasks), "records": len(records)}))


if __name__ == "__main__":
    main()
