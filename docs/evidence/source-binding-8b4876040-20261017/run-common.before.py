#!/usr/bin/env python3
"""Finite source composition at named acquisition sites; original tools unchanged."""
from pathlib import Path
import ast,hashlib,importlib.util,json,sys
E=Path(__file__).resolve().parent
HELPER_SHA='966cf0bbea108acaf8a63cfaa7ab6b62632e4355f6d37949d40774d019ce458b'
TOOLS={'main': ('docs/evidence/main67107-fronts-6e45-20261017/bind-main-fronts.py', '7fe43d4cf71a2aa2748c313ae3184f0fda76fa0a18924f216ca94c22d4c54d98'), 'refresh': ('docs/evidence/main67107-fronts-6e45-20261017/refresh-composed.py', 'dbfb6f9dfbec702ed3f626588ceff61b9d454d08b5bd10f17dc1b07283cc3b64'), 'parts': ('docs/evidence/main67107-fronts-6e45-20261017/verify-source-parts-composed.py', '48c26519908cda623bb69ebacce8324271dafe8b62660789cad9be262b3c4120'), 'checks': ('docs/evidence/main67107-fronts-6e45-20261017/check-source-parts-composed.py', '1d04e1cc6ea2c606ac8338c264ffbfd28bc2d9ba31b6675774bcb9d9e4d1a2bc')}
def sha(b):return hashlib.sha256(b).hexdigest()
raw=(E/'stage.py').read_bytes();assert sha(raw)==HELPER_SHA
spec=importlib.util.spec_from_file_location('compile35f_closed_acquisition',E/'stage.py');helper=importlib.util.module_from_spec(spec);spec.loader.exec_module(helper)
compile35f_stage=helper.stage
def program(tool,repo):
 repo=Path(repo);relative,expected=TOOLS[tool];path=repo/relative;before=path.read_text();assert sha(before.encode())==expected
 current=before;edits=[]
 def replace(old,new):
  nonlocal current
  assert current.count(old)==1,(tool,old)
  current=current.replace(old,new,1);edits.append((old,new))
 def constant(name,change):
  tree=ast.parse(current);nodes=[n for n in ast.walk(tree) if isinstance(n,ast.Assign)and any(isinstance(t,ast.Name)and t.id==name for t in n.targets)]
  assert len(nodes)==1;node=nodes[0].value;old=ast.get_source_segment(current,node);value=ast.literal_eval(node);replace(old,repr(change(value)))
 if tool=='main':
  replace("current=(S/row['path']).read_bytes();text=current.decode()","current=compile35f_stage(R,row['path'],(S/row['path']).read_bytes());text=current.decode()")
  replace("bindings[row['path']]=sha(current);targets[target['archive']]=sha(old)","bindings[row['path']]=sha((S/row['path']).read_bytes());targets[target['archive']]=sha(old)")
  replace("b=(S/row['path']).read_bytes();bindings[row['path']]=sha(b);row['sha256']=sha(b)","b=compile35f_stage(R,row['path'],(S/row['path']).read_bytes());bindings[row['path']]=sha((S/row['path']).read_bytes());row['sha256']=sha(b)")
 elif tool=='refresh':
  constant('replacement',lambda s:s.replace('raw=(R/f).read_bytes()','raw=compile35f_stage(R,f,(R/f).read_bytes())',1))
  constant('new_parts',lambda s:s.replace('docs/evidence/main67107-fronts-6e45-20261017/verify-source-parts-composed.py','docs/evidence/compile-acquisition-35f-20261017/run-common.py',1))
  replace("'__name__':'__main__','alias_stage':alias_stage})","'__name__':'__main__','alias_stage':alias_stage,'compile35f_stage':compile35f_stage})")
 elif tool=='parts':
  constant('ACQUIRE_NEW',lambda s:s.replace("raw=(R/r['source']).read_bytes()","raw=compile35f_stage(R,r['source'],(R/r['source']).read_bytes())",1))
  constant('PARTS_NEW',lambda s:s.replace("'TARGET':TARGET})","'TARGET':TARGET,'compile35f_stage':compile35f_stage})",1))
  replace("'projected_verifier':projected,'alias_stage':alias_stage,'TARGET':TARGET})","'projected_verifier':projected,'alias_stage':alias_stage,'TARGET':TARGET,'compile35f_stage':compile35f_stage})")
 elif tool=='checks':
  replace("raw=(R/m.TARGET).read_bytes();sha=lambda b:hashlib.sha256(b).hexdigest()","raw=compile35f_stage(R,m.TARGET,(R/m.TARGET).read_bytes());sha=lambda b:hashlib.sha256(b).hexdigest()")
  replace("# Concrete full census is deliberately raw and is a separate old read site.","# Concrete full census is deliberately raw and is a separate old read site.\nhistorical_compile_input=raw\nraw=(R/m.TARGET).read_bytes()\nassert compile35f_stage(R,m.TARGET,raw)==historical_compile_input")
 else:raise AssertionError('foreign closed common tool')
 restored=current
 for old,new in reversed(edits):
  assert restored.count(new)==1
  restored=restored.replace(new,old,1)
 assert restored==before;ast.parse(current)
 return path,current,{'tool':tool,'original_sha256':sha(before.encode()),'projected_sha256':sha(current.encode()),'whole_original_restored':True,'exact_edits':[{'before':a,'after':b}for a,b in edits]}
if __name__=='__main__':
 tool='parts'
 if '--tool'in sys.argv:
  at=sys.argv.index('--tool');tool=sys.argv[at+1];del sys.argv[at:at+2]
 assert tool in TOOLS
 repo=Path(sys.argv[sys.argv.index('--repo')+1]).resolve()
 path,current,proof=program(tool,repo)
 sys.argv[0]=str(path)
 exec(compile(current,str(path),'exec'),{'__file__':str(path),'__name__':'__main__','compile35f_stage':compile35f_stage})
