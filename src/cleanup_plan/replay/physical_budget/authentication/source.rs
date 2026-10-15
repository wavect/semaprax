//! Full immediate predecessor projections before all protected historical hops.
use sha2::{Digest,Sha256};
const DATA: &str=include_str!(concat!(env!("CARGO_MANIFEST_DIR"),"/docs/evidence/held-auth-839/source-inverses.json"));
fn hex(source: &str) -> String { format!("{:x}",crate::digest_hex::LowerHex(Sha256::digest(source.as_bytes()))) }
pub(in crate::cleanup_plan::replay) fn previous(path: &str,current: &str) -> String {
    let staged=crate::source_reflows_merge::held(path,current);
    let current=staged.as_str();
    let rows:serde_json::Value=serde_json::from_str(DATA).expect("fixed full held-auth source inverses");
    let rows=rows.as_array().unwrap();
    let matches:Vec<_>=rows.iter().filter(|row|row["path"]==path).collect();
    assert!(matches.len()<=1,"unique whole-source held-auth inverse");
    let Some(row)=matches.first() else { return current.to_owned(); };
    assert_eq!(hex(current),row["after_sha256"]);
    assert_eq!(current,row["after"].as_str().unwrap(),"complete actual current input");
    let old=row["before"].as_str().expect("existing predecessor owner");
    assert_eq!(hex(old),row["before_sha256"]);
    old.to_owned()
}
pub(in crate::cleanup_plan::replay) fn previous_include(path: &str,current: &str) -> String {
    let source=match path {
        "../../canonical_shared.rs"=>"src/cleanup_plan/replay/factored/canonical_shared.rs",
        "../scratch_tests.rs"=>"src/cleanup_plan/replay/factored/canonical_shared/scratch_tests.rs",
        "source.rs"=>"src/cleanup_plan/replay/factored/canonical_shared/ordered_weights/source.rs",
        "../../../../../../tests/hir_module_boundaries.rs"=>"tests/hir_module_boundaries.rs",
        "../../../../../../tests/workspace/semantic_graph/source_inventory.rs"=>"tests/workspace/semantic_graph/source_inventory.rs",
        "../../../../../../docs/RFC-0003-CLEANUP-AND-RESOURCE-ABI.md"=>"docs/RFC-0003-CLEANUP-AND-RESOURCE-ABI.md",
        _=>return current.to_owned(),
    };
    previous(source,current)
}
#[test]
fn held_full_current_sources_and_immediate_inverses_authenticate_every_byte() {
    let rows:serde_json::Value=serde_json::from_str(DATA).unwrap();
    let sources=[
        ("crates/semaprax-native-rust-interop-builder/src/implementation/tests/ledger_capacity/source_inventory.rs",include_str!(concat!(env!("CARGO_MANIFEST_DIR"),"/crates/semaprax-native-rust-interop-builder/src/implementation/tests/ledger_capacity/source_inventory.rs"))),
        ("docs/RFC-0003-CLEANUP-AND-RESOURCE-ABI.md",include_str!(concat!(env!("CARGO_MANIFEST_DIR"),"/docs/RFC-0003-CLEANUP-AND-RESOURCE-ABI.md"))),
        ("src/cleanup_plan/replay/allocation/oracle.rs",include_str!(concat!(env!("CARGO_MANIFEST_DIR"),"/src/cleanup_plan/replay/allocation/oracle.rs"))),
        ("src/cleanup_plan/replay/allocation/oracle/zero_tests.rs",include_str!(concat!(env!("CARGO_MANIFEST_DIR"),"/src/cleanup_plan/replay/allocation/oracle/zero_tests.rs"))),
        ("src/cleanup_plan/replay/factored/canonical_shared.rs",include_str!(concat!(env!("CARGO_MANIFEST_DIR"),"/src/cleanup_plan/replay/factored/canonical_shared.rs"))),
        ("src/cleanup_plan/replay/factored/canonical_shared/before_held.rs",include_str!(concat!(env!("CARGO_MANIFEST_DIR"),"/src/cleanup_plan/replay/factored/canonical_shared/before_held.rs"))),
        ("src/cleanup_plan/replay/factored/canonical_shared/before_held_memo.rs",include_str!(concat!(env!("CARGO_MANIFEST_DIR"),"/src/cleanup_plan/replay/factored/canonical_shared/before_held_memo.rs"))),
        ("src/cleanup_plan/replay/factored/canonical_shared/before_held_retained.rs",include_str!(concat!(env!("CARGO_MANIFEST_DIR"),"/src/cleanup_plan/replay/factored/canonical_shared/before_held_retained.rs"))),
        ("src/cleanup_plan/replay/factored/canonical_shared/before_ordered.rs",include_str!(concat!(env!("CARGO_MANIFEST_DIR"),"/src/cleanup_plan/replay/factored/canonical_shared/before_ordered.rs"))),
        ("src/cleanup_plan/replay/factored/canonical_shared/held_auth_tests.rs",include_str!(concat!(env!("CARGO_MANIFEST_DIR"),"/src/cleanup_plan/replay/factored/canonical_shared/held_auth_tests.rs"))),
        ("src/cleanup_plan/replay/factored/canonical_shared/memo/retained.rs",include_str!(concat!(env!("CARGO_MANIFEST_DIR"),"/src/cleanup_plan/replay/factored/canonical_shared/memo/retained.rs"))),
        ("src/cleanup_plan/replay/factored/canonical_shared/memo/retained/held_tests.rs",include_str!(concat!(env!("CARGO_MANIFEST_DIR"),"/src/cleanup_plan/replay/factored/canonical_shared/memo/retained/held_tests.rs"))),
        ("src/cleanup_plan/replay/factored/canonical_shared/ordered_weights/amortized_source.rs",include_str!(concat!(env!("CARGO_MANIFEST_DIR"),"/src/cleanup_plan/replay/factored/canonical_shared/ordered_weights/amortized_source.rs"))),
        ("src/cleanup_plan/replay/factored/canonical_shared/ordered_weights/source.rs",include_str!(concat!(env!("CARGO_MANIFEST_DIR"),"/src/cleanup_plan/replay/factored/canonical_shared/ordered_weights/source.rs"))),
        ("src/cleanup_plan/replay/factored/canonical_shared/scratch_tests.rs",include_str!(concat!(env!("CARGO_MANIFEST_DIR"),"/src/cleanup_plan/replay/factored/canonical_shared/scratch_tests.rs"))),
        ("src/cleanup_plan/replay/live_oracle/source_custody/current_view.rs",include_str!(concat!(env!("CARGO_MANIFEST_DIR"),"/src/cleanup_plan/replay/live_oracle/source_custody/current_view.rs"))),
        ("src/cleanup_plan/replay/physical_budget.rs",include_str!(concat!(env!("CARGO_MANIFEST_DIR"),"/src/cleanup_plan/replay/physical_budget.rs"))),
        ("src/cleanup_plan/replay/physical_budget/authentication.rs",include_str!(concat!(env!("CARGO_MANIFEST_DIR"),"/src/cleanup_plan/replay/physical_budget/authentication.rs"))),
        ("src/cleanup_plan/replay/physical_budget/authentication/source.rs",include_str!(concat!(env!("CARGO_MANIFEST_DIR"),"/src/cleanup_plan/replay/physical_budget/authentication/source.rs"))),
        ("src/cleanup_plan/replay/physical_budget/authentication/tests.rs",include_str!(concat!(env!("CARGO_MANIFEST_DIR"),"/src/cleanup_plan/replay/physical_budget/authentication/tests.rs"))),
        ("src/cleanup_plan/replay/physical_storage.rs",include_str!(concat!(env!("CARGO_MANIFEST_DIR"),"/src/cleanup_plan/replay/physical_storage.rs"))),
        ("src/cleanup_plan/replay/physical_storage/zero_tests.rs",include_str!(concat!(env!("CARGO_MANIFEST_DIR"),"/src/cleanup_plan/replay/physical_storage/zero_tests.rs"))),
        ("tests/documentation.rs",include_str!(concat!(env!("CARGO_MANIFEST_DIR"),"/tests/documentation.rs"))),
        ("tests/documentation/held_auth_sources.rs",include_str!(concat!(env!("CARGO_MANIFEST_DIR"),"/tests/documentation/held_auth_sources.rs"))),
        ("tests/hir_module_boundaries.rs",include_str!(concat!(env!("CARGO_MANIFEST_DIR"),"/tests/hir_module_boundaries.rs"))),
        ("tests/workspace/semantic_graph/source_inventory.rs",include_str!(concat!(env!("CARGO_MANIFEST_DIR"),"/tests/workspace/semantic_graph/source_inventory.rs"))),
    ];
    assert_eq!(sources.len(),rows.as_array().unwrap().len());
    for (path,raw_current) in sources {
        let staged=crate::source_reflows_merge::held(path,raw_current);
        let current=staged.as_str();
        let found:Vec<_>=rows.as_array().unwrap().iter().filter(|row|row["path"]==path).collect();
        assert_eq!(found.len(),1);let row=found[0];
        assert_eq!(hex(current),row["after_sha256"]);
        assert_eq!(current,row["after"].as_str().unwrap());
        if let Some(before)=row["before"].as_str() {
            assert_eq!(hex(before),row["before_sha256"]);
            let inverse=previous(path,raw_current);assert_eq!(inverse,before);
            // Full-file forward and inverse replace every byte exactly once.
            assert_eq!(current.replacen(current,&inverse,1),before);
            assert_eq!(before.replacen(before,current,1),current);
        } else { assert!(row["before_sha256"].is_null()); }
    }
}
#[test]
fn held_complete_old_receivers_and_original_fee_body_are_exact_independent_sources() {
    let old=include_str!(concat!(env!("CARGO_MANIFEST_DIR"),"/docs/evidence/held-auth-839/src__cleanup_plan__replay__factored__canonical_shared__memo__retained.rs.before.txt"));
    assert_eq!(old,include_str!("../../factored/canonical_shared/before_held_retained.rs"));
    let ordinary=include_str!("../../factored/canonical_shared/memo.rs");
    assert_eq!(ordinary,include_str!(concat!(env!("CARGO_MANIFEST_DIR"),"/docs/evidence/held-auth-839/src__cleanup_plan__replay__factored__canonical_shared__memo.rs.before.txt")));
    let adapted=ordinary.replace("use super::{Diagnostic, ReplayBudget, ResolvedFunction};","use super::super::{Diagnostic, ReplayBudget, ResolvedFunction};")
        .replace("\n#[cfg(test)]\nmod tests;\n","\n")
        .replace("mod retained;","#[path = \"before_held_retained.rs\"]\nmod retained;")
        .replace("pub(super) use retained::RetainedMemo;","pub(in crate::cleanup_plan::replay::factored::canonical_shared) use retained::RetainedMemo;");
    assert_eq!(adapted,include_str!("../../factored/canonical_shared/before_held_memo.rs"));
    assert_eq!(include_str!("../../factored/canonical_shared/before_ordered.rs")
        .replace("use super::{app_census, epsilon_alias, successor_targets, tests};\nuse super::before_held::memo;","use super::{app_census, epsilon_alias, memo, successor_targets, tests};"),
        include_str!(concat!(env!("CARGO_MANIFEST_DIR"),"/docs/evidence/held-auth-839/src__cleanup_plan__replay__factored__canonical_shared__before_ordered.rs.before.txt")));
    let old_tests=include_str!(concat!(env!("CARGO_MANIFEST_DIR"),"/docs/evidence/held-auth-839/src__cleanup_plan__replay__factored__canonical_shared__scratch_tests.rs.before.txt"));
    let marker="#[test]\nfn retained_warm_authentication_pays_the_real_result_at_exact_and_one_short_frontiers()";
    let complete_old_body=&old_tests[old_tests.find(marker).unwrap()..];
    assert!(include_str!("../../factored/canonical_shared/scratch_tests.rs").contains(complete_old_body));
    let retained=include_str!("../../factored/canonical_shared/memo/retained.rs");
    let warm=retained.split_once("        if self.controls_paid {").unwrap().1.split_once("        // The ordinary function").unwrap().0;
    assert!(!warm.contains(".authenticate(") && !warm.contains("reserve_bytes(") && !warm.contains("prepare_builder("));
    assert!(warm.contains("authenticate_zero_into("));
    // Original receiving authorization remains physically present on cold activation.
    assert!(retained.contains("if let Err(error) = self.memo.authenticate(function, budget)"));
}
