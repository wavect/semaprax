#!/usr/bin/env python3
"""WikiSkill process bridge for `skill.evolve/v1` (HN-15).

Protocol: one JSON request on stdin, one JSON result on stdout (see
docs/HARNESS-EVOLUTION-V1.md). Exit 69 with {"unavailable": reason} when the
backend cannot run; any other failure exits 1 with {"error": reason} (the host
reports `aborted`). Progress goes to stderr (kept in the experiment workspace).

Backend: the pinned community implementation ashutoshsinghpr7/wikiskill 0.1.5
(not author-verified official code), driven only through its CLI
(`init`, `run-task`, `gate`, `maintain`, `propose`). This bridge:

  evolve  consented traces -> the candidate's raw layer; train rollouts;
          `maintain` (wiki update); `propose` (candidate skill or no_action).
          The host gate decides; the candidate's own inner gate is not run.
  solve   one isolated rollout via `run-task`; the answer is read from
          `answer.txt`; the host grades it (no expected answer is ever sent).

Environment (supplied by the experiment spec, never scanned):
  WIKISKILL_BIN        pinned `wikiskill` executable (required)
  WIKISKILL_WORK_ROOT  every workspace must resolve under it (git reset guard)
  WIKISKILL_SHIM_DIR   directory holding the metering `claude` shim; the first
                       `claude` on PATH must be that shim (required). The shim
                       ledger `calls.jsonl` sits next to that directory.
  WIKISKILL_SRC        optional git checkout; HEAD must equal WIKISKILL_COMMIT
  WIKISKILL_COMMIT     optional pinned commit
  WIKISKILL_MAX_TURNS  per-task turn budget (default 8)
  WIKISKILL_WIKI_FROM  wiki directory of an earlier finished workspace to start from (iteration N+1)
  WIKISKILL_RESUME     "1": adopt an existing evolve workspace; only a missing maintain/propose is re-run
"""
import hashlib
import json
import os
import re
import shutil
import subprocess
import sys

NAME = "wikiskill"
BACKEND = "claude"
INNER_VAL = 2  # train tasks reserved as the candidate's own validation split


class Unavailable(Exception):
    pass


class Failed(Exception):
    pass


def log(msg):
    sys.stderr.write(f"[wikiskill-bridge] {msg}\n")
    sys.stderr.flush()


def sha(b):
    return "sha256:" + hashlib.sha256(b).hexdigest()


def real(p):
    return os.path.realpath(p)


def inside(child, root):
    child, root = real(child), real(root)
    return child == root or child.startswith(root + os.sep)


def ledger_calls(env):
    """Started shim calls so far (the authoritative model-call count)."""
    shim = env.get("WIKISKILL_SHIM_DIR", "")
    path = os.path.join(os.path.dirname(shim.rstrip("/")), "calls.jsonl")
    if not os.path.exists(path):
        return 0
    with open(path) as f:
        return sum(1 for line in f if '"event": "start"' in line)


def ledger_rows(env):
    shim = env.get("WIKISKILL_SHIM_DIR", "")
    path = os.path.join(os.path.dirname(shim.rstrip("/")), "calls.jsonl")
    if not os.path.exists(path):
        return []
    with open(path) as f:
        return [json.loads(line) for line in f if line.strip()]


def assert_calls_ok(env, since):  # since: ledger row count when this request began
    """A backend call that failed (auth, cap, API error) must abort the run, never read as no-action."""
    rows = ledger_rows(env)[since:]
    # A call that reached the model but stopped at its turn budget is an ordinary task failure;
    # one that never reached a model (not logged in, refused by the cap, crash) is a backend failure.
    bad = [r for r in rows if r.get("event") == "refused" or
           (r.get("event") == "end" and (r.get("rc") != 0 or r.get("is_error")) and not r.get("models"))]
    if bad:
        raise Failed(f"{len(bad)} backend call(s) failed or were refused by the spend cap; first: {bad[0]}")


