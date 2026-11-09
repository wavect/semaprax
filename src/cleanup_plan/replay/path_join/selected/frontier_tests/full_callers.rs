//! Genuine full callers include cold directories and complete output/budget Drop.
use super::*;
use crate::cleanup_plan::replay::primitive_fixture;
const SCALAR: &str = r#"module compact.caller;
@id("one") fn one(n:i64)->i64 {n+1}
@id("main") fn main()->i64 {let a=one(1);let b=one(a);if b==3 {7}else{0}}
"#;
const LEXICAL: &str = r#"module compact.lexical;
@id("probe") fn probe(flag:bool)->i64 {
 let key=str"x";let view=str_as_bytes(key);let mut next=0;
 while next<3 {next=next+1;if flag {continue;};if next==2 {return if byte_len(view)==1usize {7}else{0};};0}
 9
}
@id("main") fn main()->i64 {probe(false)}
"#;
fn program(s: &str) -> ResolvedProgram {
    crate::hir::resolve(&crate::check(s, "compact-owner.spx").unwrap()).unwrap()
}
#[test]
fn tiny_repeated_and_lexical_full_callers_report_actual_cold_cost_without_assuming_a_saving() {
    for source in [SCALAR, LEXICAL] {
        let program = program(source);
        let source = primitive_fixture::Context::new(&program);
        for function in &program.functions {
            let mut outcomes = Vec::new();
            for old in [true, false] {
                let ((result, paid, work), a) = measure(|| cost(&source, function, old));
                assert!(!a.invalid);
                eprintln!("incoming-frontier full {} old={old} physical={paid} work={work} allocations={} bytes={}",function.id,a.allocations,a.allocated);
                assert!(a.allocated <= paid, "complete caller physical funding");
                outcomes.push(result);
                let (_, a) = measure(|| {
                    for _ in 0..3 {
                        drop(cost(&source, function, old));
                    }
                });
                assert!(!a.invalid && a.live == 0);
                assert_eq!(a.allocated, a.freed);
            }
            assert_eq!(
                outcomes[0], outcomes[1],
                "complete immediate-old/current semantic result including six diagnostic fields"
            );
            assert!(
                outcomes[1].is_ok(),
                "fixture must reach successful complete path replay: {:?}",
                outcomes[1]
            );
        }
    }
}
#[test]
fn actual_initializer_and_lexical_hostiles_keep_immediate_old_first_complete_diagnostic() {
    for case in 0..6 {
        let mut program = program(LEXICAL);
        let index = program
            .functions
            .iter()
            .position(|f| f.id.as_str() == "probe")
            .unwrap();
        let plan = &mut program.functions[index].cleanup_plan;
        match case {
            0 => {
                plan.entry = BlockId(u32::MAX);
            }
            1 => {
                plan.lexical_control.as_mut().unwrap().loops[0].latch_entry =
                    Some(BlockId(u32::MAX));
            }
            2 => {
                plan.lexical_control.as_mut().unwrap().exits[0].resume = Some(BlockId(u32::MAX));
            }
            3 => {
                plan.lexical_control.as_mut().unwrap().exits[0].exit =
                    crate::cleanup_plan::ExitTargetId(u32::MAX);
            }
            4 => {
                plan.blocks[0].id = BlockId(u32::MAX);
            }
            _ => {
                let edges = plan
                    .blocks
                    .iter()
                    .find_map(|b| match &b.terminator {
                        CleanupTerminator::Branch(e) => Some(e.clone()),
                        _ => None,
                    })
                    .unwrap();
                plan.edges[edges[0].0 as usize].condition = EdgeCondition::Always;
                plan.edges[edges[1].0 as usize].condition = EdgeCondition::Always;
            }
        }
        let source = primitive_fixture::Context::new(&program);
        let f = &program.functions[index];
        let old = cost(&source, f, true).0;
        let new = cost(&source, f, false).0;
        assert!(old.is_err());
        assert_eq!(old, new, "exact first six-field diagnostic case {case}");
        let (_, a) = measure(|| {
            drop(cost(&source, f, true));
            drop(cost(&source, f, false));
        });
        assert!(!a.invalid && a.live == 0);
        assert_eq!(a.allocated, a.freed);
    }
}
