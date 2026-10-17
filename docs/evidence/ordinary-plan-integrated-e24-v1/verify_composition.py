"""Finite source-only six-stage composition and actual three-scope union proof."""
from pathlib import Path
from collections import Counter
import hashlib,json,re,subprocess,runpy,copy
R=Path(__file__).resolve().parents[3];D=Path(__file__).resolve().parent
E24='e24e99787e8dec6cf5aed2930ff909cedc187f97';C874='c8740045e33126a4375aadd1dc524a25be9245fc'
LIMIT=2*1024*1024
h=lambda text:hashlib.sha256(text.encode()).hexdigest()
def read(path):
 with (R/path).open('rb') as stream:data=stream.read(LIMIT+1)
 assert len(data)<=LIMIT,path
 return data.decode()
def git(commit,path):return subprocess.check_output(['git','show',commit+':'+path],cwd=R).decode()
def load(text):
 def unique(pairs):
  result={}
  for key,value in pairs:assert key not in result;result[key]=value
  return result
 return json.loads(text,object_pairs_hook=unique)
def refused(fn):
 try:fn()
 except (AssertionError,KeyError,ValueError,IndexError):return True
 return False
def edge(row,text,name):
 item=row[name];assert h(text)==item['after_sha256'];value=text.encode();last=len(value)
 for edit in item['inverse']:
  start=edit['start_byte'];after=edit['after'].encode();end=start+len(after)
  assert 0<=start<=end<=last and value[start:end]==after
  value=value[:start]+edit['before'].encode()+value[end:];last=start
 value=value.decode();assert h(value)==item['before_sha256'];return value
legacy_path='docs/evidence/ordinary-plan-history-c8740045-v1/inputs.json'
# All old artifacts, including every complete old inverse/manifest checkpoint, stay exact.
old_artifacts=[legacy_path,'docs/evidence/ordinary-plan-history-c8740045-v1/root-inverse.json',
 'docs/evidence/ordinary-plan-history-c8740045-v1/replay.rs.before.txt','docs/evidence/ordinary-plan-history-c8740045-v1/manifest.json']
for path in old_artifacts:assert read(path)==git(E24,path)
legacy=load(read(legacy_path));rows=load(read('docs/evidence/ordinary-plan-integrated-e24-v1/stages.json'))
assert len(rows)==len(legacy)==6 and [row['path'] for row in rows]==[row['path'] for row in legacy]
helper=read('src/source_reflows_merge/ordinary_plan_inputs.rs');raw=read('docs/evidence/ordinary-plan-integrated-e24-v1/stages.json')
assert f'const INTEGRATED_LENGTH: usize = {len(raw.encode())};'in helper
assert f'const INTEGRATED_SHA256: &str = "{h(raw)}";'in helper
negative=0;stage_proof=[]
def acquire(row,old,expected,current,physical):
 if expected!=old['before_sha256']:return current
 assert h(physical)==row['current_sha256'] and len(physical.encode())==row['current_length']
 checkpoint=edge(row,physical,'current_to_e24');assert checkpoint==row['e24']
 prior=edge(row,checkpoint,'e24_to_legacy');assert prior==row['legacy'] and h(prior)==old['after_sha256']
 assert current in [physical,checkpoint,prior,old['before']]
 for change in old['inverse']:
  assert prior.count(change['after'])==1
  prior=prior.replace(change['after'],change['before'],1)
 assert prior==old['before'] and h(prior)==old['before_sha256'];return prior
for row,old in zip(rows,legacy):
 path=row['path'];physical=read(path);expected=old['before_sha256']
 assert row['expected_sha256']==expected and row['e24']==git(E24,path)
 for stage in [physical,row['e24'],row['legacy'],old['before']]:assert acquire(row,old,expected,stage,physical)==old['before']
 for expected_other,current in [(h(physical),physical),('older selected namespace','older source stage')]:assert acquire(row,old,expected_other,current,physical)==current
 for bad in ['',physical+' ',physical+physical,'foreign']:
  assert refused(lambda:acquire(row,old,expected,bad,physical));negative+=1
  assert refused(lambda:acquire(row,old,expected,physical,bad));negative+=1
 for name in ['current_to_e24','e24_to_legacy']:
  bad=copy.deepcopy(row);bad[name]['before_sha256']='0'*64
  assert refused(lambda:acquire(bad,old,expected,physical,physical));negative+=1
  if len(row[name]['inverse'])>1:
   bad=copy.deepcopy(row);bad[name]['inverse'].reverse()
   assert refused(lambda:acquire(bad,old,expected,physical,physical));negative+=1
 stage_proof.append({'path':path,'current':h(physical),'e24':h(row['e24']),'legacy':h(row['legacy']),'c874':expected})
