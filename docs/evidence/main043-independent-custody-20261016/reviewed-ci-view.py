"""Exact reviewed-b452 CI view; current CI code/tests remain actual raw inputs."""
import hashlib,json
from pathlib import Path
INPUT_LENGTH = 20077
INPUT_SHA = '1310fb32e1549ee3fcc0005587556f7f8fe9efefaddedd7cfc5443bc444d15f8'
PREFIX = "reviewed-ci-3d5-historical-view"
PATHS = ("scripts/ci-low-stack-trace.py", "scripts/tests/test_ci_low_stack_trace.py")
WORKFLOW = ".github/workflows/ci.yml"
def sha(b):return hashlib.sha256(b).hexdigest()
def restore(row,current):
 assert row['path'] in PATHS
 assert len(current.encode())<=1048576 and sha(current.encode())==row['current_sha256']
 assert current==row['current'];edits=row['inverse'];assert len(edits)<=64
 lines=current.splitlines(keepends=True);last=len(lines)
 for e in reversed(edits):
  a,z=e['start'],e['end'];assert 0<=a<=z<=last and z<=len(lines)
  assert ''.join(lines[a:z])==e['current'];lines[a:z]=e['previous'].splitlines(keepends=True);last=a
 result=''.join(lines);assert sha(result.encode())==row['old_sha256'];return result
def historical(repo,path,current):
 assert path in PATHS+(WORKFLOW,)
 assert current==(Path(repo)/path).read_text()
 if path==WORKFLOW:return current
 d=Path(__file__).parent/PREFIX;b=(d/'inputs.json').read_bytes()
 assert len(b)==INPUT_LENGTH<=2097152 and sha(b)==INPUT_SHA
 rows=json.loads(b);assert len(rows)==2 and tuple(r['path']for r in rows)==PATHS
 row=next(r for r in rows if r['path']==path);assert row['archive']==str(PATHS.index(path))+'.b452-before.py.txt'
 original=(d/row['archive']).read_text();assert sha(original.encode())==row['old_sha256']
 result=restore(row,current);assert result==original
 for bad in ['',current+' ',current+current,'!'+current[1:]]:
  try:restore(row,bad)
  except AssertionError:pass
  else:raise AssertionError('changed CI input admitted')
 forged=dict(row);forged['old_sha256']='0'*64
 try:restore(forged,current)
 except AssertionError:pass
 else:raise AssertionError('forged old CI admitted')
 return result
