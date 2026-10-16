#!/usr/bin/env python3
"""Two complete independent browser lineages; source-only, no current capture."""
from pathlib import Path
import hashlib,json,re,subprocess
R=Path(__file__).resolve().parents[3];D=Path(__file__).resolve().parent
B='df2dfeee51ebb0e7c6ba930f604aa2eccf2c5fd2';M='b80d733efe85a85b121d9ab876ff4b9969c4a6df';P='tests/wasm/scalar_browser_ci_contract/'
sha=lambda b:hashlib.sha256(b).hexdigest()
def git(*a):return subprocess.check_output(['git',*a],cwd=R)
def inverse(row):
 current=(R/row['source']).read_bytes();assert sha(current)==row['current_sha256'];s=current.decode()
 for e in row['edits']:assert s.count(e['new'])==1;s=s.replace(e['new'],e['old'],1)
 old=s.encode();assert sha(old)==row['original_sha256'];assert old==git('show',row['checkpoint_revision']+':'+row['checkpoint_source'])==(R/row['archive']).read_bytes()
 return old
previous=json.loads((D/'current-to-df2.json').read_text());main=json.loads((D/'current-to-main.json').read_text());assert len(previous)==len(main)==4
for row in previous+main:inverse(row)
row=json.loads((D/'new-main-root.json').read_text());mainroot=inverse(row)
# Original whole functions/assertions stay compiled; no legacy body forwarding
# or fallback bill is substituted. Only whole-input setup/lexical paths vary.
def asserts(s):return [l.strip()for l in s.splitlines()if 'assert' in l]
for source,oldpath in [(P+'historical_main_c451.rs',P+'historical_builder.rs'),(P+'before_c451.rs',P+'before_c451.rs'),(P+'main_c451_before_2600.rs',P+'before_2600.rs')]:
 assert asserts((R/source).read_text())==asserts(git('show',M+':'+oldpath).decode()),source
before_root=git('show',B+':'+P+'historical_builder.rs');current=(R/P/'historical_builder.rs').read_bytes();assert current.startswith(before_root)and current.count(before_root)==1
for source in [P+'historical_main2600.rs',P+'before_2600.rs']:
 assert (R/source).read_bytes()==git('show',B+':'+source)
runner='platform-tests/wasm-scalar-browser-v1/test-fixtures.mjs';assert (R/runner).read_bytes()==git('show',M+':'+runner)
for p in git('ls-tree','-r','--name-only',M,'--','docs/evidence/browser-graph-pins-c451086ce/').decode().splitlines():assert (R/p).read_bytes()==git('show',M+':'+p)
# Verify actual historical full-wire identity after only the documented counter
# and bound selfdigest; these are previous observations, never combined new pins.
def parsed(raw):
 v=json.loads(raw);field=b',"graph_digest":'+json.dumps(v['graph_digest']).encode();assert raw.count(field)==1;payload=raw.replace(field,b'',1)
 expected='sha256:'+sha(b'semaprax.project-semantic-graph.artifact-digest.v1\0'+len(payload).to_bytes(8,'little')+payload);assert expected==v['graph_digest'];return v
bridges=[]
for name in ['baseline','renamed']:
 p=R/'docs/evidence/browser-graph-pins-c451086ce'/('current-hosted-calculator-'+name+'.json');raw=p.read_bytes().removesuffix(b'\n');v=parsed(raw);assert v['budget']['used_builder_bytes']==2971194
 for target,bill in [('docs/evidence/browser-graph-pins-2600e49eb/current-hosted-calculator-'+name+'.json',3296148),('docs/evidence/actual47f-pin-fixture-successor-20261016/actual-browser/calculator-'+name+'.json',3477789)]:
  old=(R/target).read_bytes().removesuffix(b'\n');o=parsed(old);assert o['budget']['used_builder_bytes']==bill
  field=b'"used_builder_bytes":2971194';assert raw.count(field)==1;restore=raw.replace(field,('"used_builder_bytes":'+str(bill)).encode(),1)
  field=b'"graph_digest":'+json.dumps(v['graph_digest']).encode();assert restore.count(field)==1;restore=restore.replace(field,b'"graph_digest":'+json.dumps(o['graph_digest']).encode(),1);assert restore==old
  bridges.append({'fixture':name,'previous':target,'complete_bytes':True,'current_source':'actual published Mainc451','later_combined_pin':'UNKNOWN_NOT_CAPTURED'})
owners=json.loads((D/'new-owners.json').read_text());assert len(owners)==3
for o in owners:
 b=(R/o['source']).read_bytes();assert sha(b)==o['sha256'];assert len(b.splitlines())==o['lines']<=1500
newsrc={o['source']for o in owners};allowed={r['source']for r in previous}|newsrc
for p in git('diff',B,'--name-only').decode().splitlines():assert p in allowed or p.startswith('docs/evidence/'+D.name+'/')or p.startswith('docs/evidence/browser-graph-pins-c451086ce/'),p
# Explicit recursive registrations resolve actual files under their #[path]
# parent directories; this avoids the historical custom-path child trap.
for p in [P+'historical_builder.rs',P+'historical_main_c451.rs',P+'before_c451.rs']:
 s=(R/p).read_text()
 for rel in re.findall(r'#\[path = "([^"]+)"\]\nmod \w+;',s):assert (R/p).parent.joinpath(rel).is_file(),(p,rel)
proof={'status':'PASS_SOURCE_ONLY','old_df2_full_inverse_rows':4,'independent_Main_full_inverse_rows':4,'complete_Main_current_helper_metadata_inverse':True,'old_ours_root_functions_and_all_other_browser_files_unchanged':True,'Main_three_plus_before_three_plus_old_four_assertions_registered':True,'Main_runner_and_capture_archive_exact':True,'actual_historical_fullwire_bridges':bridges,'runtime_table_bytes':sum((D/p).stat().st_size for p in ['current-to-df2.json','current-to-main.json']),'new_Rust_owners':owners,'global_audits_authority_and_raw_front':'PENDING_JOIN_WITH_RUNTIME_COMPONENT','future_combined_bill':'UNKNOWN','Cargo_or_owning_test_or_new_capture':False}
(D/'source-proof.json').write_text(json.dumps(proof,indent=2)+'\n');print(json.dumps(proof,indent=2))
