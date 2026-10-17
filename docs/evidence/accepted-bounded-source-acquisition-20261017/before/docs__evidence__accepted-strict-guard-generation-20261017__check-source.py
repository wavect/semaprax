"""Only finite strict controls; no common/verifier/Rust execution."""
from pathlib import Path
import runpy,json,subprocess,hashlib
R=Path(__file__).resolve().parents[3];D=Path(__file__).resolve().parent
m=runpy.run_path(str(D/'guard_payload.py'));new=m['accepted_guard_extensions'](R)
old=runpy.run_path(str(R/'docs/evidence/main043-independent-custody-20261016/guard-extensions.py'))
allowed=json.loads((D/'current-allowed.json').read_text());count=0
old['verify_recipe']()
def refuses(f):
 global count
 try:f()
 except (AssertionError,ValueError):count+=1;return
 raise AssertionError('hostile guard admitted')
for path in allowed:
 raw=(R/path).read_text();original=subprocess.check_output(['git','show','a82e00cc8:'+path],cwd=R).decode()
 new['verify_guard'](path,raw,original)
 before=subprocess.check_output(['git','show','5f5808c6cf89a29388f25056baee2e7bd5f0dd08:'+path],cwd=R).decode()
 old['verify_guard'](path,before,original)
 archive=D/'before'/(path.replace('/','__')+'.txt');assert archive.read_text()==before
 refuses(lambda:old['verify_guard'](path,raw,original))
 record=new['verify_guard'](path,raw,original);fragment=record['added_fragments'][0]
 for bad in [raw.replace(fragment,'',1),raw+fragment,raw+'include_str!("/foreign.rs"),']:
  refuses(lambda:new['verify_guard'](path,bad,original))
 if path==m['CARRIER']:
  for name in m['SCRATCH']:
   literal='include_str!("../../'+name+'")'
   for bad in [raw+literal+',',raw.replace(literal+',','',1),raw.replace(literal+',','',1)+literal+',']:
    refuses(lambda:new['verify_guard'](path,bad,original))
   first=raw.index(literal);second=raw.index(literal,first+len(literal))
   for at in (first,second):
    missing=raw[:at]+raw[at+len(literal)+1:]
    refuses(lambda:new['verify_guard'](path,missing,original))
    moved=missing.replace('(capacity) => { audit_source_join!(', '(capacity) => { audit_source_join!('+literal+',',1)
    refuses(lambda:new['verify_guard'](path,moved,original))
  previous=json.loads((D/'before/guard-extension-allowed.json').read_text())[path]['maximum']
  added=set(allowed[path]['maximum'])-set(previous)
  for name in added:
   literal='include_str!("../../'+name+'")'
   refuses(lambda:new['verify_guard'](path,raw+literal+',',original))
W=R/'docs/evidence/main043-independent-custody-20261016/verify-or-refresh.py'
s=W.read_text();prefix=s.split("exec(compile(current,str(P),'exec')",1)[0];space={'__file__':str(W)};exec(compile(prefix,str(W),'exec'),space)
assert space['current'].replace(space['guard_new'],space['guard_old']).replace(space['new'],space['old'],1).encode()==space['original']
result={'status':'PASS_SOURCE_ONLY_STRICT_AND_BOOTSTRAP','complete_current_carriers':3,'complete_original_guard_carriers':3,'old_maximums_unchanged':True,'refusals':count,'original_verifier_executed':False,'rust_executed':False}
(D/'source-controls.json').write_text(json.dumps(result,indent=2)+'\n');print(json.dumps(result))
