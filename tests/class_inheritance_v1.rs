use std::path::Path;
use std::process::Command;

use semaprax::cleanup_plan::CleanupTransition;
use semaprax::{format, graph, hir, parse, verify, wasm};

const CORPUS: &str = r#"module test.inheritance.v1;

@id("t1.animal")
class Animal {
    @id("t1.animal.tag")
    tag: bool,
    @id("t1.animal.legs")
    legs: i64,

    @id("t1.animal.describe")
    fn describe(self: Animal) -> i64
{
        self.legs
    }

    @id("t1.animal.label")
    fn label(self: Animal) -> string
{
        let base = "animal";
        base
    }
}

@id("t1.dog")
class Dog : Animal {
    @id("t1.dog.bark")
    bark: i64,

    @id("t1.dog.describe")
    fn describe(self: Dog) -> i64
{
        super.describe() + self.bark
    }

    @id("t1.dog.label")
    fn label(self: Dog) -> string
{
        let base = "dog";
        base
    }
}

@id("t1.puppy")
class Puppy : Dog {
    @id("t1.puppy.cute")
    cute: i64,

    @id("t1.puppy.score")
    fn score(self: Puppy) -> i64
{
        self.describe() + self.cute
    }
}

@id("app.main")
fn main() -> i64
{
    let d = Dog { tag: true, legs: 4, bark: 2 };
    let p = Puppy { tag: false, legs: 4, bark: 1, cute: 10 };
    let a: Animal = p;
    if d.describe() == 6 && a.describe() == 4 && p.score() == 15 { if d.label() == "dog" && a.label() == "animal" { d.describe() } else { 0 } } else { 0 }
}
"#;

fn parse_ok(source: &str) -> semaprax::ast::Program {
    parse(source, Path::new("inheritance-v1.spx")).expect("inheritance program must parse")
}

fn resolved(source: &str) -> hir::ResolvedProgram {
    let program = parse_ok(source);
    hir::resolve(&program).expect("inheritance program must resolve")
}

fn codes_from_verify(source: &str) -> Vec<&'static str> {
    let program = parse_ok(source);
    verify::verify(&program)
        .into_iter()
        .map(|diagnostic| diagnostic.code)
        .collect()
}

fn codes_from_resolve(source: &str) -> Vec<&'static str> {
    let program = parse_ok(source);
    hir::resolve(&program)
        .expect_err("program must be rejected")
        .into_iter()
        .map(|diagnostic| diagnostic.code)
        .collect()
}

#[test]
fn inheritance_corpus_round_trips_canonically_and_resolves() {
    let program = parse_ok(CORPUS);
    assert!(verify::verify(&program).is_empty());
    let canonical = format::canonical(&program);
    assert_eq!(canonical, CORPUS);
    let reparsed = parse(&canonical, Path::new("inheritance-canonical.spx")).unwrap();
    assert_eq!(format::canonical(&reparsed), canonical);

    // The resolved child sequence extends the standalone ancestor exactly:
    // inherited members keep their prefix positions and own fields append.
    let resolved = resolved(CORPUS);
    let animal_fields = resolved
        .declarations
        .record_fields(&hir::DeclarationId::new("t1.animal"))
        .unwrap();
    let dog_fields = resolved
        .declarations
        .record_fields(&hir::DeclarationId::new("t1.dog"))
        .unwrap();
    let puppy_fields = resolved
        .declarations
        .record_fields(&hir::DeclarationId::new("t1.puppy"))
        .unwrap();
    assert_eq!(
        dog_fields[..animal_fields.len()],
        animal_fields[..],
        "Dog must begin with Animal's exact effective prefix"
    );
    assert_eq!(
        puppy_fields[..dog_fields.len()],
        dog_fields[..],
        "Puppy must begin with Dog's exact effective prefix"
    );
    assert!(resolved.declarations.class_extends(
        &hir::DeclarationId::new("t1.dog"),
        &hir::DeclarationId::new("t1.animal"),
    ));
}

