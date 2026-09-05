//! Declaration-level checks reached from `verify`.
//!
//! These call the checks directly with a parsed program and a
//! [`TypeTable`], so each stable diagnostic code is pinned to the exact
//! declared shape that produces it rather than to whichever check happens to
//! run first in the whole-program pass.

use std::path::Path;

use super::*;

const DECLARATIONS: &str = r#"module test.declared_type;

@id("t.plain")
record Plain {
    @id("t.plain.count")
    count: i64,
}

@id("t.buffer")
record Buffer {
    @id("t.buffer.payload")
    payload: Bytes,
}

@id("t.token")
resource Token {
    @id("t.token.drop")
    drop trivial;
}

@id("t.cell")
record Cell<T> {
    @id("t.cell.value")
    value: T,
}

@id("app.pick")
fn pick<Left, Right>(left: Left, right: Right, flag: bool) -> Left
{
    if flag { left } else { left }
}

@id("app.scalar")
fn scalar(value: i64) -> i64
{
    let doubled = value + value;
    doubled
}

@id("app.aggregate")
fn aggregate(value: i64) -> Plain
{
    Plain { count: value }
}

@id("app.bytes")
fn bytes(value: i64) -> i64
{
    let sample = [1u8, 2u8];
    value
}

@id("app.main")
fn main() -> i64
{
    0
}
"#;

fn parsed(source: &str, path: &str) -> Program {
    crate::parse(source, Path::new(path)).expect("fixture parses")
}

fn named(name: &str, arguments: Vec<Type>) -> Type {
    Type::Named {
        name: name.to_owned(),
        arguments,
    }
}

fn function<'a>(program: &'a Program, name: &str) -> &'a Function {
    program
        .functions
        .iter()
        .find(|function| function.name == name)
        .expect("fixture declares the function")
}

fn param(name: &str, mode: ParamMode, ty: Type) -> Param {
    Param {
        name: name.to_owned(),
        mode,
        ty,
        span: Span::default(),
    }
}

fn declared_type_codes(ty: &Type, parameters: &[&str]) -> Vec<&'static str> {
    let program = parsed(DECLARATIONS, "declared-type.spx");
    let types = TypeTable::new(&program);
    let parameters = parameters.iter().copied().collect::<HashSet<&str>>();
    let mut diagnostics = Vec::new();
    check_declared_type(
        &program,
        ty,
        Span::default(),
        &types,
        &parameters,
        &mut diagnostics,
    );
    diagnostics
        .into_iter()
        .map(|diagnostic| diagnostic.code)
        .collect()
}

#[test]
fn an_unknown_type_name_is_t001_outside_a_generic_and_t220_inside_one() {
    // Outside a generic declaration the only reading is "you never declared
    // this type"; inside one it is more likely a mistyped parameter, and the
    // two repairs are different.
    assert_eq!(
        declared_type_codes(&named("Absent", Vec::new()), &[]),
        ["SPX-T001"]
    );
    assert_eq!(
        declared_type_codes(&named("Absent", Vec::new()), &["T"]),
        ["SPX-T220"]
    );
    // An in-scope parameter given arguments is also T220.
    assert_eq!(
        declared_type_codes(&named("T", vec![Type::I64]), &["T"]),
        ["SPX-T220"]
    );
    // A bare in-scope parameter and a declared type are both silent.
    assert!(declared_type_codes(&named("T", Vec::new()), &["T"]).is_empty());
    assert!(declared_type_codes(&named("Plain", Vec::new()), &[]).is_empty());
}

#[test]
fn type_argument_arity_and_element_admission_have_separate_codes() {
    // Arity first: `Option` takes exactly one argument.
    assert_eq!(
        declared_type_codes(&named("Option", Vec::new()), &[]),
        ["SPX-T221"]
    );
    assert_eq!(
        declared_type_codes(&named("Cell", vec![Type::I64, Type::I64]), &[]),
        ["SPX-T221"]
    );
    // A correctly shaped instance with a non-copy argument is T223, which is a
    // different repair from an arity mistake.
    assert_eq!(
        declared_type_codes(&named("Option", vec![Type::Usize]), &[]),
        ["SPX-T223"]
    );
    assert_eq!(
        declared_type_codes(&named("Cell", vec![Type::String]), &[]),
        ["SPX-T223"]
    );
    assert!(declared_type_codes(&named("Option", vec![Type::I64]), &[]).is_empty());
    assert!(declared_type_codes(&named("Cell", vec![Type::Bool]), &[]).is_empty());
}

