"""Bounded source-only registration controls; never invokes Rust or a refresh."""
from pathlib import Path
from collections import Counter
import copy, hashlib, json, re, runpy, subprocess
R=Path(__file__).resolve().parents[3]
D=Path(__file__).resolve().parent
G=R/'docs/evidence/main043-independent-custody-20261016'
BASE='a75a82f035e401d372fb4714418334717042e99c'
LIMIT=2*1024*1024
sha=lambda text:hashlib.sha256(text.encode()).hexdigest()
def read(path):
 with (R/path).open('rb') as stream: data=stream.read(LIMIT+1)
 assert len(data)<=LIMIT,('bounded source',path)
 return data.decode()
def unique(pairs):
 result={}
 for key,value in pairs:
  assert key not in result,('duplicate JSON field',key)
  result[key]=value
 return result
def load(text):return json.loads(text,object_pairs_hook=unique)
def git(path,base=BASE):return subprocess.check_output(['git','show',base+':'+path],cwd=R).decode()
def refused(fn):
 try:fn()
 except (AssertionError,KeyError,ValueError):return True
 return False
allowed_path='docs/evidence/main043-independent-custody-20261016/guard-extension-allowed.json'
old=load(git(allowed_path));allowed=load(read(allowed_path))
manifest=load(read('docs/evidence/ordinary-plan-history-c8740045-v1/manifest.json'))
owners=tuple(row['path'] for row in manifest['files'])
assert len(owners)==9 and len(set(owners))==9
for row in manifest['files']:assert sha(read(row['path']))==row['sha256']
helper=runpy.run_path(str(G/'guard-extensions.py'))
def observed(path,current,original):
 fragments=helper['fragments'](path,current,original)
 base=(R/path).parent if path=='tests/hir_module_boundaries.rs' else (R/path).parent.parent
 counts=Counter()
 for fragment in fragments:
  for literal in re.findall(r'include_str!\("([^"\n]+)"\)',fragment):
   counts[(base/literal).resolve().relative_to(R).as_posix()]+=1
 return counts
def exact(path,entry,counts):
 for kind in ('required','maximum'):
  assert set(entry[kind])==set(old[path][kind])|set(owners),('only the nine new registrations',path,kind)
  for key,value in old[path][kind].items():assert entry[kind][key]==value,('original entry changed',path,kind,key)
 for owner in owners:
  assert owner not in old[path]['required'] and owner not in old[path]['maximum']
  assert counts[owner]>0
  assert entry['required'][owner]==entry['maximum'][owner]==counts[owner],('exact current scope count',path,owner)
proof=[];negatives=0
for path,entry in allowed.items():
 current=read(path);original=git(path,'a82e00cc8')
 counts=observed(path,current,original);exact(path,entry,counts)
 helper['verify_guard'](path,current,original,False)
 for owner in owners:
  for kind in ('required','maximum'):
   for value in (0,counts[owner]+1):
    bad=copy.deepcopy(entry);bad[kind][owner]=value
    assert refused(lambda:exact(path,bad,counts));negatives+=1
   bad=copy.deepcopy(entry);del bad[kind][owner]
   assert refused(lambda:exact(path,bad,counts));negatives+=1
  # Exercise the original complete guard on actual missing/duplicate include text.
  base=(R/path).parent if path=='tests/hir_module_boundaries.rs' else (R/path).parent.parent
  literal=next(m.group() for m in re.finditer(r'include_str!\("([^"\n]+)"\)',current)
      if (base/m.group(1)).resolve()==R/owner)
  for bad in (current.replace(literal+',','',1), current+literal+','):
   assert refused(lambda:helper['verify_guard'](path,bad,original,False));negatives+=1
 bad=copy.deepcopy(entry);bad['required']['src/foreign.rs']=1;bad['maximum']['src/foreign.rs']=1
 assert refused(lambda:exact(path,bad,counts));negatives+=1
 previous=next(iter(old[path]['required']));bad=copy.deepcopy(entry);bad['required'][previous]+=1
 assert refused(lambda:exact(path,bad,counts));negatives+=1
 if 'ledger_capacity' in path:assert len(current.splitlines())==1500
 proof.append({'carrier':path,'before_sha256':sha(git(path)),'current_sha256':sha(current),
   'original_a82e_sha256':sha(original),'physical_lines':len(current.splitlines()),
   'prior_required_entries':len(old[path]['required']),'prior_maximum_entries':len(old[path]['maximum']),
   'owners':{owner:{'observed':counts[owner],'required':entry['required'][owner],'maximum':entry['maximum'][owner]} for owner in owners}})
assert refused(lambda:load('{"a":1,"a":2}'));negatives+=1
# Every changed original plus all three complete raw carriers is reconstructible.
packet=load(read('docs/evidence/ordinary-plan-history-registration-a75-v1/inverses.json'))
for entry in packet:
 record=read('docs/evidence/ordinary-plan-history-registration-a75-v1/'+entry['record'])
 assert len(record.encode())==entry['length'] and sha(record)==entry['sha256']
 row=load(record);current=read(row['path']);assert sha(current)==row['after_sha256']
 last=len(current)
 for edit in row['inverse']:
  start=edit['start_character'];end=start+len(edit['after'])
  assert 0<=start<=end<=last and current[start:end]==edit['after']
  current=current[:start]+edit['before']+current[end:];last=start
 assert current==row['before']==git(row['path']) and sha(current)==row['before_sha256']
result={'verification_script_sha256':sha(read('docs/evidence/ordinary-plan-history-registration-a75-v1/verify_registration.py')),'status':'PASS_SOURCE_ONLY_NINE_PLAN_OWNERS_THREE_RAW_SCOPES','base':BASE,
 'new_physical_Rust_owners':9,'raw_carriers':proof,'negative_controls':negatives,
 'whole_before_inverse_records':len(packet),'Cargo':False,'Rust_or_language_runtime':False,
 'existing_required_and_maximum_entries_unchanged':True,'original_a82e_raw_bytes_retained':True}
(D/'proof.json').write_text(json.dumps(result,indent=2)+'\n')
print(json.dumps({key:value for key,value in result.items() if key!='raw_carriers'}))
