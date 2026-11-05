//! Full predecessor comparisons across ownership, residual and status carriers.
use super::*;
use crate::bounded_output::with_limit_usage_refusal;

const SOURCE: &str = r#"
module history.owned;
@id("choose") fn choose(left:own Bytes,right:own Bytes,gate:bool)->Bytes {
    if gate {left}else{right}
}
@id("flow") fn flow(value:own Bytes,gate:bool)->Bytes {
    let first=value;
    let second=bytes_zeroed(2usize);
    choose(first,second,gate)
}
@id("renew") fn renew(gate:bool)->usize {
    let mut words=vec_with_capacity<string>(3usize);
    words=vec_push<string>(words,"ab");
    if gate {words=vec_push<string>(words,"cd");}
    vec_len<string>(words)
}
@id("owned_result") fn owned_result(value:own Result<Bytes,Bytes>)->Result<Bytes,Bytes> {
    let payload=value?;
    Result<Bytes,Bytes>::Ok {value:payload}
}
@id("copy_result") fn copy_result(value:own Result<i64,Bytes>)->Result<i64,Bytes> {
    let payload=value?;
    Result<i64,Bytes>::Ok {value:payload}
}
@id("copy_option") fn copy_option(value:Option<i64>)->Option<i64> {
    let payload=value?;
    Option<i64>::Some {value:payload}
}
@id("owned_option") fn owned_option(value:own Option<Bytes>)->Option<Bytes> {
    let payload=value?;
    Option<Bytes>::Some {value:payload}
}
@id("scalar") fn scalar(value:i64,gate:bool)->i64 {
    let selected=match value {0=>1,1=>2,_=>3,};
    if gate {selected+1}else{selected+2}
}
@id("main") fn main()->i64 {0}
"#;
// Keep the complete original source above as a separately checked refusal.
// This declaration has never been admitted; all other owned/residual/scalar
// functions enter both real history producers without changing their bytes.
const UNSUPPORTED_OWNED_OPTION: &str = r#"@id("owned_option") fn owned_option(value:own Option<Bytes>)->Option<Bytes> {
    let payload=value?;
    Option<Bytes>::Some {value:payload}
}
"#;

fn admitted_source() -> String {
    assert_eq!(SOURCE.matches(UNSUPPORTED_OWNED_OPTION).count(), 1);
    let blank = UNSUPPORTED_OWNED_OPTION
        .chars()
        .map(|character| if character == '\n' { '\n' } else { ' ' })
        .collect::<String>();
    let admitted = SOURCE.replacen(UNSUPPORTED_OWNED_OPTION, &blank, 1);
    assert_eq!(admitted.len(), SOURCE.len());
    assert_eq!(admitted.replace(&blank, UNSUPPORTED_OWNED_OPTION), SOURCE);
    admitted
}

#[test]
fn original_owned_option_remains_an_exact_unadmitted_residual_control() {
    let diagnostics = crate::check(SOURCE, "history-owned.spx").unwrap_err();
    let errors = diagnostics
        .iter()
        .filter(|diagnostic| diagnostic.severity.is_error())
        .collect::<Vec<_>>();
    assert_eq!(errors.len(), 2, "{diagnostics:?}");
    for (diagnostic, message) in errors.iter().zip([
        "`?` with a live resource binding is not supported yet",
        "Option `?` accepts only direct `i64` or `bool` source and enclosing payloads",
    ]) {
        assert_eq!(diagnostic.code, "SPX-T218");
        assert_eq!(diagnostic.severity, crate::diagnostic::Severity::Error);
        assert_eq!(diagnostic.message, message);
        assert_eq!(diagnostic.path.as_deref(), Some("history-owned.spx"));
        assert_eq!(
            diagnostic.span,
            Some(crate::ast::Span {
                start: 1028,
                end: 1034,
                line: 30,
                column: 17,
            })
        );
        assert_eq!(diagnostic.help, None);
    }
}

