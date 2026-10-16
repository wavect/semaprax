#!/usr/bin/env python3
"""Fixed complete-source custody and lexical contract proof; no Rust execution."""
import argparse,hashlib,json,re,subprocess
from functools import lru_cache
from pathlib import Path
p=argparse.ArgumentParser();p.add_argument('--repo',required=True);a=p.parse_args();r=Path(a.repo).resolve();d=r/'docs/evidence/private-authority-roots-scratch-765-20261016';m=json.loads((d/'construction.json').read_text())
def sha(b):return hashlib.sha256(b).hexdigest()
def git(path):return subprocess.check_output(['git','show',m['base']+':'+path],cwd=r)
rows=json.loads((d/'source-inverses.json').read_text())
assert sorted(x['source'] for x in rows)==sorted(m['preexisting_paths']+m['unchanged_protected_paths'])
for row in rows:
 before=(d/'before'/row['source']).read_bytes();after=(r/row['source']).read_bytes()
 assert before==git(row['source']) and sha(before)==row['before_sha256']
 assert sha(after)==row['after_sha256'],row['source']
 lines=after.decode().splitlines(True)
 for e in row['inverse']:
  assert sha(''.join(lines[e['start']:e['end']]).encode())==e['after_sha256']
  lines[e['start']:e['end']]=e['before'].splitlines(True)
 assert ''.join(lines).encode()==before,('full inverse mismatch',row['source'])
for path in m['unchanged_protected_paths']:assert (r/path).read_bytes()==git(path),path
for path in m['new_paths']:assert sha((r/path).read_bytes())==m['new_sha256'][path],path
# The trusted parser is exact-base source-only Python, not a Rust execution.
ns={};exec(git('docs/evidence/production-source-authority-partition-20261016/original-module-parser-prefix.py').decode().split('# Source only:')[0],ns);mask=ns['mask']
@lru_cache(maxsize=None)
def bodies(s):
 mm=mask(s);out=[]
 for match in re.finditer(r'\bfn\s+(\w+)\s*(?:<[^{};]*>)?\s*\(',mm):
  at=mm.find('{',match.end());semi=mm.find(';',match.end())
  if at<0 or (0<=semi<at):continue
  depth=1;end=at+1
  while depth:depth+=(mm[end]=='{')-(mm[end]=='}');end+=1
  out.append((match[1],s[at:end]))
 return out
def text(path):return (r/path).read_text()
def old(path):return git(path).decode()
def one(s,name):return next(body for key,body in bodies(s) if key==name)
def flat(s):return re.sub(r'\s+','',s)
owner='src/workspace_graph/expected_projection/private_layout/authority_invocation.rs'
assert bodies(text(owner))==bodies(old(owner)), 'complete original invocation changed'
assert '#[cfg(test)]\n    pub(in crate::workspace_graph) fn derive_bound' in text(owner)
# All three old core algorithms stay byte-exact; only dependency imports move.
for path in ['src/workspace_graph/cache_key_original.rs','src/workspace_graph/function_alias/before_core.rs','src/workspace_graph/constructor_failure/receiving/before.rs']:
 assert bodies(text(path))==bodies(old(path)),path
 assert 'validation::before_roots::' in text(path) and 'before_private_roots::reconstruct_workspace_declaration_facts_with_authority' in text(path)
# The full saved compact/signature module changes only explicit relocation paths
# and visibility. Literal contents and complete executable algorithms remain exact.
v='src/workspace_graph/validation.rs';expected=old(v).replace('use super::expected_projection','use crate::workspace_graph::expected_projection').replace('use super::{\n','use crate::workspace_graph::{\n',1).replace('super::index_authored','crate::workspace_graph::index_authored').replace('super::expected_projection::','crate::workspace_graph::expected_projection::').replace('pub(super)','pub(in crate::workspace_graph)')
assert text('src/workspace_graph/validation/before_roots.rs')==expected
root='src/workspace_graph.rs';full=old(root);start=full.index('fn reconstruct_workspace_declaration_facts_with_authority(');end=full.index('\nfn ',start+3)
assert text('src/workspace_graph/before_private_roots.rs')=='//! Complete previous full declaration consumer; owning authority stays original.\nuse super::*;\npub(super) '+full[start:end]
# Every original unrelated function body remains exact.
for path,changed in [(v,{'record_declarations','validate_stub_signatures_authored'}),(root,{'build_resolved_core_current','reconstruct_workspace_declaration_facts','reconstruct_workspace_declaration_facts_with_authority'})]:
 for name,body in bodies(old(path)):
  if name not in changed:assert body in [b for n,b in bodies(text(path)) if n==name],(path,name)
# Four complete consumers retain the receiver through their final lexical scope;
# no declaration/signature loop or final equality is deleted or moved to a cache.
for name in ['record_declarations','validate_stub_signatures_authored']:
 olds=[b for n,b in bodies(old(v)) if n==name];news=[b for n,b in bodies(text(v)) if n==name]
 assert len(olds)==len(news)
 for before,after in zip(olds,news):
  before=re.sub(r'let private = match authority \{\n.*?\n\s*\};','let private = CURRENT;',before,flags=re.S)
  after=re.sub(r'let private = Current::derive\(authority, (source|caller), programs, authored\)\?;','let private = CURRENT;',after)
  assert flat(before)==flat(after),(name,'consumer body/order changed')
