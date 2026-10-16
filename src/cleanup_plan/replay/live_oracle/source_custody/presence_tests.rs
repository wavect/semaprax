//! Explicit current CI + immutable 512-depth source-custody chain.
#[test]
fn merged_current_ci_reflow_and_presence512_complete_inputs_remain_exact() {
    crate::source_reflows_merge::main7f8_presence::controls();
    let mut walk = ci_str!("../../../../hir/lexical_exit/walk.rs").to_owned();
    super::presence::restore("src/hir/lexical_exit/walk.rs", &mut walk);
    let mut owner = ci_str!("../../../../hir/lexical_exit/owner_funded.rs").to_owned();
    super::presence::restore("src/hir/lexical_exit/owner_funded.rs", &mut owner);
}
