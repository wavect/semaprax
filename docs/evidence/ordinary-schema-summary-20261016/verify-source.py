"""Whole-source composition proof only. Does not execute Rust or prove parity."""
from pathlib import Path
import hashlib,json,re,subprocess
R=Path(__file__).resolve().parents[3];D=Path(__file__).resolve().parent
sha=lambda b:hashlib.sha256(b).hexdigest()
# Exact additive6db predecessor; original scripts/capsules archived in full.
import runpy
compose=runpy.run_path(str(R/'docs/evidence/main-eb2-finite-predecessors-20261016/finite.py'))['compose']

def restore(row,raw):
 raw=compose('ordinary',row['source'],raw)
 assert sha(raw)==row['current_sha256'] and len(raw)==row['current_length']
 text=raw.decode();cursor=0;out=''
 for e in row['inverse']:
  assert cursor<=e['start']<=e['end']<=len(text)
  assert text[e['start']:e['end']]==e['after']
  out+=text[cursor:e['start']]+e['before'];cursor=e['end']
 restored=(out+text[cursor:]).encode();assert sha(restored)==row['before_sha256'];return restored
m=json.loads((D/'source-capsule.json').read_text());negatives=0
for row in m['rows']:
 raw=(R/row['source']).read_bytes();old=(D/row['archive']).read_bytes()
 assert old==subprocess.check_output(['git','show',m['base']+':'+row['source']],cwd=R)
 assert restore(row,raw)==old
 for bad in [raw+b' ',raw[:-1],raw.replace(raw[:1],b'!',1)]:
  try:restore(row,bad)
  except AssertionError:negatives+=1
  else:raise AssertionError(('complete source drift accepted',row['source']))
for row in m['new_sources']:
 b=(R/row['source']).read_bytes();assert sha(b)==row['sha256'] and len(b)==row['length']
 for guard in m['raw_guards']:assert row['source'] in (R/guard).read_text(),('new raw source missing',guard,row['source'])
def assertions(source):
 out=[]
 for match in re.finditer(r'\bassert(?:_eq|_ne)?!\s*\(',source):
  start=match.start();pos=match.end();depth=1
  while depth:
   raw=re.match(r'r(#{0,})"',source[pos:])
   if raw:
    close='"'+raw[1];pos=source.index(close,pos+raw.end())+len(close);continue
   if source[pos]=='"':
    pos+=1
    while source[pos]!='"':pos+=2 if source[pos]=='\\' else 1
    pos+=1;continue
   char=re.match(r"'(?:\\.|[^'\n])'",source[pos:])
   if char:pos+=char.end();continue
   if source.startswith('//',pos):pos=source.index('\n',pos)+1;continue
   if source.startswith('/*',pos):pos=source.index('*/',pos+2)+2;continue
   depth+=(source[pos]=='(')-(source[pos]==')');pos+=1
  out.append(source[start:pos])
 return out
# Every old source assertion stays complete, in order; constructors and the two
# named whole-source read sites are the sole adaptations, never assert bodies.
for p in m['assertion_subjects']:
 old=(D/'before'/p.replace('/','__')).read_text();current=(R/p).read_text()
 assert assertions(old)==assertions(current),(p,'complete old assertion body/order changed')
# Complete original materializer body, no diagnostic/order rewrites.
old=(D/'before/src__cleanup_plan__replay__schema.rs').read_text();a=old.index('pub(super) fn selected_schema_with_index(');b=old.index('\nfn function_has_owner_admission(',a)
reference=(R/'src/cleanup_plan/replay/schema/original_materializer.rs').read_text()
assert reference=='//! Complete pre-adoption schema materializer, independently executable.\nuse super::*;\n'+old[a:b]
for file in ['collector.rs','generic.rs','profile.rs','shape.rs']:
 p='src/cleanup_plan/replay/paid_type_facts/inventory_summary/'+file
 assert (R/p).read_bytes()==subprocess.check_output(['git','show',m['base']+':'+p],cwd=R)
# Paid constructors and every existing ordinary semantic collector are unchanged.
assert not subprocess.check_output(['git','diff',m['base'],'--','src/cleanup_plan/replay/paid_type_facts.rs','src/cleanup_plan/replay/paid_type_facts'],cwd=R)
for file in ['src/cleanup/schema_summary.rs','src/cleanup/schema_summary/generic_record.rs','src/cleanup/schema_summary/support.rs','src/cleanup/schema_summary/tests.rs','src/cleanup/inventory_builder.rs','src/cleanup/generic_record.rs']:
 assert (R/file).read_bytes()==subprocess.check_output(['git','show',m['base']+':'+file],cwd=R)