def preflight(env):
    binary = env.get("WIKISKILL_BIN", "")
    if not binary or not os.access(binary, os.X_OK):
        raise Unavailable("wikiskill executable not found (set WIKISKILL_BIN to a pinned install)")
    shim = env.get("WIKISKILL_SHIM_DIR", "")
    claude = shutil.which("claude", path=env.get("PATH", ""))
    if not shim or not claude or real(os.path.dirname(claude)) != real(shim):
        raise Unavailable("the first `claude` on PATH is not the metering shim; refusing an unmetered paid backend")
    root = env.get("WIKISKILL_WORK_ROOT", "")
    if not root or not os.path.isabs(root):
        raise Unavailable("WIKISKILL_WORK_ROOT is not set; refusing to run git reset-capable code unanchored")
    src, pin = env.get("WIKISKILL_SRC"), env.get("WIKISKILL_COMMIT")
    if src and pin:
        head = subprocess.run(["git", "-C", src, "rev-parse", "HEAD"], capture_output=True, text=True).stdout.strip()
        if head != pin:
            raise Unavailable(f"wikiskill source is at {head[:12]}, not the pinned {pin[:12]}")


def ws_guard(ws, env):
    """The candidate resets its skill tree with `git reset --hard`; only ever inside the work root."""
    if not inside(ws, env["WIKISKILL_WORK_ROOT"]) or not inside(ws, os.getcwd()):
        raise Failed(f"workspace {ws} is outside the experiment workspace / work root; aborting")


def cli(env, ws, *args, check=True):
    """Run the pinned wikiskill CLI against one isolated workspace."""
    ws_guard(ws, env)
    log("run: wikiskill " + " ".join(a for a in args if not a.startswith("/")))
    p = subprocess.run([env["WIKISKILL_BIN"], *args], env=env, cwd=os.getcwd(), capture_output=True, text=True)
    if p.stderr.strip():
        sys.stderr.write(p.stderr[-2000:] + "\n")
    if check and p.returncode != 0:
        raise Failed(f"wikiskill {args[0]} exited {p.returncode}: {(p.stdout + p.stderr)[-300:]}")
    return p


def slug(s):
    return re.sub(r"[^a-z0-9_-]+", "-", s.lower()).strip("-") or "t"


def task_spec(tid, split, prompt, expected=None):
    grader = ({"type": "exact", "file": "answer.txt", "expected": expected} if expected is not None
              else {"type": "contains", "file": "answer.txt", "needle": "\u0000never-matches"})
    return {"id": slug(tid), "split": split, "title": "Repair a SEMAPRAX compiler diagnostic",
            "prompt": prompt, "sandbox": {"task.md": prompt}, "grader": grader}


def make_ws(env, name, tasks):
    """Fresh candidate workspace holding only `tasks` (no demo bench), claude backend."""
    ws = os.path.join(os.getcwd(), "wikiskill", name)
    ws_guard(ws, env)
    if os.path.isdir(ws):
        shutil.rmtree(ws)
    cfg = os.path.join(os.getcwd(), "wikiskill", "empty-config")
    os.makedirs(cfg, exist_ok=True)  # init copies credentials from here: there are none, by design
    cli(dict(env, CLAUDE_CONFIG_DIR=cfg), ws, "init", NAME, "--ws", ws, "--backend", BACKEND)
    shutil.rmtree(os.path.join(ws, "bench", "tasks"), ignore_errors=True)
    with open(os.path.join(ws, "tasks.json"), "w") as f:
        json.dump(tasks, f, indent=1)
    return ws


