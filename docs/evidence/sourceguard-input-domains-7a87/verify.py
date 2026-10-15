"""Finite source-only guard repair. No Rust/CLI/test/process execution."""
from pathlib import Path
import ast,hashlib,json,re,subprocess,sys
R=Path.cwd();D=Path(__file__).resolve().parent;sys.path.insert(0,str(D));from source_inputs import Inputs
from help_literals import catalog,verify_frozen
BASE='d20af8ebf634b63e97fb84fbc5e9fe8242879720';sha=lambda b:hashlib.sha256(b).hexdigest();helper=Inputs(R);neg=0
rows=json.loads((D/'current-to-d20.json').read_text())
for row in rows:
 path=row['source'];raw=(R/path).read_text();old=helper.project(rows,path,raw)
 assert old==(R/row['archive']).read_text()==subprocess.check_output(['git','show',BASE+':'+path],text=True)
 for bad in [raw+' ', ' '+raw]:
  try:helper.project(rows,path,bad)
  except AssertionError:neg+=1
  else:raise AssertionError('changed complete input accepted')
p='src/bounded_output/classifier_string.rs';raw=(R/p).read_text();prior=helper.call('source36_classifier_input',p,raw);classifier=json.loads((D/'classifier-input.json').read_text())[0];assert prior==(R/classifier['archive']).read_text()==subprocess.check_output(['git','show',classifier['checkpoint']+':'+p],text=True);assert subprocess.run(['git','merge-base','--is-ancestor',classifier['checkpoint'],BASE],capture_output=True).returncode==0
for bad in [raw+' ', ' '+raw]:
 try:helper.call('source36_classifier_input',p,bad)
 except AssertionError:neg+=1
 else:raise AssertionError('changed classifier accepted')
a='src/cleanup_plan/replay/live_oracle/source_custody/a.rs';old=(D/'before'/a).read_text();new=(R/a).read_text();x='include_str!("../../../../bounded_output/classifier_string.rs").to_owned()';y='crate::source_reflows_merge::source36_classifier_input("src/bounded_output/classifier_string.rs", include_str!("../../../../bounded_output/classifier_string.rs"))';assert new.count(y)==1 and new.replace(y,x,1)==old;assert len(new.splitlines())==len(old.splitlines())==1500
# Execute the original unchanged literal decoder and inverse function AST only.
oldrunner=R/'docs/evidence/source36-after-next-838-839-20261015/verify_source36.py';assert oldrunner.read_bytes()==subprocess.check_output(['git','show',BASE+':'+str(oldrunner.relative_to(R))]);nodes=[x for x in ast.parse(oldrunner.read_text()).body if isinstance(x,ast.FunctionDef) and x.name in ['string_at','apply']];env={'re':re,'json':json,'root':R};exec(compile(ast.Module(body=nodes,type_ignores=[]),'original-unchanged-inverse-functions','exec'),env)
p=R/a;s=p.read_text();pattern=r'let mut current =\s*(?:include_str!\("([^"]+)"\)\.to_owned\(\)|super::current_view::project\("([^"]+)", include_str!\("([^"]+)"\)\)|crate::source_reflows_merge::source36_classifier_input\("([^"]+)", include_str!\("([^"]+)"\)\))\s*;';starts=list(re.finditer(pattern,s));assert len(starts)==17;sources=[]
for k,m in enumerate(starts):
 end=starts[k+1].start() if k+1<len(starts) else len(s);section=s[m.end():end];path=(p.parent/(m[1] or m[3] or m[5])).resolve();current=path.read_text()
 if m[2]:current=helper.source36(m[2],current)
 if m[4]:current=helper.call('source36_classifier_input',m[4],current)
 current=env['apply'](current,section,p);archives=re.findall(r'include_str!\(\s*"([^"]+\.before)"\s*\)',section);assert archives;assert current==(p.parent/archives[-1]).resolve().read_text();sources.append(str(path.relative_to(R)))
# Entire old help body survives; only a new complete-input transport is inserted.
p='tests/offline_package/resolver_cli.rs';old=(D/'before'/p).read_text();new=(R/p).read_text();start=new.index('    // Authenticate the COMPLETE current catalog');end=new.index('    for (current_line, restored) in RESTORED',start);assert new[:start]+new[end:]==old
actual=catalog((R/'src/cli/help.rs').read_text());assert actual==(D/'current-help-all.txt').read_text();frozen=(D/'frozen-help-input.txt').read_text();block=new[start:end];assertions=re.search(r'for \(current_line, prior_line\) in (\[.*?\]) \{',block,re.S);assert assertions
from help_literals import value
edits,_=value(assertions[1],0);assert len(edits)==7
for current_line,prior_line in edits:assert actual.count(current_line)==1;actual=actual.replace(current_line,prior_line,1)
assert actual==frozen;help_pins=verify_frozen(old,frozen);assert len(help_pins)==11
# SourceB and the two historical constructor/ownership tail owners are untouched.
unchanged=['src/cleanup_plan/replay/live_oracle/source_custody/b.rs','src/cleanup_plan/replay/paid_type_facts/inventory_encoded/tests/constructors.rs','src/hir/validation/canonical_loans/ownership_tests.rs','src/cli/help.rs','tests/hir_module_boundaries.rs','tests/workspace/semantic_graph/source_inventory.rs','crates/semaprax-native-rust-interop-builder/src/implementation/tests/ledger_capacity/source_inventory.rs']
for p in unchanged:assert (R/p).read_bytes()==subprocess.check_output(['git','show',BASE+':'+p])
assert all(len((R/row['source']).read_text().splitlines())<=1500 for row in rows)
changes=subprocess.check_output(['git','diff','--name-status',BASE,'--','docs/evidence'],text=True).splitlines();assert all(x.startswith('A\tdocs/evidence/sourceguard-input-domains-7a87/') for x in changes),changes
result={'state':'SOURCE_ONLY_NO_RUST_EXECUTION','base':BASE,'complete_immediate_rows':3,'whole_classifier_input_checkpoint':classifier['checkpoint'],'all17SourceA_original_full_inverses_equal':True,'SourceA_actual_prefixes_used':True,'SourceA_lines':1500,'SourceB_and_tail_owner_bytes_unchanged':True,'original_help_normalizations':47,'positive_current_help_usages':7,'original_whole_help_pins':help_pins,'tamper_refusals':neg,'Python_projection_cache_bytes':0,'new_test_bodies':0,'runtime_or_gain_claim':False}
(D/'source-proof.json').write_text(json.dumps(result,indent=2)+'\n');print(json.dumps(result,indent=2))
