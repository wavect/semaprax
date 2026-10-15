"""Frozen source contract and private environment; preparation never freezes source."""
from pathlib import Path
import hashlib,json,os,subprocess
TARGET_RELATIVE = "target/final-cgu64-20261012"
NODE_PREFIX = "/Users/kevin/.nvm/versions/node/v24.3.0/bin"
def verify_frozen(manifest, root):
 if not manifest.get('source_subject') or manifest.get('freeze_required'):
  raise SystemExit('Manifest remains UNFROZEN; root must authorize final HEAD')
 head=subprocess.check_output(['git','rev-parse','HEAD'],cwd=root,text=True).strip()
 tree=subprocess.check_output(['git','rev-parse','HEAD^{tree}'],cwd=root,text=True).strip()
 if head!=manifest['source_subject'] or tree!=manifest.get('source_tree'):
  raise SystemExit('Frozen HEAD/tree drift')
 for relative,digest in manifest['source_file_sha256'].items():
  p=(root/relative).resolve()
  if not p.is_relative_to(root.resolve()) or hashlib.sha256(p.read_bytes()).hexdigest()!=digest:
   raise SystemExit('Frozen source-file digest drift: '+relative)
def private_target(root):
 target=(root/TARGET_RELATIVE).resolve()
 if not target.is_relative_to(root.resolve()) or not target.is_relative_to((root/'target').resolve()):
  raise SystemExit('Target escapes this checkout')
 return target
def prepare_environment(env, out):
 if not (Path(NODE_PREFIX)/'node').is_file():raise SystemExit('Required nvm Node24 executable unavailable')
 env['PATH']=NODE_PREFIX+os.pathsep+env.get('PATH','/usr/bin:/bin')
 version=subprocess.check_output([str(Path(NODE_PREFIX)/'node'),'--version'],env=env,text=True).strip()
 if not version.startswith('v24.'):raise SystemExit('Owning driver requires Node24')
 (out/'runtime-tmp').mkdir(exist_ok=True)
 env.update(CARGO_BUILD_JOBS='1',CARGO_NET_OFFLINE='true')
 return version
