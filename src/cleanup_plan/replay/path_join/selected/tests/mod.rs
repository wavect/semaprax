//! Selected path family against complete immediate pre-path drivers.
use super::*;
use crate::cleanup_plan::replay::{
    lexical_replay, primitive_fixture, scalar_replay, selected_context,
};
use crate::test_allocation_observer::measure;
const LIMIT: usize = 64 * 1024 * 1024;
const LEXICAL: &str = r#"module structural.selected;
@id("probe") fn probe(flag:bool)->i64 {
 let key=str"x";let view=str_as_bytes(key);let mut next=0;
 while next<3 {next=next+1;if flag {continue;};if next==2 {return if byte_len(view)==1usize {7}else{0};};0}
 9
}
@id("main") fn main()->i64 {probe(false)}
"#;
const SCALAR: &str = r#"module selected.schema.family;
@id("family.one") fn one(n:i64)->i64 {n+1}
@id("family.main") fn main()->i64 {one(3)}
@id("family.fill") fn fill(data:own Bytes)->Bytes {data}
@id("family.bytes") fn bytes()->i64 {
 let data=fill(bytes_zeroed(1usize));
 return if byte_len(bytes_as_slice(data))==1usize {7}else{0};
}
"#;
// Exact original unreachable fixture remains an explicit parser refusal.
const SCALAR_UNREACHABLE: &str = r#"module selected.schema.family;
@id("family.one") fn one(n:i64)->i64 {n+1}
@id("family.main") fn main()->i64 {one(3)}
@id("family.fill") fn fill(data:own Bytes)->Bytes {data}
@id("family.bytes") fn bytes()->i64 {
 let data=fill(bytes_zeroed(1usize));
 return if byte_len(bytes_as_slice(data))==1usize {7}else{0};
 0
}
"#;
// These are genuinely different selected domains. SCALAR above remains the
// complete original owning-Bytes refusal subject, not a Copy scalar witness.
const COPY_SCALAR: &str = r#"module selected.schema.copy;
@id("family.one") fn one(n:i64)->i64 {n+1}
@id("family.main") fn main()->i64 {one(3)}
"#;
const OWNED_LEXICAL: &str = r#"module selected.schema.owned_lexical;
@id("family.bytes") fn bytes()->i64 {
 let data=bytes_zeroed(1usize);
 if byte_len(bytes_as_slice(data))==1usize {return 7;};
 0
}
"#;
// A real loop escape selects lexical replay; the original library above
// remains the exact standalone T105 and scalar-profile refusal witness.
const OWNED_LEXICAL_LOOP: &str = r#"module selected.schema.owned_lexical;
@id("family.bytes") fn bytes()->i64 {
 let data=bytes_zeroed(1usize);
 let mut next=0;
 while next<1 {next=next+1;if next==1 {continue;};0}
 if byte_len(bytes_as_slice(data))==1usize {return 7;};
 0
}
"#;
fn program(source: &str) -> ResolvedProgram {
    let parsed = crate::parse(source, "selected-path-family.spx").unwrap();
    if parsed
        .functions
        .iter()
        .any(|function| function.name == "main")
    {
        return crate::hir::resolve(&crate::check(source, "selected-path-family.spx").unwrap())
            .unwrap();
    }
    // Keep the complete original library and both exact public refusals. Its
    // owning success is a real project import/call, never an appended entry.
    assert!(
        [OWNED_LEXICAL, OWNED_LEXICAL_LOOP].contains(&source),
        "closed selected library fixtures"
    );
    let rejected = vec![Diagnostic {
        code: "SPX-T105",
        severity: crate::diagnostic::Severity::Error,
        message: "executable module must define `fn main() -> i64`".into(),
        path: Some("selected-path-family.spx".to_owned()),
        span: Some(parsed.functions[0].span),
        help: Some("a module without `fn main() -> i64` is a library module: check it through the project that owns it with `semaprax check <project-dir>`, or add `main` to run this file alone".into()),
    }];
    assert_eq!(
        crate::check(source, "selected-path-family.spx").unwrap_err(),
        rejected
    );
    assert_eq!(crate::hir::resolve(&parsed).unwrap_err(), rejected);
    const CONSUMER: &str = r#"module selected.library.consumer;
use function @id("family.bytes") from selected.schema.owned_lexical as bytes;
@id("selected.library.consumer.main") fn main()->i64 {bytes()}
"#;
    let sources = [
        ("selected-path-family.spx", source),
        ("selected-library-consumer.spx", CONSUMER),
    ]
    .into_iter()
    .map(|(path, text)| crate::workspace_graph::WorkspaceSource {
        path: path.to_owned(),
        source: crate::format::canonical(&crate::parse(text, path).unwrap()),
    })
    .collect();
    let project = crate::workspace_graph::build_owned(sources).unwrap();
    let linked = project
        .linked_scalar_program_with_roots(
            "selected.library.consumer",
            &["family.bytes".to_owned()],
            crate::project::ProjectProfile::OwnedDataApiV1,
            false,
        )
        .unwrap();
    assert_eq!(linked.functions.len(), parsed.functions.len() + 1);
    assert!(linked
        .functions
        .iter()
        .any(|f| f.id.as_str() == "family.bytes"));
    crate::hir::validate(&linked).unwrap();
    linked
}
fn render(error: Error, function: &ResolvedFunction) -> Diagnostic {
    match error {
        Error::Owned(error) => error,
        Error::Structural(receipt) => receipt.legacy(function),
        Error::Fact(receipt) => receipt.legacy(function),
        Error::Failed => panic!("fresh invocation cannot already have failed"),
    }
}
fn owner_cost(
    source: &dyn ReplayContext,
    function: &ResolvedFunction,
    old: bool,
) -> (Result<(), Diagnostic>, usize, usize) {
    let lexical = crate::hir::lexical_exit::requires(function)
        .expect("genuine checked HIR lexical classification");
    let mut budget = ReplayBudget::with_physical(function, LIMIT).unwrap();
    let initial = budget.remaining;
    let context = match selected_context::Context::new_paid(source, function, &mut budget, lexical)
    {
        Ok(context) => context,
        Err(error) => {
            return (
                Err(render(error, function)),
                budget.physical.as_ref().unwrap().charged(),
                initial - budget.remaining,
            )
        }
    };
    let result = match (old, lexical) {
        (true, true) => lexical_replay::before_selected_path::validate_before_selected_path(
            &context,
            function,
            &mut budget,
        ),
        (true, false) => scalar_replay::before_selected_path::validate_before_selected_path(
            &context,
            function,
            &mut budget,
        ),
        (false, true) => lexical_replay::validate_paid(&context, function, &mut budget),
        (false, false) => scalar_replay::validate_paid(&context, function, &mut budget),
    };
    // Detailed rendering is an explicit legacy observation after paid replay.
    (
        result.map_err(|e| render(e, function)),
        budget.physical.as_ref().unwrap().charged(),
        initial - budget.remaining,
    )
}
fn owner(
    source: &dyn ReplayContext,
    function: &ResolvedFunction,
    old: bool,
) -> Result<(), Diagnostic> {
    owner_cost(source, function, old).0
}