def inject_traces(ws, req):
    """Consented host traces -> the candidate's raw layer (iteration 1, train)."""
    with open(req["trace_path"], "rb") as f:
        lines = f.read().decode().splitlines()
    groups = {}
    for line in lines:
        if line.strip():
            rec = json.loads(line)
            groups.setdefault(rec.get("task_id", "trace"), []).append(rec)
    d = os.path.join(ws, "raw", "traces", "iter-01", "train")
    os.makedirs(d, exist_ok=True)
    for i, (_, recs) in enumerate(sorted(groups.items()), 1):
        name = f"semaprax-trace-{i:02d}"
        with open(os.path.join(d, name + ".jsonl"), "w") as f:
            f.write(json.dumps({"backend": "semaprax-host", "tool_call_count": len(recs),
                                "message_count": len(recs)}) + "\n")
            for r in recs:
                f.write(json.dumps(r, sort_keys=True) + "\n")
        with open(os.path.join(d, name + ".meta.json"), "w") as f:
            json.dump({"task_id": name, "split": "train", "iter": 1, "score": None,
                       "title": "Semaprax compiler check and repair trace",
                       "source": "semaprax.evolution-trace.v1", "source_digest": req["trace_digest"],
                       "transcript": f"raw/traces/iter-01/train/{name}.jsonl"}, f, indent=1)
    return len(groups)


def split_front_matter(md):
    m = re.match(r"\A---\n(.*?)\n---\n?(.*)\Z", md, re.S)
    if not m:
        return {}, md
    fm = {}
    for line in m.group(1).splitlines():
        k, _, v = line.partition(":")
        fm[k.strip()] = v.strip().strip("\"'")
    return fm, m.group(2)


def copy_wiki(ws, host_ws):
    entries = []
    src = os.path.join(ws, "wiki")
    for base, dirs, files in os.walk(src):
        dirs[:] = sorted(d for d in dirs if d != ".git")
        for fn in sorted(files):
            p = os.path.join(base, fn)
            rel = os.path.relpath(p, src)
            dst = os.path.join(host_ws, "wiki", rel)
            os.makedirs(os.path.dirname(dst), exist_ok=True)
            shutil.copy2(p, dst)
            with open(dst, "rb") as f:
                entries.append({"path": "wiki/" + rel, "digest": sha(f.read())})
    return entries


def calls_in(env, ws):
    """Shim calls whose working directory is under `ws` (attributes spend to one workspace)."""
    base = real(ws)
    return sum(1 for r in ledger_rows(env) if r.get("event") == "start"
               and (r.get("cwd") == base or r.get("cwd", "").startswith(base + os.sep)))


def carry_wiki(env, ws):
    """Iteration N+1 starts from the wiki of a finished earlier workspace (the wiki is never rolled back)."""
    src = env.get("WIKISKILL_WIKI_FROM")
    if not src:
        return
    if not inside(src, env["WIKISKILL_WORK_ROOT"]):
        raise Failed(f"wiki source {src} is outside the work root")
    dst = os.path.join(ws, "wiki")
    shutil.rmtree(dst, ignore_errors=True)
    shutil.copytree(src, dst, ignore=shutil.ignore_patterns(".git"))
    log(f"wiki carried over from {os.path.basename(os.path.dirname(os.path.dirname(src)))}")


