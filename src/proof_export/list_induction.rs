//! LAW-08: a deliberately closed, source-authenticated list induction lane.
//! The mathematical `List Int` model describes successful executions of the
//! checked `Iter<i64>`/`Vec<i64>` carrier; it grants no runtime or ABI proof.
use super::{kernel_report, LeanKernel};
use crate::ast::{Expr, ExprKind, Function, MatchMode, MatchPattern, ParamMode, Program, Type};
use crate::diagnostic::Diagnostic;
use serde::{Deserialize, Serialize};
use sha2::{Digest as _, Sha256};

pub const PROFILE: &str = "semaprax.list-induction-i64.v1";
pub const CERTIFICATE_SCHEMA: &str = "semaprax.list-induction-certificate.v1";
pub const PROOF_MODULE_SCHEMA: &str = "semaprax.list-induction-proof-module.v1";
const NAMESPACE: &str = "SemapraxLaw08";
const THEOREMS: [(&str, &str, &str); 5] = [
    (
        "append_eq",
        "list.append",
        "append left suffix = left ++ suffix",
    ),
    ("append_empty", "list.append", "append [] input = input"),
    (
        "reverse_eq",
        "list.reverse",
        "reverse input = input.reverse",
    ),
    (
        "reverse_length",
        "list.reverse",
        "(reverse input).length = input.length",
    ),
    (
        "reverse_involution",
        "list.reverse",
        "reverse (reverse input) = input",
    ),
];

/// The only theorem names and source declaration identities admitted by this
/// closed profile. LawSet selection cannot relabel a proof as another body.
pub fn declaration_for_theorem(theorem: &str) -> Option<&'static str> {
    THEOREMS
        .iter()
        .find(|(name, _, _)| *name == theorem)
        .map(|(_, declaration, _)| *declaration)
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProofModule {
    pub schema: String,
    pub append_eq: String,
    pub append_empty: String,
    pub reverse_eq: String,
    pub reverse_length: String,
    pub reverse_involution: String,
}

impl ProofModule {
    fn bodies(&self) -> [&str; 5] {
        [
            &self.append_eq,
            &self.append_empty,
            &self.reverse_eq,
            &self.reverse_length,
            &self.reverse_involution,
        ]
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct CoverageRow {
    pub stable_id: String,
    pub outcome: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Certificate {
    pub schema: String,
    pub profile: String,
    pub source_sha256: String,
    pub definitions_sha256: String,
    pub proof_module_sha256: String,
    pub lean_source_sha256: String,
    pub theorem_law_ids: Vec<(String, String)>,
    pub coverage: Vec<CoverageRow>,
    pub proof_module: ProofModule,
    pub lean_source: String,
    pub axioms: Vec<(String, Vec<String>)>,
    pub nonclaims: Vec<String>,
}

fn refused(reason: &str) -> Diagnostic {
    Diagnostic::io("SPX-LI001", format!("list induction refused: {reason}"))
}

fn digest(bytes: &[u8]) -> String {
    let bytes = Sha256::digest(bytes);
    let mut out = String::from("sha256:");
    for byte in bytes {
        use std::fmt::Write as _;
        write!(&mut out, "{byte:02x}").expect("String formatting");
    }
    out
}

fn sequence(ty: &Type, name: &str) -> bool {
    matches!(ty, Type::Named { name: found, arguments }
        if found == name && arguments.as_slice() == [Type::I64])
}

fn variable(expr: &Expr, name: &str) -> bool {
    matches!(&expr.kind, ExprKind::Var(found) if found == name)
}

fn call<'a>(expr: &'a Expr, name: &str, type_arguments: &[Type]) -> Option<&'a [Expr]> {
    match &expr.kind {
        ExprKind::Call {
            name: found,
            type_arguments: found_types,
            args,
        } if found == name && found_types == type_arguments => Some(args),
        _ => None,
    }
}

fn call1(expr: &Expr, name: &str, type_arguments: &[Type], arg: impl Fn(&Expr) -> bool) -> bool {
    call(expr, name, type_arguments).is_some_and(|args| matches!(args, [value] if arg(value)))
}

fn tail(expr: &Expr) -> Option<&Expr> {
    match &expr.kind {
        ExprKind::Block { statements, tail } if statements.is_empty() => Some(tail),
        _ => None,
    }
}

fn list_match<'a>(
    function: &'a Function,
    input: &str,
) -> Option<(&'a Expr, &'a Expr, String, String)> {
    let ExprKind::Match {
        mode: MatchMode::Own,
        scrutinee,
        arms,
    } = &tail(&function.body)?.kind
    else {
        return None;
    };
    if !call1(scrutinee, "iter_next", &[Type::I64], |value| {
        variable(value, input)
    }) || arms.len() != 2
        || arms.iter().any(|arm| arm.guard.is_some())
    {
        return None;
    }
    let MatchPattern::Variant {
        type_name: done_type,
        case_name: done_case,
        fields: done_fields,
        ..
    } = &arms[0].pattern
    else {
        return None;
    };
    let MatchPattern::Variant {
        type_name: yield_type,
        case_name: yield_case,
        fields,
        ..
    } = &arms[1].pattern
    else {
        return None;
    };
    if done_type != "IterStep"
        || done_case != "Done"
        || !done_fields.is_empty()
        || yield_type != "IterStep"
        || yield_case != "Yield"
        || fields.len() != 2
        || fields[0].name != "item"
        || fields[1].name != "rest"
        || fields[0].binding == fields[1].binding
    {
        return None;
    }
    Some((
        &arms[0].value,
        &arms[1].value,
        fields[0].binding.clone(),
        fields[1].binding.clone(),
    ))
}

