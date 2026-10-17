"""Source-only complete inverse and physical-prefix controls; no Rust execution."""
from pathlib import Path
import hashlib,json,re,subprocess
R=Path(__file__).resolve().parents[3];D=Path(__file__).resolve().parent
BASE='36c6deb98720e243b0daaf81e813fbe3ec3b0a2d'
sha=lambda b:hashlib.sha256(b).hexdigest()
for row in json.loads((D/'whole-inverses.json').read_bytes()):
 raw=(R/row['path']).read_bytes();assert sha(raw)==row['after_sha256'] and raw.decode()==row['after']
 old=(R/row['before_archive']).read_bytes();assert sha(old)==row['before_sha256'] and old.decode()==row['before']
 assert old==subprocess.check_output(['git','show',BASE+':'+row['path']],cwd=R)
 # Exact whole inverse, never a normalized source comparison.
 assert raw.decode().replace(row['after'],row['before'],1).encode()==old
rows=json.loads((D/'subjects.json').read_bytes());refusals=0
for n,row in enumerate(rows):
 raw=(R/row['path']).read_bytes();held=(D/f'{n}.current.rs.txt').read_bytes();prior=(D/f'{n}.prior.rs.txt').read_bytes()
 assert raw==held and len(raw)==row['current_length'] and sha(raw)==row['current_sha256']
 assert len(prior)==row['prior_length'] and sha(prior)==row['prior_sha256']
 restored=prior.decode().removesuffix('\n#[cfg(test)]\npub(crate) mod work;\n')
 archive=R/('docs/evidence/opt810-live-oracle/src__'+row['path'][4:].replace('/','__')+'.before')
 assert restored.encode()==archive.read_bytes()
 for bad in [b'',raw[:-1],raw+b' ',prior,raw.replace(b'\n',b'\r\n')]:
  assert bad!=held;refusals+=1
helper=(R/'tests/support/production_source_inventory.rs').read_text();old=(D/'changed-2.before.txt').read_text()
assert helper.startswith(old.replace('    assert_relocated_prefix(raw);','    assert_current_relocated_prefix(raw);',1))
oldguard=old[old.index('fn relocated_prefix()'):];assert oldguard in helper
block=helper.split("fn current_relocated_prefix() -> &'static str {",1)[1].split('\n}',1)[0]
paths=re.findall(r'include_str!\("../../([^"\n]+)"\)',block);assert len(paths)==5
pieces=[(R/p).read_text()for p in paths];prefix=''.join(pieces)
def admitted(raw):
 return bool(prefix) and raw.startswith(prefix) and not raw[len(prefix):].startswith(prefix)
assert admitted(prefix+'remaining original inventory')
for i in range(5):
 assert not admitted(''.join(pieces[:i]+pieces[i+1:]));refusals+=1
for i in range(4):
 changed=list(pieces);changed[i],changed[i+1]=changed[i+1],changed[i];assert not admitted(''.join(changed));refusals+=1
for bad in ['',prefix[1:],'sentinel'+prefix,prefix+prefix]:assert not admitted(bad);refusals+=1
# Both owning domains physically place the same five subjects first. No raw3 edits.
for scope in ['hir','projection']:
 full=json.loads((R/f'docs/evidence/a3-e47-final-custody-20261016/{scope}.json').read_bytes())
 assert [c['path']for c in full['chunks'][:5]]==paths
 actual=''.join((R/c['path']).read_text()if 'path'in c else c['literal']for c in full['chunks']);assert admitted(actual)
result={'status':'PASS_SOURCE_ONLY_NO_RUST_RUNTIME','whole_changed_sources':3,'whole_funded_subjects':2,'physical_domains':2,'refusals':refusals,'old_guard_and_all_old_assertions_retained':True}
(D/'source-controls.json').write_text(json.dumps(result,indent=2)+'\n');print(json.dumps(result))
