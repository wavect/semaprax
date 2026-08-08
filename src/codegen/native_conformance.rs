//! Exact native-vs-reference evidence for the first resource cleanup slice.
//!
//! This module is test-only. Production resource lowering remains gated by
//! `SPX-B104`; the probe composes the private staged runtimes and cleanup
//! emitter directly so the gate cannot accidentally leak an artifact.

use std::collections::{BTreeMap, HashMap};
use std::fmt::Write as _;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};

use crate::cleanup_plan::{
    execute_for_conformance, CleanupScenario, ContractPhase, StatusCase, StatusProducer,
    StatusSourceId,
};
use crate::conformance::{NormalizedStatus, OperationOutcome, TraceOutcome, TraceResult};
use crate::hir::{
    self, ExpressionId, OwnershipMode, ResolvedFunction, ResolvedProgram, ResolvedType,
};
use crate::parse;

use super::native_cleanup;
use super::native_cleanup_emit;
use super::native_conformance_materialize;
use super::native_conformance_wire;
use super::{native_resource, native_runtime, native_trace_runtime, native_value};

static NEXT_PROBE: AtomicU64 = AtomicU64::new(0);

struct ProbeDirectory {
    path: PathBuf,
}

impl ProbeDirectory {
    fn create(suffix: u64) -> Self {
        let path = std::env::temp_dir().join(format!(
            "semaprax-native-conformance-{}-{suffix}",
            std::process::id()
        ));
        std::fs::create_dir(&path).unwrap();
        Self { path }
    }

    fn path(&self) -> &Path {
        &self.path
    }
}

impl Drop for ProbeDirectory {
    fn drop(&mut self) {
        let Ok(metadata) = std::fs::symlink_metadata(&self.path) else {
            return;
        };
        if metadata.file_type().is_symlink() || metadata.is_file() {
            let _ = std::fs::remove_file(&self.path);
        } else if metadata.is_dir() {
            // This is the exact unique directory created above. Rust's
            // remove_dir_all does not follow directory symlinks encountered
            // beneath it, so hostile replacement cannot escape this root.
            let _ = std::fs::remove_dir_all(&self.path);
        }
    }
}

const SOURCE: &str = r#"module test.native_conformance;

@id("token.type")
resource Token {
    @id("token.drop")
    drop trivial;
}

@id("token.discard")
fn discard(value: own Token) -> i64 { 0 }

@id("token.discard-two")
fn discard_two(first: own Token, second: own Token) -> i64 { 0 }

@id("token.requires")
fn requires_guard(value: own Token, allowed: bool) -> i64
    requires allowed
{
    0
}

@id("token.checked")
fn checked(value: own Token, number: i64) -> i64
    requires number >= 0
{
    number + 1
}

@id("token.identity")
fn identity(value: own Token) -> Token { value }

@id("token.choose-second")
fn choose_second(first: own Token, count: i64, second: own Token) -> Token
    requires count >= 0
{
    second
}

@id("token.ensures-false")
fn ensures_false(value: own Token) -> Token
    ensures false
{
    value
}

@id("app.main")
fn main() -> i64 { 0 }
"#;

fn program() -> ResolvedProgram {
    let parsed = parse(SOURCE, Path::new("native-conformance.spx")).unwrap();
    hir::resolve(&parsed).unwrap()
}

fn function<'a>(program: &'a ResolvedProgram, id: &str) -> &'a ResolvedFunction {
    program
        .functions
        .iter()
        .find(|function| function.id.as_str() == id)
        .unwrap_or_else(|| panic!("missing function `{id}`"))
}

#[derive(Clone, Copy, Debug)]
enum Payload {
    Zero,
    Maximum,
}

impl Payload {
    fn c_expression(self) -> &'static str {
        match self {
            Self::Zero => "(uintptr_t)UINT32_C(0)",
            Self::Maximum => "UINTPTR_MAX",
        }
    }
}

#[derive(Clone, Copy, Debug)]
enum CaseArgument {
    Resource(Payload),
    Bool(bool),
    I64(i64),
}

#[derive(Clone, Debug)]
struct Case {
    scenario_id: &'static str,
    function_id: &'static str,
    arguments: Vec<CaseArgument>,
    booleans: BTreeMap<ExpressionId, bool>,
    operations: BTreeMap<StatusSourceId, OperationOutcome>,
    result: Option<TraceResult>,
}

impl Case {
    fn scenario(&self, nonce: u64) -> CleanupScenario {
        let mut scenario = CleanupScenario::new(self.scenario_id, self.result.clone());
        scenario.booleans = self.booleans.clone();
        scenario.operations = self.operations.clone();
        scenario.context_nonce = nonce;
        scenario
    }
}

fn contract_source(function: &ResolvedFunction, phase: ContractPhase) -> StatusSourceId {
    function
        .cleanup_plan
        .status_sources
        .iter()
        .find_map(|source| match source.producer {
            StatusProducer::ContractFalse { phase: actual, .. } if actual == phase => {
                Some(source.id.clone())
            }
            _ => None,
        })
        .unwrap_or_else(|| panic!("missing {phase:?} source in `{}`", function.id))
}

