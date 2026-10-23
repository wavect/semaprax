from pathlib import Path
from importlib.machinery import SourceFileLoader
import json,os,hashlib
r=Path.cwd();s=Path('/Volumes/Semaprax/tmp/ci-38383508340');o=s/'windows-starter-7a252';t=Path('/Volumes/Semaprax/internal-cache-offload-20261016/rustup-home/toolchains/1.97.1-aarch64-apple-darwin/bin');g=SourceFileLoader('guard',str(s/'device-free-ram-guard.py')).load_module();ns={};exec(compile((r/'docs/evidence/production-source-authority-partition-20261016/original-module-parser-prefix.py').read_text(),'original-parser','exec'),ns)
def function(text,name):
 a=text.index('fn '+name+'(');m=ns['mask'](text);i=m.index('{',a);b=i+1;depth=1
 while depth:depth+=(m[b]=='{')-(m[b]=='}');b+=1
 return text[a:b]
p=r/'crates/semaprax-toolchain/src/new_project/application.rs';text=p.read_text();parent=(p.parent/'../new_project.rs').resolve().read_text();a=parent.rfind('#[derive',0,parent.index('pub(crate) struct NewProjectFailure'));b=parent.index('\npub(crate) trait WriteHook',a);failure=parent[a:b]
subject=text[:text.index('pub(super) fn run(')].replace(function(text,'diagnostic'),'').replace('mod held_input_path;','#[path = "'+str(p.with_suffix('')/'held_input_path.rs')+'"]\nmod held_input_path;')
body='#![allow(dead_code)]\nuse std::{fmt,fs,path::{Path,PathBuf},sync::atomic::{AtomicU64,Ordering}};\n'+next(x for x in parent.splitlines()if x.startswith('static STAGING_SERIAL:'))+'\n'+failure+'\nmod application {\n'+subject+'\n'+function(text,'root')+'\n#[test]\n'+function(text,'starter_input_held_regular_reader_preserves_exact_bytes_and_bounds')+'\n#[test]\n'+function(text,'starter_reader_refuses_parent_rebinding_after_the_held_open')+'\n}\n'
source=o/'reader-current.rs';source.write_text(body)
platform=None
for line in (o/'platform-build.log').read_text().splitlines():
 if line.startswith('{'):
  d=json.loads(line)
  if d.get('reason')=='compiler-artifact'and d.get('target',{}).get('name')=='semaprax_native_rust_interop_platform':platform=next(x for x in d['filenames']if x.endswith('.rlib'))
assert platform
normal=json.loads((s/'native-provider-receipt-295/normal-command.json').read_text())['command'];dependency=next(x for x in normal if x.startswith('dependency='));library=r/'target/ci-private/native-provider-receipt-295/libsemaprax-native-receipt.rlib';release=r/'target/ci-private/source-gate-release-1.97/release/deps'
cmd=[str(t/'rustc'),'--edition=2021','--test','-Cdebuginfo=0','-Ccodegen-units=64','-Zthreads=1','-Zno-parallel-backend','-L',dependency,'-L','dependency='+str(release),'--extern','semaprax_native_rust_interop_platform='+platform,str(source),'-o',str(o/'reader-current')]
(o/'reader-scope.json').write_text(json.dumps(dict(command=cmd,actual_reader_functions_and_original_test_bodies_preserved=True,actual_failure_type=True,genuine_platform=platform,compiler_IR='not involved in reader; unrelated application operations not qualified',no_mock_platform_or_IR=True,qualification='native current reader and portable Windows-prefix construction; actual Windows OS execution remains hosted',application_sha256=hashlib.sha256(p.read_bytes()).hexdigest(),prefix_sha256=hashlib.sha256((p.with_suffix('')/'held_input_path.rs').read_bytes()).hexdigest()),indent=2))
env=dict(os.environ,RUSTC_BOOTSTRAP='1',CARGO_MANIFEST_DIR=str(r));g.guarded(cmd,o/'reader-build.log',env,start_gib=3.0,pause_gib=2.0,resume_gib=3.0);g.guarded([str(o/'reader-current'),'--test-threads=1'],o/'reader-tests.log',env,start_gib=3.0,pause_gib=2.0,resume_gib=3.0)
clippy=[str(t/'clippy-driver'),*cmd[1:]];clippy[clippy.index('-o')+1]=str(o/'reader-clippy.rmeta');clippy+=['--emit=metadata','-Dwarnings'];g.guarded(clippy,o/'reader-clippy.log',env,start_gib=3.0,pause_gib=2.0,resume_gib=3.0)
