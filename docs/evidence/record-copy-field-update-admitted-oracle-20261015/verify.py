from pathlib import Path
import json,subprocess,hashlib,re
R=Path(__file__).resolve().parents[3];D=Path(__file__).resolve().parent;H='35cc6325790e00f7bcbd0c0bedf51518939f9b47'
g=lambda *a:subprocess.check_output(['git',*a],cwd=R);sha=lambda b:hashlib.sha256(b).hexdigest();rows=json.loads((D/'source-inverses.json').read_text());assert len(rows)==2
assert json.loads((D/'current-to-35cc.json').read_text())=={'fixture-current':rows}
for row in rows:
 b=(R/row['source']).read_bytes();assert sha(b)==row['current_sha256']
 for e in row['edits']:
  n=e['new'].encode();assert b.count(n)==1;b=b.replace(n,e['old'].encode(),1)
 assert b==g('show',H+':'+row['source'])==(R/row['archive']).read_bytes() and sha(b)==row['original_sha256']
old=(D/'before/tests/support/record_copy_field_update.rs').read_text();new=(R/'tests/support/record_copy_field_update.rs').read_text()
constant=lambda s,n:re.search(r'(?:pub\(crate\) )?const '+n+r': &str = r#"(.*?)"#;',s,re.S)[1]
assert constant(old,'OLD_ROW_ORACLE')==constant(new,'OLD_ROW_ORACLE')
for name in ['cases','declaration','source','failure','evaluation_order_source','empty_failure','multi_source','boundary_source']:
 pat=r'pub\(crate\) fn '+name+r'\(.*?\n\}\n';assert re.search(pat,new,re.S)[0]==re.search(pat,old,re.S)[0],name
admitted=constant(new,'ADMITTED_ROW_ORACLE');assert 'vec_update_field' not in admitted and admitted.count('vec_clone_at<Row>')==admitted.count('vec_replace<Row>')==1
assert 'string_from_str(string_as_str(title))' in admitted and 'bytes_copy(bytes_as_slice(payload))' in admitted
def oracle(s,which):
 decl=re.search(r'pub\(crate\) fn declaration.*?r#"(.*?)"#',s,re.S)[1].replace('{ty}','i64').replace('{{','{').replace('}}','}')
 body=re.search(r'pub\(crate\) fn oracle_source\(\).*?r#"(.*?)"#',s,re.S)[1]
 return body.replace('{'+which+'}',constant(s,which)).replace('{}',decl,1).replace('{{','{').replace('}}','}')
legacy=oracle(old,'OLD_ROW_ORACLE');current=oracle(new,'ADMITTED_ROW_ORACLE');assert current.replace(admitted,constant(new,'OLD_ROW_ORACLE'),1)==legacy
needle='bytes_as_slice(old.payload)';at=legacy.index(needle);assert len(legacy[:at].encode())==393 and len(needle.encode())==27 and legacy[:at].count('\n')+1==10
proof={'mode':'SOURCE_ONLY_UNEXECUTED_FIXTURE_CORRECTION','basis':H,'complete_current_to_35cc_rows':2,'original_OLD_ROW_ORACLE_bytes_exact':True,'original_positive_corpus_builders_exact':8,'negative_full_program_equals_original':True,'actual_prior_T266_span':[393,420,10,102],'all_six_negative_fields_asserted':True,'new_oracle_independent_clone_replace':True,'no_production_change':True,'new_selectors':0,'compiled_input_view_bytes':(D/'current-to-35cc.json').stat().st_size}
(D/'source-proof.json').write_text(json.dumps(proof,indent=2)+'\n');print(json.dumps(proof))
