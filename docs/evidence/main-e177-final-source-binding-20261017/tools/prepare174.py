"""Prepare only; retain all174 subjects and the unchanged original binder."""
from pathlib import Path
import json,ast,hashlib,subprocess
R=Path('/Volumes/Semaprax/worktrees/main-e177-source-integration-20261017');REL='docs/evidence/main-e177-final-source-binding-20261017';N=REL+'/frozen174/';D=R/REL;H=lambda b:hashlib.sha256(b).hexdigest()
def keep(p,b):q=R/p;q.parent.mkdir(parents=True,exist_ok=True);q.write_bytes(b)
def assignments(s,values):
 nodes={n.targets[0].id:n.value for n in ast.parse(s).body if isinstance(n,ast.Assign)and len(n.targets)==1 and isinstance(n.targets[0],ast.Name)};lines=s.splitlines(True);edits=[]
 for k,v in values.items():n=nodes[k];a=sum(map(len,lines[:n.lineno-1]))+n.col_offset;b=sum(map(len,lines[:n.end_lineno-1]))+n.end_col_offset;edits.append((a,b,repr(v)))
 for a,b,v in sorted(edits,reverse=True):s=s[:a]+v+s[b:]
 return s
bd=json.loads((R/'docs/evidence/main09013-source-namespaces-v1/binding-descriptor.json').read_bytes());manifest=(R/bd['manifest']).read_bytes();v=json.loads(manifest);assert len(v['physical'])==len({x['path']for x in v['physical']})==174
changed={}
for row in v['physical']:
 raw=(R/row['path']).read_bytes();pair=(len(raw),H(raw))
 if pair!=(row['current_len'],row['current_sha256']):changed[row['path']]=pair
old='docs/evidence/next-big-batch-source-binding-20261017/frozen174/bind-next.py';before=(R/old).read_bytes();s=before.decode();names=('LEGACY','STRICT');values={n.targets[0].id:ast.literal_eval(n.value)for n in ast.parse(s).body if isinstance(n,ast.Assign)and len(n.targets)==1 and isinstance(n.targets[0],ast.Name)and n.targets[0].id in names};oldstrict=dict(values['STRICT'])
for p,row in values['LEGACY'].items():b=(R/p).read_bytes();row['actual']=(len(b),H(b))
helper=REL+'/guard_payload.py';cfg=next(ast.literal_eval(n.value)for n in ast.parse((R/helper).read_bytes()).body if isinstance(n,ast.Assign)and isinstance(n.targets[0],ast.Name)and n.targets[0].id=='CONFIG');packet=json.loads((R/cfg['packet']['path']).read_bytes());needed={helper,cfg['packet']['path'],cfg['guard']['path'],cfg['guard']['archive'],cfg['public']['archive'],cfg['table']['path'],cfg['table']['prior']['archive']}|set(cfg['modules'])
for source in cfg['closed_prior_sources']:
 needed.add(source);row=next((r for r in packet['rows']if r['source']==source),None)
 if row:needed.add(row['current']['archive']);needed.update(x['archive']for x in row['views'])
strict=values['STRICT']
for p in needed:strict.setdefault(p,None)
for p in strict:
 raw=(R/p).read_bytes();pair=(len(raw),H(raw))
 if len(raw)>262144:assert p in oldstrict and tuple(oldstrict[p])==pair and p=='docs/evidence/workspace21-legacy-source-repair-20261017/whole-source-inverses.json'and pair==(317943,'86f53656e6765ddc606675d1356d0fc91416c878b2d607373a4aed7bbac04b04'),p
 strict[p]=pair
s=assignments(s,dict(REL=N,BEFORE_LEN=len(manifest),BEFORE_SHA=H(manifest),CHANGED=changed,LEGACY=values['LEGACY'],STRICT=strict,STRICT_HELPER=helper));ast.parse(s)
for name,path in [('input-domains.before.json',bd['manifest']),('incoming090_semantic_receivers.before.rs.txt',bd['module']),('bind_current.before.py.txt',bd['binder']),('bind-next.before.py.txt',old)]:keep(N+name,(R/path).read_bytes())
keep(N+'bind-next.py',s.encode());keep(REL+'/frozen174-preparation.json',(json.dumps(dict(status='PREPARED_NOT_EXECUTED',membership=174,changed=changed,strict_dependencies=len(strict),strict_cap=262144,only_inherited317943_exception_exact=True,binder_unchanged_sha256=H((R/bd['binder']).read_bytes()),whole_wrapper_before_sha256=H(before),wrapper_after_sha256=H(s.encode()),explicit_execution_expected_repo_and_HEAD_required=True),indent=2)+'\n').encode());print('PREPARED_NOT_EXECUTED',len(changed),len(strict))