fn arithmetic_source(function: &ResolvedFunction) -> StatusSourceId {
    function
        .cleanup_plan
        .status_sources
        .iter()
        .find_map(|source| {
            matches!(source.producer, StatusProducer::CheckedArithmetic { .. })
                .then(|| source.id.clone())
        })
        .unwrap_or_else(|| panic!("missing arithmetic source in `{}`", function.id))
}

fn cases(program: &ResolvedProgram) -> Vec<Case> {
    let requires = function(program, "token.requires");
    let checked = function(program, "token.checked");
    let choose_second = function(program, "token.choose-second");
    let ensures = function(program, "token.ensures-false");
    let requires_source = contract_source(requires, ContractPhase::Requires);
    let checked_requires = contract_source(checked, ContractPhase::Requires);
    let checked_arithmetic = arithmetic_source(checked);
    let choose_second_requires = contract_source(choose_second, ContractPhase::Requires);
    let ensures_source = contract_source(ensures, ContractPhase::Ensures);

    let cases = vec![
        Case {
            scenario_id: "discard-zero",
            function_id: "token.discard",
            arguments: vec![CaseArgument::Resource(Payload::Zero)],
            booleans: BTreeMap::new(),
            operations: BTreeMap::new(),
            result: Some(TraceResult::I64(0)),
        },
        Case {
            scenario_id: "discard-max",
            function_id: "token.discard",
            arguments: vec![CaseArgument::Resource(Payload::Maximum)],
            booleans: BTreeMap::new(),
            operations: BTreeMap::new(),
            result: Some(TraceResult::I64(0)),
        },
        Case {
            scenario_id: "discard-two-reverse",
            function_id: "token.discard-two",
            arguments: vec![
                CaseArgument::Resource(Payload::Zero),
                CaseArgument::Resource(Payload::Maximum),
            ],
            booleans: BTreeMap::new(),
            operations: BTreeMap::new(),
            result: Some(TraceResult::I64(0)),
        },
        Case {
            scenario_id: "requires-false",
            function_id: "token.requires",
            arguments: vec![
                CaseArgument::Resource(Payload::Maximum),
                CaseArgument::Bool(false),
            ],
            booleans: BTreeMap::from([(requires_source.expression.clone(), false)]),
            operations: BTreeMap::new(),
            result: None,
        },
        Case {
            scenario_id: "requires-true",
            function_id: "token.requires",
            arguments: vec![
                CaseArgument::Resource(Payload::Zero),
                CaseArgument::Bool(true),
            ],
            booleans: BTreeMap::from([(requires_source.expression.clone(), true)]),
            operations: BTreeMap::new(),
            result: Some(TraceResult::I64(0)),
        },
        Case {
            scenario_id: "checked-success",
            function_id: "token.checked",
            arguments: vec![CaseArgument::Resource(Payload::Zero), CaseArgument::I64(41)],
            booleans: BTreeMap::from([(checked_requires.expression.clone(), true)]),
            operations: BTreeMap::from([(checked_arithmetic.clone(), OperationOutcome::Success)]),
            result: Some(TraceResult::I64(42)),
        },
        Case {
            scenario_id: "checked-add-overflow",
            function_id: "token.checked",
            arguments: vec![
                CaseArgument::Resource(Payload::Maximum),
                CaseArgument::I64(i64::MAX),
            ],
            booleans: BTreeMap::from([(checked_requires.expression.clone(), true)]),
            operations: BTreeMap::from([(
                checked_arithmetic.clone(),
                OperationOutcome::Failure(NormalizedStatus::arithmetic(StatusCase::AddOverflow)),
            )]),
            result: None,
        },
        Case {
            scenario_id: "checked-precondition-false",
            function_id: "token.checked",
            arguments: vec![CaseArgument::Resource(Payload::Zero), CaseArgument::I64(-1)],
            booleans: BTreeMap::from([(checked_requires.expression.clone(), false)]),
            operations: BTreeMap::new(),
            result: None,
        },
        Case {
            scenario_id: "identity-zero",
            function_id: "token.identity",
            arguments: vec![CaseArgument::Resource(Payload::Zero)],
            booleans: BTreeMap::new(),
            operations: BTreeMap::new(),
            result: Some(TraceResult::Owned {
                type_id: program
                    .types
                    .iter()
                    .find(|item| item.id.as_str() == "token.type")
                    .unwrap()
                    .id
                    .clone(),
            }),
        },
        Case {
            scenario_id: "identity-max",
            function_id: "token.identity",
            arguments: vec![CaseArgument::Resource(Payload::Maximum)],
            booleans: BTreeMap::new(),
            operations: BTreeMap::new(),
            result: Some(TraceResult::Owned {
                type_id: program
                    .types
                    .iter()
                    .find(|item| item.id.as_str() == "token.type")
                    .unwrap()
                    .id
                    .clone(),
            }),
        },
        Case {
            scenario_id: "choose-second-zero-max",
            function_id: "token.choose-second",
            arguments: vec![
                CaseArgument::Resource(Payload::Zero),
                CaseArgument::I64(17),
                CaseArgument::Resource(Payload::Maximum),
            ],
            booleans: BTreeMap::from([(choose_second_requires.expression.clone(), true)]),
            operations: BTreeMap::new(),
            result: Some(TraceResult::Owned {
                type_id: program
                    .types
                    .iter()
                    .find(|item| item.id.as_str() == "token.type")
                    .unwrap()
                    .id
                    .clone(),
            }),
        },
        Case {
            scenario_id: "choose-second-zero-zero",
            function_id: "token.choose-second",
            arguments: vec![
                CaseArgument::Resource(Payload::Zero),
                CaseArgument::I64(17),
                CaseArgument::Resource(Payload::Zero),
            ],
            booleans: BTreeMap::from([(choose_second_requires.expression.clone(), true)]),
            operations: BTreeMap::new(),
            result: Some(TraceResult::Owned {
                type_id: program
                    .types
                    .iter()
                    .find(|item| item.id.as_str() == "token.type")
                    .unwrap()
                    .id
                    .clone(),
            }),
        },
        Case {
            scenario_id: "choose-second-requires-false",
            function_id: "token.choose-second",
            arguments: vec![
                CaseArgument::Resource(Payload::Zero),
                CaseArgument::I64(-1),
                CaseArgument::Resource(Payload::Maximum),
            ],
            booleans: BTreeMap::from([(choose_second_requires.expression.clone(), false)]),
            operations: BTreeMap::new(),
            result: None,
        },
        Case {
            scenario_id: "ensures-false",
            function_id: "token.ensures-false",
            arguments: vec![CaseArgument::Resource(Payload::Maximum)],
            booleans: BTreeMap::from([(ensures_source.expression.clone(), false)]),
            operations: BTreeMap::new(),
            result: None,
        },
    ];
    assert_eq!(
        cases
            .iter()
            .map(|case| case.scenario_id)
            .collect::<Vec<_>>(),
        crate::semantic_trace::OWNED_RESOURCE_CORPUS_V1_SCENARIOS
    );
    cases
}