fn pure(function: &Function) -> bool {
    function.explicit_id
        && function.type_parameters.is_empty()
        && function.effects.is_empty()
        && function.yields.is_none()
        && function.follows.is_none()
        && function.requires.is_empty()
        && function.ensures.is_empty()
}

fn reverse_shape(function: &Function) -> bool {
    if function.stable_id != "list.reverse"
        || !pure(function)
        || function.params.len() != 1
        || !sequence(&function.return_type, "Vec")
        || function.params[0].mode != ParamMode::Own
        || !sequence(&function.params[0].ty, "Iter")
    {
        return false;
    }
    let Some((done, yielded, item, rest)) = list_match(function, &function.params[0].name) else {
        return false;
    };
    let empty = call1(done, "vec_with_capacity", &[Type::I64], |value| {
        matches!(value.kind, ExprKind::Usize(8192))
    });
    let recursive = |expr: &Expr| call1(expr, &function.name, &[], |value| variable(value, &rest));
    let step = call(yielded, "vec_push", &[Type::I64]).is_some_and(
        |args| matches!(args, [first, second] if recursive(first) && variable(second, &item)),
    );
    empty && step
}

fn append_shape(function: &Function) -> bool {
    if function.stable_id != "list.append"
        || !pure(function)
        || function.params.len() != 2
        || !sequence(&function.return_type, "Vec")
        || function.params[0].mode != ParamMode::Own
        || !sequence(&function.params[0].ty, "Vec")
        || function.params[1].mode != ParamMode::Own
        || !sequence(&function.params[1].ty, "Iter")
    {
        return false;
    }
    let left = &function.params[0].name;
    let Some((done, yielded, item, rest)) = list_match(function, &function.params[1].name) else {
        return false;
    };
    if !variable(done, left) {
        return false;
    }
    call(yielded, &function.name, &[]).is_some_and(|args| {
        matches!(args, [first, second]
            if call(first, "vec_push", &[Type::I64]).is_some_and(|push| matches!(push, [values, value]
                if variable(values, left) && variable(value, &item))) && variable(second, &rest))
    })
}

fn checked_source(program: &Program) -> Result<(), Diagnostic> {
    if let Some(error) = crate::verify::verify(program).into_iter().next() {
        return Err(error);
    }
    let hir = crate::hir::resolve(program).map_err(|errors| {
        errors
            .into_iter()
            .next()
            .unwrap_or_else(|| refused("HIR resolution failed"))
    })?;
    crate::hir::validate(&hir).map_err(|_| refused("HIR replay failed"))
}

