"""Strict finite ff869 merge source domains; no Rust execution."""
from pathlib import Path
import subprocess,json,hashlib,copy
R=Path.cwd();D=Path(__file__).resolve().parent;AE='ae91fa963ecfe85f4cc830cda85fff2f5bdbe094';FF='ff869dd96ef97f179078c08fca098398be2d8e8e';MERGE='72afbfc55';sha=lambda b:hashlib.sha256(b).hexdigest()
def git(*a):return subprocess.check_output(['git',*a])
def old(rev,p):
 x=subprocess.run(['git','show',rev+':'+p],capture_output=True);return x.stdout if x.returncode==0 else None
def put(n,x):(D/n).write_text(json.dumps(x,indent=2)+'\n')
def row(p,new,before,rev):return {'source':p,'checkpoint_revision':rev,'current_sha256':sha(new),'original_sha256':sha(before),'edits':[{'new':new.decode(),'old':before.decode()}]}
# These source inputs include the complete unchanged helper predecessor itself.
paths=git('diff','--name-only',AE,'--','src','tests','crates','CHANGELOG.md','docs/AGENT-SKILL-BUNDLE-V1.json','docs/DECLARATION-IDENTITY-BACKING-V1.md').decode().splitlines();prior=[];added=[]
for p in paths:
 new=(R/p).read_bytes();before=old(AE,p)
 if before is None:added.append({'source':p,'sha256':sha(new)});continue
 if new!=before:prior.append(row(p,new,before,AE))
put('current-to-ae91.json',prior);put('new-source-owners.json',added)
ci=json.loads((R/'src/source_reflows.json').read_text());assert len(ci)==87;live=copy.deepcopy(ci);views=[];changes=[]
for r in live:
 p=r['path'];target=old(FF,p);raw=(R/p).read_bytes();assert target is not None
 # Old complete after input comes from the exact old main baseline, not inferred text.
 historical=old('7f9080f85316bf0e2e8f530cf4c5adbcda247c16',p);assert sha(historical)==r['after_sha256'],p
 if target!=historical:
  r['inverse'].insert(0,{'after':target.decode(),'before':historical.decode()});r['after_sha256']=sha(target);changes.append(p)
 if raw!=target:views.append(row(p,raw,target,FF))
doc_owners=['src/hir/borrowed_scalar/option_u8.rs','src/hir/owned_leaf_collection.rs','src/hir/static_text/backing.rs','src/hir/validation/generic_template/call_graph/ordered.rs','src/workspace_graph/identity_resolution.rs']
for p in doc_owners:
 assert not any(r['path']==p for r in live)
 target=old(FF,p);before=old(AE,p);assert target!=before
 # All five main changes are exactly documentation backticks; full bytes bind both sides.
 assert target.decode().replace('`','')==before.decode().replace('`',''),p
 live.append({'path':p,'after_sha256':sha(target),'before_sha256':sha(before),'inverse':[{'after':target.decode(),'before':before.decode()}]})
 raw=(R/p).read_bytes()
 if raw!=target:views.append(row(p,raw,target,FF))
 changes.append(p)
put('live-mainff-ci92.json',live);put('current-to-mainff.json',views)
assembly=[]
for p in git('diff','--name-only',MERGE,'--','src','tests','crates').decode().splitlines():assembly.append(row(p,(R/p).read_bytes(),old(MERGE,p),git('rev-parse',MERGE).decode().strip()))
put('assembly-inverses.json',assembly)
put('successor.json',{'frozen_prior_domain':AE,'actual_main_baseline':FF,'isolated_merge':git('rev-parse',MERGE).decode().strip(),'strict_prior_rows':len(prior),'new_sources':added,'active_ci_records':len(live),'ci_changed_main_inputs':changes,'main_ff_projection_rows':len(views),'assembly_sources':[r['source'] for r in assembly],'source_only':True,'old_runtime_json_edited':False,'runtime_bodies_executed':0})
print('prepared',len(prior),'strictae91 inputs;',len(views),'actualff views;',len(changes),'CI doc deltas;',len(assembly),'assembly sources')
