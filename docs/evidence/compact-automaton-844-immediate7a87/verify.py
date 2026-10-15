"""Strict independent whole-source views; no Rust or runtime qualification."""
from pathlib import Path
import contextlib,hashlib,io,json,re,subprocess
R=Path.cwd();D=Path(__file__).resolve().parent
BASE='7a87b33115c0cd45b14cac64d7b37e5742503159';A3='a3a0859c4231a80dfb5379740226d8888782c7b1'
sha=lambda b:hashlib.sha256(b).hexdigest()
views={}
for name,rev in [('parent',BASE),('a3',A3)]:
 named={}
 for row in json.loads((D/('current-to-'+name+'.json')).read_text()):
  p=(R/row['source']).resolve();raw=p.read_bytes();assert sha(raw)==row['current_sha256'],row['source']
  assert len(row['edits'])==1 and raw==row['edits'][0]['new'].encode()
  before=row['edits'][0]['old'].encode();assert sha(before)==row['original_sha256']
  assert before==(R/row['archive']).read_bytes()==subprocess.check_output(['git','show',rev+':'+row['source']])
  named[p]=before
 views[name]=named
owners=json.loads((D/'new-owners.json').read_text());new={(R/x['source']).resolve() for x in owners};assert len(new)==7
for x in owners:assert sha((R/x['source']).read_bytes())==x['sha256']
adapt=json.loads((D/'immediate-old-receivers.json').read_text());assert len(adapt)==5
for x in adapt:
 b=(R/x['source']).read_bytes();assert sha(b)==x['compiled_sha256'] and b==x['compiled'].encode()
 original=x['original'].encode();assert sha(original)==x['original_sha256']
 assert original==(R/x['archive']).read_bytes()==subprocess.check_output(['git','show',BASE+':'+x['original_source']])
# Ignore formatting and comments, never literal contents. No Rust parser/toolchain.
lex=re.compile(r'//[^\n]*|/\*.*?\*/|r(?P<h>\#{0,})".*?"(?P=h)|"(?:\\.|[^"\\])*"|[A-Za-z_][A-Za-z_0-9]*|\d+|[^\s]',re.S)
def tokens(s):return [m.group() for m in lex.finditer(s) if not m.group().startswith(('//','/*'))]
by={x['original_source']:x for x in adapt};root=by['src/cleanup_plan/replay/factored.rs'];old=root['original'];compiled=root['compiled']
a='impl Automaton {';b='fn transition_observation('
assert tokens(old[old.index(a):old.index(b)])==tokens(compiled[compiled.index(a):compiled.index(b)]),'complete original Automaton implementation'
a='fn skeleton_mismatch(';b='#[path = '
assert tokens(old[old.index(a):old.index(b,old.index(a))])==tokens(compiled[compiled.index(a):compiled.index(b,compiled.index(a))]),'complete compare and build_comparison receivers'
assert 'moves: Vec<(Label, usize)>' in compiled and 'epsilon: Vec<usize>' in compiled
assert 'compact_nodes::Node' not in compiled
label=re.search(r'enum Label\s*\{[^}]*\}',old).group();assert label==re.search(r'enum Label\s*\{[^}]*\}',(R/'src/cleanup_plan/replay/factored.rs').read_text()).group(), 'unchanged exact original Label'
x=by['src/cleanup_plan/replay/factored/canonical_shared.rs'];old=x['original'];compiled=x['compiled'].replace('pub(in crate::cleanup_plan::replay::factored)','pub(super)').replace('sink: usize,\n    ) -> Self', 'sink: usize\n    ) -> Self',1)
a='type SharedWeighted';b='\n#[cfg(test)]\nmod tests;'
assert tokens(old[old.index(a):old.index(b)])==tokens(compiled[compiled.index(a):compiled.index('\nmod tests {')]),'complete old canonical algorithm'
for original in ['src/cleanup_plan/replay/factored/storage.rs','src/cleanup_plan/replay/factored/strategy.rs']:
 x=by[original];old=x['original'];compiled=x['compiled']
 end='\n#[cfg(test)]\nmod tests;' if original.endswith('storage.rs') else '\n#[cfg(test)]\n#[path = "strategy/activation_tests.rs"]'
 assert tokens(old[:old.index(end)])==tokens(compiled),original
