#!/usr/bin/env python3
"""Complete protected Git inventory; closed cfg(test) Main65 additions only after75d8."""
from pathlib import Path
import argparse,hashlib,json,subprocess
p=argparse.ArgumentParser();p.add_argument('--allow-dirty',action='store_true');a=p.parse_args();R=Path(__file__).resolve().parents[3];D=Path(__file__).resolve().parent;g=lambda *x:subprocess.check_output(['git',*x],cwd=R);sha=lambda b:hashlib.sha256(b).hexdigest();BASE='9ce9ba33681c1dbcf4aad49b55a9808136fd2592';dirty=g('status','--porcelain');assert a.allow_dirty or not dirty
classes={}
for name in ['hir','projection']:classes.update(json.loads((D/(name+'.json')).read_text())['classes'])
closed={p for p,row in classes.items()if row['role']=='test-only'}
assert '#[cfg(test)]\n#[macro_use]\nmod source_reflows;'in(R/'src/lib.rs').read_text();closed.add('src/source_reflows.rs')
interop='crates/semaprax-native-rust-interop-builder/src/implementation/tests/ledger_capacity/source_inventory.rs';assert '#[cfg(test)]\n#[path = "implementation/tests.rs"]\nmod tests;'in(R/'crates/semaprax-native-rust-interop-builder/src/implementation.rs').read_text();assert '#[path = "tests/ledger_capacity.rs"]\nmod ledger_capacity;'in(R/'crates/semaprax-native-rust-interop-builder/src/implementation/tests.rs').read_text();assert 'include!("ledger_capacity/source_inventory.rs");'in(R/'crates/semaprax-native-rust-interop-builder/src/implementation/tests/ledger_capacity.rs').read_text();closed.add(interop)
protected=lambda p:p.startswith(('src/','crates/','examples/','.cargo/'))or p in ['Cargo.toml','Cargo.lock','build.rs']
tree={}
for line in g('ls-tree','-r',BASE).decode().splitlines():
 info,path=line.split('\t',1)
 if protected(path):tree[path]=info.split()[2]
current={p for p in g('ls-files','--cached','--others','--exclude-standard').decode().splitlines()if protected(p)};assert set(tree)<=current;assert current-set(tree)<=closed
fmt=g('rev-parse','--show-object-format').decode().strip();exact=0;changed=[]
for path,blob in tree.items():
 b=(R/path).read_bytes()
 if hashlib.new(fmt,b'blob '+str(len(b)).encode()+b'\0'+b).hexdigest()==blob:exact+=1
 else:
  assert path in closed,path;changed.append(path)
for path in current:
 if path.endswith(('Cargo.toml','Cargo.lock','build.rs'))or path.startswith('.cargo/'):assert path in tree and(R/path).read_bytes()==g('show',BASE+':'+path)
print(json.dumps({'status':'COMPLETE_PRODUCTION_SOURCE_EQUIVALENCE_PASS','current_source_head':g('rev-parse','HEAD').decode().strip(),'production_source_head':BASE,'complete_protected_files':len(current),'byte_exact_files':exact,'closed_test_only_modified':changed,'closed_test_only_new':sorted(current-set(tree)),'all_Cargo_build_inputs_exact':True,'only_production_source_difference':'none; complete production and Cargo bytes unchanged','rust_executed':False},sort_keys=True))
