"""Source-only raw-default successor: complete prior views and one literal transport edit."""
from pathlib import Path
import contextlib,hashlib,io,json,subprocess,tomllib
R=Path(__file__).resolve().parents[3];D=Path(__file__).resolve().parent;BASE='56dc66694';RAW='57e1485ca695771d0f381ce20f4d740ee98f2b86';h=lambda b:hashlib.sha256(b).hexdigest();rows=json.loads((D/'current-to-56dc.json').read_text());assert len(rows)==3;named={}
for r in rows:
 p=R/r['source'];old=(R/r['archive']).read_bytes();new=p.read_bytes();assert new==r['edits'][0]['new'].encode() and old==r['edits'][0]['old'].encode();assert h(new)==r['current_sha256'] and h(old)==r['original_sha256'];assert old==subprocess.check_output(['git','show',BASE+':'+r['source']],cwd=R);assert r['identity_binding']==(old==new);named[p.resolve()]=old
assert json.loads((D/'source-inverses.json').read_text())==[r for r in rows if not r['identity_binding']]
old,new=rows[0]['edits'][0]['old'],rows[0]['edits'][0]['new'];gzip_path='/docs/evidence/compressed-selected-child-fixture-841/exact-source-inverses.json.gz';raw_path='/docs/evidence/selected-child-refusal-781/exact-source-inverses.json';assert old.count(gzip_path)==1 and new==old.replace(gzip_path,raw_path,1)
assert new.encode()==subprocess.check_output(['git','show',RAW+':'+rows[0]['source']],cwd=R),'exact tested raw helper bytes, not an invented decoder variant'
assert rows[1]['identity_binding'],'whole original source shim/assertion body remains exact'
for p in ['Cargo.toml','Cargo.lock','docs/evidence/compressed-selected-child-fixture-841/exact-source-inverses.json.gz','docs/evidence/selected-child-refusal-781/exact-source-inverses.json']:
 assert (R/p).read_bytes()==subprocess.check_output(['git','show',BASE+':'+p],cwd=R),p
lock=tomllib.loads((R/'Cargo.lock').read_text());prior=tomllib.loads(subprocess.check_output(['git','show',RAW+':Cargo.lock'],cwd=R,text=True));names=['adler2','simd-adler32','miniz_oxide','crc32fast','flate2']
for n in names:
 assert [x for x in lock['package'] if x['name']==n]==[x for x in prior['package'] if x['name']==n] and any(x['name']==n for x in lock['package']),n
raw=(R/'docs/evidence/selected-child-refusal-781/exact-source-inverses.json').read_bytes();assert len(raw)==5196870 and len(json.loads(raw))==41
# All previous source scripts/tables/archives are executed only in their complete authenticated old domain.
script=R/'docs/evidence/alias-module-loader-fix-847-20261015/verify.py';code=script.read_text();rb,rt,wt,co=Path.read_bytes,Path.read_text,Path.write_text,subprocess.check_output;captured=[]
def read_bytes(p,*a,**k):return named[p.resolve()] if p.resolve() in named else rb(p,*a,**k)
def read_text(p,*a,**k):return named[p.resolve()].decode() if p.resolve() in named else rt(p,*a,**k)
def write(p,s,*a,**k):assert p.name=='source-proof.json';captured.append(json.loads(s));return len(s)
try:
 Path.read_bytes,Path.read_text,Path.write_text=read_bytes,read_text,write
 with contextlib.redirect_stdout(io.StringIO()):exec(compile(code,str(script),'exec'),{'__file__':str(script),'__name__':'__main__'})
finally:Path.read_bytes,Path.read_text,Path.write_text,subprocess.check_output=rb,rt,wt,co
assert captured
proof={'status':'SOURCE_ONLY_RAW_DEFAULT_NOT_CURRENT_RUNTIME_QUALIFIED','complete_named_rows':3,'actual_changed_source_rows':2,'one_payload_literal_changed':True,'helper_exact_tested_raw_arm':RAW,'root_source_shim_and_all_41_original_assertion_rows_preserved':True,'all_decoder_hostile_controls_preserved':True,'five_test_dependencies_and_Cargo_bytes_unchanged':True,'raw_fixture_and_gzip_sidecar_unchanged':True,'old_runtime_JSON_scripts_archives_unchanged':True,'whole_predecessor_source_proof':captured[-1],'new_test_bodies':0,'Rust_Cargo_tests_executed':False,'RAM_token_semantic_or_app_gain_claim':False}
(D/'source-proof.json').write_text(json.dumps(proof,indent=2)+'\n');print('PASS exact tested raw helper, one literal,3whole views,41original rows,unchanged predecessor proof; no Rust')