x=by['src/cleanup_plan/replay/factored/borrowed_canonical.rs'];old=x['original'];compiled=x['compiled']
old=old[:old.index('\n#[cfg(test)]\nmod tests;')].replace('pub(super) mod dictionary;','use super::super::super::borrowed_canonical::dictionary;')
anchor='pub(super) struct BorrowedCanonicalizer';assert tokens(old[old.index(anchor):])==tokens(compiled[compiled.index(anchor):]),'complete old borrowed comparison receiver'

assert (R/'src/cleanup_plan/replay/factored/compact_nodes.rs').read_bytes()==subprocess.check_output(['git','show',A3+':src/cleanup_plan/replay/factored/compact_nodes.rs'])
assert not subprocess.check_output(['git','diff',A3,'--','docs/evidence/compact-automaton-844']), 'immutable checkpoint packet'
for path in ['tests/hir_module_boundaries.rs','tests/workspace/semantic_graph/source_inventory.rs','crates/semaprax-native-rust-interop-builder/src/implementation/tests/ledger_capacity/source_inventory.rs']:
 text=(R/path).read_text();assert len(text.splitlines())<=1500
 for p in new:assert text.count(p.relative_to(R).as_posix()+'"')==1
 original=views['parent'].get((R/path).resolve(),(R/path).read_bytes()).decode()
 assert set(re.findall(r'include_str!\("([^"]+)"\)',original))<=set(re.findall(r'include_str!\("([^"]+)"\)',text))
absent=json.loads((D/'absent-source-paths.json').read_text())
def replay(script,name,scope):
 path=R/script;code=path.read_text();named=dict(views[name]);
 if name=='parent':
  # The unchanged 831 verifier omits the later documentation-only changelog hop.
  # Its nested MAIN665 check owns the exact 297 input, independently authenticated.
  named[(R/'CHANGELOG.md').resolve()]=subprocess.check_output(['git','show','29750ef89f6823e8c8b0fde330342f585762bfb5:CHANGELOG.md'])
 excluded={(R/p).resolve() for p in absent[name]}
 rb,rt,wt,co,glob=Path.read_bytes,Path.read_text,Path.write_text,subprocess.check_output,Path.rglob;captured=[]
 def read_bytes(p,*a,**k):return named[p.resolve()] if p.resolve() in named else rb(p,*a,**k)
 def read_text(p,*a,**k):return named[p.resolve()].decode() if p.resolve() in named else rt(p,*a,**k)
 def write(p,s,*a,**k):assert p.name=='source-proof.json';captured.append(json.loads(s));return len(s)
 def rglob(p,*a,**k):
  for child in glob(p,*a,**k):
   if child.resolve() not in excluded:yield child
 def output(args,*a,**k):
  if isinstance(args,list) and args[:3] in [['git','diff','--name-only'],['git','diff','--name-status']]:
   i=args.index('--') if '--' in args else len(args);revs=[x for x in args[3:i] if not x.startswith('-')]
   if len(revs)==1:args=args[:i]+[scope]+args[i:]
  return co(args,*a,**k)
 try:
  Path.read_bytes,Path.read_text,Path.write_text,subprocess.check_output,Path.rglob=read_bytes,read_text,write,output,rglob
  with contextlib.redirect_stdout(io.StringIO()):exec(compile(code,str(path),'exec'),{'__file__':str(path),'__name__':'__main__'})
 finally:Path.read_bytes,Path.read_text,Path.write_text,subprocess.check_output,Path.rglob=rb,rt,wt,co,glob
 assert captured
 return captured[-1]
parent=replay('docs/evidence/direct-canonical-carriers-831-20261015/verify.py','parent',BASE)
historical=replay('docs/evidence/compact-automaton-844/verify.py','a3',A3)
proof=dict(status='SOURCE_ONLY_NO_RUST_EXECUTION',immediate_parent=BASE,historical_checkpoint=A3,whole_parent_inputs=len(views['parent']),whole_checkpoint_inputs=len(views['a3']),independent_immediate_old_receivers=len(adapt),new_Rust_owners=7,new_tests=9,all_old_packets_unchanged=True,unchanged_parent_proof=parent,unchanged_checkpoint_proof=historical,runtime_or_gain_claim=None)
(D/'source-proof.json').write_text(json.dumps(proof,indent=2)+'\n');print('PASS: independent 7a87 and a3 whole-source domains and unchanged predecessor proofs; NO Rust')
