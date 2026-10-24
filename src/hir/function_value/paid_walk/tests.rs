use super::*;
use crate::bounded_output::{reserve_active_required, with_limit_usage_refusal};
use crate::hir::closure::borrowed::is_borrowed;
use crate::hir::*;
use crate::test_allocation_observer::measure;
#[path = "old_borrowed.rs"]
mod old_borrowed;
#[path = "old_mutable.rs"]
mod old_mutable;

fn resolved(text: &str) -> ResolvedProgram {
    crate::hir::resolve(&crate::check(text, "paid-function-walk.spx").unwrap()).unwrap()
}
fn wide() -> ResolvedProgram {
    let mut source = String::from("module paid.walk; @id(\"walk.main\") fn main()->i64 {");
    for index in 0..80 {
        source.push_str(&format!("let value{index}={index};"));
    }
    source.push_str("42}");
    resolved(&source)
}

#[test]
fn paid_production_walk_keeps_complete_old_order_roots_templates_and_presence() {
    let fixtures = [
        "module walk; @id(\"walk.main\") fn main()->i64 requires true ensures result>0 { let a=if true {41}else{0}; a+1 }",
        "module walk; @id(\"w.inc\") fn inc(value:i64)->i64{value+1} @id(\"w.apply\") fn apply(callback:fn(i64)->i64,value:i64)->i64{callback(value)} @id(\"w.main\") fn main()->i64{apply(inc,41)}",
    ];
    fn retained_fixture(source: &str) -> &str {
        let start =
            source.find("const SOURCE: &str = r#\"").unwrap() + "const SOURCE: &str = r#\"".len();
        &source[start..start + source[start..].find("\"#;").unwrap()]
    }
    let borrowed = retained_fixture(include_str!(
        "../../../../tests/language/function_values/borrowed_closures.rs"
    ));
    let mutable = retained_fixture(include_str!(
        "../../../../tests/language/function_values/mutable_closures.rs"
    ));
    for program in fixtures
        .into_iter()
        .chain([borrowed, mutable])
        .map(resolved)
        .chain(std::iter::once(wide()))
    {
        for function in program
            .functions
            .iter()
            .chain(program.function_instances.iter().map(|i| &i.function))
        {
            let mut old = Vec::new();
            function_value::walk(function, |e| old.push(e.id.as_str().to_owned()));
            let mut current = Vec::new();
            walk(function, |e| current.push(e.id.as_str().to_owned())).unwrap();
            assert_eq!(current, old);
        }
        assert_eq!(
            requires(&program).unwrap(),
            function_value::requires_function_values(&program)
        );
        assert_eq!(
            crate::hir::closure::mutable::validate_uses(&program),
            old_mutable::validate_uses(&program)
        );
        assert_eq!(
            crate::hir::closure::borrowed::validate_uses(&program),
            old_borrowed::validate_uses(&program)
        );
        let mut forged = program.clone();
        // A hostile mutable parameter retains the complete old first diagnostic.
        forged.functions[0].params.push(ResolvedParam {
            id: ValueId::new("forged.parameter".to_owned()),
            name: "forged".to_owned(),
            ty: ResolvedType::MutFunctionI64,
            ownership: OwnershipMode::Own,
            span: crate::ast::Span::default(),
        });
        let current = crate::hir::closure::mutable::validate_uses(&forged);
        let original = old_mutable::validate_uses(&forged);
        assert!(current.is_err());
        assert_eq!(current, original);
    }
}

