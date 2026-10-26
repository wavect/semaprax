"""Verify full preserved native wires without normalizing any live artifact."""
from pathlib import Path
import json,hashlib,copy
O=Path(__file__).parent
def wire(v):return json.dumps(v,ensure_ascii=False,separators=(',',':')).encode()
def sha(b):return 'sha256:'+hashlib.sha256(b).hexdigest()
def digest(schema,payload):
 domain=(schema.rsplit('.v1',1)[0]+'.artifact-digest.v1\0').encode()
 return sha(domain+len(payload).to_bytes(8,'little')+payload)
def rebound(v,delta,graph):
 if isinstance(v,list):return [rebound(x,delta,graph) for x in v]
 if not isinstance(v,dict):return v
 out={k:rebound(x,delta,graph) for k,x in v.items()}
 if 'used_builder_bytes' in out and out['used_builder_bytes']==926453:out['used_builder_bytes']+=delta
 if 'workspace_graph_digest' in out:out['workspace_graph_digest']=graph
 key='graph_digest' if 'graph_digest' in out else 'artifact_digest' if 'artifact_digest' in out else None
 if key:
  payload={k:x for k,x in out.items() if k!=key}
  out[key]=digest(out['schema'],wire(payload))
 return out

proof=json.loads((O/'proof.json').read_text())
mac={row['name']:json.loads((O/(row['name']+'-current-macos.json')).read_bytes()) for row in proof['rows']}
for row in proof['rows']:
 name=row['name'];data=mac[name]
 assert wire(data)==(O/(name+'-current-macos.json')).read_bytes()
 assert sha(wire(data))==row['current_macos']
 for suffix,delta,key in [('current-portable',-144,'current_portable'),('old-macos',2232,'old_macos'),('old-portable',2088,'old_portable')]:
  graph=rebound(mac['public-graph'],delta,None)['graph_digest']
  candidate=wire(rebound(data,delta,graph))
  assert candidate==(O/(name+'-'+suffix+'.json')).read_bytes(), (name,suffix)
  assert sha(candidate)==row[key],(name,key)
print('PASS: all 36 full wires, all 18 previous known answers, every current cross-host byte')
