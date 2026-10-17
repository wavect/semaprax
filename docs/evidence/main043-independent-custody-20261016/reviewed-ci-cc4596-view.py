"""Closed cc4596 -> 3d5 CI input view; execute the complete reviewed-b452 proof.

Only the historical equality receives a distinct SSD input directory. Actual CI
scripts, their executions, and raw source audits continue to read the repository.
"""
import hashlib, json, runpy, tempfile
from pathlib import Path
INPUT_LENGTH = 32738
INPUT_SHA = '09f6b120b1e2634d64fc5211dd21872ba92e880bc7279a43118b816057e0ef54'
OLD_HELPER_SHA = '65bebec36af234e9860777e986ad4563ca24db02ce7a6266403168a241df770f'
PATHS = ("scripts/ci-low-stack-trace.py", "scripts/tests/test_ci_low_stack_trace.py")
WORKFLOW = ".github/workflows/ci.yml"
def sha(raw):
 return hashlib.sha256(raw).hexdigest()
def restore(row, current):
 assert row['path'] in PATHS
 raw=current.encode('utf-8')
 assert len(raw)<=1048576 and sha(raw)==row['current_sha256'] and current==row['current']
 lines=current.splitlines(keepends=True);last=len(lines)
 assert len(row['inverse'])<=64
 for edit in reversed(row['inverse']):
  start,end=edit['start'],edit['end']
  assert type(start) is int and type(end) is int and 0<=start<=end<=last and end<=len(lines)
  assert ''.join(lines[start:end])==edit['current']
  lines[start:end]=edit['previous'].splitlines(keepends=True);last=start
 result=''.join(lines)
 assert sha(result.encode('utf-8'))==row['old_sha256']
 return result
def historical(repo, path, current):
 assert path in PATHS+(WORKFLOW,)
 # read_bytes is intentional: CRLF must not normalize through outer read_text.
 assert (Path(repo)/path).read_bytes().decode('utf-8')==current
 d=Path(__file__).parent
 old_helper=d/'reviewed-ci-view.py'
 assert sha(old_helper.read_bytes())==OLD_HELPER_SHA
 old=runpy.run_path(str(old_helper))
 if path==WORKFLOW:
  return old['historical'](repo,path,current)
 data=d/'reviewed-ci-cc4596-outer-view';raw=(data/'inputs.json').read_bytes()
 assert len(raw)==INPUT_LENGTH<=2097152 and sha(raw)==INPUT_SHA
 rows=json.loads(raw)
 assert len(rows)==2 and tuple(row['path'] for row in rows)==PATHS
 row=rows[PATHS.index(path)]
 assert row['archive']==str(PATHS.index(path))+'.3d5-before.py.txt'
 previous=(data/row['archive']).read_bytes()
 assert sha(previous)==row['old_sha256']
 result=restore(row,current)
 assert result.encode('utf-8')==previous
 assert row['b452_archive']==str(PATHS.index(path))+'.b452-before.py.txt'
 b452=(d/'reviewed-ci-3d5-historical-view'/row['b452_archive']).read_bytes()
 assert sha(b452)==row['b452_sha256']
 # The original helper is byte-exact and retains its complete original assertions.
 # Give it a real, explicitly historical 3d5 file, never a fake raw-current value.
 with tempfile.TemporaryDirectory(prefix='main043-reviewed-ci-3d5-',dir='/Volumes/Semaprax/runtime-tmp') as historical_root:
  subject=Path(historical_root)/path;subject.parent.mkdir(parents=True)
  subject.write_bytes(previous)
  reviewed=old['historical'](historical_root,path,result)
 assert reviewed.encode('utf-8')==b452
 return reviewed
