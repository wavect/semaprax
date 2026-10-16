#!/usr/bin/env python3
"""Complete Git production file-set comparison, with closed test chains only."""
from pathlib import Path
import argparse,hashlib,json,subprocess
A=argparse.ArgumentParser();A.add_argument('--allow-dirty',action='store_true');a=A.parse_args();R=Path(__file__).resolve().parents[3];D=Path(__file__).resolve().parent;BASE='b5eca55b96890e1a815f0d8d9e7b42c78d743b07';g=lambda *x:subprocess.check_output(['git',*x],cwd=R);sha=lambda b:hashlib.sha256(b).hexdigest();dirty=g('status','--porcelain').decode();assert a.allow_dirty or not dirty
tree={}
for line in g('ls-tree','-r',BASE).decode().splitlines():
 info,path=line.split('\t',1);tree[path]=info.split()[2]
object_format=g('rev-parse','--show-object-format').decode().strip()
old=set(tree);current=set(g('ls-files','--cached','--others','--exclude-standard').decode().splitlines());protected=lambda p:p.startswith(('src/','crates/','examples/','.cargo/'))or p in ['Cargo.toml','Cargo.lock','build.rs'];old={p for p in old if protected(p)};current={p for p in current if protected(p)};classes={}
for name in ['hir','projection']:
 m=json.loads((D/(name+'.json')).read_text());classes.update(m['classes'])
replay='src/cleanup_plan/replay.rs';interop='crates/semaprax-native-rust-interop-builder/src/implementation/tests/ledger_capacity/source_inventory.rs';closed={p for p,row in classes.items()if row['role']=='test-only'}
assert '#[cfg(test)]\n#[macro_use]\nmod source_reflows;'in(R/'src/lib.rs').read_text()
closed.add('src/source_reflows.rs')
assert '#[cfg(test)]\n#[path = "implementation/tests.rs"]\nmod tests;'in(R/'crates/semaprax-native-rust-interop-builder/src/implementation.rs').read_text()
assert '#[path = "tests/ledger_capacity.rs"]\nmod ledger_capacity;'in(R/'crates/semaprax-native-rust-interop-builder/src/implementation/tests.rs').read_text()
assert 'include!("ledger_capacity/source_inventory.rs");'in(R/'crates/semaprax-native-rust-interop-builder/src/implementation/tests/ledger_capacity.rs').read_text();closed.add(interop)
# All classifications are authenticated by the unchanged authority verifier.
import contextlib,io,runpy
with contextlib.redirect_stdout(io.StringIO()):runpy.run_path(str(D/'verify-authority.py'),run_name='__main__')
assert not(old-current),'removed production candidate';assert current-old<=closed,sorted((current-old)-closed)
changed=[];shipping=[]
for p in sorted(old&current):
 after=(R/p).read_bytes();blob=hashlib.new(object_format,b'blob '+str(len(after)).encode()+b'\0'+after).hexdigest()
 if tree[p]==blob:shipping.append((p,sha(after)));continue
 before=g('show',BASE+':'+p)
 if p==replay:assert after.replace(b'#[cfg(test)]\nmod sequence_prefix;\n',b'',1)==before
 else:assert p in closed,p
 changed.append(p)
for p in old|current:
 if p.endswith(('Cargo.toml','Cargo.lock','build.rs'))or p.startswith('.cargo/'):assert p in old&current and(R/p).read_bytes()==g('show',BASE+':'+p),p
print(json.dumps({'status':'COMPLETE_PRODUCTION_SOURCE_EQUIVALENCE_PASS'if not dirty else'SOURCE_ONLY_DIRTY_PRODUCTION_EQUIVALENCE_PASS','current_source_head':g('rev-parse','HEAD').decode().strip(),'production_source_head':BASE,'complete_protected_file_set':len(current),'byte_exact_files':len(shipping),'closed_test_only_modified_files':changed,'closed_test_only_new_files':sorted(current-old),'shipping_replay_registration_only':True,'all_Cargo_build_inputs_exact':True,'rust_executed':False},sort_keys=True))