fn definitions(program: &Program) -> Result<(String, Vec<CoverageRow>), Diagnostic> {
    checked_source(program)?;
    let reverse = program
        .functions
        .iter()
        .find(|function| function.stable_id == "list.reverse")
        .ok_or_else(|| refused("source reverse declaration absent"))?;
    let append = program
        .functions
        .iter()
        .find(|function| function.stable_id == "list.append")
        .ok_or_else(|| refused("source append declaration absent"))?;
    if !reverse_shape(reverse) || !append_shape(append) {
        return Err(refused(
            "source list definition is outside the direct-tail profile",
        ));
    }
    let mut coverage = Vec::new();
    for function in &program.functions {
        coverage.push(CoverageRow {
            stable_id: function.stable_id.clone(),
            outcome: if matches!(function.stable_id.as_str(), "list.append" | "list.reverse") {
                "exported_direct_tail".into()
            } else {
                "unsupported_function".into()
            },
        });
    }
    for declaration in &program.types {
        coverage.push(CoverageRow {
            stable_id: declaration.stable_id.clone(),
            outcome: "unsupported_type_declaration".into(),
        });
    }
    for declaration in &program.interfaces {
        coverage.push(CoverageRow {
            stable_id: declaration.stable_id.clone(),
            outcome: "unsupported_interface".into(),
        });
    }
    for declaration in &program.protocols {
        coverage.push(CoverageRow {
            stable_id: declaration.stable_id.clone(),
            outcome: "unsupported_protocol".into(),
        });
    }
    for declaration in &program.implementations {
        coverage.push(CoverageRow {
            stable_id: declaration.stable_id.clone(),
            outcome: "unsupported_implementation".into(),
        });
    }
    for declaration in &program.agents {
        coverage.push(CoverageRow {
            stable_id: declaration.stable_id.clone(),
            outcome: "unsupported_agent".into(),
        });
    }
    for declaration in &program.session_protocols {
        coverage.push(CoverageRow {
            stable_id: declaration.stable_id.clone(),
            outcome: "unsupported_session_protocol".into(),
        });
    }
    coverage.sort_by(|left, right| left.stable_id.cmp(&right.stable_id));
    let source = format!("import Init\n/- {PROFILE}; source bodies authenticated as direct Iter<i64> tail recursion.\n   Elements are exact i64 values modeled as Lean Int. Runtime capacity, call depth,\n   checked usize arithmetic, lowering, and public ABI are NOT kernel theorems. -/\nnamespace {NAMESPACE}\ndef append (left suffix : List Int) : List Int :=\n  match suffix with\n  | [] => left\n  | item :: rest => append (left ++ [item]) rest\ntermination_by suffix.length\ndef reverse (input : List Int) : List Int :=\n  match input with\n  | [] => []\n  | item :: rest => (reverse rest) ++ [item]\ntermination_by input.length\n");
    Ok((source, coverage))
}

fn admissible_tactic(body: &str) -> bool {
    if body.is_empty()
        || body.len() > 4096
        || body.lines().count() > 64
        || body.contains('#')
        || body.chars().any(|character| {
            character == '\t' || character == '\r' || character.is_control() && character != '\n'
        })
    {
        return false;
    }
    let forbidden = [
        "sorry",
        "admit",
        "axiom",
        "unsafe",
        "run_tac",
        "run_term_elab_m",
        "theorem",
        "def",
        "opaque",
        "constant",
        "instance",
        "import",
        "namespace",
        "macro",
        "syntax",
        "set_option",
        "elab",
    ];
    !body
        .split(|character: char| !character.is_ascii_alphanumeric() && character != '_')
        .any(|word| forbidden.contains(&word.to_ascii_lowercase().as_str()))
}

fn render(
    program: &Program,
    proofs: &ProofModule,
) -> Result<(String, String, Vec<CoverageRow>), Diagnostic> {
    if proofs.schema != PROOF_MODULE_SCHEMA
        || proofs.bodies().iter().any(|body| !admissible_tactic(body))
    {
        return Err(refused(
            "proof module has an unsupported declaration or tactic",
        ));
    }
    let (definitions, coverage) = definitions(program)?;
    let mut source = definitions.clone();
    for ((name, _, statement), body) in THEOREMS.iter().zip(proofs.bodies()) {
        let parameters = if name.starts_with("append_") {
            "(left suffix : List Int)"
        } else {
            "(input : List Int)"
        };
        let statement = if *name == "append_empty" {
            "append [] input = input"
        } else {
            statement
        };
        let parameters = if *name == "append_empty" {
            "(input : List Int)"
        } else {
            parameters
        };
        source.push_str(&format!(
            "theorem {name} {parameters} : {statement} := by\n"
        ));
        for line in body.lines() {
            source.push_str("  ");
            source.push_str(line);
            source.push('\n');
        }
        source.push('\n');
    }
    for (name, _, _) in THEOREMS {
        source.push_str(&format!("#print axioms {name}\n"));
    }
    source.push_str(&format!("end {NAMESPACE}\n"));
    Ok((source, definitions, coverage))
}

