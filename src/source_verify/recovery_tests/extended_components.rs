//! Raw declaration components remain distinct from valid result and nominal facts.
use crate::ast::program_source::ProgramSource;
use crate::diagnostic::Diagnostic;
use crate::source_verify::{
    declared_type::check_declared_type, type_table::TypeTable, IterativeVerifier,
};
use std::collections::{HashMap, HashSet};
use std::path::Path;

fn source(declarations: &str, body: &str) -> String {
    format!("module t; {declarations} @id(\"t.main\") fn main()->i64{{{body}}}")
}
fn codes(diagnostics: &[Diagnostic]) -> Vec<&str> {
    diagnostics.iter().map(|d| d.code).collect()
}
fn receive(source: &str, before: bool) -> (Option<crate::ast::Type>, Vec<Diagnostic>) {
    receive_function(source, before, "main")
}
fn receive_function(
    source: &str,
    before: bool,
    name: &str,
) -> (Option<crate::ast::Type>, Vec<Diagnostic>) {
    receive_function_route(source, before, name, false)
}
fn receive_function_route(
    source: &str,
    before: bool,
    name: &str,
    previous_parameter: bool,
) -> (Option<crate::ast::Type>, Vec<Diagnostic>) {
    let program = crate::parse(source, Path::new("extended-components.spx")).unwrap();
    let types = TypeTable::new(&program);
    let function_cells = crate::source_verify::capacity::source_capacity_functions(&program);
    let functions = function_cells
        .iter()
        .take(program.functions.len())
        .map(|(_, function)| (function.name(), function))
        .collect::<crate::source_verify::function_map::FunctionMap<'_>>();
    let main = functions[name].payload();
    let mut diagnostics = Vec::new();
    // Real signature checker creates the original receipts before body checking.
    for function in program.functions() {
        let parameters = function
            .type_parameters
            .iter()
            .map(|parameter| parameter.name.as_str())
            .collect::<HashSet<_>>();
        check_declared_type(
            &program,
            &function.return_type,
            function.span,
            &types,
            &parameters,
            &mut diagnostics,
        );
        for parameter in &function.params {
            check_declared_type(
                &program,
                &parameter.ty,
                parameter.span,
                &types,
                &parameters,
                &mut diagnostics,
            );
        }
    }
    for declaration in program.types() {
        let parameters = declaration
            .type_parameters
            .iter()
            .map(|p| p.name.as_str())
            .collect::<HashSet<_>>();
        let mut fields = Vec::new();
        match &declaration.kind {
            crate::ast::TypeDeclarationKind::Record { fields: declared }
            | crate::ast::TypeDeclarationKind::Class {
                fields: declared, ..
            } => fields.extend(declared),
            crate::ast::TypeDeclarationKind::Variant { cases } => {
                for case in cases {
                    fields.extend(&case.fields);
                }
            }
            crate::ast::TypeDeclarationKind::Resource { .. } => {}
        }
        for field in fields {
            check_declared_type(
                &program,
                &field.ty,
                field.span,
                &types,
                &parameters,
                &mut diagnostics,
            );
        }
        if let crate::ast::TypeDeclarationKind::Class { methods, .. } = &declaration.kind {
            for method in methods {
                check_declared_type(
                    &program,
                    &method.return_type,
                    method.span,
                    &types,
                    &parameters,
                    &mut diagnostics,
                );
                for parameter in &method.params {
                    check_declared_type(
                        &program,
                        &parameter.ty,
                        parameter.span,
                        &types,
                        &parameters,
                        &mut diagnostics,
                    );
                }
            }
        }
    }
    let variables = main
        .params
        .iter()
        .map(|parameter| {
            (
                parameter.name.clone(),
                crate::source_verify::binding::Binding {
                    contract_local: false,
                    traversal_row: None,
                    ty: parameter.ty.clone(),
                    mode: parameter.mode,
                    availability: crate::source_verify::binding::Availability::Available,
                    moved_places: HashMap::new(),
                    definitely_partial: HashSet::new(),
                    native_unit_discard: false,
                    mutable: false,
                    active_loans: std::collections::BTreeSet::new(),
                    borrow_origin: None,
                },
            )
        })
        .collect();
    let previous_recovery = previous_parameter.then(|| {
        crate::source_verify::recovery::Recovery::before_parameter_signature(
            &program,
            main,
            &variables,
            &types,
            &diagnostics,
        )
    });
    let mut verifier = IterativeVerifier::new(
        &program,
        functions
            .get(main.name.as_str())
            .expect("frozen current declaration cell"),
        variables,
        &functions,
        &types,
        None,
        true,
        &mut diagnostics,
    );
    verifier.extended_before_799 = before;
    if let Some(previous_recovery) = previous_recovery {
        // Complete archived parameter, component and operator receivers are
        // required: changing only the component route retains today's signature.
        verifier.recovery = previous_recovery;
        verifier.extended_before_799 = true;
        verifier.operators_before_799 = true;
    }
    let value = verifier.run(&main.body).unwrap().map(|value| value.ty);
    drop(verifier);
    (value, diagnostics)
}

