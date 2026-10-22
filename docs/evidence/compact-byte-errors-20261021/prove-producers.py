from pathlib import Path
import hashlib,json,re
R=Path.cwd();S=Path('/Volumes/Semaprax/tmp/ci-38383508340');O=S/'byte-error-9bb8';ns={};exec((S/'source-reflow-tools.py').read_text().split('def wrap():')[0],ns);old=(O/'physical_budget-before.rs').read_text();new=(R/'src/cleanup_plan/replay/physical_budget.rs').read_text();root=(R/'src/cleanup_plan/replay.rs').read_text();H=lambda b:hashlib.sha256(b).hexdigest()
def functions(s,name):
 ts=list(ns['tokens'](s));out=[]
 for i,t in enumerate(ts[:-2]):
  if t[0]!='fn' or ts[i+1][0]!=name:continue
  j=next(j for j in range(i+1,len(ts))if ts[j][0]=='{');depth=1;k=j+1
  while depth:
   depth+=(ts[k][0]=='{')-(ts[k][0]=='}');k+=1
  out.append(s[t[1]:ts[k-1][2]])
 return out
def normalized(s):
 raw=list(ns['tokens'](s));remove=[]
 for i,t in enumerate(raw[:-1]):
  if t[0]!='budget_refusal' or raw[i+1][0]!='(':continue
  depth=1;j=i+2
  while depth:
   depth+=(raw[j][0]=='(')-(raw[j][0]==')');j+=1
  # Strings are lexical gaps: only a comma followed by whitespace is optional.
  if raw[j-2][0]==',' and not s[raw[j-2][2]:raw[j-1][1]].strip():remove.append(raw[j-2][1])
 for i in reversed(remove):s=s[:i]+s[i+1:]
 ts=[t[0]for t in ns['tokens'](s)];out=[];i=0
 while i<len(ts):
  if ts[i:i+4]==['.','into','(',')']:i+=4
  else:out.append(ts[i]);i+=1
 return out

pairs=[]
for current,previous,source in [('reserve_compact_bytes','reserve_bytes',old),('reserve_compact_bytes_without_merge_census','reserve_bytes_without_merge_census',old),('reserve_compact_clone','reserve_clone',old),('reserve_compact_array','reserve_array',old),('charge_compact_skeleton','charge_skeleton',root)]:
 before=functions(source,previous);after=functions(new,current);assert len(before)==len(after)>0
 for a,b in zip(before,after):
  transformed=b.replace('reserve_compact_bytes_without_merge_census','reserve_bytes_without_merge_census').replace('reserve_compact_bytes','reserve_bytes').replace('reserve_compact_clone','reserve_clone').replace('reserve_compact_array','reserve_array').replace('charge_compact_skeleton','charge_skeleton').replace('ByteFailure','Diagnostic').replace('compact_refusal','budget_refusal')
  assert normalized(a)==normalized(transformed),current
  strings=lambda s:re.findall(r'"(?:[^"\\]|\\.)*"',s)
  assert strings(a)==strings(b),current
  pairs.append(dict(original=previous,compact=current,all_tokens_equal_after_only_private_ABI_changes_and_optional_refusal_call_trailing_commas=True,all_complete_string_literals_exact=True,original_body_sha256=H(a.encode()),compact_body_sha256=H(b.encode())))
# The public owned-message constructor moves to the semantic boundary. The
# private refusal body retains every other token and every complete literal.
a=functions(old,'budget_refusal')[0];b=functions(new,'compact_refusal')[0]
transformed=b.replace('compact_refusal','budget_refusal').replace('-> ByteFailure','-> Diagnostic')
for value in ['String::new()', 'text']:
 pattern=r'ByteFailure\s*\{\s*code:\s*"SPX-H006",\s*message:\s*std::borrow::Cow::Owned\('+re.escape(value)+r'\),?\s*\}'
 transformed,count=re.subn(pattern,lambda m:'Diagnostic::io("SPX-H006", '+value+')',transformed)
 assert count==1,value
assert normalized(a)==normalized(transformed)
assert re.findall(r'"(?:[^"\\]|\\.)*"',a)==re.findall(r'"(?:[^"\\]|\\.)*"',b)
pairs.append(dict(original='budget_refusal',compact='compact_refusal',all_nonconstructor_tokens_and_all_complete_literals_exact=True,owned_message_constructor_relocated_to_the_existing_public_diagnostic_boundary=True,original_body_sha256=H(a.encode()),compact_body_sha256=H(b.encode())))
a=old.index('\nimpl ReplayBudget {');b=new.index('\n// Compact producers are selected');retained=new[new.index('\nimpl ReplayBudget {'):b];assert retained.strip()==old[a:].strip()
proof=dict(status='COMPLETE_PRODUCER_BODY_PROTOCOL_EQUIVALENCE_PASS',source='src/cleanup_plan/replay/physical_budget.rs',current_source_sha256=H(new.encode()),whole_original_impl_retained_verbatim=True,original_whole_source_sha256=H(old.encode()),root_work_source_sha256=H(root.encode()),pairs=pairs,normalization='Private method names and result error type, explicit error-carrier conversion, and rustfmt optional trailing comma only at budget_refusal calls; every other token and every complete string literal is compared.',limit='Structural producer proof and real public-Diagnostic carrier tests; actual branch-copy budget tests still require the full compiler test harness.')
(O/'producer-proof.json').write_text(json.dumps(proof,indent=2)+'\n');print('All',len(pairs),'original and compact producer bodies preserve their complete token and string-literal protocols.')
cp=json.loads((O/'carrier-source-proof.json').read_text());carrier=new[new.index('// Byte-payment results'):new.index('\nimpl ReplayBudget {')];tests=new[new.index('#[cfg(test)]\nmod byte_failure_tests'):];assert H(carrier.encode())==cp['carrier_sha256']and H(tests.encode())==cp['tests_sha256'];cp.update(final_source_sha256=H(new.encode()),compiled_carrier_and_all_three_test_bodies_remain_exact=True);(O/'carrier-source-proof.json').write_text(json.dumps(cp,indent=2)+'\n')