fn emit_probe(
    program: &ResolvedProgram,
    cases: &[(Case, crate::conformance::ConformanceTrace)],
) -> String {
    let abi = native_resource::build_resource_abi(program).unwrap();
    let mut source = String::new();
    native_runtime::emit_status_runtime(&mut source);
    source.push_str(&abi.declarations);
    source.push_str("#include <stdio.h>\n\n");
    source.push_str(super::NATIVE_SCALAR_RUNTIME_C);
    native_trace_runtime::emit_trace_runtime(&mut source);
    source.push_str(C_WIRE_WRITER);
    source.push_str(
        "static void spx_retain_status_runtime(void) {\n\
         (void)spx_status_resolve;\n\
         (void)spx_status_attach_detail;\n\
         (void)spx_status_resolve_detail;\n\
         (void)spx_status_record_requires_false;\n\
         (void)spx_status_record_ensures_false;\n\
         (void)spx_status_record_arithmetic;\n\
         }\n",
    );
    for (index, (case, oracle)) in cases.iter().enumerate() {
        emit_case(&mut source, program, &abi, index, case, oracle);
    }
    source.push_str("int main(int argc, char **argv) {\n");
    source.push_str("    if (argc != 3) return 200;\n");
    for index in 0..cases.len() {
        writeln!(
            source,
            "    if (strcmp(argv[1], \"{index}\") == 0) return spx_case_{index}(argv[2]);"
        )
        .unwrap();
    }
    source.push_str("    return 201;\n}\n");
    source
}

