from pathlib import Path
import json,hashlib,difflib
R=Path.cwd();S=Path('/Volumes/Semaprax/tmp/ci-38383508340');O=S/'byte-error-9bb8';E=R/'docs/evidence/compact-byte-errors-20261021';ns={};exec((S/'source-reflow-tools.py').read_text().split('def wrap():')[0],ns);H=lambda b:hashlib.sha256(b).hexdigest()
def function(s,name):
 ts=list(ns['tokens'](s));found=[]
 for i,t in enumerate(ts[:-2]):
  if t[0]!='fn' or ts[i+1][0]!=name:continue
  j=next(j for j in range(i+1,len(ts))if ts[j][0]=='{');depth=1;k=j+1
  while depth:depth+=(ts[k][0]=='{')-(ts[k][0]=='}');k+=1
  a=s.rfind('\n',0,t[1])+1;found.append((a,ts[k-1][2]))
 assert len(found)==1,name
 return found[0]
for rel,names in [('src/cleanup_plan/replay/branch_clone.rs',[('inline_controls','compact_roles'),('controls',None)]),('src/cleanup_plan/replay/branch_clone/direct.rs',[('inline_controls','compact_roles'),('small_inline_controls','compact_small_roles')])]:
 p=R/rel;s=p.read_text();before=(O/'before'/rel).read_text();extra=[]
 for name,compact in names:
  a,b=function(s,name);x,y=function(before,name)
  if compact:extra.append(s[a:b].replace('fn '+name+'(', 'fn '+compact+'(',1).replace('fn '+name+'<','fn '+compact+'<',1))
  s=s[:a]+before[x:y]+s[b:]
 # New banks live alongside the entire unchanged original banks.
 p.write_text(s+'\n// Current compact producer receiving roles; original banks above fund retained controls.\n'+'\n\n'.join(extra)+'\n')
# Only production call identifiers change, never any embedded source literal.
rel='src/cleanup_plan/replay.rs';p=R/rel;old=p.read_text();q=O/'before'/rel;q.parent.mkdir(parents=True,exist_ok=True);q.write_text(old);edits=[]
for t,a,b in ns['tokens'](old):
 if t=='inline_controls' and (old[max(0,a-40):a].endswith('branch_clone::') or old[max(0,a-40):a].endswith('branch_clone::direct::')):edits.append((a,b,'compact_roles'))
assert len(edits)==5
new=old;indexed=[];delta=0
for a,b,replacement in edits:
 at=a+delta;indexed.append([at,replacement,old[a:b]]);delta+=len(replacement)-(b-a)
for a,b,replacement in reversed(edits):new=new[:a]+replacement+new[b:]
assert len(new.splitlines())==len(old.splitlines());p.write_text(new)
# Compact exact line-context inverses preserve all immutable predecessor bytes.
a=old.splitlines(True);b=new.splitlines(True);groups=list(difflib.SequenceMatcher(None,a,b,autojunk=False).get_grouped_opcodes(2));inverse=[];restored=new
for group in reversed(groups):
 i1,i2=group[0][1],group[-1][2];j1,j2=group[0][3],group[-1][4];before=''.join(a[i1:i2]);after=''.join(b[j1:j2]);assert restored.count(after)==1;restored=restored.replace(after,before,1);inverse.append([after,before])
assert restored==old
row=dict(source=rel,length=len(new.encode()),sha256=H(new.encode()),checkpoint_length=len(old.encode()),checkpoint_sha256=H(old.encode()),inverse=inverse,indexed_inverse=indexed)
raw=(json.dumps(row,separators=(',',':'))+'\n').encode();(E/'replay-successor.json').write_bytes(raw)
rows=json.loads((O/'selected.json').read_text());rows.append({'source':rel})
# Preserve and authenticate the full previous CI row before extending its exact inverse.
rel='src/source_reflows/ci_inputs.rs';p=R/rel;q=O/'before'/rel;q.parent.mkdir(parents=True,exist_ok=True);q.write_bytes(p.read_bytes());s=p.read_text();needle='    rows\n}\npub(super) fn prior';assert s.count(needle)==1
addition=f'''    let successor = reader::read(
        &std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("docs/evidence/compact-byte-errors-20261021/replay-successor.json"),
        {len(raw)}, "{H(raw)}", 2_097_152,
    ).unwrap();
    let successor: serde_json::Value = serde_json::from_str(&successor).unwrap();
    let row = &mut rows[0];
    assert_eq!(row["source"], successor["source"]);
    assert_eq!(row["current_length"], successor["checkpoint_length"]);
    assert_eq!(row["current_sha256"], successor["checkpoint_sha256"]);
    let current = compiled(row["source"].as_str().unwrap()).unwrap().1;
    assert_eq!(current.len() as u64, successor["length"]);
    assert_eq!(digest(current), successor["sha256"]);
    let mut checkpoint = current.to_owned();
    for edit in successor["indexed_inverse"].as_array().unwrap().iter().rev() {{
        let at = usize::try_from(edit[0].as_u64().unwrap()).unwrap();
        let after = edit[1].as_str().unwrap();
        assert_eq!(&checkpoint[at..at + after.len()], after);
        checkpoint.replace_range(at..at + after.len(), edit[2].as_str().unwrap());
    }}
    assert_eq!(checkpoint.len() as u64, row["current_length"]);
    assert_eq!(digest(&checkpoint), row["current_sha256"]);
    row["inverse"].as_array_mut().unwrap().extend(successor["indexed_inverse"].as_array().unwrap().iter().cloned());
    row["current_length"] = successor["length"].clone();
    row["current_sha256"] = successor["sha256"].clone();
'''
s=s.replace(needle,addition+needle,1);p.write_text(s);rows.append({'source':rel})
rel='src/source_reflows_merge/runtime_json.rs';p=R/rel;q=O/'before'/rel;q.parent.mkdir(parents=True,exist_ok=True);q.write_bytes(p.read_bytes());s=p.read_text();needle='    let successor: serde_json::Value = serde_json::from_str(&successor).unwrap();';assert s.count(needle)==1;s=s.replace(needle,needle+f'''
    let replay_successor = main043_raw_file(
        &std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("docs/evidence/compact-byte-errors-20261021/replay-successor.json"),
        {len(raw)}, "{H(raw)}",
    ).unwrap();
    let replay_successor: serde_json::Value = serde_json::from_str(&replay_successor).unwrap();
    let successors = [successor, replay_successor];''',1);needle='        if successor["source"] == source {';assert s.count(needle)==1;s=s.replace(needle,'        for successor in successors.iter().filter(|row| row["source"] == source) {',1);p.write_text(s);rows.append({'source':rel});(O/'selected.json').write_text(json.dumps(rows))
(O/'replay-stage-proof.json').write_text(json.dumps(dict(status='COMPLETE_NEW_ROOT_TO_PREVIOUS_ROOT_BYTE_INVERSE_PASS',row=row,unchanged_root_line_count=len(new.splitlines()),old_fee_bank_bodies_retained_exact=True,source_literals_not_rewritten=True),indent=2)+'\n')
print('Preserved original fee banks; bound all five production payments to compact banks; exact Root inverse bytes',len(raw))
