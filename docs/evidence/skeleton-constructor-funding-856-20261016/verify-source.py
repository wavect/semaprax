#!/usr/bin/env python3
"""Finite complete-source custody; never compiles or executes Rust."""
import argparse,hashlib,json,re,subprocess
from pathlib import Path
p=argparse.ArgumentParser();p.add_argument('--repo',required=True);a=p.parse_args();r=Path(a.repo).resolve();d=r/'docs/evidence/skeleton-constructor-funding-856-20261016';m=json.loads((d/'construction.json').read_text());rows=json.loads((d/'source-inverses.json').read_text())
def sha(s):return hashlib.sha256(s.encode()).hexdigest()
def git(p):return subprocess.check_output(['git','show',m['base']+':'+p],cwd=r).decode()
assert sorted(x['source'] for x in rows)==m['preexisting_paths']
previous={}
for row in rows:
 old=(d/'before'/row['source']).read_text();new=(r/row['source']).read_text();assert old==git(row['source']);assert sha(old)==row['before_sha256'];assert sha(new)==row['after_sha256'];value=new
 for e in row['inverse']:
  assert value.count(e['after'])==1;value=value.replace(e['after'],e['before'],1)
 assert value==old;previous[row['source']]=old
ns={};exec(git('docs/evidence/production-source-authority-partition-20261016/original-module-parser-prefix.py').split('# Source only:')[0],ns);mask=ns['mask']
def bodies(s):
 mm=mask(s);out=[]
 for match in re.finditer(r'\bfn\s+(\w+)\s*(?:<[^{};]*>)?\s*\(',mm):
  start=match.start();at=mm.find('{',match.end());semi=mm.find(';',match.end())
  if at<0 or (0<=semi<at):continue
  depth=1;end=at+1
  while depth:depth+=(mm[end]=='{')-(mm[end]=='}');end+=1
  out.append((match[1],s[at:end]))
 return out
def one(s,name):return next(body for key,body in bodies(s) if key==name)
# All original helper bodies, tests and ceilings remain executable and exact.
# Only the ordinary constructor is replaced; its full original body is kept.
diag_paths=[p for p in m['preexisting_paths'] if '/sequence_prefix/' in p]
for path in diag_paths:
 new=(r/path).read_text();new=re.sub(r'assert!\(alloc\.allocated <= paid, "limit=\{limit\} exact=\{exact\} paid=\{paid\} alloc=\{alloc:\?\} refused=\{refused\} first=\{first:\?\}"\);','assert!(alloc.allocated <= paid);',new)
 assert new==git(path),('original owner changed',path)
assert len(diag_paths)==5
for path,old in previous.items():
 if not path.endswith('.rs') or path.endswith('skeleton_work.rs') or path in diag_paths:continue
 assert bodies((r/path).read_text())==bodies(old),('old function body changed',path)
path='src/cleanup_plan/replay/skeleton_work.rs';current=(r/path).read_text();old=previous[path]
assert one(current,'with_index_before_funding')==one(old,'with_index')
new=one(current,'with_index')
assert new.index('budget.prepare_builder(function)?')<new.index('budget.prepare_clone_cursor(function)?')<new.index('let controls')<new.index('let mut reserve')<new.index('bindings_with_index_funded')
order=['bindings_with_index_funded','checked_moving_string_operands_funded','function_reads_funded','requires_funded','owner_funded']
assert [new.index(x) for x in order]==sorted(new.index(x) for x in order)
assert 'Err(FundedError::Refused(error)) => return Err(error)' in new
assert 'index.is_some()' in new and 'RenewalBindings::empty()' in new
assert 'oracle_with_index(program, function, budget, index, lease)' in new
for name in {name for name,_ in bodies(old)}-{'with_index'}:
 assert [body for key,body in bodies(current) if key==name]==[body for key,body in bodies(old) if key==name],('other constructor/algorithm changed',name)
assert len(current.splitlines())<1500
promoted=json.loads((d/'promoted-modules.json').read_text());assert len(promoted)==13
# Every promoted path is raw-bound in all three original full authority unions.
guards=['tests/hir_module_boundaries.rs','tests/workspace/semantic_graph/source_inventory.rs','crates/semaprax-native-rust-interop-builder/src/implementation/tests/ledger_capacity/source_inventory.rs']
for guard in guards:
 text=(r/guard).read_text();literals=re.findall(r'include_str!\(\s*"([^"]+)"',text)
 for path in promoted:assert any(x.endswith('/'+path) for x in literals),(guard,path)
for join in json.loads((d/'raw-join-bindings.json').read_text()):
 text=(r/join['guard']).read_text();assert text.count(join['added'])==1
 assert text.replace(join['added'],'',1)==git(join['guard'])
assert (r/guards[0]).read_text()==git(guards[0])
# No transfer algorithm, prefix helper, complete previous encoder, global
# allocator/ledger, replay work cap, or external pressure baseline is changed.
for path in ['src/cleanup_plan/replay/sequence_prefix.rs','src/cleanup_plan/replay/sequence_prefix/before.rs','src/cleanup_plan/replay/sequence_prefix/main_current.rs','src/cleanup_plan/replay/sequence_prefix/main24/helper.rs','src/cleanup_plan/replay/sequence_prefix/main_c11/helper.rs','src/cleanup_plan/replay/allocation.rs','src/cleanup_plan/replay/physical_budget.rs','src/cleanup_plan/replay.rs','src/bounded_output.rs','src/bounded_output/checked_string.rs','src/hir/nodes/instance_key.rs','src/cleanup_plan/replay/skeleton_work/oracle_entry.rs']:
 assert (r/path).read_text()==git(path),path
# Selected test-oracle work receivers and deterministic scripted payers remain
# behind their original cfg(test) boundaries; no blanket warning exemptions.
owner=(r/'src/hir/lexical_exit/owner_funded.rs').read_text();assert '#[cfg(test)]\nmod work;' in owner
assert '#[cfg(test)]\npub(crate) mod payer' in (r/'src/hir/funded.rs').read_text()
assert '#[cfg(test)]\npub(crate) mod work;' in (r/'src/hir/funded.rs').read_text()
for path in promoted:
 text=(r/path).read_text()
 assert 'FundedError::Unfunded(' not in text
 if path!='src/hir/funded.rs':assert '#[allow(dead_code' not in text
print(json.dumps(dict(status='SOURCE_ONLY_CONSTRUCTOR_FUNDING_CUSTODY_PASS',preexisting_inputs=len(rows),promoted_modules=13,unchanged_five_owner_assertions=True,old_constructor_body_exact=True,old_helper_bodies_exact=True,three_raw_inventories_complete=True,unchanged_transfer_algorithms_and_caps=True,rust_executed=False)))