#[test]
fn parent_prefix_layouts_match_the_standalone_parent_on_both_targets() {
    // The native emitter pins every aggregate field offset with a
    // `_Static_assert`, so byte-level layout claims are checked by the C
    // compiler itself. The child struct must carry the standalone parent's
    // exact prefix offsets plus its own appended fields.
    if !command_available("clang") {
        return;
    }
    let program = parse_ok(CORPUS);
    let generated = semaprax::codegen::emit_c(&program).unwrap();
    let symbol = |id: &str| format!("spx_record_{}", hex_identity(id));
    let field = |id: &str| format!("spx_field_{}", hex_identity(id));

    let animal = symbol("t1.animal");
    let dog = symbol("t1.dog");
    let puppy = symbol("t1.puppy");

    // Standalone parent layout: bool then i64.
    assert!(generated.contains(&format!(
        "_Static_assert(sizeof(struct {animal}) == UINT32_C(16)"
    )));
    assert!(generated.contains(&format!(
        "_Static_assert(offsetof(struct {animal}, {}) == UINT32_C(0)",
        field("t1.animal.tag")
    )));
    assert!(generated.contains(&format!(
        "_Static_assert(offsetof(struct {animal}, {}) == UINT32_C(8)",
        field("t1.animal.legs")
    )));

    // Child prefix repeats the parent offsets exactly; own fields append.
    assert!(generated.contains(&format!(
        "_Static_assert(offsetof(struct {dog}, {}) == UINT32_C(0)",
        field("t1.animal.tag")
    )));
    assert!(generated.contains(&format!(
        "_Static_assert(offsetof(struct {dog}, {}) == UINT32_C(8)",
        field("t1.animal.legs")
    )));
    assert!(generated.contains(&format!(
        "_Static_assert(offsetof(struct {dog}, {}) == UINT32_C(16)",
        field("t1.dog.bark")
    )));
    assert!(generated.contains(&format!(
        "_Static_assert(sizeof(struct {dog}) == UINT32_C(24)"
    )));

    // Three-level chain appends again without disturbing either prefix.
    assert!(generated.contains(&format!(
        "_Static_assert(offsetof(struct {puppy}, {}) == UINT32_C(8)",
        field("t1.animal.legs")
    )));
    assert!(generated.contains(&format!(
        "_Static_assert(offsetof(struct {puppy}, {}) == UINT32_C(16)",
        field("t1.dog.bark")
    )));
    assert!(generated.contains(&format!(
        "_Static_assert(offsetof(struct {puppy}, {}) == UINT32_C(24)",
        field("t1.puppy.cute")
    )));
    assert!(generated.contains(&format!(
        "_Static_assert(sizeof(struct {puppy}) == UINT32_C(32)"
    )));

    // Wasm32 aggregate lowering consumes the same canonical layouts through
    // its independent reconstruction gate (validated during resolution).
}

#[test]
fn native_inheritance_executes_identically_at_o0_and_o2() {
    if !command_available("clang") {
        return;
    }
    let program = parse_ok(CORPUS);
    let generated = semaprax::codegen::emit_c(&program).unwrap();
    assert_native_main_exit_at_o0_o2(&generated, "inheritance", 6);
}

