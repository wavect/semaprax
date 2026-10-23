from pathlib import Path
from importlib.machinery import SourceFileLoader
import json,os
r=Path.cwd();o=Path('/Volumes/Semaprax/tmp/ci-38383508340/native-provider-receipt-295');g=SourceFileLoader('g',str(o.parent/'device-free-ram-guard.py')).load_module();t='/Volumes/Semaprax/internal-cache-offload-20261016/rustup-home/toolchains/1.97.1-aarch64-apple-darwin/bin';c=Path('/Volumes/Semaprax/worktrees/ci-canonical-54e');d=c/'target/ci-private/clippy-1.97/debug/deps';meta=json.loads((o.parent/'active-canonical-5956/focused-externs.json').read_text());env=dict(os.environ,CARGO_MANIFEST_DIR=str(r),RUSTC_BOOTSTRAP='1');args=['--edition=2021','--test','-Cdebuginfo=0','-Ccodegen-units=64','-Zthreads=1','-Zno-parallel-backend','-L','dependency='+str(d),'--extern','semaprax='+str(r/'target/ci-private/native-provider-receipt-295/libsemaprax-native-receipt.rlib'),*meta['args'],str(o/'cxx-original.rs'),'-o',str(o/'cxx-original')]
g.guarded([t+'/rustc',*args],o/'cxx-build.log',env,start_gib=2.5,pause_gib=1.8,resume_gib=2.5)
g.guarded([str(o/'cxx-original'),'cxx_owned_data_package::','--test-threads=1'],o/'all-original-cxx-controls.log',env,start_gib=2.5,pause_gib=1.8,resume_gib=2.5)
g.guarded([str(o/'cxx-original'),'diagnostic_genuine_current_ir_sizes','--exact','--nocapture','--test-threads=1'],o/'genuine-ir-size-diagnostic.log',env,start_gib=2.5,pause_gib=1.8,resume_gib=2.5)
