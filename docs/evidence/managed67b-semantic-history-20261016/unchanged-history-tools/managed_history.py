"""Fixed ten-wire managed history; only exact budget/digest cascade may vary."""
from pathlib import Path
import hashlib,json
HERE=Path(__file__).resolve().parent
sha=lambda raw:hashlib.sha256(raw).hexdigest()
def encoded(value):return json.dumps(value,separators=(',',':'),ensure_ascii=False).encode()
def self_digest(raw,name):
 data=json.loads(raw);assert encoded(data)==raw,'noncanonical fixture byte framing'
 if name in ('public-graph','projection-wire'):
  key='graph_digest';domain=b'semaprax.workspace-semantic-graph.artifact-digest.v1\0'
 else:
  key='artifact_digest';kind='review'if name.endswith('review')else'impact'if name.endswith('impact')else'context';domain=('semaprax.workspace-semantic-'+kind+'.artifact-digest.v1\0').encode()
 field=b','+encoded(key)+b':'+encoded(data[key]);assert raw.count(field)==1
 payload=raw.replace(field,b'',1);assert data[key]=='sha256:'+sha(domain+len(payload).to_bytes(8,'little')+payload)
 return data
def restore_exact(raw,prior,allowed):
 a=json.loads(raw);b=json.loads(prior);edits={};changes=[]
 def walk(x,y,path=''):
  assert type(x)is type(y),('type drift',path)
  if isinstance(x,dict):
   assert list(x)==list(y),('key/order drift',path)
   for k in x:walk(x[k],y[k],path+'/'+k)
  elif isinstance(x,list):
   assert len(x)==len(y),('array length drift',path)
   for i,(v,w)in enumerate(zip(x,y)):walk(v,w,path+'/'+str(i))
  elif x!=y:
   assert path in allowed,('non-budget/digest field drift',path)
   key=path.rsplit('/',1)[1];after=encoded(key)+b':'+encoded(x);before=encoded(key)+b':'+encoded(y)
   assert after not in edits or edits[after]==before;edits[after]=before;changes.append(path)
 walk(a,b)
 restored=raw
 for after,before in edits.items():
  assert restored.count(after)==prior.count(before)>0,('field multiplicity drift',after)
  restored=restored.replace(after,before)
 assert restored==prior,'complete historical raw byte restoration failed'
 return changes
def verify_managed_history(wires,root):
 binding=json.loads((HERE/'capture-history-binding.json').read_text());allow=json.loads((HERE/'managed-allowed-paths.json').read_text());wires=Path(wires);root=Path(root)
 assert {p.name for p in wires.glob('*.json')}==set(binding['actual83ca_managed_all10_wire_sha256'])
 out=[]
 for file,h in binding['actual83ca_managed_all10_wire_sha256'].items():
  raw=(wires/file).read_bytes();before=(root/'docs/evidence/actual83ca-pin-successor-20261016/actual-managed/wires'/file).read_bytes();assert sha(before)==h
  self_digest(raw,Path(file).stem);self_digest(before,Path(file).stem);changes=restore_exact(raw,before,allow[file]);out.append({'wire':file,'new_sha256':sha(raw),'prior83ca_sha256':h,'changed_paths':changes,'full_raw_restored':True})
  # The complete47 domain stays separate and is never replaced by83ca.
  old=(Path('/Volumes/Semaprax/provenance/managed-capture-47f-20261016/wires')/file).read_bytes();assert sha(old)==binding['actual47_managed_all10_wire_sha256'][file];self_digest(old,Path(file).stem);restore_exact(raw,old,allow[file])
 # Independent actual55e domain; original47/83ca checks above remain unchanged.
 history=json.loads((HERE/'actual55e-history-binding.json').read_text())
 capture=Path(history['capture']);assert sha((capture/'candidates.json').read_bytes())==history['candidates_sha256']
 assert (capture/'source-head.txt').read_text().strip()==history['source_head']
 assert set(history['all10_wire_sha256'])==set(binding['actual83ca_managed_all10_wire_sha256'])
 for file,expected in history['all10_wire_sha256'].items():
  before=(capture/'wires'/file).read_bytes();raw=(wires/file).read_bytes();assert sha(before)==expected
  self_digest(before,Path(file).stem);changed=restore_exact(raw,before,allow[file])
  out.append({'wire':file,'new_sha256':sha(raw),'prior55e_sha256':expected,'changed_paths':changed,'full_raw_restored':True})
 return out