#[test]
fn paid_production_walk_spill_exact_short_sticky_and_drop_are_literal() {
    let program = wide();
    let function = &program.functions[0];
    let observe = |limit| {
        with_limit_usage_refusal(limit, || {
            measure(|| {
                let mut visited = 0;
                let result = walk(function, |_| visited += 1);
                let admitted = result.is_ok();
                drop(result);
                (admitted, visited)
            })
        })
    };
    let (((admitted, count), observed), refused, required, _) = observe(usize::MAX);
    assert!(admitted && !refused && count > 80 && required > 0);
    assert!(!observed.invalid && observed.live == 0);
    assert_eq!(observed.allocated, observed.freed);
    assert!(observed.allocated <= required && observed.highwater <= required);
    let (((admitted, exact_count), exact), refused, used, _) = observe(required);
    assert!(admitted && !refused && used == required && exact_count == count);
    assert!(!exact.invalid && exact.live == 0 && exact.allocated == exact.freed);
    let (((admitted, _), short), refused, _, first) = observe(required - 1);
    assert!(!admitted && refused && first.is_some());
    assert!(!short.invalid && short.live == 0 && short.allocated == short.freed);
    with_limit_usage_refusal(0, || {
        assert!(!reserve_active_required(1));
        let mut visited = 0;
        assert!(walk(function, |_| visited += 1).is_err());
        assert_eq!(visited, 0);
        assert!(!reserve_active_required(0));
    });
    let scalar = resolved("module scalar; @id(\"scalar.main\") fn main()->i64{42}");
    let (((), inline), refused, used, _) = with_limit_usage_refusal(0, || {
        measure(|| {
            walk(&scalar.functions[0], |_| {}).unwrap();
        })
    });
    assert!(!refused && used == 0);
    let ((), old_inline) = measure(|| function_value::walk(&scalar.functions[0], |_| {}));
    assert!(!old_inline.invalid && old_inline.live == 0);
    assert_eq!(old_inline.allocated, old_inline.freed);
    assert!(old_inline.allocated > inline.allocated);
    assert_eq!(inline.allocated, 0);
    assert_eq!(inline.live, 0);
    assert!(!inline.invalid);
}

#[test]
fn complete_old_walk_presence_and_uses_sources_remain_exact() {
    let saved = include_str!(
        "../../../../docs/evidence/overlay-transitive-funding/function_value_before_paid_walk.txt"
    );
    let current = include_str!("../../function_value.rs");
    let wrapper = "pub(crate) fn walk<'a>(function: &'a ResolvedFunction, mut visit: impl FnMut(&'a ResolvedExpr)) {\n    walk_with(function, &mut visit);\n}\n\n";
    let kernel = "#[inline(never)]\nfn walk_with<'a>(function: &'a ResolvedFunction, visit: &mut dyn FnMut(&'a ResolvedExpr)) {\n";
    assert_eq!(current.matches(wrapper).count(), 1);
    assert_eq!(current.matches(kernel).count(), 1);
    let joined = wrapper.to_owned() + kernel;
    assert_eq!(current.matches(&joined).count(), 1);
    let current = current.replacen(&joined, "pub(crate) fn walk<'a>(function: &'a ResolvedFunction, mut visit: impl FnMut(&'a ResolvedExpr)) {\n", 1);
    let current = current.as_str();
    fn body<'a>(source: &'a str, name: &str) -> &'a str {
        let start = source.find(name).unwrap();
        let end = start + source[start..].find("\n}\n").unwrap() + 3;
        &source[start..end]
    }
    for name in [
        "pub(crate) fn walk",
        "pub fn requires_function_values",
        "pub(crate) fn function_uses_value",
        "pub(crate) fn template_uses_value",
    ] {
        assert_eq!(body(current, name), body(saved, name));
    }
    for (old, receiver) in [
        (include_str!("../../../../docs/evidence/overlay-transitive-funding/mutable_closure_before_paid_walk.txt"), include_str!("old_mutable.rs")),
        (include_str!("../../../../docs/evidence/overlay-transitive-funding/borrowed_closure_before_paid_walk.txt"), include_str!("old_borrowed.rs")),
    ] {
        assert_eq!(receiver, format!("use super::*;\n{}", &old[old.find("pub(crate) fn validate_uses(").unwrap()..]));
    }
}