fn emit_case(
    source: &mut String,
    program: &ResolvedProgram,
    abi: &native_resource::NativeResourceAbi,
    case_index: usize,
    case: &Case,
    oracle: &crate::conformance::ConformanceTrace,
) {
    let function = function(program, case.function_id);
    let index = native_cleanup::classify(program, function).unwrap();
    let mut values = native_value::plan(program, function, &index, abi, &HashMap::new()).unwrap();
    let dictionary =
        crate::semantic_trace::build_semantic_event_dictionary(program, &function.id).unwrap();
    values.cleanup_bindings.semantic_events = Some(dictionary.clone());
    let capacity = values.required_event_capacity;
    assert!(capacity > 1);
    let declarations = native_value::emit_declarations(&values);
    let cleanup = native_cleanup_emit::emit_with_block_prologues(
        &index,
        &values.cleanup_bindings,
        |block, output| {
            output.push_str(&native_value::emit_block_prologue(&values, block));
            Ok(())
        },
    )
    .unwrap();
    let result_type = abi.c_type(program, &function.return_type).unwrap();
    assert_eq!(function.params.len(), case.arguments.len());
    for (parameter, argument) in function.params.iter().zip(&case.arguments) {
        assert!(
            matches!(
                (&parameter.ty, parameter.ownership, argument),
                (
                    ResolvedType::Nominal { .. },
                    OwnershipMode::Own,
                    CaseArgument::Resource(_)
                ) | (
                    ResolvedType::Bool,
                    OwnershipMode::Value,
                    CaseArgument::Bool(_)
                ) | (
                    ResolvedType::I64,
                    OwnershipMode::Value,
                    CaseArgument::I64(_)
                )
            ),
            "case `{}` argument does not match parameter `{}`",
            case.scenario_id,
            parameter.id
        );
    }

    write!(
        source,
        "static spx_status_token spx_cleanup_case_{case_index}(struct spx_context *spx_bind_context"
    )
    .unwrap();
    for (parameter_index, parameter) in function.params.iter().enumerate() {
        write!(
            source,
            ", {} spx_bind_param_{parameter_index}",
            abi.c_type(program, &parameter.ty).unwrap()
        )
        .unwrap();
    }
    writeln!(source, ", {result_type} *spx_bind_result_out) {{").unwrap();
    for line in declarations.lines() {
        writeln!(source, "    {line}").unwrap();
    }
    for declaration in &values.declarations {
        if let native_value::NativeValueDeclaration::ResourceStorage { binding, .. } = declaration {
            writeln!(source, "    (void){binding};").unwrap();
        }
    }
    source.push_str(&cleanup);
    source.push_str("}\n");

    writeln!(
        source,
        "static int spx_case_{case_index}(const char *path) {{"
    )
    .unwrap();
    source.push_str("    spx_retain_status_runtime();\n");
    for (position, (parameter, argument)) in function.params.iter().zip(&case.arguments).enumerate()
    {
        let c_type = abi.c_type(program, &parameter.ty).unwrap();
        match argument {
            CaseArgument::Resource(payload) => {
                writeln!(
                    source,
                    "    {c_type} spx_case_input_{position} = {{{}}};",
                    payload.c_expression()
                )
                .unwrap();
                writeln!(
                    source,
                    "    const uintptr_t spx_case_original_{position} = spx_case_input_{position}.payload;"
                )
                .unwrap();
            }
            CaseArgument::Bool(value) => {
                writeln!(
                    source,
                    "    {c_type} spx_case_input_{position} = {};",
                    if *value { "true" } else { "false" }
                )
                .unwrap();
            }
            CaseArgument::I64(value) => {
                writeln!(
                    source,
                    "    {c_type} spx_case_input_{position} = {};",
                    super::c_i64(*value)
                )
                .unwrap();
            }
        }
    }
    let success = matches!(oracle.outcome, TraceOutcome::Success { .. });
    let owned_result_input = match values.result() {
        native_value::NativeValueResult::ScalarI64 => {
            assert_eq!(function.return_type, ResolvedType::I64);
            None
        }
        native_value::NativeValueResult::OwnedInput {
            parameter_index,
            parameter,
            owner_ordinal,
        } => {
            let selected = function
                .params
                .get(*parameter_index)
                .expect("owned result evidence selects an existing parameter");
            assert_eq!(&selected.id, parameter);
            assert_eq!(selected.ownership, OwnershipMode::Own);
            assert_eq!(selected.ty, function.return_type);
            assert!(matches!(
                case.arguments.get(*parameter_index),
                Some(CaseArgument::Resource(_))
            ));
            let exact_owner_ordinal = function.params[..*parameter_index]
                .iter()
                .filter(|candidate| candidate.ownership == OwnershipMode::Own)
                .count();
            assert_eq!(exact_owner_ordinal, *owner_ordinal);
            Some(*parameter_index)
        }
    };
    let owned_result = owned_result_input.is_some();
    if owned_result {
        let position = owned_result_input.expect("owned result lacks its owned argument");
        writeln!(source, "    const uintptr_t spx_case_poison = spx_case_input_{position}.payload == UINTPTR_MAX ? (uintptr_t)UINT32_C(0) : UINTPTR_MAX;").unwrap();
        writeln!(
            source,
            "    {result_type} spx_case_result = {{spx_case_poison}};"
        )
        .unwrap();
    } else {
        source.push_str("    const int64_t spx_case_poison = -INT64_C(777777777777777777);\n");
        source.push_str("    int64_t spx_case_result = spx_case_poison;\n");
    }
    writeln!(
        source,
        "    struct spx_status_entry spx_small_status[UINT32_C(1)] = {{0}};"
    )
    .unwrap();
    source.push_str("    struct spx_context spx_small_context = {0};\n");
    writeln!(source, "    if (!spx_context_init(&spx_small_context, UINT64_C({}), spx_small_status, UINT32_C(1), NULL, NULL, NULL)) return 30;", case_index + 1).unwrap();
    writeln!(
        source,
        "    struct spx_trace_event spx_small_events[UINT32_C({})] = {{0}};",
        capacity - 1
    )
    .unwrap();
    source.push_str("    struct spx_trace_buffer spx_small_trace = {0};\n");
    writeln!(source, "    if (!spx_trace_buffer_init(&spx_small_trace, spx_small_events, UINT32_C({}))) return 31;", capacity - 1).unwrap();
    writeln!(source, "    if (spx_trace_attach_preflight(&spx_small_context, &spx_small_trace, UINT32_C({capacity}))) return 32;").unwrap();
    source.push_str("    if (spx_small_context.trace != NULL || spx_small_trace.length != UINT32_C(0) || spx_small_trace.state != SPX_TRACE_BUFFER_READY) return 33;\n");
    for (position, argument) in case.arguments.iter().enumerate() {
        if matches!(argument, CaseArgument::Resource(_)) {
            writeln!(
                source,
                "    if (spx_case_input_{position}.payload != spx_case_original_{position}) return 34;"
            )
            .unwrap();
        }
    }
    if owned_result {
        source.push_str("    if (spx_case_result.payload != spx_case_poison) return 35;\n");
    } else {
        source.push_str("    if (spx_case_result != spx_case_poison) return 35;\n");
    }

    source.push_str("    struct spx_status_entry spx_status_entries[UINT32_C(1)] = {0};\n");
    source.push_str("    struct spx_context spx_context = {0};\n");
    writeln!(source, "    if (!spx_context_init(&spx_context, UINT64_C({}), spx_status_entries, UINT32_C(1), NULL, NULL, NULL)) return 40;", case_index + 101).unwrap();
    writeln!(
        source,
        "    struct spx_trace_event spx_events[UINT32_C({capacity})] = {{0}};"
    )
    .unwrap();
    source.push_str("    struct spx_trace_buffer spx_trace = {0};\n");
    writeln!(
        source,
        "    if (!spx_trace_buffer_init(&spx_trace, spx_events, UINT32_C({capacity}))) return 41;"
    )
    .unwrap();
    writeln!(source, "    if (!spx_trace_attach_preflight(&spx_context, &spx_trace, UINT32_C({capacity}))) return 42;").unwrap();
    write!(
        source,
        "    spx_status_token spx_status = spx_cleanup_case_{case_index}(&spx_context"
    )
    .unwrap();
    for position in 0..case.arguments.len() {
        write!(source, ", spx_case_input_{position}").unwrap();
    }
    source.push_str(", &spx_case_result);\n");
    if success {
        source.push_str("    if (spx_status != SPX_STATUS_SUCCESS) return 43;\n");
        match &oracle.outcome {
            TraceOutcome::Success {
                result: TraceResult::I64(value),
            } => {
                writeln!(
                    source,
                    "    if (spx_case_result != {}) return 44;",
                    super::c_i64(*value)
                )
                .unwrap();
            }
            TraceOutcome::Success {
                result: TraceResult::Owned { .. },
            } => {
                let position = owned_result_input.expect("owned result lacks its owned argument");
                writeln!(source, "    if (spx_case_result.payload != spx_case_input_{position}.payload) return 44;").unwrap();
            }
            _ => panic!("unsupported success result in first corpus"),
        }
    } else if owned_result {
        source.push_str("    if (spx_status == SPX_STATUS_SUCCESS || spx_case_result.payload != spx_case_poison) return 45;\n");
    } else {
        source.push_str("    if (spx_status == SPX_STATUS_SUCCESS || spx_case_result != spx_case_poison) return 45;\n");
    }
    writeln!(
        source,
        "    if (spx_trace.length != UINT32_C({})) return 46;",
        oracle.events.len()
    )
    .unwrap();
    for (event_index, event) in oracle.events.iter().enumerate() {
        let ordinal = dictionary
            .ordinal_for(&event.event)
            .expect("reference event must be bound by the generated dictionary");
        writeln!(
            source,
            "    if (spx_events[UINT32_C({event_index})].semantic_ordinal != UINT32_C({ordinal})) return 48;"
        )
        .unwrap();
    }
    let (result_tag, scalar_result, owned_type) = match &function.return_type {
        ResolvedType::I64 => (
            1,
            if success {
                "spx_case_result"
            } else {
                "INT64_C(0)"
            },
            "NULL".to_owned(),
        ),
        ResolvedType::Nominal { declaration, .. } => {
            (4, "INT64_C(0)", format!("\"{}\"", declaration.as_str()))
        }
        _ => panic!("unsupported first-corpus result"),
    };
    writeln!(
        source,
        "    if (!spx_write_trace(path, \"{}\", \"{}\", &spx_context, &spx_trace, spx_status, UINT32_C({result_tag}), {scalar_result}, {owned_type})) return 47;",
        case.scenario_id,
        function.id.as_str()
    )
    .unwrap();
    source.push_str("    return 0;\n}\n");
}

