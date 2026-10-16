import argparse,json,hashlib,subprocess
from pathlib import Path
p=argparse.ArgumentParser();p.add_argument('--repo',type=Path,default=Path.cwd());a=p.parse_args();r=a.repo.resolve();d=r/'docs/evidence/branch-clone-funded-next-20261016';sha=lambda x:hashlib.sha256(x).hexdigest(); packet=json.loads((d/'source-inverses.json').read_text())
for row in packet['sources']:
 old=subprocess.check_output(['git','show',packet['base']+':'+row['path']],cwd=r)
 assert old==row['before'].encode()==(d/row['before_archive']).read_bytes()
 current=(r/row['path']).read_bytes();assert current==row['current'].encode()
 assert sha(old)==row['before_sha256'] and sha(current)==row['current_sha256']
for row in json.loads((d/'new-owners.json').read_text()): assert sha((r/row['path']).read_bytes())==row['sha256']
root=(r/'src/cleanup_plan/replay.rs').read_text();oldroot=packet['sources'][0]['before'];before=(d/'caller.before.rs.txt').read_text();after=(d/'caller.after.rs.txt').read_text();assert root.count(after)==1 and root.replace(after,before,1)==oldroot
mainroot=subprocess.check_output(['git','show',packet['main']+':src/cleanup_plan/replay.rs'],cwd=r);assert (d/'main-replay.before.rs.txt').read_bytes()==mainroot
original_tests=(d/'tests.before.rs.txt').read_text();assert (r/'src/cleanup_plan/replay/branch_clone/tests.rs').read_text().replace('let root = main_replay_input(include_str!("../../replay.rs"));','let root = include_str!("../../replay.rs");')==original_tests
scope=(r/'src/cleanup_plan/replay/branch_clone/before.rs').read_text();mainhelper=(d/'branch_clone.before.rs.txt').read_text().replace('use super::*;','use super::super::*;\nuse super::before as branch_clone;').replace('pub(super) fn for_push','pub(in crate::cleanup_plan::replay::branch_clone) fn for_push').replace('tests::original_selected()', 'super::tests::original_selected()').replace('\n#[cfg(test)]\nmod tests;\n','\n');assert scope.startswith(mainhelper+'\n')
mainplan=(r/'docs/evidence/branch-clone-capacity-v1/plan_skeleton_paths.before.rs.txt').read_text().replace('let mut branch = skeleton_clone(', 'let mut branch = branch_clone::for_push(');assert scope.count(mainplan.replace('fn plan_skeleton_paths(', 'pub(super) fn plan_skeleton_paths(',1))==1
assert sha(root.encode()) in scope and sha(mainroot) in scope
code=(r/'src/cleanup_plan/replay/branch_clone.rs').read_text();assert code.index('budget.charge_skeleton(function, 1, "plan branch observation push")')<code.index('let capacity =')<code.index('budget.reserve_array::<T>')<code.index('let mut result = Vec::with_capacity')
assert root.index('branch_clone::controls::<Rc<SkeletonObservation>, SkeletonObservation>()')<root.index('let branch = branch_clone::with_observation(')
for path in ['src/cleanup_plan/replay/physical_clone.rs','src/cleanup_plan/replay/physical_budget.rs','src/cleanup_plan/replay/physical_storage.rs']:
 assert (r/path).read_bytes()==subprocess.check_output(['git','show',packet['base']+':'+path],cwd=r)
for path in ['src/cleanup_plan/replay/branch_clone.rs','src/cleanup_plan/replay/branch_clone/before.rs','src/cleanup_plan/replay/branch_clone/tests.rs','src/cleanup_plan/replay/branch_clone/funded_tests.rs']:assert len((r/path).read_text().splitlines())<=1500
assert len(root.splitlines())<=len(oldroot.splitlines())
print(json.dumps(dict(status='PASS',kind='SOURCE_ONLY',full_inverses=3,new_owners=2,main_original_test_bodies=3,new_pending_bodies=2,production_execution=False)))
