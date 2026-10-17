"""Offline source controls for the exact Alias1ae verification acquisition."""
from pathlib import Path
import argparse, ast, hashlib, importlib.util, json, tempfile
E=Path(__file__).resolve().parent
p=argparse.ArgumentParser();p.add_argument('--repo',type=Path,default=E.parents[2]);a=p.parse_args();R=a.repo.resolve()
spec=importlib.util.spec_from_file_location('source_parts_composed_controls',E/'verify-source-parts-composed.py')
m=importlib.util.module_from_spec(spec);spec.loader.exec_module(m)
parts, current, projected, alias_stage=m.programs(R)
old_parts=parts.read_bytes();old_verifier=(R/'docs/evidence/a3-e47-final-custody-20261016/verify-source.py').read_bytes()
assert current.replace(m.PARTS_NEW,m.PARTS_OLD,1).encode()==old_parts
assert projected.replace(m.ACQUIRE_NEW,m.ACQUIRE_OLD,1).encode()==old_verifier
# All protected assertions, raw-owner reads and complete inherited programs
# remain byte-exact after reversing only the one acquisition expression.
assert projected.count(m.ACQUIRE_NEW)==1 and projected.count(m.ACQUIRE_OLD)==0
raw=(R/m.TARGET).read_bytes();sha=lambda b:hashlib.sha256(b).hexdigest()
staged=alias_stage(R,m.TARGET,raw)
assert sha(staged)=='a75c86aeb4a3e36f43e6ab963f6ff5357b8f9694c34250df32627a6ac513975b'
# Execute the ACTUAL replacement acquisition/hash/decode statements, with only
# their direct source byte read replaced by a local input object. Metadata and
# the exact shared alias_stage still read the real authenticated source files.
code=compile('\n'.join(line[2:] for line in m.ACQUIRE_NEW.splitlines())+'\n','actual-verifier-acquisition','exec')
class File:
    def __init__(self, data): self.data=data
    def read_bytes(self): return self.data
class Repo:
    def __init__(self, source, data): self.source=source;self.data=data
    def __fspath__(self): return str(R)
    def __truediv__(self, path): return File(self.data) if path==self.source else R/path
positive=negative=0
def acquire(domain,source,data,expected):
    ns={'R':Repo(source,data),'r':{'source':source,'current_sha256':expected},'domain':{'name':domain},'TARGET':m.TARGET,'alias_stage':alias_stage,'h':sha}
    exec(code,ns)
    assert ns['raw'].decode()==ns['s']
    return ns['raw']
def refuses(call):
    global negative
    try:call()
    except (AssertionError,UnicodeError):negative+=1
    else:raise AssertionError('hostile source/domain admitted')
assert acquire('Alias1ae',m.TARGET,raw,sha(staged))==staged;positive+=1
for domain in ['Previous16c','Maina3','MainE47','Alias1ae-neighbor']:
    assert acquire(domain,m.TARGET,raw,sha(raw))==raw;positive+=1
    refuses(lambda domain=domain:acquire(domain,m.TARGET,raw,sha(staged)))
assert acquire('Alias1ae','unrelated/source.rs',b'unchanged raw',sha(b'unchanged raw'))==b'unchanged raw';positive+=1
for hostile in [raw+b' ',raw[:-1],bytes([raw[0]^1])+raw[1:],raw.replace(b'\n',b'\r\n'),b'\xff'+raw[1:]]:
    refuses(lambda hostile=hostile:acquire('Alias1ae',m.TARGET,hostile,sha(staged)))
# Historical/intermediate bytes cannot masquerade as current raw acquisition.
helper=R/'docs/evidence/main67107-fronts-6e45-20261017/composed_stage.py'
spec=importlib.util.spec_from_file_location('hostile_intermediate_stage',helper)
outer=importlib.util.module_from_spec(spec);spec.loader.exec_module(outer)
intermediate=outer.outer_stage(R,'ownedGrowth246fd230',m.TARGET,raw)
for already_staged in [staged,intermediate]:
    refuses(lambda already_staged=already_staged:acquire('Alias1ae',m.TARGET,already_staged,sha(staged)))
# Retargeting the historical expected SHA to raw current bytes is refused.
refuses(lambda:acquire('Alias1ae',m.TARGET,raw,sha(raw)))
# Whole old-program and metadata pin controls use only small isolated SSD files.
with tempfile.TemporaryDirectory(prefix='parts-composed-controls-',dir='/Volumes/Semaprax/runtime-tmp') as name:
    Q=Path(name)
    inputs=[parts.relative_to(R),Path('docs/evidence/a3-e47-final-custody-20261016/verify-source.py')]
    for path in inputs:
        q=Q/path;q.parent.mkdir(parents=True,exist_ok=True);q.write_bytes((R/path).read_bytes())
    m.programs(Q);positive+=1
    for path in inputs:
        q=Q/path;original=q.read_bytes();q.write_bytes(original+b' ')
        refuses(lambda:m.programs(Q));q.write_bytes(original)
    for path in ['composed_stage.py','stages.json']:
        q=Q/'docs/evidence/main67107-fronts-6e45-20261017'/path;q.parent.mkdir(parents=True,exist_ok=True);q.write_bytes((R/q.relative_to(Q)).read_bytes())
    path=Path('docs/evidence/fade-custody-size-ownedgrowth-20261017/alias-root.json');q=Q/path;q.parent.mkdir(parents=True,exist_ok=True);q.write_bytes((R/path).read_bytes())
    assert alias_stage(Q,m.TARGET,raw)==staged;positive+=1
    for path in [Path('docs/evidence/main67107-fronts-6e45-20261017/composed_stage.py'),Path('docs/evidence/main67107-fronts-6e45-20261017/stages.json'),Path('docs/evidence/fade-custody-size-ownedgrowth-20261017/alias-root.json')]:
        q=Q/path;original=q.read_bytes();q.write_bytes(original+b' ')
        refuses(lambda:alias_stage(Q,m.TARGET,raw));q.write_bytes(original)
# Concrete full census is deliberately raw and is a separate old read site.
D=R/'docs/evidence/a3-e47-final-custody-20261016'
owner=[r for r in json.loads((D/'new-owners.json').read_bytes()) if r['source']==m.TARGET]
assert len(owner)==1 and owner[0]['sha256']==sha(raw) and owner[0]['lines']==len(raw.splitlines())<=1500
assert "b=(R/r['source']).read_bytes();assert h(b)==r['sha256']" in projected
print(json.dumps({'status':'PASS_SOURCE_ONLY_COMPOSED_ACQUISITION','positive':positive,'negative':negative,'alias_scope':'Alias1ae AND exactownedgrowth only','current_raw_sha256':sha(raw),'historical_input_sha256':sha(staged),'raw_owner_census_unchanged':True,'all_original_programs_full_inverse':True,'no_common_refresh_or_Rust_execution':True}))