const C_WIRE_WRITER: &str = r#"
#include <stdio.h>

static bool spx_write_bytes(FILE *file, const void *bytes, size_t length) {
    return length == 0 || fwrite(bytes, UINT32_C(1), length, file) == length;
}

static bool spx_write_u32(FILE *file, uint32_t value) {
    unsigned char bytes[4] = {
        (unsigned char)(value & UINT32_C(0xff)),
        (unsigned char)((value >> 8) & UINT32_C(0xff)),
        (unsigned char)((value >> 16) & UINT32_C(0xff)),
        (unsigned char)((value >> 24) & UINT32_C(0xff))
    };
    return spx_write_bytes(file, bytes, sizeof(bytes));
}

static bool spx_write_i64(FILE *file, int64_t value) {
    uint64_t raw = (uint64_t)value;
    unsigned char bytes[8];
    for (uint32_t index = UINT32_C(0); index < UINT32_C(8); ++index) {
        bytes[index] = (unsigned char)((raw >> (index * UINT32_C(8))) & UINT64_C(0xff));
    }
    return spx_write_bytes(file, bytes, sizeof(bytes));
}

static bool spx_write_text(FILE *file, const char *text) {
    if (text == NULL) return false;
    size_t length = strlen(text);
    if (length > UINT32_MAX) return false;
    return spx_write_u32(file, (uint32_t)length) &&
        spx_write_bytes(file, text, length);
}

