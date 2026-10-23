from pathlib import Path
import json,tomllib
ns={};exec(compile(Path('/Volumes/Semaprax/tmp/ci-38383508340/inline-targets-d83/run-focused-root.py').read_text().split('cmd=[')[0],'genuine-normal-environment','exec'),ns)
r=Path.cwd();o=Path('/Volumes/Semaprax/tmp/ci-38383508340/native-provider-receipt-295');m=r/'target/ci-private/native-provider-receipt-295';m.mkdir(parents=True,exist_ok=True);env=ns['env'];g=ns['g'];meta=ns['meta'];d=ns['d'];tool=ns['t']/'rustc'
cmd=[str(tool),'--edition=2021','--crate-name','semaprax','--crate-type','rlib','-Copt-level=0','-Cdebuginfo=0','-Ccodegen-units=256','-Cmetadata=actual_native_provider_receipt_295','-Zthreads=1','-Zno-parallel-backend','-L','dependency='+str(d),*meta['args']]
for name in meta['features']:cmd+=['--cfg','feature="'+name+'"']
cmd +=[str(r/'src/lib.rs'),'-o',str(m/'libsemaprax-native-receipt.rlib')]
(o/'normal-command.json').write_text(json.dumps(dict(command=cmd,scope='Complete genuine current production compiler; normal cfg, all features, original default limits, original IR, no source overlay or diagnostic mode override'),indent=2)+'\n')
clippy=[str(tool.with_name('clippy-driver')),*cmd[1:]];clippy[clippy.index('-o')+1]=str(m/'normal-clippy.rmeta');clippy+=['--emit=metadata','-Dwarnings']
for group,items in tomllib.loads((r/'Cargo.toml').read_text()).get('lints',{}).items():
 for lint,value in items.items():
  level=value if isinstance(value,str)else value['level'];clippy += ['--'+level,(('clippy::'if group=='clippy'else'')+lint)]
g.guarded(clippy,o/'normal-clippy.log',env,start_gib=4.5,pause_gib=2.75,resume_gib=3.5)
