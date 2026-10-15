"""Finite assembly data preparation; no runtime/compiler execution."""
from pathlib import Path
import hashlib,json,subprocess,sys,gzip
R=Path.cwd();D=Path(__file__).resolve().parent;BASE='7800b014151b6ce0709c351ccdfd52ae25832e07';MERGED='b1e3cad9100494e81ac7e5f6ae2695eb0ee586da'
BRANCHES={'9d':'9d4358078f9e2ae7740683c06bbf939284038058','842':'8025cd196fd15629cdfb6646fe281fb74f606a04','843':'538673710c463b911c2e5a38d8f7fc9275f99c62'}
sha=lambda b:hashlib.sha256(b).hexdigest()
def git(*a):return subprocess.check_output(['git',*a],cwd=R)
def put(n,x):(D/n).write_text(json.dumps(x,indent=2)+'\n')
paths=set(git('diff','--name-only',BASE,MERGED,'--','src','tests','crates','docs/ARCHITECTURE.md').decode().splitlines());paths.update(git('diff','--name-only',MERGED).decode().splitlines());assert all(not p.startswith('docs/evidence/') for p in paths)
for stage,rev in BRANCHES.items():
 rows=[];equal=[];missing=[]
 for p in sorted(paths):
  current=(R/p).read_bytes();has=subprocess.run(['git','cat-file','-e',rev+':'+p],cwd=R,stderr=subprocess.DEVNULL).returncode==0
  if not has:missing.append({'source':p,'current_sha256':sha(current),'target_absent':True});continue
  old=git('show',rev+':'+p)
  if old==current:equal.append({'source':p,'sha256':sha(current)});continue
  rows.append({'source':p,'checkpoint_revision':rev,'current_sha256':sha(current),'original_sha256':sha(old),'edits':[{'new':current.decode(),'old':old.decode()}]})
 put('view-'+stage+'.json',rows);put('view-'+stage+'-inventory.json',{'revision':rev,'equal':equal,'absent':missing,'changed':len(rows)})
rows=[]
for p in git('diff','--name-only',MERGED).decode().splitlines():
 current=(R/p).read_bytes();old=git('show',MERGED+':'+p);rows.append({'source':p,'checkpoint_revision':git('rev-parse',MERGED).decode().strip(),'current_sha256':sha(current),'original_sha256':sha(old),'edits':[{'new':current.decode(),'old':old.decode()}]})
put('immediate-merged-inverse.json',rows)
patch=git('diff','--full-index','--binary',MERGED,'--',*[r['source'] for r in rows]);(D/'immediate-merged-inverse.patch.gz').write_bytes(gzip.compress(patch,mtime=0))
# The later minimal sticky correction is a fourth ordinary source-proof domain,
# without changing the three original runtime projectors or their input tables.
sticky_revision='7d59963a1b841064e6296c138c01810df2b4d587'
sticky=[]
for r in json.loads((R/'docs/evidence/held-closure-receiver-843-sticky-fix/source-inverses.json').read_text()):
 p=r['path'];cur=(R/p).read_bytes();old=git('show',sticky_revision+':'+p)
 sticky.append({'source':p,'checkpoint_revision':sticky_revision,'current_sha256':sha(cur),'original_sha256':sha(old),'edits':[{'new':cur.decode(),'old':old.decode()}]})
put('sticky-source-view.json',sticky)
put('branches.json',{'common_predecessor':BASE,'merged_input':git('rev-parse',MERGED).decode().strip(),'branches':BRANCHES,'source_paths':sorted(paths),'original_production_unchanged_except_source_custody_helpers':[r['source'] for r in rows]})
print('prepared',len(paths),'source paths; metadata helpers',len(rows))