static bool spx_write_storage(
    FILE *file,
    const struct spx_trace_storage_descriptor *storage
) {
    if (!spx_write_u32(file, storage->kind)) return false;
    switch (storage->kind) {
        case SPX_TRACE_STORAGE_VALUE:
            return spx_write_text(file, storage->value_id);
        case SPX_TRACE_STORAGE_TEMPORARY:
            return spx_write_text(file, storage->expression_id);
        case SPX_TRACE_STORAGE_PROVISIONAL_RESULT:
            return true;
        default:
            return false;
    }
}

static bool spx_write_place(
    FILE *file,
    const struct spx_trace_place_descriptor *place
) {
    return spx_write_storage(file, &place->storage) &&
        spx_write_u32(file, UINT32_C(0));
}

static bool spx_write_status_source(
    FILE *file,
    const struct spx_trace_status_source_descriptor *source
) {
    return spx_write_text(file, source->expression_id) &&
        spx_write_u32(file, source->lane);
}

static bool spx_write_status(
    FILE *file,
    const struct spx_trace_normalized_status *status
) {
    return spx_write_text(file, status->schema) &&
        spx_write_text(file, status->domain_id) &&
        spx_write_u32(file, status->code) &&
        spx_write_u32(file, status->status_class) &&
        spx_write_u32(file, status->retryability);
}

static bool spx_write_event(FILE *file, const struct spx_trace_event *event) {
    if (!spx_write_u32(file, event->kind) ||
        !spx_write_text(file, event->function_id) ||
        !spx_write_u32(file, UINT32_C(0))) return false;
    switch (event->kind) {
        case SPX_TRACE_TRANSFER:
            return spx_write_text(file, event->data.transfer.at_expression_id) &&
                spx_write_place(file, &event->data.transfer.source) &&
                spx_write_place(file, &event->data.transfer.destination);
        case SPX_TRACE_SELECT_FAILURE:
            return spx_write_status_source(
                    file, &event->data.select_failure.source) &&
                spx_write_status(file, &event->data.select_failure.status);
        case SPX_TRACE_FINALIZE_BEGIN:
        case SPX_TRACE_FINALIZE_END:
            return spx_write_place(file, &event->data.finalize.source) &&
                spx_write_text(file, event->data.finalize.lifecycle_id) &&
                spx_write_u32(file, event->data.finalize.guard_flag) &&
                spx_write_u32(file, UINT32_C(0));
        case SPX_TRACE_RESULT_COMMIT:
            if (!spx_write_u32(file, event->data.result_commit.source.kind)) return false;
            if (event->data.result_commit.source.kind == SPX_TRACE_RESULT_SCALAR) {
                return spx_write_text(
                    file, event->data.result_commit.source.scalar_expression_id
                );
            }
            if (event->data.result_commit.source.kind == SPX_TRACE_RESULT_OWNED) {
                return spx_write_place(
                    file, &event->data.result_commit.source.owned_storage
                );
            }
            return false;
        default:
            return false;
    }
}

static bool spx_write_trace(
    const char *path,
    const char *scenario,
    const char *root_function,
    const struct spx_context *context,
    const struct spx_trace_buffer *trace,
    spx_status_token returned_status,
    uint32_t result_kind,
    int64_t scalar_result,
    const char *owned_result_type
) {
    static const unsigned char magic[8] = {'S','P','X','T','R','C','1','\0'};
#if defined(_WIN32)
    FILE *file = NULL;
    if (fopen_s(&file, path, "wb") != 0 || file == NULL) return false;
#else
    FILE *file = fopen(path, "wb");
    if (file == NULL) return false;
#endif
    bool ok = spx_write_bytes(file, magic, sizeof(magic)) &&
        spx_write_u32(file, UINT32_C(1)) &&
        spx_write_u32(file, trace->length) &&
        spx_write_text(file, scenario) &&
        spx_write_text(file, root_function);
    for (uint32_t index = UINT32_C(0); ok && index < trace->length; ++index) {
        ok = spx_write_event(file, &trace->events[index]);
    }
    if (ok && returned_status == SPX_STATUS_SUCCESS) {
        ok = spx_write_u32(file, UINT32_C(1)) &&
            spx_write_u32(file, result_kind);
        if (ok && result_kind == UINT32_C(1)) {
            ok = spx_write_i64(file, scalar_result);
        } else if (ok && result_kind == UINT32_C(4)) {
            ok = spx_write_text(file, owned_result_type);
        } else {
            ok = false;
        }
    } else if (ok) {
        const struct spx_trace_event *selected = NULL;
        for (uint32_t index = UINT32_C(0); index < trace->length; ++index) {
            if (trace->events[index].kind == SPX_TRACE_SELECT_FAILURE) {
                if (selected != NULL) { ok = false; break; }
                selected = &trace->events[index];
            }
        }
        const struct spx_normalized_status *returned =
            spx_status_resolve(context, returned_status);
        if (selected == NULL || returned == NULL ||
            strcmp(returned->schema,
                   selected->data.select_failure.status.schema) != 0 ||
            strcmp(returned->domain_id,
                   selected->data.select_failure.status.domain_id) != 0 ||
            returned->code != selected->data.select_failure.status.code ||
            returned->status_class !=
                selected->data.select_failure.status.status_class ||
            returned->retryability !=
                selected->data.select_failure.status.retryability) {
            ok = false;
        }
        if (ok) {
            ok = spx_write_u32(file, UINT32_C(2)) &&
                spx_write_status_source(
                    file, &selected->data.select_failure.source
                ) && spx_write_status(
                    file, &selected->data.select_failure.status
                );
        }
    }
    if (fclose(file) != 0) ok = false;
    return ok;
}
"#;

