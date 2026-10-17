"""Closed successor-stage extension of the complete original endpoint binder.
No runtime/pins. Run explicitly only during the later parent common rebind.
"""
from pathlib import Path
import hashlib,json,importlib,sys
D=Path(__file__).resolve().parent;R=D.parents[2]
sha=lambda b:hashlib.sha256(b).hexdigest()
DATA_SHA="41327f4c8e236917741b66131247a526792aa641f027ddd342fc3bce72f47ba2"
ORIGINAL_SHA="d07eb7b76220b90ef37778a9998251ac46ad32bed32ac2f2232adc7988012a8f"
def _singleton_canonical_stage(repo,raw):
 data=(D/'source-acquisition.json').read_bytes();assert sha(data)==DATA_SHA
 row=json.loads(data)[0];assert row['source']=='src/cleanup_plan/replay/factored/canonical_shared.rs'
 assert sha(raw)==row['current_sha256']
 text=raw.decode()
 for edit in row['inverse']:
  assert text.count(edit['after'])==1;text=text.replace(edit['after'],edit['before'],1)
 before=(repo/row['archive']).read_bytes();assert text.encode()==before
 assert sha(before)==row['before_sha256']
 # The original binder already authenticates this exact helper before calling.
 module=importlib.import_module('composed_stage')
 target=repo/'docs/evidence/main67107-fronts-6e45-20261017/composed_stage.py'
 assert Path(module.__file__).resolve()==target.resolve()
 assert sha(target.read_bytes())=='d86e9260ffbb3328b7640d8907d45d62eebd7b5d978b0d5e683bd4efff472322'
 return module.canonical_stage(repo,before)
def main():
 path=R/'docs/evidence/main67107-fronts-6e45-20261017/bind-composed-endpoints.py'
 original=path.read_bytes();assert sha(original)==ORIGINAL_SHA
 old='from composed_stage import canonical_stage'
 new='canonical_stage = _singleton_canonical_stage'
 source=original.decode();assert source.count(old)==1
 rendered=source.replace(old,new,1)
 assert rendered.replace(new,old,1).encode()==original
 # Every original predicate/statement and actual raw-source receipt is retained.
 exec(compile(rendered,str(path),'exec'),{'__name__':'__main__','__file__':str(path),'_singleton_canonical_stage':_singleton_canonical_stage})
if __name__=='__main__':main()
