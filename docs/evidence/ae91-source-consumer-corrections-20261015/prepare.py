"""Finite actual ae91 consumer domains; no Rust execution."""
from pathlib import Path
import json,hashlib,subprocess,re,ast
R=Path.cwd();D=Path(__file__).resolve().parent;AE='ae91fa963ecfe85f4cc830cda85fff2f5bdbe094';OLD='3d4b996c9a8cd2b100a3c594e741314c108b5591';sha=lambda b:hashlib.sha256(b).hexdigest()
def git(*a):return subprocess.check_output(['git',*a])
def put(n,x):(D/n).write_text(json.dumps(x,indent=2)+'\n')
def row(p,new,old):return {'source':p,'current_sha256':sha(new),'original_sha256':sha(old),'edits':[{'new':new.decode(),'old':old.decode()}]}
for domain,p in [('registry833','src/workspace_graph/expected_projection/private_layout/registry/scratch_tests.rs'),('cold835','src/workspace_graph/cold_canonical/tests.rs'),('ordered838','src/cleanup_plan/replay/factored/canonical_shared/ordered_weights/tests.rs')]:
 target=git('show',OLD+':'+p);new=(R/p).read_bytes();put(domain+('-observer.json' if domain=='ordered838' else '-children.json'),[row(p,new,target)])
# Evaluate only the exact root concat macro's literal include/string tokens.
ns={};exec((R/'docs/evidence/source-consumer-closure-fixes-20261015/module-parser-original.py.txt').read_text(),ns);mask=ns['mask']
def rootbody(s):
 m=mask(s);z=re.search(r'macro_rules!\s+semantic_graph_projection_root_source_inventory\s*\{',m);a=z.end();b=a;d=1
 while d:d+=(m[b]=='{')-(m[b]=='}');b+=1
 return s[a:b-1]
def join(revision):
 inv='tests/workspace/semantic_graph/source_inventory.rs';s=git('show',revision+':'+inv).decode() if revision else (R/inv).read_text();body=rootbody(s);out=[]
 for x in re.finditer(r'include_str!\s*\(\s*("(?:\\.|[^"\\])*")\s*\)|("(?:\\.|[^"\\])*")',body):
  if x[1]:p=ast.literal_eval(x[1]);assert p.startswith('../../');p=p[6:];out.append(git('show',revision+':'+p) if revision else (R/p).read_bytes())
  else:out.append(ast.literal_eval(x[2]).encode())
 return b''.join(out)
put('facade.json',row('expected_projection::root_join',join(None),join(OLD)))
paths=git('diff','--name-only',AE,'--','src','tests','crates').decode().splitlines();prior=[]
for p in paths:prior.append(row(p,(R/p).read_bytes(),git('show',AE+':'+p)))
put('current-to-ae91.json',prior);put('basis.json',{'frozen_prior':AE,'historical_children':OLD,'source_only':True,'immediate_source_owners':paths,'new_test_bodies':0,'runtime_bodies_executed':0})
print('prepared complete inputs',len(prior))