fn sanitizer_supported(directory: &Path, label: &str, flags: &[&str]) -> bool {
    let source = directory.join(format!("{label}-support.c"));
    let executable = directory.join(format!("{label}-support{}", std::env::consts::EXE_SUFFIX));
    std::fs::write(&source, "int main(void) { return 0; }\n").unwrap();
    let compiled = Command::new("clang")
        .args(["-std=c11", "-Werror"])
        .args(flags)
        .arg(&source)
        .arg("-o")
        .arg(&executable)
        .output()
        .unwrap();
    let supported = compiled.status.success()
        && Command::new(&executable)
            .output()
            .is_ok_and(|output| output.status.success());
    if !supported {
        eprintln!(
            "skipping {label}: compiler/runtime support probe failed: {}",
            String::from_utf8_lossy(&compiled.stderr)
        );
    }
    let _ = std::fs::remove_file(source);
    let _ = std::fs::remove_file(executable);
    supported
}

fn assert_payload_opacity(cases: &[(Case, crate::conformance::ConformanceTrace)]) {
    let trace = |scenario_id: &str| {
        &cases
            .iter()
            .find(|(case, _)| case.scenario_id == scenario_id)
            .unwrap_or_else(|| panic!("missing payload-opacity scenario `{scenario_id}`"))
            .1
    };
    for (zero, maximum) in [
        ("discard-zero", "discard-max"),
        ("identity-zero", "identity-max"),
        ("choose-second-zero-max", "choose-second-zero-zero"),
    ] {
        let zero = trace(zero);
        let maximum = trace(maximum);
        assert_ne!(zero.scenario_id, maximum.scenario_id);
        assert_eq!(zero.root_function, maximum.root_function);
        assert_eq!(zero.events, maximum.events);
        assert_eq!(zero.outcome, maximum.outcome);
    }
}

#[test]
fn choose_second_fixture_uses_the_exact_planned_owner() {
    let program = program();
    let selected = function(&program, "token.choose-second");
    let abi = native_resource::build_resource_abi(&program).unwrap();
    let cleanup = native_cleanup::classify(&program, selected).unwrap();
    let values = native_value::plan(&program, selected, &cleanup, &abi, &HashMap::new()).unwrap();
    assert_eq!(
        values.result(),
        &native_value::NativeValueResult::OwnedInput {
            parameter_index: 2,
            parameter: selected.params[2].id.clone(),
            owner_ordinal: 1,
        }
    );

    let case = cases(&program)
        .into_iter()
        .find(|case| case.scenario_id == "choose-second-zero-max")
        .unwrap();
    let oracle = execute_for_conformance(&program, &selected.id, case.scenario(901)).unwrap();
    let source = emit_probe(&program, &[(case, oracle)]);
    assert!(source.contains(
        "spx_case_input_2.payload == UINTPTR_MAX ? (uintptr_t)UINT32_C(0) : UINTPTR_MAX"
    ));
    assert!(source.contains("spx_case_result.payload != spx_case_input_2.payload"));
}

fn assert_real_value_lowering(source: &str) {
    for required in [
        ", bool spx_bind_param_1",
        ", int64_t spx_bind_param_1",
        "bool spx_case_input_1 = false;",
        "bool spx_case_input_1 = true;",
        "int64_t spx_case_input_1 = INT64_C(41);",
        "int64_t spx_case_input_1 = INT64_C(9223372036854775807);",
        "int64_t spx_case_input_1 = -INT64_C(1);",
        " = spx_rt_contract(spx_bind_context,",
        " = spx_rt_add(spx_bind_context,",
    ] {
        assert!(
            source.contains(required),
            "native corpus omitted real value lowering fragment `{required}`"
        );
    }
    assert!(
        source
            .lines()
            .any(|line| line.contains(" = (") && line.contains(" >= ")),
        "native corpus omitted the staged i64 Ge comparison"
    );
    assert!(!source.contains("spx_bind_bool_"));
    assert!(!source.contains("\"status record\""));
}

