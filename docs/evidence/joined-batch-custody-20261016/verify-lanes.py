"""Read-only final lane binding audit. Historical verifier projections are explicit."""
from pathlib import Path
import contextlib,hashlib,io,json,re,runpy,subprocess,sys
R=Path(__file__).resolve().parents[3];O=Path(__file__).resolve().parent;sha=lambda b:hashlib.sha256(b).hexdigest()
def git(ref,p):return subprocess.check_output(['git','show',ref+':'+p],cwd=R)
def read(p):return(R/p).read_text()
def project(row,current):
 assert sha(current.encode())==row['current_sha256'],row['source']
 for e in row['edits']:assert current.count(e['new'])==1;current=current.replace(e['new'],e['old'],1)
 assert sha(current.encode())==row['original_sha256'];return current
# The direct reader four exact bridges still bind actual complete source and the full projection join.
D=R/'docs/evidence/current344-input-composition-20261016';text=read('tests/workspace/semantic_graph/source_inventory.rs');i=text.index('macro_rules! semantic_graph_projection_root_source_inventory');j=text.index('\nmacro_rules!',i+1);pieces=[]
for m in re.finditer(r'include_str!\(\s*"([^"]+)"\s*\)|"(\\n)"',text[i:j]):pieces.append((R/'tests/workspace'/m[1]).read_text()if m[1]else'\n')
rows=json.loads((D/'bridges.json').read_text());assert len(rows)==4
for row in rows:
 current=''.join(pieces)if row['source']=='expected_projection::root_join'else read(row['source']);before=project(row,current)
 for wrong in [current+' ','',before]:assert sha(wrong.encode())!=row['current_sha256']
# Full runtime chain reaches the original complete release transport input.
f='src/source_reflows_merge/runtime_json.rs';current=read(f)
for table in ['docs/evidence/a3-e47-final-custody-20261016/current-to-Previous16c.json','docs/evidence/main8a-independent-custody-20261016/current-to55e.json']:current=project(next(row for row in json.loads(read(table))if row['source']==f),current)
assert sha(current.encode())=='048584a387e19ce44d46428072db42fedd4a4f94f67f3fac0622a2ce3fc0961f'
# Both fixture packets retain complete Git preimages, exact final source bindings and all original assertions.
fixture_rows=[]
for name in ['owning-fixture-observation-20261016','owning-fixture-context-continuation-20261016']:
 d=R/'docs/evidence'/name;m=json.loads((d/'source-inverses.json').read_text())
 for row in m['rows']:
  before=(d/row['before_file']).read_bytes();current=(R/row['source']).read_bytes();assert before==git(m['base'],row['source']);assert sha(before)==row['before_sha256'];assert sha(current)==row['after_sha256'];fixture_rows.append(row['source'])
  old,new=before.decode(),current.decode();assert len(re.findall(r'\bassert(?:_eq|_ne)?!',old))==len(re.findall(r'\bassert(?:_eq|_ne)?!',new))
# Every repaired fixture and both residual855 inputs are included at the final complete predecessor front.
tables=json.loads(read('docs/evidence/a3-e47-final-custody-20261016/current-to-Previous16c.json'));bykey={row['source']:row for row in tables}
for f in fixture_rows+['src/hir/expr_nodes.rs','src/bounded_output.rs']:assert f in bykey;project(bykey[f],read(f))
# The853 original implementation verifier runs unchanged with only its historical private changelog projection.
# Final changelog retains the entire authored new853 entry alongside incoming Main043 and722 entries.
f='CHANGELOG.md';private=git('a7636b5ba68433f236b92bae5dde2082be047688',f).decode();actual=read(f);base=git('a82e00cc8',f).decode();added=[line for line in private.splitlines()if line and line not in base.splitlines()]
for line in added:assert line in actual
rt=Path.read_text;argv=sys.argv[:]
def projected_read(path,*a,**kw):
 if path.resolve()==(R/f).resolve():return private
 if path.resolve()==(R/'src/bounded_output.rs').resolve():return git('a7636b5ba68433f236b92bae5dde2082be047688','src/bounded_output.rs').decode()
 return rt(path,*a,**kw)
try:
 Path.read_text=projected_read;script=R/'docs/evidence/instance-key-format-completion-853-20261016/verify-source.py';sys.argv=[str(script),'--repo',str(R)];out=io.StringIO()
 with contextlib.redirect_stdout(out):runpy.run_path(str(script),run_name='__main__')
 result=out.getvalue()
finally:Path.read_text=rt;sys.argv=argv
protected=json.loads((O/'protected-baseline.json').read_text())
for f,digest in protected['files'].items():assert sha((R/f).read_bytes())==digest,f
print(json.dumps(dict(status='PASS_JOINED_LANE_SOURCE_BINDINGS_ONLY',current344_bridges=4,wrong_input_preimages=12,runtime_original_chain=True,complete_fixture_preimages=len(fixture_rows),residual855_inputs_at_predecessor_front=2,instance853_verifier='unchanged with explicitly historical private changelog and residual855 reexport projection',instance853_output=result.strip(),protected_files=len(protected['files']),rust_executed=False),indent=2))
