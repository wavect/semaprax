"""Strict newest-source custody preparation; no compiler/runtime execution."""
from pathlib import Path
import sys,json,subprocess,hashlib,gzip,shutil
R=Path.cwd();D=Path(__file__).resolve().parent;OLD='93650ecbef340264fb09e3be5649cd7962c682ea';MAIN='7f9080f85316bf0e2e8f530cf4c5adbcda247c16';BASIS='d5dd5e63d';sha=lambda b:hashlib.sha256(b).hexdigest()
def git(*a):return subprocess.check_output(['git',*a],cwd=R)
def put(n,x):(D/n).write_text(json.dumps(x,indent=2)+'\n')
def row(p,new,old,rev):return {'source':p,'checkpoint_revision':rev,'current_sha256':sha(new),'original_sha256':sha(old),'edits':[{'new':new.decode(),'old':old.decode()}]}
paths=git('diff','--name-only',OLD,'--','.github','src','tests','crates','docs/ARCHITECTURE.md','docs/DECLARATION-IDENTITY-BACKING-V1.md','docs/AGENT-SKILL-BUNDLE-V1.json','CHANGELOG.md').decode().splitlines();rows=[];added=[]
for p in paths:
 raw=(R/p).read_bytes();before=subprocess.run(['git','show',OLD+':'+p],cwd=R,capture_output=True)
 if before.returncode:added.append({'source':p,'sha256':sha(raw),'new_owner':True});continue
 if raw!=before.stdout:rows.append(row(p,raw,before.stdout,OLD))
put('current-to-9365.json',rows);put('new-owners.json',added)
records=json.loads((R/'src/source_reflows.json').read_text());assert len(records)==87;assert (R/'src/source_reflows.json').read_bytes()==git('show',MAIN+':src/source_reflows.json');main=[]
for r in records:
 p=r['path'];raw=(R/p).read_bytes();target=git('show',MAIN+':'+p);assert sha(target)==r['after_sha256'],p
 if raw!=target:main.append(row(p,raw,target,MAIN))
put('current-to-main7f.json',main)
full=[]
for p in git('diff','--name-only',BASIS).decode().splitlines():full.append(row(p,(R/p).read_bytes(),git('show',BASIS+':'+p),git('rev-parse',BASIS).decode().strip()))
put('immediate-inverses.json',full)
external=Path('/Volumes/Semaprax/provenance/main7f9080-merge-20261015');inputs=json.loads((external/'complete-inputs.json').read_text());assert inputs['paths']==25 and len(inputs['inputs'])==75
for r in inputs['inputs']:
 if r.get('absent'):
  assert subprocess.run(['git','cat-file','-e',r['ref']+':'+r['path']],cwd=R,stderr=subprocess.DEVNULL).returncode!=0
  continue
 raw=(external/r['archive']).read_bytes();plain=gzip.decompress(raw);assert sha(plain)==r['sha256'];assert plain==git('show',r['ref']+':'+r['path']);dest=D/r['archive'];dest.parent.mkdir(parents=True,exist_ok=True);dest.write_bytes(raw)
put('complete-inputs.json',inputs)
put('successor.json',{'production_basis':git('rev-parse',BASIS).decode().strip(),'prior_source_domain':OLD,'latest_main_domain':MAIN,'new_source_rows':len(rows),'new_files':len(added),'current_main_records':87,'latest_main_projection_rows':len(main),'assembly_edits':[r['source'] for r in full],'production_edited':False,'source_only':True})
print('prepared',len(rows),'strict9365 rows,',len(added),'newowners,',len(main),'latestmain87 deltas,',len(full),'assembly helpers')
