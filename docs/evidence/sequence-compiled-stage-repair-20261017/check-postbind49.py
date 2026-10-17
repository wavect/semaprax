"""Source-only final unchanged-row reauthentication after binders; no compilation."""
from pathlib import Path
import argparse,json,hashlib,os,stat,subprocess
PINS={'docs/evidence/main-e177-final-source-binding-20261017/current-inputs.json': (33454, 'c7ef5558171837e13132324c39d047d9dac2f0f210322fd0b104e2e843be3209'), 'docs/evidence/main-e177-final-source-binding-20261017/guard_payload.py': (19974, 'b283d115088881f5ba6649c52fdb2a4b7939beb20b601041a245b2cf422d2fc2'), 'docs/evidence/main-e177-full-controller-acquisition-20261017/inputs.json': (237694, '77863eae5848b0a166e6d71393ecec786d4992bc4163b3f644766d901a18dc29'), 'src/source_reflows_merge/main_e177_final_inputs.rs': (14776, 'ddbb8e44763c6eaaa44a1d0bf17afa796883b7b2d6a925c0b7509e85e352d5c5')}
p=argparse.ArgumentParser();p.add_argument('--repo',type=Path,required=True);p.add_argument('--expected-head',required=True);p.add_argument('--out',type=Path,required=True);a=p.parse_args();R=a.repo.resolve()
assert R.is_relative_to('/Volumes/Semaprax/worktrees')
head=subprocess.check_output(['git','rev-parse','HEAD'],cwd=R).decode().strip();assert head==a.expected_head
H=lambda b:hashlib.sha256(b).hexdigest()
def held(path,n,h):
 # Distinct exact historical cap of the new closed Rust/source domain only.
 # No original262144 strict-reader cap is changed or reused here.
 assert 0<=n<=9753094
 fd=os.open(R/path,os.O_RDONLY|os.O_NOFOLLOW|os.O_NONBLOCK|os.O_CLOEXEC)
 ident=lambda s:(s.st_dev,s.st_ino,s.st_mode,s.st_size,s.st_mtime_ns,s.st_ctime_ns)
 try:
  first=os.fstat(fd);assert stat.S_ISREG(first.st_mode)and first.st_size==n
  with os.fdopen(fd,'rb',closefd=False)as f:b=f.read(n);assert len(b)==n and not f.read(1)
  assert ident(first)==ident(os.fstat(fd))==ident(os.lstat(R/path))and H(b)==h,path
  return b
 finally:os.close(fd)
held_inputs={p:held(p,*pin)for p,pin in PINS.items()}
packet=json.loads(held_inputs['docs/evidence/main-e177-final-source-binding-20261017/current-inputs.json']);assert len(packet['rows'])==49 and packet['source_cap']==9753094
for row in packet['rows']:
 current=row['current'];b=held(row['source'],current['length'],current['sha256']);assert b==held(current['archive'],current['length'],current['sha256'])
 for prior in row['views']:held(prior['archive'],prior['length'],prior['sha256'])
for row in packet['complete_old_table_anchors']:assert held(row['path'],row['length'],row['sha256'])==held(row['archive'],row['length'],row['sha256'])
main=json.loads(held_inputs['docs/evidence/main-e177-full-controller-acquisition-20261017/inputs.json']);assert len(main['rows'])==471 and main['main_head']=='e177a4f4ec59ca53051b6e2322626eb50daeed43'
for row in main['rows']:
 held(row['path'],row['current_length'],row['current_sha256']);held(row['main_archive'],row['main_length'],row['main_sha256'])
import ast
cfg=next(ast.literal_eval(n.value)for n in ast.parse(held_inputs['docs/evidence/main-e177-final-source-binding-20261017/guard_payload.py']).body if isinstance(n,ast.Assign)and isinstance(n.targets[0],ast.Name)and n.targets[0].id=='CONFIG')
assert len(cfg['modules'])==36
for path,row in cfg['modules'].items():held(path,row['length'],row['sha256'])
for path,row in cfg['carriers'].items():
 b=held(path,row['length'],row['sha256']).decode()
 for module in cfg['modules']:assert b.count('include_str!("'+row['base']+module+'")')==1
 if 'ledger_capacity'in path:assert len(b.splitlines())<=1500
row=cfg['table'];b=held(row['path'],row['length'],row['sha256']);old=held(row['prior']['archive'],row['prior']['length'],row['prior']['sha256']);lines=old.decode().splitlines()
for x in row['insertions']:lines.insert(x['at'],x['path'])
assert b==('\n'.join(lines)+'\n').encode()and len(lines)==len(set(lines))==441
report=dict(status='PASS_SOURCE_ONLY_POSTBIND_REAUTHENTICATION',head=head,dirty_paths=subprocess.check_output(['git','status','--porcelain','--untracked-files=all'],cwd=R).decode().splitlines(),current_rows=49,incoming_namespace_current_rows=471,whole_immutable_incoming_archives=471,old_table_anchors=2,new_raw_modules=36,ordered_table_rows=441,pins=PINS,Cargo=False,Rust_executed=False)
assert not a.out.exists();a.out.write_text(json.dumps(report,indent=2)+'\n');print(a.out)
