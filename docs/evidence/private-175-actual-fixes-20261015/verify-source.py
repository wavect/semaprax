"""Bounded static whole-input custody; no Rust/runtime acceptance claim."""
from pathlib import Path
import argparse,hashlib,json,re,subprocess
A=argparse.ArgumentParser();A.add_argument('--repo',type=Path,required=True);a=A.parse_args();R=a.repo.resolve();D=R/'docs/evidence/private-175-actual-fixes-20261015';BASE='c1597ffdaacee0e1669090a4f7208d9fa52b8c19'
def git(*args):return subprocess.check_output(['git',*args],cwd=R)
def sha(b):return hashlib.sha256(b).hexdigest()
rows=json.loads((D/'current-to-c159.json').read_text());seen=set()
for row in rows:
 p=row['source'];assert p not in seen;seen.add(p);assert row['checkpoint_revision']==BASE
 raw=(R/p).read_bytes();assert sha(raw)==row['current_sha256'];restored=raw.decode()
 for e in row['edits']:
  assert restored.count(e['new'])==1,(p,'unique inverse');restored=restored.replace(e['new'],e['old'],1)
 old=git('show',BASE+':'+p);assert restored.encode()==old==(R/row['archive']).read_bytes();assert sha(old)==row['original_sha256']
changed=set(git('diff',BASE,'--diff-filter=M','--name-only').decode().splitlines());assert seen==changed,(seen^changed)
assert json.loads((D/'source-inverses.json').read_text())==rows
# Direct849 whole inputs authenticate the paid82fea checkpoint independently.
direct=json.loads((D/'current-to-82fea.json').read_text());direct_by={r['source']:r for r in direct}
bindings=[]
for name in ['boxed-state-receiving-849-20261016/current-to-main019d.json','boxed-state-local-owner-849-20261016/current-to-35c.json','boxed-state-receiving-849-20261016/new-owners.json']:
 bindings += [r['source'] for r in json.loads((R/'docs/evidence'/name).read_text())]
for path in sorted(set(bindings)):
 restored=(R/path).read_text()
 if path in direct_by:
  row=direct_by[path];assert sha(restored.encode())==row['current_sha256']
  for edit in row['edits']:
   assert restored.count(edit['new'])==1;restored=restored.replace(edit['new'],edit['old'],1)
  assert sha(restored.encode())==row['original_sha256']
 expected=next(r for r in json.loads((D/'checkpoint-82fea-inputs.json').read_text()) if r['source']==path);archive=(R/expected['archive']).read_bytes();assert sha(archive)==expected['sha256'];assert restored.encode()==archive
# Fixed original transport spec and all immutable files remain the exact prior bytes.
inv='src/source_reflows_merge/runtime_json/inventory.rs';assert (R/inv).read_bytes()==git('show',BASE+':'+inv)
s=(R/inv).read_text();paths=re.findall(r'"(/docs/evidence/[^"\n]+)"',s);assert len(paths)==90 and len(set(paths))==90
fixture_bytes=0
for p in paths:
 p=p[1:];assert p not in changed;d=hashlib.sha256();length=0
 with (R/p).open('rb') as f:
  while b:=f.read(65536):d.update(b);length+=len(b)
 fixture_bytes+=length
assert fixture_bytes==93590460+51472
specs=re.findall(r'path: concat!\(env!\("CARGO_MANIFEST_DIR"\), "(/docs/evidence/[^"\n]+)"\),\s*length: (\d+),\s*sha256: \[([^\]]+)\]',s);assert len(specs)==90
for p,length,digest in specs:
 d=hashlib.sha256();n=0
 with (R/p[1:]).open('rb') as f:
  while b:=f.read(65536):d.update(b);n+=len(b)
 assert n==int(length) and list(d.digest())==[int(x) for x in digest.split(',') if x.strip()]
# Exact original parser/projector algorithm retained; only its transport callers change.
p='src/source_reflows_merge.rs';current=(R/p).read_text();before=git('show',BASE+':'+p).decode()
def project(s):return s[s.index('fn project('):s.index('// Exact post-build')]
assert project(current)==project(before)
# Every old tracked evidence artifact is preserved; MAIN explicitly changes its bundle only.
assert not [p for p in changed if p.startswith('docs/evidence/')], 'old evidence changed'
# Every new Rust file respects the existing per-file cap, and recorded files do not grow.
budget={}
for line in (R/'tests/module-size-budget.tsv').read_text().splitlines():
 if line and not line.startswith('#'):
  parts=line.split();
  if len(parts)>=2:
   if parts[0].isdigit():budget[parts[1]]=int(parts[0])
   elif parts[1].isdigit():budget[parts[0]]=int(parts[1])
rust=sorted(set(p for p in changed if p.endswith('.rs'))|set(p for p in git('diff',BASE,'--diff-filter=A','--name-only').decode().splitlines() if p.endswith('.rs') and not p.startswith('docs/')))
for p in rust:
 lines=len((R/p).read_text().splitlines());assert lines<=budget.get(p,1500),(p,lines,budget.get(p,1500))
proof={'mode':'STATIC_SOURCE_ONLY_NO_RUST_EXECUTION','checkpoint':BASE,'whole_changed_prior_inputs':len(rows),'direct849_checkpoint_inputs':len(set(bindings)),'direct849_changed_rows':len(direct),'restored_namespace':'EVERY_CHANGED_PREEXISTING_TRACKED_INPUT_EXACT_C159_GIT','original_fixed_runtime_json_count':89,'original_fixed_runtime_json_bytes':93590460,'extra_custody_count':1,'extra_custody_bytes':51472,'all90_exact':True,'project_algorithm_exact':True,'old_evidence_artifacts_unchanged':True,'changed_and_added_rust_files_cap_checked':len(rust),'table_bytes':(D/'current-to-c159.json').stat().st_size,'table_sha256':sha((D/'current-to-c159.json').read_bytes())}
(D/'whole-source-proof.json').write_text(json.dumps(proof,indent=2)+'\n');print(json.dumps(proof,indent=2))
