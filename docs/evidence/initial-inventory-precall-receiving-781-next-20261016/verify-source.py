#!/usr/bin/env python3
"""Finite complete-source custody, not compiler/runtime/authority qualification."""
from pathlib import Path
import argparse,json,hashlib,re,subprocess,runpy,sys,io,contextlib
p=argparse.ArgumentParser();p.add_argument('--repo',type=Path,default=Path.cwd());R=p.parse_args().repo.resolve();D=R/'docs/evidence/initial-inventory-precall-receiving-781-next-20261016';data=json.loads((D/'source-inverses.json').read_text());B=data['base'];sha=lambda b:hashlib.sha256(b).hexdigest();old=lambda path:subprocess.check_output(['git','show',B+':'+path],cwd=R);view={}
assert len(data['rows'])==8 and len(data['new_sources'])==3
for row in data['rows']:
 a=(D/row['before_archive']).read_bytes();b=(R/row['source']).read_bytes();assert a==old(row['source']);assert sha(a)==row['before_sha256']and sha(b)==row['after_sha256'];s=b.decode()
 for e in row['edits']:assert s.count(e['new'])==1;s=s.replace(e['new'],e['old'],1)
 assert s.encode()==a;view[(R/row['source']).resolve()]=a
for row in data['new_sources']:
 b=(R/row['source']).read_bytes();assert sha(b)==row['sha256'];assert len(b.splitlines())==row['lines']<=1500
T='src/cleanup/inventory_builder/typed.rs';S='src/cleanup/inventory_builder/typed/receiving.rs';original=old(T).decode();current=(R/T).read_text()
def entry(text,name):
 a=text.index('pub(crate) fn '+name+'(');return text[a:text.index('// Sole work-failure producer:',a)]
saved=(R/'src/cleanup/inventory_builder/typed/before_initial.rs').read_text();assert saved[saved.index('pub(crate) fn build('):]==entry(original,'build')
body=entry(current,'build_after_initial_receiving').replace('build_after_initial_receiving(', 'build(',1).replace('    #[cfg(test)]\n    note_initial_entry(physical, remaining);\n','',1)
initial='''    match physical.reserve(INITIAL_BYTES) {
        Ok(()) => {}
        Err(_) => return Err(InventoryFailure(Kind::Receiving)),
    }
    debit(remaining, INITIAL_CONTROL_INITIALIZERS)?;
'''
body=body.replace(') -> Result<CleanupInventory, InventoryFailure> {\n',') -> Result<CleanupInventory, InventoryFailure> {\n'+initial,1);assert body==entry(original,'build')
a=original.index('// Sole work-failure producer:');b=current.index('// Sole work-failure producer:');tail=current[b:current.index('\n#[cfg(test)]\nthread_local! {\n    static INITIAL_ENTRIES',b)]
assert tail.replace('pub(crate) fn debit(', 'fn debit(',1)==original[a:]
orig=old(S).decode();rec=(R/S).read_text();pat=r"type Controls<[^=]+> = \((.*?)\n\);";assert re.search(pat,rec,re.S)[0]==re.search(pat,orig,re.S)[0]
pat=r"type Initial<[^=]+> = \((.*?)\n\);";assert len([x for x in re.search(pat,rec,re.S)[1].splitlines()if x.strip()])==44
assert 'pub(crate) const INITIAL_CONTROL_INITIALIZERS: usize = 44;'in rec
saved=(R/'src/cleanup/inventory_builder/typed/before_initial_receiving.rs').read_text().replace('pub(in super::super) const INITIAL_', 'pub(super) const INITIAL_');fragment=orig[orig.index('type Initial<'):orig.index('macro_rules! receiving')];assert fragment in saved
paid=(R/'src/cleanup_plan/replay/inventory_coverage/paid.rs').read_text();assert paid.index('admit_initial!(budget, function, permit)')<paid.index('let inventory = {')<paid.index('build_prepaid_typed_flat_record_inventory(')
macro=paid[paid.index('macro_rules! admit_initial'):paid.index('pub(super) use admit_initial;')];assert macro.index('reserve_bytes(')<macro.index('debit_typed_inventory_initial(');assert 'fn 'not in macro and 'charge('not in macro and 'return Err($permit.refuse('in macro
# Production traversal/shape helpers and sealed max alternatives stay complete.
fixed=['src/cleanup/inventory_builder.rs','src/cleanup_plan/replay/inventory_coverage.rs','src/cleanup_plan/replay/physical_storage.rs','src/cleanup_plan/replay/physical_budget.rs','src/cleanup_plan/replay/lexical_replay.rs','src/cleanup_plan/replay/scalar_replay.rs']+['src/cleanup/inventory_builder/typed/'+n+'.rs'for n in ['failure','queries','selected','traversal']]
for path in fixed:assert (R/path).read_bytes()==old(path)
for packet in ['typed-raw-inventory-paid-boundary-781-next-20261016','static-initial-layout-781-next-20261016']:
 for path in subprocess.check_output(['git','ls-tree','-r','--name-only',B,'--','docs/evidence/'+packet],cwd=R).decode().splitlines():assert (R/path).read_bytes()==old(path)
# Replay the unchanged prior algorithms only after restoring their entire60ad inputs.
rb,rt,argv=Path.read_bytes,Path.read_text,sys.argv
try:
 def readb(p):return view.get(p.resolve(),rb(p))
 def readt(p,*args,**kw):return view[p.resolve()].decode(kw.get('encoding')or'utf-8')if p.resolve()in view else rt(p,*args,**kw)
 Path.read_bytes,Path.read_text=readb,readt;sys.argv=['verify-source.py','--repo',str(R)];out=io.StringIO()
 with contextlib.redirect_stdout(out):runpy.run_path(str(R/'docs/evidence/static-initial-layout-781-next-20261016/verify-source.py'),run_name='__main__')
 assert 'PASS_SOURCE_ONLY'in out.getvalue()
finally:Path.read_bytes,Path.read_text,sys.argv=rb,rt,argv
print(json.dumps({'status':'PASS_SOURCE_ONLY','whole_inputs':8,'new_Rust_owners':3,'initial_actual_fields':44,'common_actual_fields_unchanged':32,'whole_60ad_entry_and_initial_layout_independent':True,'old_Diagnostic_builder_callback_checker_and_physical_primitive_exact':True,'prior_source_algorithms_replayed':True,'issue781':'OPEN_PER_METHOD_ACQUISITION_SCHEMA_SUPPLEMENTAL_AND_OTHER_FAMILIES','rust_executed':False},sort_keys=True))
