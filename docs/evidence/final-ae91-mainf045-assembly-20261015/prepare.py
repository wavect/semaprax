"""Finite named branch views before unchanged strict historical source hooks."""
from pathlib import Path
import subprocess,json,hashlib
R=Path.cwd();D=Path(__file__).resolve().parent;FIX='9f0b2a5302a6a0ebce5b5f3e856c090e6033c48e';MAIN='068a2ae54d74f74f8e0960ec0343d54ae05e4420';AE='ae91fa963ecfe85f4cc830cda85fff2f5bdbe094';sha=lambda b:hashlib.sha256(b).hexdigest()
def git(*a):return subprocess.check_output(['git',*a])
def blob(rev,p):
 x=subprocess.run(['git','show',rev+':'+p],capture_output=True);return x.stdout if x.returncode==0 else None
def put(n,x):(D/n).write_text(json.dumps(x,indent=2)+'\n')
def load(p):return json.loads((R/p).read_text())
def row(p,new,old):return {'source':p,'current_sha256':sha(new),'original_sha256':sha(old),'edits':[{'new':new.decode(),'old':old.decode()}]}
def apply(rows,p,b):
 rs=[r for r in rows if r['source']==p];assert len(rs)<=1
 if not rs:return b
 r=rs[0];assert sha(b)==r['current_sha256'],p
 for e in r['edits']:
  a=e['new'].encode();assert b.count(a)==1;b=b.replace(a,e['old'].encode(),1)
 assert sha(b)==r['original_sha256'];return b
paths=git('diff','--name-only',AE,'--','src','tests','crates','CHANGELOG.md','docs/ARCHITECTURE.md','docs/AGENT-SKILL-BUNDLE-V1.json','docs/DECLARATION-IDENTITY-BACKING-V1.md').decode().splitlines();views={};reconciles={}
for stage,rev in [('fixes',FIX),('main',MAIN)]:
 rows=[];extra=[]
 for p in paths:
  raw=(R/p).read_bytes();target=blob(rev,p)
  if target is None:continue
  if target!=raw:rows.append(row(p,raw,target))
  if stage=='fixes':b=apply(load('docs/evidence/ae91-source-consumer-corrections-20261015/current-to-ae91.json'),p,target)
  else:
   b=apply(load('docs/evidence/mainf045-source-successor-20261015/current-to-0bf.json'),p,target);b=apply(load('docs/evidence/mainff869-source-successor-20261015/current-to-ae91.json'),p,b)
  ae=blob(AE,p)
  if ae is not None and ae!=b:extra.append(row(p,b,ae))
 put('view-'+stage+'.json',rows);put(stage+'-ae91-reconcile.json',extra);views[stage]=len(rows);reconciles[stage]=[r['source'] for r in extra]
put('basis.json',{'fixes_branch':FIX,'main_branch':MAIN,'common_ae91':AE,'actual_latest_main':'f0455caf96706a0545e754957fcc903434a7f880','source_input_paths':paths,'view_counts':views,'extra_independent_receipt_domains':reconciles,'production_edits_by_assembly':0,'new_test_bodies_by_assembly':0,'source_only':True});print('views',views,'independent remaining receipt domains',reconciles)
