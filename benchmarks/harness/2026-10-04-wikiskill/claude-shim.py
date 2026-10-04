#!/Users/kevin/.local/bin/python3
"""Call-counting, budget-capping shim for the real `claude` CLI (HN-15 bounded run).

Caps: 60 invocations and USD 5.00 total (ledger: ../calls.jsonl). Refuses with exit 75
once either is reached. Forces --model haiku and --setting-sources project; strips any
--dangerously-skip-permissions; clamps --max-budget-usd to min(request, 1.00, remaining).
Never touches ~/.claude. The candidate's per-workspace CLAUDE_CONFIG_DIR is not forwarded
to claude (auth would break); its profile skills are exposed as project skills in cwd.
Cost source: total_cost_usd from the stream-json/json `result` event (measured, not estimated).
"""
import fcntl, hashlib, json, os, shutil, subprocess, sys, time

REAL = "/Users/kevin/.local/bin/claude"
BASE = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
LEDGER = os.path.join(BASE, "calls.jsonl")
LOCK = os.path.join(BASE, ".lock")
MAX_CALLS, MAX_USD, PER_CALL = 60, 5.00, 1.00


def ledger():
    rows = []
    if os.path.exists(LEDGER):
        for line in open(LEDGER):
            line = line.strip()
            if line:
                rows.append(json.loads(line))
    return rows


def append(row):
    with open(LEDGER, "a") as f:
        f.write(json.dumps(row, sort_keys=True) + "\n")


def main():
    args = sys.argv[1:]
    out, i = [], 0
    requested = None
    while i < len(args):
        a = args[i]
        if a == "--dangerously-skip-permissions":
            i += 1; continue
        if a in ("--model", "--setting-sources"):
            i += 2; continue
        if a == "--max-budget-usd":
            requested = float(args[i + 1]); i += 2; continue
        if a in ("--allowedTools", "--allowed-tools") and i + 1 < len(args):
            out += [a, args[i + 1] + ",Skill"]; i += 2; continue
        out.append(a); i += 1
    lockf = open(LOCK, "w")
    fcntl.flock(lockf, fcntl.LOCK_EX)
    rows = ledger()
    calls = sum(1 for r in rows if r.get("event") == "start")
    spent = sum(r.get("cost_usd") or 0.0 for r in rows if r.get("event") == "end")
    if calls >= MAX_CALLS or spent >= MAX_USD:
        append({"event": "refused", "t": time.time(), "calls": calls, "spent_usd": round(spent, 4)})
        fcntl.flock(lockf, fcntl.LOCK_UN)
        sys.stderr.write(f"claude-shim: cap reached (calls={calls}/{MAX_CALLS}, usd={spent:.4f}/{MAX_USD}); refusing\n")
        return 75
    budget = min(requested if requested else PER_CALL, PER_CALL, MAX_USD - spent)
    cid = f"c{calls + 1:03d}"
    digest = hashlib.sha256("\0".join(args).encode()).hexdigest()[:16]
    t0 = time.time()
    append({"event": "start", "id": cid, "t": t0, "args_sha256": digest, "cwd": os.getcwd(),
            "budget_usd": round(budget, 4), "cost_usd": None})
    fcntl.flock(lockf, fcntl.LOCK_UN)

    env = dict(os.environ)
    ccd = env.pop("CLAUDE_CONFIG_DIR", None)
    cwd = os.getcwd()
    if ccd and os.path.isdir(os.path.join(ccd, "skills")):
        proj = os.path.join(cwd, ".claude", "skills")
        os.makedirs(proj, exist_ok=True)
        for n in os.listdir(proj):
            p = os.path.join(proj, n)
            if os.path.islink(p):
                os.unlink(p)
        for n in sorted(os.listdir(os.path.join(ccd, "skills"))):
            src = os.path.realpath(os.path.join(ccd, "skills", n))
            dst = os.path.join(proj, n)
            if not os.path.lexists(dst):
                os.symlink(src, dst)
    cmd = [REAL] + out + ["--model", "haiku", "--setting-sources", "project",
                          "--max-budget-usd", f"{budget:.2f}"]
    p = subprocess.run(cmd, env=env, stdin=sys.stdin, stdout=subprocess.PIPE, stderr=sys.stderr)
    data = p.stdout
    sys.stdout.buffer.write(data); sys.stdout.flush()
    cost, models, turns, is_err = None, None, None, None
    text = data.decode("utf-8", "replace")
    for line in reversed([l for l in text.splitlines() if l.strip()]):
        try:
            j = json.loads(line)
        except ValueError:
            continue
        if isinstance(j, dict) and "total_cost_usd" in j:
            cost = j["total_cost_usd"]; turns = j.get("num_turns"); is_err = j.get("is_error")
            models = sorted((j.get("modelUsage") or {}).keys())
            break
    fcntl.flock(lockf, fcntl.LOCK_EX)
    append({"event": "end", "id": cid, "t": time.time(), "ms": int((time.time() - t0) * 1000),
            "rc": p.returncode, "cost_usd": cost if cost is not None else 0.0,
            "cost_measured": cost is not None, "is_error": is_err, "turns": turns, "models": models})
    fcntl.flock(lockf, fcntl.LOCK_UN)
    return p.returncode


sys.exit(main())