#[test]
fn specialized_raw_parameter_refusal_keeps_known_argument_and_result_diagnostics() {
    let source = self::source(
        "@id(\"t.f\") fn f<T>(a:T,b:Missing,c:bool)->i64{0}",
        "if f<i64>(1,2,3){1}else{0}",
    );
    let old = receive(&source, true);
    let current = receive(&source, false);
    assert_eq!(
        codes(&old.1),
        ["SPX-T220", "SPX-T205", "SPX-T205", "SPX-T210"]
    );
    assert_eq!(codes(&current.1), ["SPX-T220", "SPX-T205", "SPX-T210"]);
    assert_eq!(
        current.1,
        vec![old.1[0].clone(), old.1[2].clone(), old.1[3].clone()]
    );
    assert_eq!(current.0, old.0);
    let program = crate::parse(&source, Path::new("extended-components.spx")).unwrap();
    let types = TypeTable::new(&program);
    let function = &program.functions[0];
    let receipt = vec![old.1[0].clone()];
    let recovery = crate::source_verify::recovery::Recovery::signature(
        &program,
        &program.functions[1],
        &HashMap::new(),
        &types,
        &receipt,
    );
    assert_eq!(
        recovery.before_actual_callee_component_refusal(
            function,
            Some(1),
            &program,
            &types,
            &receipt
        ),
        None
    );
    assert_eq!(
        recovery.callee_component_refusal(function, Some(1), &program, &types, &receipt),
        Some(0)
    );
    for field in 0..6 {
        let mut forged = receipt.clone();
        match field {
            0 => forged[0].code = "SPX-T001",
            1 => forged[0].severity = crate::diagnostic::Severity::Warning,
            2 => forged[0].message = "forged".into(),
            3 => forged[0].path = Some("forged.spx".to_owned()),
            4 => forged[0].span = None,
            5 => forged[0].help = Some("forged".to_owned()),
            _ => unreachable!(),
        }
        assert_eq!(
            recovery.callee_component_refusal(function, Some(1), &program, &types, &forged),
            None
        );
    }
}

#[test]
fn specialized_unknown_return_withdraws_alias_without_erasing_sibling_refusal() {
    let source = self::source(
        "@id(\"t.f\") fn f<T>(a:T)->Missing{0}",
        "let alias=f<i64>(1); alias+missing_sibling",
    );
    let old = receive(&source, true);
    let current = receive(&source, false);
    assert_eq!(codes(&current.1), ["SPX-T220", "SPX-T202"]);
    assert_eq!(current.1[0], old.1[0]);
    assert_eq!(
        current.1[1],
        *old.1.iter().find(|d| d.code == "SPX-T202").unwrap()
    );
    assert_eq!(current.0, None);
}

#[test]
fn admitted_specialized_and_method_successes_keep_complete_old_values_and_receipts() {
    for (declarations, body) in [
        ("@id(\"t.f\") fn f<T>(a:T)->T{a}", "f<i64>(1)+2"),
        ("@id(\"t.c\") class C{@id(\"t.x\") x:i64,@id(\"t.get\") fn get(self:C,a:i64)->i64{self.x+a}}", "let c=C{x:1}; c.get(2)"),
        ("@id(\"t.r\") record R{@id(\"t.x\") x:i64,}", "let r=R{x:1}; r.x+2"),
    ] {
        let source = self::source(declarations, body);
        assert_eq!(receive(&source, false), receive(&source, true));
        let program=crate::parse(&source, Path::new("extended-components.spx")).unwrap();
        assert!(crate::source_verify::verify(&program).is_empty());
    }
}

#[test]
fn method_parameter_refusal_uses_raw_ordinal_after_self_and_retains_known_result() {
    let source=self::source("@id(\"t.c\") class C{@id(\"t.x\") x:i64,@id(\"t.f\") fn f(self:C,a:Missing,b:bool)->i64{0}}", "let c=C{x:1}; if c.f(2,3){1}else{0}");
    let old = receive(&source, true);
    let current = receive(&source, false);
    assert_eq!(
        codes(&old.1),
        ["SPX-T001", "SPX-T205", "SPX-T205", "SPX-T210"]
    );
    assert_eq!(
        current.1,
        vec![old.1[0].clone(), old.1[2].clone(), old.1[3].clone()]
    );
    assert_eq!(current.0, old.0);
}

