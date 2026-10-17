from pathlib import Path
import json,hashlib,copy
ROOT=Path('/Volumes/Semaprax/worktrees/large-next-implementation-integration/AI-Lang')
OLD=ROOT/'docs/evidence/managed67b-semantic-history-20261016/actual67b/wires'
NEW=Path('/Volumes/Semaprax/provenance/actual-managed-fade6f4df-20261017/wires')
OUT=Path(__file__).resolve().parent
sha=lambda b:hashlib.sha256(b).hexdigest()
enc=lambda x:json.dumps(x,separators=(',',':'),ensure_ascii=False).encode()
def bound_digest(value,role):
 key='graph_digest' if role=='graph' else 'artifact_digest'
 domain=('semaprax.workspace-semantic-'+('graph' if role=='graph' else role)+'.artifact-digest.v1\0').encode()
 raw=enc(value);field=b','+enc(key)+b':'+enc(value[key]);assert raw.count(field)==1
 payload=raw.replace(field,b'',1)
 return 'sha256:'+sha(domain+len(payload).to_bytes(8,'little')+payload)
def restore(value,prior,role):
 assert bound_digest(value,role)==value['graph_digest' if role=='graph' else 'artifact_digest']
 assert bound_digest(prior,role)==prior['graph_digest' if role=='graph' else 'artifact_digest']
 value=copy.deepcopy(value)
 if role=='graph':
  assert set(value)==set(prior)
  value['budget']['used_builder_bytes']=prior['budget']['used_builder_bytes']
  value['graph_digest']=prior['graph_digest']
 else:
  value['workspace_graph_digest']=prior['workspace_graph_digest']
  if role=='review':
   value['context']=restore(value['context'],prior['context'],'context')
   value['impact']=restore(value['impact'],prior['impact'],'impact')
  value['budget']['workspace']['used_builder_bytes']=prior['budget']['workspace']['used_builder_bytes']
  value['artifact_digest']=prior['artifact_digest']
 assert value==prior,'nonbilling semantic difference'
 assert bound_digest(value,role)==value['graph_digest' if role=='graph' else 'artifact_digest']
 return value
binding=json.loads((OLD.parent.parent/'transition-binding.json').read_text());rows=[];controls=0
for p in sorted(OLD.glob('*.json')):
 old=p.read_bytes();new=(NEW/p.name).read_bytes()
 assert sha(old)==binding['actual_wire_sha256'][p.name]
 assert enc(json.loads(old))==old and enc(json.loads(new))==new
 role='graph' if p.stem in ('public-graph','projection-wire') else 'review' if p.stem.endswith('review') else 'impact' if p.stem.endswith('impact') else 'context'
 a,b=json.loads(new),json.loads(old);restored=restore(a,b,role);assert enc(restored)==old
 # A recomputed digest is insufficient authority for nonbilling changes.
 hostile=copy.deepcopy(a)
 if role=='graph':hostile['budget']['used_declarations']+=1
 elif role=='review':hostile['context']['budget']['workspace']['used_declarations']+=1;hostile['context']['artifact_digest']=bound_digest(hostile['context'],'context')
 else:hostile['budget']['workspace']['used_declarations']+=1
 hostile['graph_digest' if role=='graph' else 'artifact_digest']=bound_digest(hostile,role)
 try:restore(hostile,b,role)
 except AssertionError:controls+=1
 else:raise AssertionError('hostile semantic mutation accepted')
 rows.append({'name':p.stem,'prior_wire_sha256':sha(old),'actual_wire_sha256':sha(new),'whole_prior_bytes_restored':True})
assert len(rows)==10 and controls==10
receipt={'source_head':'fade6f4df00808a4f9b763fff22cd5f3898683f4','status':'ACTUAL_PRIOR_CAPTURE_COMPARISON_ONLY_NO_PIN_APPLY','prior_capture':'committed actual67b','rows':rows,'recomputed_digest_semantic_hostiles_refused':controls,'public_builder_bytes':{'prior':1114864,'actual':1160532},'projection_builder_bytes':{'prior':2933905,'actual':3302915},'scope':'All ten complete actual wires equal prior bytes after only explicit builder bill, graph binding, and independently validated nested self-digest restoration. This is historical fade evidence; current production has since changed. Original strict updater refusal preserved; no replacement build/capture authority or pin application.'}
(OUT/'comparison.json').write_text(json.dumps(receipt,indent=2)+'\n');print('10 complete wire restorations PASS; 10 recomputed-digest semantic mutants refused; no pins applied')