#[test]
fn native_zero_sized_upcasts_initialize_each_exact_physical_carrier() {
    if !command_available("clang") {
        return;
    }
    let source = r#"module test.inheritance.zero_sized;

@id("zero.empty")
class Empty {}

@id("zero.empty_child")
class EmptyChild : Empty {
    @id("zero.empty_child.marker") marker: i64,
}

@id("zero.only")
record ZeroOnly {
    @id("zero.only.empty") empty: [u8; 0],
}

@id("zero.base")
class ZeroBase {
    @id("zero.base.payload") payload: ZeroOnly,
}

@id("zero.child")
class ZeroChild : ZeroBase {
    @id("zero.child.marker") marker: i64,
}

@id("zero.touch_empty")
fn touch_empty(value: Empty) -> i64 { 1 }

@id("zero.touch_base")
fn touch_base(value: ZeroBase) -> i64 { 40 }

@id("app.main")
fn main() -> i64 {
    let direct = Empty {};
    let inherited: Empty = EmptyChild { marker: 1 };
    let erased: ZeroBase = ZeroChild {
        payload: ZeroOnly { empty: [] },
        marker: 2,
    };
    touch_empty(direct) + touch_empty(inherited) + touch_base(erased)
}
"#;
    let program = parse_ok(source);
    assert!(verify::verify(&program).is_empty());
    let generated = semaprax::codegen::emit_c(&program).unwrap();

    assert_eq!(
        generated
            .matches(".spx_empty_record_padding = UINT8_C(0);")
            .count(),
        2,
        "direct construction and empty-ancestor upcast must each initialize the frozen byte"
    );
    assert_eq!(
        generated
            .matches(".spx_zero_sized_record_carrier = UINT8_C(0);")
            .count(),
        2,
        "all-zero record construction and ancestor upcast must each initialize a C carrier"
    );
    assert_native_main_exit_at_o0_o2(&generated, "zero-sized-inheritance", 42);
}

#[test]
fn projected_zero_sized_nominal_keeps_its_exact_carrier_across_backends() {
    let source = r#"module test.zero_sized_projection;

@id("zero.projected")
record Zero {
    @id("zero.projected.empty") empty: [u8; 0],
}

@id("zero.holder")
record Holder {
    @id("zero.holder.zero") zero: Zero,
    @id("zero.holder.value") value: i64,
}

@id("zero.consume")
fn consume(value: Zero) -> i64 { 7 }

@id("app.main")
fn main() -> i64 {
    let holder = Holder {
        zero: Zero { empty: [] },
        value: 1,
    };
    let projected = consume(holder.zero) + consume(Holder {
        zero: Zero { empty: [] },
        value: 2,
    }.zero);
    let match_input = Holder {
        zero: Zero { empty: [] },
        value: 3,
    };
    let matched = match match_input {
        Holder { zero, value } => consume(zero) + value,
    };
    projected + matched
}
"#;
    let program = parse_ok(source);
    assert!(verify::verify(&program).is_empty());
    let generated = semaprax::codegen::emit_c(&program).unwrap();
    assert_eq!(
        generated
            .matches(".spx_zero_sized_record_carrier = UINT8_C(0);")
            .count(),
        6,
        "place, rvalue, and pattern projection must each materialize the exact nominal carrier"
    );
    if command_available("clang") {
        assert_native_main_exit_at_o0_o2(&generated, "zero-sized-projection", 24);
    }
    if command_available("node") {
        let id = unique_id();
        let output = std::env::temp_dir().join(format!("semaprax-zero-sized-web-{id}"));
        wasm::build_web(&program, &output).unwrap();
        let script = Path::new(env!("CARGO_MANIFEST_DIR")).join("scripts/verify-web.mjs");
        let result = Command::new("node")
            .arg(script)
            .arg(&output)
            .arg("24")
            .output();
        let _ = std::fs::remove_dir_all(&output);
        let result = result.unwrap();
        assert!(
            result.status.success(),
            "zero-sized projection node failed: {}",
            String::from_utf8_lossy(&result.stderr)
        );
        assert_eq!(String::from_utf8_lossy(&result.stdout).trim(), "24");
    }
}

const WASM_CORPUS: &str = r#"module test.inheritance.web;

@id("t2.animal")
class Animal {
    @id("t2.animal.tag")
    tag: bool,
    @id("t2.animal.legs")
    legs: i64,

    @id("t2.animal.describe")
    fn describe(self: Animal) -> i64
{
        self.legs
    }
}

@id("t2.dog")
class Dog : Animal {
    @id("t2.dog.bark")
    bark: i64,

    @id("t2.dog.describe")
    fn describe(self: Dog) -> i64
{
        super.describe() + self.bark
    }
}

@id("t2.puppy")
class Puppy : Dog {
    @id("t2.puppy.cute")
    cute: i64,