#[test]
fn method_unknown_return_withdraws_zero_and_nonzero_argument_aliases() {
    for (params, args) in [("self:C", ""), ("self:C,a:i64", "1")] {
        let declarations = format!(
            "@id(\"t.c\") class C{{@id(\"t.x\") x:i64,@id(\"t.f\") fn f({params})->Missing{{0}}}}"
        );
        let source = self::source(
            &declarations,
            &format!("let c=C{{x:1}}; let alias=c.f({args}); alias+1"),
        );
        let old = receive(&source, true);
        let current = receive(&source, false);
        assert_eq!(codes(&current.1), ["SPX-T001"]);
        assert_eq!(current.1[0], old.1[0]);
        assert!(old.1.len() > current.1.len());
        assert_eq!(current.0, None);
    }
}

#[test]
fn record_component_refusal_keeps_nominal_value_and_known_field_mismatch() {
    let declarations =
        "@id(\"t.r\") record R{@id(\"t.bad\") bad:Missing,@id(\"t.known\") known:bool,}";
    let original = self::source(declarations, "if R{bad:1,known:2}{1}else{0}");
    let refused = crate::parse(&original, Path::new("extended-components.spx")).unwrap_err();
    assert_eq!(
        refused,
        Diagnostic::error(
            "SPX-P106",
            "expected `}` after block",
            crate::ast::Span {
                start: 120,
                end: 121,
                line: 1,
                column: 121
            }
        )
        .at_path("extended-components.spx")
    );
    let source = self::source(declarations, "if (R{bad:1,known:2}){1}else{0}");
    let old = receive(&source, true);
    let current = receive(&source, false);
    assert_eq!(
        codes(&old.1),
        ["SPX-T001", "SPX-T215", "SPX-T215", "SPX-T210"]
    );
    assert_eq!(
        current.1,
        vec![old.1[0].clone(), old.1[2].clone(), old.1[3].clone()]
    );
    assert_eq!(current.0, old.0);
}

#[test]
fn record_projection_refusal_withdraws_only_unknown_component_and_keeps_sibling() {
    let source = self::source(
        "@id(\"t.r\") record R{@id(\"t.bad\") bad:Missing,@id(\"t.known\") known:i64,}",
        "let r=R{bad:1,known:2}; let alias=r.bad; alias+missing_sibling",
    );
    let old = receive(&source, true);
    let current = receive(&source, false);
    assert_eq!(codes(&current.1), ["SPX-T001", "SPX-T202"]);
    assert_eq!(current.1[0], old.1[0]);
    assert_eq!(
        current.1[1],
        *old.1.iter().find(|d| d.code == "SPX-T202").unwrap()
    );
    assert_eq!(current.0, None);
}

#[test]
fn field_receipt_requires_exact_six_fields_and_in_scope_generic_parameters() {
    let source = self::source(
        "@id(\"t.r\") record R<T>{@id(\"t.bad\") bad:Missing,@id(\"t.good\") good:T,}",
        "0",
    );
    let program = crate::parse(&source, Path::new("extended-components.spx")).unwrap();
    let types = TypeTable::new(&program);
    let declaration = &program.types[0];
    let crate::ast::TypeDeclarationKind::Record { fields } = &declaration.kind else {
        panic!("record fixture")
    };
    let main = &program.functions[0];
    let mut receipt = Vec::new();
    check_declared_type(
        &program,
        &fields[0].ty,
        fields[0].span,
        &types,
        &HashSet::from(["T"]),
        &mut receipt,
    );
    let recovery = crate::source_verify::recovery::Recovery::signature(
        &program,
        main,
        &HashMap::new(),
        &types,
        &receipt,
    );
    assert_eq!(codes(&receipt), ["SPX-T220"]);
    assert_eq!(
        recovery.before_actual_declared_field_refusal("R", &fields[0], &program, &types, &receipt),
        None
    );
    assert_eq!(
        recovery.declared_field_refusal("R", &fields[0], &program, &types, &receipt),
        Some(0)
    );
    for field in 0..6 {
        let mut forged = receipt.clone();
        match field {
            0 => forged[0].code = "SPX-T205",
            1 => forged[0].severity = crate::diagnostic::Severity::Warning,
            2 => forged[0].message = "forged".into(),
            3 => forged[0].path = Some("forged.spx".to_owned()),
            4 => forged[0].span = None,
            5 => forged[0].help = Some("forged".to_owned()),
            _ => unreachable!(),
        }
        assert_eq!(
            recovery.declared_field_refusal("R", &fields[0], &program, &types, &forged),
            None
        );
    }
    let forged = vec![
        Diagnostic::error("SPX-T001", "unknown type `T`", fields[1].span).at_path(program.path()),
    ];
    assert_eq!(
        recovery.declared_field_refusal("R", &fields[1], &program, &types, &forged),
        None
    );
}

