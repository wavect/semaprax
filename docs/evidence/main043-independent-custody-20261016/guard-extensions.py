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
 # Macro bodies are indivisible. Non-macro characters remain exact anchors;
 # whitespace between anchors can belong to an added run only if removing
 # that complete run retains the exact old whitespace (not a normalized view).
 macro=re.compile(r'include_str!\("[^"\n]+"\),')
 grammar=r'(?:\s*include_str!\("[^"\n]+"\),)+\s*'
 def tokens(text):
  end=0
  for match in macro.finditer(text):
   for character in text[end:match.start()]:yield(False,character)
   yield(True,match.group());end=match.end()
  for character in text[end:]:yield(False,character)
 def anchors(text):
  space=[]
  for token in tokens(text):
   if not token[0] and token[1].isspace():space.append(token[1]);continue
   yield(''.join(space),token);space=[]
  yield(''.join(space),None)
 def addition(before,after):
  if before==after:return
  delta=len(after)-len(before);assert delta>0,('non-additive original guard edit',path)
  # A gap contains only old whitespace and new complete macros. Choose the
  # longest exact retained prefix; every candidate restores the same raw gap.
  # This resolves whitespace ownership without splitting any macro token.
  for prefix in range(len(before),-1,-1):
   new=after[prefix:prefix+delta]
   if after[:prefix]+after[prefix+delta:]==before and re.fullmatch(grammar,new):
    out.append(new);return
  raise AssertionError(('foreign additive fragment or changed original whitespace',path))
 left=iter(anchors(original));space,expected=next(left);out=[];added=[]
 for token in tokens(current):
  if not token[0] and token[1].isspace():added.append(token[1]);continue
  if token==expected:
   addition(space,''.join(added));added=[];space,expected=next(left)
  else:
   assert token[0],('non-additive original guard edit',path)
   added.append(token[1])
 assert expected is None,('missing original guard token',path)
 addition(space,''.join(added));restored=current
 for new in out:
  assert re.fullmatch(grammar,new),('foreign additive fragment',path)
  assert current.count(new)==1,('nonunique additive fragment',path)
  assert restored.count(new)==1,('overlapping additive fragment',path)
  restored=restored.replace(new,'',1)
 assert restored==original,('original raw guard changed',path)
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
