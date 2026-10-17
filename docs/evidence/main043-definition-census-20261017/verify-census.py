"""Closed current-definition census only; no binder/refresh/compiler execution."""
from pathlib import Path
import json,hashlib,copy,subprocess,os
R=Path(__file__).resolve().parents[3];D=Path(__file__).resolve().parent
TABLE='docs/evidence/main043-independent-custody-20261016/definition-bindings.json'
SOURCE='src/source_reflows/main043/semantic_receivers.rs'
sha=lambda raw:hashlib.sha256(raw).hexdigest()
def unique(pairs):
 result={}
 for k,v in pairs:
  assert k not in result,('duplicate JSON key',k)
  result[k]=v
 return result
def refused(f):
 try:f()
 except (AssertionError,KeyError,ValueError):return True
 return False
before_raw=(D/'definition-bindings.json.before').read_bytes();assert sha(before_raw)=='8f6e7a2c069366398de3e868533d6989da16ccdb425461d53873aa342a9bd829'
raw=(R/TABLE).read_bytes();assert sha(raw)=='e32604159e0d9e668957214416a1b1042fb2097741ddae20b1bd95cc683c5952'
old=json.loads(before_raw,object_pairs_hook=unique);actual=json.loads(raw,object_pairs_hook=unique)
paths=[v['path']for v in old['definitions']];assert len(paths)==len(set(paths))==12
old_source=(D/'semantic_receivers.rs.before.txt').read_bytes();new_source=(D/'semantic_receivers.rs.current.txt').read_bytes()
assert len(old_source)==4318 and sha(old_source)=='2ef2eb1aa9ceff7702f991a45b42c87d397936dca612d2e8ce25f93bbdd4d09d'
assert len(new_source)==6160 and sha(new_source)=='4bc99dc2b93442b83cb679e3c0e108f3b50121d90e8aee4ceed01296e7ec33b5'
assert (R/SOURCE).read_bytes()==new_source
index=paths.index(SOURCE)
def validate(table,overrides=None):
 assert set(table)==set(old)
 assert {k:v for k,v in table.items()if k!='definitions'}=={k:v for k,v in old.items()if k!='definitions'}
 rows=table['definitions'];assert len(rows)==12 and [v['path']for v in rows]==paths
 for i,row in enumerate(rows):
  assert set(row)==set(old['definitions'][i])=={'path','bytes','sha256'}
  if i!=index:assert row==old['definitions'][i]
  payload=(overrides or {}).get(row['path'],(R/row['path']).read_bytes())
  # The original complete-current definition contract, retained unchanged.
  assert row['bytes']==len(payload) and row['sha256']==sha(payload)
validate(actual)
inverted=copy.deepcopy(actual);inverted['definitions'][index]=old['definitions'][index]
assert json.dumps(inverted,indent=2).encode()+b'\n'==before_raw
negative=0
for row in actual['definitions']:
 payload=(R/row['path']).read_bytes()
 for changed in [payload+b' ',b'!'+payload[1:]]:
  assert refused(lambda:validate(actual,{row['path']:changed}));negative+=1
 for field,value in [('bytes',row['bytes']+1),('sha256','0'*64)]:
  changed=copy.deepcopy(actual);next(x for x in changed['definitions']if x['path']==row['path'])[field]=value
  assert refused(lambda:validate(changed));negative+=1
for kind in ['missing','duplicate','reorder','foreign','extra_field','stale_table','metadata']:
 changed=copy.deepcopy(actual)
 if kind=='missing':changed['definitions'].pop()
 elif kind=='duplicate':changed['definitions'].append(changed['definitions'][0])
 elif kind=='reorder':changed['definitions'].reverse()
 elif kind=='foreign':changed['definitions'][index]['path']='foreign.rs'
 elif kind=='extra_field':changed['definitions'][index]['unreviewed']=True
 elif kind=='stale_table':changed=old
 else:changed['verification_script_sha256']='0'*64
 assert refused(lambda:validate(changed));negative+=1
assert refused(lambda:json.loads('{"path":1,"path":2}',object_pairs_hook=unique));negative+=1
cuts=json.loads((D/'definition-source-inverse.json').read_text());lines=new_source.decode().splitlines(True);last=len(lines)
for e in reversed(cuts['line_inverse']):
 a,z=e['start'],e['end'];assert 0<=a<=z<=last<=len(lines)and ''.join(lines[a:z])==e['current'];lines[a:z]=e['previous'].splitlines(True);last=a
assert ''.join(lines).encode()==old_source
verifier='docs/evidence/main043-independent-custody-20261016/verify-or-refresh.py'
base=subprocess.check_output(['git','show','197e7814c86f144804eaa65ef1143828c4fd39bd:'+verifier],cwd=R,env=dict(os.environ,GIT_OPTIONAL_LOCKS='0'))
assert base==(R/verifier).read_bytes()
print(json.dumps({'status':'PASS_TARGETED_SOURCE_ONLY_DEFINITION_CENSUS','definition_rows':12,'updated_rows':1,'unchanged_ordered_rows':11,'hostile_controls':negative,'whole_old_table_restored':True,'whole_old_definition_restored':True,'original_verifier_whole_source_unchanged':True,'new_Rust_selectors':0,'common_refresh_run':False,'Cargo_or_runtime_run':False}))
