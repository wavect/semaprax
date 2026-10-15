#!/usr/bin/env python3
"""Bounded source-only runtime fixture pilot; never executes Rust or IO controls."""
from pathlib import Path
import contextlib,hashlib,io,json,re,subprocess,tomllib
R=Path(__file__).resolve().parents[3];D=Path(__file__).resolve().parent;B='c37b7eea8b680ca4cb63c85758c6d7713745e8d3';h=lambda b:hashlib.sha256(b).hexdigest()
rows=json.loads((D/'current-to-c37.json').read_text());assert len(rows)==2;named={}
for row in rows:
 p=R/row['source'];s=p.read_text();assert h(s.encode())==row['current_sha256'];assert list(hashlib.sha256(s.encode()).digest())==row['current_digest']
 for e in row['edits']:assert s.count(e['new'])==1;s=s.replace(e['new'],e['old'],1)
 assert h(s.encode())==row['original_sha256'];assert s.encode()==(R/row['archive']).read_bytes()==subprocess.check_output(['git','show',B+':'+row['source']],cwd=R);named[p.resolve()]=s.encode()
helper=R/rows[0]['source'];s=helper.read_text();old=named[helper.resolve()].decode()
def fn(text,name):
 a=text.index('fn '+name);i=text.index('{',a);depth=1
 for m in re.finditer(r'//[^\n]*|/\*.*?\*/|"(?:\\.|[^"\\])*"|[{}]',text[i+1:],re.S):
  if m.group()=='{':depth+=1
  elif m.group()=='}':
   depth-=1
   if not depth:return text[a:i+1+m.end()]
 raise AssertionError(name)
for name in ['digest','decode','plain','check_decoder_contract','prior_source']:assert fn(s,name)==fn(old,name),name
assert 'GZIP_SHA' not in s and 'include_bytes!' not in s
raw='docs/evidence/selected-child-refusal-781/exact-source-inverses.json';b=(R/raw).read_bytes();assert len(b)==5196870 and h(b)=='2ff3e371c8bfde876ede4f5e7445a26067858e4a0d84610cca94d5b3ccc1240e'
assert b==subprocess.check_output(['git','show',B+':'+raw],cwd=R)
assert 'const OUTPUT_CAP: usize = 5_196_870;' in s
body=fn(s,'read_fixture');assert body.count('File::open(')==1;assert body.index('expected_len > OUTPUT_CAP')<body.index('File::open(')<body.index('file.metadata()')<body.index('!metadata.is_file()')<body.index('read_fixture_body(')
body=fn(s,'read_fixture_body');assert body.index('expected_len > OUTPUT_CAP')<body.index('try_reserve_exact(expected_len)')<body.index('read_exact(')<body.index('input.read(')<body.index('digest(&bytes)')<body.index('String::from_utf8')
assert 'read_to_end' not in body and 'static ' not in fn(s,'load')
assert 'Err(Failure::Input | Failure::Bound)' in s
for p in ['Cargo.toml','Cargo.lock','src/cleanup_plan/replay/selected_context/child/tests/source.rs']:
 assert (R/p).read_bytes()==subprocess.check_output(['git','show',B+':'+p],cwd=R),p
lock=tomllib.loads((R/'Cargo.lock').read_text());assert all(any(x['name']==n for x in lock['package']) for n in ['adler2','simd-adler32','miniz_oxide','crc32fast','flate2'])
for p in ['tests/hir_module_boundaries.rs','tests/workspace/semantic_graph/source_inventory.rs','crates/semaprax-native-rust-interop-builder/src/implementation/tests/ledger_capacity/source_inventory.rs']:
 assert (R/p).read_text().count(str(helper.relative_to(R))[4:]+'"')==1
 assert (R/p).read_bytes()==subprocess.check_output(['git','show',B+':'+p],cwd=R)
# Re-run the unchanged raw-default proof only in its complete named c37 domain.
prior=R/'docs/evidence/raw-selected-child-default-841-20261015/verify.py';code=prior.read_text();rb,rt,wt=Path.read_bytes,Path.read_text,Path.write_text;captured=[]
def write(p,text,*a,**k):assert p.name=='source-proof.json';captured.append(json.loads(text));return len(text)
try:
 Path.read_bytes=lambda p,*a,**k:named[p.resolve()] if p.resolve() in named else rb(p,*a,**k)
 Path.read_text=lambda p,*a,**k:named[p.resolve()].decode() if p.resolve() in named else rt(p,*a,**k)
 Path.write_text=write
 with contextlib.redirect_stdout(io.StringIO()):exec(compile(code,str(prior),'exec'),{'__file__':str(prior),'__name__':'__main__'})
finally:Path.read_bytes,Path.read_text,Path.write_text=rb,rt,wt
assert captured[-1]['all_decoder_hostile_controls_preserved']
print(json.dumps({'status':'SOURCE_ONLY_NOT_EXECUTED','whole_source_inverses':2,'original_decoder_functions_byte_exact':5,'original41_assertion_rows_and_parent_body_unchanged':True,'five_test_dependencies_unchanged':True,'new_IO_test_bodies':1,'pending_exact_owners':2,'runtime_repository_fixture_required':True,'old_raw_default_source_proof':captured[-1],'measurement_or_RAM_claim':False},indent=2))