fn same_owner(program: &ResolvedProgram, id: &str, expected_success: bool) {
    let source = primitive_fixture::Context::new(program);
    let function = program
        .functions
        .iter()
        .find(|f| f.id.as_str() == id)
        .unwrap();
    let mut outcomes = Vec::new();
    for old in [true, false] {
        let ((result, bytes, work), observed) = measure(|| owner_cost(&source, function, old));
        assert!(!observed.invalid);
        eprintln!(
            "selected-path {} old={old} physical={bytes} work={work}",
            function.id
        );
        // Returned detailed diagnostic owns text until the observation drops it.
        outcomes.push(result);
    }
    assert_eq!(
        outcomes[0], outcomes[1],
        "all six fields/first refusal {id}"
    );
    let ((immediate, bytes, work), observed) =
        measure(|| super::frontier_tests::owner_cost(&source, function));
    assert!(!observed.invalid);
    eprintln!(
        "compact-frontier {} immediate-old physical={bytes} work={work}",
        function.id
    );
    assert_eq!(
        outcomes[1], immediate,
        "complete immediate-old six-field diagnostic {id}"
    );
    drop(immediate);
    let (_, observed) = measure(|| drop(super::frontier_tests::owner_cost(&source, function)));
    assert!(!observed.invalid && observed.live == 0);
    assert_eq!(observed.allocated, observed.freed);
    assert_eq!(
        outcomes[1].is_ok(),
        expected_success,
        "{id}: {:?}",
        outcomes[1]
    );
    let (_, observed) = measure(|| {
        for old in [true, false] {
            drop(owner(&source, function, old));
        }
    });
    assert!(!observed.invalid && observed.live == 0);
    assert_eq!(observed.allocated, observed.freed);
}

mod boundary;
#[allow(dead_code, reason = "complete original enum/carrier layout witnesses")]
mod layout;
mod nominal;
mod session;
mod whole;

mod history;
mod renewal;

mod lookup;
mod strings;

// Compile the complete existing preflight owner without changing its bytes.
// Nonlexical ordinary replay preflights before the selected scalar profile;
// these intentionally distinct entry routes retain their own first refusal.
#[path = "../../../cfg_bounds/tests/old.rs"]
mod ordinary_cfg_before;

mod fixture_scope;
