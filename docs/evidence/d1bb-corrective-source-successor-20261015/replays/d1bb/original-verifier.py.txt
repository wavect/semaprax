"""Bounded current/main/9365/key-compatibility custody, no Rust execution."""
from pathlib import Path
import sys,json,hashlib,subprocess,gzip
R=Path.cwd();D=Path(__file__).resolve().parent;sys.path.insert(0,str(R/'docs/evidence/incremental-source-composition-5f1fecd-20261015'));from source_tools import Git
G=Git(R);sha=lambda b:hashlib.sha256(b).hexdigest();read=lambda p:(R/p).read_bytes();load=lambda p:json.loads(read(p));meta=json.loads((D/'successor.json').read_text());OLD=meta['prior_source_domain'];MAIN=meta['latest_main_domain'];F='docs/evidence/final-842-843-source-assembly-20261015/';C='docs/evidence/source-composition-f813-held-20261015/'
def apply(rows,p,b):
 found=[r for r in rows if r['source']==p];assert len(found)<=1
 if not found:return b
 r=found[0];assert sha(b)==r['current_sha256'],p
 for e in r['edits']:assert b.count(e['new'].encode())==1;b=b.replace(e['new'].encode(),e['old'].encode(),1)
 assert sha(b)==r['original_sha256'];return b
try:
 rows=json.loads((D/'current-to-9365.json').read_text());negatives=0
 for r in rows:
  p=r['source'];cur=read(p);assert apply([r],p,cur)==G.get(OLD,p)
  try:apply([r],p,G.get(OLD,p))
  except AssertionError:negatives+=1
  else:raise AssertionError('stale input accepted:'+p)
 def prior(p):
  b=apply(rows,p,read(p));assert b==G.get(OLD,p);return b
 oldmeta=load(F+'branches.json');oldviews={s:load(F+'view-'+s+'.json') for s in ['9d','842','843']};hooks={'9d':load('docs/evidence/compiler-receiving-fix-7800-20261015/predecessor-projections.json'),'842':load('docs/evidence/inline-weighted-successor-842/source-inverses.json'),'843':load('docs/evidence/held-closure-receiver-843/runtime-projections.json')}
 def inherited_main(p):
  raw=prior(p);stages=[apply(hooks[s],p,apply(oldviews[s],p,raw)) for s in hooks];assert stages[0]==stages[1]==stages[2];return apply(load(C+'main-projections.json'),p,stages[0])
 newci=load('src/source_reflows.json');oldci=json.loads(G.get(OLD,'src/source_reflows.json'));assert len(newci)==87 and len(oldci)==85;assert read('src/source_reflows.json')==G.get(MAIN,'src/source_reflows.json');main=json.loads((D/'current-to-main7f.json').read_text());byold={r['path']:r for r in oldci}
 for r in newci:
  p=r['path'];inherited_main(p) # Original prior_main executes this strict route too.
  b=apply(main,p,read(p));assert b==G.get(MAIN,p);assert sha(b)==r['after_sha256']
  for e in r['inverse']:assert b.count(e['after'].encode())==1;b=b.replace(e['after'].encode(),e['before'].encode(),1)
  assert sha(b)==r['before_sha256']
  if p in byold:
   old=byold[p];assert old['before_sha256']==r['before_sha256'];assert all(e in r['inverse'] for e in old['inverse'])
 for p in oldmeta['source_paths']:
  if G.get(OLD,p) is not None:prior(p)
 # New compile adapter changes only the old helper after the complete old producer.
 key='docs/evidence/inline-weighted-successor-842-key-compat-fix-20261015/source-inverses.json';keyrows=load(key);assert len(keyrows)==1
 for r in keyrows:
  p=r['source'];cur=read(p);assert sha(cur)==r['current_sha256'];assert apply([r],p,cur)==G.get(OLD,p);old=G.get(OLD,p);marker=b'pub(super) fn same_closure_state';assert cur.split(marker)[0]==old.split(marker)[0]
 owners=['src/cleanup_plan/replay/path_join/storage/empty_renewals.rs','src/cleanup_plan/replay/path_join/storage/empty_renewals/tests.rs'];audits=[('tests/hir_module_boundaries.rs','../'),('tests/workspace/semantic_graph/source_inventory.rs','../../'),('crates/semaprax-native-rust-interop-builder/src/implementation/tests/ledger_capacity/source_inventory.rs','../../../../../')]
 for p,prefix in audits:
  cur=read(p).decode();expected=G.get(meta['production_basis'],p).decode();anchor='        include_str!("'+prefix+'src/source_reflows_merge.rs"),';addition=''.join('\n        include_str!("'+prefix+n+'"),' for n in owners);expected=expected.replace(anchor,anchor+addition)
  if p.startswith('crates/'):assert expected.count('\n}\n\n//')==2;expected=expected.replace('\n}\n\n//','\n}\n//');assert len(cur.splitlines())==1500
  assert expected==cur
  for n in owners:assert cur.count('include_str!("'+prefix+n+'")')==1;assert read(n)==G.get(MAIN,n)
 inputs=json.loads((D/'complete-inputs.json').read_text());assert inputs['paths']==25 and len(inputs['inputs'])==75;present=absent=0
 for r in inputs['inputs']:
  raw=G.get(r['ref'],r['path'])
  if r.get('absent'):assert raw is None;absent+=1;continue
  b=gzip.decompress((D/r['archive']).read_bytes());assert b==raw and sha(b)==r['sha256'];assert hashlib.sha1(b'blob '+str(len(b)).encode()+b'\0'+b).hexdigest()==r['git_blob'];present+=1
 # No old packet/proof/archive is changed; the only current runtime table delta is main's87-record table.
 changed=subprocess.check_output(['git','diff','--name-only','--diff-filter=M',meta['production_basis']],text=True).splitlines();assert sorted(changed)==sorted(meta['assembly_edits']);assert len(changed)==4
 for r in json.loads((D/'immediate-inverses.json').read_text()):assert apply([r],r['source'],read(r['source']))==G.get(meta['production_basis'],r['source'])
 for p in subprocess.check_output(['git','ls-tree','-r','--name-only',OLD,'--',F],text=True).splitlines():assert read(p)==G.get(OLD,p)
 proof={'status':'SOURCE_ONLY_NO_RUST_EXECUTION','production_basis':meta['production_basis'],'prior_domain':OLD,'latest_main':MAIN,'strict_current_to_9365_rows':len(rows),'stale_raw_refusals':negatives,'live_main_records':87,'prior_main_records_preserved':85,'main_target_deltas':len(main),'compatibility_full_owner_rows':1,'complete_old_producer_unchanged_before_adapter':True,'full_new_owners_in_each_of_three_audits':owners,'interop_lines':1500,'merge_paths':25,'merge_records':75,'present_inputs':present,'explicit_absent_inputs':absent,'assembly_files':changed,'production_byte_exact_to_basis':True,'old9365_packet_byte_exact':True,'git_cache_peak_bytes':G.peak,'runtime_bodies_executed':0}
 (D/'source-proof.json').write_text(json.dumps(proof,indent=2)+'\n');print(json.dumps(proof,indent=2))
finally:G.close()
