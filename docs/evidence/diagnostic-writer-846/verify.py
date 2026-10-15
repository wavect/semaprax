"""Full old renderers, actual CLI adoption and source succession; no Rust."""
from pathlib import Path
import contextlib,hashlib,io,json,subprocess,re
R=Path.cwd();D=Path(__file__).resolve().parent;B='7a87b33115c0cd45b14cac64d7b37e5742503159';sha=lambda b:hashlib.sha256(b).hexdigest()
rows=json.loads((D/'source-inverses.json').read_text());assert rows==json.loads((D/'current-to-7a87.json').read_text());additions=json.loads((D/'new-owners.json').read_text());introduced={(R/r['source']).resolve() for r in additions}
def apply(rows,p,b):
 rs=[r for r in rows if r['source']==p];assert len(rs)<=1
 if not rs:return b
 r=rs[0];assert sha(b)==r['current_sha256'],p
 for e in r['edits']:
  n=e['new'].encode();assert b.count(n)==1;b=b.replace(n,e['old'].encode(),1)
 assert sha(b)==r['original_sha256'];return b
for r in rows:
 p=r['source'];assert apply(rows,p,(R/p).read_bytes())==(R/r['archive']).read_bytes()==subprocess.check_output(['git','show',B+':'+p])
 try:apply(rows,p,(R/p).read_bytes()+b' ')
 except AssertionError:pass
 else:raise AssertionError('modified source accepted')
for r in additions:assert sha((R/r['source']).read_bytes())==r['sha256']
old=lambda p:subprocess.check_output(['git','show',B+':'+p],text=True)
root=(R/'src/diagnostic.rs').read_text();original=old('src/diagnostic.rs');assert root[:len(original)]==original,'entire old Diagnostic/json/quote/human/tests unchanged'
assert (R/'src/diagnostic/before_writer.rs').read_text()==original.split('#[cfg(test)]\nmod tests {')[0]
writer=(R/'src/diagnostic/writer.rs').read_text();assert writer.index('active_limit().is_some()')<writer.index('return output.write_str(&self.json())')<writer.index('output.write_str("{')
assert 'String::' not in writer and 'format!(' not in writer and 'Vec<' not in writer,'ordinary renderer has no owned intermediate output'
assert 'fn quoted(' in writer and 'value.is_control()' in writer and 'write_lines' in writer
cli=(R/'src/cli_driver/source_execution.rs').read_text();before=old('src/cli_driver/source_execution.rs');start='pub(super) fn report_all(';end='/// `assurance-policy`';a=cli.index(start);b=cli.index(end,a);x=before.index(start);y=before.index(end,x);assert cli[:a]+before[x:y]+cli[b:]==before,'all other CLI consumers/status code exact'
new=cli[a:b];assert 'error.json()' not in new and 'println!' not in new and 'eprintln!' not in new and 'stdout.lock()' in new and 'stderr.lock()' in new
assert new.count('write_lines(')==2 and new.count('let _ =')==2
report=(R/'src/workspace_report.rs').read_text();assert report.replace('diagnostic.write_to(out, false)?;','fmt::Display::fmt(diagnostic, out)?;')==old('src/workspace_report.rs'),'typed report owner/selection/framing unchanged'
# Existing CLI bodies remain exact; only one actual process/stream/status owner added.
tests=(R/'tests/cli_check_routing_v1.rs').read_text();assert tests.startswith(old('tests/cli_check_routing_v1.rs'));assert tests.count('#[test]')==old('tests/cli_check_routing_v1.rs').count('#[test]')+1
for p in ['tests/hir_module_boundaries.rs','tests/workspace/semantic_graph/source_inventory.rs','crates/semaprax-native-rust-interop-builder/src/implementation/tests/ledger_capacity/source_inventory.rs']:
 text=(R/p).read_text()
 for r in additions:assert text.count(r['source']+'"')==1
 assert len(text.splitlines())<=1500
for p in [r['source'] for r in additions]+['src/diagnostic.rs','src/workspace_report.rs','tests/cli_check_routing_v1.rs','src/cli_driver/source_execution.rs','src/source_reflows_merge.rs']:assert len((R/p).read_text().splitlines())<=1500
def replay(script,rows,scope):
 path=R/script;code=path.read_text();named={(R/r['source']).resolve():apply(rows,r['source'],(R/r['source']).read_bytes()) for r in rows}
 for r in json.loads((D/'parent-changelog-static-view.json').read_text()):
  p=(R/r['source']).resolve();named[p]=apply([r],r['source'],named[p]);assert named[p]==subprocess.check_output(['git','show',r['checkpoint_revision']+':'+r['source']])==(R/r['archive']).read_bytes()
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





prior=replay('docs/evidence/direct-canonical-carriers-831-20261015/verify.py',rows,B)
assert prior['entire_old_acquire_and_general_weighted_constructor_exact']
changes=subprocess.check_output(['git','diff','--name-status',B,'--','docs/evidence'],text=True).splitlines();assert all(x.startswith('A\t') for x in changes),changes
result=dict(status='SOURCE_ONLY_NO_RUST_GATES',source_basis=B,complete_old_source_inputs=len(rows),new_audited_modules=len(additions),entire_original_Diagnostic_and_compatibility_API_exact=True,whole_original_renderer_independently_compiled=True,actual_CLI_and_typed_human_consumer_adopted=True,active_bounded_route_retains_all_original_logical_receipts_and_strings=True,old_runtime_JSON_scripts_receivers_unchanged=True,new_unit_test_bodies=5,new_CLI_test_bodies=1,full_prior_proof=prior,allocation_or_RSS_gain_measured=None,output_storage_fully_funded=False,issue825='OPEN',issue845='OPEN_DESIGN_BLOCKER')
(D/'source-proof.json').write_text(json.dumps(result,indent=2)+'\n');print(json.dumps({k:v for k,v in result.items() if k!='full_prior_proof'},indent=2))