pub fn prove(
    program: &Program,
    proofs: &ProofModule,
    kernel: &impl LeanKernel,
) -> Result<Certificate, Diagnostic> {
    let (lean_source, definitions, coverage) = render(program, proofs)?;
    let run = kernel.check(&lean_source)?;
    let expected = THEOREMS
        .iter()
        .map(|(name, _, _)| format!("{NAMESPACE}.{name}"))
        .collect::<Vec<_>>();
    let kernel_report::KernelVerdict::Checked { axioms } =
        kernel_report::parse(&expected, &run.toolchain, &run.output)
    else {
        return Err(refused(
            "pinned kernel did not confirm the complete fixed law inventory",
        ));
    };
    let canonical = crate::format::canonical(program);
    let proof_bytes = serde_json::to_vec(proofs).expect("closed proof module JSON");
    Ok(Certificate {
        schema: CERTIFICATE_SCHEMA.into(),
        profile: PROFILE.into(),
        source_sha256: digest(canonical.as_bytes()),
        definitions_sha256: digest(definitions.as_bytes()),
        proof_module_sha256: digest(&proof_bytes),
        lean_source_sha256: digest(lean_source.as_bytes()),
        theorem_law_ids: THEOREMS
            .iter()
            .map(|(name, law, _)| (format!("{NAMESPACE}.{name}"), (*law).into()))
            .collect(),
        coverage,
        proof_module: proofs.clone(),
        lean_source,
        axioms,
        nonclaims: vec![
            "no_runtime_or_lowering_proof".into(),
            "no_public_list_abi".into(),
            "runtime_length_and_capacity_are_checked_bounded".into(),
        ],
    })
}

/// Replay the self-contained certificate envelope. This does not attest that
/// a separately stored proof module is still the current authored module.
pub fn verify(
    program: &Program,
    certificate: &Certificate,
    kernel: &impl LeanKernel,
) -> Result<(), Diagnostic> {
    if certificate.schema != CERTIFICATE_SCHEMA || certificate.profile != PROFILE {
        return Err(refused("certificate schema or profile drift"));
    }
    let (lean_source, definitions, coverage) = render(program, &certificate.proof_module)?;
    let proof_bytes =
        serde_json::to_vec(&certificate.proof_module).expect("closed proof module JSON");
    if certificate.source_sha256 != digest(crate::format::canonical(program).as_bytes())
        || certificate.definitions_sha256 != digest(definitions.as_bytes())
        || certificate.proof_module_sha256 != digest(&proof_bytes)
        || certificate.lean_source_sha256 != digest(lean_source.as_bytes())
        || certificate.lean_source != lean_source
        || certificate.coverage != coverage
        || certificate.theorem_law_ids
            != THEOREMS
                .iter()
                .map(|(name, law, _)| (format!("{NAMESPACE}.{name}"), (*law).into()))
                .collect::<Vec<_>>()
    {
        return Err(refused(
            "source, definition, proof module or law association drift before kernel",
        ));
    }
    let current = prove(program, &certificate.proof_module, kernel)?;
    if &current != certificate {
        return Err(refused(
            "source, definition, proof module, law association or kernel replay drift",
        ));
    }
    Ok(())
}

/// Replay against the caller's current, separately held proof module. A stale
/// authored lemma is refused before invoking the kernel, even when the old
/// self-contained certificate envelope still replays successfully.
pub fn verify_against_module(
    program: &Program,
    current_proofs: &ProofModule,
    certificate: &Certificate,
    kernel: &impl LeanKernel,
) -> Result<(), Diagnostic> {
    if current_proofs != &certificate.proof_module {
        return Err(refused(
            "current proof module differs from certified proof module",
        ));
    }
    verify(program, certificate, kernel)
}
