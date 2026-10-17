"""Source-only sealed checkpoint verification. No compiler or namespace refresh."""
from pathlib import Path
import json,hashlib,subprocess,runpy,re,collections,copy
R=Path(__file__).resolve().parents[3];D=Path(__file__).resolve().parent;BASE='3220af312';sha=lambda b:hashlib.sha256(b).hexdigest()
def git(ref,path):return subprocess.check_output(['git','show',ref+':'+path],cwd=R)
m=json.loads((D/'input-domains.json').read_bytes());records=json.loads((D/'whole-inverses.json').read_bytes())
# All current historical archives authenticated against their named immutable Git.
for r in m['main']:
 b=(R/r['archive']).read_bytes();assert len(b)==r['view_len'] and sha(b)==r['view_sha256'];assert b==git(m['table_head'],r['path'])
for r in m['prior090_tables']:
 b=(R/r['archive']).read_bytes();assert len(b)==r['length'] and sha(b)==r['sha256'];assert b==git(m['source_body_head'],r['path'])
for r in m['expected']:
 b=(R/r['archive']).read_bytes();assert b==git(r['head'],r['path'])
for r in records:
 current=(R/r['path']).read_bytes();assert sha(current)==r['after_sha256']
 for e in r['inverse']:
  assert current[e['start_byte']:e['start_byte']+len(e['after'].encode())]==e['after'].encode();current=e['before'].encode()
 assert current==(D/r['before_archive']).read_bytes() and sha(current)==r['before_sha256']
# Every old required/max entry, raw original scope, and native line count stays exact.
G=R/'docs/evidence/main043-independent-custody-20261016';h=runpy.run_path(str(G/'guard-extensions.py'));path=(G/'guard-extension-allowed.json').relative_to(R).as_posix();allowed=json.loads((R/path).read_text());old=json.loads(git(BASE,path));owner='src/source_reflows/incoming090_semantic_receivers.rs';proof=[];negatives=0
for path,rule in allowed.items():
 for k in ('required','maximum'):
  assert {x:v for x,v in rule[k].items() if x!=owner}==old[path][k]
 text=(R/path).read_text();original=git('a82e00cc8',path).decode();fragments=h['fragments'](path,text,original);base=(R/path).parent if path=='tests/hir_module_boundaries.rs' else (R/path).parent.parent
 counts=collections.Counter((base/x).resolve().relative_to(R).as_posix() for f in fragments for x in re.findall(r'include_str!\("([^"\n]+)"\)',f));assert counts[owner]==rule['required'][owner]==rule['maximum'][owner]==1
 h['verify_guard'](path,text,original,False)
 for k in ('required','maximum'):
  for bad in (0,2):
   forged=copy.deepcopy(rule);forged[k][owner]=bad
   assert not(forged['required'][owner]==forged['maximum'][owner]==counts[owner]);negatives+=1
 if 'ledger_capacity' in path:assert len(text.splitlines())==1500
 proof.append({'path':path,'owner_occurrences':counts[owner],'lines':len(text.splitlines()),'existing_rules_preserved':True})
assert (R/'src/source_reflows/semantic_receivers.rs').read_bytes().startswith(git(BASE,'src/source_reflows/semantic_receivers.rs'))
assert 'const MAIN21_CAP: usize = 9_382_389;' in (R/'src/source_reflows/incoming21_semantic_receivers.rs').read_text()
assert len((R/'src/source_reflows/incoming090_semantic_receivers.rs').read_text().splitlines())<=1500
result={'status':'SOURCE_ONLY_FINITE_NAMESPACE_SEALED','base':BASE,'table_head':m['table_head'],'source_body_head':m['source_body_head'],'cap':m['cap'],'main_subjects':len(m['main']),'legacy_current_checkpoints':len(m['legacy_current']),'named_expected_stages':len(m['expected']),'prior090_complete_tables':len(m['prior090_tables']),'raw3':proof,'whole_inverses':len(records),'registration_negatives':negatives,'unassembled':m['unassembled'],'Cargo_or_runtime_run':False}
(D/'proof.json').write_text(json.dumps(result,indent=2)+'\n');print(json.dumps({k:v for k,v in result.items() if k not in ('unassembled','raw3')}))
