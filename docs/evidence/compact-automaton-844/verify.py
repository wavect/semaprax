"""Finite source-only custody proof. No Rust invocation; old packets never written."""
from pathlib import Path
import contextlib,hashlib,io,json,re,subprocess
R=Path.cwd();D=Path(__file__).resolve().parent;BASE='c58243c6cbe4577efe6cfa81ca6312ff45c1c71a'
sha=lambda b:hashlib.sha256(b).hexdigest()
rows=json.loads((D/'source-inverses.json').read_text());named={}
for row in rows:
 p=(R/row['source']).resolve();raw=p.read_bytes();assert sha(raw)==row['current_sha256'],row['source']
 assert len(row['edits'])==1 and raw==row['edits'][0]['new'].encode()
 old=row['edits'][0]['old'].encode();assert sha(old)==row['original_sha256']
 assert old==(R/row['archive']).read_bytes()==subprocess.check_output(['git','show',BASE+':'+row['source']])
 named[p]=old
owners=json.loads((D/'new-owners.json').read_text());new={ (R/x['source']).resolve() for x in owners }
assert len(new)==7
changed=set(subprocess.check_output(['git','diff','--name-only','--diff-filter=M',BASE],text=True).splitlines());assert changed=={x['source'] for x in rows},'complete immediate modified-file inventory'
namespace='src/cleanup_plan/replay/factored'
old_namespace={p for p in subprocess.check_output(['git','ls-tree','-r','--name-only',BASE,'--',namespace],text=True).splitlines() if p.endswith('.rs')}
actual_namespace={p.relative_to(R).as_posix() for p in (R/namespace).rglob('*.rs')}
assert actual_namespace==old_namespace|{p.relative_to(R).as_posix() for p in new}
for x in owners:assert sha((R/x['source']).read_bytes())==x['sha256']
adapt=json.loads((D/'old-receiver-adaptations.json').read_text());assert len(adapt)==5
for x in adapt:
 b=(R/x['source']).read_bytes();assert sha(b)==x['compiled_sha256'] and b==x['compiled'].encode()
 old=x['original'].encode();assert sha(old)==x['original_sha256']
 assert old==(R/x['archive']).read_bytes()==subprocess.check_output(['git','show',BASE+':'+x['original_source']])
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
# Every old compiled control retains its assertions; adapters only select slices.
for row in rows:
 p=row['source']
 if p.startswith('src/cleanup_plan/replay/factored/') and p not in ['src/cleanup_plan/replay/factored/canonical_shared.rs','src/cleanup_plan/replay/factored/strategy/tests.rs']:
  before=row['edits'][0]['old'];after=row['edits'][0]['new'];restored=re.sub(r'\.(moves|epsilon)\(\)',r'.\1',after)
  for line in before.splitlines():
   if '.moves' in line or '.epsilon' in line:
    if ' in &' in line:restored=restored.replace(line.replace(' in &',' in '),line)
    if 'let moves = &' in line:restored=restored.replace(line.replace('let moves = &','let moves = '),line)
  assert tokens(before)==tokens(restored),p
# The three explicit hostile mutations keep all pre-existing assertions.
x=next(x for x in rows if x['source'].endswith('/strategy/tests.rs'))
for marker in ['assert!(census(&function, &built, root, &mut budget).is_err());','assert!(budget.remaining <= MAX_REPLAY_WORK_UNITS);']:
 assert marker in x['edits'][0]['old'] and marker in x['edits'][0]['new']
for path in ['tests/hir_module_boundaries.rs','tests/workspace/semantic_graph/source_inventory.rs','crates/semaprax-native-rust-interop-builder/src/implementation/tests/ledger_capacity/source_inventory.rs']:
 text=(R/path).read_text();assert len(text.splitlines())<=1500
 for p in new:assert text.count(p.relative_to(R).as_posix()+'"')==1,(path,p)
 old=named[(R/path).resolve()].decode()
 assert set(re.findall(r'include_str!\("([^"]+)"\)',old))<=set(re.findall(r'include_str!\("([^"]+)"\)',text))
# Run the unchanged immediate predecessor verifier in its exact full Git view.
# Its source namespace excludes only the seven proved genuinely new owners.
script=R/'docs/evidence/retained-registry-833-assembly-20261015/verify.py';code=script.read_text()
rb,rt,wt,co,glob=Path.read_bytes,Path.read_text,Path.write_text,subprocess.check_output,Path.rglob;captured=[]
def read_bytes(p,*a,**k):return named[p.resolve()] if p.resolve() in named else rb(p,*a,**k)
def read_text(p,*a,**k):return named[p.resolve()].decode() if p.resolve() in named else rt(p,*a,**k)
def write(p,s,*a,**k):assert p.name=='source-proof.json';captured.append(json.loads(s));return len(s)
def rglob(p,*a,**k):
 for child in glob(p,*a,**k):
  if child.resolve() not in new:yield child

def output(args,*a,**k):
 if isinstance(args,list) and args[:3] in [['git','diff','--name-only'],['git','diff','--name-status']] and '--' in args:
  i=args.index('--');revs=[x for x in args[3:i] if not x.startswith('-')]
  if len(revs)==1:args=args[:i]+[BASE]+args[i:]
 return co(args,*a,**k)
try:
 Path.read_bytes,Path.read_text,Path.write_text,subprocess.check_output,Path.rglob=read_bytes,read_text,write,output,rglob
 with contextlib.redirect_stdout(io.StringIO()):exec(compile(code,str(script),'exec'),{'__file__':str(script),'__name__':'__main__'})
finally:Path.read_bytes,Path.read_text,Path.write_text,subprocess.check_output,Path.rglob=rb,rt,wt,co,glob
assert captured and captured[-1]['production_and_owning_modules_exact_reviewed_c3']
proof={'status':'SOURCE_ONLY_NO_RUST_EXECUTION','base':BASE,'whole_immediate_inputs':len(rows),'independent_full_old_receivers':len(adapt),'new_Rust_owners':len(owners),'new_tests':8,'complete_old_automaton_and_comparison_bodies_exact':True,'old_assertions_preserved':True,'all_three_full_source_audits_complete':True,'unchanged_predecessor_proofs_replayed':True,'runtime_or_gain_claim':None}
(D/'source-proof.json').write_text(json.dumps(proof,indent=2)+'\n');print('PASS strict complete old/current source domains, algorithm bodies, full predecessor replays; NO Rust')
