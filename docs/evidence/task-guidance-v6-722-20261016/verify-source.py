#!/usr/bin/env python3
"""Finite source custody/fact checks only; does not run Rust or help."""
from pathlib import Path
import hashlib,json,re,subprocess,tomllib
ROOT=Path(__file__).resolve().parents[3];P=Path(__file__).resolve().parent
j=json.loads((P/'source-inverses.json').read_text())
def sha(b):return hashlib.sha256(b).hexdigest()
def git(path):return subprocess.check_output(['git','show',j['base']+':'+path],cwd=ROOT)
for row in j['rows']:
 old=(P/row['before_archive']).read_bytes();new=(ROOT/row['path']).read_bytes()
 assert old==git(row['path']) and sha(old)==row['before_sha256']
 assert sha(new)==row['after_sha256']
 restored=new.decode()
 for edit in reversed(row['edits']):
  assert restored.count(edit['new'])==1,row['path']
  restored=restored.replace(edit['new'],edit['old'],1)
 assert restored.encode()==old,row['path']
for row in json.loads((P/'new-owners.json').read_text()):
 assert sha((ROOT/row['path']).read_bytes())==row['sha256'],row['path']
for path in ['src/cli/help.rs','src/cli/help/authoring/discovery.rs','src/cli/help/library.rs','src/project/scaffold.rs','src/project/scaffold/authoring.rs','std/catalog.json','docs/CLI-HELP-V5.md','docs/AGENT-AUTHORING-GUIDE.md']:
 assert (ROOT/path).read_bytes()==git(path),path
old=git('src/cli/help/authoring.rs').decode();new=(ROOT/'src/cli/help/authoring.rs').read_text()
new=new.replace('mod tasks;\n','',1).replace('  author:tasks checked task pages\\n','',1)
block='''    if query == "author:tasks" || query.starts_with("author:task:") {
        return tasks::lookup(query, MAX_BYTES);
    }
'''
assert new.count(block)==1;assert new.replace(block,'',1)==old,'old V5 code/card/test bytes changed'
routes=json.loads(re.search(r'const ROUTES: &str = ("(?:[^"\\]|\\.)*");',(ROOT/'src/cli/help/authoring.rs').read_text())[1]);assert len(routes.encode())==2040<=2048
catalog=json.loads((ROOT/'src/cli/help/authoring/tasks/catalog.json').read_text());source=(ROOT/'src/project/scaffold.rs').read_text()
def constant(name):
 raw=re.search(r'const '+name+r': &str =\s*("(?:[^"\\]|\\.)*");',source);assert raw,name
 return json.loads(raw[1]).replace('{{name}}','task-demo').replace('{{module}}','task_demo').replace('{{command}}','task-demo.command')
for route,prefix,files in zip(catalog['tasks'],['SOURCE_COMMAND_FILE_TEXT','STDIN_STREAM_DATA'],[[('src/app.spx','APP'),('src/tests.spx','TESTS')],[('src/app.spx','APP'),('src/input.spx','INPUT'),('src/tests.spx','TESTS')]]):
 manifest=constant(prefix+'_MANIFEST');m=tomllib.loads(manifest);sources={path:constant(prefix+'_'+key) for path,key in files}
 assert route['project_profile']==m['package']['profile'] and route['input']==m['command']['input'] and route['capabilities']==m['capabilities']['required'] and route['sources']==m['modules']['sources']
 assert route['target']=='native64'
 actual={'semaprax.toml':manifest,**sources};assert route['files']==[{'path':p,'sha256':sha(v.encode())}for p,v in actual.items()]
 ids=sorted(set(re.findall(r'use function @id\("(std\.[^"]+)"\)', ''.join(sources.values()))));assert route['declarations']==ids
 if prefix=='SOURCE_COMMAND_FILE_TEXT':assert route['dependencies']=={'std.int.decimal':'=0.1.0'} and m['dependencies']=={'std':{'int':{'decimal':'=0.1.0'}}}
 else:assert not route['dependencies'] and not m.get('dependencies') and not ids
library=json.loads((ROOT/'std/catalog.json').read_text())
for route in catalog['tasks']:
 for id in route['declarations']:assert sum(d['id']==id for m in library['modules'] for d in m['declarations'])==1,id
for row in json.loads((P/'new-owners.json').read_text()):
 if row['path'].endswith('.rs'):assert len((ROOT/row['path']).read_text().splitlines())<=1500
assert len((ROOT/'src/cli/help.rs').read_text().splitlines())==1500
assert (ROOT/'tests/cli_help_surface_v1.rs').read_text().endswith('#[path = "cli_help_surface_v1/task_pages.rs"]\nmod task_pages;\n')
assert (ROOT/'crates/semaprax-toolchain/tests/cli_help_surface_v1.rs').read_text().endswith('#[path = "../../../tests/cli_help_surface_v1/task_pages.rs"]\nmod task_pages;\n')
print('PASS: 8 complete before/inverse inputs; old V5/scaffold/catalog exact; generated descriptor source facts; 2040-byte routes; new owners/caps. Rust/help/executable qualification NOT RUN.')
