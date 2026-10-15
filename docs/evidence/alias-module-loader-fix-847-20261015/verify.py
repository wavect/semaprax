"""Strict source-loader wiring successor; no compiler invocation."""
from pathlib import Path
import contextlib,hashlib,io,json,subprocess
R=Path(__file__).resolve().parents[3];D=Path(__file__).resolve().parent;BASE='340d26f5b00165e81c2508adf3dc4a4b1cb4312b';h=lambda b:hashlib.sha256(b).hexdigest()
rows=json.loads((D/'current-to-340.json').read_text());assert len(rows)==3;named={}
for row in rows:
 p=R/row['source'];raw=p.read_bytes();old=row['edits'][0]['old'].encode();assert raw==row['edits'][0]['new'].encode() and h(raw)==row['current_sha256'];assert h(old)==row['original_sha256'] and old==(R/row['archive']).read_bytes()==subprocess.check_output(['git','show',BASE+':'+row['source']],cwd=R);named[p.resolve()]=old
for path,before,after in [('src/workspace_graph/expected_projection/borrowed_imports/typed.rs','\nmod alias;','\n#[path = "typed/alias.rs"]\nmod alias;'),('src/workspace_graph/expected_projection/borrowed_imports/typed/alias.rs','#[cfg(test)]\nmod tests;','#[cfg(test)]\n#[path = "alias/tests.rs"]\nmod tests;')]:
 p=R/path;old=named[p.resolve()].decode();assert old.count(before)==1 and p.read_text()==old.replace(before,after),path
assert (R/'src/workspace_graph/expected_projection/borrowed_imports/typed/alias.rs').is_file()
assert (R/'src/workspace_graph/expected_projection/borrowed_imports/typed/alias/tests.rs').is_file()
script=R/'docs/evidence/final-main27424-alias847-assembly-20261015/verify.py';code=script.read_text();rb,rt,wt,co=Path.read_bytes,Path.read_text,Path.write_text,subprocess.check_output;captured=[]
def read_bytes(p,*a,**k):return named[p.resolve()] if p.resolve() in named else rb(p,*a,**k)
def read_text(p,*a,**k):return named[p.resolve()].decode() if p.resolve() in named else rt(p,*a,**k)
def write(p,s,*a,**k):assert p.name=='source-proof.json';captured.append(json.loads(s));return len(s)
def output(args,*a,**k):
 if isinstance(args,list) and args[:2]==['git','diff']:
  i=args.index('--') if '--' in args else len(args);revs=[v for v in args[2:i] if not v.startswith('-')]
  if len(revs)==1:args=args[:i]+[BASE]+args[i:]
 return co(args,*a,**k)
try:
 Path.read_bytes,Path.read_text,Path.write_text,subprocess.check_output=read_bytes,read_text,write,output
 with contextlib.redirect_stdout(io.StringIO()):exec(compile(code,str(script),'exec'),{'__file__':str(script),'__name__':'__main__'})
finally:Path.read_bytes,Path.read_text,Path.write_text,subprocess.check_output=rb,rt,wt,co
assert captured
proof={'status':'SOURCE_ONLY_MODULE_LOADER_SUCCESSOR','full_sources':3,'new_test_bodies':0,'six_existing_alias_test_bodies_now_linked_by_explicit_paths':True,'actual_Rust_or_test_execution':False,'unchanged_whole_previous_source_proof':captured[-1]}
(D/'source-proof.json').write_text(json.dumps(proof,indent=2)+'\n');print('PASS strict3source predecessor and unchanged final assembly proof; NO Rust')
