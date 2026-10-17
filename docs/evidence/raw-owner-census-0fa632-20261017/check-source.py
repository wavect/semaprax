"""Bounded complete owner census and unchanged actual raw-guard controls."""
from pathlib import Path
from collections import Counter
import hashlib,json,re,runpy,subprocess
R=Path(__file__).resolve().parents[3];D=Path(__file__).resolve().parent;BASE='0fa63204bc67d3c35245a8634b90c7ecd4984707'
g=lambda *args:subprocess.check_output(['git',*args],cwd=R)
sha=lambda b:hashlib.sha256(b).hexdigest()
packet=json.loads((D/'additions.json').read_bytes());G=R/'docs/evidence/main043-independent-custody-20261016';m=runpy.run_path(str(G/'guard-extensions.py'))
assert (G/'guard-extensions.py').read_bytes()==g('show',BASE+':docs/evidence/main043-independent-custody-20261016/guard-extensions.py')
domains=json.loads(g('show','1746eaaf0:docs/evidence/a3-e47-final-custody-20261016/domains.json'));oldfiles=set(g('ls-tree','-r','--name-only',domains[0]['commit']).decode().splitlines());tracked=set(g('ls-files').decode().splitlines());owners=sorted(f for f in tracked-oldfiles if f.endswith('.rs') and (f.startswith('src/') or f.startswith('tests/wasm/')))
assert len(owners)==159
allow=json.loads((G/'guard-extension-allowed.json').read_bytes());oldallow=json.loads((D/'before/guard-extension-allowed.json').read_bytes());refusals=0;records=[]
for edit in packet['edits']:
 path=edit['source'];raw=(R/path).read_text();before=(R/edit['before_archive']).read_text();assert before.encode()==g('show',BASE+':'+path)
 assert raw.count(edit['addition'])==1 and raw.replace(edit['addition'],'',1)==before
 assert sha(raw.encode())==edit['current_sha256'] and sha(before.encode())==edit['before_sha256']
 original=g('show','a82e00cc8:'+path).decode();m['verify_guard'](path,raw,original)
 assert all(f in raw for f in owners)
 assert [f for f in owners if f not in before]==packet['new_subjects']
 previous=Counter(re.findall(r'include_str!\("([^"\n]+)"\)',before));current=Counter(re.findall(r'include_str!\("([^"\n]+)"\)',raw));tokens=re.findall(r'include_str!\("([^"\n]+)"\)',edit['addition']);assert current==previous+Counter(tokens)
 # The insertion is adjacent to the existing real payload, never the sentinel.
 assert edit['insert_after']+edit['addition'] in raw
 for line in before.splitlines(True):
  if 'α' in line:assert line in raw
 if path.startswith('crates/'):assert len(raw.splitlines())==1500
 for field in ('maximum','required'):
  restored=dict(allow[path][field])
  for source in packet['new_subjects']:assert restored.pop(source)==1
  assert restored==oldallow[path][field]
 for token in tokens:
  literal='include_str!("'+token+'"),'
  assert raw.count(literal)==1
  for bad in (raw.replace(literal,literal+literal,1),raw.replace(literal,'include_str!("/foreign-owner.rs"),',1)):
   try:m['verify_guard'](path,bad,original)
   except (AssertionError,ValueError):refusals+=1
   else:raise AssertionError('hostile raw include admitted')
 records.append({'source':path,'complete_original_retained':True,'all_prior_include_counts_retained':True,'new_actual_payload_includes':2,'before_missing':packet['new_subjects'],'after_missing':[],'lines':len(raw.splitlines())})
assert set(allow)==set(oldallow)
result={'status':'PASS_SOURCE_ONLY_NO_COMMON_OR_RUST_EXECUTION','historical_census_base':domains[0]['commit'],'census_subjects':len(owners),'guards':records,'hostile_actual_verifier_refusals':refusals,'new_subjects':[{ 'source':f,'length':len((R/f).read_bytes()),'sha256':sha((R/f).read_bytes())} for f in packet['new_subjects']],'unchanged_guard_parser_sha256':sha((G/'guard-extensions.py').read_bytes())}
(D/'source-controls.json').write_text(json.dumps(result,indent=2)+'\n');print(json.dumps({k:v for k,v in result.items() if k not in ('guards','new_subjects')}))
