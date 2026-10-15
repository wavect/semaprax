"""Finite source/fixture proof only. Does not execute Rust or tests."""
from pathlib import Path
import argparse,json,re,hashlib,subprocess
P=argparse.ArgumentParser();P.add_argument('--repo',type=Path,required=True);R=P.parse_args().repo.resolve();D=Path(__file__).resolve().parent;H='79d5be03a898c00869aebd4efc62ec2d59088551';h=lambda b:hashlib.sha256(b).hexdigest();g=lambda *a:subprocess.check_output(['git',*a],cwd=R)
rows=json.loads((D/'source-inverses.json').read_text());assert len(rows)==6
for row in rows:
 current=(R/row['source']).read_bytes();assert h(current)==row['current_sha256'];old=current
 for e in row['edits']:assert old.count(e['new'].encode())==1;old=old.replace(e['new'].encode(),e['old'].encode(),1)
 assert h(old)==row['original_sha256'] and old==(R/row['archive']).read_bytes()==g('show',H+':'+row['source'])
root=(R/'src/source_reflows_merge.rs').read_text();old=(D/'before/src/source_reflows_merge.rs').read_text();project=lambda s:s[s.index('fn project('):s.index('// Exact post-build')];assert project(root)==project(old)
expected=re.sub(r'&?runtime_json::load\((runtime_json::Fixture::(?:F\d\d|Custody))\)',r'\1',old);expected=re.sub(r'(?<!fn )\bproject\((\s*runtime_json::Fixture::)',r'runtime_json::project(\1',expected);expected=re.sub(r'\bproject\(&(data|rows),',r'runtime_json::project(\1,',expected)
# These roots contain ordinary quoted literals; preserve exact literal sequence,
# then compare all non-comment/non-whitespace syntax after approved substitution.
strings=r'"(?:\\.|[^"\\])*"';assert re.findall(strings,root)==re.findall(strings,expected)
def syntax(s):
 s=re.sub(strings,'""',s);s=re.sub(r'//[^\n]*|/\*.*?\*/','',s,flags=re.S);return ''.join(s.split()).replace(',)', ')')
assert syntax(root)==syntax(expected);assert root.count('runtime_json::Fixture::')==90 and 'runtime_json::load' not in root
inventory=(R/'src/source_reflows_merge/runtime_json/inventory.rs').read_bytes();assert inventory==g('show',H+':src/source_reflows_merge/runtime_json/inventory.rs')
fixed=re.findall(r'Self::(F\d\d|Custody) => Spec \{\s*path: concat!\(env!\("CARGO_MANIFEST_DIR"\), "([^"\n]+)"\),\s*length: (\d+),\s*sha256: \[([^\]]*)\],\s*\}',inventory.decode(),re.S);assert len(fixed)==90
membership=(R/'src/source_reflows_merge/runtime_json/membership.rs').read_text();compiled={name:[json.loads(x) for x in re.findall(strings,text)] for name,text in re.findall(r'Fixture::(F\d\d|Custody) => &\[(.*?)\]',membership,re.S)};assert len(compiled)==90
entries=0
for name,path,length,digest in fixed:
 raw=(R/path.lstrip('/')).read_bytes();assert len(raw)==int(length) and h(raw)==bytes(int(x.strip()) for x in digest.split(',') if x.strip()).hex();data=json.loads(raw);assert isinstance(data,list);actual=sorted(x['source'] for x in data if isinstance(x.get('source'),str));assert all(x.isascii() for x in actual);assert compiled[name]==actual;entries+=len(actual);assert raw==g('show',H+':'+path.lstrip('/'))
assert entries==897
new=json.loads((D/'new-owner.json').read_text());assert h((R/new['source']).read_bytes())==new['sha256']
for row in rows:
 if row['source'].endswith('source_inventory.rs') or row['source']=='tests/hir_module_boundaries.rs':assert (R/row['source']).read_text().count('src/source_reflows_merge/runtime_json/membership.rs"')==1
interop=R/'crates/semaprax-native-rust-interop-builder/src/implementation/tests/ledger_capacity/source_inventory.rs';assert len(interop.read_text().splitlines())==1500
for p in ['src/source_reflows_merge.rs','src/source_reflows_merge/runtime_json/membership.rs']:assert len((R/p).read_text().splitlines())<=1500
before=(D/'before/src/source_reflows_merge/runtime_json/tests.rs').read_text();after=(R/'src/source_reflows_merge/runtime_json/tests.rs').read_text();anchor='    // Ten distinct hostile boundaries.';assert before[before.index(anchor):]==after[after.index(anchor):]
print(json.dumps({'status':'SOURCE_ONLY_NOT_EXECUTED','fixed_fixtures_exact':90,'exact_key_entries_with_multiplicity':897,'unchanged_project_body':True,'same90call_fixture_routing':True,'ten_old_hostile_boundaries_byteexact':True,'whole_predecessor_inputs':6,'old_JSON_changed':False,'performance_claim':False}))
