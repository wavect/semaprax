"""Latest MAIN source integration; all old guard scripts and JSON stay immutable."""
from pathlib import Path
import contextlib,hashlib,io,json,subprocess
R=Path.cwd();D=Path(__file__).resolve().parent;B='397326184614b463bee6438cf85f6fae905cd97a';M='66517a983';sha=lambda b:hashlib.sha256(b).hexdigest()
rows=json.loads((D/'current-to-397.json').read_text());additions=json.loads((D/'new-owners.json').read_text());introduced={(R/r['source']).resolve() for r in additions}
def apply(rows,p,b):
 rs=[r for r in rows if r['source']==p];assert len(rs)<=1
 if not rs:return b
 r=rs[0];assert sha(b)==r['current_sha256'],p
 for e in r['edits']:
  new=e['new'].encode();assert b.count(new)==1,p;b=b.replace(new,e['old'].encode(),1)
 assert sha(b)==r['original_sha256'];return b
negative=0
for r in rows:
 p=r['source'];raw=(R/p).read_bytes();old=apply(rows,p,raw);assert old==(R/r['archive']).read_bytes()==subprocess.check_output(['git','show',B+':'+p])
 for bad in [raw+b' ',b' '+raw]:
  try:apply(rows,p,bad)
  except AssertionError:negative+=1
  else:raise AssertionError('altered source accepted')
for r in additions:assert sha((R/r['source']).read_bytes())==r['current_sha256']
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


prior=replay('docs/evidence/retained-registry-833-assembly-20261015/verify.py',rows,B)
assert prior['production_and_owning_modules_exact_reviewed_c3']
for r in json.loads((D/'main-input-bindings.json').read_text()):
 p=r['source'];raw=(R/p).read_bytes();assert sha(raw)==r['sha256'] and raw==subprocess.check_output(['git','show',M+':'+p])
# Reproduce the complete original source inverse authored by MAIN's hygiene owner.
p='src/ast/expression_statement.rs';current=(R/p).read_text();start=current.index('// Grow only physical scratch;');end=current.index('fn pattern_names');assert sha(current[start:end].encode())=='51465710d1df111f979fa3055fb3712f74070e87b35e82586b00ab035bb3a7c2'
projected=current[:start]+current[end:]
for new,old in [('    let mut stack = Vec::new();\n    if !grow_frames(&mut stack, 1, &mut reserve) {','    if !reserve(std::mem::size_of::<[Option<(&Expr, usize, usize)>; 513]>()) {'),('    let mut patterns = [None; 129];\n','    let mut patterns = [None; 129];\n    let mut stack = [None; 513];\n'),('if len == 513 || !grow_frames(&mut stack, len + 1, &mut reserve) {','if len == stack.len() {'),('\n#[cfg(test)]\nmod tests;\n','')]:
 assert projected.count(new)==1;projected=projected.replace(new,old,1)
assert projected==(R/'docs/evidence/format-hygiene-scratch-v1/expression_statement.before.rs.txt').read_text() and sha(projected.encode())=='cefdc29bb8580857812046848522f3644eb4f132ab4f0c799045076d65f7cf18'
# Every inherited artifact stays unchanged. MAIN may only add its new evidence.
changes=subprocess.check_output(['git','diff','--name-status',B,'--','docs/evidence'],text=True).splitlines();assert all(x.startswith('A\t') for x in changes),changes
for p in ['tests/hir_module_boundaries.rs','tests/workspace/semantic_graph/source_inventory.rs','crates/semaprax-native-rust-interop-builder/src/implementation/tests/ledger_capacity/source_inventory.rs']:
 s=(R/p).read_text()
 for r in additions:assert s.count(r['source']+'"')==1
 assert len(s.splitlines())<=1500
assert (R/'Cargo.toml').read_bytes()==subprocess.check_output(['git','show',B+':Cargo.toml']) and (R/'Cargo.lock').read_bytes()==subprocess.check_output(['git','show',B+':Cargo.lock'])
result=dict(status='SOURCE_ONLY_NO_RUST_GATES',inherited_checkpoint=B,actual_MAIN=M,whole_previous_inputs=len(rows),new_audited_modules=len(additions),altered_input_refusals=negative,full_previous_proof=prior,MAIN_hygiene_complete_original_restored=True,all_old_runtime_JSON_and_scripts_unchanged=True,dev_dependencies_and_lock_unchanged_since_pilot=True,Rust_execution=False,paired_measurement=False)
(D/'source-proof.json').write_text(json.dumps(result,indent=2)+'\n');print(json.dumps({k:v for k,v in result.items() if k!='full_previous_proof'},indent=2))
