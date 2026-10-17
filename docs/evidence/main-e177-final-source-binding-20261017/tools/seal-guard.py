from pathlib import Path
import json,hashlib,subprocess,ast
R=Path('/Volumes/Semaprax/worktrees/main-e177-source-integration-20261017');AFA='afa1e4d9da88da6420776ae3184cc9efb552d022';REL='docs/evidence/main-e177-final-source-binding-20261017';D=R/REL;EXT=Path(__file__).parent;H=lambda b:hashlib.sha256(b).hexdigest();G=lambda p:subprocess.check_output(['git','show',AFA+':'+p],cwd=R)
def pin(path,b):return dict(path=path,length=len(b),sha256=H(b))
def archive(path,b):
 p=REL+'/guards/'+path.replace('/','__')+'.before.txt';(R/p).parent.mkdir(parents=True,exist_ok=True);(R/p).write_bytes(b);return dict(**pin(path,b),archive=p)
prior='docs/evidence/next-big-batch-source-binding-20261017/guard_payload.py';old=G(prior);tree=ast.parse(old);oldconfig=next(ast.literal_eval(n.value)for n in tree.body if isinstance(n,ast.Assign)and any(isinstance(t,ast.Name)and t.id=='CONFIG'for t in n.targets))
inv=json.loads((D/'hook-and-raw-inverses.json').read_bytes());packetpath=REL+'/current-inputs.json';packetraw=(R/packetpath).read_bytes();packet=json.loads(packetraw);assert len(packetraw)<=262144
public='docs/evidence/main043-independent-custody-20261016/verify-or-refresh.py';publicraw=G(public);guardpath=REL+'/guard_payload.py';oldsha=H(old)
publicedits=[dict(before=prior,after=guardpath,count=1),dict(before='guard_raw=held(guard_helper,'+str(len(old))+')',after='guard_raw=held(guard_helper,@GUARD_LENGTH@)',count=1),dict(before=oldsha,after='@GUARD_SHA256@',count=1)]
modules={p:pin(p,(R/p).read_bytes())for p in inv['new_modules']};carriers={}
for p in inv['raw_carriers']:
 base='../'if p=='tests/hir_module_boundaries.rs'else('../../'if p.startswith('tests/')else'../../../../../');carriers[p]=dict(**pin(p,(R/p).read_bytes()),base=base)
t=inv['ordered_table'];before=(D/'prior426.whole.tsv.txt').read_bytes();priorarch=REL+'/prior426.whole.tsv.txt';table=dict(**pin(t['path'],(R/t['path']).read_bytes()),prior=dict(length=len(before),sha256=H(before),archive=priorarch),insertions=t['insertions'],rows=t['rows'])
config=dict(packet=pin(packetpath,packetraw),rows=len(packet['rows']),guard=archive(prior,old),this_guard=guardpath,public=archive(public,publicraw),public_edits=publicedits,modules=modules,carriers=carriers,prior_carriers=oldconfig['carriers'],workspace_owner=pin(oldconfig['workspace_owner'],G(oldconfig['workspace_owner'])),table=table,closed_prior_sources=sorted(set(oldconfig['parent_sources']['root'])|set(oldconfig['parent_sources']['future'])|set(oldconfig['required_modules'])|set(carriers)))
s=(EXT/'guard-stage.py.in').read_text();assert s.count('@CONFIG@')==1;s=s.replace('@CONFIG@',repr(config));ast.parse(s);raw=s.encode();assert len(raw)<=65536;(R/guardpath).write_bytes(raw)
inverses=[]
for kind,path in [('public',public),('strict','docs/evidence/accepted-strict-guard-generation-20261017/check-source.py'),('final6','docs/evidence/accepted-final6-source-binding-20261017/check-source.py')]:
 b=G(path);code=b.decode()
 if kind=='public':edits=publicedits
 else:edits=[dict(before=prior,after=guardpath,count=1),dict(before=str(len(old))+",'"+oldsha+"'",after="@GUARD_LENGTH@,'@GUARD_SHA256@'",count=1)]
 steps=[]
 for e in edits:
  z=e['after'].replace('@GUARD_LENGTH@',str(len(raw))).replace('@GUARD_SHA256@',H(raw));assert code.count(e['before'])==e['count'],(path,e['before']);code=code.replace(e['before'],z);steps.append(dict(before=e['before'],after=z,count=e['count']))
 restored=code
 for e in reversed(steps):assert restored.count(e['after'])==e['count'];restored=restored.replace(e['after'],e['before'])
 assert restored.encode()==b;ast.parse(code);a=code.encode();(R/path).write_bytes(a);inverses.append(dict(source=path,before=archive(path,b),after=pin(path,a),edits=steps))
(D/'strict-stage-receipt.json').write_text(json.dumps(dict(status='SOURCE_ONLY_PENDING_CONTROLS',guard=pin(guardpath,raw),source_packet=pin(packetpath,packetraw),modules=len(modules),table_rows=table['rows'],whole_afa_guard=pin(prior,old),guard_acquisition_substitutions=4,bootstrap_whole_inverses=inverses,strict_cap_unchanged=262144),indent=2)+'\n');print(json.dumps({'guard_bytes':len(raw),'guard_sha256':H(raw),'modules':len(modules),'table_rows':table['rows']}))