    @id("t2.puppy.score")
    fn score(self: Puppy) -> i64
{
        self.describe() + self.cute
    }
}

@id("app.main")
fn main() -> i64
{
    let d = Dog { tag: true, legs: 4, bark: 2 };
    let p = Puppy { tag: false, legs: 4, bark: 1, cute: 10 };
    let a: Animal = p;
    if d.describe() == 6 && a.describe() == 4 && p.score() == 15 { d.describe() } else { 0 }
}
"#;

#[test]
fn wasm_inheritance_matches_native_results_in_node() {
    if !command_available("node") {
        return;
    }
    let program = parse_ok(WASM_CORPUS);
    assert_eq!(format::canonical(&program), WASM_CORPUS);
    let id = unique_id();
    let output = std::env::temp_dir().join(format!("semaprax-inheritance-web-{id}"));
    wasm::build_web(&program, &output).unwrap();
    let script = Path::new(env!("CARGO_MANIFEST_DIR")).join("scripts/verify-web.mjs");
    let result = Command::new("node")
        .arg(script)
        .arg(&output)
        .arg("6")
        .output();
    let _ = std::fs::remove_dir_all(&output);
    let result = result.unwrap();
    assert!(
        result.status.success(),
        "node failed: {}",
        String::from_utf8_lossy(&result.stderr)
    );
    assert_eq!(String::from_utf8_lossy(&result.stdout).trim(), "6");
}

#[test]
fn graph_json_is_deterministic_additive_and_byte_identical_for_pre_feature_programs() {
    let program = parse_ok(CORPUS);
    let json = graph::to_json(&program).unwrap();
    let json_again = graph::to_json(&program).unwrap();
    assert_eq!(json, json_again, "graph JSON must be deterministic");
    assert!(json.contains("\"kind\":\"class\""), "{json}");
    assert!(json.contains("\"extends\":\"t1.animal\""), "{json}");
    assert!(json.contains("\"kind\":\"upcast\""), "{json}");
    assert!(json.contains("\"extends\":\"t1.animal\""), "{json}");

    // Programs without inheritance syntax stay byte-identical to pre-feature
    // compiler output: this revision digest is pinned from the feature base.
    let scalar = parse(
        "module t;\n@id(\"t.main\") fn main() -> i64 { 42 }\n",
        Path::new("scalar.spx"),
    )
    .unwrap();
    let scalar_json = graph::to_json(&scalar).unwrap();
    assert!(scalar_json.contains(
        "\"revision\":\"sha256:b5334af912d2f72a36e30d3f1a65f110cebcd4b8db2f2dd6fab05d1d2903f1ec\"",
    ));
    assert!(!scalar_json.contains("\"kind\":\"class\""));

    // The committed classes example (pre-inheritance OO surface) also stays
    // byte-identical; only inheritance programs gain new graph facts.
    let classes = parse(
        &std::fs::read_to_string(
            Path::new(env!("CARGO_MANIFEST_DIR")).join("examples/classes.spx"),
        )
        .unwrap(),
        Path::new("classes.spx"),
    )
    .unwrap();
    assert!(verify::verify(&classes).is_empty());
    let classes_json = graph::to_json(&classes).unwrap();
    assert!(classes_json.contains("\"kind\":\"class\""));
    assert!(!classes_json.contains("\"extends\""));
    assert!(!classes_json.contains("\"kind\":\"upcast\""));
    assert!(!classes_json.contains("\"kind\":\"super_method\""));
}

