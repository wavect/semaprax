"""Exact additive include fragments only; never projects a raw source audit."""
from pathlib import Path
from collections import Counter
import difflib,hashlib,json,re,subprocess
R=Path(__file__).resolve().parents[3];D=Path(__file__).resolve().parent;sha=lambda s:hashlib.sha256(s.encode()).hexdigest()
def recipe():
 old=(D/'guard-extension-original/verify-or-refresh.py').read_text();assert old==subprocess.check_output(['git','show','787f84774:docs/evidence/main043-independent-custody-20261016/verify-or-refresh.py'],cwd=R).decode();value=old
 for e in json.loads((D/'guard-extension-recipe-edits.json').read_text()):assert value.count(e['before'])==1;value=value.replace(e['before'],e['after'],1)
 return value
def verify_recipe():assert(D/'verify-or-refresh.py').read_text()==recipe()
def fragments(path,current,original):
 out=[];left=original.splitlines(True);right=current.splitlines(True)
 for op,i,j,u,v in difflib.SequenceMatcher(None,left,right,autojunk=False).get_opcodes():
  if op=='equal':continue
  before=''.join(left[i:j]);after=''.join(right[u:v]);prefix=0
  while prefix<min(len(before),len(after))and before[prefix]==after[prefix]:prefix+=1
  suffix=0
  while suffix<len(before)-prefix and suffix<len(after)-prefix and before[-1-suffix]==after[-1-suffix]:suffix+=1
  assert before[prefix:len(before)-suffix if suffix else None]=='',('non-additive original guard edit',path)
  new=after[prefix:len(after)-suffix if suffix else None]
  grammar=r'(?:\s*include_str!\("[^"\n]+"\),)+\s*'
  if not re.fullmatch(grammar,new):
   # Greedy common-prefix trimming can stop INSIDE a shared include_str path.
   # Recover only a unique complete added macro run whose exact removal
   # restores the entire unchanged old hunk. No original byte is projected.
   delta=len(after)-len(before);candidates=[]
   for match in re.finditer(r'include_str!\(',after):
    lo=match.start();piece=after[lo:lo+delta]
    if delta>0 and re.fullmatch(grammar,piece) and after[:lo]+after[lo+delta:]==before:candidates.append(piece)
   assert len(candidates)==1,('nonunique or foreign additive token run',path)
   new=candidates[0]
  assert re.fullmatch(grammar,new),('foreign additive fragment',path)
  assert current.count(new)==1,('nonunique additive fragment',path);out.append(new)
 return out
def verify_guard(path,current,original,refresh=False):
 allowed=json.loads((D/'guard-extension-allowed.json').read_text())[path];edits=fragments(path,current,original);observed=Counter()
 for e in edits:
  for token in re.findall(r'include_str!\("([^"\n]+)"\)',e):
   base=(R/path).parent if path=='tests/hir_module_boundaries.rs'else(R/path).parent.parent;observed[str((base/token).resolve().relative_to(R))]+=1
 assert all(allowed['required'].get(k,0)<=observed[k]<=v for k,v in allowed['maximum'].items()),('missing or duplicate added include',path)
 assert set(observed)<=set(allowed['maximum']),('foreign added include',path)
 restored=current
 for e in edits:assert restored.count(e)==1;restored=restored.replace(e,'',1)
 assert restored==original,('original raw guard changed',path)
 record=dict(source=path,current_sha256=sha(current),original_sha256=sha(original),added_fragments=edits)
 target=D/'guard-extension-bindings'/Path(path).name
 # Inventory basenames collide, so use the entire repository path as the key.
 target=D/'guard-extension-bindings'/(path.replace('/','__')+'.json')
 if refresh:target.parent.mkdir(parents=True,exist_ok=True);target.write_text(json.dumps(record,indent=2)+'\n')
 else:assert json.loads(target.read_text())==record,('stale complete guard binding',path)
 return record
def controls():
 verify_recipe();count=0
 for path in json.loads((D/'guard-extension-allowed.json').read_text()):
  raw=(R/path).read_text();old=subprocess.check_output(['git','show','a82e00cc8:'+path],cwd=R).decode();record=verify_guard(path,raw,old);first=record['added_fragments'][0]
  for wrong in [raw.replace(first,'',1),raw+first,raw+'include_str!("/foreign.rs"),']:
   try:verify_guard(path,wrong,old)
   except (AssertionError,ValueError):count+=1
   else:raise AssertionError('missing/duplicate/foreign fragment accepted')
 print(json.dumps(dict(status='PASS_SOURCE_ONLY_GUARD_EXTENSIONS',old_full_guards_preserved=3,missing_duplicate_foreign_refused=count,raw_sources_projected=False,rust_executed=False)))
if __name__=='__main__':controls()
