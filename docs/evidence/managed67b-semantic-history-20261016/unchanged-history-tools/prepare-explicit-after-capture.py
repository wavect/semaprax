#!/usr/bin/env python3
"""Bind explicit actual ROOT operands, then run the original strict read-only plan.

No build, capture, producer, source write or pin application is performed.
"""
from pathlib import Path
import argparse,hashlib,json,re,shutil,subprocess,sys
D=Path(__file__).resolve().parent
P=argparse.ArgumentParser();P.add_argument('--repo',required=True,type=Path);P.add_argument('--expected-head',required=True);P.add_argument('--round',required=True,type=Path);P.add_argument('--capture',required=True,type=Path);P.add_argument('--readiness',required=True,type=Path)
for n in ['contract','manifest','build-receipt','producer-receipt','readiness']:P.add_argument('--expected-'+n+'-sha256',required=True)
P.add_argument('--out',required=True,type=Path);a=P.parse_args();R=a.repo.resolve();Q=a.round.resolve();OUT=a.out.resolve();BOUND=OUT.with_name(OUT.name+'.bound-tools')
def sha(p):
 with Path(p).open('rb')as f:return hashlib.file_digest(f,'sha256').hexdigest()
def git(*args):return subprocess.check_output(['git',*args],cwd=R,text=True).strip()
for p in [R,Q,a.capture.resolve(),a.readiness.resolve(),OUT,BOUND]:assert str(p).startswith('/Volumes/Semaprax/')
assert not OUT.exists()and not BOUND.exists();assert not OUT.is_relative_to(R)and not BOUND.is_relative_to(R)
assert re.fullmatch('[0-9a-f]{40}',a.expected_head)
for n in ['contract','manifest','build_receipt','producer_receipt','readiness']:assert re.fullmatch('[0-9a-f]{64}',getattr(a,'expected_'+n+'_sha256'))
assert git('rev-parse','HEAD')==a.expected_head and not git('status','--porcelain','--untracked-files=all')
assert sha(Q/'expected-counts.json')==a.expected_manifest_sha256
assert sha(Q/'producer-build-contract.json')==a.expected_contract_sha256
assert sha(a.readiness)==a.expected_readiness_sha256
manifest=json.loads((Q/'expected-counts.json').read_text());assert manifest['source_subject']==a.expected_head and Path(manifest['draft_repository']).resolve()==R
proof=json.loads(a.readiness.read_text());assert proof['source_subject']==a.expected_head and proof['status']=='ACTUAL_PRODUCER_ONLY_THREE_BUILD_ARTIFACTS_VALIDATED_NO_CAPTURE'
preparation=json.loads((D/'tool-preparation.json').read_text())
for n,h in preparation['immutable_originals'].items():assert sha(D/'original-tool-sources'/n)==h
for n,h in preparation['derived_sources'].items():assert sha(D/n)==h
# Bind a private external source toolset. Preserve full predecessor sources/inverse.
BOUND.mkdir()
for n in preparation['immutable_originals']:shutil.copyfile(D/n,BOUND/n)
shutil.copyfile(D/'actual55e-history-binding.json',BOUND/'actual55e-history-binding.json')
strict=BOUND/'strict-828f-capture-only.py';before=strict.read_text();old='55e805c2f6e23dda58ce4c72223914ed511b117b';assert before.count(old)==1;strict.write_text(before.replace(old,a.expected_head,1));assert strict.read_text().replace(a.expected_head,old,1)==before
record=json.loads((D/'skeleton-record.json').read_text());record.update(repository=str(R),frozen_source_head=a.expected_head,frozen_source_tree=git('rev-parse','HEAD^{tree}'),required_round=str(Q),required_manifest_sha256=a.expected_manifest_sha256,producer_build_contract_sha256=a.expected_contract_sha256,strict_checker_full_head_locked=a.expected_head,state='BOUND_ACTUAL_THREE_SUCCESS_REQUIRED_NO_FUTURE_VALUES',capture_performed=False,pin_apply=False)
record['old_source_sha256']={}
for name in json.loads((D/'skeleton-record.json').read_text())['old_source_sha256']:
 raw=(R/name).read_bytes();assert raw==subprocess.check_output(['git','show',a.expected_head+':'+name],cwd=R);p=BOUND/'frozen-source-before'/name;p.parent.mkdir(parents=True,exist_ok=True);p.write_bytes(raw);record['old_source_sha256'][name]=hashlib.sha256(raw).hexdigest()
record['trusted_tool_sha256']={n:sha(BOUND/n)for n in record['trusted_tool_sha256']};record['trusted_tool_sha256']['actual55e-history-binding.json']=sha(BOUND/'actual55e-history-binding.json')
(BOUND/'skeleton-record.json').write_text(json.dumps(record,indent=2)+'\n')
(BOUND/'explicit-binding.json').write_text(json.dumps({'source_head':a.expected_head,'round':str(Q),'manifest_sha256':a.expected_manifest_sha256,'contract_sha256':a.expected_contract_sha256,'strict_predecessor_sha256':sha(D/'strict-828f-capture-only.py'),'strict_bound_sha256':sha(strict),'strict_single_fullhead_literal_inverse':True,'capture':str(a.capture.resolve()),'capture_execution':False,'pin_apply':False},indent=2)+'\n')
cmd=[sys.executable,str(BOUND/'plan-after-capture.py'),'--expected-head',a.expected_head,'--round',str(Q),'--capture',str(a.capture.resolve()),'--readiness',str(a.readiness.resolve()),'--expected-contract-sha256',a.expected_contract_sha256,'--expected-manifest-sha256',a.expected_manifest_sha256,'--expected-build-receipt-sha256',a.expected_build_receipt_sha256,'--expected-producer-receipt-sha256',a.expected_producer_receipt_sha256,'--expected-readiness-sha256',a.expected_readiness_sha256,'--out',str(OUT)]
subprocess.run(cmd,check=True)
result=json.loads((OUT/'prepared-patch-record.json').read_text());amount=result['actual_new_amount'];assert type(amount)is int and amount>0
(OUT/'immediate-live83ca-summary.json').write_text(json.dumps({'source_head':a.expected_head,'live_source_before_bill':1111428,'actual_captured_bill':amount,'delta':amount-1111428,'historical55e_bill':1110628,'legacy35cc_comparison_separate':True,'all_complete_histories':['35cc','364d','47f','83ca','55e'],'source_apply':False,'exclusive_gain_or_causal_claim':False},indent=2)+'\n')
assert git('rev-parse','HEAD')==a.expected_head and not git('status','--porcelain','--untracked-files=all')
print(OUT/'prepared-patch-record.json')