#[test]
fn cleanup_plans_stay_schema_identical_and_finalize_nothing_for_copy_corpora() {
    let resolved = resolved(CORPUS);
    let main = resolved
        .functions
        .iter()
        .find(|function| function.id.as_str() == "app.main")
        .unwrap();

    // The corpus classes are Copy-valued (all fields are i64/bool), so the
    // plan carries no liveness slots and no transfers; independent replay ran
    // inside hir::resolve and would have rejected any shape mismatch.
    assert_eq!(
        main.cleanup_plan.schema,
        semaprax::cleanup_plan::CLEANUP_PLAN_SCHEMA_V2
    );
    assert!(main.cleanup_plan.slots.is_empty());
    assert!(main.cleanup_plan.blocks.iter().all(|block| block
        .transitions
        .iter()
        .all(|transition| !matches!(transition, CleanupTransition::Transfer { .. }))));
    assert!(main
        .cleanup_plan
        .exits
        .iter()
        .all(|exit| exit.finalize_in_order.is_empty()));

    // The upcast itself is visible in the resolved body and contributes no
    // cleanup structure of its own.
    let main_ast_calls = format::canonical(&parse_ok(CORPUS));
    assert!(main_ast_calls.contains("let a: Animal = p;"));
}

#[test]
fn unknown_non_class_and_generic_parents_are_rejected_with_stable_codes() {
    let unknown = r#"module t;
@id("t.c") class C : Missing { @id("t.c.x") x: i64, }
@id("app.main") fn main() -> i64 { 0 }
"#;
    assert!(codes_from_verify(unknown).contains(&"SPX-T227"));

    let not_a_class = r#"module t;
@id("t.r") record R { @id("t.r.x") x: i64, }
@id("t.c") class C : R { @id("t.c.y") y: i64, }
@id("app.main") fn main() -> i64 { 0 }
"#;
    assert!(codes_from_verify(not_a_class).contains(&"SPX-T227"));

    let generic_parent = r#"module t;
@id("t.b") class B<T> { @id("t.b.x") x: T, }
@id("t.c") class C : B<i64> { @id("t.c.y") y: i64, }
@id("app.main") fn main() -> i64 { 0 }
"#;
    assert!(codes_from_verify(generic_parent).contains(&"SPX-T227"));
}

#[test]
fn inheritance_cycles_are_rejected_with_stable_code() {
    let mutual = r#"module t;
@id("t.a") class A : B { @id("t.a.x") x: i64, }
@id("t.b") class B : A { @id("t.b.y") y: i64, }
@id("app.main") fn main() -> i64 { 0 }
"#;
    assert!(codes_from_verify(mutual).contains(&"SPX-T228"));

    let self_cycle = r#"module t;
@id("t.a") class A : A { @id("t.a.x") x: i64, }
@id("app.main") fn main() -> i64 { 0 }
"#;
    assert!(codes_from_verify(self_cycle).contains(&"SPX-T228"));
}

#[test]
fn member_collisions_with_ancestors_are_rejected_with_stable_code() {
    let duplicate_field = r#"module t;
@id("t.a") class A { @id("t.a.x") x: i64, }
@id("t.b") class B : A { @id("t.b.x") x: i64, }
@id("app.main") fn main() -> i64 { 0 }
"#;
    assert!(codes_from_verify(duplicate_field).contains(&"SPX-T229"));

    let method_shadows_field = r#"module t;
@id("t.a") class A { @id("t.a.x") x: i64, }
@id("t.b") class B : A {
    @id("t.b.x")
    fn x(self: B) -> i64
{
        1
    }
}
@id("app.main") fn main() -> i64 { 0 }
"#;
    assert!(codes_from_verify(method_shadows_field).contains(&"SPX-T229"));
}

