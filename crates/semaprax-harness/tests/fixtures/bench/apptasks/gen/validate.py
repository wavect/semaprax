import sys, os, json, shutil, subprocess, tempfile
root = sys.argv[1]
PY = "/Users/kevin/.local/bin/python3"; NODE = "/Users/kevin/.nvm/versions/node/v24.3.0/bin/node"; CC = "/private/tmp/claude-501/hp-tools/semaprax-compiler"
def cp(src, dst):
    if not os.path.isdir(src): return
    for dp, _, fs in os.walk(src):
        for f in fs:
            s = os.path.join(dp, f); r = os.path.relpath(s, src); d = os.path.join(dst, r)
            os.makedirs(os.path.dirname(d), exist_ok=True); shutil.copy(s, d)
def grade(sb, step):
    for g in step["grade"]:
        cmd = [c.replace("{python}", PY).replace("{node}", NODE).replace("{compiler}", CC) for c in g["cmd"]]
        p = subprocess.run(cmd, cwd=sb, capture_output=True, text=True, timeout=120)
        if p.returncode != 0 or ("expect_stdout" in g and g["expect_stdout"] not in p.stdout):
            return False, (p.stdout + p.stderr)[-600:]
    return True, ""
bad = 0
for t in sorted(os.listdir(root + "/tasks")):
    d = f"{root}/tasks/{t}"; meta = json.load(open(d + "/task.json"))
    sb = tempfile.mkdtemp(); cp(d + "/project", sb)
    for i, st in enumerate(meta["steps"]):
        for j in range(i + 1): cp(f"{d}/{meta['steps'][j]['id']}/grader", sb)
        ok, out = grade(sb, st)
        print(t, st["id"], "pristine-fails" if not ok else "PRISTINE PASSES(bad)")
        if ok: bad += 1
        else: 
            if "-v" in sys.argv: print("   ", out.replace("\n", "\n    ")[-300:])
        cp(f"{d}/{st['id']}/reference", sb)
        for j in range(i + 1): cp(f"{d}/{meta['steps'][j]['id']}/grader", sb)
        ok, out = grade(sb, st)
        print(t, st["id"], "reference-passes" if ok else "REFERENCE FAILS(bad)")
        if not ok: bad += 1; print(out)
    shutil.rmtree(sb)
print("BAD", bad)