#[test]
fn complete_pre_extension_entry_call_and_record_receivers_are_byte_exact() {
    for (saved, archive, registration) in [
        (
            include_str!("../iterative/enter/enter_before_extended.rs"),
            include_str!(
                "../../../docs/evidence/extended-components-799/iterative__enter.rs.before"
            ),
            Some("#[cfg(test)]\nmod enter_before_components;\n"),
        ),
        (
            include_str!("../iterative/calls/calls_before_extended.rs"),
            include_str!(
                "../../../docs/evidence/extended-components-799/iterative__calls.rs.before"
            ),
            Some("#[cfg(test)]\nmod calls_before_components;\n"),
        ),
        (
            include_str!("../iterative/records/records_before_extended.rs"),
            include_str!(
                "../../../docs/evidence/extended-components-799/iterative__records.rs.before"
            ),
            None,
        ),
    ] {
        let mut restored = saved.replace("before_extended_", "").replace(
            "crate::source_verify::vec_field::signature(",
            "super::super::vec_field::signature(",
        );
        if let Some(registration) = registration {
            assert_eq!(
                restored
                    .matches("// Parent registers the complete prior component receiver.\n")
                    .count(),
                1
            );
            restored = restored.replace(
                "// Parent registers the complete prior component receiver.\n",
                registration,
            );
        }
        assert_eq!(restored, archive);
    }
}

#[test]
fn variant_and_update_components_keep_nominal_results_and_known_field_errors() {
    let source=self::source("@id(\"t.v\") variant V{@id(\"t.case\") Case{@id(\"t.bad\") bad:Missing,@id(\"t.known\") known:bool,},}","V::Case{bad:1,known:2}");
    let old = receive(&source, true);
    let current = receive(&source, false);
    assert_eq!(codes(&old.1), ["SPX-T001", "SPX-T215", "SPX-T215"]);
    assert_eq!(current.1, vec![old.1[0].clone(), old.1[2].clone()]);
    assert_eq!(current.0, old.0);
    assert!(current.0.is_some());
    let source = self::source(
        "@id(\"t.r\") record R{@id(\"t.bad\") bad:Missing,@id(\"t.known\") known:bool,}",
        "let r=R{bad:1,known:true}; r with {bad:2,known:3}",
    );
    let old = receive(&source, true);
    let current = receive(&source, false);
    assert_eq!(
        codes(&old.1),
        ["SPX-T001", "SPX-T215", "SPX-T215", "SPX-T215"]
    );
    assert_eq!(current.1, vec![old.1[0].clone(), old.1[3].clone()]);
    assert_eq!(current.0, old.0);
    assert!(current.0.is_some());
}

#[test]
fn inherited_raw_field_authentication_rejects_detached_or_changed_components() {
    let source=self::source("@id(\"t.a\") class A{@id(\"t.bad\") bad:Missing,} @id(\"t.b\") class B:A{@id(\"t.known\") known:i64,}","0");
    let program = crate::parse(&source, Path::new("extended-components.spx")).unwrap();
    let types = TypeTable::new(&program);
    let crate::ast::TypeDeclarationKind::Class { fields, .. } = &program.types[0].kind else {
        panic!("class fixture")
    };
    let mut receipt = Vec::new();
    check_declared_type(
        &program,
        &fields[0].ty,
        fields[0].span,
        &types,
        &HashSet::new(),
        &mut receipt,
    );
    let recovery = crate::source_verify::recovery::Recovery::signature(
        &program,
        &program.functions[0],
        &HashMap::new(),
        &types,
        &receipt,
    );
    let merged = &types.merged_class_fields["B"][0];
    assert_eq!(
        recovery.declared_field_refusal("B", merged, &program, &types, &receipt),
        Some(0)
    );
    let mut detached = merged.clone();
    detached.stable_id.push_str(".forged");
    assert_eq!(
        recovery.declared_field_refusal("B", &detached, &program, &types, &receipt),
        None
    );
    assert_eq!(
        recovery.declared_field_refusal("MissingContainer", merged, &program, &types, &receipt),
        None
    );
}