#[test]
fn override_signature_mismatch_is_rejected_with_stable_code() {
    let return_type_mismatch = r#"module t;
@id("t.a") class A {
    @id("t.a.f")
    fn f(self: A) -> i64
{
        1
    }
}
@id("t.b") class B : A {
    @id("t.b.f")
    fn f(self: B) -> bool
{
        true
    }
}
@id("app.main") fn main() -> i64 { 0 }
"#;
    assert!(codes_from_verify(return_type_mismatch).contains(&"SPX-T230"));

    let parameter_mismatch = r#"module t;
@id("t.a") class A {
    @id("t.a.f")
    fn f(self: A, amount: i64) -> i64
{
        amount
    }
}
@id("t.b") class B : A {
    @id("t.b.f")
    fn f(self: B, amount: bool) -> i64
{
        1
    }
}
@id("app.main") fn main() -> i64 { 0 }
"#;
    assert!(codes_from_verify(parameter_mismatch).contains(&"SPX-T230"));

    // An exact-signature override is admitted and dispatches statically.
    let exact_override = r#"module t;
@id("t.a") class A {
    @id("t.a.f")
    fn f(self: A) -> i64
{
        1
    }
}
@id("t.b") class B : A {
    @id("t.b.f")
    fn f(self: B) -> i64
{
        super.f() + 1
    }
}
@id("app.main") fn main() -> i64
{
    let b = B {};
    if b.f() == 2 { 42 } else { 0 }
}
"#;
    let program = parse_ok(exact_override);
    assert!(verify::verify(&program).is_empty());
    hir::resolve(&program).expect("exact override resolves");
}

#[test]
fn super_outside_an_override_is_rejected_with_stable_code() {
    let outside_class = r#"module t;
@id("t.a") class A {
    @id("t.a.f")
    fn f(self: A) -> i64
{
        1
    }
}
@id("t.top") fn top() -> i64
{
    super.f()
}
@id("app.main") fn main() -> i64 { 0 }
"#;
    assert!(codes_from_resolve(outside_class).contains(&"SPX-T231"));

    let parentless_class = r#"module t;
@id("t.a") class A {
    @id("t.a.f")
    fn f(self: A) -> i64
{
        super.f()
    }
}
@id("app.main") fn main() -> i64 { 0 }
"#;
    assert!(codes_from_resolve(parentless_class).contains(&"SPX-T231"));

    let unknown_super_method = r#"module t;
@id("t.a") class A { @id("t.a.x") x: i64, }
@id("t.b") class B : A {
    @id("t.b.f")
    fn f(self: B) -> i64
{
        super.missing()
    }
}
@id("app.main") fn main() -> i64 { 0 }
"#;
    assert!(codes_from_resolve(unknown_super_method).contains(&"SPX-T231"));
}

#[test]
fn upcast_to_non_ancestor_is_rejected_with_stable_code() {
    let unrelated_classes = r#"module t;
@id("t.a") class A { @id("t.a.x") x: i64, }
@id("t.b") class B { @id("t.b.y") y: i64, }
@id("app.main") fn main() -> i64
{
    let b = B { y: 1 };
    let a: A = b;
    a.x
}
"#;
    assert!(codes_from_resolve(unrelated_classes).contains(&"SPX-T232"));

    let scalar_target = r#"module t;
@id("t.a") class A { @id("t.a.x") x: i64, }
@id("app.main") fn main() -> i64
{
    let n: i64 = true;
    n
}
"#;
    let program = parse_ok(scalar_target);
    let errors = hir::resolve(&program)
        .expect_err("declared type must accept the value type or an ancestor")
        .into_iter()
        .map(|diagnostic| diagnostic.code)
        .collect::<Vec<_>>();
    assert!(errors.contains(&"SPX-T232"), "{errors:?}");
}

#[test]
fn upcast_discarding_owned_state_is_rejected_with_stable_code() {
    // The source-level slice closes string-bearing members outright, so the
    // resolver-level guard is exercised by resolving the same shape directly:
    // even without the earlier gate, an upcast whose child-declared suffix
    // carries owned state fails closed before any backend runs.
    let owned_suffix = r#"module t;
@id("t.a") class A { @id("t.a.x") x: i64, }
@id("t.b") class B : A { @id("t.b.name") name: string, }
@id("app.main") fn main() -> i64
{
    let b = B { x: 1, name: "rex" };
    let a: A = b;
    a.x
}
"#;
    assert!(codes_from_verify(owned_suffix).contains(&"SPX-T234"));
    assert!(codes_from_resolve(owned_suffix).contains(&"SPX-T233"));
}

