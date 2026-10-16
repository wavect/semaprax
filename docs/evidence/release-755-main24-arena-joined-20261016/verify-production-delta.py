#!/usr/bin/env python3
"""Complete protected Git inventory; closed cfg(test) additions only after55."""
from pathlib import Path
import argparse,hashlib,json,subprocess
p=argparse.ArgumentParser();p.add_argument('--allow-dirty',action='store_true');a=p.parse_args();R=Path(__file__).resolve().parents[3];D=Path(__file__).resolve().parent;g=lambda *x:subprocess.check_output(['git',*x],cwd=R);sha=lambda b:hashlib.sha256(b).hexdigest();BASE='55f9d346fb76bd84b89e50291b1781c04825e6c7';dirty=g('status','--porcelain');assert a.allow_dirty or not dirty
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
 else:assert path in closed,path;changed.append(path)
for path in current:
 if path.endswith(('Cargo.toml','Cargo.lock','build.rs'))or path.startswith('.cargo/'):assert path in tree and(R/path).read_bytes()==g('show',BASE+':'+path)
# Relative to prior2c, only reviewed755 shipping source or the exact declared
# Main24 registration /854 cfg(test) hooks may differ.
prior='2c4306e1b19c9899a4b11abe7ca1e604f91fde15';component='09b2f0643461506be1fdbb5add8a6e5fce770284';production=[]
for path in g('diff','--name-only',prior,BASE,'--','src','crates','examples','.cargo','Cargo.toml','Cargo.lock','build.rs').decode().splitlines():
 if path in closed:continue
 b=(R/path).read_bytes()
 if path=='src/cleanup_plan/replay.rs':
  row=next(x for x in json.loads((R/'docs/evidence/main24-755-joint-next-20261016/source-inverses.json').read_text())if x['path']==path);s=b.decode();assert sha(b)==row['after_sha256']
  for e in row['inverse']:assert s.count(e['after'])==1;s=s.replace(e['after'],e['before'],1)
  assert s.encode()==g('show',component+':'+path)
 elif path=='src/cleanup_plan/replay/factored/canonical_shared.rs':
  row=next(x for x in json.loads((R/'docs/evidence/arena-acceptance-controls-854-next-20261016/source-inverses.json').read_text())['rows']if x['source']==path);s=b.decode();assert sha(b)==row['after_sha256'];assert s[row['offset']:row['offset']+len(row['after'])]==row['after'];s=s[:row['offset']]+row['before']+s[row['offset']+len(row['after']):];assert s.encode()==g('show',component+':'+path)
 else:assert b==g('show',component+':'+path),(path,'only755 shipping changes permitted');production.append(path)
print(json.dumps({'status':'COMPLETE_PRODUCTION_SOURCE_EQUIVALENCE_PASS','current_source_head':g('rev-parse','HEAD').decode().strip(),'production_source_head':BASE,'complete_protected_files':len(current),'byte_exact_files':exact,'closed_test_only_modified':changed,'closed_test_only_new':sorted(current-set(tree)),'all_Cargo_build_inputs_exact':True,'prior2c_shipping_changes_exact755':production,'Main24_and854_production_algorithms_preserved':True,'rust_executed':False},sort_keys=True))
