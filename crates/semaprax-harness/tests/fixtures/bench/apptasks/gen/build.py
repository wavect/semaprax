import sys, os, json, shutil
sys.path.insert(0, os.path.dirname(__file__))
sys.path.insert(0, os.path.dirname(__file__) + "/..")
import part1, part2, part3, part4
for n in ("t_feature_py","t_refactor_py","t_feature_js"): globals()[n] = getattr(part1, n)
part2_names = ("t_spx_repair","t_js_repair","t_failing_py")
part3.EMPTY = ""
import types
mods = {}
for m in (part1, part2, part3, part4):
    for k, v in vars(m).items():
        if k.startswith("t_"): mods[k] = v
order = ["t_feature_py","t_refactor_py","t_feature_js","t_spx_repair","t_js_repair","t_failing_py","t_failing_js","t_mixed_invoice","t_mixed_units","t_reuse_py","t_reuse_js","t_maintenance"]
out = sys.argv[1]
if os.path.exists(out): shutil.rmtree(out)
def w(p, text):
    os.makedirs(os.path.dirname(p), exist_ok=True)
    open(p, "w").write(text)
ids = []
for name in order:
    t = mods[name]()
    d = f"{out}/tasks/{t['id']}"
    ids.append(t["id"])
    for p, c in t["project"].items(): w(f"{d}/project/{p}", c)
    meta = dict(id=t["id"], **{"class": t["cls"]}, languages=t["langs"], query=t["query"], steps=[])
    for s in t["steps"]:
        for p, c in s["hidden"].items(): w(f"{d}/{s['id']}/grader/{p}", c)
        for p, c in s["ref"].items(): w(f"{d}/{s['id']}/reference/{p}", c)
        st = {k: v for k, v in s.items() if k not in ("hidden", "ref")}
        meta["steps"].append(st)
    w(f"{d}/task.json", json.dumps(meta, indent=2, sort_keys=True) + "\n")
print("\n".join(ids))
