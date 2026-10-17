"""Pure controls only: never invoke the binder or its generated program."""
from pathlib import Path
import ast,hashlib,json,runpy
D=Path(__file__).resolve().parent
x=runpy.run_path(str(D/'bind-next.py'),run_name='pure_source_controls')
raw=x['held'](D/'bind_current.before.py.txt',x['BINDER_LEN'],x['BINDER_SHA'])
ast.parse(x['render'](raw))
m=json.loads(x['held'](D/'input-domains.d2bff.json',x['BEFORE_LEN'],x['BEFORE_SHA']))
prior={z['path']:(z['current_len'],z['current_sha256'])for z in m['physical']}
physical=dict(prior);physical.update(x['RAW3']);x['guard_physical'](m,physical)
refusals=0
def refuse(call):
 global refusals
 try:call()
 except(AssertionError,ValueError):refusals+=1
 else:raise AssertionError('foreign input accepted')
for p in x['RAW3']:
 for pair in [(0,'0'*64),(physical[p][0]+1,physical[p][1])]:
  bad=dict(physical);bad[p]=pair;refuse(lambda:x['guard_physical'](m,bad))
p=next(k for k in physical if k not in x['RAW3']);bad=dict(physical);bad[p]=(0,'0'*64)
refuse(lambda:x['guard_physical'](m,bad))
for b in [raw[:-1],raw+b'\n',raw.replace(b'LIMIT=9464242',b'LIMIT=9464243')]:refuse(lambda:x['render'](b))
print(json.dumps({'status':'PASS_SOURCE_ONLY_NOT_BINDER_EXECUTED','whole_original_binder_inverse':True,'physical_rows':174,'permitted_changed_rows':3,'refusals':refusals}))