#[test]
fn complete_signature_carrier_projection_preserves_all_unrelated_shared_fields() {
    let mut projected =
        include_str!("../../../docs/evidence/extended-components-799/scope.rs.before").to_owned();
    let before = r####"pub(super) enum VerifierFunctionSignature<'a> {
    Borrowed(&'a Function),
    Specialized {
        params: Vec<Param>,
        return_type: Type,
        implicit_unique_ownership: bool,
    },
}
"####;
    let after = r####"pub(super) enum VerifierFunctionSignature<'a> {
    Borrowed(&'a Function),
    Specialized {
        params: Vec<Param>,
        return_type: Type,
        implicit_unique_ownership: bool,
    },
    SourceSpecialized {
        function: &'a Function,
        params: Vec<Param>,
        return_type: Type,
    },
}
"####;
    assert_eq!(projected.matches(before).count(), 1);
    projected = projected.replacen(before, after, 1);
    let before = r####"pub(super) fn verifier_signature_owned_capacity(
    signature: &VerifierFunctionSignature<'_>,
) -> usize {
    match signature {
        VerifierFunctionSignature::Borrowed(_) => 0,
        VerifierFunctionSignature::Specialized {
            params,
            return_type,
            ..
        } => params
            .capacity()
            .saturating_mul(std::mem::size_of::<Param>())
            .saturating_add(
                params
                    .iter()
                    .map(|param| {
                        param
                            .name
                            .capacity()
                            .saturating_add(ast_type_owned_capacity(&param.ty))
                    })
                    .sum::<usize>(),
            )
            .saturating_add(ast_type_owned_capacity(return_type)),
    }
}
"####;
    let after = r####"pub(super) fn verifier_signature_owned_capacity(
    signature: &VerifierFunctionSignature<'_>,
) -> usize {
    match signature {
        VerifierFunctionSignature::Borrowed(_) => 0,
        VerifierFunctionSignature::Specialized {
            params,
            return_type,
            ..
        }
        | VerifierFunctionSignature::SourceSpecialized {
            params,
            return_type,
            ..
        } => params
            .capacity()
            .saturating_mul(std::mem::size_of::<Param>())
            .saturating_add(
                params
                    .iter()
                    .map(|param| {
                        param
                            .name
                            .capacity()
                            .saturating_add(ast_type_owned_capacity(&param.ty))
                    })
                    .sum::<usize>(),
            )
            .saturating_add(ast_type_owned_capacity(return_type)),
    }
}
"####;
    assert_eq!(projected.matches(before).count(), 1);
    projected = projected.replacen(before, after, 1);
    let before = r####"impl VerifierFunctionSignature<'_> {
    pub(super) fn params(&self) -> &[Param] {
        match self {
            Self::Borrowed(function) => &function.params,
            Self::Specialized { params, .. } => params,
        }
    }

    pub(super) fn return_type(&self) -> &Type {
        match self {
            Self::Borrowed(function) => &function.return_type,
            Self::Specialized { return_type, .. } => return_type,
        }
    }

    pub(super) fn implicit_unique_ownership(&self) -> bool {
        match self {
            Self::Borrowed(_) => true,
            Self::Specialized {
                implicit_unique_ownership,
                ..
            } => *implicit_unique_ownership,
        }
    }
}
"####;
    let after = r####"impl<'p> VerifierFunctionSignature<'p> {
    pub(super) fn source_function(&self) -> Option<&'p Function> {
        match self {
            Self::Borrowed(function) | Self::SourceSpecialized { function, .. } => Some(*function),
            Self::Specialized { .. } => None,
        }
    }

    pub(super) fn params(&self) -> &[Param] {
        match self {
            Self::Borrowed(function) => &function.params,
            Self::Specialized { params, .. } | Self::SourceSpecialized { params, .. } => params,
        }
    }

    pub(super) fn return_type(&self) -> &Type {
        match self {
            Self::Borrowed(function) => &function.return_type,
            Self::Specialized { return_type, .. } | Self::SourceSpecialized { return_type, .. } => {
                return_type
            }
        }
    }

    pub(super) fn implicit_unique_ownership(&self) -> bool {
        match self {
            Self::Borrowed(_) | Self::SourceSpecialized { .. } => true,
            Self::Specialized {
                implicit_unique_ownership,
                ..
            } => *implicit_unique_ownership,
        }
    }
}
"####;
    assert_eq!(projected.matches(before).count(), 1);
    projected = projected.replacen(before, after, 1);
    projected = projected.replacen(
        "pub(super) struct VerifierScope",
        "#[cfg(test)]\nmod layout_original;\n\npub(super) struct VerifierScope",
        1,
    );
    let original_state = r####"pub(super) struct VariantMatchState<'a> {
    pub(super) refused_input: Option<usize>,
    pub(super) expression: &'a Expr,
    pub(super) arms: &'a [crate::ast::MatchArm],
    pub(super) parent_scope: usize,
    pub(super) index: usize,
    pub(super) outer_names: Vec<String>,
    pub(super) baseline: HashMap<String, Binding>,
    pub(super) arm_states: Vec<HashMap<String, Binding>>,
    pub(super) covered: HashSet<String>,
    pub(super) wildcard_seen: bool,
    pub(super) result: Option<CheckedValue>,
    pub(super) variant_name: Option<String>,
    pub(super) variant_arguments: Vec<Type>,
    pub(super) declared_cases: Option<&'a [VariantCaseDeclaration]>,
    pub(super) guard_pending: bool,
    pub(super) mode: MatchMode,
    pub(super) needs_drop: bool,
}

