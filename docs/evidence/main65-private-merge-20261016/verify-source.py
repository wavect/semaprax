#!/usr/bin/env python3
"""Separate exact75d8 and scopedMain65 source inputs; no executable gate."""
from pathlib import Path
import argparse,contextlib,hashlib,io,json,runpy,subprocess,sys
p=argparse.ArgumentParser();p.add_argument('--repo',type=Path,default=Path(__file__).resolve().parents[3]);R=p.parse_args().repo.resolve();D=R/'docs/evidence/main65-private-merge-20261016';BASE='75d8e629d07bcef62abdab4c87a62697e3602d65';MAIN='65cc921528b601674c66ccf751e8ebb9cd7868e1';OLD=R/'docs/evidence/prepaid-canonical-cursor-clippy-20261016';sha=lambda b:hashlib.sha256(b).hexdigest();CO=subprocess.check_output
rows=json.loads((D/'current-to-75d8.json').read_text());assert len(rows)==10;view={}
for row in rows:
 path=row['source'];b=(R/path).read_bytes();assert sha(b)==row['current_sha256'];s=b.decode()
 for e in row['edits']:assert s.count(e['new'])==1;s=s.replace(e['new'],e['old'],1)
 b=s.encode();assert sha(b)==row['original_sha256'];assert b==CO(['git','show',BASE+':'+path],cwd=R)==(D/'before/75d8'/path).read_bytes();view[str((R/path).resolve())]=b
assert json.loads((D/'browser-current-to-75d8.json').read_text())==[x for x in rows if x['source']=='tests/wasm/scalar_browser_ci_contract.rs']
for row in json.loads((D/'main65-owning-inputs.json').read_text()):
 b=(R/row['physical']).read_bytes();assert sha(b)==row['current_sha256'];s=b.decode()
 for e in row['edits']:assert s.count(e['new'])==1;s=s.replace(e['new'],e['old'],1)
 assert sha(s.encode())==row['original_sha256'];assert s.encode()==CO(['git','show',MAIN+':'+row['source']],cwd=R)
for archive,original in [('main-runner.mjs.txt','platform-tests/wasm-scalar-browser-v1/test-fixtures.mjs'),('main-browser-parent.rs.txt','tests/wasm/scalar_browser_ci_contract.rs'),('historical_builder.main.rs.txt','tests/wasm/scalar_browser_ci_contract/historical_builder.rs')]:assert(D/archive).read_bytes()==CO(['git','show',MAIN+':'+original],cwd=R)
# Existing live pins/receivers, complete Main24/f1 namespaces and old packets stay exact.
fixed=['platform-tests/wasm-scalar-browser-v1/test-fixtures.mjs','tests/wasm/scalar_browser_ci_contract/historical_builder.rs']+CO(['git','ls-tree','-r','--name-only',BASE,'--','src/cleanup_plan/replay/sequence_prefix/main24','src/cleanup_plan/replay/sequence_prefix/main_current.rs'],cwd=R).decode().splitlines()
for path in fixed:assert(R/path).read_bytes()==CO(['git','show',BASE+':'+path],cwd=R)
for path in CO(['git','ls-tree','-r','--name-only',BASE,'--',str(OLD.relative_to(R))],cwd=R).decode().splitlines():assert(R/path).read_bytes()==CO(['git','show',BASE+':'+path],cwd=R)
rb0,rt0,wt0=Path.read_bytes,Path.read_text,Path.write_text;argv=sys.argv[:]
def rb(p):return view.get(str(p.resolve()),rb0(p))
def rt(p,*args,**kw):
 b=view.get(str(p.resolve()));return b.decode(kw.get('encoding')or'utf-8')if b is not None else rt0(p,*args,**kw)
def wt(p,s,*args,**kw):assert p.resolve().is_relative_to(R/'docs/evidence');return len(s)
try:
 Path.read_bytes=rb;Path.read_text=rt;Path.write_text=wt;sys.argv=[str(OLD/'verify-source.py'),'--repo',str(R)];out=io.StringIO()
 with contextlib.redirect_stdout(out):runpy.run_path(str(OLD/'verify-source.py'),run_name='__main__')
 prior=json.loads(out.getvalue())
finally:Path.read_bytes=rb0;Path.read_text=rt0;Path.write_text=wt0;sys.argv=argv
source=(R/'src/source_reflows_merge.rs').read_text()
for name in ['main_dd0_previous','arena854_input','main7c6_input','main_afba_input','main_f1_input','main24_input']:
 block=source[source.index('fn '+name+'('):];assert block.index('main65_previous')<block.index('cursor_clippy_input')
owners=json.loads((D/'new-owners.json').read_text());assert len(owners)==13
for row in owners:
 b=(R/row['source']).read_bytes();assert sha(b)==row['sha256'] and len(b.splitlines())==row['lines']<=1500
 for file in ['tests/hir_module_boundaries.rs','tests/workspace/semantic_graph/source_inventory.rs','crates/semaprax-native-rust-interop-builder/src/implementation/tests/ledger_capacity/source_inventory.rs']:assert(R/file).read_text().count(row['source'])==1
assert len((R/'crates/semaprax-native-rust-interop-builder/src/implementation/tests/ledger_capacity/source_inventory.rs').read_text().splitlines())==1500
checks=json.loads((D/'pending-selectors.json').read_text());assert len(checks)==24 and len({(x['package'],x['kind'],x['selector'])for x in checks})==24
C=json.loads((D/'arm-contract.json').read_text());assert sha((R/C['selector_source']).read_bytes())==C['common_source_sha256'];assert sha((R/C['final_normalizer']).read_bytes())==C['final_normalizer_sha256']
CO(['git','merge-base','--is-ancestor',MAIN,'HEAD'],cwd=R)
print(json.dumps({'status':'PASS_SOURCE_ONLY','whole75d8_rows':10,'independent_Main65_owning_inputs':6,'prior_source_algorithms':prior['status'],'new_owners_all_three_raw_unions':13,'new_selectors':24,'existing_live_runner_and_receivers_exact75d8':True,'actual_Main65_ancestor':True,'rust_executed':False,'original_registration_preflight_run':False},sort_keys=True))
