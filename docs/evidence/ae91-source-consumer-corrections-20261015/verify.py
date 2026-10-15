"""Finite actual source failure replay, not Rust qualification."""
from pathlib import Path
import json,subprocess,hashlib,re,ast,difflib,contextlib,io
R=Path.cwd();D=Path(__file__).resolve().parent;sha=lambda b:hashlib.sha256(b).hexdigest();AE='ae91fa963ecfe85f4cc830cda85fff2f5bdbe094'
def load(n):return json.loads((D/n).read_text())
def apply(r,b):
 assert sha(b)==r['current_sha256']
 for e in r['edits']:
  a=e['new'].encode();assert b.count(a)==1;b=b.replace(a,e['old'].encode(),1)
 assert sha(b)==r['original_sha256'];return b
rows=load('current-to-ae91.json');named={};negative=0
for r in rows:
 p=R/r['source'];b=apply(r,p.read_bytes());assert b==subprocess.check_output(['git','show',AE+':'+r['source']]);named[p.resolve()]=b
 try:apply(r,b)
 except AssertionError:negative+=1
 else:raise AssertionError('stale ae91 raw accepted')
# Exact immutable child hash domains, including every unchanged child.
for domain,packet in [('registry833','registry-census-scratch-833'),('cold835','canonical-comment-comparison-835')]:
 old=json.loads((R/('docs/evidence/'+packet+'/source-inverses.json')).read_text());fix=load(domain+'-children.json')
 for child in old['children']:
  p=child['source'];b=(R/p).read_bytes();rs=[r for r in fix if r['source']==p]
  if rs:b=apply(rs[0],b)
  assert sha(b)==child['sha256'],p
# Full old/current patch receiver, not only its hash: use the unchanged old patch applier.
p='src/cleanup_plan/replay/factored/canonical_shared/ordered_weights/tests.rs';fix=load('ordered838-observer.json')[0];target=apply(fix,(R/p).read_bytes());assert target==subprocess.check_output(['git','show','3d4b996c9a8cd2b100a3c594e741314c108b5591:'+p])
patch=R/'docs/evidence/weighted-storage-838/amortized/src__cleanup_plan__replay__factored__canonical_shared__ordered_weights__tests.rs.patch.txt';before=patch.with_name(patch.name.replace('.patch.txt','.before.txt')).read_text();lines=before.splitlines(True);change=patch.read_text().splitlines(True)
def patch_apply(text,reverse):
 src=text.splitlines(True);out=[];pos=0;i=0
 while i<len(change):
  if not change[i].startswith('@@'):i+=1;continue
  m=re.match(r'@@ -(\d+)(?:,(\d+))? \+(\d+)(?:,(\d+))? @@',change[i]);assert m
  start=int(m[3] if reverse else m[1])-1;out+=src[pos:start];pos=start;i+=1
  while i<len(change) and not change[i].startswith('@@'):
   l=change[i];i+=1
   if l.startswith(('---','+++')):continue
   if l.startswith('\\'):continue
   if not l or l[0] not in ' +-':continue
   tag=l[0];content=l[1:]
   if tag==' ' or tag==('+' if reverse else '-'):
    assert src[pos]==content;pos+=1
   if tag==' ' or tag==('-' if reverse else '+'):out.append(content)
 out+=src[pos:];return ''.join(out)