#[test]
fn string_bearing_members_are_closed_with_stable_code() {
    let direct = r#"module t;
@id("t.a") class A { @id("t.a.name") name: string, }
@id("app.main") fn main() -> i64 { 0 }
"#;
    assert!(codes_from_verify(direct).contains(&"SPX-T234"));

    let transitive = r#"module t;
@id("t.r") record R { @id("t.r.name") name: string, }
@id("t.a") class A { @id("t.a.payload") payload: R, }
@id("app.main") fn main() -> i64 { 0 }
"#;
    assert!(codes_from_verify(transitive).contains(&"SPX-T234"));
}

#[test]
fn inherited_methods_upcast_drop_free_suffixes_through_calls() {
    // Inherited-method calls consume the receiver through the same guarded
    // prefix upcast; this corpus exercises it on every chain depth.
    let calls = r#"module t;
@id("t.a") class A {
    @id("t.a.f")
    fn f(self: A) -> i64
{
        self.x
    }
    @id("t.a.x") x: i64,
}
@id("t.b") class B : A {
    @id("t.b.mid") mid: i64,
}
@id("t.c") class C : B {
    @id("t.c.tail") tail: i64,
}
@id("app.main") fn main() -> i64
{
    let c = C { x: 7, mid: 0, tail: 0 };
    c.f()
}
"#;
    let program = parse_ok(calls);
    assert!(verify::verify(&program).is_empty());
    hir::resolve(&program).expect("drop-free inherited call resolves");
}

fn hex_identity(value: &str) -> String {
    use std::fmt::Write as _;
    let mut output = String::with_capacity(value.len() * 2);
    for byte in value.bytes() {
        write!(output, "{byte:02x}").unwrap();
    }
    output
}

fn assert_native_main_exit_at_o0_o2(generated: &str, label: &str, expected: i32) {
    let main_symbol = format!("spx_decl_{}", hex_identity("app.main"));
    let probe = format!(
        r#"
int main(void) {{
    struct spx_status_entry entries[UINT32_C(32)];
    struct spx_context context = {{0}};
    if (!spx_context_init(&context, UINT64_C(64), entries, UINT32_C(32), NULL, NULL, NULL)) return 10;
    int64_t result = 0;
    if ({main_symbol}(&context, &result) != SPX_STATUS_SUCCESS) return 11;
    return (int)result;
}}
"#,
    );
    for optimization in ["-O0", "-O2"] {
        let id = unique_id();
        let stem = format!("semaprax-{label}-{id}");
        let source = std::env::temp_dir().join(format!("{stem}.c"));
        let executable =
            std::env::temp_dir().join(format!("{stem}{}", std::env::consts::EXE_SUFFIX));
        std::fs::write(&source, format!("{generated}\n{probe}")).unwrap();
        let compiled = Command::new("clang")
            .args([
                "-std=c11",
                optimization,
                "-Wall",
                "-Wextra",
                "-Werror",
                "-DSPX_NO_ENTRY_WRAPPER",
            ])
            .arg(&source)
            .arg("-o")
            .arg(&executable)
            .output()
            .unwrap();
        if !compiled.status.success() {
            let _ = std::fs::remove_file(&source);
            let _ = std::fs::remove_file(&executable);
            panic!(
                "{label} C failed at {optimization}: {}",
                String::from_utf8_lossy(&compiled.stderr)
            );
        }
        let executed = Command::new(&executable).output().unwrap();
        let _ = std::fs::remove_file(&source);
        let _ = std::fs::remove_file(&executable);
        assert_eq!(
            executed.status.code(),
            Some(expected),
            "{label} program exited unexpectedly at {optimization}: {}",
            String::from_utf8_lossy(&executed.stderr)
        );
    }
}

fn command_available(name: &str) -> bool {
    Command::new(name)
        .arg("--version")
        .output()
        .map(|output| output.status.success())
        .unwrap_or(false)
}

fn unique_id() -> u64 {
    use std::sync::atomic::{AtomicU64, Ordering};
    static NEXT: AtomicU64 = AtomicU64::new(0);
    let id = NEXT.fetch_add(1, Ordering::Relaxed);
    (std::process::id() as u64) << 8 | id
}