#[test]
fn first_native_resource_corpus_matches_the_reference_across_codegen_modes() {
    let require_sanitizers =
        std::env::var_os("SEMAPRAX_REQUIRE_NATIVE_SANITIZERS").is_some_and(|value| value == "1");
    if Command::new("clang").arg("--version").output().is_err() {
        assert!(
            !require_sanitizers,
            "SEMAPRAX_REQUIRE_NATIVE_SANITIZERS=1 but Clang is unavailable"
        );
        return;
    }
    let program = program();
    let cases = cases(&program)
        .into_iter()
        .enumerate()
        .map(|(index, case)| {
            let selected = function(&program, case.function_id);
            let oracle = execute_for_conformance(
                &program,
                &selected.id,
                case.scenario(u64::try_from(index + 1).unwrap()),
            )
            .unwrap();
            (case, oracle)
        })
        .collect::<Vec<_>>();
    let reverse = cases
        .iter()
        .find(|(case, _)| case.scenario_id == "discard-two-reverse")
        .unwrap();
    let reverse_flags = reverse
        .1
        .events
        .iter()
        .filter_map(|event| match event.event {
            crate::conformance::TraceEventKind::FinalizeBegin { guard_flag, .. }
            | crate::conformance::TraceEventKind::FinalizeEnd { guard_flag, .. } => {
                Some(guard_flag.0)
            }
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(reverse_flags, [1, 1, 0, 0]);

    let rejected_owned_result = cases
        .iter()
        .find(|(case, _)| case.scenario_id == "choose-second-requires-false")
        .unwrap();
    assert!(matches!(
        rejected_owned_result.1.outcome,
        TraceOutcome::Failure { .. }
    ));
    let rejected_cleanup_flags = rejected_owned_result
        .1
        .events
        .iter()
        .filter_map(|event| match event.event {
            crate::conformance::TraceEventKind::FinalizeBegin { guard_flag, .. }
            | crate::conformance::TraceEventKind::FinalizeEnd { guard_flag, .. } => {
                Some(guard_flag.0)
            }
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(rejected_cleanup_flags, [1, 1, 0, 0]);
    assert!(rejected_owned_result.1.events.iter().all(|event| !matches!(
        event.event,
        crate::conformance::TraceEventKind::ResultCommit { .. }
    )));
    assert_payload_opacity(&cases);
    let c_source = emit_probe(&program, &cases);
    assert_real_value_lowering(&c_source);
    let suffix = NEXT_PROBE.fetch_add(1, Ordering::Relaxed);
    let directory = ProbeDirectory::create(suffix);
    let directory = directory.path();
    let source_path = directory.join("probe.c");
    std::fs::write(&source_path, c_source).unwrap();

    let mut configurations = vec![("o0", vec!["-O0"], None), ("o2", vec!["-O2"], None)];
    if cfg!(unix) {
        let address = vec!["-O1", "-fno-omit-frame-pointer", "-fsanitize=address"];
        let address_supported = sanitizer_supported(directory, "asan", &address);
        assert!(
            address_supported || !require_sanitizers,
            "SEMAPRAX_REQUIRE_NATIVE_SANITIZERS=1 but the ASan compile/run probe failed"
        );
        if address_supported {
            configurations.push(("asan", address, Some("address")));
        }
        let undefined = vec![
            "-O1",
            "-fno-omit-frame-pointer",
            "-fsanitize=undefined",
            "-fno-sanitize-recover=undefined",
        ];
        let undefined_supported = sanitizer_supported(directory, "ubsan", &undefined);
        assert!(
            undefined_supported || !require_sanitizers,
            "SEMAPRAX_REQUIRE_NATIVE_SANITIZERS=1 but the UBSan compile/run probe failed"
        );
        if undefined_supported {
            configurations.push(("ubsan", undefined, Some("undefined")));
        }
    } else {
        assert!(
            !require_sanitizers,
            "SEMAPRAX_REQUIRE_NATIVE_SANITIZERS=1 is Unix-only; this target cannot satisfy the required ASan/UBSan evidence"
        );
    }
    let mut canonical_frames = BTreeMap::new();
    for (configuration, flags, sanitizer) in configurations {
        let executable_name = format!("probe-{configuration}{}", std::env::consts::EXE_SUFFIX);
        let executable = directory.join(&executable_name);
        let compiled = Command::new("clang")
            .args(["-std=c11", "-Wall", "-Wextra", "-Werror"])
            .args(flags)
            .arg(&source_path)
            .arg("-o")
            .arg(&executable)
            .output()
            .unwrap();
        assert!(
            compiled.status.success(),
            "native conformance {configuration} compile failed: {}",
            String::from_utf8_lossy(&compiled.stderr)
        );

        for (case_index, (case, oracle)) in cases.iter().enumerate() {
            let output_name = format!("trace-{configuration}-{case_index}.bin");
            let mut command = Command::new(&executable);
            command
                .args([case_index.to_string(), output_name.clone()])
                .current_dir(directory);
            if sanitizer == Some("address") {
                command.env("ASAN_OPTIONS", "halt_on_error=1:abort_on_error=1");
            } else if sanitizer == Some("undefined") {
                command.env("UBSAN_OPTIONS", "halt_on_error=1:print_stacktrace=1");
            }
            let executed = command.output().unwrap();
            assert!(
                executed.status.success(),
                "native conformance {configuration}/{} failed with {:?}: {}",
                case.scenario_id,
                executed.status.code(),
                String::from_utf8_lossy(&executed.stderr)
            );
            let bytes = std::fs::read(directory.join(&output_name)).unwrap();
            let wire = native_conformance_wire::decode(&bytes).unwrap();
            let selected = function(&program, case.function_id);
            let native =
                native_conformance_materialize::materialize(&program, selected, wire).unwrap();
            assert_eq!(native, *oracle);
            assert_eq!(native.to_json(), oracle.to_json());
            if let Some(previous) = canonical_frames.insert(case.scenario_id, bytes.clone()) {
                assert_eq!(
                    bytes, previous,
                    "wire output for {} changed at {configuration}",
                    case.scenario_id
                );
            }
        }
    }
}