#[test]
fn fixed_arrays_and_unadmitted_byte_carriers_are_t268_and_stop_further_reports() {
    // A fixed array is never a generic argument, whatever the carrier.
    assert_eq!(
        declared_type_codes(&named("Option", vec![Type::ArrayU8(4)]), &[]),
        ["SPX-T268"]
    );
    // `Result<Bytes, Bytes>` is outside the owned-byte prelude list. T268 is
    // reported once and the copy-scalar check is skipped, so the author sees
    // the byte rule rather than two competing repairs.
    assert_eq!(
        declared_type_codes(&named("Result", vec![Type::Bytes, Type::Bytes]), &[]),
        ["SPX-T268"]
    );
    // The admitted owned-byte carriers stay silent.
    assert!(declared_type_codes(&named("Option", vec![Type::Bytes]), &[]).is_empty());
    assert!(declared_type_codes(&named("Result", vec![Type::Bytes, Type::I64]), &[]).is_empty());
    assert!(declared_type_codes(&named("Result", vec![Type::Bool, Type::Bytes]), &[]).is_empty());
}

fn ownership_codes(function_name: &str, param: Param) -> Vec<&'static str> {
    let program = parsed(DECLARATIONS, "declared-type.spx");
    let types = TypeTable::new(&program);
    let mut diagnostics = Vec::new();
    check_ownership_mode(
        &program,
        function(&program, function_name),
        &param,
        &types,
        &mut diagnostics,
    );
    diagnostics
        .into_iter()
        .map(|diagnostic| diagnostic.code)
        .collect()
}

#[test]
fn borrowed_view_parameters_demand_borrow_with_their_own_codes() {
    // `str` and `Slice<u8>` are both borrow-only, but they report under
    // different stable codes and repair tools key on that.
    assert_eq!(
        ownership_codes("scalar", param("text", ParamMode::Value, Type::Str)),
        ["SPX-O115"]
    );
    assert_eq!(
        ownership_codes("scalar", param("text", ParamMode::Own, Type::Str)),
        ["SPX-O115"]
    );
    assert!(ownership_codes("scalar", param("text", ParamMode::Borrow, Type::Str)).is_empty());

    assert_eq!(
        ownership_codes("scalar", param("view", ParamMode::Value, Type::SliceU8)),
        ["SPX-T263"]
    );
    assert!(ownership_codes("scalar", param("view", ParamMode::Borrow, Type::SliceU8)).is_empty());
}

#[test]
fn owned_bytes_parameters_admit_own_and_monomorphic_borrow_only() {
    assert!(ownership_codes("scalar", param("data", ParamMode::Own, Type::Bytes)).is_empty());
    assert!(ownership_codes("scalar", param("data", ParamMode::Borrow, Type::Bytes)).is_empty());
    assert_eq!(
        ownership_codes("scalar", param("data", ParamMode::Value, Type::Bytes)),
        ["SPX-T263"]
    );
    assert_eq!(
        ownership_codes("scalar", param("data", ParamMode::Shared, Type::Bytes)),
        ["SPX-T263"]
    );
    // `borrow Bytes` is the one synchronous borrowed owner carrier and is
    // admitted only where there is no type substitution to reason about.
    assert_eq!(
        ownership_codes("pick", param("data", ParamMode::Borrow, Type::Bytes)),
        ["SPX-T263"]
    );
    assert!(ownership_codes("pick", param("data", ParamMode::Own, Type::Bytes)).is_empty());
}

#[test]
fn drop_bearing_aggregates_need_a_mode_and_value_types_must_not_have_one() {
    // An authored resource and a record that merely holds owned bytes both
    // require an explicit mode, under the same code.
    assert_eq!(
        ownership_codes(
            "scalar",
            param("token", ParamMode::Value, named("Token", Vec::new()))
        ),
        ["SPX-O001"]
    );
    assert_eq!(
        ownership_codes(
            "scalar",
            param("buffer", ParamMode::Value, named("Buffer", Vec::new()))
        ),
        ["SPX-O001"]
    );
    assert!(ownership_codes(
        "scalar",
        param("token", ParamMode::Own, named("Token", Vec::new()))
    )
    .is_empty());

    // The converse: a mode on a pure value type is its own code, so the two
    // mistakes never share a repair.
    assert_eq!(
        ownership_codes("scalar", param("value", ParamMode::Own, Type::I64)),
        ["SPX-O002"]
    );
    assert_eq!(
        ownership_codes(
            "scalar",
            param("plain", ParamMode::Borrow, named("Plain", Vec::new()))
        ),
        ["SPX-O002"]
    );
    assert!(ownership_codes("scalar", param("value", ParamMode::Value, Type::I64)).is_empty());
    assert!(ownership_codes(
        "scalar",
        param("plain", ParamMode::Value, named("Plain", Vec::new()))
    )
    .is_empty());
}

