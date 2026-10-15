"""Exact source-only proof. Never compiles or executes Rust or reports runtime gains."""
from pathlib import Path
import hashlib,json,re
R=Path(__file__).resolve().parents[3];D=Path(__file__).resolve().parent
sha=lambda b:hashlib.sha256(b).hexdigest()
packet=json.loads((D/'source-inverses.json').read_text())
for row in packet['owners']+json.loads((D/'documentation-inverses.json').read_text()):
 current=(R/row['source']).read_bytes();old=(R/row['archive']).read_bytes()
 assert sha(current)==row['current_sha256'] and sha(old)==row['original_sha256']
 text=current.decode()
 for e in row['edits']:
  assert text.count(e['new'])==1;text=text.replace(e['new'],e['before'],1)
 assert text.encode()==old
for child in packet['children']:assert sha((R/child['source']).read_bytes())==child['sha256']
def authored(path):
 row=next(row for row in packet['owners']if row['source']==path);old=(R/row['archive']).read_text()
 return next(e['new']for e in row['edits']if e['before']==old),old
base='src/workspace_graph/expected_projection/private_layout'
root,old=authored('src/workspace_graph.rs');assert root.replace('        layouts.release_scratch();\n','',1)==old
assert root.index('        layouts.release_scratch();')<root.index('let (core, overflowed, consumed, refusal)',root.index('        layouts.release_scratch();'))
layout,oldlayout=authored(base+'.rs')
a=oldlayout.index('    fn derive_with_work(');b=oldlayout.index('    pub(super) fn add_costs(',a);oldderive=oldlayout[a:b]
assert oldderive.replace('fn derive_with_work(','fn derive_before_scratch(',1)in layout
newderive=layout.split('    fn derive_with_scratch(',1)[1].split('    pub(super) fn add_costs(',1)[0]
expected=oldderive.replace('    fn derive_with_work(','',1).replace('caller: &Program,',"caller: &'a Program,",1).replace('        ledger: Option<&std::rc::Rc<ledger::Ledger>>,','        ledger: Option<&std::rc::Rc<ledger::Ledger>>,\n        census: &mut Option<names::Census<\'a>>,',1).replace('names::first(caller)?','names::first_reusing(caller, census, remaining)?')
expected=expected.replace('            .ok_or_else(refusal)?;\n        reserve_builder_structure(\n            capacity','            .ok_or_else(refusal)?;\n        let capacity = capacity.min(count);\n        reserve_builder_structure(\n            capacity',1)
assert newderive==expected,'both passes/work/order/checked total fold preserved, plus exact834 min'
capacity=json.loads((D/'capacity-bound-834.json').read_text());assert sha((D/'original/private_layout-834.rs').read_bytes())==capacity['source_sha256']
assert '        let capacity = capacity.min(count);'in (D/'original/private_layout-834.rs').read_text()
names,oldnames=authored(base+'/names.rs');oldfirst=oldnames.split('pub(super) fn first(',1)[1].split('\nfn refusal()',1)[0]
assert 'pub(super) fn first('+oldfirst in names
kept=names[:names.index('\npub(super) fn first_reusing')]+names[names.index('\nfn refusal()'):]
kept=kept[:kept.index('\n#[cfg(test)]\nimpl Census')]
kept=kept.replace('pub(super) struct Census<','struct Census<',1)
start=kept.index('    fn reserve_reuse()');end=kept.index('    fn new()',start)
kept=kept[:start]+kept[end:]
assert kept.rstrip()==oldnames.rstrip(),'all complete old census methods and literals unchanged'
oldregistry=(D/'original/registry.rs').read_text();assert (R/(base+'/before_registry.rs')).read_text()==oldregistry.replace('#[cfg(test)]\n#[path = "registry/tests.rs"]\nmod tests;\n','').replace('LayoutClosure::derive_with_work(','LayoutClosure::derive_before_scratch(',1)
assert (R/(base+'/owner_index.rs')).read_bytes()==(D/'original/owner_index.rs').read_bytes()
registry,_=authored(base+'/registry.rs')
for term in ['census: RefCell<Option<super::names::Census<\'a>>>','std::mem::size_of::<Self>()','&self.programs[index]','&mut self.census.borrow_mut()','self.census.borrow_mut().take();','self.release_scratch();']:assert term in registry
assert names.index('super::debit(work, 1)?;')<names.index('*held = Some(Census::new()?);')
assert names.index('super::debit(work, 5)?;')<names.index('census.expressions.clear();')
assert names.index('Census::reserve_reuse()?;')<names.index('let census = held.as_mut()')
a=names.index('    fn reserve_reuse()');b=names.index('    fn new()',a)
reuse=names[a:b]
original_pay=oldnames.split('    fn new()',1)[1].split('        let expressions = Vec::with_capacity(DEPTH);',1)[0].split('        let bytes =',1)[1]
assert reuse.split('        let bytes =',1)[1].split('        reserve_builder_structure(bytes)',1)[0]==original_pay.split('        reserve_builder_structure(bytes)',1)[0]
assert 'transmute'not in names+registry and 'unsafe {'not in names+registry
inventory=(R/'tests/workspace/semantic_graph/source_inventory.rs').read_text()
for child in packet['children']:assert child['source']in inventory
pending=json.loads((D/'pending-selectors.json').read_text());assert pending['new']==7 and pending['required_existing']==11
for row in pending['checks']:
 assert sha((R/row['source']).read_bytes())==row['source_sha256']
 assert not row['source_binding']['ignored']
 assert 'fn '+row['source_binding']['name']+'('in (R/row['source']).read_text()
for child in packet['children']:
 p=R/child['source']
 for ref in re.findall(r'include_str!\(\s*"([^"]+)"',p.read_text()):assert(p.parent/ref).is_file()
proof={'status':'SOURCE_ONLY_UNEXECUTED_NO_COMPILER_OR_RUNTIME','basis':packet['basis'],'whole_changed_source_owners':5,'documentation_originals':2,'new_children':packet['children'],'complete_old_registry_executable':True,'complete_old_census_and_layout_derivation_bodies_byte_exact':True,'old_registry_adapter':'only old test module registration omitted; whole old registry calls cfgtest old940 method by adapted name','old_ownership_index_full_source_unchanged':True,'all_old_traversals_work_error_order_and_checked_total_fold_retained':True,'new_helper_inherits_exact_834_capacity_min':capacity,'new_control_work_operations_per_successful_census':6,'new_teardown_work_prepaid_per_acquired_generation':1,'new_header_prepayment':'enlarged original LayoutRegistry Self size, charged before census backing','scratch_release':'before each core owner acquisition; monotonic receipt unchanged; later cold caller reacquires','retained_scratch_overlap_disclosed':True,'distinct_child_physical_permission':'Fresh full original census backing reservation in each child before borrowing held vectors; prior receipt grants no permission','reservation_saving_blocked':'No persistent parent physical Budget owner exists on this route; semantic ledger and numeric receipt are insufficient','residuals':['per-derivation layout traversal frames','per-derivation ownership-index facts and frames'],'all_new_sources_joined_into_existing_audit':True,'source_registered_new_selectors':7,'source_registered_existing_selectors':11,'actual_execution':False,'actual_before_after_metrics':None,'physical_or_reservation_gain_claim':False,'global_custody':'Prior global Source36/packets unchanged; integrator must compose all five full owners after adoption and preserve other lanes including 834 capacity min'}
(D/'source-proof.json').write_text(json.dumps(proof,indent=2)+'\n');print('PASS source-only5fullsource inverses+2docs+2children; whole independent old registry/census/derive,all7new+11exactexistingcontrols,noRustexecution')
