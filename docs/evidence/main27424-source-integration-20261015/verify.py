#!/usr/bin/env python3
"""Bounded complete branch source joins; no compiler/test execution."""
from pathlib import Path
import json,hashlib,subprocess,contextlib,io,re
R=Path(__file__).resolve().parents[3];D=Path(__file__).resolve().parent;h=lambda b:hashlib.sha256(b).hexdigest();g=lambda *a:subprocess.check_output(['git',*a],cwd=R)
views={}
for stage in ['7f','523','main']:
 rows=json.loads((D/f'final-to-{stage}.json').read_text());views[stage]={}
 for r in rows:
  p=R/r['source'];b=p.read_bytes();assert h(b)==r['current_sha256'];text=b.decode()
  for e in r['edits']:assert text.count(e['new'])==1;text=text.replace(e['new'],e['old'],1)
  assert h(text.encode())==r['original_sha256'];assert text.encode()==(R/r['archive']).read_bytes()==g('show',r['checkpoint_revision']+':'+r['source']);views[stage][p.resolve()]=text.encode()
def replay(stage,path):
 rb,rt=Path.read_bytes,Path.read_text;aliases=views[stage];script=R/path;code=script.read_text()
 try:
  Path.read_bytes=lambda p,*a,**k:aliases[p.resolve()] if p.resolve() in aliases else rb(p,*a,**k)
  Path.read_text=lambda p,*a,**k:aliases[p.resolve()].decode() if p.resolve() in aliases else rt(p,*a,**k)
  with contextlib.redirect_stdout(io.StringIO()) as out:exec(compile(code,str(script),'exec'),{'__file__':str(script),'__name__':'__main__'})
 finally:Path.read_bytes,Path.read_text=rb,rt
 return json.loads(out.getvalue())
proof7f=replay('7f','docs/evidence/field-update-hir-visitor-836-20261015/verify.py');proof523=replay('523','docs/evidence/compact-old-receiver-compile-fix-844-20261015/verify.py')
for row in json.loads((D/'exact-main-inputs.json').read_text()):assert h((R/row['source']).read_bytes())==row['sha256']==h(g('show',row['revision']+':'+row['source']))
# Complete actual MAIN source guard is replayed against its named whole inputs.
def main(path):return views['main'].get((R/path).resolve(),(R/path).read_bytes()).decode()
old=(R/'docs/evidence/structural-call-cursor-v1/calls.before.rs.txt').read_text();a=old.index('pub(super) struct CallCursor');b=old.index('\nenum Signature');owner=old[a:b]
assert main('src/cleanup_plan/replay/structural_control/calls.rs').replace('mod cursor;\npub(super) use cursor::CallCursor;\n',owner)==old
assert owner in (R/'src/cleanup_plan/replay/structural_control/calls/cursor_before.rs').read_text()
new=main('src/cleanup_plan/replay/structural_control/calls/cursor.rs');nf=new[new.index('    pub(super) fn find('):new.index('\n}\n\n#[cfg(test)]')+1];of=owner[owner.index('    fn find('):len(owner)-2]
assert nf.replace('pub(super) fn find','fn find').replace('        self.grow(function, 1, budget)?;\n','').replace('if depth == DEPTH','if depth == self.frames.len()').replace('                    self.grow(function, depth + 1, budget)?;\n','')==of
# Complete immediate677 generic cursor is independently compiled, only find visibility adapts.
oldgeneric=g('show','677e658:src/cleanup_plan/replay/structural_control/calls.rs').decode();chunk=oldgeneric[oldgeneric.index('pub(super) struct CallCursor'):oldgeneric.index('\nenum Signature')]
before=(R/'src/cleanup_plan/replay/structural_control/calls/cursor_before_selected.rs').read_text();prefix='//! Complete immediate677 generic selected-policy cursor.\nuse super::*;\n\n';assert before==prefix+chunk.replace('    fn find<E: Policy>(', '    pub(super) fn find<E: Policy>(',1)
# Caller dispatch/Kind/receipts remain677; only old fixed owner is replaced with new module.
actual=(R/'src/cleanup_plan/replay/structural_control/calls.rs').read_text();assert actual==oldgeneric.replace(chunk,'mod cursor;\npub(super) use cursor::CallCursor;\n',1)
production=(R/'src/cleanup_plan/replay/structural_control/calls/cursor.rs').read_text();assert production.count('E::enter::<')==3
assert production.index('let permit = E::enter::<()>')<production.index('budget.reserve_bytes(function, 0')<production.index('if required <= self.frames.len()')
assert 'replay_error(' not in production and production.count('E::static_error(')==5
# MAIN help timing intent is already covered by the stronger complete677 dual-domain test.
help=(R/'tests/offline_package/resolver_cli.rs').read_bytes();assert help==g('show','677e658:tests/offline_package/resolver_cli.rs')
mhelp=g('show','27424cdcd:tests/offline_package/resolver_cli.rs').decode()
def restored(s):
 start=s.index('    const RESTORED:');return s[start:s.index('    ];',start)+6]
assert restored(mhelp).replace('[--native-max-output-bytes N] [--native-timing]','[--native-max-output-bytes N]')==restored(help.decode())
# Every new child joins all three full source roots; all original packet trees stay exact.
children=['cursor.rs','cursor/tests.rs','cursor_before.rs','cursor_before_selected.rs']
for path in ['tests/hir_module_boundaries.rs','tests/workspace/semantic_graph/source_inventory.rs','crates/semaprax-native-rust-interop-builder/src/implementation/tests/ledger_capacity/source_inventory.rs']:
 text=(R/path).read_text()
 for child in children:assert text.count('structural_control/calls/'+child+'"')==1
assert len((R/'crates/semaprax-native-rust-interop-builder/src/implementation/tests/ledger_capacity/source_inventory.rs').read_text().splitlines())==1500
print(json.dumps({'status':'SOURCE_ONLY_NOT_EXECUTABLY_QUALIFIED','named_full_views':{s:len(v) for s,v in views.items()},'actual_MAIN_source_guard_preserved':True,'complete_immediate_generic677_and_MAIN_concrete_cursors_preserved':True,'generic_new_find_grow_paid_entries':3,'MAIN_growth_backing_header_work_depth_preserved':True,'stronger_frozen_help_677_controls_preserved':True,'new_children_all_three_audits':4,'prior_7f_proof':proof7f['status'],'prior_523_proof':proof523['status'],'rust_execution':False,'application_or_pin_pass_claim':False},indent=2))
