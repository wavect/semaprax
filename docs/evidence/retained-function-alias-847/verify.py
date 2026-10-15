"""Finite strict immediate-input/custody replay. Never invokes Rust."""
from pathlib import Path
import contextlib,hashlib,io,json,re,subprocess
R=Path.cwd();D=Path(__file__).resolve().parent;BASE='7a87b33115c0cd45b14cac64d7b37e5742503159';sha=lambda b:hashlib.sha256(b).hexdigest()
rows=json.loads((D/'current-to-7a87.json').read_text());named={}
for row in rows:
 p=(R/row['source']).resolve();raw=p.read_bytes();assert sha(raw)==row['current_sha256'],row['source']
 assert len(row['edits'])==1 and raw==row['edits'][0]['new'].encode()
 old=row['edits'][0]['old'].encode();assert sha(old)==row['original_sha256']
 assert old==(R/row['archive']).read_bytes()==subprocess.check_output(['git','show',BASE+':'+row['source']]);named[p]=old
owners=json.loads((D/'new-owners.json').read_text())
introduced={(R/x['source']).resolve() for x in owners};assert len(introduced)==8
for x in owners:assert sha((R/x['source']).read_bytes())==x['sha256']
receivers=json.loads((D/'independent-receivers.json').read_text());assert len(receivers)==4
for x in receivers:
 current=(R/x['source']).read_bytes();assert current==x['compiled'].encode() and sha(current)==x['compiled_sha256']
 old=x['original'].encode();assert sha(old)==x['original_sha256'];assert old==(R/x['archive']).read_bytes()==subprocess.check_output(['git','show',BASE+':'+x['original_source']])
lex=re.compile(r'//[^\n]*|/\*.*?\*/|r(?P<h>\#{0,})".*?"(?P=h)|"(?:\\.|[^"\\])*"|[A-Za-z_][A-Za-z_0-9]*|\d+|[^\s]',re.S)
def tokens(s):return [m.group() for m in lex.finditer(s) if not m.group().startswith(('//','/*'))]
by={x['original_source']:x for x in receivers}
x=by['src/ast/program_source/decl_slices.rs'];assert x['compiled']==x['original']
x=by['src/hir/identity_resolution/nominal.rs'];assert x['compiled']==x['original']
x=by['src/workspace_graph/expected_projection/borrowed_imports/typed.rs'];a='fn construct_using';compiled=x['compiled'][x['compiled'].index(a):].replace('super::super::super::super::generic_type_import','super::super::super::generic_type_import').replace('super::super::super::super::owned_function_import','super::super::super::owned_function_import');original=x['original'][x['original'].index(a):x['original'].index('\n#[cfg(test)]\nmod directory_tests')];assert tokens(compiled)==tokens(original),'complete independent old constructor and directory production bodies; original tests remain registered once'
x=by['src/workspace_graph.rs'];a='fn build_resolved_core_current(';old=x['original'][x['original'].index(a):x['original'].index('\nconst MAX_CHECKED_VALUE_VISITS')];compiled=x['compiled'][x['compiled'].index(a):x['compiled'].index('\n#[cfg(test)]\nmod tests;')];assert tokens(old)==tokens(compiled),'complete independent immediate old core body'
# Separate source825 historical input, reconstructed from unchanged original edits.
prior=json.loads((R/'docs/evidence/constructor-directory-propagation-825/source-inverses.json').read_text())
active=json.loads((D/'directory825-domain.json').read_text());assert len(active)==len(prior)==3
for previous,row in zip(prior,active):
 assert row['source']==previous['source']
 raw=(R/row['source']).read_bytes();assert raw==row['edits'][0]['new'].encode() and sha(raw)==row['current_sha256']
 target=row['edits'][0]['old'];assert sha(target.encode())==row['original_sha256']==previous['current_sha256']
 original=(R/previous['archive']).read_text();assert sha(original.encode())==previous['original_sha256']
 restored=target
 for edit in previous['edits']:
  assert restored.count(edit['new'])==1;restored=restored.replace(edit['new'],edit['before'],1)
 assert restored==original and target==(R/row['archive']).read_text()
