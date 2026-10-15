"""Finite corrective source joins. No Rust/Cargo/runtime execution."""
from pathlib import Path
import json,subprocess,hashlib
R=Path.cwd();D=Path(__file__).resolve().parent;OLD='d1bb8d4149e9ec96968ffbd8ec26a86d23ea9ae2';BASIS='64222b83381c365c9e4af1e858640b8cf696bfe7';sha=lambda b:hashlib.sha256(b).hexdigest()
def git(*a):return subprocess.check_output(['git',*a],cwd=R)
def put(n,x):(D/n).write_text(json.dumps(x,indent=2)+'\n')
def row(p,cur,old,stage):return {'source':p,'checkpoint_revision':stage,'current_sha256':sha(cur),'original_sha256':sha(old),'edits':[{'new':cur.decode(),'old':old.decode()}]}
paths=git('diff','--name-only',OLD,'--','src','tests','crates').decode().splitlines();rows=[row(p,(R/p).read_bytes(),git('show',OLD+':'+p),OLD) for p in paths];assert len(rows)==5;put('current-to-d1bb.json',rows)
scalar=[]
for r in json.loads((R/'docs/evidence/scalar-source-guard-adapter-1dee-20261015/source-projections.json').read_text()):
 p=r['source'];cur=(R/p).read_bytes();assert len(r['edits'])==1;target=r['edits'][0]['new'].encode();assert sha(target)==r['current_sha256']
 if cur!=target:
  entry=row(p,cur,target,'named scalar input');del entry['checkpoint_revision'];entry['target_packet']='docs/evidence/scalar-source-guard-adapter-1dee-20261015/source-projections.json';entry['target_complete_input_field']='edits[0].new';scalar.append(entry)
assert len(scalar)==2;put('scalar-policy-inputs.json',scalar)
warning=[]
for r in json.loads((R/'docs/evidence/d1bb-warning-corrections-20261015/source-inverses.json').read_text()):
 p=r.get('source',r.get('path'));cur=(R/p).read_bytes();old=git('show','ef85e74802c444c8cc1365fa84db57750af5673f:'+p);warning.append(row(p,cur,old,'ef85e74802c444c8cc1365fa84db57750af5673f'))
put('warning-source-view.json',warning)
put('immediate-inverses.json',[row(p,(R/p).read_bytes(),git('show',BASIS+':'+p),BASIS) for p in git('diff','--name-only',BASIS).decode().splitlines()])
put('successor.json',{'production_basis':BASIS,'prior_domain':OLD,'corrective_source_owners':paths,'assembly_files':git('diff','--name-only',BASIS).decode().splitlines(),'new_selectors':0,'pending_parent_bodies':310,'source_only':True})
print('prepared',len(rows),'d1bb inputs,',len(scalar),'scalar input domains; no old data rewritten')