summary=(R/'src/cleanup/schema_summary.rs').read_text()
assert summary.index('if let Some(error) = builder.slots.profile_error')>summary.index('for expression in &function.ensures')
assert 'Err(error) if self.profile_error.is_none() => self.profile_error = Some(error)'in summary
schema=(R/'src/cleanup_plan/replay/schema.rs').read_text();assert 'crate::cleanup::build_inventory'not in schema
assert '#[cfg(test)]\npub(super) mod original_materializer;'in schema
# Replay the exact new Rust source-read inverse, preserving the old assertion.
adapter=(R/'src/cleanup_plan/replay/schema/ordinary_tests.rs').read_text()
block=adapter.split('for (after, before) in [',1)[1].split('] {',1)[0]
quoted=r'"(?:\\.|[^"\\])*"';pairs=re.findall(r'\(\s*('+quoted+r')\s*,\s*('+quoted+r')\s*\)',block)
assert len(pairs)==4
pairs=[(json.loads(a),json.loads(b))for a,b in pairs]
def schema_before(raw):
 for after,before in pairs:
  assert raw.count(after)==1;raw=raw.replace(after,before,1)
 assert raw==old
 return raw
assert schema_before(schema)==old
read_negatives=0
for hostile in [schema.replace('inventory.has_iterator','false',1),schema+'inventory.has_iterator',schema+'// foreign current source\n']:
 try:schema_before(hostile)
 except AssertionError:read_negatives+=1
 else:raise AssertionError('hostile schema read-site accepted')
reference='src/cleanup_plan/replay/schema/paid_summary/original_receiver.rs'
current_reference=(R/reference).read_text();alias='use super::super::original_materializer as schema;\n';assert current_reference.count(alias)==1
assert current_reference.replace(alias,'',1)==(D/'before'/reference.replace('/','__')).read_text()
# Current raw guards are strict additive extensions of their complete preimages.
for guard in m['raw_guards']:
 row=next(x for x in m['rows']if x['source']==guard);assert restore(row,(R/guard).read_bytes())==(D/row['archive']).read_bytes()
 p=R/guard;assert len(p.read_text().splitlines())<=1500 or guard.startswith('tests/')
allowed='docs/evidence/main043-independent-custody-20261016/guard-extension-allowed.json';before=json.loads((D/'before'/allowed.replace('/','__')).read_text());current=json.loads((R/allowed).read_text())
# The parent has independently joined 765/854 on this exact immutable head.
# Accept its complete allowed registry plus our exact additions as a second
# explicit composition, without admitting arbitrary extra subjects.
joined=json.loads(subprocess.check_output(['git','show','12e7f8a4d3fcb3c5464327a68a8e13506b43e303:'+allowed],cwd=R))
boundary_joined=json.loads(subprocess.check_output(['git','show','f61449708234f99b2a50fa7455a2a1154b765876:'+allowed],cwd=R))
record_joined=json.loads(subprocess.check_output(['git','show','d96247b3cb69282c3d1b835b2428779e8bde82c7:'+allowed],cwd=R))
next_joined=json.loads(subprocess.check_output(['git','show','1f1521e2d4f07ae06e4d1a1d90dbcbdbe92a50f3:'+allowed],cwd=R))
allowed_registry=False
main_joined=json.loads(subprocess.check_output(['git','show','6db5964817b0c3e2a368074995c32fe00b5228fa:'+allowed],cwd=R))
for predecessor in [before,joined,boundary_joined,record_joined,next_joined,main_joined]:
 expected=json.loads(json.dumps(predecessor))
 for guard in m['raw_guards']:
  for key in ['required','maximum']:
   expected[guard][key].update({x['source']:1 for x in m['new_sources']})
   expected[guard][key].update({x:1 for x in m['extra_raw'].get(guard,[])})
 allowed_registry|=current==expected
assert allowed_registry,'foreign/missing/duplicate allowed registry composition'

proof=dict(mode='SOURCE_ONLY_NO_RUST_EXECUTION',whole_preimages=len(m['rows']),changed_whole_endpoints=sum(x['changed']for x in m['rows']),unchanged_retained_preimages=sum(not x['changed']for x in m['rows']),whole_current_drift_negatives_refused=negatives,complete_old_materializer_body=True,paid_subtree_byte_exact=True,ordinary_summary_four_sources_byte_exact=True,complete_old_inventory_and_generic_shape_byte_exact=True,delayed_profile_error_publication_byte_exact=True,old_assertion_subjects=len(m['assertion_subjects']),whole_schema_missing_duplicate_foreign_negatives_refused=read_negatives,raw_joins=3,new_complete_raw_subjects=len(m['new_sources']),native_inventory_lines=len((R/m['raw_guards'][-1]).read_text().splitlines()),runtime_parity='PENDING',paid_metrics='PENDING',authority_scope='PARENT_REVIEW_PENDING')
(D/'source-proof.json').write_text(json.dumps(proof,indent=2)+'\n');print(json.dumps(proof,indent=2))
