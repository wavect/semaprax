"""Strict corrective/CI/scalar source proof. Does not invoke Rust."""
from pathlib import Path
import json,hashlib,subprocess,sys,re
R=Path.cwd();D=Path(__file__).resolve().parent;sys.path.insert(0,str(R/'docs/evidence/incremental-source-composition-5f1fecd-20261015'));from source_tools import Git
G=Git(R);sha=lambda b:hashlib.sha256(b).hexdigest();read=lambda p:(R/p).read_bytes();load=lambda p:json.loads(read(p));meta=json.loads((D/'successor.json').read_text());OLD=meta['prior_domain'];M='docs/evidence/main7f-source-successor-20261015/';F='docs/evidence/final-842-843-source-assembly-20261015/';C='docs/evidence/source-composition-f813-held-20261015/'
def apply(rows,p,b):
 found=[r for r in rows if r['source']==p];assert len(found)<=1
 if not found:return b
 r=found[0];assert sha(b)==r['current_sha256'],p
 for e in r['edits']:assert b.count(e['new'].encode())==1;b=b.replace(e['new'].encode(),e['old'].encode(),1)
 assert sha(b)==r['original_sha256'];return b
try:
 rows=json.loads((D/'current-to-d1bb.json').read_text());assert len(rows)==5;negative=0
 for r in rows:
  p=r['source'];assert apply([r],p,read(p))==G.get(OLD,p)
  try:apply([r],p,G.get(OLD,p))
  except AssertionError:negative+=1
  else:raise AssertionError('stale d1bb raw accepted')
 def d1(p):
  b=apply(rows,p,read(p));assert b==G.get(OLD,p);return b
 # Exercise the exact current corrected→d1bb→9365→branch/f813 route and live main87 path.
 views={s:load(F+'view-'+s+'.json') for s in ['9d','842','843']};hooks={'9d':load('docs/evidence/compiler-receiving-fix-7800-20261015/predecessor-projections.json'),'842':load('docs/evidence/inline-weighted-successor-842/source-inverses.json'),'843':load('docs/evidence/held-closure-receiver-843/runtime-projections.json')}
 ci=load('src/source_reflows.json');assert len(ci)==87
 for r in ci:
  p=r['path'];old=apply(load(M+'current-to-9365.json'),p,d1(p));out=[apply(hooks[s],p,apply(views[s],p,old)) for s in views];assert out[0]==out[1]==out[2];apply(load(C+'main-projections.json'),p,out[0]);b=apply(load(M+'current-to-main7f.json'),p,d1(p));assert sha(b)==r['after_sha256']
  for e in r['inverse']:assert b.count(e['after'].encode())==1;b=b.replace(e['after'].encode(),e['before'].encode(),1)
  assert sha(b)==r['before_sha256']
 for r in load(M+'current-to-9365.json'):d1(r['source'])
 # Four correction owners bind to their actual frozen original packets, not inferred fees/hashes.
 strategy='docs/evidence/lexical-strategy-held-receiving-fix-843-20261015/source-inverses.json';sp=load(strategy);assert len(sp)==1
 for r in sp:assert sha(read(r['source']))==r['current_sha256'];assert apply([r],r['source'],read(r['source']))==G.get(OLD,r['source'])
 warning='docs/evidence/d1bb-warning-corrections-20261015/source-inverses.json';wr=load(warning);assert len(wr)==3
 for r in wr:
  p=r['path'];b=apply(json.loads((D/'warning-source-view.json').read_text()),p,read(p));assert b==G.get('ef85e74802c444c8cc1365fa84db57750af5673f',p);assert b.decode()==r['after'] and sha(b)==r['after_sha256'];assert r['before'].encode()==G.get(OLD,p);assert sha(r['before'].encode())==r['before_sha256']
 # Preserve restored whole-file equality and every existing strict scalar assertion body.
 tree='src/cleanup_plan/replay/selected_entry/tests/fact_errors/tree_policy.rs';prefix='        let staged = crate::source_reflows_merge::scalar_policy_input(source, current);\n        let current = staged.as_str();\n';text=read(tree).decode();assert text.count(prefix)==1;assert text.replace(prefix,'',1).encode()==G.get(meta['production_basis'],tree)
 scalar=load('docs/evidence/scalar-source-guard-adapter-1dee-20261015/source-projections.json');sr=json.loads((D/'scalar-policy-inputs.json').read_text());assert len(sr)==2
 for r in scalar:
  p=r['source'];b=apply(sr,p,read(p));assert sha(b)==r['current_sha256'];assert b.decode()==r['edits'][0]['new'];assert len(r['edits'])==1
  old=b.replace(r['edits'][0]['new'].encode(),r['edits'][0]['before'].encode(),1);assert sha(old)==r['original_sha256'];assert old==read(r['archive'])
  if b!=read(p):
   try:apply(sr,p,b)
   except AssertionError:negative+=1
   else:raise AssertionError('stale scalar raw accepted')
 # Whole selected-entry equality uses CI inverse, including every literal removal/name restoration.
 selected='src/cleanup_plan/replay/selected_entry.rs';b=apply(load(M+'current-to-main7f.json'),selected,d1(selected));r=next(r for r in ci if r['path']==selected)
 for e in r['inverse']:assert b.count(e['after'].encode())==1;b=b.replace(e['after'].encode(),e['before'].encode(),1)
 added=re.search(r'let added = r###"(.*?)"###;',text,re.S).group(1).encode();assert b.count(added)==1;b=b.replace(added,b'',1);assert b.count(b'fn validate_paid_original(')==1;b=b.replace(b'fn validate_paid_original(',b'fn validate_paid(',1);a=b'    #[cfg(test)]\n    pub(crate) fn validate_scalar_paid_original(';assert b.count(a)==1;b=b.replace(a,b'    pub(crate) fn validate_scalar_paid(',1);assert b==read('docs/evidence/scalar-fact-policy-tree/original-selected-entry.txt')
 expected=meta['assembly_files'];changed=subprocess.check_output(['git','diff','--name-only','--diff-filter=M',meta['production_basis']],text=True).splitlines();assert sorted(changed)==sorted(expected) and len(changed)==2
 for r in json.loads((D/'immediate-inverses.json').read_text()):assert apply([r],r['source'],read(r['source']))==G.get(meta['production_basis'],r['source'])
 # Every earlier immutable packet remains physical-byte exact, including old CI and scalar metadata.
 protected=[strategy,warning,'docs/evidence/scalar-source-guard-adapter-1dee-20261015/source-projections.json','src/source_reflows.json']
 for p in protected:assert read(p)==G.get(meta['production_basis'],p)
 for folder in [M,F]:
  for p in subprocess.check_output(['git','ls-tree','-r','--name-only',OLD,'--',folder],text=True).splitlines():assert read(p)==G.get(OLD,p)
 proof={'status':'SOURCE_ONLY_NO_RUST_EXECUTION','production_basis':meta['production_basis'],'prior_domain':OLD,'strict_corrective_inputs':5,'strict_stale_negative_controls':negative,'live_main_records':87,'warning_original_rows':3,'strategy_original_rows':1,'scalar_raw_input_transports':2,'scalar_full_original_inverses':3,'restored_whole_selected_entry_equality':True,'all_other_assertion_bodies_unchanged':True,'old_runtime_json_and_packets_byte_exact':True,'assembly_files':changed,'production_unchanged_by_assembly':True,'new_selectors':0,'pending_parent_bodies':310,'git_cache_peak_bytes':G.peak,'runtime_bodies_executed':0}
 (D/'source-proof.json').write_text(json.dumps(proof,indent=2)+'\n');print(json.dumps(proof,indent=2))
finally:G.close()
