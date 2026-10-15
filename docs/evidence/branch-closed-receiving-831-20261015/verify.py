"""Complete source-only branch owner proof; no compiler or executable invocation."""
from pathlib import Path
import contextlib,hashlib,io,json,re,subprocess
R=Path(__file__).resolve().parents[3];D=Path(__file__).resolve().parent;BASE='e93f7169240520ad18613d38f3c51795ae696bfc';h=lambda b:hashlib.sha256(b).hexdigest();rows=json.loads((D/'current-to-e93f.json').read_text());assert rows==json.loads((D/'source-inverses.json').read_text()) and len(rows)==4;named={}
for row in rows:
 p=R/row['source'];raw=p.read_bytes();old=(R/row['archive']).read_bytes();assert raw==row['edits'][0]['new'].encode() and h(raw)==row['current_sha256'];assert old==row['edits'][0]['old'].encode() and h(old)==row['original_sha256'];assert old==subprocess.check_output(['git','show',BASE+':'+row['source']],cwd=R);named[p.resolve()]=old
old=rows[0]['edits'][0]['old'];new=rows[0]['edits'][0]['new'];a='        let accepted = closed.get(&self.sink)';assert old[old.index(a):]==new[new.index(a):], 'entire target producer and canonical algorithms unchanged'
start='            for (node, weight) in state.iter() {';end='            ordered_weights::Closed::Owned(closed)';oldloop=old[old.index(start):old.index(end)];newloop=new[new.index(start):new.index('            &owned\n')].replace('owned.', 'closed.');assert oldloop==newloop,'complete general arithmetic, work, ordering and error body'
a='            for (_, routes) in closure.iter() {';b='            ordered_weights::Closed::Borrowed(closure)';assert old[old.index(a):old.index(b)]==new[new.index(a):new.index('            borrowed = closure;')],'complete borrowed arithmetic/work'
assert 'ordered_weights::Closed::' not in new and 'let borrowed;' in new and 'let mut owned;' in new
assert new.index('"factored borrowed closed receiving controls"')<new.index('let Some(closure) = self.closure_with_receiving',new.index('let borrowed;'))
assert new.index('"factored ordered accumulator controls"',new.index('let borrowed;'))<new.index('owned = ordered_weights::OrderedWeights::new();')
assert rows[1]['edits'][0]['new'].replace('#[cfg(test)]\n#[derive(Debug)]\npub(super) enum Closed {','#[derive(Debug)]\npub(super) enum Closed {').replace('#[cfg(test)]\nimpl Closed {','impl Closed {')==rows[1]['edits'][0]['old']
oldtest=rows[3]['edits'][0]['old'];newtest=rows[3]['edits'][0]['new'];assert newtest.startswith(oldtest) and newtest[len(oldtest):].count('#[test]')==1
# All existing strict physical assertions and complete original receiver remain intact.
for token in ['tuples[1].0 < tuples[0].0','current_metrics().allocated <= budget.physical.as_ref().unwrap().charged()','for limit in 0..=exact']:
 assert token in oldtest and token in newtest
oldreceiver='src/cleanup_plan/replay/factored/canonical_shared/before_receiving_layers.rs';assert (R/oldreceiver).read_bytes()==subprocess.check_output(['git','show',BASE+':'+oldreceiver],cwd=R)
for p in ['../canonical_shared.rs','ordered_weights.rs','../../../../source_reflows_merge.rs','receiving_layer_tests.rs']:
 assert (R/'src/cleanup_plan/replay/factored/canonical_shared'/p).resolve().is_file()
# Replay the entire unchanged preceding source proof through its exact e93f physical domain.
script=R/'docs/evidence/alias-module-loader-fix-847-20261015/verify.py';code=script.read_text();rb,rt,wt,co=Path.read_bytes,Path.read_text,Path.write_text,subprocess.check_output;captured=[]
def read_bytes(p,*a,**k):return named[p.resolve()] if p.resolve() in named else rb(p,*a,**k)
def read_text(p,*a,**k):return named[p.resolve()].decode() if p.resolve() in named else rt(p,*a,**k)
def write(p,s,*a,**k):assert p.name=='source-proof.json';captured.append(json.loads(s));return len(s)
try:
 Path.read_bytes,Path.read_text,Path.write_text=read_bytes,read_text,write
 with contextlib.redirect_stdout(io.StringIO()):exec(compile(code,str(script),'exec'),{'__file__':str(script),'__name__':'__main__'})
finally:Path.read_bytes,Path.read_text,Path.write_text,subprocess.check_output=rb,rt,wt,co
assert captured
proof={'status':'SOURCE_ONLY_BRANCH_OWNER_CONSTRUCTOR_REMOVAL_NOT_QUALIFIED','full_immediate_sources':4,'new_test_bodies':1,'complete_e93f_target_and_canonical_algorithms_unchanged':True,'complete_borrowed_and_general_arithmetic_work_order_unchanged':True,'complete_original_831_receiver_unchanged':True,'strict_reservation_comparison_unchanged':True,'old_runtime_JSON_and_proof_files_unchanged':True,'unchanged_whole_preceding_source_proof':captured[-1],'Rust_Cargo_or_tests_executed':False,'strict831_PASS_or_closure_claim':False}
(D/'source-proof.json').write_text(json.dumps(proof,indent=2)+'\n');print('PASS complete4source inverse, exact algorithms, unchanged full predecessor proof; no Rust')
