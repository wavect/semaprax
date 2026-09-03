use super::*;
#[path = "tests/static_protocol.rs"]
mod static_protocol;
fn checked_value_fixture() -> hir::ResolvedProgram {
    let program = parse(r#"module values;
@id("values.config") record Config { @id("values.config.value") value: i64, }
@id("values.envelope") record Envelope { @id("values.envelope.item") item: Config, }
@id("values.choice") variant Choice { @id("values.choice.some") Some { @id("values.choice.item") item: i64, }, @id("values.choice.none") None, }
@id("values.disconnected") fn disconnected() -> i64
    requires (Config { value: 1 }).value == 1
    ensures result >= 0
{
    let envelope = Envelope { item: Config { value: 42 } };
    match envelope { Envelope { item: Config { value: selected } } => selected, }
}
@id("values.variant") fn variant_value() -> i64 {
    let choice = Choice::Some { item: 42 };
    match choice { Choice::Some { item: selected } => selected, Choice::None {} => 0, }
}
@id("values.main") fn main() -> i64 { 0 }
"#, Path::new("values.spx")).unwrap();
    hir::resolve(&program).unwrap()
}
#[test]
fn nominal_values_outside_signatures_retain_exact_checked_type_facts() {
    let resolved = checked_value_fixture();
    assert!(resolved.functions.iter().all(
        |function| function.params.is_empty() && function.return_type == hir::ResolvedType::I64
    ));
    let retained =
        retained_signature_type_facts(&resolved.functions, &resolved.declarations).unwrap();
    assert_eq!(retained.len(), 3);
    for (id, kind) in [
        ("values.config", hir::DeclarationKind::Record),
        ("values.envelope", hir::DeclarationKind::Record),
        ("values.choice", hir::DeclarationKind::Variant),
    ] {
        let ty = hir::ResolvedType::Nominal {
            declaration: hir::DeclarationId::new(id.to_owned()),
            arguments: Vec::new(),
        };
        assert_eq!(
            retained.get(&ty.identity_key()),
            Some(&(kind, resolved.declarations.type_facts(&ty).unwrap()))
        );
    }
}

#[test]
fn retained_value_facts_keep_the_4096_cap_and_reject_missing_checked_facts() {
    let resolved = checked_value_fixture();
    let ty = hir::ResolvedType::Nominal {
        declaration: hir::DeclarationId::new("values.config".to_owned()),
        arguments: Vec::new(),
    };
    let facts = resolved.declarations.type_facts(&ty).unwrap();
    let mut retained = (0..MAX_DECLARATIONS)
        .map(|index| {
            (
                format!("occupied.{index}"),
                (hir::DeclarationKind::Record, facts.clone()),
            )
        })
        .collect();
    let error =
        retain_checked_nominal_type(&ty, &resolved.declarations, &mut retained).unwrap_err();
    assert!(is_named_limit(&error, "declarations"));
    assert_eq!(retained.len(), MAX_DECLARATIONS);
    let absent = hir::ResolvedType::Nominal {
        declaration: hir::DeclarationId::new("values.absent".to_owned()),
        arguments: Vec::new(),
    };
    let error = retain_checked_nominal_type(&absent, &resolved.declarations, &mut BTreeMap::new())
        .unwrap_err();
    assert!(error.iter().any(|error| error.code == "SPX-G173"));
}

#[test]
fn checked_value_cursor_traversal_enforces_visit_and_depth_limits() {
    let resolved = checked_value_fixture();
    let mut visits = MAX_CHECKED_VALUE_VISITS - 1;
    let mut retained = BTreeMap::new();
    retain_checked_value_types(
        CheckedValueNode::Type(&hir::ResolvedType::I64),
        &resolved.declarations,
        &mut retained,
        &mut visits,
    )
    .unwrap();
    assert_eq!(visits, MAX_CHECKED_VALUE_VISITS);
    let error = retain_checked_value_types(
        CheckedValueNode::Type(&hir::ResolvedType::I64),
        &resolved.declarations,
        &mut retained,
        &mut visits,
    )
    .unwrap_err();
    assert!(is_named_limit(&error, "checked_value_visits"));
    let function = resolved
        .functions
        .iter()
        .find(|function| function.id.as_str() == "values.main")
        .unwrap();
    let owner = hir::FunctionExecutionId::Monomorphic(function.id.clone());
    let mut expression = function.body.clone();
    for index in 0..=MAX_CHECKED_VALUE_DEPTH {
        expression = hir::ResolvedExpr {
            id: hir::ExpressionId::new(&owner, &format!("depth.{index}")),
            ty: hir::ResolvedType::I64,
            ownership: hir::OwnershipMode::Value,
            kind: hir::ResolvedExprKind::Block {
                statements: Vec::new(),
                tail: Box::new(expression),
            },
            span: Span::default(),
        };
    }
    let error = retain_checked_value_types(
        CheckedValueNode::Expression(&expression),
        &resolved.declarations,
        &mut retained,
        &mut 0,
    )
    .unwrap_err();
    assert!(is_named_limit(&error, "checked_value_depth"));
}

fn source(path: &str, source: &str) -> WorkspaceSource {
    WorkspaceSource {
        path: path.to_owned(),
        source: source.to_owned(),
    }
}

fn canonical_source(path: &str, source: &str) -> WorkspaceSource {
    let program = parse(source, Path::new(path)).expect("test source must parse");
    WorkspaceSource {
        path: path.to_owned(),
        source: format::canonical(&program),
    }
}

#[test]
fn scalar_linker_uses_real_provider_bodies_for_two_closures() {
    let provider = canonical_source(
        "lib/math.spx",
        r#"
module lib.math;

@id("lib.answer")
fn answer() -> i64 { 41 }
"#,
    );
    let app = canonical_source(
        "app/main.spx",
        r#"
module app.main;
use function @id("lib.answer") from lib.math as answer;

@id("app.main")
fn main() -> i64 { answer() + 1 }
"#,
    );
    let test = canonical_source(
        "test/main.spx",
        r#"
module test.main;
use function @id("lib.answer") from lib.math as answer;

@id("test.main")
fn main() -> i64 { answer() + 2 }
"#,
    );

    let (entry, test) = build_owned(vec![provider, app, test])
        .unwrap()
        .into_linked_scalar_programs("app.main", "test.main")
        .unwrap();
    let answer = hir::DeclarationId::new("lib.answer");
    let entry_answer = entry
        .functions
        .iter()
        .find(|function| function.id == answer)
        .unwrap();
    let test_answer = test
        .functions
        .iter()
        .find(|function| function.id == answer)
        .unwrap();
    let hir::ResolvedExprKind::Block { tail, .. } = &entry_answer.body.kind else {
        panic!("resolved provider body must retain its block");
    };
    assert!(matches!(tail.kind, hir::ResolvedExprKind::Int(41)));
    assert_eq!(entry_answer.body, test_answer.body);
    assert!(entry.functions.iter().all(|function| !function
        .id
        .as_str()
        .starts_with("workspace.synthetic.main.")));
    assert_eq!(entry.entrypoint.as_str(), "app.main");
    assert_eq!(test.entrypoint.as_str(), "test.main");
}

#[test]
fn project_web_roots_retain_selected_disconnected_call_closure_only() {
    let app = canonical_source(
        "app/main.spx",
        r#"
module app.main;

@id("app.main")
fn main() -> i64 { 0 }
"#,
    );
    let test = canonical_source(
        "test/main.spx",
        r#"
module test.main;

@id("test.main")
fn main() -> i64 { 0 }
"#,
    );
    let exports = canonical_source(
        "lib/exports.spx",
        r#"
module lib.exports;

@id("lib.helper")
fn helper(value: i64) -> i64 { value + 1 }

@id("lib.selected")
fn selected(value: i64) -> i64 { helper(value) }

@id("lib.unselected")
fn unselected(value: i64) -> i64 { value + 100 }
"#,
    );
    let build = build_owned(vec![app, test, exports]).unwrap();
    build
        .validate_entire_scalar_workspace("app.main", "test.main")
        .unwrap();
    let linked = build
        .linked_scalar_program_with_roots(
            "app.main",
            &["lib.selected".to_owned()],
            crate::project::ProjectProfile::ScalarV1,
        )
        .unwrap();
    let ids = linked
        .functions
        .iter()
        .map(|function| function.id.as_str())
        .collect::<BTreeSet<_>>();
    assert_eq!(
        ids,
        BTreeSet::from(["app.main", "lib.helper", "lib.selected"])
    );
    assert!(!ids.contains("lib.unselected"));
    assert!(!ids.contains("test.main"));
    hir::validate(&linked).unwrap();

    let error = build
        .linked_scalar_program_with_roots(
            "app.main",
            &["lib.absent".to_owned()],
            crate::project::ProjectProfile::ScalarV1,
        )
        .unwrap_err();
    assert_eq!(error[0].code, "SPX-W115");
    assert!(error[0]
        .message
        .contains("does not name an authenticated function"));
}

#[test]
fn owned_data_roots_retain_exact_private_aggregate_closure_and_authenticated_members() {
    let app = canonical_source(
        "app/main.spx",
        r#"
module app.main;

@id("app.main")
fn main() -> i64 { 0 }
"#,
    );
    let test = canonical_source(
        "test/main.spx",
        r#"
module test.main;

@id("test.main")
fn main() -> i64 { 0 }
"#,
    );
    let exports = canonical_source(
        "lib/exports.spx",
        r#"
module lib.exports;

@id("lib.payload")
record Payload {
    @id("lib.payload.value") value: i64,
}

@id("lib.wrapper")
record Wrapper {
    @id("lib.wrapper.payload") payload: Payload,
}

@id("lib.choice")
variant Choice {
    @id("lib.choice.ready") Ready {
        @id("lib.choice.ready.payload") payload: i64,
    },
    @id("lib.choice.empty") Empty,
}

@id("lib.unused")
record Unused {
    @id("lib.unused.value") value: bool,
}

@id("lib.helper")
fn helper(value: i64) -> i64 {
    let wrapper = Wrapper { payload: Payload { value } };
    let nested = match wrapper {
        Wrapper { payload: Payload { value: inner } } => inner,
    };
    let choice = Choice::Ready { payload: nested };
    match choice {
        Choice::Ready { payload } => payload,
        Choice::Empty {} => 0,
    }
}

@id("lib.selected")
fn selected(value: i64) -> i64 { helper(value) }

@id("lib.unselected")
fn unselected(value: i64) -> i64 {
    let ignored = Unused { value: true };
    value
}
"#,
    );
    let build = build_owned(vec![app, test, exports]).unwrap();
    let linked = build
        .linked_scalar_program_with_roots(
            "app.main",
            &["lib.selected".to_owned()],
            crate::project::ProjectProfile::OwnedDataApiV1,
        )
        .unwrap();
    let functions = linked
        .functions
        .iter()
        .map(|function| function.id.as_str())
        .collect::<BTreeSet<_>>();
    assert_eq!(
        functions,
        BTreeSet::from(["app.main", "lib.helper", "lib.selected"])
    );
    let authored_types = linked
        .types
        .iter()
        .map(|declaration| declaration.id.as_str())
        .filter(|id| !crate::prelude::is_compiler_owned_id(id))
        .collect::<BTreeSet<_>>();
    assert_eq!(
        authored_types,
        BTreeSet::from(["lib.choice", "lib.payload", "lib.wrapper"])
    );
    assert!(linked
        .declarations
        .declaration(&hir::DeclarationId::new("lib.choice.ready.payload"))
        .is_some());
    assert!(linked
        .declarations
        .declaration(&hir::DeclarationId::new("lib.unused"))
        .is_none());

    let mut hostile = build;
    hostile.hir.declarations.remove("lib.choice.ready.payload");
    let error = hostile
        .linked_scalar_program_with_roots(
            "app.main",
            &["lib.selected".to_owned()],
            crate::project::ProjectProfile::OwnedDataApiV1,
        )
        .unwrap_err();
    assert_eq!(error[0].code, "SPX-G173");
    assert!(error[0].message.contains("has no Phase-A fact"));
}

#[test]
fn useful_data_cross_module_borrowed_slice_boundary_remains_admitted() {
    let app = canonical_source(
        "app/main.spx",
        r#"
module app.main;

@id("app.main")
fn main() -> i64 { 0 }
"#,
    );
    let test = canonical_source(
        "test/main.spx",
        r#"
module test.main;

@id("test.main")
fn main() -> i64 { 0 }
"#,
    );
    let exports = canonical_source(
        "lib/exports.spx",
        r#"
module lib.exports;

@id("lib.byte-length")
fn byte_length(value: borrow Slice<u8>) -> usize {
    let alias = value;
    byte_len(alias)
}

@id("lib.byte-count")
fn byte_count(value: borrow Slice<u8>) -> usize {
    let mut index = 0usize;
    while index < byte_len(value) {
        index = index + 1usize;
        index < byte_len(value)
    }
    match byte_get(value, 0usize) {
        Option::Some { value: _ } => index,
        Option::None {} => index,
    }
}

@id("lib.unselected")
fn unselected(value: i64) -> i64 { value + 1 }
"#,
    );
    let build = build_owned(vec![app, test, exports]).unwrap();
    build
        .validate_entire_project_workspace(
            "app.main",
            "test.main",
            crate::project::ProjectProfile::UsefulDataV1,
        )
        .unwrap();
    build
        .linked_scalar_program_with_roots(
            "app.main",
            &["lib.byte-count".to_owned(), "lib.byte-length".to_owned()],
            crate::project::ProjectProfile::UsefulDataV1,
        )
        .unwrap();
    build
        .validate_entire_project_workspace(
            "app.main",
            "test.main",
            crate::project::ProjectProfile::LineCommandIoV1,
        )
        .unwrap();
}

#[test]
fn scalar_linker_is_identity_based_when_provider_display_names_match() {
    let left = canonical_source(
        "lib/left.spx",
        r#"
module lib.left;

@id("lib.left.value")
fn value() -> i64 { 20 }
"#,
    );
    let right = canonical_source(
        "lib/right.spx",
        r#"
module lib.right;

@id("lib.right.value")
fn value() -> i64 { 22 }
"#,
    );
    let app = canonical_source(
        "app/main.spx",
        r#"
module app.main;
use function @id("lib.left.value") from lib.left as left_value;
use function @id("lib.right.value") from lib.right as right_value;

@id("app.main")
fn main() -> i64 { left_value() + right_value() }
"#,
    );

    let linked = build_owned(vec![left, right, app])
        .unwrap()
        .linked_scalar_program("app.main")
        .unwrap();
    let left = hir::DeclarationId::new("lib.left.value");
    let right = hir::DeclarationId::new("lib.right.value");
    assert_eq!(
        linked.declarations.declaration(&left).unwrap().name,
        "value"
    );
    assert_eq!(
        linked.declarations.declaration(&right).unwrap().name,
        "value"
    );
    assert_eq!(
        linked
            .functions
            .iter()
            .filter(|function| function.name == "value")
            .count(),
        2
    );
    hir::validate(&linked).unwrap();
}

#[test]
fn scalar_linker_rejects_entry_and_test_main_signature_drift_before_linking() {
    let sources = || {
        [
            ("app/main.spx", "app.main", "app.main"),
            ("test/main.spx", "test.main", "test.main"),
        ]
        .into_iter()
        .map(|(path, module, id)| {
            canonical_source(
                path,
                &format!("module {module};\n\n@id(\"{id}\")\nfn main() -> i64 {{ 0 }}\n"),
            )
        })
        .collect::<Vec<_>>()
    };

    for module_name in ["app.main", "test.main"] {
        let mut build = build_owned(sources()).unwrap();
        build
            .hir
            .modules
            .iter_mut()
            .find(|module| module.module == module_name)
            .unwrap()
            .functions
            .iter_mut()
            .find(|function| function.name == "main")
            .unwrap()
            .return_type = hir::ResolvedType::Bool;
        let error = build
            .into_linked_scalar_programs("app.main", "test.main")
            .unwrap_err();
        assert_eq!(error[0].code, "SPX-G172");
        assert_eq!(
            error[0].message,
            "workspace scalar entry module `main` must have the exact signature fn main() -> i64"
        );
    }
}

#[test]
fn scalar_linker_ignores_disconnected_nonscalar_modules_and_rejects_provider_mains() {
    let app = canonical_source(
        "app/main.spx",
        r#"
module app.main;

@id("app.main")
fn main() -> i64 { 0 }
"#,
    );
    let test = canonical_source(
        "test/main.spx",
        r#"
module test.main;

@id("test.main")
fn main() -> i64 { 0 }
"#,
    );
    let disconnected = canonical_source(
        "other/record.spx",
        r#"
module other.record;

@id("other.record")
record Record { @id("other.record.value") value: i64, }

@id("other.value")
fn value() -> i64 { 0 }
"#,
    );
    let (entry, tests) = build_owned(vec![app.clone(), test, disconnected])
        .unwrap()
        .into_linked_scalar_programs("app.main", "test.main")
        .unwrap();
    assert_eq!(entry.module, "app.main");
    assert_eq!(tests.module, "test.main");
    assert!(entry.types.is_empty() && tests.types.is_empty());
    assert_eq!(entry.functions.len(), 1);
    assert_eq!(tests.functions.len(), 1);

    let provider = canonical_source(
        "lib/provider.spx",
        r#"
module lib.provider;

@id("lib.value")
fn value() -> i64 { 1 }

@id("lib.main")
fn main() -> i64 { value() }
"#,
    );
    let consumer = canonical_source(
        "app/main.spx",
        r#"
module app.main;
use function @id("lib.value") from lib.provider as value;

@id("app.main")
fn main() -> i64 { value() }
"#,
    );
    let error = build_owned(vec![provider, consumer])
        .unwrap()
        .linked_scalar_program("app.main")
        .unwrap_err();
    assert_eq!(error[0].code, "SPX-G172");
}

fn effect_edge_sources() -> Vec<WorkspaceSource> {
    let library = r#"
module lib.core;
permit { audit.write, network.read }

@id("lib.zero")
fn zero() -> i64 { 0 }

@id("lib.multi")
fn multi() -> i64 uses { audit.write, network.read } { 42 }
"#;
    let app = r#"
module app.main;
use function @id("lib.zero") from lib.core as zero;
use function @id("lib.multi") from lib.core as multi;
permit { audit.write, network.read }

@id("app.main")
fn main() -> i64 uses { audit.write, network.read } {
    zero() + multi()
}

@id("app.other")
fn other() -> i64 uses { audit.write, network.read } { 0 }
"#;
    vec![
        canonical_source("app/main.spx", app),
        canonical_source("lib/core.spx", library),
    ]
}

fn effect_edge_fixture() -> WorkspaceGraphBuild {
    build_owned(effect_edge_sources()).expect("effect-edge fixture must build")
}

fn parsed_sources(sources: &[WorkspaceSource]) -> Vec<Program> {
    sources
        .iter()
        .map(|source| {
            parse(&source.source, Path::new(&source.path))
                .expect("canonical workspace fixture must parse")
        })
        .collect()
}

fn identity_fact_sources() -> Vec<WorkspaceSource> {
    let library = r#"
module lib.core;

@id("lib.imported")
record Imported {
    @id("lib.imported.value")
    value: i64,
}

@id("lib.foreign")
record Foreign {
    @id("lib.foreign.value")
    value: i64,
}

@id("lib.answer")
fn answer() -> i64 { 42 }
"#;
    let app = r#"
module app.identity;
use function @id("lib.answer") from lib.core as answer;
use type @id("lib.imported") from lib.core as Imported;

@id("app.record")
record Record {
    value: i64,
}

@id("app.choice")
variant Choice {
    Number { value: i64, },
}

@id("app.explicit_variant")
variant ExplicitVariant {
    @id("app.explicit_variant.ready")
    Ready {
        @id("app.explicit_variant.ready.code")
        code: i64,
    },
}

@id("app.token")
resource Token {
    @id("app.token.drop")
    drop trivial;
}

@id("app.host")
interface Host permits {} {
    @id("app.host.observe")
    import fn observe(value: own Token) -> unit
        effects {}
        failure infallible
        consumes value always;
}

fn helper() -> i64 { answer() }

@id("app.main")
fn main() -> i64 { helper() }
"#;
    vec![
        canonical_source("app/identity.spx", app),
        canonical_source("lib/core.spx", library),
    ]
}

fn assert_identity_shape_error(
    mut mutate: impl FnMut(&mut BTreeMap<String, WorkspaceDeclarationFact>),
) {
    let sources = identity_fact_sources();
    let programs = parsed_sources(&sources);
    let build = build_owned(sources).expect("identity fixture must build");
    let mut facts = expected_declaration_facts(&programs).unwrap();
    mutate(&mut facts);
    let error = validate_retained_declaration_shapes(&build.hir.modules, &facts)
        .expect_err("mutated identity fact must fail closed");
    assert_eq!(error[0].code, "SPX-G173");
    assert_eq!(
        error[0].message,
        "retained workspace declaration shape disagrees with authored identity facts"
    );
}

fn assert_resolved_identity_error(
    mut mutate: impl FnMut(&str, &mut Program, &[Program]),
    expected: &str,
) {
    let sources = identity_fact_sources();
    let programs = parsed_sources(&sources);
    let build = build_owned(sources).expect("identity fixture must build");
    let resolved_modules = {
        let authored = index_authored(&programs).unwrap();
        programs
            .iter()
            .map(|program| {
                let mut synthetic = synthetic_program(program, &authored, &programs).unwrap();
                mutate(&program.module, &mut synthetic, &programs);
                (
                    program.module.clone(),
                    hir::resolve(&synthetic).expect("mutated identity HIR must resolve"),
                )
            })
            .collect::<Vec<_>>()
    };
    let error = workspace_declaration_facts(&resolved_modules, &build.hir.modules, &programs)
        .expect_err("mutated resolved identity must fail closed");
    assert_eq!(error[0].code, "SPX-G173");
    assert_eq!(error[0].message, expected);
}

fn assert_effect_validation_error(mut mutate: impl FnMut(&mut Vec<WorkspaceEdge>), expected: &str) {
    let mut build = effect_edge_fixture();
    mutate(&mut build.edges);
    let error = validate_effect_and_capability_edges(&build.hir.modules, &build.edges)
        .expect_err("mutated proof must fail closed");
    assert_eq!(error[0].code, "SPX-G173");
    assert_eq!(error[0].message, expected);
}

fn assert_call_validation_error(mut mutate: impl FnMut(&mut Vec<WorkspaceEdge>)) {
    let sources = effect_edge_sources();
    let programs = parsed_sources(&sources);
    let mut build = build_owned(sources).expect("call-edge fixture must build");
    mutate(&mut build.edges);
    let error = validate_retained_facts(&programs, &build.hir.modules, &build.edges)
        .expect_err("mutated call proof must fail closed");
    assert_eq!(error[0].code, "SPX-G173");
    assert_eq!(
        error[0].message,
        "emitted workspace call edges disagree with authenticated AST/HIR occurrences"
    );
}

fn mutate_multi_call_family(
    edges: &mut [WorkspaceEdge],
    mut mutate: impl FnMut(&mut WorkspaceEdge),
) {
    let call = edges
        .iter()
        .find(|edge| edge.kind == "call" && edge.target == "lib.multi")
        .expect("multi-effect call must exist")
        .clone();
    let family = edges
        .iter()
        .enumerate()
        .filter(|(_, edge)| {
            matches!(edge.kind, "call" | "effect_requirement")
                && CallOccurrenceKey::from_edge(edge) == CallOccurrenceKey::from_edge(&call)
        })
        .map(|(index, _)| index)
        .collect::<Vec<_>>();
    assert_eq!(family.len(), 3, "one call must own exactly two effects");
    for index in family {
        mutate(&mut edges[index]);
    }
}

#[test]
fn zero_and_multi_effect_targets_replay_exact_occurrences() {
    let build = effect_edge_fixture();
    validate_effect_and_capability_edges(&build.hir.modules, &build.edges).unwrap();

    let zero_call = build
        .edges
        .iter()
        .find(|edge| edge.kind == "call" && edge.target == "lib.zero")
        .expect("zero-effect call must exist");
    assert!(!build.edges.iter().any(|edge| {
        edge.kind == "effect_requirement"
            && CallOccurrenceKey::from_edge(edge) == CallOccurrenceKey::from_edge(zero_call)
    }));

    let multi_call = build
        .edges
        .iter()
        .find(|edge| edge.kind == "call" && edge.target == "lib.multi")
        .expect("multi-effect call must exist");
    let effects = build
        .edges
        .iter()
        .filter(|edge| {
            edge.kind == "effect_requirement"
                && CallOccurrenceKey::from_edge(edge) == CallOccurrenceKey::from_edge(multi_call)
        })
        .map(|edge| edge.target.as_str())
        .collect::<BTreeSet<_>>();
    assert_eq!(effects, BTreeSet::from(["audit.write", "network.read"]));
}

#[test]
fn altered_effect_requirement_sets_fail_closed() {
    const SET_MISMATCH: &str =
        "workspace call effect requirements disagree with retained target HIR";

    assert_effect_validation_error(
        |edges| {
            let index = edges
                .iter()
                .position(|edge| edge.kind == "effect_requirement" && edge.target == "audit.write")
                .unwrap();
            edges.remove(index);
        },
        SET_MISMATCH,
    );
    assert_effect_validation_error(
        |edges| {
            let mut extra = edges
                .iter()
                .find(|edge| edge.kind == "effect_requirement")
                .unwrap()
                .clone();
            extra.target = "storage.read".to_owned();
            edges.push(extra);
        },
        SET_MISMATCH,
    );
    assert_effect_validation_error(
        |edges| {
            edges
                .iter_mut()
                .find(|edge| edge.kind == "effect_requirement" && edge.target == "audit.write")
                .unwrap()
                .target = "storage.read".to_owned();
        },
        SET_MISMATCH,
    );
    assert_effect_validation_error(
        |edges| {
            let duplicate = edges
                .iter()
                .find(|edge| edge.kind == "effect_requirement")
                .unwrap()
                .clone();
            edges.push(duplicate);
        },
        "workspace call effect requirement is duplicated",
    );
}

#[test]
fn coupled_call_effect_key_substitutions_fail_exact_reconstruction() {
    assert_call_validation_error(|edges| {
        mutate_multi_call_family(edges, |edge| edge.site = "requires");
    });
    assert_call_validation_error(|edges| {
        let expression = hir::workspace_expression_identity(
            &hir::DeclarationId::new("app.main".to_owned()),
            "body.tail.left",
        );
        mutate_multi_call_family(edges, |edge| edge.expression = expression.clone());
    });
    assert_call_validation_error(|edges| {
        mutate_multi_call_family(edges, |edge| {
            edge.ast_path = "body.tail.left".to_owned();
        });
    });
    assert_call_validation_error(|edges| {
        mutate_multi_call_family(edges, |edge| edge.alias = "zero".to_owned());
    });
    assert_call_validation_error(|edges| {
        mutate_multi_call_family(edges, |edge| edge.ordinal = 0);
    });
    assert_call_validation_error(|edges| {
        mutate_multi_call_family(edges, |edge| edge.caller = "app.other".to_owned());
    });
}

#[test]
fn missing_extra_and_duplicate_call_edges_fail_exact_reconstruction() {
    assert_call_validation_error(|edges| {
        let index = edges
            .iter()
            .position(|edge| edge.kind == "call" && edge.target == "lib.multi")
            .unwrap();
        edges.remove(index);
    });
    assert_call_validation_error(|edges| {
        let mut extra = edges
            .iter()
            .find(|edge| edge.kind == "call" && edge.target == "lib.multi")
            .unwrap()
            .clone();
        extra.expression = hir::workspace_expression_identity(
            &hir::DeclarationId::new("app.main".to_owned()),
            "body.extra",
        );
        extra.ast_path = "body.extra".to_owned();
        extra.ordinal = 2;
        edges.push(extra);
    });
    assert_call_validation_error(|edges| {
        let duplicate = edges
            .iter()
            .find(|edge| edge.kind == "call" && edge.target == "lib.multi")
            .unwrap()
            .clone();
        edges.push(duplicate);
    });
}

#[test]
fn altered_capability_authority_facts_fail_closed() {
    const MISMATCH: &str =
        "workspace capability-authority edges disagree with retained module permits";

    assert_effect_validation_error(
        |edges| {
            let index = edges
                .iter()
                .position(|edge| edge.kind == "capability_authority" && edge.caller == "app.main")
                .unwrap();
            edges.remove(index);
        },
        MISMATCH,
    );
    assert_effect_validation_error(
        |edges| {
            let mut extra = edges
                .iter()
                .find(|edge| edge.kind == "capability_authority" && edge.caller == "app.main")
                .unwrap()
                .clone();
            extra.target = "storage.read".to_owned();
            extra.expression = "permit.2".to_owned();
            extra.ast_path = "permit.2".to_owned();
            extra.ordinal = 2;
            edges.push(extra);
        },
        MISMATCH,
    );
    assert_effect_validation_error(
        |edges| {
            let indexes = edges
                .iter()
                .enumerate()
                .filter(|(_, edge)| {
                    edge.kind == "capability_authority" && edge.caller == "app.main"
                })
                .map(|(index, _)| index)
                .collect::<Vec<_>>();
            assert_eq!(indexes.len(), 2);
            let first = edges[indexes[0]].target.clone();
            edges[indexes[0]].target = edges[indexes[1]].target.clone();
            edges[indexes[1]].target = first;
        },
        MISMATCH,
    );
    assert_effect_validation_error(
        |edges| {
            edges
                .iter_mut()
                .find(|edge| edge.kind == "capability_authority" && edge.caller == "app.main")
                .unwrap()
                .target = "storage.read".to_owned();
        },
        MISMATCH,
    );
}

#[test]
fn zero_effect_generic_template_has_retained_caller_authority() {
    let library = r#"
module lib.core;

@id("lib.zero")
fn zero() -> i64 { 0 }
"#;
    let app = r#"
module app.main;
use function @id("lib.zero") from lib.core as zero;

@id("app.keep")
fn keep<T>(value: T) -> T {
    let observed = zero();
    if observed == 0 { value } else { value }
}

@id("app.main")
fn main() -> i64 { keep<i64>(42) }
"#;
    let build = build_owned(vec![
        canonical_source("app/main.spx", app),
        canonical_source("lib/core.spx", library),
    ])
    .unwrap();
    let call = build
        .edges
        .iter()
        .find(|edge| edge.kind == "call" && edge.caller == "app.keep")
        .expect("authored template call must be retained");
    assert_eq!(call.target, "lib.zero");
    validate_effect_and_capability_edges(&build.hir.modules, &build.edges).unwrap();
}

#[test]
fn indexed_effect_proof_replays_many_multi_effect_calls() {
    const CALLS: usize = 128;
    let library = r#"
module lib.core;
permit { audit.write, network.read }

@id("lib.multi")
fn multi() -> i64 uses { audit.write, network.read } { 42 }
"#;
    let mut app = String::from(
        r#"
module app.main;
use function @id("lib.multi") from lib.core as multi;
permit { audit.write, network.read }

@id("app.main")
fn main() -> i64 uses { audit.write, network.read } {
"#,
    );
    for index in 0..CALLS {
        app.push_str(&format!("    let value_{index} = multi();\n"));
    }
    app.push_str("    0\n}\n");

    let build = build_owned(vec![
        canonical_source("app/main.spx", &app),
        canonical_source("lib/core.spx", library),
    ])
    .unwrap();
    assert_eq!(
        build
            .edges
            .iter()
            .filter(|edge| edge.kind == "call" && edge.target == "lib.multi")
            .count(),
        CALLS
    );
    assert_eq!(
        build
            .edges
            .iter()
            .filter(|edge| edge.kind == "effect_requirement")
            .count(),
        CALLS * 2
    );
    validate_effect_and_capability_edges(&build.hir.modules, &build.edges).unwrap();
}

#[test]
fn indexed_proof_replays_many_permits_and_zero_effect_calls() {
    const PERMITS: usize = 96;
    const CALLS: usize = 96;
    let permits = (0..PERMITS)
        .map(|index| format!("capability.effect_{index}"))
        .collect::<Vec<_>>()
        .join(", ");
    let library = format!(
        "module lib.core;\npermit {{ {permits} }}\n\n@id(\"lib.zero\")\nfn zero() -> i64 {{ 0 }}\n"
    );
    let mut app = format!(
            "module app.main;\nuse function @id(\"lib.zero\") from lib.core as zero;\npermit {{ {permits} }}\n\n@id(\"app.main\")\nfn main() -> i64 {{\n"
        );
    for index in 0..CALLS {
        app.push_str(&format!("    let value_{index} = zero();\n"));
    }
    app.push_str("    0\n}\n");

    let build = build_owned(vec![
        canonical_source("app/main.spx", &app),
        canonical_source("lib/core.spx", &library),
    ])
    .unwrap();
    assert_eq!(
        build
            .edges
            .iter()
            .filter(|edge| edge.kind == "call")
            .count(),
        CALLS
    );
    assert_eq!(
        build
            .edges
            .iter()
            .filter(|edge| edge.kind == "capability_authority")
            .count(),
        PERMITS * 2
    );
    assert!(!build
        .edges
        .iter()
        .any(|edge| edge.kind == "effect_requirement"));
    validate_effect_and_capability_edges(&build.hir.modules, &build.edges).unwrap();
}

#[test]
fn canonical_use_parses_formats_and_single_file_rejects() {
    let text = "module app.main;\nuse function @id(\"lib.answer\") from lib.core as answer;\n\n@id(\"app.main\")\nfn main() -> i64\n{\n    answer()\n}\n";
    let program = parse(text, Path::new("app/main.spx")).unwrap();
    assert_eq!(format::canonical(&program), text);
    let error = hir::resolve(&program).expect_err("single-file HIR must reject workspace use");
    assert_eq!(error[0].code, "SPX-G172");
    assert_eq!(
        error[0].message,
        "source module imports require Workspace Semantic Graph resolution"
    );
}

#[test]
fn scalar_cross_file_call_resolves_once_and_reconstructs_edge() {
    let app = "module app.main;\nuse function @id(\"lib.answer\") from lib.core as answer;\n\n@id(\"app.main\")\nfn main() -> i64\n{\n    answer()\n}\n";
    let library = "module lib.core;\n\n@id(\"lib.answer\")\nfn answer() -> i64\n{\n    42\n}\n";
    let build = build_owned(vec![
        source("lib/core.spx", library),
        source("app/main.spx", app),
    ])
    .unwrap();
    assert_eq!(build.hir.modules.len(), 2);
    let app_hir = build
        .hir
        .modules
        .iter()
        .find(|module| module.module == "app.main")
        .unwrap();
    let lib_hir = build
        .hir
        .modules
        .iter()
        .find(|module| module.module == "lib.core")
        .unwrap();
    assert_eq!(app_hir.functions.len(), 1);
    assert_eq!(app_hir.functions[0].id.as_str(), "app.main");
    assert_eq!(lib_hir.functions.len(), 1);
    assert_eq!(lib_hir.functions[0].id.as_str(), "lib.answer");
    assert!(build.hir.shared_prelude_ids.contains(prelude::OPTION_ID));
    assert!(build.edges.iter().any(|edge| edge.kind == "call"
        && edge.caller == "app.main"
        && edge.target == "lib.answer"));
}

#[test]
fn module_kind_alias_and_cycle_confusion_fail_closed() {
    let a = "module a;\nuse function @id(\"b.value\") from b as value;\n\n@id(\"a.main\")\nfn main() -> i64\n{\n    value()\n}\n";
    let b = "module b;\nuse function @id(\"a.main\") from a as other;\n\n@id(\"b.value\")\nfn value() -> i64\n{\n    other()\n}\n";
    let error = build_owned(vec![source("a.spx", a), source("b.spx", b)])
        .err()
        .expect("cycle must fail");
    assert_eq!(error[0].code, "SPX-G172");
    assert!(error[0].message.contains("a -> b -> a"));
}

#[test]
fn file_limit_fails_before_parse() {
    let error = build_owned(vec![source("a.spx", "not parsed")])
        .err()
        .expect("one file is outside the admitted domain");
    assert_eq!(error[0].code, "SPX-G170");
    assert_eq!(
        error[0].message,
        "Workspace Semantic Graph requires 2..16 source files"
    );
}

#[test]
fn repeated_calls_preserve_contract_body_paths_and_root_local_ordinals() {
    let library = r#"
module lib.core;

@id("lib.flag")
fn flag() -> bool { true }

@id("lib.answer")
fn answer() -> i64 { 42 }
"#;
    let app = r#"
module app.main;
use function @id("lib.flag") from lib.core as flag;
use function @id("lib.answer") from lib.core as answer;

@id("app.main")
fn main() -> i64
    requires flag()
    requires flag()
    ensures flag()
    ensures flag()
{
    answer() + answer() + answer()
}

"#;

    let build = build_owned(vec![
        canonical_source("app/main.spx", app),
        canonical_source("lib/core.spx", library),
    ])
    .unwrap();
    let calls = build
        .edges
        .iter()
        .filter(|edge| edge.kind == "call" && edge.caller == "app.main")
        .map(|edge| {
            (
                edge.site,
                edge.ast_path.as_str(),
                edge.ordinal,
                edge.target.as_str(),
            )
        })
        .collect::<BTreeSet<_>>();

    assert_eq!(
        calls,
        BTreeSet::from([
            ("requires", "requires.0", 0, "lib.flag"),
            ("requires", "requires.1", 0, "lib.flag"),
            ("body", "body.tail.left.left", 0, "lib.answer"),
            ("body", "body.tail.left.right", 1, "lib.answer"),
            ("body", "body.tail.right", 2, "lib.answer"),
            ("ensures", "ensures.0", 0, "lib.flag"),
            ("ensures", "ensures.1", 0, "lib.flag"),
        ])
    );
}

#[test]
fn interleaved_template_sites_are_authored_once_across_two_materializations() {
    let library = r#"
module lib.core;

@id("lib.answer")
fn answer() -> i64 { 42 }
"#;
    let app = r#"
module app.main;
use function @id("lib.answer") from lib.core as answer;

@id("app.before")
fn before() -> i64 { answer() }

@id("app.keep")
fn keep<T>(value: T) -> T {
    let observed = answer();
    if observed == 42 { value } else { value }
}

@id("app.after")
fn after() -> i64 { answer() }

@id("app.main")
fn main() -> i64 {
    let number = keep<i64>(before());
    if keep<bool>(true) { number + after() } else { 0 }
}
"#;

    let build = build_owned(vec![
        canonical_source("app/main.spx", app),
        canonical_source("lib/core.spx", library),
    ])
    .unwrap();
    let app_hir = build
        .hir
        .modules
        .iter()
        .find(|module| module.module == "app.main")
        .unwrap();
    assert_eq!(app_hir.function_templates.len(), 1);
    assert_eq!(app_hir.function_templates[0].id.as_str(), "app.keep");
    assert_eq!(app_hir.function_instances.len(), 2);
    assert!(app_hir
        .function_instances
        .iter()
        .all(|instance| instance.template.as_str() == "app.keep"));

    let call_sites = build
        .edges
        .iter()
        .filter(|edge| edge.kind == "call" && edge.target == "lib.answer")
        .map(|edge| (edge.caller.as_str(), edge.ast_path.as_str()))
        .collect::<BTreeSet<_>>();
    assert_eq!(
        call_sites,
        BTreeSet::from([
            ("app.after", "body.tail"),
            ("app.before", "body.tail"),
            ("app.keep", "body.s0.value"),
        ])
    );
    assert_eq!(
        build
            .edges
            .iter()
            .filter(|edge| edge.kind == "call"
                && edge.caller == "app.keep"
                && edge.target == "lib.answer")
            .count(),
        1,
        "materialized instances must not duplicate one authored template site"
    );
}

#[test]
fn automatic_call_owner_retains_automatic_identity_origin() {
    let library = r#"
module lib.core;

@id("lib.answer")
fn answer() -> i64 { 42 }
"#;
    let app = r#"
module app.main;
use function @id("lib.answer") from lib.core as answer;

fn helper() -> i64 { answer() }

@id("app.main")
fn main() -> i64 { helper() }
"#;

    let build = build_owned(vec![
        canonical_source("app/main.spx", app),
        canonical_source("lib/core.spx", library),
    ])
    .unwrap();
    let call = build
        .edges
        .iter()
        .find(|edge| edge.kind == "call" && edge.target == "lib.answer")
        .expect("automatic helper call must be retained");
    let fact = build
        .hir
        .declarations
        .get(&call.caller)
        .expect("automatic caller must have one declaration fact");
    assert_eq!(fact.origin, hir::IdentityOrigin::Automatic);
    assert_eq!(fact.path.as_deref(), Some("app/main.spx"));
    assert_eq!(fact.module.as_deref(), Some("app.main"));
}

#[test]
fn identity_facts_preserve_authored_origins_parents_and_exact_prelude() {
    let build = build_owned(identity_fact_sources()).unwrap();
    let facts = &build.hir.declarations;
    let assert_fact =
        |id: &str, kind: hir::DeclarationKind, origin: hir::IdentityOrigin, owner: Option<&str>| {
            let fact = facts
                .get(id)
                .unwrap_or_else(|| panic!("missing fact `{id}`"));
            assert_eq!(fact.kind, kind, "kind for `{id}`");
            assert_eq!(fact.origin, origin, "origin for `{id}`");
            assert_eq!(fact.owner.as_deref(), owner, "owner for `{id}`");
            assert_eq!(fact.path.as_deref(), Some("app/identity.spx"));
            assert_eq!(fact.module.as_deref(), Some("app.identity"));
        };

    assert_fact(
        "auto:app.identity.helper",
        hir::DeclarationKind::Function,
        hir::IdentityOrigin::Automatic,
        None,
    );
    assert_fact(
        "auto:field:app.record.value",
        hir::DeclarationKind::Field,
        hir::IdentityOrigin::Automatic,
        Some("app.record"),
    );
    assert_fact(
        "auto:case:app.choice.Number",
        hir::DeclarationKind::VariantCase,
        hir::IdentityOrigin::Automatic,
        Some("app.choice"),
    );
    assert_fact(
        "auto:case-field:auto:case:app.choice.Number.value",
        hir::DeclarationKind::CaseField,
        hir::IdentityOrigin::Automatic,
        Some("auto:case:app.choice.Number"),
    );
    assert_fact(
        "app.explicit_variant",
        hir::DeclarationKind::Variant,
        hir::IdentityOrigin::Explicit,
        None,
    );
    assert_fact(
        "app.explicit_variant.ready",
        hir::DeclarationKind::VariantCase,
        hir::IdentityOrigin::Explicit,
        Some("app.explicit_variant"),
    );
    assert_fact(
        "app.explicit_variant.ready.code",
        hir::DeclarationKind::CaseField,
        hir::IdentityOrigin::Explicit,
        Some("app.explicit_variant.ready"),
    );
    assert_fact(
        "app.token.drop",
        hir::DeclarationKind::ResourceDrop,
        hir::IdentityOrigin::Explicit,
        Some("app.token"),
    );
    assert_fact(
        "app.host",
        hir::DeclarationKind::Interface,
        hir::IdentityOrigin::Explicit,
        None,
    );
    assert_fact(
        "app.host.observe",
        hir::DeclarationKind::Import,
        hir::IdentityOrigin::Explicit,
        Some("app.host"),
    );

    let compiler = facts
        .iter()
        .filter(|(_, fact)| fact.origin == hir::IdentityOrigin::CompilerOwned)
        .map(|(id, fact)| {
            assert_eq!(fact.path, None, "compiler fact path for `{id}`");
            assert_eq!(fact.module, None, "compiler fact module for `{id}`");
            id.as_str()
        })
        .collect::<BTreeSet<_>>();
    let expected = prelude::all_ids().into_iter().collect::<BTreeSet<_>>();
    assert_eq!(compiler, expected);
    assert_eq!(build.hir.shared_prelude_ids, expected);
}

#[test]
fn imported_stubs_and_synthetic_mains_are_not_retained() {
    let build = build_owned(identity_fact_sources()).unwrap();
    let app = build
        .hir
        .modules
        .iter()
        .find(|module| module.module == "app.identity")
        .unwrap();
    let library = build
        .hir
        .modules
        .iter()
        .find(|module| module.module == "lib.core")
        .unwrap();
    let app_functions = app
        .functions
        .iter()
        .map(|function| function.id.as_str())
        .collect::<BTreeSet<_>>();
    let library_functions = library
        .functions
        .iter()
        .map(|function| function.id.as_str())
        .collect::<BTreeSet<_>>();
    assert_eq!(
        app_functions,
        BTreeSet::from(["app.main", "auto:app.identity.helper"])
    );
    assert_eq!(library_functions, BTreeSet::from(["lib.answer"]));
    assert!(!build.hir.declarations.contains_key("lib.answer.stub"));
    assert!(!build
        .hir
        .declarations
        .contains_key("workspace.synthetic.main.lib.core"));
    let imported = build.hir.declarations.get("lib.answer").unwrap();
    assert_eq!(imported.path.as_deref(), Some("lib/core.spx"));
    assert_eq!(imported.module.as_deref(), Some("lib.core"));
}

#[test]
fn missing_or_substituted_identity_shape_facts_fail_closed() {
    assert_identity_shape_error(|facts| {
        facts.remove("auto:app.identity.helper");
    });
    assert_identity_shape_error(|facts| {
        facts.get_mut("app.record").unwrap().kind = hir::DeclarationKind::Interface;
    });
    assert_identity_shape_error(|facts| {
        facts.get_mut("auto:field:app.record.value").unwrap().owner = Some("app.choice".to_owned());
    });
    assert_identity_shape_error(|facts| {
        facts.get_mut("app.record").unwrap().path = Some("wrong/path.spx".to_owned());
    });
    assert_identity_shape_error(|facts| {
        facts.get_mut("app.record").unwrap().module = Some("wrong.module".to_owned());
    });
}

#[test]
fn substituted_identity_origin_disagrees_with_retained_hir() {
    let sources = identity_fact_sources();
    let programs = parsed_sources(&sources);
    let build = build_owned(sources).unwrap();
    let synthetic_modules = {
        let authored = index_authored(&programs).unwrap();
        programs
            .iter()
            .map(|program| {
                let synthetic = synthetic_program(program, &authored, &programs).unwrap();
                (
                    program.module.clone(),
                    hir::resolve(&synthetic).expect("synthetic identity fixture must resolve"),
                )
            })
            .collect::<Vec<_>>()
    };
    let mut substituted = programs.clone();
    substituted
        .iter_mut()
        .find(|program| program.module == "app.identity")
        .unwrap()
        .functions
        .iter_mut()
        .find(|function| function.stable_id == "auto:app.identity.helper")
        .unwrap()
        .explicit_id = true;

    let error = workspace_declaration_facts(&synthetic_modules, &build.hir.modules, &substituted)
        .expect_err("origin substitution must fail closed");
    assert_eq!(error[0].code, "SPX-G173");
    assert_eq!(
        error[0].message,
        "authored workspace declaration facts disagree with retained HIR"
    );
}

#[test]
fn independent_prelude_map_detects_kind_and_owner_substitutions() {
    let build = build_owned(identity_fact_sources()).unwrap();
    let actual = build
        .hir
        .declarations
        .iter()
        .filter(|(_, fact)| fact.origin == hir::IdentityOrigin::CompilerOwned)
        .map(|(id, fact)| (id.clone(), fact.clone()))
        .collect::<BTreeMap<_, _>>();
    let expected = expected_compiler_declaration_facts().unwrap();
    assert_eq!(actual, expected);

    let root_id = expected
        .iter()
        .find(|(_, fact)| fact.owner.is_none())
        .map(|(id, _)| id.clone())
        .expect("prelude must have a root declaration");
    let mut wrong_kind = expected.clone();
    wrong_kind.get_mut(&root_id).unwrap().kind = hir::DeclarationKind::Function;
    assert_ne!(actual, wrong_kind);

    let child_id = expected
        .iter()
        .find(|(_, fact)| fact.owner.is_some())
        .map(|(id, _)| id.clone())
        .expect("prelude must have a child declaration");
    let mut wrong_owner = expected;
    wrong_owner.get_mut(&child_id).unwrap().owner = Some("wrong.prelude.owner".to_owned());
    assert_ne!(actual, wrong_owner);
}

#[test]
fn rogue_and_nonimported_foreign_resolved_roots_fail_closed() {
    assert_resolved_identity_error(
        |module, synthetic, programs| {
            if module != "app.identity" {
                return;
            }
            let mut rogue = programs
                .iter()
                .find(|program| program.module == "app.identity")
                .unwrap()
                .functions
                .iter()
                .find(|function| function.name == "helper")
                .unwrap()
                .clone();
            rogue.stable_id = "rogue.function".to_owned();
            rogue.explicit_id = true;
            rogue.name = "rogue".to_owned();
            synthetic.functions.push(rogue);
        },
        "resolved workspace declaration has an unauthenticated synthetic or rogue root",
    );
    assert_resolved_identity_error(
        |module, synthetic, programs| {
            if module != "app.identity" {
                return;
            }
            let foreign = programs
                .iter()
                .find(|program| program.module == "lib.core")
                .unwrap()
                .types
                .iter()
                .find(|declaration| declaration.stable_id == "lib.foreign")
                .unwrap()
                .clone();
            synthetic.types.push(foreign);
        },
        "resolved workspace declaration leaks a non-imported foreign authority",
    );
}

#[test]
fn extra_descendant_under_imported_type_fails_closed() {
    assert_resolved_identity_error(
        |module, synthetic, _| {
            if module != "app.identity" {
                return;
            }
            let imported = synthetic
                .types
                .iter_mut()
                .find(|declaration| declaration.stable_id == "lib.imported")
                .unwrap();
            let TypeDeclarationKind::Record { fields } = &mut imported.kind else {
                panic!("imported fixture must be a record")
            };
            let mut extra = fields[0].clone();
            extra.stable_id = "lib.imported.extra".to_owned();
            extra.explicit_id = true;
            extra.name = "extra".to_owned();
            fields.push(extra);
        },
        "resolved workspace declaration leaks a non-imported foreign authority",
    );
}

#[test]
fn synthetic_main_allowlist_is_exact_and_collision_fails_closed() {
    let sources = identity_fact_sources();
    let programs = parsed_sources(&sources);
    let build = build_owned(sources).unwrap();
    let synthetic_modules = {
        let authored = index_authored(&programs).unwrap();
        programs
            .iter()
            .map(|program| {
                let synthetic = synthetic_program(program, &authored, &programs).unwrap();
                (
                    program.module.clone(),
                    hir::resolve(&synthetic).expect("exact synthetic main must resolve"),
                )
            })
            .collect::<Vec<_>>()
    };
    workspace_declaration_facts(&synthetic_modules, &build.hir.modules, &programs).unwrap();

    let collision = canonical_source(
        "collision/lib.spx",
        r#"
module collision.lib;

@id("workspace.synthetic.main.collision.lib")
fn helper() -> i64 { 0 }
"#,
    );
    let app = canonical_source(
        "collision/app.spx",
        r#"
module collision.app;

@id("collision.app.main")
fn main() -> i64 { 0 }
"#,
    );
    let error = build_owned(vec![app, collision])
        .err()
        .expect("authored synthetic-main collision must fail closed");
    assert_eq!(error[0].code, "SPX-G173");
    assert_eq!(
        error[0].message,
        "generated workspace synthetic main identity collides with an authored declaration"
    );
}

#[test]
fn dependency_depths_cover_chain_diamond_branching_and_canonical_cycle() {
    let names = (0..16)
        .map(|index| format!("m{index:02}"))
        .collect::<Vec<_>>();
    let mut chain = BTreeMap::new();
    for (index, module) in names.iter().enumerate() {
        let dependencies = if index == 0 {
            BTreeSet::new()
        } else {
            BTreeSet::from([names[index - 1].as_str()])
        };
        chain.insert(module.as_str(), dependencies);
    }
    let depths = dependency_depths(&chain).unwrap();
    for (index, module) in names.iter().enumerate() {
        assert_eq!(depths[module.as_str()], index + 1);
    }

    let diamond = BTreeMap::from([
        ("leaf", BTreeSet::new()),
        ("left", BTreeSet::from(["leaf"])),
        ("right", BTreeSet::from(["leaf"])),
        ("root", BTreeSet::from(["left", "right"])),
        ("wide", BTreeSet::from(["leaf", "left", "right"])),
    ]);
    let depths = dependency_depths(&diamond).unwrap();
    assert_eq!(depths["leaf"], 1);
    assert_eq!(depths["left"], 2);
    assert_eq!(depths["right"], 2);
    assert_eq!(depths["root"], 3);
    assert_eq!(depths["wide"], 3);

    let cycle = BTreeMap::from([
        ("z", BTreeSet::from(["a"])),
        ("a", BTreeSet::from(["m"])),
        ("m", BTreeSet::from(["z"])),
    ]);
    let error = dependency_depths(&cycle).unwrap_err();
    assert_eq!(error[0].code, "SPX-G172");
    assert_eq!(
        error[0].message,
        "workspace module dependency cycle: a -> m -> z -> a"
    );
}

#[test]
fn logical_limits_accept_exact_and_reject_one_over() {
    for (field, maximum) in [
        ("files", MAX_FILES),
        ("total_source_bytes", MAX_TOTAL_SOURCE_BYTES),
        ("declarations", MAX_DECLARATIONS),
        ("callables", MAX_CALLABLES),
        ("calls", MAX_CALLS),
        ("uses", MAX_USES),
        ("resolved_cross_file_edges", MAX_CROSS_FILE_EDGES),
        ("dependency_depth", MAX_DEPENDENCY_DEPTH),
        ("builder_bytes", MAX_BUILDER_BYTES),
    ] {
        assert_eq!(checked_usage(0, maximum, field, maximum).unwrap(), maximum);
        let error = checked_usage(maximum, 1, field, maximum).unwrap_err();
        assert_eq!(error[0].code, "SPX-G171");
        assert_eq!(
            error[0].message,
            format!("Workspace Semantic Graph `{field}` exceeds {maximum}")
        );
    }
}

#[test]
fn exact_file_limit_builds_and_one_over_rejects_before_parse() {
    let exact = (0..MAX_FILES)
            .map(|index| {
                canonical_source(
                    &format!("m{index:02}.spx"),
                    &format!(
                        "module m{index:02};\n\n@id(\"m{index:02}.entry\")\nfn entry() -> i64 {{ {index} }}\n"
                    ),
                )
            })
            .collect::<Vec<_>>();
    build_owned(exact).unwrap();

    let over = (0..=MAX_FILES)
        .map(|index| source(&format!("bad{index}.spx"), "not parsed"))
        .collect::<Vec<_>>();
    let error = build_owned(over).err().expect("file one-over must fail");
    assert_eq!(error[0].code, "SPX-G171");
}

#[test]
fn edge_append_and_builder_prebound_enforce_exact_boundaries() {
    let edge = WorkspaceEdge {
        caller_path: "a.spx".to_owned(),
        caller: "a.main".to_owned(),
        target_path: "b.spx".to_owned(),
        target: "b.value".to_owned(),
        kind: "call",
        site: "body",
        expression: "expression".to_owned(),
        ast_path: "body.tail".to_owned(),
        alias: "value".to_owned(),
        ordinal: 0,
    };
    let mut exact = vec![edge.clone(); MAX_CROSS_FILE_EDGES - 1];
    push_edge(&mut exact, edge.clone()).unwrap();
    let error = push_edge(&mut exact, edge).unwrap_err();
    assert_eq!(error[0].code, "SPX-G171");

    let (exact, overflowed) = crate::bounded_output::with_limit(MAX_BUILDER_BYTES, || {
        charge_builder_prebound(MAX_BUILDER_BYTES)
    });
    assert!(!overflowed);
    exact.unwrap();
    let (over, overflowed) = crate::bounded_output::with_limit(MAX_BUILDER_BYTES, || {
        charge_builder_prebound(MAX_BUILDER_BYTES + 1)
    });
    assert!(overflowed || over.is_err());
}

#[test]
fn four_two_parameter_materializations_and_t226_premises_are_preserved() {
    let app = canonical_source(
        "generic/app.spx",
        r#"
module generic.app;
@id("generic.first") fn first<T, U>(left: T, right: U) -> T { left }
@id("generic.app.main") fn main() -> i64 {
    let ii = first<i64, i64>(1, 2);
    let ib = first<i64, bool>(ii, true);
    let bi = first<bool, i64>(false, ib);
    if first<bool, bool>(bi, true) { ib } else { 0 }
}
"#,
    );
    let leaf = canonical_source(
        "generic/leaf.spx",
        "module generic.leaf;\n@id(\"generic.leaf.value\") fn value() -> i64 { 0 }\n",
    );
    let build = build_owned(vec![app, leaf]).unwrap();
    let module = build
        .hir
        .modules
        .iter()
        .find(|module| module.module == "generic.app")
        .unwrap();
    assert_eq!(module.function_instances.len(), 4);

    for invalid in [
        r#"module bad.direct;
@id("bad.b") fn b<T>(value: T) -> T { value }
@id("bad.a") fn a<T>(value: T) -> T { b<i64>(0) }
@id("bad.main") fn main() -> i64 { 0 }"#,
        r#"module bad.transitive;
@id("bad.b") fn b<T>(value: T) -> T { value }
@id("bad.middle") fn middle() -> i64 { b<i64>(0) }
@id("bad.a") fn a<T>(value: T) -> T { let seen = middle(); if seen == 0 { value } else { value } }
@id("bad.main") fn main() -> i64 { 0 }"#,
    ] {
        let bad = canonical_source("bad.spx", invalid);
        let leaf = canonical_source(
            "leaf.spx",
            "module leaf;\n@id(\"leaf.value\") fn value() -> i64 { 0 }\n",
        );
        let error = build_owned(vec![bad, leaf])
            .err()
            .expect("T226 must survive");
        assert!(error.iter().any(|diagnostic| diagnostic.code == "SPX-T226"));
    }
}

#[test]
fn long_identity_many_calls_and_deep_paths_replay_deterministically() {
    const CALLS: usize = 32;
    const DEPTH: usize = 12;
    let target = format!("lib.{}", "x".repeat(64));
    let library = canonical_source(
        "long/lib.spx",
        &format!("module long.lib;\n@id(\"{target}\") fn value(input: i64) -> i64 {{ input }}\n"),
    );
    let mut app = format!("module long.app;\nuse function @id(\"{target}\") from long.lib as value;\n@id(\"long.app.main\") fn main() -> i64 {{\n");
    for index in 0..CALLS {
        app.push_str(&format!("let value_{index} = value(0);\n"));
    }
    let mut tail = "0".to_owned();
    for _ in 0..DEPTH {
        tail = format!("value({tail})");
    }
    app.push_str(&format!("{tail}\n}}\n"));
    let app = canonical_source("long/app.spx", &app);
    let first = build_owned(vec![app.clone(), library.clone()]).unwrap();
    let second = build_owned(vec![library, app]).unwrap();
    assert_eq!(first.edges, second.edges);
    assert_eq!(
        first
            .edges
            .iter()
            .filter(|edge| edge.kind == "call" && edge.target == target)
            .count(),
        CALLS + DEPTH
    );
    assert!(first
        .edges
        .iter()
        .any(|edge| edge.kind == "call" && edge.ast_path.matches(".arg.0").count() == DEPTH - 1));
}

fn is_named_limit(diagnostics: &[Diagnostic], field: &str) -> bool {
    diagnostics.first().is_some_and(|diagnostic| {
        diagnostic.code == "SPX-G171" && diagnostic.message.contains(&format!("`{field}` exceeds"))
    })
}

#[test]
fn source_byte_limit_is_checked_before_parse_at_one_over() {
    let exact = vec![
        source("a.spx", &"x".repeat(MAX_TOTAL_SOURCE_BYTES - 1)),
        source("b.spx", "x"),
    ];
    let error = build_owned(exact).err().expect("exact bytes reach parsing");
    assert!(!is_named_limit(&error, "total_source_bytes"));

    let over = vec![
        source("a.spx", &"x".repeat(MAX_TOTAL_SOURCE_BYTES)),
        source("b.spx", "x"),
    ];
    let error = build_owned(over).err().expect("one-over bytes must fail");
    assert!(is_named_limit(&error, "total_source_bytes"));
}

fn declaration_boundary_source(functions: usize) -> WorkspaceSource {
    let mut text = String::from(
        r#"
module boundary.declarations;
@id("d.token") resource Token { @id("d.token.drop") drop trivial; }
@id("d.record") record Record { value: i64, }
@id("d.variant") variant Variant { Case { value: i64, }, }
@id("d.host") interface Host permits {} {
    @id("d.host.consume") import fn consume(value: own Token) -> unit
        effects {} failure infallible consumes value always;
}
"#,
    );
    for index in 0..functions {
        text.push_str(&format!(
            "@id(\"d.f{index}\") fn f{index}() -> i64 {{ 0 }}\n"
        ));
    }
    canonical_source("z_declarations.spx", &text)
}

#[test]
fn mixed_declaration_limit_exact_advances_and_one_over_is_g171() {
    let leaf = canonical_source(
        "a_leaf.spx",
        "module leaf;\n@id(\"leaf.f\") fn f() -> i64 { 0 }\n",
    );
    let exact = declaration_boundary_source(MAX_DECLARATIONS - 10);
    let exact_program = parse(&exact.source, Path::new(&exact.path)).unwrap();
    assert_eq!(
        declaration_count(&exact_program),
        Some(MAX_DECLARATIONS - 1)
    );
    let error = build_owned(vec![exact, leaf.clone()])
        .err()
        .expect("later gate expected");
    assert!(!is_named_limit(&error, "declarations"));

    let over = declaration_boundary_source(MAX_DECLARATIONS - 9);
    let error = build_owned(vec![over, leaf])
        .err()
        .expect("one-over declarations fail");
    assert!(is_named_limit(&error, "declarations"), "{error:?}");
}

fn callable_boundary_source(functions: usize) -> WorkspaceSource {
    let mut text = String::from("module boundary.callables;\n");
    for index in 0..functions {
        text.push_str(&format!(
            "@id(\"c.f{index}\") fn f{index}() -> i64 {{ 0 }}\n"
        ));
    }
    canonical_source("callables.spx", &text)
}

#[test]
fn callable_limit_exact_advances_and_one_over_is_g171() {
    let leaf = canonical_source(
        "leaf.spx",
        "module leaf;\n@id(\"leaf.f\") fn f() -> i64 { 0 }\n",
    );
    let exact = callable_boundary_source(MAX_CALLABLES - 1);
    let error = build_owned(vec![exact, leaf.clone()])
        .err()
        .expect("later gate expected");
    assert!(!is_named_limit(&error, "callables"));
    let over = callable_boundary_source(MAX_CALLABLES);
    let error = build_owned(vec![over, leaf])
        .err()
        .expect("one-over callables fail");
    assert!(is_named_limit(&error, "callables"));
}

fn call_boundary_source(body_calls: usize) -> WorkspaceSource {
    let mut text = String::from(
        r#"
module boundary.calls;
@id("calls.flag") fn flag() -> bool { true }
@id("calls.zero") fn zero() -> i64 { 0 }
@id("calls.keep") fn keep<T>(value: T) -> T
    requires flag()
    ensures flag()
{ let seen = zero(); value }
@id("calls.main") fn main() -> i64 {
"#,
    );
    for index in 0..body_calls {
        text.push_str(&format!("let value_{index} = zero();\n"));
    }
    text.push_str("0\n}\n");
    canonical_source("calls.spx", &text)
}

#[test]
fn call_limit_exact_advances_and_one_over_is_g171() {
    let leaf = canonical_source(
        "leaf.spx",
        "module leaf;\n@id(\"leaf.f\") fn f() -> i64 { 0 }\n",
    );
    let exact = call_boundary_source(MAX_CALLS - 3);
    let error = build_owned(vec![exact, leaf.clone()])
        .err()
        .expect("later gate expected");
    assert!(!is_named_limit(&error, "calls"));
    let over = call_boundary_source(MAX_CALLS - 2);
    let error = build_owned(vec![over, leaf])
        .err()
        .expect("one-over calls fail");
    assert!(is_named_limit(&error, "calls"));
}

fn use_boundary_source(uses: usize) -> WorkspaceSource {
    let mut text = String::from("module boundary.uses;\n");
    for index in 0..uses {
        text.push_str(&format!(
            "use function @id(\"missing.f{index}\") from missing.module as f{index};\n"
        ));
    }
    text.push_str("@id(\"uses.main\") fn main() -> i64 { 0 }\n");
    canonical_source("uses.spx", &text)
}

#[test]
fn use_limit_exact_advances_and_one_over_is_g171() {
    let leaf = canonical_source(
        "leaf.spx",
        "module leaf;\n@id(\"leaf.f\") fn f() -> i64 { 0 }\n",
    );
    let exact = use_boundary_source(MAX_USES);
    let error = build_owned(vec![exact, leaf.clone()])
        .err()
        .expect("later gate expected");
    assert!(!is_named_limit(&error, "uses"));
    let over = use_boundary_source(MAX_USES + 1);
    let error = build_owned(vec![over, leaf])
        .err()
        .expect("one-over uses fail");
    assert!(is_named_limit(&error, "uses"));
}

#[test]
fn branching_copy_default_expansion_is_rejected_by_builder_preflight() {
    const LEVELS: usize = 12;
    const BRANCHES: usize = 4;
    let mut provider = String::from("module hostile.copy;\n");
    provider.push_str("@id(\"copy.r0\") record R0 { @id(\"copy.r0.value\") value: i64, }\n");
    for level in 1..LEVELS {
        provider.push_str(&format!("@id(\"copy.r{level}\") record R{level} {{\n"));
        for field in 0..BRANCHES {
            provider.push_str(&format!(
                "@id(\"copy.r{level}.f{field}\") f{field}: R{},\n",
                level - 1
            ));
        }
        provider.push_str("}\n");
    }
    provider.push_str(&format!(
        "@id(\"copy.make\") fn make(value: R{}) -> R{} {{ value }}\n",
        LEVELS - 1,
        LEVELS - 1
    ));

    let mut consumer = String::from("module hostile.consumer;\n");
    consumer.push_str("use function @id(\"copy.make\") from hostile.copy as make;\n");
    for level in 0..LEVELS {
        consumer.push_str(&format!(
            "use type @id(\"copy.r{level}\") from hostile.copy as R{level};\n"
        ));
    }
    consumer.push_str("@id(\"hostile.main\") fn main() -> i64 { 0 }\n");
    let sources = vec![
        canonical_source("hostile/consumer.spx", &consumer),
        canonical_source("hostile/copy.spx", &provider),
    ];
    let programs = parsed_sources(&sources);
    let authored = index_authored(&programs).unwrap();
    let consumer = programs
        .iter()
        .find(|program| program.module == "hostile.consumer")
        .unwrap();
    let error = match synthetic_builder_bytes(consumer, &authored, &programs) {
        Ok(_) => panic!("branching default expansion must fail pre-HIR"),
        Err(error) => error,
    };
    assert!(is_named_limit(&error, "builder_bytes"));
}

#[test]
fn long_nominal_and_child_id_repetition_is_rejected_by_builder_preflight() {
    const READERS: usize = 512;
    let type_id = format!("long.type.{}", "t".repeat(2048));
    let field_id = format!("long.field.{}", "f".repeat(2048));
    let provider = canonical_source(
            "long-type/provider.spx",
            &format!(
                "module long_type.provider;\n@id(\"{type_id}\") record Long {{ @id(\"{field_id}\") value: i64, }}\n@id(\"long.type.local\") fn local() -> i64 {{ 0 }}\n"
            ),
        );
    let mut consumer = format!(
        "module long_type.consumer;\nuse type @id(\"{type_id}\") from long_type.provider as L;\n"
    );
    for index in 0..READERS {
        consumer.push_str(&format!(
                "@id(\"reader.{index}\") fn read_{index}(value: L) -> i64 {{ match value {{ L {{ value }} => value, }} }}\n"
            ));
    }
    consumer.push_str("@id(\"long.type.main\") fn main() -> i64 { let value = L { value: 0 }; match value { L { value } => value, } }\n");
    let consumer = canonical_source("long-type/consumer.spx", &consumer);
    let sources = vec![consumer, provider];
    let programs = parsed_sources(&sources);
    let authored = index_authored(&programs).unwrap();
    let consumer = programs
        .iter()
        .find(|program| program.module == "long_type.consumer")
        .unwrap();
    let error = match synthetic_builder_bytes(consumer, &authored, &programs) {
        Ok(_) => panic!("long rewritten nominal identities must fail pre-HIR"),
        Err(error) => error,
    };
    assert!(is_named_limit(&error, "builder_bytes"));
}

fn minimum_successful_builder_limit(sources: &[WorkspaceSource]) -> usize {
    assert!(build_owned_with_builder_limit(sources.to_vec(), MAX_BUILDER_BYTES).is_ok());
    let mut low = 0usize;
    let mut high = MAX_BUILDER_BYTES;
    while low < high {
        let middle = low + (high - low) / 2;
        if build_owned_with_builder_limit(sources.to_vec(), middle).is_ok() {
            high = middle;
        } else {
            low = middle + 1;
        }
    }
    low
}

#[test]
#[should_panic(
    expected = "private Workspace Semantic Graph builder limit cannot exceed the production maximum"
)]
fn private_builder_limit_cannot_widen_the_production_cap() {
    let _ = build_owned_with_builder_limit(Vec::new(), MAX_BUILDER_BYTES + 1);
}

fn assert_exact_builder_limit_error(error: &[Diagnostic], limit: usize) {
    assert_eq!(error[0].code, "SPX-G171");
    assert_eq!(
        error[0].message,
        format!("Workspace Semantic Graph `builder_bytes` exceeds {limit}")
    );
}

#[test]
fn all_four_generic_materializations_have_an_exact_minimum_builder_limit() {
    let app = canonical_source(
        "generic-limit/app.spx",
        r#"
module generic_limit.app;
@id("generic.limit.first") fn first<T, U>(left: T, right: U) -> T { left }
@id("generic.limit.main") fn main() -> i64 {
    let ii = first<i64, i64>(1, 2);
    let ib = first<i64, bool>(ii, true);
    let bi = first<bool, i64>(false, ib);
    if first<bool, bool>(bi, true) { ib } else { 0 }
}
"#,
    );
    let leaf = canonical_source(
        "generic-limit/leaf.spx",
        "module generic_limit.leaf;\n@id(\"generic.limit.leaf\") fn leaf() -> i64 { 0 }\n",
    );
    let sources = vec![app, leaf];
    let minimum = minimum_successful_builder_limit(&sources);
    assert!(minimum > 0);
    let first = build_owned_with_builder_limit(sources.clone(), minimum).unwrap();
    let second = build_owned_with_builder_limit(sources.clone(), minimum).unwrap();
    assert_eq!(first.edges, second.edges);
    assert_eq!(first.hir.declarations, second.hir.declarations);

    let error = match build_owned_with_builder_limit(sources, minimum - 1) {
        Ok(_) => panic!("minimum minus one must fail"),
        Err(error) => error,
    };
    assert_exact_builder_limit_error(&error, minimum - 1);
}

#[test]
fn late_module_work_has_an_exact_combined_minimum_builder_limit() {
    let provider = canonical_source(
        "late/a_provider.spx",
        r#"
module late.provider;
@id("late.value") fn value() -> i64 { 1 }
"#,
    );
    let minimal = canonical_source(
        "late/z_consumer.spx",
        r#"
module late.consumer;
@id("late.main") fn main() -> i64 { 0 }
"#,
    );
    let mut consumer = String::from(
        r#"
module late.consumer;
use function @id("late.value") from late.provider as value;
@id("late.main") fn main() -> i64 {
"#,
    );
    for index in 0..96 {
        consumer.push_str(&format!("let value_{index} = value();\n"));
    }
    consumer.push_str("0\n}\n");
    let consumer = canonical_source("late/z_consumer.spx", &consumer);

    let base = vec![provider.clone(), minimal];
    let combined = vec![provider, consumer];
    let base_minimum = minimum_successful_builder_limit(&base);
    let combined_minimum = minimum_successful_builder_limit(&combined);
    assert!(base_minimum < combined_minimum);
    assert!(build_owned_with_builder_limit(base, combined_minimum - 1).is_ok());

    let exact = build_owned_with_builder_limit(combined.clone(), combined_minimum).unwrap();
    let replay = build_owned_with_builder_limit(combined.clone(), combined_minimum).unwrap();
    assert_eq!(exact.edges, replay.edges);
    assert_eq!(exact.hir.declarations, replay.hir.declarations);
    let error = match build_owned_with_builder_limit(combined, combined_minimum - 1) {
        Ok(_) => panic!("late module must consume the final builder byte"),
        Err(error) => error,
    };
    assert_exact_builder_limit_error(&error, combined_minimum - 1);
}