"####;
    let packed_state = r####"/// The known owned name and unknown-input provenance are mutually exclusive.
/// Retain the original String allocation on known inputs; unknown inputs carry
/// only the frozen diagnostic ordinal, never a fabricated name or checked type.
const _: () =
    assert!(std::mem::size_of::<VariantMatchName>() == std::mem::size_of::<Option<String>>());

pub(super) enum VariantMatchName {
    Known(String),
    Unknown(Option<usize>),
}

impl VariantMatchName {
    pub(super) fn as_ref(&self) -> Option<&String> {
        match self {
            Self::Known(name) => Some(name),
            Self::Unknown(_) => None,
        }
    }
    pub(super) fn as_deref(&self) -> Option<&str> {
        self.as_ref().map(String::as_str)
    }
    pub(super) fn refused_input(&self) -> Option<usize> {
        match self {
            Self::Known(_) => None,
            Self::Unknown(origin) => *origin,
        }
    }
}

impl From<Option<String>> for VariantMatchName {
    fn from(name: Option<String>) -> Self {
        match name {
            Some(name) => Self::Known(name),
            None => Self::Unknown(None),
        }
    }
}

pub(super) struct VariantMatchState<'a> {
    pub(super) expression: &'a Expr,
    pub(super) arms: &'a [crate::ast::MatchArm],
    pub(super) parent_scope: usize,
    pub(super) index: usize,
    pub(super) outer_names: Vec<String>,
    pub(super) baseline: HashMap<String, Binding>,
    pub(super) arm_states: Vec<HashMap<String, Binding>>,
    pub(super) covered: HashSet<String>,
    pub(super) wildcard_seen: bool,
    pub(super) result: Option<CheckedValue>,
    pub(super) variant_name: VariantMatchName,
    pub(super) variant_arguments: Vec<Type>,
    pub(super) declared_cases: Option<&'a [VariantCaseDeclaration]>,
    pub(super) guard_pending: bool,
    pub(super) mode: MatchMode,
    pub(super) needs_drop: bool,
}

"####;
    assert_eq!(projected.matches(original_state).count(), 1);
    projected = projected.replacen(original_state, packed_state, 1);
    assert_eq!(projected, include_str!("../scope.rs"));
}

#[test]
fn applied_parameter_refusal_is_not_a_missing_declaration_component() {
    let source = self::source("@id(\"t.f\") fn f<T>(a:T<i64>)->i64{0}", "0");
    let program = crate::parse(&source, Path::new("extended-components.spx")).unwrap();
    let types = TypeTable::new(&program);
    let function = &program.functions[0];
    let parameter = &function.params[0];
    let mut receipt = Vec::new();
    check_declared_type(
        &program,
        &parameter.ty,
        parameter.span,
        &types,
        &HashSet::from(["T"]),
        &mut receipt,
    );
    assert_eq!(
        receipt,
        vec![Diagnostic::error(
            "SPX-T220",
            "type parameter `T` cannot take type arguments",
            parameter.span
        )
        .at_path("extended-components.spx")]
    );
    let recovery = crate::source_verify::recovery::Recovery::signature(
        &program,
        &program.functions[1],
        &HashMap::new(),
        &types,
        &receipt,
    );
    assert_eq!(
        recovery.callee_component_refusal(function, Some(0), &program, &types, &receipt),
        None
    );
    assert_eq!(
        recovery.before_actual_callee_component_refusal(
            function,
            Some(0),
            &program,
            &types,
            &receipt
        ),
        None
    );
}