before=one(old(root),'reconstruct_workspace_declaration_facts_with_authority');after=one(text(root),'reconstruct_workspace_declaration_facts_with_authority')
before=re.sub(r'let private = match authority \{\n.*?\n\s*\};','let private = CURRENT;',before,flags=re.S)
after=re.sub(r'let private = expected_projection::private_layout::authority::invocation::scratch::Current::derive\(authority, source, programs, authored\)\?;','let private = CURRENT;',after)
assert flat(before)==flat(after)
# All old ledger algorithms/caps remain exact; this added identity type only
# clones an existing Rc and reads pointer/sticky state. It cannot reserve/refund.
assert text('src/bounded_output.rs')==old('src/bounded_output.rs')+'\npub(crate) mod invocation_binding;\n'
binding=text('src/bounded_output/invocation_binding.rs');assert 'Rc::ptr_eq(held, &current) && !held.overflowed.get()' in binding
for forbidden in ['.set(', 'reserve(', 'unsafe', 'with_limit', 'refund']:assert forbidden not in binding
s=text('src/workspace_graph/expected_projection/private_layout/authority_invocation/scratch.rs')
# Exact complete predecessor DFS/dedup and query bodies, adapting only the view.
authority=old('src/workspace_graph/expected_projection/private_layout/authority.rs')
for name in ['contains','validate_signatures']:
 before=one(authority,name).replace('in &self.roots','in self.roots')
 # first two occurrences are enum dispatch; the complete view method is last.
 after=[b for n,b in bodies(s) if n==name][-1]
 assert flat(before)==flat(after),('complete query changed',name)
ready=one(authority,'finish_ready');walk=ready[ready.index('        visit('):ready.index('        Ok(Self {')]
walk=walk.replace('            &mut remaining,','            remaining,').replace('for (m, id) in &roots','for (m, id) in roots.iter()').replace('            programs,','            self.base.programs,')
fill=one(s,'fill');newwalk=fill[fill.index('        visit('):fill.rindex('        Ok(())')]
assert flat(walk)==flat(newwalk),'complete root walk/dedup changed'
# Complete per-caller preparation keeps the same WORK/import/possible/proof/
# explicit-only/frame order. Only the returned owning root receiver is replaced.
prepare=one(old(owner),'derive_current')
prepare=re.sub(r'self\s*\.programs', 'self.base.programs', prepare).replace('self.authored','self.base.authored')
prepare=re.sub(r'return Ok\(PrivateTypes \{\s*roots: Vec::new\(\),\s*remaining: Cell::new\(remaining\),\s*\}\);','return Ok(remaining);',prepare)
prepare=re.sub(r'PrivateTypes::finish_ready\(\s*caller,\s*self.base.programs,\s*held,\s*state.count,\s*remaining,\s*&mut state.frames,\s*\)', 'self.fill(caller, held, state.count, &mut remaining, &mut state.frames, roots,)?; Ok(remaining)', prepare)
assert flat(prepare)==flat(one(s,'prepare')), 'per-caller preparation/work/proof order changed'
lease=one(s,'lease_bound');assert lease.index('required(controls)?')<lease.index('try_borrow_mut()')<lease.index('let mut lease')<lease.index('self.prepare(')
assert 'if !roots.is_empty() || !state.frames.is_empty()' in lease
assert 'if !self.complete' in s and 'self.state.failed = true;' in s
assert 'self.roots.clear();' in s and 'self.state.frames.clear();' in s
assert 'remaining: Cell::new(0)' in lease and 'lease.remaining.set(remaining)' in lease
assert '.checked_add(size_of::<RefMut' in s and 'size_of::<Result<View' in s
assert 'Layout::array::<(&str, &str)>(count)' in fill
assert fill.index('required(layout.size())?')<fill.index('try_reserve_exact(count)')<fill.index('visit(')
for bad in ['unsafe', 'transmute', 'mem::forget', '#[allow(dead_code', 'with_limit', 'sort(', 'refund(']:assert bad not in s,bad
# Every new Rust module is raw-bound in each complete original union.
# Root/caller compilation source bindings remain exact registration-chain inputs.
modules=json.loads((d/'current-source-modules.json').read_text())
for join in json.loads((d/'raw-join-bindings.json').read_text()):
 source=text(join['path']);assert source.count(join['fragment'])==1
 assert source.replace(join['fragment'],'',1)==old(join['path']),join['path']
 literals=re.findall(r'include_str!\(\s*"([^"]+)"',source)
 for path in modules:assert any(x.endswith('/'+path) for x in literals),(join['path'],path)
 assert len(source.splitlines())==len(old(join['path']).splitlines()),'raw guard line count grew'
for path in m['new_paths']:
 if path.endswith('.rs'):assert len(text(path).splitlines())<=1500,path
selectors=json.loads((d/'pending-selectors.json').read_text());assert len(selectors['new'])==6 and len(selectors['unchanged_original'])==3
print(json.dumps(dict(status='SOURCE_ONLY_PRIVATE_AUTHORITY_ROOTS_SCRATCH_CUSTODY_PASS',preexisting_inputs=len(rows),new_modules=len(m['new_paths']),old_invocation_and_three_owning_tests_unchanged=True,all_four_old_consumers_executable=True,three_raw_inventories_complete=True,full_old_traversal_queries_and_work_preserved=True,no_caps_or_ledger_algorithms_changed=True,rust_executed=False)))