# Exact old direct compile35f body returns to its original seven-source scope.
main=read('src/source_reflows_merge/main67107_inputs.rs');old_main=git(C874,'src/source_reflows_merge/main67107_inputs.rs')
def body(text,name,next_name):return text.split(name,1)[1].split(next_name,1)[0]
assert body(main,'pub(crate) fn compile35f_input','pub(super) fn compile35f_expected_input')==body(old_main,'pub(crate) fn compile35f_input','pub(super) fn compile35f_expected_input')
assert 'ordinary_plan_inputs::acquire(source, current)'not in main
assert main.count('ordinary_plan_inputs::acquire_expected(source, expected, current)')==1
# The retained original recipe is complete, preceded only by explicit finite delegation.
recipe='docs/evidence/ordinary-plan-history-c8740045-v1/bind_sources.py';assert read(recipe).endswith(git(E24,recipe))
manifest=load(read('docs/evidence/ordinary-plan-integrated-e24-v1/current-manifest.json'))
owners=[item['path'] for item in manifest['files']];assert len(owners)==len(set(owners))==9
for item in manifest['files']:assert h(read(item['path']))==item['sha256'] and item['lines']<=1500
G=R/'docs/evidence/main043-independent-custody-20261016';guard=runpy.run_path(str(G/'guard-extensions.py'))
allowed_path='docs/evidence/main043-independent-custody-20261016/guard-extension-allowed.json'
allowed=load(read(allowed_path));assert read(allowed_path)==git(E24,allowed_path)
baseline=load(git(C874,allowed_path));main21=['src/cleanup_plan/replay/expression_facts/receiving/helpers/matching.rs',
 'src/cleanup_plan/replay/expression_facts/receiving/helpers/matching/tests.rs','src/source_reflows/incoming21_semantic_receivers.rs']
raw_proof=[]
for path,rule in allowed.items():
 current=read(path);assert current==git(E24,path),'never project or mutate actual raw scopes'
 original=git('a82e00cc8',path);fragments=guard['fragments'](path,current,original)
 base=(R/path).parent if path=='tests/hir_module_boundaries.rs'else(R/path).parent.parent;counts=Counter()
 for fragment in fragments:
  for literal in re.findall(r'include_str!\("([^"\n]+)"\)',fragment):counts[(base/literal).resolve().relative_to(R).as_posix()]+=1
 for kind in ['required','maximum']:
  for key,value in baseline[path][kind].items():assert rule[kind][key]==value
  assert set(rule[kind])==set(baseline[path][kind])|set(main21)|set(owners)
 for owner in owners+main21:assert rule['required'][owner]==rule['maximum'][owner]==counts[owner]>0
 assert all(counts[owner]==1 for owner in owners)
 scope=2 if path=='tests/hir_module_boundaries.rs'else 1 if path.startswith('tests/workspace')else 3
 assert [counts[owner]for owner in main21]==[scope,scope,1]
 if 'ledger_capacity'in path:assert len(current.splitlines())==1500
 guard['verify_guard'](path,current,original,False)
 # New exact additions must fail on missing, duplicate and unknown current raw source tokens.
 for owner in owners+main21:
  token=next(m.group()for m in re.finditer(r'include_str!\("([^"\n]+)"\),',current)if(base/m.group(1)).resolve()==R/owner)
  for bad in [current.replace(token,'',1),current+token]:
   assert refused(lambda:guard['verify_guard'](path,bad,original,False));negative+=1
 raw_proof.append({'path':path,'current_sha256':h(current),'original_a82e_sha256':h(original),
  'native_1500':len(current.splitlines())==1500 if 'ledger_capacity'in path else None,
  'old_required_entries':len(baseline[path]['required']),'old_maximum_entries':len(baseline[path]['maximum']),
  'new_exact_counts':{owner:counts[owner]for owner in owners+main21}})
assert refused(lambda:load('{"x":1,"x":2}'));negative+=1
result={'status':'PASS_SOURCE_ONLY_FINITE_EXPECTED_NAMESPACE_COMPOSITION','base':E24,
 'six_complete_stages':stage_proof,'actual_raw_scopes_unchanged':raw_proof,'negative_controls':negative,
 'retained_original_artifacts':old_artifacts,'Cargo':False,'Rust_tests':False,'language_runtime':False,
 'script_sha256':h(read('docs/evidence/ordinary-plan-integrated-e24-v1/verify_composition.py'))}
(D/'proof.json').write_text(json.dumps(result,indent=2)+'\n')
print(json.dumps({k:v for k,v in result.items()if k not in ['six_complete_stages','actual_raw_scopes_unchanged','retained_original_artifacts']}))