#[test]
fn declared_nominal_arity_refusal_keeps_complete_original_receipt_without_unknown_origin() {
    let source = self::source(
        "@id(\"t.r\") record Known{@id(\"t.x\") x:i64,} @id(\"t.f\") fn f<T>(a:Known<T>)->i64{0}",
        "0",
    );
    let program = crate::parse(&source, Path::new("extended-components.spx")).unwrap();
    let types = TypeTable::new(&program);
    let function = &program.functions[0];
    let mut receipt = Vec::new();
    check_declared_type(
        &program,
        &function.params[0].ty,
        function.params[0].span,
        &types,
        &HashSet::from(["T"]),
        &mut receipt,
    );
    assert_eq!(
        receipt,
        vec![
            Diagnostic::error(
                "SPX-T221",
                "type `Known` expects 0 type arguments, received 1",
                function.params[0].span,
            )
            .at_path("extended-components.spx"),
            Diagnostic::error(
                "SPX-T223",
                "generic copy type `Known` accepts only direct `i64` or `bool` arguments",
                function.params[0].span,
            )
            .at_path("extended-components.spx"),
        ]
    );
    let recovery = crate::source_verify::recovery::Recovery::signature(
        &program,
        &program.functions[1],
        &HashMap::new(),
        &types,
        &receipt,
    );
    assert_eq!(
        recovery.callee_component_refusal(function, Some(0), &program, &types, &receipt),
        None
    );
    assert_eq!(
        recovery.before_actual_callee_component_refusal(
            function,
            Some(0),
            &program,
            &types,
            &receipt
        ),
        None
    );
    assert_eq!(receive(&source, false), receive(&source, true));
}

#[test]
fn actual_failure_repair_keeps_complete_previous_component_receiver_and_projection() {
    // Begin corrective source inverse guard.
    let current = include_str!("extended_components.rs");
    let begin = "    // Begin corrective source inverse guard.\n";
    let end = "    // End corrective source inverse guard.\n";
    assert_eq!(current.matches(begin).count(), 1);
    assert_eq!(current.matches(end).count(), 1);
    let (prefix, guarded) = current.split_once(begin).unwrap();
    let (_, suffix) = guarded.split_once(end).unwrap();
    let mut restored = format!("{prefix}{suffix}");
    let inverse: Vec<[String; 2]> = serde_json::from_str(include_str!(
        "../../../docs/evidence/actual-refusal-components-799/extended-components-corrective-inverse.json"
    ))
    .unwrap();
    assert_eq!(inverse.len(), 7);
    for [before, after] in inverse {
        assert_eq!(restored.matches(after.as_str()).count(), 1);
        restored = restored.replacen(&after, &before, 1);
    }
    assert_eq!(
        restored,
        include_str!("../../../docs/evidence/actual-refusal-components-799/src__source_verify__recovery_tests__extended_components.rs.corrective-before")
    );
    // End corrective source inverse guard.
    let saved = include_str!("../recovery/callee_components/callee_components_before_actual.rs")
        .replace("use super::super::Recovery;", "use super::Recovery;")
        .replace("before_actual_", "");
    assert_eq!(saved, include_str!("../../../docs/evidence/actual-refusal-components-799/src__source_verify__recovery__callee_components.rs.before"));
    let calls = include_str!("../iterative/calls.rs");
    let boundary =
        "\nimpl IterativeVerifier<'_, '_, '_> {\n    // Availability and partial-move diagnostics";
    assert_eq!(calls.matches(boundary).count(), 1);
    assert_eq!(calls.split_once(boundary).unwrap().0, include_str!("../../../docs/evidence/actual-refusal-components-799/src__source_verify__iterative__calls.rs.before"));
    let recovery = include_str!("../recovery.rs");
    let begin = "\n    // Begin exact producer-origin parameter receiver.\n";
    let end = "    // End exact producer-origin parameter receiver.\n";
    assert_eq!(recovery.matches(begin).count(), 1);
    assert_eq!(recovery.matches(end).count(), 1);
    let (prefix, added) = recovery.split_once(begin).unwrap();
    let (_, suffix) = added.split_once(end).unwrap();
    let recovery = format!("{prefix}{suffix}").replace(
        "callee_components::unknown_component_receipt(diagnostic)\n                        && diagnostic.span == Some(parameter.span)",
        "diagnostic.code == \"SPX-T001\" && diagnostic.span == Some(parameter.span)",
    ).replace(
        ".filter(|refusal| callee_components::unknown_component_receipt(refusal))",
        ".filter(|refusal| refusal.code == \"SPX-T001\")",
    ).replace("\n#[cfg(test)]\nmod before_parameter_signature;\n", "").replace(
        ".any(callee_components::unknown_component_receipt)",
        ".any(|diagnostic| diagnostic.code == \"SPX-T001\")",
    );
    assert_eq!(recovery, include_str!("../../../docs/evidence/actual-refusal-components-799/src__source_verify__recovery.rs.before"));
    let entry = include_str!("../iterative/enter.rs").replace(
        "                if self.result_type.is_some() && self.recovery.reads_value(scope, name, expression)\n                {\n                    self.values.push(None);\n                    return Ok(());\n                }\n",
        "",
    ).replace(
        "                    self.publish_source_place(place, scope);",
        "                    self.values.push(Some(CheckedValue {\n                        ty: place.ty,\n                        mode: place.mode,\n                        native_unit: false,\n                    }));",
    );
    assert_eq!(entry, include_str!("../../../docs/evidence/actual-refusal-components-799/src__source_verify__iterative__enter.rs.before"));
}

