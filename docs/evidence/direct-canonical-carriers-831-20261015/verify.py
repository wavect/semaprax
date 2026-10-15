"""Complete source inverses and actual carrier elimination only; no Rust."""
from pathlib import Path
import contextlib,hashlib,io,json,subprocess,re
R=Path.cwd();D=Path(__file__).resolve().parent;B='29750ef89f6823e8c8b0fde330342f585762bfb5';sha=lambda b:hashlib.sha256(b).hexdigest();introduced=set()
rows=json.loads((D/'current-to-297.json').read_text());assert rows==json.loads((D/'source-inverses.json').read_text())
def apply(rows,p,b):
 rs=[r for r in rows if r['source']==p];assert len(rs)<=1
 if not rs:return b
 r=rs[0];assert sha(b)==r['current_sha256'],p
 for e in r['edits']:
  n=e['new'].encode();assert b.count(n)==1;b=b.replace(n,e['old'].encode(),1)
 assert sha(b)==r['original_sha256'];return b
for r in rows:
 p=r['source'];assert apply(rows,p,(R/p).read_bytes())==(R/r['archive']).read_bytes()==subprocess.check_output(['git','show',B+':'+p])
 def reject():apply(rows,p,(R/p).read_bytes()+b' ')
 try:reject()
 except AssertionError:pass
 else:raise AssertionError('altered physical source accepted')
C='src/cleanup_plan/replay/factored/canonical_shared';old=lambda p:subprocess.check_output(['git','show',B+':'+p],text=True)
slot=(R/(C+'/closure_receiving.rs')).read_text();before=old(C+'/closure_receiving.rs')
def part(s,a,b):return s[s.index(a):s.index(b,s.index(a))]
start='    pub(in crate::cleanup_plan::replay::factored) fn acquire(';end='    pub(super) fn valid('
assert part(slot,start,end)==part(before,start,end),'entire old acquire remains independently compiled'
assert '#[cfg(test)]\n'+start in slot
assert part(slot,'    pub(super) const CONTROLS: usize',';')==part(before,'    pub(super) const CONTROLS: usize',';'),'old controls are independently verbatim, not derived from direct controls'
initializer=re.search(r'Ok\((Self \{.*?\n        \})\)',part(before,start,end),re.S).group(1)
new_ctor=part(slot,'    pub(in crate::cleanup_plan::replay::factored) fn from_prepaid(','    // The unit producer')
assert initializer in new_ctor and 'Result<' not in new_ctor
# Five real entries use the exact direct prepayment and Self-only transport.
callers=[C+'.rs','src/cleanup_plan/replay/factored.rs','src/cleanup_plan/replay/factored/strategy.rs']
for p in callers:
 s=(R/p).read_text();expected=3 if p==C+'.rs' else 1
 assert len(re.findall(r'::from_prepaid\(function,budget,?\)',re.sub(r'\s+','',s)))==expected
 assert s.count('::DIRECT_CONTROLS,')==expected
 assert 'ClosureReceiving::acquire(' not in s
 assert s.count('"factored held closure invocation controls"')==expected
current=(R/(C+'.rs')).read_text();previous=old(C+'.rs')
assert part(current,'fn own_weighted(','fn weighted_successor(')==part(previous,'fn own_weighted(','fn weighted_successor('),'old general constructor and all fees retained'
assert 'let root = own_weighted(function, budget, root)?;' not in current
assert current.count('"factored direct root enum receiving"')==1
root=part(current,'        // This direct root construction','        let root_copy = share_weighted')
for item in ['2 * std::mem::size_of::<SmallWeighted>()','2 * std::mem::size_of::<SharedWeighted>()','std::mem::size_of::<Result<(), Diagnostic>>()','physical_clone::rc_backing_bytes::<SmallWeighted>()','let shared = SmallWeighted::Many(root);','let root = Rc::new(shared);']:assert item in root,item
assert 'Result<SharedWeighted, Diagnostic>' not in root
assert root.index('budget.reserve_bytes(')<root.index('let shared =')<root.index('let root = Rc::new')
tests=(R/(C+'/closure_receiving/tests.rs')).read_text();assert tests.startswith(old(C+'/closure_receiving/tests.rs'));assert tests.count('#[test]')==old(C+'/closure_receiving/tests.rs').count('#[test]')+1
assert 'Budget acquisition, slot initialization, refusal and both Drops' in tests and 'allocation.allocated <= paid' in tests
# Baseline, strict assertion and all complete old independent receivers are exact.
for p in [C+'/receiving_layer_tests.rs',C+'/before_receiving_layers.rs',C+'/before_closure_receiver.rs',C+'/before_closure_invocation.rs',C+'/before_inline_weighted.rs']:
 f=R/p
 if f.exists():assert f.read_text()==old(p)
