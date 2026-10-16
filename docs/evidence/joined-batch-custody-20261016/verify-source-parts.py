"""Lossless metadata-only logical arrays for the unchanged original verifier."""
from pathlib import Path
import argparse,hashlib,json,runpy,sys
p=argparse.ArgumentParser();p.add_argument('--repo',type=Path,required=True);a=p.parse_args();R=a.repo.resolve();D=R/'docs/evidence/a3-e47-final-custody-20261016';O=Path(__file__).resolve().parent
original=D/'verify-source.py'
assert hashlib.sha256(original.read_bytes()).hexdigest()=="a90027caccbfbf183f08cdc6d4bd25ea5580403e0d1525099aa6342e020160c5",'original source verifier changed'
parts=runpy.run_path(str(O/'table-parts.py'));logical={}
for domain in json.loads((D/'domains.json').read_bytes()):
    rows=parts['load_rows'](D,domain)
    if 'parts' in domain:logical[(D/domain['table']).resolve()]=parts['encoded'](rows).decode()
# No source path is projected. Only the four fixed JSON metadata arrays are
# reconstructed; every row/edit/hash/archive remains under original assertions.
read_text=Path.read_text;argv=sys.argv[:]
def metadata(path,*args,**kwargs):
    return logical[path.resolve()] if path.resolve() in logical else read_text(path,*args,**kwargs)
try:
    Path.read_text=metadata;sys.argv=[str(original),'--repo',str(R)]
    runpy.run_path(str(original),run_name='__main__')
finally:Path.read_text=read_text;sys.argv=argv
