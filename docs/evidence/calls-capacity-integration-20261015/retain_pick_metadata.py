"""Retain both complete metadata histories before composing against ROOT source."""
from pathlib import Path
import subprocess,json,hashlib,sys
r=Path.cwd();p=r/'docs/evidence/calls-capacity-integration-20261015';sha=lambda x:hashlib.sha256(x).hexdigest()
rev=subprocess.check_output(['git','rev-parse',sys.argv[1]],text=True).strip();root=subprocess.check_output(['git','rev-parse','HEAD'],text=True).strip();base=subprocess.check_output(['git','rev-parse',rev+'^'],text=True).strip()
def blob(revision,name):
 x=subprocess.run(['git','show',revision+':'+name],stdout=subprocess.PIPE,stderr=subprocess.DEVNULL);return x.stdout if x.returncode==0 else None
rows=[]
for name in subprocess.check_output(['git','diff-tree','--no-commit-id','--name-only','-r',rev],text=True).splitlines():
 if not name.startswith('docs/evidence/') or not name.endswith('.json'):continue
 incoming=blob(rev,name);ours=blob(root,name);prior=blob(base,name)
 if ours is None or incoming is None:continue
 row={'path':name,'incoming_revision':rev,'root_revision':root,'base_revision':base}
 for side,data in [('base',prior),('root',ours),('incoming',incoming)]:
  if data is None:continue
  dest=p/'conflict-inputs'/rev/side/name;dest.parent.mkdir(parents=True,exist_ok=True);dest.write_bytes(data);row[side+'_sha256']=sha(data)
 # Keep the ROOT linear history as the active branch. Its original hooks are
 # extended only by later authenticated complete preinverses. The complete
 # incoming parallel history is independently checked from immutable source.
 (r/name).write_bytes(ours);subprocess.run(['git','add',name],check=True);rows.append(row)
(p/('retained-'+rev+'.json')).write_text(json.dumps(rows,indent=2)+'\n')
print(json.dumps({'incoming':rev,'root':root,'full_metadata_branches_retained':len(rows)}))