#[test]
fn record_layout_recursion_sees_cycles_through_other_declarations() {
    let program = parsed(
        r#"module test.declared_type_recursion;

@id("t.left")
record Left {
    @id("t.left.right")
    right: Right,
}

@id("t.right")
record Right {
    @id("t.right.left")
    left: Left,
}

@id("t.leaf")
record Leaf {
    @id("t.leaf.count")
    count: i64,
}

@id("t.pair")
record Pair {
    @id("t.pair.first")
    first: Leaf,
    @id("t.pair.second")
    second: Leaf,
}

@id("t.cell")
record Cell<T> {
    @id("t.cell.value")
    value: T,
}

@id("app.main")
fn main() -> i64
{
    0
}
"#,
        "declared-type-recursion.spx",
    );
    let types = TypeTable::new(&program);
    let recursive = |name: &str| {
        record_layout_is_recursive(name, &types, &mut HashSet::new(), &mut HashSet::new())
    };

    // Neither `Left` nor `Right` names itself; only the cycle between them is
    // infinite, and the walk must find it from either end.
    assert!(recursive("Left"));
    assert!(recursive("Right"));
    // Two fields of the same record type are a diamond, not a cycle. Treating
    // a repeated visit as recursion would reject ordinary aggregates.
    assert!(!recursive("Pair"));
    assert!(!recursive("Leaf"));
    // A field whose type is one of the declaration's own parameters is not a
    // nominal edge, and an undeclared name is not one either.
    assert!(!recursive("Cell"));
    assert!(!recursive("Absent"));
}

#[test]
fn scalar_substitutions_enumerate_every_assignment_in_a_stable_order() {
    assert_eq!(scalar_function_substitutions(0), vec![Vec::<Type>::new()]);
    assert_eq!(
        scalar_function_substitutions(1),
        vec![vec![Type::I64], vec![Type::Bool]]
    );
    // The first parameter varies fastest. This order reaches the generic
    // validator, so a change reorders the diagnostics it reports.
    assert_eq!(
        scalar_function_substitutions(2),
        vec![
            vec![Type::I64, Type::I64],
            vec![Type::Bool, Type::I64],
            vec![Type::I64, Type::Bool],
            vec![Type::Bool, Type::Bool],
        ]
    );
    assert_eq!(scalar_function_substitutions(4).len(), 16);
}

#[test]
fn function_type_substitution_replaces_parameters_and_rebuilds_nesting() {
    let program = parsed(DECLARATIONS, "declared-type.spx");
    let pick = function(&program, "pick");
    let arguments = [Type::I64, Type::Bool];

    assert_eq!(
        substitute_function_type(pick, &arguments, &named("Left", Vec::new())),
        Some(Type::I64)
    );
    assert_eq!(
        substitute_function_type(pick, &arguments, &named("Right", Vec::new())),
        Some(Type::Bool)
    );
    // Substitution reaches inside a generic instance and rebuilds it.
    assert_eq!(
        substitute_function_type(
            pick,
            &arguments,
            &named("Option", vec![named("Left", Vec::new())])
        ),
        Some(named("Option", vec![Type::I64]))
    );
    // Unrelated names and scalars pass through unchanged.
    assert_eq!(
        substitute_function_type(pick, &arguments, &named("Plain", Vec::new())),
        Some(named("Plain", Vec::new()))
    );
    assert_eq!(
        substitute_function_type(pick, &arguments, &Type::Usize),
        Some(Type::Usize)
    );
    // Too few arguments must fail rather than leave a parameter name behind
    // that would later read as a concrete type.
    assert_eq!(
        substitute_function_type(pick, &[Type::I64], &named("Right", Vec::new())),
        None
    );
    assert!(validation_specialize_signature(pick, &[Type::I64]).is_none());
    let (params, return_type) =
        validation_specialize_signature(pick, &arguments).expect("full substitution");
    assert_eq!(params[0].ty, Type::I64);
    assert_eq!(params[1].ty, Type::Bool);
    assert_eq!(return_type, Type::I64);
}