for p in callers+[C+'/closure_receiving.rs',C+'/closure_receiving/tests.rs','src/source_reflows_merge.rs']:assert len((R/p).read_text().splitlines())<=1500
def replay(script,rows,scope):
 path=R/script;code=path.read_text();named={(R/r['source']).resolve():apply(rows,r['source'],(R/r['source']).read_bytes()) for r in rows}
 rb,rt,wt,co,glob=Path.read_bytes,Path.read_text,Path.write_text,subprocess.check_output,Path.rglob;proofs=[]
 def bounded_glob(p,*a,**k):
  for child in glob(p,*a,**k):
   if child.resolve() in introduced:continue
   yield child
 def bound_bytes(p,*a,**k):return named[p.resolve()] if p.resolve() in named else rb(p,*a,**k)
 def bound_text(p,*a,**k):return named[p.resolve()].decode() if p.resolve() in named else rt(p,*a,**k)
 def write(p,s,*a,**k):assert p.name=='source-proof.json';proofs.append(json.loads(s));return len(s)
 def output(args,*a,**k):
  if isinstance(args,list) and args[:3]==['git','diff','--name-only'] and '--' in args:
   i=args.index('--');revs=[x for x in args[3:i] if not x.startswith('-')]
   if len(revs)==1:args=args[:i]+[scope]+args[i:]
  return co(args,*a,**k)
 try:
  Path.read_bytes,Path.read_text,Path.write_text,subprocess.check_output,Path.rglob=bound_bytes,bound_text,write,output,bounded_glob
  with contextlib.redirect_stdout(io.StringIO()) as out:exec(compile(code,str(path),'exec'),{'__file__':str(path),'__name__':'__main__'})
 finally:Path.read_bytes,Path.read_text,Path.write_text,subprocess.check_output,Path.rglob=rb,rt,wt,co,glob
 return proofs[-1] if proofs else json.loads(out.getvalue())




prior=replay('docs/evidence/retry-fixture-full-source-view-61f-20261015/verify.py',rows,B)
assert prior['old_literal_hash_and_all_splices_unchanged']
changes=subprocess.check_output(['git','diff','--name-status',B,'--','docs/evidence'],text=True).splitlines();assert all(x.startswith('A\t') for x in changes),changes
result=dict(status='SOURCE_ONLY_NO_RUST_GATES',source_basis=B,complete_current_to_old_inputs=len(rows),entire_old_acquire_and_general_weighted_constructor_exact=True,actual_direct_slot_sites=5,new_test_bodies=1,all_old_test_bodies_and_strict163_unchanged=True,bootstrap_symbolic_removed='size_of::<Result<ClosureReceiving,Diagnostic>>()',root_symbolic_net_removed='size_of::<Result<SharedWeighted,Diagnostic>>() - 2*size_of::<SharedWeighted>()',all_old_runtime_JSON_unchanged=True,full_prior_proof=prior,actual_allocation_or_reservation_gain=None,strict831_issue_closure=False)
(D/'source-proof.json').write_text(json.dumps(result,indent=2)+'\n');print(json.dumps({k:v for k,v in result.items() if k!='full_prior_proof'},indent=2))