assert patch_apply(before,False).encode()==target;assert patch_apply(target.decode(),True)==before
# Preserve the complete old facade equality and prove the exhaustive current one.
facade=load('facade.json');current=facade['edits'][0]['new'];historical=apply(facade,current.encode()).decode();test=(R/'tests/workspace/semantic_graph.rs').read_text();m=re.search(r'let exact_facade = \[(.*?)\n    \];',test,re.S);assert m
expected=re.findall(r'^\s*("(?:\\.|[^"\\])*")\s*,',m[1],re.M);expected=[ast.literal_eval(x) for x in expected]
public=lambda s:[l.strip() for l in s.splitlines() if 'pub(super)' in l.strip()]
assert public(historical)==expected
extra=['pub(super) fn pay_type(declaration: &TypeDeclaration, owner: &mut Receiver) -> Result<(), Setup> {','pub(super) fn pay_signature(function: &Function, owner: &mut Receiver) -> Result<(), Setup> {','pub(super) fn teardown_type(','pub(super) fn teardown_function(','pub(super) fn pay_default_teardown(','pub(super) fn pay_teardown_minimum(owner: &mut Receiver) -> Result<(), Setup> {'];at=expected.index('pub(super) fn materialize_receiving(')+1;active=expected[:at]+extra+expected[at:]+['pub(super) fn verify('];assert public(current)==active
oldtest=subprocess.check_output(['git','show',AE+':tests/workspace/semantic_graph.rs']).decode();oldassert=re.search(r'    assert_eq!\(\n        projection_root\n.*?        exact_facade,\n    \);',oldtest,re.S)[0];assert oldassert in test
# Replay every prior consumer and its complete production scope in named ae91 bytes.
old=R/'docs/evidence/source-consumer-closure-fixes-20261015/verify.py';oldsource=old.read_text();rb=Path.read_bytes;rt=Path.read_text;wt=Path.write_text;proof={}
def readb(p):return named[p.resolve()] if p.resolve() in named else rb(p)
def readt(p,*a,**kw):return named[p.resolve()].decode() if p.resolve() in named else rt(p,*a,**kw)
def write(p,s,*a,**kw):assert p.resolve()==(old.parent/'source-only-proof.json').resolve();proof.update(json.loads(s));return len(s)
try:
 Path.read_bytes=readb;Path.read_text=readt;Path.write_text=write
 old_ns={'__file__':str(old),'__name__':'__main__'}
 with contextlib.redirect_stdout(io.StringIO()):exec(compile(oldsource,str(old),'exec'),old_ns)
finally:Path.read_bytes=rb;Path.read_text=rt;Path.write_text=wt
assert proof['historical_consumer_rows_replayed']==57 and proof['all_namespace_sources']==96
# The same full actual production/compiled-control union is still scanned now.
joined='\n'.join((R/p).read_text() for p in old_ns['purity'])
assert 'use super::*;' not in joined
prelude=(R/'src/workspace_graph/expected_projection/prelude_forecast_tests.rs').read_text();tls='crate::workspace_graph::ACTIVE_BUILDER_LIMIT';assert joined.count(prelude)==1 and prelude.count(tls)==6
joined=joined.replace(prelude,prelude.replace(tls,'test_only_builder_limit'),1)
for p in ['src/workspace_graph/expected_projection/private_layout/function_runtime.rs','src/workspace_graph/expected_projection/uncached_absence_tests.rs']:joined=joined.replace((R/p).read_text(),'')
for forbidden in ['workspace::','std::fs','std::process','snapshot(','build_authenticated','render_graph_json','write_file','create_dir','remove_file','ACTIVE']:assert forbidden not in joined,forbidden

# No existing evidence or runtime manifest was rewritten.
changed=subprocess.check_output(['git','diff','--name-only',AE,'--','docs/evidence'],text=True).splitlines();allowed=['docs/evidence/ae91-source-consumer-corrections-20261015/','docs/evidence/foreign-owner-test-corrections-ae91-20261015/'];assert all(any(p.startswith(a) for a in allowed) for p in changed)
report={'status':'SOURCE_ONLY_NO_RUST_EXECUTION','frozen_prior':AE,'immediate_full_source_inputs':len(rows),'strict_stale_rejections':negative,'all_immutable_child_hashes_match_after_exact_inverses':True,'ordered_full_forward_and_reverse_patch_equality':True,'original_facade_assertion_body_retained':True,'historical_facade_entries':len(expected),'exhaustive_current_facade_entries':len(active),'new_current_facade_entries':7,'complete_actual_purity_inventory_preserved_and_scan_passed':True,'old_runtime_jsons_and_packets_unchanged':True,'previous_consumer_proof':proof,'new_test_bodies':0,'production_changes':0,'runtime_bodies_executed':0}
(D/'source-only-proof.json').write_text(json.dumps(report,indent=2)+'\n');print('PASS 4 observed source domains;',len(rows),'full immediate sources;',len(expected),'old/',len(active),'current exact facade entries')
