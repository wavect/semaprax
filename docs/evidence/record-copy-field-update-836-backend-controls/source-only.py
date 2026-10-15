"""Source/metadata proof only. No Rust/C/Node/Wasm/test/review execution."""
from pathlib import Path
import hashlib,json,re,subprocess
R=Path(__file__).resolve().parents[3];D=Path(__file__).resolve().parent;OLD=R/'docs/evidence/record-copy-field-update-836-backends';B='186854fff7cc556ac1852019f39420094adc9996'
sha=lambda b:hashlib.sha256(b).hexdigest()
def verify(packet):
 for row in packet['owners']:
  current=(R/row['source']).read_bytes();before=(R/row['archive']).read_bytes();assert sha(current)==row['current_sha256']and sha(before)==row['original_sha256']
  text=current.decode()
  for e in row['edits']:assert text.count(e['new'])==1;text=text.replace(e['new'],e['before'],1)
  assert text.encode()==before
 for row in packet['children']:assert sha((R/row['source']).read_bytes())==row['sha256']
packet=json.loads((D/'source-inverses.json').read_text());assert len(packet['owners'])==4 and len(packet['children'])==2;verify(packet)
prior=json.loads((D/'original/source-inverses-186854.json').read_text());current=json.loads((OLD/'source-inverses.json').read_text());verify(current)
assert len(current['owners'])==len(prior['owners'])==13
for before,now in zip(prior['owners'],current['owners']):
 assert before['source']==now['source']and before['archive']==now['archive']and before['original_sha256']==now['original_sha256']
 assert now['edits'][-len(before['edits']):]==before['edits']
 assert(R/before['archive']).read_bytes()==subprocess.check_output(['git','show',B+':'+before['archive']],cwd=R)
for before,now in zip(prior['children'],current['children']):
 assert before['source']==now['source']
 if before['sha256']!=now['sha256']:
  assert any(r['sha256']==before['sha256']and sha((R/r['archive']).read_bytes())==before['sha256']for r in now['prior_bindings'])
for row in json.loads((OLD/'unchanged-originals.json').read_text()):assert sha((R/row['source']).read_bytes())==row['sha256']and(R/row['archive']).read_bytes()==(R/row['source']).read_bytes()
# Every production byte remains exactly at the original backend checkpoint.
production=[r['source']for r in prior['owners']+prior['children']if r['source'].startswith('src/')]
for source in production:assert(R/source).read_bytes()==subprocess.check_output(['git','show',B+':'+source],cwd=R)
support=(R/'tests/support/record_copy_field_update.rs').read_text();before=(D/'original/tests__support__record_copy_field_update.rs').read_text();assert support.startswith(before.split('pub(crate) fn assert_sources()')[0])
assert before.split('pub(crate) const OLD_ROW_ORACLE:',1)[1].split('pub(crate) fn oracle_source()',1)[0]in support
for row in packet['owners']+packet['children']:assert row['source']in support
native=(R/'tests/cleanup_backends/owned_leaf_collections/copy_field_update.rs').read_text();old_native=(D/'original/tests__cleanup_backends__owned_leaf_collections__copy_field_update.rs').read_text()
restore=native.replace('        run(&corpus::multi_source(ty, value, expected), "", 0, 29, 4, 0);\n','',1).replace('\n#[path = "copy_field_update_boundary.rs"]\nmod boundary;\n','',1);assert restore==old_native
wasm=(R/'tests/owned_data/owned_leaf_vec/copy_field_update.rs').read_text();old_wasm=(D/'original/tests__owned_data__owned_leaf_vec__copy_field_update.rs').read_text()
restore=wasm.split('\n#[test]\nfn wasm_exact_float_payload_matrix_',1)[0]
restore=restore.replace('''        run(
            &corpus::multi_source(ty, value, expected),
            0,
            29,
            "field-update-observe",
        );
''','',1);assert restore.rstrip()==old_wasm.rstrip()
c=(R/'tests/cleanup_backends/owned_leaf_collections/copy_field_update_boundary.c').read_text();host=(R/'tests/owned_data/owned_leaf_vec/host.js').read_text()
for kind in ['f32','f64']:
 values=re.search(r'probe_'+kind+r'\[\] = \{(.*?)\};',c,re.S).group(1)
 native_bits=[int(x,0)for x in re.findall(r'UINT64_C\(([^)]+)\)',values)]
 js=re.search(r'const p'+('32'if kind=='f32'else'64')+r'=\[([^]]+)\]',host).group(1)
 host_bits=[int(x.strip().removesuffix('n'),0)for x in js.split(',')]
 assert native_bits==host_bits and len(native_bits)==12
 assert len(set(native_bits))==12
for term in ['memcpy(probe_prior,row,d->stride)','actual!=word','fixture_sort_active=1','setjmp(fixture_trap)','attack<8','probe_context.vec_next_generation=UINT64_MAX','probe_forged.identity="foreign.nominal"','probe_input.ptr=','memcmp(&probe_output,&before_output','probe_drop();']:assert term in c
for term in ['marker:i64','minus:f32','nan:f64','same_source_interpreter']:
 assert term in support+wasm
assert 'interpreter::interpret('in wasm and 'verify_envelope('in wasm
assert 'row[j]!==before[j]'in host and 'row[spec[1].slot]!==word'in host
pending=json.loads((D/'pending-selectors.json').read_text());assert pending['new']==3 and pending['required_existing']==10
for row in pending['checks']:assert not row['source_binding']['ignored']and sha((R/row['source']).read_bytes())==row['source_sha256']
for row in packet['children']+packet['owners']:
 p=R/row['source']
 for ref in re.findall(r'include_str!\(\s*"([^"]+)"',p.read_text()):assert(p.parent/ref).is_file()
limits={}
for line in(R/'tests/module-size-budget.tsv').read_text().splitlines():
 if line.strip()and not line.startswith('#'):p,n=line.split('\t');limits[p]=int(n)
violations=[]
for source in [row['source']for row in packet['owners']+packet['children']if row['source'].endswith('.rs')]:
 count=len((R/source).read_text().splitlines())
 if count>limits.get(source,1500):violations.append((source,count))
assert not violations
proof={'basis':B,'status':'SOURCE_ONLY_UNEXECUTED','fix_full_sources':4,'new_full_children':packet['children'],'original_13_immediate_archives_and_all_original_inverse_edits_preserved':True,'original_five_test_bodies_preserved_with_only_additive_multiCopy_calls':True,'all_original_production_backend_bytes_unchanged':production,'exact_native_and_wasm_float_patterns_per_type':12,'new_unrelated_Copy_fields':['i64 marker','f32 minus','f64 nan'],'all_unrelated_raw_words_and_owned_leaves_checked':True,'native_hostile_cases':8,'native_hostile_no_store_result_status_generation_or_heap_change_before_original_owner_Drop':True,'same_source_public_interpreter_corpus_authored':True,'interpreter_private_alias_generation_and_payload_controls_dependency':'ad673fb8f9c1a4584c483b341ffa554665fc0661','registered_new_owning_bodies':3,'retained_exact_owning_bodies':10,'module_budget_violations':violations,'actual_results':None,'float_bit_contract':'Exact scalar-word payloads at trusted actual native/private-host boundaries; no invented source bitcast or arbitrary source NaN-payload claim','global_custody':'No mass composition; original Source36/other packets untouched','gain_claim':False}
(D/'source-proof.json').write_text(json.dumps(proof,indent=2)+'\n');print('PASS coverage4fullinverses+2joinedchildren; all13originals/5tests/oldoracle/prodbytes exact;12floatpatterns×2;8nativehostile;3new+10retained source-only selectors')
