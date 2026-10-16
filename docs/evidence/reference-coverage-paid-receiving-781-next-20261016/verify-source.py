from pathlib import Path
import hashlib,json,subprocess
D=Path(__file__).resolve().parent;R=D.parents[2];B="9a3c4fbf969340e7ee29fd9cf931586da30cf7f2"
sha=lambda b:hashlib.sha256(b).hexdigest()
def git(*a):return subprocess.check_output(["git",*a],cwd=R)
rows=json.loads((D/"source-inverses.json").read_text())
for row in rows:
 p=row["source"];now=(R/p).read_bytes();assert sha(now)==row["current_sha256"],p
 text=now.decode()
 for e in row["edits"]:
  assert text.count(e["new"])==1,p;text=text.replace(e["new"],e["old"],1)
 assert sha(text.encode())==row["original_sha256"],p
 assert text.encode()==git("show",B+":"+p)==(R/row["archive"]).read_bytes(),p
owners=json.loads((D/"new-owners.json").read_text())
for row in owners:
 assert sha((R/row["source"]).read_bytes())==row["sha256"]
 assert row["lines"]<=1500
for p in ["tests/hir_module_boundaries.rs","tests/workspace/semantic_graph/source_inventory.rs","crates/semaprax-native-rust-interop-builder/src/implementation/tests/ledger_capacity/source_inventory.rs"]:
 s=(R/p).read_text()
 for child in owners:assert s.count(child["source"].split("src/")[1])==1
 assert len(s.splitlines())<=1500 or p=="tests/hir_module_boundaries.rs"
# Complete old receiver/insert helper remain a byte-exact prefix.
p="src/cleanup_plan/replay/structural_maps.rs"
assert (R/p).read_bytes()==git("show",B+":"+p)+b"\npub(super) mod paid_reference;\n"
for p in ["src/cleanup_plan/replay/lexical_replay.rs","src/cleanup_plan/replay/scalar_replay.rs"]:
 old=git("show",B+":"+p).decode();now=(R/p).read_text()
 assert now.replace("structural_maps::paid_reference::validate(function, budget)?;","structural_maps::validate_reference_coverage(function, budget)?;",1)==old
# No mutation to any predecessor JSON or the report/initial setup authorities.
assert not [p for p in git("diff",B,"--name-only","--diff-filter=M").decode().splitlines()if p.endswith(".json")]
for p in ["src/cleanup_plan/replay/selected_entry/paid.rs","src/cleanup_plan/replay/physical_budget.rs","src/cleanup_plan/replay/semantic_refusal.rs"]:
 assert (R/p).read_bytes()==git("show",B+":"+p),p
child=(R/"src/cleanup_plan/replay/selected_context/child.rs").read_text()
assert child.count("struct BudgetDiagnostic(crate::diagnostic::Diagnostic);")==1
paid=(R/"src/cleanup_plan/replay/structural_maps/paid_reference.rs").read_text()
assert "physical.is_none" not in paid and "budget.selected()" not in paid
assert paid.index("ArrayReceiving<'_>")<paid.index("child_reserve_array::<usize>")
assert paid.index("if !$set.contains($source)")<paid.index("NodeReceiving<'_>")<paid.index("child_reserve_btree_node")
proof={"status":"SOURCE_ONLY_PASS_NO_RUST_EXECUTION","full_inverse_rows":len(rows),"new_source_owners":len(owners),"complete_old_reference_receiver_and_insert_status_byteexact":True,"same_ledger_per_actual_call":True,"all_predecessor_JSON_unchanged":True,"new_owning_bodies":2,"whole781":"OPEN_remaining_dynamic_families","authority":"FINAL_JOIN_MUST_REGENERATE"}
(D/"source-proof.json").write_text(json.dumps(proof,indent=2)+"\n");print(json.dumps(proof))
