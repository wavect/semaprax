from pathlib import Path
import json,os
from importlib.machinery import SourceFileLoader
r=Path.cwd();o=Path('/Volumes/Semaprax/tmp/ci-38383508340/windows-starter-7a252');c=Path('/Volumes/Semaprax/worktrees/ci-canonical-54e');h=r/'target/ci-private/current-source-harness';h.mkdir(parents=True,exist_ok=True)
for p in (c/'target/ci-private/current-source-harness').glob('*.rs'):(h/p.name).write_text(p.read_text().replace(str(c),str(r)))
g=SourceFileLoader('guard',str(o.parent/'device-free-ram-guard.py')).load_module();release=json.loads((o.parent/'source-release-deps-1.97.json').read_text());d={n:v['path'] for n,v in release.items()};normal=json.loads((o/'normal-command.json').read_text())['command'];d['libc']=next(normal[i+1].split('=',1)[1] for i,v in enumerate(normal) if v=='--extern'and normal[i+1].startswith('libc='));t='/Volumes/Semaprax/internal-cache-offload-20261016/rustup-home/toolchains/1.97.1-aarch64-apple-darwin/bin';env=dict(os.environ,RUSTC_BOOTSTRAP='1',CARGO_MANIFEST_DIR=str(r));common=['--test','--edition=2021','-Copt-level=1','-Cdebuginfo=0','-Ccodegen-units=64','-Zthreads=1','-Zno-parallel-backend','-L','dependency='+release['sha2']['dependency_directory'],'--remap-path-prefix='+str(h)+'=src']
for name,p in d.items():common+=['--extern',name+'='+p]
g.guarded([t+'/rustc',*common,str(h/'actual-current.rs'),'-o',str(h/'tests')],o/'overlay-build.log',env,start_gib=2.5,pause_gib=1.8,resume_gib=2.5)
for selector,name in [('source_reflows_merge::main_e177_final_inputs::overlay_v2::','overlay-controls.log'),('compiled_lookup_split_','compiled-whole-predecessor-controls.log'),('current_audit_entry_','new-entry-controls.log')]:g.guarded([str(h/'tests'),selector,'--test-threads=1'],o/name,env)
