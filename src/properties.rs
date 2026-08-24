//! Deterministic, read-only Property-Test Generation v1.

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use sha2::{Digest as _, Sha256};

use crate::ast::{
    BinaryOp, Expr, ExprKind, Function, ParamMode, Program, Statement, Type, UnaryOp,
};
use crate::bounded_output::{with_limit, BudgetedJoin as _};
use crate::diagnostic::{quote_json, Diagnostic};
use crate::{format, graph, parse, patch, verify};

macro_rules! bformat {
    ($($argument:tt)*) => {
        crate::bounded_output::budgeted_format(format_args!($($argument)*))
    };
}

pub const SCHEMA: &str = "semaprax.property-tests.v1";

const DEFAULT_MAX_CASES: usize = 64;
const DEFAULT_MAX_FUNCTIONS: usize = 64;
const DEFAULT_MAX_BYTES: usize = 64 * 1024;
pub const DEFAULT_SEED: u64 = 0x9e37_79b9_7f4a_7c15;

pub const MAX_CASES_LIMIT: usize = 4096;
pub const MAX_FUNCTIONS_LIMIT: usize = 1024;
const MAX_TOTAL_STEPS: usize = 1_000_000;
const MAX_CALL_DEPTH: usize = 16;

const REASON_GENERIC_FUNCTION: &str = "generic_function";
const REASON_DECLARED_EFFECTS: &str = "declared_effects";
const REASON_UNSUPPORTED_PARAMETER_MODE: &str = "unsupported_parameter_mode";
const REASON_UNSUPPORTED_PARAMETER_TYPE: &str = "unsupported_parameter_type";
const REASON_UNSUPPORTED_RESULT_TYPE: &str = "unsupported_result_type";
const REASON_EVALUATION_STEP_BUDGET_EXHAUSTED: &str = "evaluation_step_budget_exhausted";
const REASON_FLOAT_LITERAL: &str = "float_literal";
const REASON_INT32_LITERAL: &str = "int32_literal";
const REASON_CHAR_LITERAL: &str = "char_literal";
const REASON_UINT8_LITERAL: &str = "uint8_literal";
const REASON_RECORD_CONSTRUCTION: &str = "record_construction";
const REASON_VARIANT_CONSTRUCTION: &str = "variant_construction";
const REASON_RECORD_UPDATE: &str = "record_update";
const REASON_RECORD_PROJECTION: &str = "record_projection";
const REASON_MATCH_EXPRESSION: &str = "match_expression";
const REASON_TRY_EXPRESSION: &str = "try_expression";
const REASON_ASSIGNMENT: &str = "assignment_statement";
const REASON_WHILE_LOOP: &str = "while_statement";
const REASON_GENERIC_CALL: &str = "generic_call";
const REASON_UNRESOLVED_CALL: &str = "unresolved_call";
const REASON_UNRESOLVED_VARIABLE: &str = "unresolved_variable";
const REASON_UNSUPPORTED_CALLEE: &str = "unsupported_callee";
const REASON_ILL_TYPED_EXPRESSION: &str = "ill_typed_expression";
const REASON_METHOD_CALL: &str = "method_call";

const RUNTIME_ARITHMETIC_OVERFLOW: &str = "arithmetic_overflow";
const RUNTIME_DIVISION_BY_ZERO: &str = "division_by_zero";
const RUNTIME_REMAINDER_BY_ZERO: &str = "remainder_by_zero";
const RUNTIME_NEGATION_OVERFLOW: &str = "negation_overflow";
const RUNTIME_CALL_DEPTH_EXCEEDED: &str = "call_depth_exceeded";
const RUNTIME_CALLEE_REQUIRES_VIOLATED: &str = "callee_requires_violated";

const TRUNCATION_BYTE_BUDGET: &str = "byte_budget";
const TRUNCATION_FUNCTION_BUDGET: &str = "function_budget";
const TRUNCATION_STEP_BUDGET: &str = "step_budget";

const NONCLAIMS_JSON: &str = "\"no_symbolic_execution_or_smt\",\
\"no_static_contract_discharge\",\
\"no_counterexample_minimization\",\
\"no_statistical_coverage_guarantee\",\
\"not_a_test_runner\",\
\"no_target_execution\"";

const I64_LATTICE: [i64; 11] = [
    0,
    1,
    -1,
    2,
    -2,
    3,
    -3,
    i64::MIN,
    i64::MAX,
    i64::MIN + 1,
    i64::MAX - 1,
];

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PropertyTestOptions {
    pub max_cases: usize,
    pub max_functions: usize,
    pub max_bytes: usize,
    pub seed: u64,
}

impl PropertyTestOptions {
    pub fn new(
        max_cases: usize,
        max_functions: usize,
        max_bytes: usize,
        seed: u64,
    ) -> Result<Self, Diagnostic> {
        if max_cases == 0 || max_cases > MAX_CASES_LIMIT {
            return Err(option_error(format!(
                "property test max_cases must be between 1 and {MAX_CASES_LIMIT}"
            )));
        }
        if max_functions == 0 || max_functions > MAX_FUNCTIONS_LIMIT {
            return Err(option_error(format!(
                "property test max_functions must be between 1 and {MAX_FUNCTIONS_LIMIT}"
            )));
        }
        if !(graph::MIN_AGENT_CONTEXT_BYTES..=graph::MAX_AGENT_CONTEXT_BYTES).contains(&max_bytes) {
            return Err(option_error(format!(
                "property test max_bytes must be between {} and {}",
                graph::MIN_AGENT_CONTEXT_BYTES,
                graph::MAX_AGENT_CONTEXT_BYTES
            )));
        }
        Ok(Self {
            max_cases,
            max_functions,
            max_bytes,
            seed,
        })
    }
}