def evolve(req, env):
    train = req["train_tasks"]
    if len(train) <= INNER_VAL:
        raise Failed("need more than 2 train tasks (the candidate keeps 2 as its own val split)")
    turns = env.get("WIKISKILL_MAX_TURNS", "8")
    cut = len(train) - INNER_VAL
    tasks = [task_spec(t["id"], "train" if i < cut else "val", t["prompt"], t["expected"])
             for i, t in enumerate(train)]
    start = len(ledger_rows(env))
    ws = os.path.join(os.getcwd(), "wikiskill", "evolve")
    ppath = os.path.join(ws, "runs", "proposals", "iter-01.json")
    if env.get("WIKISKILL_RESUME") == "1" and os.path.isdir(ws):
        # Re-adopt an existing evolution workspace whose earlier model work was metered. Nothing that
        # finished is repeated: only a maintain/propose step that left no result is run again.
        ws_guard(ws, env)
        log("resuming an existing evolution workspace; finished model work is not repeated")
        if not os.path.exists(ppath):
            cli(env, ws, "maintain", NAME, "--ws", ws, "--iter", "1")
            cli(env, ws, "propose", NAME, "--ws", ws, "--iter", "1")
            assert_calls_ok(env, start)
    else:
        ws = make_ws(env, "evolve", tasks)
        carry_wiki(env, ws)
        base = cli(env, ws, "gate", NAME, "--ws", ws, "--split", "val", "--iter", "0")
        log("candidate-side baseline: " + base.stdout.strip().splitlines()[-1])
        for t in tasks[:cut]:
            cli(env, ws, "run-task", NAME, t["id"], "--ws", ws, "--iter", "1", "--max-turns", turns)
        log(f"injected {inject_traces(ws, req)} consented Semaprax trace groups into the candidate raw layer")
        cli(env, ws, "maintain", NAME, "--ws", ws, "--iter", "1")
        cli(env, ws, "propose", NAME, "--ws", ws, "--iter", "1")  # wiki is kept whatever the proposer does
        assert_calls_ok(env, start)
    out = {"wiki": copy_wiki(ws, os.getcwd()), "model_calls": calls_in(env, ws), "iterations": 1}

    if not os.path.exists(ppath):
        out["no_action_reason"] = "proposer wrote no proposal file"
        return out
    with open(ppath) as f:
        prop = json.load(f)
    action = prop.get("action")
    if action != "create":
        out["no_action_reason"] = ("proposer chose no_action" if action == "no_action" else
                                   f"proposer returned `{action}` but the active skill set is empty")
        return out
    fm, body = split_front_matter(prop.get("skill_md", ""))
    name = slug(prop.get("name") or fm.get("name") or "skill")[:48]
    out["candidate"] = {"name": name, "description": (fm.get("description") or "WikiSkill-proposed skill")[:500],
                        "body": body.strip() + "\n"}
    return out


def solve(req, env):
    t, skill = req["task"], req.get("skill")
    label = "cand-" + sha(skill["name"].encode())[7:15] if skill else "base"
    start = len(ledger_rows(env))
    start_calls = ledger_calls(env)
    ws = make_ws(env, "solve-" + label, [task_spec(t["id"], "val", t["prompt"])])
    if skill:
        d = os.path.join(ws, "skills", "active", slug(skill["name"]))
        os.makedirs(d, exist_ok=True)
        with open(os.path.join(d, "SKILL.md"), "w") as f:
            f.write(skill["body"])
    tid = slug(t["id"])
    cli(env, ws, "run-task", NAME, tid, "--ws", ws, "--iter", "1", "--max-turns",
        env.get("WIKISKILL_MAX_TURNS", "8"), check=False)
    assert_calls_ok(env, start)
    ans = os.path.join(ws, "bench", "tasks", tid, "answer.txt")
    answer = open(ans).read().strip() if os.path.exists(ans) else ""
    loaded = False
    sess = os.path.join(ws, "runs", "iter-01", "val", tid, "session.jsonl")
    if skill and os.path.exists(sess):
        with open(sess, errors="replace") as f:
            txt = f.read()
        loaded = '"name": "Skill"' in txt and slug(skill["name"]) in txt
    log(f"solve {t['id']} skill={'yes' if skill else 'no'} loaded_via_Skill_tool={loaded}")
    return {"answer": answer[: 60 * 1024], "model_calls": max(ledger_calls(env) - start_calls, 1)}


def main():
    try:
        req = json.load(sys.stdin)
    except ValueError:
        print(json.dumps({"unavailable": "request is not JSON"}))
        return 69
    env = dict(os.environ)
    try:
        preflight(env)
        out = evolve(req, env) if "train_tasks" in req else solve(req, env)
    except Unavailable as e:
        print(json.dumps({"unavailable": str(e)}))
        return 69
    except Failed as e:
        log(f"failed: {e}")
        print(json.dumps({"error": str(e)}))
        return 1
    print(json.dumps(out, sort_keys=True))
    return 0


if __name__ == "__main__":
    sys.exit(main())
