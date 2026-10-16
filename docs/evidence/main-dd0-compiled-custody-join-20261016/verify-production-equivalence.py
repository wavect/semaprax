"""Read-only currentHEAD equivalence after the full fixed sourceproof passes."""
from pathlib import Path
import argparse,json,subprocess,hashlib
A=argparse.ArgumentParser();A.add_argument('--expected-head',required=True);A.add_argument('--out',type=Path,required=True);a=A.parse_args()
D=Path(__file__).resolve().parent;R=D.parents[2]
def git(*x):return subprocess.check_output(['git',*x],cwd=R,text=True).strip()
assert git('rev-parse','HEAD')==a.expected_head and not git('status','--porcelain','--untracked-files=all')
assert not a.out.exists()and not a.out.resolve().is_relative_to(R)
subprocess.run(['python3',str(D/'verify-source.py')],check=True)
assert git('rev-parse','HEAD')==a.expected_head and not git('status','--porcelain','--untracked-files=all')
proof=json.loads((D/'source-proof.json').read_text());assert proof['complete_production_source_equivalence_to83ca']
result={'status':'COMPLETE_PRODUCTION_SOURCE_EQUIVALENCE_PASS','production_source_head':'83ca0e105e5f4eb6336ed04ff80df6ad0366e99f','current_source_head':a.expected_head,'source_proof_sha256':hashlib.sha256((D/'source-proof.json').read_bytes()).hexdigest(),'current_consumer_PASS':False,'producer_artifacts_and_captures_are_explicit83ca_history':True,'all_original_production_source_and_Cargo_bytes_unchanged':True}
a.out.write_text(json.dumps(result,indent=2)+'\n');print(json.dumps(result))
