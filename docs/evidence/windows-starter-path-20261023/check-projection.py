from pathlib import Path
from importlib.machinery import SourceFileLoader
import json,os,re
r=Path.cwd();o=Path('/Volumes/Semaprax/tmp/ci-38383508340/windows-starter-7a252');g=SourceFileLoader('g',str(o.parent/'device-free-ram-guard.py')).load_module();ns={};exec(compile((r/'docs/evidence/production-source-authority-partition-20261016/original-module-parser-prefix.py').read_text(),'parser','exec'),ns)
def rebase(s):
 m=ns['mask'](s);edits=[]
 for v in re.finditer(r'include_str!\(\s*"([^"]+)"\s*\)',s):
  if m.startswith('include_str!',v.start()):edits.append((v.start(1),v.end(1),str((r/'tests/workspace'/v[1]).resolve())))
 for a,b,v in reversed(edits):s=s[:a]+v+s[b:]
 return s
text=(r/'tests/workspace/semantic_graph.rs').read_text();name='expected_projection_source_boundary_is_pure_and_keeps_shared_helpers_in_root';start=text.index('fn '+name+'(');mask=ns['mask'](text);i=mask.index('{',start);depth=1;end=i+1
while depth:depth+=(mask[end]=='{')-(mask[end]=='}');end+=1
body=text[start:end]
for helper in ['historical_projection_root_facade','historical_projection_root_facade_before','current_projection_facade_input']:
 a=text.index('fn '+helper+'(');i=mask.index('{',a);depth=1;b=i+1
 while depth:depth+=(mask[b]=='{')-(mask[b]=='}');b+=1
 body+='\n'+text[a:b]+'\n'
context=text[text.index('// The purity gate above still consumes'):text.index('\n#[test]\nfn private_layout_inventory_retains')]
# Only literal module acquisition paths are rebased. Every helper body stays real.
context=__import__('re').sub(r'#\[path = "([^"]+)"\]',lambda m:'#[path = "'+str((r/'tests/workspace'/m[1]).resolve())+'"]',context)
header=(r/'target/ci-private/current-source-harness/actual-current.rs').read_text()+'\npub(crate) use source_reflows_merge::{main_e177_final_inputs,next_big_batch_inputs};\n'
source='#![allow(unused_macros)]\nconst SEMANTIC_GRAPH_PRIVATE_LAYOUT_ROOT: &str="src/workspace_graph/expected_projection/private_layout.rs";\n#[path="'+str(r/'tests/support/production_source_inventory.rs')+'"] mod production_source_inventory;\n'+rebase((r/'tests/workspace/semantic_graph/source_inventory.rs').read_text())+'\n#[test]\n'+rebase(body)+'\n'+context+'\n';source=header+'\nmod projected_gate {\n'+source+'\n}\n';p=o/'projection-audit.rs';p.write_text(source)
(o/'projection-scope.json').write_text(json.dumps(dict(original_function=name,body_preserved_except_literal_acquisition_paths=True,no_mock_ir_or_layout=True,original_source_inventory_macros=True,scope='original source-only projection gate; no compiler/backend product qualification'),indent=2)+'\n')
t='/Volumes/Semaprax/internal-cache-offload-20261016/rustup-home/toolchains/1.97.1-aarch64-apple-darwin/bin';c=Path('/Volumes/Semaprax/worktrees/ci-canonical-54e');d=c/'target/ci-private/clippy-1.97/debug/deps';meta=json.loads((o.parent/'active-canonical-5956/focused-externs.json').read_text());env=dict(os.environ,CARGO_MANIFEST_DIR=str(r),RUSTC_BOOTSTRAP='1');args=['--edition=2021','--test','-Cdebuginfo=0','-Ccodegen-units=64','-Zthreads=1','-Zno-parallel-backend','-L','dependency='+str(d),'--extern','semaprax='+str(r/'target/ci-private/native-provider-receipt-295/libsemaprax-native-receipt.rlib'),*meta['args'],str(p),'-o',str(o/'projection-audit')]
g.guarded([t+'/rustc',*args],o/'projection-build.log',env,start_gib=2.5,pause_gib=1.8,resume_gib=2.5)
g.guarded([str(o/'projection-audit'),'projected_gate::'+name,'--exact','--test-threads=1'],o/'projection-test.log',env)
