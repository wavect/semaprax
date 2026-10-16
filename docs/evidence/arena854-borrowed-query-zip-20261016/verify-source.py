"""Read-only complete source proof. No Rust build, test or layout measurement."""
from pathlib import Path
import hashlib,json,re,subprocess
R=Path(__file__).resolve().parents[3];D=Path(__file__).resolve().parent;BASE='2d87a3b89';sha=lambda b:hashlib.sha256(b).hexdigest()
def old(p):return subprocess.check_output(['git','show',BASE+':'+p],cwd=R)
rows=json.loads((D/'current-to-2d87.json').read_text());assert len(rows)==4
for row in rows:
 p=row['source'];before=old(p);raw=(R/p).read_bytes();assert sha(raw)==row['current_sha256'];assert sha(before)==row['original_sha256'];assert(D/'before'/(p.replace('/','__')+'.txt')).read_bytes()==before
 value=raw.decode()
 for e in row['edits']:assert value.count(e['new'])==1;value=value.replace(e['new'],e['old'],1)
 assert value.encode()==before
 for wrong in [raw+b' ',b'',raw[:len(raw)//2]]:assert sha(wrong)!=row['current_sha256']
tests='src/cleanup_plan/replay/factored/canonical_shared/arena_tests.rs';before=old(tests).decode();current=(R/tests).read_text();assert current.startswith(before+'\n');assert current.count('whole duplicate-signature corpus, unchanged old receiver')==1
assert before.count('assert_eq!(error, Err(Diagnostic::io("SPX-H006", String::new())))')==1
assert current.count('assert_eq!(error, Err(Diagnostic::io("SPX-H006", String::new())))')==1
assert current.count('assert_eq!(budget.remaining, work);')==before.count('assert_eq!(budget.remaining, work);')==2
memo='src/cleanup_plan/replay/factored/canonical_shared/empty_memo.rs';b=old(memo).decode();n=(R/memo).read_text();addition=''' else if budget.refusal_spent
                || (budget.pays_allocations() && crate::bounded_output::active_refused())
            {
                // Identity is already bound before reading this same account.
                // A warm poisoned owner must not consume its report or reset
                // work through a fresh zero-byte payer authentication.
                self.failed = true;
            }''';assert n.count(addition)==1;assert n.replace(addition,'',1)==b
preflight=n[n.index('pub(super) fn preflight'):n.index('pub(super) fn failed')]
assert preflight.index('binding.function !=')<preflight.index('budget.refusal_spent')
for forbidden in ['reserve_bytes(', 'authenticate_zero', 'budget_refusal(', 'remaining =', 'refusal_text.take']:
 assert forbidden not in preflight
root='src/cleanup_plan/replay/factored/canonical_shared.rs';b=old(root).decode();n=(R/root).read_text();start=b.index('                            // New index initializer/local');end=b.index('                            let key = (accepted, key);',start);before_miss=b[start:end];start2=n.index('                            // Every ID is already resolved.');end2=n.index('                            let key = (accepted, key);',start2);after_miss=n[start2:end2];assert n==b.replace(before_miss,after_miss,1)
assert 'let mut index' not in after_miss and 'index += 1' not in after_miss
for term in ['SignatureQuery::MISS_CONTROLS','reserve_array::<[&usize; 2]>','successors.into_iter().zip(&self.child_ids)','key.push((label, *id));']:
 assert term in after_miss
arena='src/cleanup_plan/replay/factored/canonical_shared/intern_arena.rs';b=old(arena).decode();n=(R/arena).read_text();assert n[n.index('impl Query<Signature>'):]==b[b.index('impl Query<Signature>'):]
assert "successors: &'a Vec<(Label, SharedWeighted)>" in n and "children: &'a Vec<usize>" in n
assert n.count('size_of::<Result<(), super::Diagnostic>>()')==3 # unchanged exit and insertion banks, plus two miss Results
for p in ['src/cleanup_plan/replay/factored/canonical_shared/before_arena.rs','src/cleanup_plan/replay/physical_budget.rs','src/cleanup_plan/replay/physical_budget/authentication.rs','src/cleanup_plan/replay/physical_storage.rs','src/bounded_output.rs']:
 assert(R/p).read_bytes()==old(p),('independent old owner or common payer changed',p)
for g in ['tests/hir_module_boundaries.rs','tests/workspace/semantic_graph/source_inventory.rs','crates/semaprax-native-rust-interop-builder/src/implementation/tests/ledger_capacity/source_inventory.rs']:
 raw=(R/g).read_text()
 for row in rows:assert row['source'] in raw,(g,row['source'])
for row in rows:
 assert len((R/row['source']).read_bytes().splitlines())<=1500
 # These four preimages are exactly16c, so the final predecessor overlay may
 # already yield the same old view after authenticating the actual raw input.
 assert subprocess.check_output(['git','show','16c6b7f56b145ed085e4e7964caa3b1716159375:'+row['source']],cwd=R)==old(row['source'])
front=(R/'src/source_reflows_merge.rs').read_text();front=front[front.index('pub(crate) fn arena854_input'):front.index('pub(crate) fn arena854_previous')]
assert front.index('let next = project(')<front.index('main65_previous(source, current)')
assert 'main65 == current || main65 == next' in front

# Every earlier capsule and every old owner assertion remains immutable.
for prefix in ['canonical-intern-arena-854-next-20261016','canonical-intern-arena-854-joined-20261016','canonical-intern-arena-854-receiving-20261016','arena-acceptance-controls-854-next-20261016']:
 paths=subprocess.check_output(['git','ls-tree','-r','--name-only',BASE,'--','docs/evidence/'+prefix],cwd=R).decode().splitlines()
 for p in paths:assert(R/p).read_bytes()==old(p)
print(json.dumps(dict(status='PASS_SOURCE_ONLY_ARENA854_SUCCESSOR',complete_inputs=4,wrong_input_sha_preimages=12,complete_old_receiver_unchanged=True,all_original54_owner_assertions_retained=True,query_comparison_and_AVL_bodies_unchanged=True,warm_sticky_before_any_payer_call=True,three_complete_raw_joins=True,strict_paid_gate_unchanged=True,paid_reduction='PENDING_RUNTIME',rust_executed=False),indent=2))
