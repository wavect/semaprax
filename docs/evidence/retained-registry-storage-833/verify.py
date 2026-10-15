"""Bounded source custody only. Never invokes Rust or rewrites old packets."""
from pathlib import Path
import json,hashlib,subprocess,contextlib,io
R=Path.cwd();D=Path(__file__).resolve().parent
BASE='0e1a061e8ef09f786e2470b311ed7d4cb28e9d42'
sha=lambda b:hashlib.sha256(b).hexdigest()
rows=json.loads((D/'current-to-0e.json').read_text());named={}
for row in rows:
 p=R/row['source'];b=p.read_bytes();assert sha(b)==row['current_sha256'],row['source']
 for e in row['edits']:
  new=e['new'].encode();assert b.count(new)==1,row['source'];b=b.replace(new,e['old'].encode(),1)
 assert sha(b)==row['original_sha256']
 assert b==(D/'before'/row['source']).read_bytes()
 assert b==subprocess.check_output(['git','show',BASE+':'+row['source']])
 named[p.resolve()]=b
P='src/workspace_graph/expected_projection/private_layout'
# Independent old production is byte exact after listed lexical nesting/field visibility adapters.
old=(D/'before'/f'{P}/registry.rs').read_text();cut=old.index('\n#[cfg(test)]\n#[path = "registry/tests.rs"]')
import re
expected=re.sub(r'(?<!:)super::','super::super::',old[:cut])
for field in ['census','derivations','remaining']:
 expected=expected.replace('    '+field+':','    pub(super) '+field+':').replace('            pub(super) '+field+':','            '+field+':')
for method in ['is_failed','work_remaining','set_remaining']:
 expected=expected.replace('    fn '+method,'    pub(super) fn '+method)
expected+='\n#[cfg(test)]\nthread_local! {static DERIVATIONS: Cell<usize> = const {Cell::new(0)};}\n'
assert (R/f'{P}/registry/before_retained.rs').read_text()==expected
# Original selection/reset/owner derivation bodies are still independent production functions.
checks=[('names.rs','pub(super) fn first_reusing','\nfn refusal()','\npub(super) fn first_owned'),
 ('owner_index.rs','    pub(super) fn build(','    pub(super) fn owner(','    pub(super) fn build_retained('),
 ('../private_layout.rs','    fn derive_with_scratch(','    pub(super) fn add_costs(','    fn derive_with_owned_storage(')]
for rel,start,end,newend in checks:
 p=(R/P/rel).resolve();before=named[p].decode();now=p.read_text()
 assert before[before.index(start):before.index(end)]==now[now.index(start):now.index(newend)]
# Every original scratch assertion remains; only its owning receiver import changes.
p=R/f'{P}/registry/scratch_tests.rs';before=named[p.resolve()].decode()
assert p.read_text()==before.replace('use super::LayoutRegistry;','use super::before_retained::LayoutRegistry;')
# Exact cached get body retained; new physical auth is only in prepare.
start='    pub(in crate::workspace_graph) fn get(';end='    /// This caller'
now=(R/f'{P}/registry.rs').read_text();assert now[now.index(start):now.index(end)]==old[old.index(start):old.index(end)]
for rel in ['tests/hir_module_boundaries.rs','tests/workspace/semantic_graph/source_inventory.rs','crates/semaprax-native-rust-interop-builder/src/implementation/tests/ledger_capacity/source_inventory.rs']:
 s=(R/rel).read_text();assert len(s.splitlines())<=1500
 for child in ['storage.rs','before_retained.rs','retained_tests.rs']:
  assert s.count(f'private_layout/registry/{child}"')==1,(rel,child)
for item in json.loads((D/'new-owners.json').read_text()):
 assert sha((R/f'{P}/registry'/item['source']).read_bytes())==item['sha256']
# Replay the unchanged completed compiler-fix verifier in the authenticated whole 0e view.
script=R/'docs/evidence/workspace-facade-hex-fix-9b-20261015/verify.py'
source=script.read_text();rb0=Path.read_bytes;rt0=Path.read_text;wt0=Path.write_text;glob0=Path.rglob;cp0=subprocess.check_output;capture={}
new_owners={ (R/f'{P}/registry/{name}').resolve() for name in ['storage.rs','before_retained.rs','retained_tests.rs'] }
old_namespace={p for p in subprocess.check_output(['git','ls-tree','-r','--name-only',BASE,'--',P],text=True).splitlines() if p.endswith('.rs')}
actual_namespace={p.relative_to(R).as_posix() for p in (R/P).rglob('*.rs')}
assert actual_namespace==old_namespace|{p.relative_to(R).as_posix() for p in new_owners}
def cp(cmd,*a,**kw):
 # Historical verifier's one implicit-working-tree source diff is evaluated
 # against the exact authenticated 0e Git tree, never physical current source.
 if cmd[:3]==['git','diff','--name-only'] and '--' in cmd and cmd.index('--')==4:
  cmd=cmd[:4]+[BASE]+cmd[4:]
 return cp0(cmd,*a,**kw)
def glob(p,*a,**kw):
 for item in glob0(p,*a,**kw):
  if item.resolve() not in new_owners:yield item

def rb(p):return named[p.resolve()] if p.resolve() in named else rb0(p)
def rt(p,*a,**kw):return named[p.resolve()].decode() if p.resolve() in named else rt0(p,*a,**kw)
def wt(p,s,*a,**kw):assert p.resolve()==(script.parent/'source-proof.json').resolve();capture.update(json.loads(s));return len(s)
try:
 Path.read_bytes=rb;Path.read_text=rt;Path.write_text=wt;Path.rglob=glob;subprocess.check_output=cp
 with contextlib.redirect_stdout(io.StringIO()):exec(compile(source,str(script),'exec'),{'__file__':str(script),'__name__':'__main__'})
finally:Path.read_bytes=rb0;Path.read_text=rt0;Path.write_text=wt0;Path.rglob=glob0;subprocess.check_output=cp0
assert capture['complete_frozen_4_57_87_92_93_source_verifier_replayed']
proof={'status':'SOURCE_ONLY_NO_RUST_EXECUTION','base':BASE,'whole_original_inputs':len(rows),'independent_complete_old_registry':True,'old_scratch_assertions_unchanged':True,'old_derivation_census_owner_bodies_exact':True,'cached_get_body_exact':True,'all_three_new_owners_in_all_three_audits':True,'complete_frozen_source_proofs_replayed':True,'new_behavior_tests':6,'new_source_tests':1,'old_issue825_failure_paths_remain_residual':True,'measured_saving':None}
(D/'source-proof.json').write_text(json.dumps(proof,indent=2)+'\n')
print('PASS strict whole-source predecessors, original bodies, complete frozen source replays; NO Rust')
