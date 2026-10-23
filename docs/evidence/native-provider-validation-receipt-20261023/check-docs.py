from pathlib import Path
import os,json,hashlib
from importlib.machinery import SourceFileLoader
r=Path.cwd();o=Path('/Volumes/Semaprax/tmp/ci-38383508340/native-provider-receipt-295')
s=(r/'tests/documentation.rs').read_text()
tests=s[s.index('#[test]\nfn local_markdown_links_resolve'):s.index('/// The VS Code')]
helpers=s[s.index('fn collect_markdown('):s.index('mod agent_quick_reference {')]
body='use std::path::{Path, PathBuf};\n#[path = "'+str(r/'tests/documentation/archived_sources.rs')+'"]\nmod archived_sources;\n'+tests+helpers
source=o/'docs-original.rs';source.write_text(body)
d=json.loads((o/'normal-command.json').read_text())['command'];a=d.index('--extern');ext=[]
for i,x in enumerate(d):
 if x=='--extern' and d[i+1].startswith('sha2='):ext=['--extern',d[i+1]]
cmd=[d[0],'--edition=2021','--test','-Cdebuginfo=0','-L',next(x for x in d if x.startswith('dependency=')),*ext,str(source),'-o',str(o/'docs-original')]
(o/'docs-scope.json').write_text(json.dumps(dict(source_sha256=hashlib.sha256(s.encode()).hexdigest(),exact_tests=tests,exact_helpers=helpers,command=cmd),indent=2))
g=SourceFileLoader('guard',str(o.parent/'device-free-ram-guard.py')).load_module();env=dict(os.environ,CARGO_MANIFEST_DIR=str(r))
g.guarded(cmd,o/'docs-build.log',env,start_gib=2.5,pause_gib=1.8,resume_gib=2.5)
g.guarded([str(o/'docs-original'),'--test-threads=1'],o/'docs-controls.log',env,start_gib=2.5,pause_gib=1.8,resume_gib=2.5)
