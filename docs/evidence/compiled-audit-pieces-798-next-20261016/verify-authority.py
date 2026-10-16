"""Source-only replay of the finite custody/registration partition, not a Rust gate."""
from pathlib import Path
import json,hashlib,re,copy
R=Path(__file__).resolve().parents[3];D=Path(__file__).resolve().parent;sha=lambda b:hashlib.sha256(b).hexdigest()
def verify(m,overrides=None):
 sources={}
 for path,row in m['sources'].items():
  assert not Path(path).is_absolute() and '..'not in Path(path).parts;b=(overrides or {}).get(path,(R/path).read_bytes());assert len(b)==row['length'] and sha(b)==row['sha256'];sources[path]=b.decode()
 production=set();test=set()
 for path,row in m['classes'].items():
  assert row['chains'];closed=[]
  for chain in row['chains']:
   current=chain['root'];assert current in sources;is_test=chain['kind']=='test';assert is_test or (chain['kind']=='lib' and current=='src/lib.rs') or (chain['kind']=='bin' and current=='src/main.rs' and chain['package']=='semaprax')
   for edge in chain['edges']:
    assert current==edge['parent'];b=sources[current].encode();text=edge['text'].encode();assert b[edge['start']:edge['start']+len(text)]==text;current=edge['child'];assert current in sources
    for attr in edge['attributes']:
     assert attr in edge['text'];is_test|=re.fullmatch(r'#\[\s*cfg\s*\(\s*test\s*\)\s*\]',attr)is not None
   assert current==path;closed.append(is_test)
  if row['role']=='test-only':assert all(closed);test.add(path)
  else:assert row['role']=='production'and not all(closed);production.add(path)
 assert production.isdisjoint(test);observed=set();raw='';pure=''
 for chunk in m['chunks']:
  if 'path'in chunk:
   p=chunk['path'];assert p in m['classes'];observed.add(p);raw+=sources[p]
   if p in production:pure+=sources[p]
  else:raw+=chunk['literal'];pure+=chunk['literal']
 for p in m['closure_only']:
  assert p in m['classes'] and p not in observed;observed.add(p)
  if p in production:pure+=sources[p]
 assert observed==production|test
 return raw,pure,production,test
proof={}
for name in ['hir','projection']:
 m=json.loads((D/(name+'.json')).read_text());raw,pure,p,t=verify(m)
 owner=(R/('tests/hir_module_boundaries.rs'if name=='hir'else'tests/workspace/semantic_graph.rs')).read_text()
 anchor=owner.index('let validation = production_source_inventory'if name=='hir'else'let authority_projection = production_projection')
 block=owner[owner.index('for forbidden in [',anchor):];block=block[:block.index('] {')]
 forbidden=re.findall(r'"([^"\n]*)"',block)
 if name=='projection':forbidden=['use super::*;']+forbidden
 bad=[x for x in forbidden if x in pure];assert not bad,(name,bad)
 proof[name]={'raw_sha256':sha(raw.encode()),'production_sha256':sha(pure.encode()),'unique_production_files':len(p),'unique_test_only_files':len(t),'chunks_including_multiplicity_and_separators':len(m['chunks']),'all_original_forbidden_checks_clear':True,'exact_existing_forbidden_tokens':forbidden}
 # Three in-memory admission negatives: arbitrary test reclassification,
 # changed current parent bytes, and an unclassified new raw child.
 neg=copy.deepcopy(m);path=next(x for x in p if all(c['kind']=='lib' for c in m['classes'][x]['chains']));neg['classes'][path]['role']='test-only'
 try:verify(neg)
 except AssertionError:pass
 else:raise AssertionError('production reclassification accepted')
 parent=next(iter(m['sources']));b=(R/parent).read_bytes()
 try:verify(m,{parent:b+b' '})
 except AssertionError:pass
 else:raise AssertionError('current source drift accepted')
 neg=copy.deepcopy(m);neg['chunks'].append({'path':'src/unclassified_authority_child.rs'})
 try:verify(neg)
 except AssertionError:pass
 else:raise AssertionError('unclassified child accepted')
 proof[name]['three_static_negatives_refused']=True

# Exact chunk order/literals and all roles are unchanged from the full baseline.
import subprocess
base='b5eca55b96890e1a815f0d8d9e7b42c78d743b07'
for name in ['hir','projection']:
 old=json.loads(subprocess.check_output(['git','show',base+':docs/evidence/afba-presence-successor-20261016/'+name+'.json'],cwd=R))
 now=json.loads((D/(name+'.json')).read_text())
 assert now['chunks']==old['chunks'],name
 assert now['closure_only']==old['closure_only'],name
 assert {p:r['role']for p,r in now['classes'].items()}=={p:r['role']for p,r in old['classes'].items()},name
 for p,row in old['classes'].items():
  if row['role']=='production':assert now['sources'][p]==old['sources'][p],(name,p,'shipping source drift')
proof['identical_complete_chunk_sequence_literals_and_roles']=True
proof['mode']='SOURCE_ONLY_NO_RUST_EXECUTION_NO_MEMORY_CLAIM'
(D/'authority-source-proof.json').write_text(json.dumps(proof,indent=2)+'\n');print(json.dumps(proof,indent=2))
