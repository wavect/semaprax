"""Complete BCC source transport and preserved MAIN34d source proof; no Rust."""
from pathlib import Path
import argparse,hashlib,json,subprocess,os,tempfile
REL=Path('docs/evidence/mainbcc-ordinary-source-domain-20261017')
MAIN='bccbe9696eca892045b0603ad663234062df15f4'
FORMAT_NAMESPACE_SHA='c67837ba0c7b584b32b59bba4a6c414b628ba2b00326e8510f8efc6ca7279b1e'
sha=lambda b:hashlib.sha256(b).hexdigest()
def previous_receiver(repo,current):
 R=Path(repo);D=R/REL
 # Authenticate physical RAW and acquire the complete prior namespace first.
 import importlib.util
 bridge=R/'docs/evidence/c874-supplement-custody-domains-20261017/format-namespace.py'
 assert sha(bridge.read_bytes())==FORMAT_NAMESPACE_SHA
 spec=importlib.util.spec_from_file_location('closed_format_namespace',bridge);module=importlib.util.module_from_spec(spec);spec.loader.exec_module(module)
 current=module.previous_namespace(R,current)
 original=(D/'original-programs/src__source_reflows__semantic_receivers.rs').read_bytes()
 assert sha(original)=='6816d73e025e149033a83a0bce3f0f1bd3072563fad3a8b715395274a6a8eb2b'
 bcc=(D/'original-programs/semantic_receivers.bcc.txt').read_bytes()
 assert bcc==subprocess.check_output(['git','-C',str(R),'show',MAIN+':src/source_reflows/semantic_receivers.rs'])
 marker=b'#[test]\nfn ordinary_replay_routes_keep_complete_predecessors_and_existing_controls() {'
 assert bcc.count(marker)==1
 body=bcc[bcc.index(marker):]
 preamble=b'\n    // This complete incoming owner reads only its independent immutable BCC domain.\n    let main_read = |path: std::path::PathBuf| mainbcc_read(&path);\n    let invert = mainbcc_invert;'
 body=body.replace(marker,marker+preamble,1).replace(b'std::fs::read_to_string(',b'main_read(')
 expected=original.replace(b'use super::*;',b'use super::*;\nuse super::incoming67107_semantic_receivers::{mainbcc_read, mainbcc_invert};',1)+b'\n'+body
 assert current.encode('utf8')==expected,'complete old receiver or complete BCC owner changed'
 return original.decode('utf8')
def authenticate_current(path,archived):
 raw=path.read_bytes();assert raw==archived,('physical current BCC input drift',str(path))
 return raw
def main():
 p=argparse.ArgumentParser();p.add_argument('--repo',type=Path,required=True);p.add_argument('--inputs',type=Path,required=True);p.add_argument('--production-overlay',type=Path);a=p.parse_args();R=a.repo.resolve();I=a.inputs.resolve();D=R/REL
 old=lambda path:subprocess.check_output(['git','-C',str(R),'show',MAIN+':'+path])
 manifest=json.loads((D/'input-manifest.json').read_bytes());entries=manifest['inputs'];assert len(entries)==13 and len({e['path']for e in entries})==13
 table=old('docs/evidence/ordinary-replay-routes-v1/sources.json');assert sha(table)==manifest['original_table_sha256'];rows=json.loads(table);assert len(rows)==6
 ordered=['docs/evidence/ordinary-replay-routes-v1/sources.json']
 for row in rows:
  current=old(row['path']);before=old(row['archive']);assert sha(current)==row['after_sha256'] and sha(before)==row['before_sha256']
  text=current.decode()
  for edit in row['inverse']:
   assert text.count(edit['after'])==1;text=text.replace(edit['after'],edit['before'],1)
  assert text.encode()==before;ordered.extend([row['path'],row['archive']])
 assert [e['path']for e in entries]==ordered
 for e in entries:
  for prefix in ['current','main']:
   path=e[prefix+'_archive'];root=R if path.startswith(str(REL))else I;b=(root/path).read_bytes()
   assert len(b)==e[prefix+'_len'] and sha(b)==e[prefix+'_sha256'] and len(b)<=1_048_576
   if prefix=='main':assert b==old(e['path'])
   else:
    if e['path']=='src/source_reflows/semantic_receivers.rs':actual=R/e['path']
    elif a.production_overlay and e['path'].startswith('src/'):actual=a.production_overlay/e['path']
    else:actual=I/e['path']
    authenticate_current(actual,b)
 previous_receiver(R,(R/'src/source_reflows/semantic_receivers.rs').read_text())
 helper=(R/'src/source_reflows/incoming67107_semantic_receivers.rs').read_text();original=(D/'original-programs/src__source_reflows__incoming67107_semantic_receivers.rs').read_text()
 # Only the unrelated existing MAIN34d scalar endpoint pins may later rebind.
 import re
 scrub=lambda s:re.sub(r'const MAIN34D_MANIFEST_(?:LEN: usize = [0-9]+|SHA: &str = "[0-9a-f]{64}");','MAIN34D_UNQUALIFIED_ENDPOINT_PIN',s)
 assert scrub(helper).startswith(scrub(original))
 bcc=(D/'original-programs/semantic_receivers.bcc.txt').read_text();start=bcc.index('fn invert(');end=bcc.index('\npub(super) fn predecessor(',start)
 inverse=bcc[start:end].replace('fn invert(','pub(super) fn mainbcc_invert(',1)
 assert helper.count(inverse)==1
 for name in ['full-source-inverse.json','main34d-endpoint-inverse.json']:
  for row in json.loads((D/name).read_bytes())['rows']:
   current=(R/row['path']).read_bytes();before=(R/row['before_archive']).read_bytes()
   assert len(current)==row['current_len'] and sha(current)==row['current_sha256']
   assert len(before)==row['before_len'] and sha(before)==row['before_sha256']
   assert row['inverse']==[{'at':0,'remove_len':len(current),'remove_sha256':sha(current),'insert_archive':row['before_archive']}]
 assert os.environ['TMPDIR']=='/Volumes/Semaprax/runtime-tmp'
 with tempfile.TemporaryDirectory(prefix='bcc-physical-source-controls-',dir=os.environ['TMPDIR'])as temporary:
  physical=Path(temporary)/'current';physical.write_bytes(b'exact\n');authenticate_current(physical,b'exact\n')
  for changed in [b'Exact\n',b'exact\r\n',b'exact',b'exact\n\n']:
   physical.write_bytes(changed)
   try:authenticate_current(physical,b'exact\n')
   except AssertionError:pass
   else:raise AssertionError('physical stale source admitted')
 print(json.dumps({'status':'PASS_SOURCE_ONLY_NOT_RUST','physical_current_inputs':13,'physical_drift_negative_controls':4,'production_overlay':str(a.production_overlay)if a.production_overlay else None,'ordered_original_rows':6,'complete_inputs':13,'whole_BCC_original_inverter':True,'complete_original_BCC_owner_and_ALL_MAIN34d_receiver_bytes_preserved':True,'no_original_table_or_archive_retargeted':True}))
if __name__=='__main__':main()