fn program() -> ResolvedProgram {
    let source = admitted_source();
    let checked = crate::check(&source, "history-owned.spx").unwrap();
    let canonical = crate::format::canonical(&checked);
    let checked = crate::check(&canonical, "history-owned.spx").unwrap();
    assert_eq!(crate::format::canonical(&checked), canonical);
    let graph = crate::graph::to_json(&checked).unwrap();
    crate::graph::verify_json(&checked, &graph).unwrap();
    crate::hir::resolve(&checked).unwrap()
}
fn budget(program: &ResolvedProgram, function: &ResolvedFunction) -> ReplayBudget {
    let mut budget = ReplayBudget::new();
    reserve_program_skeleton_work(program, std::iter::once(function), &mut budget).unwrap();
    budget
}
#[test]
fn full_owned_transition_residual_and_scalar_corpus_keeps_flat_predecessor() {
    let program = program();
    let transitions = program
        .functions
        .iter()
        .flat_map(|function| &function.cleanup_plan.blocks)
        .flat_map(|block| &block.transitions)
        .collect::<Vec<_>>();
    for required in ["initialize", "transfer", "call", "stage"] {
        assert!(
            transitions
                .iter()
                .any(|transition| match (required, transition) {
                    (
                        "initialize",
                        CleanupTransition::Initialize { .. }
                        | CleanupTransition::InitializeVariant { .. },
                    )
                    | (
                        "transfer",
                        CleanupTransition::Transfer { .. }
                        | CleanupTransition::TransferVariant { .. },
                    )
                    | ("call", CleanupTransition::CallCommit { .. })
                    | ("stage", CleanupTransition::StageCopyResult { .. }) => true,
                    _ => false,
                }),
            "missing authored transition {required}"
        );
    }
    with_limit_usage_refusal(usize::MAX, || {
        for function in &program.functions {
            assert!(function.cleanup_plan.lexical_control.is_none());
            let mut old = budget(&program, function);
            let mut new = budget(&program, function);
            let mut expected = before::produce(function, &mut old).unwrap();
            paid!(
                &mut new,
                function,
                PRODUCE_CALL_CONTROLS,
                "owned corpus producer call"
            )?;
            let actual = produce(function, &mut new)?;
            assert_eq!(new.skeleton_spent, old.skeleton_spent);
            assert_eq!(new.skeleton_remaining, old.skeleton_remaining);
            expected.sort();
            paid!(
                &mut new,
                function,
                COMPARE_CALL_CONTROLS,
                "owned corpus comparison call"
            )?;
            compare(function, &mut new, actual, &expected)?;
        }
        Ok::<(), Diagnostic>(())
    })
    .0
    .unwrap();
}
#[test]
fn branch_and_ownership_mutations_keep_all_six_diagnostic_fields() {
    let program = program();
    let original = program
        .functions
        .iter()
        .find(|function| function.id.as_str() == "flow")
        .unwrap();
    with_limit_usage_refusal(usize::MAX,|| {
        for mutation in 0..2 {
            let mut hostile=original.clone();
            if mutation==0 {
                let transition=hostile.cleanup_plan.blocks.iter_mut().flat_map(|block|&mut block.transitions)
                    .find(|transition|matches!(transition,CleanupTransition::CallCommit {arguments,..} if arguments.len()>=2)).unwrap();
                let CleanupTransition::CallCommit {arguments,..}=transition else {unreachable!()};
                arguments.reverse();
            } else {
                let edge=hostile.cleanup_plan.edges.iter_mut().find(|edge|!matches!(edge.condition,EdgeCondition::Always)).unwrap();
                edge.condition=EdgeCondition::Always;
            }
            let mut old=budget(&program,&hostile);
            let mut new=budget(&program,&hostile);
            let expected=before::produce(&hostile,&mut old);
            paid!(&mut new,&hostile,PRODUCE_CALL_CONTROLS,"hostile corpus producer call")?;
            let actual=produce(&hostile,&mut new);
            match (expected,actual) {
                (Err(old),Err(new))=>assert_eq!(new,old),
                (Ok(mut flat),Ok(actual))=>{
                    flat.sort();
                    paid!(&mut new,&hostile,COMPARE_CALL_CONTROLS,"hostile corpus comparison call")?;
                    compare(&hostile,&mut new,actual,&flat)?;
                },
                _=>panic!("full predecessor and history producer selected different failure"),
            }
        }
        Ok::<(),Diagnostic>(())
    }).0.unwrap();
}
