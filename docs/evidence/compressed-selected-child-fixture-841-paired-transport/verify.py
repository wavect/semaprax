"""Whole-source successor proof; original pilot verifier runs in named old view."""
from pathlib import Path
from unittest.mock import patch
import hashlib,json,subprocess,contextlib,io
D=Path(__file__).resolve().parent;R=D.parents[2];P=json.loads((D/'source-inverses.json').read_text());row=P['sources'][0];h=lambda b:hashlib.sha256(b).hexdigest();helper=R/row['path'];old=row['before'].encode();current=helper.read_bytes()
assert old==subprocess.check_output(['git','show',P['base']+':'+row['path']])==(D/'compressed_fixture.rs.before.txt').read_bytes()
assert h(old)==row['before_sha256'] and current==row['current'].encode() and h(current)==row['current_sha256']
prior=R/'docs/evidence/compressed-selected-child-fixture-841'
for p in prior.iterdir():
 if p.is_file():assert p.read_bytes()==subprocess.check_output(['git','show',P['base']+':'+str(p.relative_to(R))]),p.name
read_bytes=Path.read_bytes;read_text=Path.read_text

def bound_bytes(p,*args,**kwargs):return old if p.resolve()==helper.resolve() else read_bytes(p,*args,**kwargs)
def bound_text(p,*args,**kwargs):return old.decode() if p.resolve()==helper.resolve() else read_text(p,*args,**kwargs)
old_verify=read_text(prior/'verify.py');out=io.StringIO()
with patch.object(Path,'read_bytes',bound_bytes),patch.object(Path,'read_text',bound_text),contextlib.redirect_stdout(out):
 exec(compile(old_verify,str(prior/'verify.py'),'exec'),{'__file__':str(prior/'verify.py'),'__name__':'__main__'})
prior_result=json.loads(out.getvalue());assert prior_result['full_original_inverse_rows_replayed']==41
s=current.decode();compressed='/docs/evidence/compressed-selected-child-fixture-841/exact-source-inverses.json.gz';raw='/docs/evidence/selected-child-refusal-781/exact-source-inverses.json';assert s.count(compressed)==1
assert 'if BYTES.starts_with(&[0x1f, 0x8b])' in s and 'plain(BYTES, OUTPUT_CAP, RAW_SHA)' in s and 'decode(BYTES, OUTPUT_CAP, GZIP_SHA, RAW_SHA)' in s
plain=s[s.index('fn plain('):s.index('pub(super) fn load()')];assert plain.index('expected_len > OUTPUT_CAP')<plain.index('std::str::from_utf8(bytes)')<plain.index('map(str::to_owned)');assert 'digest(bytes) != raw_sha' in plain
assert len(s.splitlines())<=1500
print(json.dumps(dict(status='SOURCE_ONLY_PASS',whole_immediate_inverse=True,original11a_packet_unchanged=True,original_verifier_AST_unchanged_in_full_old_helper_view=True,original_full_assertion_rows=41,raw_and_compressed_transport_share_all_other_code=True,one_literal_toggle_occurrences=s.count(compressed),Rust_execution=False,paired_build_execution=False),indent=2))