#[test]
fn generic_current_parameter_refusal_authenticates_exact_receipt_and_keeps_real_sibling() {
    let source = self::source(
        "@id(\"t.f\") fn f<T>(b:Missing,c:bool)->i64{b+missing_sibling}",
        "0",
    );
    let program = crate::parse(&source, Path::new("extended-components.spx")).unwrap();
    let function = &program.functions[0];
    let types = TypeTable::new(&program);
    let mut receipt = Vec::new();
    check_declared_type(
        &program,
        &function.params[0].ty,
        function.params[0].span,
        &types,
        &HashSet::from(["T"]),
        &mut receipt,
    );
    assert_eq!(codes(&receipt), ["SPX-T220"]);
    let variables = function
        .params
        .iter()
        .map(|parameter| {
            (
                parameter.name.clone(),
                crate::source_verify::binding::Binding {
                    contract_local: false,
                    traversal_row: None,
                    ty: parameter.ty.clone(),
                    mode: parameter.mode,
                    availability: crate::source_verify::binding::Availability::Available,
                    moved_places: HashMap::new(),
                    definitely_partial: HashSet::new(),
                    native_unit_discard: false,
                    mutable: false,
                    active_loans: std::collections::BTreeSet::new(),
                    borrow_origin: None,
                },
            )
        })
        .collect();
    let mut old = crate::source_verify::recovery::Recovery::before_parameter_signature(
        &program, function, &variables, &types, &receipt,
    );
    let mut current = crate::source_verify::recovery::Recovery::signature(
        &program, function, &variables, &types, &receipt,
    );
    assert!(!old.reads_known_refused(0, "b"));
    assert!(current.reads_known_refused(0, "b"));
    assert!(!current.reads_known_refused(0, "c"));
    for field in 0..6 {
        let mut forged = receipt.clone();
        match field {
            0 => forged[0].code = "SPX-T001",
            1 => forged[0].severity = crate::diagnostic::Severity::Warning,
            2 => forged[0].message = "forged".into(),
            3 => forged[0].path = Some("forged.spx".to_owned()),
            4 => forged[0].span = None,
            5 => forged[0].help = Some("forged".to_owned()),
            _ => unreachable!(),
        }
        let mut current = crate::source_verify::recovery::Recovery::signature(
            &program, function, &variables, &types, &forged,
        );
        assert!(!current.reads_known_refused(0, "b"));
    }
    // The component-only old route already carries current signature/operator
    // recovery, so it cannot witness the dependent operator diagnostic.
    let component_old = receive_function(&source, true, "f");
    let old = receive_function_route(&source, true, "f", true);
    let current = receive_function(&source, false, "f");
    assert_eq!(component_old, current);
    assert_eq!(codes(&old.1), ["SPX-T220", "SPX-T202", "SPX-T208"]);
    assert_eq!(current.1, vec![old.1[0].clone(), old.1[1].clone()]);
    assert_eq!(current.0, None);
    let public = crate::source_verify::verify(&program)
        .into_iter()
        .filter(|diagnostic| diagnostic.severity.is_error())
        .collect::<Vec<_>>();
    assert_eq!(public, current.1);
}

#[test]
fn complete_previous_current_parameter_signature_receiver_has_only_named_namespace_adapter() {
    let saved = include_str!("../recovery/before_parameter_signature.rs");
    let prefix = "//! Complete previous current-parameter receiver, with namespace adapter only.\nuse super::{Diagnostic, Function, Recovery};\nimpl<'p> Recovery<'p> {\n";
    assert!(saved.starts_with(prefix) && saved.ends_with("\n}\n"));
    let saved = saved
        .strip_prefix(prefix)
        .unwrap()
        .strip_suffix("\n}\n")
        .unwrap()
        .replace("before_parameter_signature(", "signature(")
        .replace(
            "pub(in crate::source_verify) fn signature(",
            "pub(super) fn signature(",
        )
        .replace("crate::source_verify::binding::", "super::binding::")
        .replace("crate::source_verify::type_table::", "super::type_table::")
        .replace(
            "crate::source_verify::declared_type::",
            "super::declared_type::",
        )
        .replace(
            "super::callee_components::unknown_component_receipt",
            "callee_components::unknown_component_receipt",
        );
    let original = include_str!("../../../docs/evidence/actual-refusal-components-799/src__source_verify__recovery.rs.parameter-before");
    let start = original.find("    pub(super) fn signature(").unwrap();
    let end = original.find("\n    pub(super) fn clear(").unwrap();
    assert_eq!(saved, &original[start..end]);
}