# Existing typed producer remains byte-token exact; the new branch is separate.
p='src/workspace_graph/expected_projection/borrowed_imports/typed.rs';old=named[(R/p).resolve()].decode();current=(R/p).read_text();assert tokens(current[:current.index('\n#[cfg(test)]\nmod directory_tests')])==tokens(old[:old.index('\n#[cfg(test)]\nmod directory_tests')]);a='        let rows = rows.as_array().unwrap();';b='\nmod alias;';assert current[current.index(a):current.index(b)]==old[old.index(a):], 'all original sourceguard assertions unchanged'
# Only the actual cloned retained branch disappears. No bound/cap/refund edit.
for p in ['src/workspace_graph/expected_projection/held_projection.rs','src/workspace_graph/expected_projection/uncached_peak.rs','src/workspace_graph/expected_projection/borrowed_imports/alias_clone.rs']:
 assert (R/p).read_bytes()==subprocess.check_output(['git','show',BASE+':'+p])
current=(R/'src/ast/program_source/function_alias.rs').read_text();assert 'impl Drop' not in current and 'Program {' not in current
alias=(R/'src/workspace_graph/expected_projection/borrowed_imports/typed/alias.rs').read_text();assert alias.index('let mut directory_owner')<alias.index('types.push(')<alias.index('functions.push(')
assert 'if retain_body && import.alias == original.name' in alias and 'FunctionView::checked(provider, original, &import.alias)' in alias
assert 'directory.get(start + index - authored.len())' in (R/'src/ast/program_source/decl_slices.rs').read_text()
for path in ['tests/hir_module_boundaries.rs','tests/workspace/semantic_graph/source_inventory.rs','crates/semaprax-native-rust-interop-builder/src/implementation/tests/ledger_capacity/source_inventory.rs']:
 text=(R/path).read_text();assert len(text.splitlines())<=1500
 for p in introduced:assert p.relative_to(R).as_posix()+'"' in text
 old=named[(R/path).resolve()].decode();assert set(re.findall(r'include_str!\("([^"]+)"\)',old))<=set(re.findall(r'include_str!\("([^"]+)"\)',text))
assert len((R/'src/workspace_graph.rs').read_text().splitlines())<=6018
assert not subprocess.check_output(['git','diff',BASE,'--','docs/evidence',':(exclude)docs/evidence/retained-function-alias-847'])
# Unchanged immediate predecessor proof, in its exact production source view.
# The inherited831 verifier omits a later documentation-only CHANGELOG hop;
# its nested MAIN665 table owns exact297, separately authenticated here.
named[(R/'CHANGELOG.md').resolve()]=subprocess.check_output(['git','show','29750ef89f6823e8c8b0fde330342f585762bfb5:CHANGELOG.md'])
script=R/'docs/evidence/direct-canonical-carriers-831-20261015/verify.py';code=script.read_text()
rb,rt,wt,co,glob=Path.read_bytes,Path.read_text,Path.write_text,subprocess.check_output,Path.rglob;captured=[]
def read_bytes(p,*a,**k):return named[p.resolve()] if p.resolve() in named else rb(p,*a,**k)
def read_text(p,*a,**k):return named[p.resolve()].decode() if p.resolve() in named else rt(p,*a,**k)
def write(p,s,*a,**k):assert p.name=='source-proof.json';captured.append(json.loads(s));return len(s)
def rglob(p,*a,**k):
 for child in glob(p,*a,**k):
  if child.resolve() not in introduced:yield child

def output(args,*a,**k):
 if isinstance(args,list) and args[:3] in [['git','diff','--name-only'],['git','diff','--name-status']]:
  i=args.index('--') if '--' in args else len(args);revs=[x for x in args[3:i] if not x.startswith('-')]
  if len(revs)==1:args=args[:i]+[BASE]+args[i:]
 return co(args,*a,**k)
try:
 Path.read_bytes,Path.read_text,Path.write_text,subprocess.check_output,Path.rglob=read_bytes,read_text,write,output,rglob
 with contextlib.redirect_stdout(io.StringIO()):exec(compile(code,str(script),'exec'),{'__file__':str(script),'__name__':'__main__'})
finally:Path.read_bytes,Path.read_text,Path.write_text,subprocess.check_output,Path.rglob=rb,rt,wt,co,glob
assert captured
proof=dict(status='SOURCE_ONLY_UNCOMPILED_UNEXECUTED',base=BASE,whole_immediate_inputs=len(rows),new_Rust_owners=len(owners),complete_independent_receivers=len(receivers),new_bodies=8,three_audit_joins_complete=True,old_packets_unchanged=True,old_clone_and_peak_helpers_byteexact=True,full_immediate_predecessor_replay=captured[-1],physical_gain_or_application_fit=None)
(D/'source-proof.json').write_text(json.dumps(proof,indent=2)+'\n');print('PASS complete immediate inputs, independent old receivers, all audits and unchanged prior proofs; NO Rust')