#[test]
fn generic_signature_slots_and_direct_arguments_admit_only_the_scalar_profile() {
    let parameters = ["T"].into_iter().collect::<HashSet<&str>>();
    for admitted in [Type::I64, Type::Bool, Type::String] {
        assert!(
            generic_function_signature_slot(&admitted, &parameters),
            "{admitted:?}"
        );
    }
    assert!(generic_function_signature_slot(
        &named("T", Vec::new()),
        &parameters
    ));
    for rejected in [
        Type::Usize,
        Type::Bytes,
        Type::Str,
        Type::SliceU8,
        Type::ArrayU8(2),
        named("T", vec![Type::I64]),
        named("Plain", Vec::new()),
    ] {
        assert!(
            !generic_function_signature_slot(&rejected, &parameters),
            "{rejected:?}"
        );
    }
    // Only `i64` and `bool` may be written as explicit type arguments, so
    // `string` is a signature slot but never a substitution.
    assert!(direct_function_type_argument(&Type::I64));
    assert!(direct_function_type_argument(&Type::Bool));
    assert!(!direct_function_type_argument(&Type::String));
}

#[test]
fn a_generic_body_is_direct_scalar_only_without_aggregates() {
    let program = parsed(DECLARATIONS, "declared-type.spx");
    assert!(generic_function_expression_is_direct_scalar(
        &function(&program, "scalar").body
    ));
    // Constructing a record and materializing a fixed array both leave the
    // profile a generic body may be substituted into.
    assert!(!generic_function_expression_is_direct_scalar(
        &function(&program, "aggregate").body
    ));
    assert!(!generic_function_expression_is_direct_scalar(
        &function(&program, "bytes").body
    ));
}

#[test]
fn call_reachability_terminates_on_a_cycle() {
    let graph = HashMap::from([
        ("a".to_owned(), vec!["b".to_owned()]),
        ("b".to_owned(), vec!["c".to_owned(), "a".to_owned()]),
        ("c".to_owned(), vec!["b".to_owned()]),
    ]);
    assert!(function_reaches(&graph, "a", "c", &mut HashSet::new()));
    // A recursive cycle must answer, not hang, and must not invent an edge to
    // a name nothing calls.
    assert!(!function_reaches(&graph, "a", "d", &mut HashSet::new()));
    assert!(function_reaches(&graph, "a", "a", &mut HashSet::new()));

    let targets = ["d", "c"].into_iter().collect::<HashSet<&str>>();
    assert!(function_reaches_any(
        &graph,
        "a",
        &targets,
        &mut HashSet::new()
    ));
    let unreachable = ["d"].into_iter().collect::<HashSet<&str>>();
    assert!(!function_reaches_any(
        &graph,
        "a",
        &unreachable,
        &mut HashSet::new()
    ));
}

#[test]
fn ordinary_result_and_option_shapes_are_recognized_by_name_and_arity() {
    assert_eq!(
        ordinary_result_arguments(&named("Result", vec![Type::I64, Type::Bool])),
        Some((&Type::I64, &Type::Bool))
    );
    assert_eq!(
        ordinary_option_argument(&named("Option", vec![Type::Bytes])),
        Some(&Type::Bytes)
    );
    // A wrong arity or a different name is not the prelude carrier, even
    // though it reads like one.
    assert!(ordinary_result_arguments(&named("Result", vec![Type::I64])).is_none());
    assert!(ordinary_result_arguments(&named("Outcome", vec![Type::I64, Type::Bool])).is_none());
    assert!(ordinary_option_argument(&named("Option", Vec::new())).is_none());
    assert!(ordinary_option_argument(&Type::I64).is_none());
}

#[test]
fn native_rust_status_domains_are_bounded_lowercase_dotted_labels() {
    assert!(native_rust_status_domain("io"));
    assert!(native_rust_status_domain("host.io-status.v1"));
    assert!(native_rust_status_domain(&"a".repeat(128)));

    // Too short, too long, uppercase, and boundary punctuation are all
    // outside the domain grammar the interop ABI transports.
    assert!(!native_rust_status_domain("a"));
    assert!(!native_rust_status_domain(""));
    assert!(!native_rust_status_domain(&"a".repeat(129)));
    assert!(!native_rust_status_domain("Host.io"));
    assert!(!native_rust_status_domain(".host"));
    assert!(!native_rust_status_domain("host."));
    assert!(!native_rust_status_domain("-host"));
    assert!(!native_rust_status_domain("host-"));
    assert!(!native_rust_status_domain("host_io"));
    assert!(!native_rust_status_domain("host io"));
}
