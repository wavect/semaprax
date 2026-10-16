#!/usr/bin/env python3
from pathlib import Path
import subprocess,json,hashlib
R=Path(__file__).resolve().parents[3];P=Path(__file__).resolve().parent;d=json.loads((P/'source-inverses.json').read_text());row=d['sources'][0]
old=subprocess.check_output(['git','show',d['base']+':'+row['source']],cwd=R).decode();now=(R/row['source']).read_text()
assert old==row['before']==(P/'path_join.before.rs.txt').read_text();assert now==row['current']
for name,text in [('before',old),('current',now)]:assert hashlib.sha256(text.encode()).hexdigest()==row[name+'_sha256']
start=old.index('        let mut groups = BTreeMap::<ControlKey, BTreeSet<Box<PathState>>>::new();');end=old.index('\n    if terminal_paths == 0')
assert now.count(old[start:end])==1
body=(P/'group-body.before.rs.txt').read_text();tail=(P/'schedule-tail.before.rs.txt').read_text()
assert now.count(body.strip())==2
# The added branch and two test registrations are the ONLY root difference.
a=now.index('        // A single incoming state is already its complete unique control group.');b=now.index('        let mut groups =',a)
restored=now[:a]+now[b:];suffix='\n#[cfg(test)]\nmod before_singleton;\n#[cfg(test)]\nmod singleton_tests;\n';assert restored.endswith(suffix);assert restored[:-len(suffix)]==old
compiled=(R/'src/cleanup_plan/replay/path_join/before_singleton.rs').read_text();expected=old
for registration in [
 '#[cfg(test)]\n#[allow(dead_code, reason = "complete original path-state replay receiver")]\nmod before;\n',
 '#[cfg(test)]\n#[allow(dead_code, reason = "complete MAIN019d callback receiver")]\n#[path = "path_join/before_receiving.rs"]\nmod before_receiving;\n',
 'mod boxed_states;\n','mod branch_join;\n','#[cfg(test)]\nmod receiver_tests;\n','pub(super) mod storage;\n','#[cfg(test)]\nmod tests;\n']:
 assert expected.count(registration)==1;expected=expected.replace(registration,'',1)
assert compiled==expected
for owner in json.loads((P/'new-owners.json').read_text()):assert hashlib.sha256((R/owner['source']).read_bytes()).hexdigest()==owner['sha256']
for path in [row['source'],'src/cleanup_plan/replay/path_join/before_singleton.rs','src/cleanup_plan/replay/path_join/singleton_tests.rs']:assert len((R/path).read_text().splitlines())<=1500
for path in ['src/cleanup_plan/replay/path_join/boxed_states.rs','src/cleanup_plan/replay/path_join/branch_join.rs','src/cleanup_plan/replay/path_join/storage.rs']:
 assert (R/path).read_bytes()==subprocess.check_output(['git','show',d['base']+':'+path],cwd=R)
proof={'status':'SOURCE_ONLY_PASS','base':d['base'],'complete_group_body_copies':2,'multistate_suffix_verbatim':True,'new_forwarding_protocol':False,'fees_caps_changed':False,'full_immediate_old_namespace':True,'new_pending_lib_bodies':3,'rust_executed':False}
(P/'source-proof.json').write_text(json.dumps(proof,indent=2)+'\n');print(json.dumps(proof,indent=2))
