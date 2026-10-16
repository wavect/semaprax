from pathlib import Path
import hashlib,json,subprocess
D=Path(__file__).resolve().parent;R=D.parents[2];B="17b12c909916ce8671b43f208b59f3432144709a"
sha=lambda b:hashlib.sha256(b).hexdigest()
for row in json.loads((D/"source-inverses.json").read_text()):
 b=(R/row["source"]).read_bytes();assert sha(b)==row["current_sha256"];s=b.decode()
 for e in row["edits"]:assert s.count(e["new"])==1;s=s.replace(e["new"],e["old"],1)
 assert sha(s.encode())==row["original_sha256"]
 assert s.encode()==(R/row["archive"]).read_bytes()==subprocess.check_output(["git","show",B+":"+row["source"]],cwd=R)
for f in ["src/cleanup_plan/replay/branch_clone/before.rs","src/cleanup_plan/replay/final_output/before_transfer_current.rs","src/cleanup_plan/replay/compact_stage/before.rs","src/cleanup_plan/replay/compact_stage/tests.rs"]:
 assert (R/f).read_bytes()==subprocess.check_output(["git","show",B+":"+f],cwd=R)
print("PASS2 full inverses; complete Main120 and historical receiver/test bodies unchanged; no Rust execution")