impl Default for PropertyTestOptions {
    fn default() -> Self {
        Self {
            max_cases: DEFAULT_MAX_CASES,
            max_functions: DEFAULT_MAX_FUNCTIONS,
            max_bytes: DEFAULT_MAX_BYTES,
            seed: DEFAULT_SEED,
        }
    }
}

fn option_error(message: String) -> Diagnostic {
    Diagnostic::io("SPX-P101", message)
}

pub fn generate(
    source_path: &Path,
    options: &PropertyTestOptions,
) -> Result<String, Vec<Diagnostic>> {
    generate_with_hook(source_path, options, &mut |_, _| Ok(()))
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum HookPhase {
    AfterParse,
    BeforeFinalCheck,
}

fn generate_with_hook(
    source_path: &Path,
    options: &PropertyTestOptions,
    hook: &mut dyn FnMut(HookPhase, &Path) -> std::io::Result<()>,
) -> Result<String, Vec<Diagnostic>> {
    let canonical_source_path = patch::canonical_source_path(source_path)?;
    let snapshot = patch::read_source_snapshot(&canonical_source_path)?;
    let program = parse(snapshot.source(), source_path).map_err(|error| vec![error])?;
    let diagnostics = verify::verify(&program);
    if diagnostics.iter().any(|item| item.severity.is_error()) {
        return Err(diagnostics);
    }
    hook(HookPhase::AfterParse, &canonical_source_path).map_err(|error| {
        vec![Diagnostic::io(
            "SPX-I207",
            format!("property test hook failed: {error}"),
        )]
    })?;
    let revision = graph::revision(&program);
    let report = build_report(snapshot.source(), source_path, &program, options);
    hook(HookPhase::BeforeFinalCheck, &canonical_source_path).map_err(|error| {
        vec![Diagnostic::io(
            "SPX-I207",
            format!("property test final-check hook failed: {error}"),
        )]
    })?;
    patch::validate_source_unchanged(&canonical_source_path, source_path, &snapshot, &revision)?;
    Ok(report)
}

#[derive(Default)]
struct Summary {
    functions_total: usize,
    functions_analyzed: usize,
    functions_deferred: usize,
    functions_with_counterexamples: usize,
    cases_attempted: usize,
    filtered_cases: usize,
    discharged_cases: usize,
    runtime_failure_cases: usize,
}

fn build_report(
    source: &str,
    source_path: &Path,
    program: &Program,
    options: &PropertyTestOptions,
) -> String {
    let mut analyzer = Analyzer::new(program);
    let mut entries: Vec<String> = Vec::new();
    let mut summary = Summary::default();
    let mut used_cases = 0usize;
    let mut step_stop = false;

    for (index, function) in program.functions.iter().enumerate() {
        if step_stop || entries.len() >= options.max_functions {
            break;
        }
        match analyzer.analyze_function(index, function, options) {
            FunctionOutcome::Deferred(reason) => {
                summary.functions_deferred += 1;
                entries.push(deferred_entry_json(function, reason));
            }
            FunctionOutcome::Analyzed(entry) => {
                summary.functions_analyzed += 1;
                summary.functions_with_counterexamples += usize::from(entry.counterexample);
                summary.cases_attempted += entry.cases_attempted;
                summary.filtered_cases += entry.filtered_cases;
                summary.discharged_cases += entry.discharged_cases;
                summary.runtime_failure_cases += entry.runtime_failure_cases;
                used_cases += entry.cases_attempted;
                entries.push(entry.json);
            }
            FunctionOutcome::Exhausted => {
                step_stop = true;
            }
        }
    }

    summary.functions_total = program.functions.len();
    let omitted_functions = program.functions.len() - entries.len();
    let mut reasons: Vec<&'static str> = Vec::new();
    if omitted_functions > 0 {
        reasons.push(if step_stop {
            TRUNCATION_STEP_BUDGET
        } else {
            TRUNCATION_FUNCTION_BUDGET
        });
    }

    let path_json = quote_json(&source_path.display().to_string());
    let revision_json = quote_json(&graph::revision(program));
    let digest_json = quote_json(&source_digest(source));

    let render =
        |count: usize, dropped: usize, render_reasons: &[&'static str]| -> (String, bool) {
            with_limit(options.max_bytes, || {
                render_report(
                    &path_json,
                    &revision_json,
                    &digest_json,
                    options,
                    count,
                    dropped,
                    omitted_functions,
                    render_reasons,
                    &summary,
                    analyzer.steps,
                    used_cases,
                    &entries,
                )
            })
        };

    let total_entries = entries.len();
    let (output, overflowed) = render(total_entries, 0, &reasons);
    if !overflowed {
        return output;
    }
    reasons.push(TRUNCATION_BYTE_BUDGET);
    let mut low = 0usize;
    let mut high = total_entries;
    let mut best: Option<(String, usize)> = None;
    while low <= high {
        let middle = (low + high) / 2;
        let dropped = total_entries - middle;
        let (candidate, still_over) = render(middle, dropped, &reasons);
        if still_over {
            if middle == 0 {
                break;
            }
            high = middle - 1;
        } else {
            best = Some((candidate, middle));
            if middle == total_entries {
                break;
            }
            low = middle + 1;
        }
    }
    let (count, dropped) = best.map_or((0, total_entries), |(_, count)| {
        (count, total_entries - count)
    });
    render(count, dropped, &reasons).0
}

#[allow(clippy::too_many_arguments)]
fn render_report(
    path_json: &str,
    revision_json: &str,
    digest_json: &str,
    options: &PropertyTestOptions,
    count: usize,
    byte_dropped: usize,
    omitted_functions: usize,
    reasons: &[&'static str],
    summary: &Summary,
    used_nodes: usize,
    used_cases: usize,
    entries: &[String],
) -> String {
    let truncated = !reasons.is_empty() || byte_dropped > 0;
    let total_omitted = omitted_functions + byte_dropped;
    let reasons_json = reasons
        .iter()
        .map(|reason| bformat!("\"{reason}\""))
        .collect::<Vec<_>>();
    let functions_json = entries[..count].budgeted_join(",");
    bformat!(
        "{{\"schema\":\"{}\",\"source\":{{\"path\":{},\"revision\":{},\"sha256\":{}}},\
\"seed\":\"{}\",\"limits\":{{\"max_cases\":{},\"max_functions\":{},\"max_bytes\":{}}},\
\"budget\":{{\"used_functions\":{},\"used_cases\":{},\"used_nodes\":{}}},\
\"truncation\":{{\"truncated\":{},\"reasons\":[{}],\"omitted_functions\":{}}},\
\"summary\":{{\"functions_total\":{},\"functions_analyzed\":{},\"functions_deferred\":{},\
\"functions_with_counterexamples\":{},\"cases_attempted\":{},\"filtered_cases\":{},\
\"discharged_cases\":{},\"runtime_failure_cases\":{}}},\
\"functions\":[{}],\"nonclaims\":[{}]}}",
        SCHEMA,
        path_json,
        revision_json,
        digest_json,
        options.seed,
        options.max_cases,
        options.max_functions,
        options.max_bytes,
        entries.len(),
        used_cases,
        used_nodes,
        truncated,
        reasons_json.budgeted_join(","),
        total_omitted,
        summary.functions_total,
        summary.functions_analyzed,
        summary.functions_deferred,
        summary.functions_with_counterexamples,
        summary.cases_attempted,
        summary.filtered_cases,
        summary.discharged_cases,
        summary.runtime_failure_cases,
        functions_json,
        NONCLAIMS_JSON,
    )
}

fn source_digest(source: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(b"semaprax.property-tests.source.v1\0");
    hasher.update((source.len() as u64).to_le_bytes());
    hasher.update(source.as_bytes());
    format!(
        "sha256:{:x}",
        crate::digest_hex::LowerHex(hasher.finalize())
    )
}

fn deferred_entry_json(function: &Function, reason: &str) -> String {
    bformat!(
        "{{\"stable_id\":{},\"name\":{},\"outcome\":\"deferred\",\"reason\":\"{}\"}}",
        quote_json(&function.stable_id),
        quote_json(&function.name),
        reason,
    )
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Value {
    Int(i64),
    Bool(bool),
}

impl Value {
    fn render(self) -> String {
        match self {
            Value::Int(value) => value.to_string(),
            Value::Bool(value) => value.to_string(),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum RuntimeReason {
    ArithmeticOverflow,
    DivisionByZero,
    RemainderByZero,
    NegationOverflow,
    CallDepthExceeded,
    CalleeRequiresViolated,
}

impl RuntimeReason {
    fn text(self) -> &'static str {
        match self {
            RuntimeReason::ArithmeticOverflow => RUNTIME_ARITHMETIC_OVERFLOW,
            RuntimeReason::DivisionByZero => RUNTIME_DIVISION_BY_ZERO,
            RuntimeReason::RemainderByZero => RUNTIME_REMAINDER_BY_ZERO,
            RuntimeReason::NegationOverflow => RUNTIME_NEGATION_OVERFLOW,
            RuntimeReason::CallDepthExceeded => RUNTIME_CALL_DEPTH_EXCEEDED,
            RuntimeReason::CalleeRequiresViolated => RUNTIME_CALLEE_REQUIRES_VIOLATED,
        }
    }
}

enum Outcome {
    Value(Value),
    Runtime(RuntimeReason),
    Unsupported(&'static str),
    Exhausted,
}

struct AnalyzedEntry {
    json: String,
    cases_attempted: usize,
    filtered_cases: usize,
    runtime_failure_cases: usize,
    discharged_cases: usize,
    counterexample: bool,
}

enum FunctionOutcome {
    Analyzed(AnalyzedEntry),
    Deferred(&'static str),
    Exhausted,
}

type Environment = Vec<(String, Value)>;

fn lookup(environment: &Environment, name: &str) -> Option<Value> {
    environment
        .iter()
        .rev()
        .find(|(key, _)| key == name)
        .map(|(_, value)| *value)
}

struct Analyzer<'a> {
    admitted: BTreeMap<&'a str, &'a Function>,
    names: BTreeSet<&'a str>,
    steps: usize,
}

impl<'a> Analyzer<'a> {
    fn new(program: &'a Program) -> Self {
        let mut admitted = BTreeMap::new();
        let mut names = BTreeSet::new();
        for function in &program.functions {
            names.insert(function.name.as_str());
            admitted.entry(function.name.as_str()).or_insert(function);
        }
        Self {
            admitted,
            names,
            steps: 0,
        }
    }

    fn admission(function: &Function) -> Option<&'static str> {
        if !function.type_parameters.is_empty() {
            return Some(REASON_GENERIC_FUNCTION);
        }
        if !function.effects.is_empty() {
            return Some(REASON_DECLARED_EFFECTS);
        }
        for param in &function.params {
            if param.mode != ParamMode::Value {
                return Some(REASON_UNSUPPORTED_PARAMETER_MODE);
            }
            if !matches!(param.ty, Type::I64 | Type::Bool) {
                return Some(REASON_UNSUPPORTED_PARAMETER_TYPE);
            }
        }
        if !matches!(function.return_type, Type::I64 | Type::Bool) {
            return Some(REASON_UNSUPPORTED_RESULT_TYPE);
        }
        None
    }

    fn scan(&mut self, expression: &Expr) -> Option<&'static str> {
        self.steps += 1;
        if self.steps >= MAX_TOTAL_STEPS {
            return Some(REASON_EVALUATION_STEP_BUDGET_EXHAUSTED);
        }
        match &expression.kind {
            ExprKind::Int(_) | ExprKind::Bool(_) | ExprKind::String(_) | ExprKind::Var(_) => None,
            ExprKind::Int32(_) => Some(REASON_INT32_LITERAL),
            ExprKind::Float32(_) | ExprKind::Float64(_) => Some(REASON_FLOAT_LITERAL),
            ExprKind::Char(_) => Some(REASON_CHAR_LITERAL),
            ExprKind::Uint8(_) => Some(REASON_UINT8_LITERAL),
            ExprKind::Call {
                name,
                type_arguments,
                args,
            } => {
                if !type_arguments.is_empty() {
                    return Some(REASON_GENERIC_CALL);
                }
                if !self.names.contains(name.as_str()) {
                    return Some(REASON_UNRESOLVED_CALL);
                }
                if !self.admitted.contains_key(name.as_str()) {
                    return Some(REASON_UNSUPPORTED_CALLEE);
                }
                args.iter().find_map(|argument| self.scan(argument))
            }
            ExprKind::Unary { value, .. } => self.scan(value),
            ExprKind::Binary { left, right, .. } => self.scan(left).or_else(|| self.scan(right)),
            ExprKind::Block { statements, tail } => statements
                .iter()
                .find_map(|statement| match statement {
                    // Field Mutation v1 targets stay outside the scalar
                    // property slice.
                    Statement::Assign { field: Some(_), .. } => Some(REASON_RECORD_PROJECTION),
                    _ => (0..statement.child_count()).find_map(|index| {
                        statement.child(index).and_then(|child| self.scan(child))
                    }),
                })
                .or_else(|| self.scan(tail)),
            ExprKind::If {
                condition,
                then_branch,
                else_branch,
            } => self
                .scan(condition)
                .or_else(|| self.scan(then_branch))
                .or_else(|| self.scan(else_branch)),
            ExprKind::ConstructRecord { .. } => Some(REASON_RECORD_CONSTRUCTION),
            ExprKind::ConstructVariant { .. } => Some(REASON_VARIANT_CONSTRUCTION),
            ExprKind::UpdateRecord { .. } => Some(REASON_RECORD_UPDATE),
            ExprKind::Project { .. } => Some(REASON_RECORD_PROJECTION),
            ExprKind::MethodCall { .. } => Some(REASON_METHOD_CALL),
            ExprKind::Match { .. } => Some(REASON_MATCH_EXPRESSION),
            ExprKind::Try { .. } => Some(REASON_TRY_EXPRESSION),
        }
    }

    fn scan_function_contracts_and_body(&mut self, function: &'a Function) -> Option<&'static str> {
        for clause in function.requires.iter().chain(function.ensures.iter()) {
            if let Some(reason) = self.scan(clause) {
                return Some(reason);
            }
        }
        self.scan(&function.body)
    }

    fn evaluate(
        &mut self,
        expression: &Expr,
        environment: &mut Environment,
        depth: usize,
    ) -> Outcome {
        self.steps += 1;
        if self.steps >= MAX_TOTAL_STEPS {
            return Outcome::Exhausted;
        }
        match &expression.kind {
            ExprKind::Int(value) => Outcome::Value(Value::Int(*value)),
            ExprKind::Int32(_) => Outcome::Unsupported(REASON_INT32_LITERAL),
            ExprKind::Float32(_) | ExprKind::Float64(_) => {
                Outcome::Unsupported(REASON_FLOAT_LITERAL)
            }
            ExprKind::Char(_) => Outcome::Unsupported(REASON_CHAR_LITERAL),
            ExprKind::Uint8(_) => Outcome::Unsupported(REASON_UINT8_LITERAL),
            ExprKind::Bool(value) => Outcome::Value(Value::Bool(*value)),
            ExprKind::MethodCall { .. } => Outcome::Unsupported(REASON_METHOD_CALL),
            ExprKind::String(_) => Outcome::Unsupported(REASON_ILL_TYPED_EXPRESSION),
            ExprKind::Var(name) => lookup(environment, name).map_or_else(
                || Outcome::Unsupported(REASON_UNRESOLVED_VARIABLE),
                Outcome::Value,
            ),
            ExprKind::Unary { op, value } => match self.evaluate(value, environment, depth) {
                Outcome::Value(Value::Bool(inner)) => match op {
                    UnaryOp::Not => Outcome::Value(Value::Bool(!inner)),
                    UnaryOp::Neg => Outcome::Unsupported(REASON_ILL_TYPED_EXPRESSION),
                },
                Outcome::Value(Value::Int(inner)) => match op {
                    UnaryOp::Neg => inner.checked_neg().map_or(
                        Outcome::Runtime(RuntimeReason::NegationOverflow),
                        |result| Outcome::Value(Value::Int(result)),
                    ),
                    UnaryOp::Not => Outcome::Unsupported(REASON_ILL_TYPED_EXPRESSION),
                },
                other => other,
            },
            ExprKind::Binary { op, left, right } => match op {
                BinaryOp::And => match self.evaluate(left, environment, depth) {
                    Outcome::Value(Value::Bool(false)) => Outcome::Value(Value::Bool(false)),
                    Outcome::Value(Value::Bool(true)) => self.evaluate(right, environment, depth),
                    other => other,
                },
                BinaryOp::Or => match self.evaluate(left, environment, depth) {
                    Outcome::Value(Value::Bool(true)) => Outcome::Value(Value::Bool(true)),
                    Outcome::Value(Value::Bool(false)) => self.evaluate(right, environment, depth),
                    other => other,
                },
                _ => {
                    let evaluated_left = self.evaluate(left, environment, depth);
                    let Outcome::Value(left_value) = evaluated_left else {
                        return evaluated_left;
                    };
                    let evaluated_right = self.evaluate(right, environment, depth);
                    let Outcome::Value(right_value) = evaluated_right else {
                        return evaluated_right;
                    };
                    combine_binary(*op, left_value, right_value)
                }
            },
            ExprKind::Call {
                name,
                type_arguments,
                args,
            } => {
                if !type_arguments.is_empty() {
                    return Outcome::Unsupported(REASON_GENERIC_CALL);
                }
                let Some(callee) = self.admitted.get(name.as_str()).copied() else {
                    return if self.names.contains(name.as_str()) {
                        Outcome::Unsupported(REASON_UNSUPPORTED_CALLEE)
                    } else {
                        Outcome::Unsupported(REASON_UNRESOLVED_CALL)
                    };
                };
                if depth >= MAX_CALL_DEPTH {
                    return Outcome::Runtime(RuntimeReason::CallDepthExceeded);
                }
                let mut arguments = Vec::with_capacity(args.len());
                for argument in args {
                    match self.evaluate(argument, environment, depth) {
                        Outcome::Value(value) => arguments.push(value),
                        other => return other,
                    }
                }
                if arguments.len() != callee.params.len() {
                    return Outcome::Unsupported(REASON_ILL_TYPED_EXPRESSION);
                }
                let mut frame: Environment = callee
                    .params
                    .iter()
                    .zip(arguments)
                    .map(|(param, value)| (param.name.clone(), value))
                    .collect();
                for clause in &callee.requires {
                    match self.evaluate(clause, &mut frame, depth + 1) {
                        Outcome::Value(Value::Bool(true)) => {}
                        Outcome::Value(Value::Bool(false)) => {
                            return Outcome::Runtime(RuntimeReason::CalleeRequiresViolated);
                        }
                        other => return other,
                    }
                }
                self.evaluate(&callee.body, &mut frame, depth + 1)
            }
            ExprKind::Block { statements, tail } => {
                let base = environment.len();
                let mut interrupted = None;
                for statement in statements {
                    match statement {
                        Statement::Let { name, value, .. } => {
                            match self.evaluate(value, environment, depth) {
                                Outcome::Value(value) => environment.push((name.clone(), value)),
                                other => {
                                    interrupted = Some(other);
                                    break;
                                }
                            }
                        }
                        Statement::Assign {
                            name, field, value, ..
                        } => {
                            // Field Mutation v1 targets stay outside the
                            // scalar property slice.
                            if field.is_some() {
                                interrupted = Some(Outcome::Unsupported(REASON_RECORD_PROJECTION));
                                break;
                            }
                            match self.evaluate(value, environment, depth) {
                                Outcome::Value(value) => {
                                    // Update the nearest binding of the name;
                                    // unknown targets are unsupported here.
                                    match environment
                                        .iter_mut()
                                        .rev()
                                        .find(|(bound, _)| bound == name)
                                    {
                                        Some((_, slot)) => *slot = value,
                                        None => {
                                            interrupted =
                                                Some(Outcome::Unsupported(REASON_ASSIGNMENT));
                                            break;
                                        }
                                    }
                                }
                                other => {
                                    interrupted = Some(other);
                                    break;
                                }
                            }
                        }
                        Statement::Unsafe { body, .. } => {
                            // The boundary's ordinary block body evaluates
                            // through this same block path; its scalar result
                            // is discarded.
                            match self.evaluate(body, environment, depth) {
                                Outcome::Value(_) => {}
                                other => {
                                    interrupted = Some(other);
                                    break;
                                }
                            }
                        }
                        Statement::While { .. } => {
                            // Property-test generation stays loop-free: the
                            // seeded candidate corpus never needs iteration,
                            // and unbounded evaluation would break the step
                            // budget contract for generated tests.
                            interrupted = Some(Outcome::Unsupported(REASON_WHILE_LOOP));
                            break;
                        }
                    }
                }
                let outcome =
                    interrupted.unwrap_or_else(|| self.evaluate(tail, environment, depth));
                environment.truncate(base);
                outcome
            }
            ExprKind::If {
                condition,
                then_branch,
                else_branch,
            } => match self.evaluate(condition, environment, depth) {
                Outcome::Value(Value::Bool(true)) => self.evaluate(then_branch, environment, depth),
                Outcome::Value(Value::Bool(false)) => {
                    self.evaluate(else_branch, environment, depth)
                }
                other => other,
            },
            ExprKind::ConstructRecord { .. } => Outcome::Unsupported(REASON_RECORD_CONSTRUCTION),
            ExprKind::ConstructVariant { .. } => Outcome::Unsupported(REASON_VARIANT_CONSTRUCTION),
            ExprKind::UpdateRecord { .. } => Outcome::Unsupported(REASON_RECORD_UPDATE),
            ExprKind::Project { .. } => Outcome::Unsupported(REASON_RECORD_PROJECTION),
            ExprKind::Match { .. } => Outcome::Unsupported(REASON_MATCH_EXPRESSION),
            ExprKind::Try { .. } => Outcome::Unsupported(REASON_TRY_EXPRESSION),
        }
    }

    fn analyze_function(
        &mut self,
        index: usize,
        function: &'a Function,
        options: &PropertyTestOptions,
    ) -> FunctionOutcome {
        if let Some(reason) = Self::admission(function) {
            return FunctionOutcome::Deferred(reason);
        }
        if let Some(reason) = self.scan_function_contracts_and_body(function) {
            return FunctionOutcome::Deferred(reason);
        }
        let parameter_kinds: Vec<ScalarKind> = function
            .params
            .iter()
            .map(|param| ScalarKind::of(&param.ty))
            .collect();
        let mut streams: Vec<u64> = function
            .params
            .iter()
            .enumerate()
            .map(|(position, _)| parameter_stream_seed(options.seed, index, position))
            .collect();
        let requires_json: Vec<String> = function
            .requires
            .iter()
            .enumerate()
            .map(|(index, clause)| {
                bformat!(
                    "{{\"index\":{},\"text\":{}}}",
                    index,
                    quote_json(&format::expr(clause, 0))
                )
            })
            .collect();
        let ensures_json: Vec<String> = function
            .ensures
            .iter()
            .enumerate()
            .map(|(index, clause)| {
                bformat!(
                    "{{\"index\":{},\"text\":{}}}",
                    index,
                    quote_json(&format::expr(clause, 0))
                )
            })
            .collect();

        let mut cases_attempted = 0usize;
        let mut filtered_cases = 0usize;
        let mut runtime_failure_cases = 0usize;
        let mut discharged_cases = 0usize;
        let mut runtime_reasons: BTreeSet<&'static str> = BTreeSet::new();
        let mut counterexample: Option<String> = None;

        for case_index in 0..options.max_cases {
            if self.steps >= MAX_TOTAL_STEPS {
                return FunctionOutcome::Exhausted;
            }
            cases_attempted += 1;
            let mut arguments = Vec::with_capacity(parameter_kinds.len());
            let mut environment: Environment = Vec::with_capacity(function.params.len());
            for (position, kind) in parameter_kinds.iter().enumerate() {
                let value = scalar_value(*kind, &mut streams[position], case_index);
                arguments.push((function.params[position].name.clone(), value));
                environment.push((function.params[position].name.clone(), value));
            }
            let arguments_json = arguments
                .iter()
                .map(|(name, value)| {
                    bformat!(
                        "{{\"name\":{},\"value\":\"{}\"}}",
                        quote_json(name),
                        value.render()
                    )
                })
                .collect::<Vec<_>>()
                .budgeted_join(",");

            let mut case_classified = false;
            for clause in function.requires.iter() {
                match self.evaluate(clause, &mut environment, 0) {
                    Outcome::Value(Value::Bool(true)) => {}
                    Outcome::Value(Value::Bool(false)) => {
                        filtered_cases += 1;
                        case_classified = true;
                        break;
                    }
                    Outcome::Value(Value::Int(_)) => {
                        return FunctionOutcome::Deferred(REASON_ILL_TYPED_EXPRESSION)
                    }
                    Outcome::Runtime(reason) => {
                        runtime_failure_cases += 1;
                        runtime_reasons.insert(reason.text());
                        case_classified = true;
                        break;
                    }
                    Outcome::Unsupported(reason) => return FunctionOutcome::Deferred(reason),
                    Outcome::Exhausted => return FunctionOutcome::Exhausted,
                }
            }
            if case_classified {
                continue;
            }
            match self.evaluate(&function.body, &mut environment, 0) {
                Outcome::Value(result_value) => {
                    environment.push(("result".to_owned(), result_value));
                    let mut ensured = true;
                    let mut found_counterexample = false;
                    for (clause_index, clause) in function.ensures.iter().enumerate() {
                        match self.evaluate(clause, &mut environment, 0) {
                            Outcome::Value(Value::Bool(true)) => {}
                            Outcome::Value(Value::Bool(false)) => {
                                counterexample = Some(bformat!(
                                    "{{\"index\":{},\"text\":{},\"arguments\":[{}],\"result\":\"{}\"}}",
                                    clause_index,
                                    quote_json(&format::expr(clause, 0)),
                                    arguments_json,
                                    result_value.render()
                                ));
                                ensured = false;
                                found_counterexample = true;
                                break;
                            }
                            Outcome::Value(Value::Int(_)) => {
                                return FunctionOutcome::Deferred(REASON_ILL_TYPED_EXPRESSION)
                            }
                            Outcome::Runtime(reason) => {
                                runtime_failure_cases += 1;
                                runtime_reasons.insert(reason.text());
                                ensured = false;
                                break;
                            }
                            Outcome::Unsupported(reason) => {
                                return FunctionOutcome::Deferred(reason)
                            }
                            Outcome::Exhausted => return FunctionOutcome::Exhausted,
                        }
                    }
                    environment.pop();
                    if found_counterexample {
                        break;
                    }
                    if ensured {
                        discharged_cases += 1;
                    }
                }
                Outcome::Runtime(reason) => {
                    runtime_failure_cases += 1;
                    runtime_reasons.insert(reason.text());
                }
                Outcome::Unsupported(reason) => return FunctionOutcome::Deferred(reason),
                Outcome::Exhausted => return FunctionOutcome::Exhausted,
            }
        }

        let runtime_reasons_json = runtime_reasons
            .iter()
            .map(|reason| bformat!("\"{reason}\""))
            .collect::<Vec<_>>();
        let found_counterexample = counterexample.is_some();
        let counterexample_json = counterexample.unwrap_or_else(|| "null".to_owned());
        let json = bformat!(
            "{{\"stable_id\":{},\"name\":{},\"outcome\":\"analyzed\",\
\"signature\":{{\"params\":[{}],\"result\":\"{}\"}},\
\"requires\":[{}],\"ensures\":[{}],\
\"cases_attempted\":{},\"filtered_cases\":{},\"runtime_failures\":{},\
\"runtime_reasons\":[{}],\"discharged_cases\":{},\"counterexample\":{}}}",
            quote_json(&function.stable_id),
            quote_json(&function.name),
            function
                .params
                .iter()
                .map(|param| {
                    bformat!(
                        "{{\"name\":{},\"type\":\"{}\"}}",
                        quote_json(&param.name),
                        scalar_type_text(&param.ty)
                    )
                })
                .collect::<Vec<_>>()
                .budgeted_join(","),
            scalar_type_text(&function.return_type),
            requires_json.budgeted_join(","),
            ensures_json.budgeted_join(","),
            cases_attempted,
            filtered_cases,
            runtime_failure_cases,
            runtime_reasons_json.budgeted_join(","),
            discharged_cases,
            counterexample_json,
        );
        FunctionOutcome::Analyzed(AnalyzedEntry {
            json,
            cases_attempted,
            filtered_cases,
            runtime_failure_cases,
            discharged_cases,
            counterexample: found_counterexample,
        })
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ScalarKind {
    Int,
    Bool,
}

impl ScalarKind {
    fn of(ty: &Type) -> Self {
        match ty {
            Type::I64 => ScalarKind::Int,
            Type::Bool => ScalarKind::Bool,
            Type::I32
            | Type::U8
            | Type::Char
            | Type::F32
            | Type::F64
            | Type::String
            | Type::Named { .. } => unreachable!(
                "ScalarKind::of called for unsupported type `{:?}`; admitted scalars are only i64 and bool",
                ty
            ),
        }
    }
}

fn scalar_type_text(ty: &Type) -> &'static str {
    match ty {
        Type::I64 => "i64",
        Type::Bool => "bool",
        Type::I32
        | Type::U8
        | Type::Char
        | Type::F32
        | Type::F64
        | Type::String
        | Type::Named { .. } => unreachable!(
            "scalar_type_text called for unsupported type `{:?}`; admitted scalars are only i64 and bool",
            ty
        ),
    }
}

fn scalar_value(kind: ScalarKind, state: &mut u64, case_index: usize) -> Value {
    match kind {
        ScalarKind::Int => {
            if case_index < I64_LATTICE.len() {
                Value::Int(I64_LATTICE[case_index])
            } else {
                Value::Int(next_sample(state) as i64)
            }
        }
        ScalarKind::Bool => {
            if case_index < 2 {
                Value::Bool(case_index == 0)
            } else {
                Value::Bool(next_sample(state) & 1 == 1)
            }
        }
    }
}

fn next_sample(state: &mut u64) -> u64 {
    let mut x = *state;
    x ^= x >> 12;
    x ^= x >> 25;
    x ^= x >> 27;
    *state = x;
    x.wrapping_mul(0x2545_F491_4F6C_DD1D)
}

fn splitmix64(state: &mut u64) -> u64 {
    *state = state.wrapping_add(0x9E37_79B9_7F4A_7C15);
    let mut z = *state;
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^ (z >> 31)
}

fn parameter_stream_seed(base: u64, function_index: usize, parameter_index: usize) -> u64 {
    let mut state = base
        ^ ((function_index as u64).wrapping_mul(0xA24B_AED4_963E_E407))
        ^ ((parameter_index as u64).wrapping_mul(0x9FB2_1C65_1E98_DF25));
    splitmix64(&mut state)
}

fn combine_binary(op: BinaryOp, left: Value, right: Value) -> Outcome {
    match (left, right) {
        (Value::Int(left), Value::Int(right)) => match op {
            BinaryOp::Add => checked_int(left.checked_add(right)),
            BinaryOp::Sub => checked_int(left.checked_sub(right)),
            BinaryOp::Mul => checked_int(left.checked_mul(right)),
            BinaryOp::Div => {
                if right == 0 {
                    Outcome::Runtime(RuntimeReason::DivisionByZero)
                } else {
                    checked_int(left.checked_div(right))
                }
            }
            BinaryOp::Rem => {
                if right == 0 {
                    Outcome::Runtime(RuntimeReason::RemainderByZero)
                } else {
                    checked_int(left.checked_rem(right))
                }
            }
            BinaryOp::Eq => Outcome::Value(Value::Bool(left == right)),
            BinaryOp::Ne => Outcome::Value(Value::Bool(left != right)),
            BinaryOp::Lt => Outcome::Value(Value::Bool(left < right)),
            BinaryOp::Le => Outcome::Value(Value::Bool(left <= right)),
            BinaryOp::Gt => Outcome::Value(Value::Bool(left > right)),
            BinaryOp::Ge => Outcome::Value(Value::Bool(left >= right)),
            BinaryOp::And | BinaryOp::Or => Outcome::Unsupported(REASON_ILL_TYPED_EXPRESSION),
        },
        (Value::Bool(left), Value::Bool(right)) => match op {
            BinaryOp::Eq => Outcome::Value(Value::Bool(left == right)),
            BinaryOp::Ne => Outcome::Value(Value::Bool(left != right)),
            _ => Outcome::Unsupported(REASON_ILL_TYPED_EXPRESSION),
        },
        _ => Outcome::Unsupported(REASON_ILL_TYPED_EXPRESSION),
    }
}

fn checked_int(value: Option<i64>) -> Outcome {
    match value {
        Some(value) => Outcome::Value(Value::Int(value)),
        None => Outcome::Runtime(RuntimeReason::ArithmeticOverflow),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicUsize, Ordering};

    static COUNTER: AtomicUsize = AtomicUsize::new(0);

    fn write_temp(source: &str) -> PathBuf {
        let path = std::env::temp_dir().join(format!(
            "semaprax-property-tests-{}-{}.spx",
            std::process::id(),
            COUNTER.fetch_add(1, Ordering::SeqCst)
        ));
        std::fs::write(&path, source).unwrap();
        path
    }

    #[allow(dead_code)]
    fn cleanup(path: &Path) {
        let _ = std::fs::remove_file(path);
    }

    const VALID_SOURCE: &str = r#"
module test.probe;

@id("probe.ok")
fn ok(value: i64) -> bool
    ensures result == true
{
    true
}

@id("app.main")
fn main() -> i64
    ensures result == 1
{
    if ok(3) { 1 } else { 0 }
}
"#;

    #[test]
    fn options_reject_out_of_bounds_values() {
        assert!(PropertyTestOptions::new(0, 8, DEFAULT_MAX_BYTES, 1).is_err());
        assert!(PropertyTestOptions::new(MAX_CASES_LIMIT + 1, 8, DEFAULT_MAX_BYTES, 1).is_err());
        assert!(PropertyTestOptions::new(8, 0, DEFAULT_MAX_BYTES, 1).is_err());
        assert!(
            PropertyTestOptions::new(8, MAX_FUNCTIONS_LIMIT + 1, DEFAULT_MAX_BYTES, 1).is_err()
        );
        assert!(PropertyTestOptions::new(8, 8, 512, 1).is_err());
        assert!(PropertyTestOptions::new(8, 8, graph::MAX_AGENT_CONTEXT_BYTES + 1, 1).is_err());
        assert!(PropertyTestOptions::new(1, 1, graph::MIN_AGENT_CONTEXT_BYTES, u64::MAX).is_ok());
    }

    #[test]
    fn defaults_are_stable() {
        let options = PropertyTestOptions::default();
        assert_eq!(options.max_cases, 64);
        assert_eq!(options.max_functions, 64);
        assert_eq!(options.max_bytes, 64 * 1024);
        assert_eq!(options.seed, DEFAULT_SEED);
    }

    #[test]
    fn stream_seeds_are_stable_and_distinct() {
        assert_eq!(
            parameter_stream_seed(7, 3, 1),
            parameter_stream_seed(7, 3, 1)
        );
        assert_ne!(
            parameter_stream_seed(7, 3, 1),
            parameter_stream_seed(7, 3, 2)
        );
        assert_ne!(
            parameter_stream_seed(7, 1, 1),
            parameter_stream_seed(7, 2, 1)
        );
        let mut first = parameter_stream_seed(DEFAULT_SEED, 0, 0);
        let a = next_sample(&mut first);
        let mut second = parameter_stream_seed(DEFAULT_SEED, 0, 0);
        let b = next_sample(&mut second);
        assert_eq!(a, b);
    }

    #[test]
    fn lattice_covers_boundaries_before_sampling() {
        let mut state = parameter_stream_seed(DEFAULT_SEED, 0, 0);
        assert_eq!(scalar_value(ScalarKind::Int, &mut state, 0), Value::Int(0));
        assert_eq!(
            scalar_value(ScalarKind::Int, &mut state, 7),
            Value::Int(i64::MIN)
        );
        assert_eq!(
            scalar_value(ScalarKind::Bool, &mut state, 0),
            Value::Bool(true)
        );
        assert_eq!(
            scalar_value(ScalarKind::Bool, &mut state, 1),
            Value::Bool(false)
        );
    }

    #[test]
    fn parse_errors_surface_as_diagnostics() {
        let path = write_temp("this is not semaprax");
        let outcome = generate(&path, &PropertyTestOptions::default());
        assert!(outcome.is_err());
        cleanup(&path);
    }

    #[test]
    fn verification_errors_fail_closed() {
        let source = r#"
module test.probe;

@id("probe.bad")
fn bad(value: i64) -> i64
    ensures result == missing
{
    value
}
"#;
        let path = write_temp(source);
        let outcome = generate(&path, &PropertyTestOptions::default());
        let errors = outcome.expect_err("verification errors must fail closed");
        assert!(errors.iter().any(|item| item.severity.is_error()));
        cleanup(&path);
    }

    #[test]
    fn drift_after_parse_fails_closed() {
        let path = write_temp(VALID_SOURCE);
        let mut mutate = |_phase: HookPhase, canonical: &Path| {
            let mut current = std::fs::read_to_string(canonical)?;
            current.push('\n');
            std::fs::write(canonical, current)
        };
        let outcome = generate_with_hook(&path, &PropertyTestOptions::default(), &mut mutate);
        assert!(outcome.is_err(), "drift after parse must reject the report");
        cleanup(&path);
    }

    #[test]
    fn clean_hooks_preserve_success() {
        let path = write_temp(VALID_SOURCE);
        let mut noop = |_phase, _canonical: &Path| Ok(());
        let outcome = generate_with_hook(&path, &PropertyTestOptions::default(), &mut noop);
        let report = outcome.expect("clean hooks must not interfere");
        assert!(report.contains(SCHEMA));
        cleanup(&path);
    }
}
