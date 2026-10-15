//! Complete pre-recovery frame/state layouts, independently compiled.
#![allow(
    dead_code,
    reason = "complete old carrier variants retain layout provenance"
)]
use super::{
    BinaryOp, Binding, CheckedValue, Expr, FieldDeclaration, Function, HashMap, HashSet, MatchMode,
    ScalarMatchState, SourceLoanId, Span, Statement, Type, TypeDeclaration, UnaryOp,
    VariantCaseDeclaration, VerifierCallTarget,
};

pub(super) enum OriginalVerifierFrame<'a> {
    ResumeLexicalReturn {
        expression: &'a Expr,
        operand: &'a Expr,
        scope: usize,
    },
    ResumeClosure {
        expression: &'a Expr,
        scope: usize,
    },
    Enter {
        expression: &'a Expr,
        scope: usize,
    },
    ResumeUnary {
        expression: &'a Expr,
        operand: &'a Expr,
        op: UnaryOp,
    },
    ResumeBinaryLeft {
        expression: &'a Expr,
        op: BinaryOp,
        right: &'a Expr,
        scope: usize,
    },
    ResumeBinaryRight {
        expression: &'a Expr,
        op: BinaryOp,
        left: &'a Expr,
        left_value: Option<CheckedValue>,
        scope: usize,
        evaluated_scope: usize,
        baseline_names: Vec<String>,
    },
    ResumeIfCondition {
        expression: &'a Expr,
        then_branch: &'a Expr,
        else_branch: &'a Expr,
        scope: usize,
    },
    ResumeIfThen {
        expression: &'a Expr,
        else_branch: &'a Expr,
        scope: usize,
        then_scope: usize,
        baseline_names: Vec<String>,
    },
    ResumeLiteralIf {
        expression: &'a Expr,
        then_branch: &'a Expr,
        else_branch: &'a Expr,
    },
    ResumeIfElse {
        expression: &'a Expr,
        then_branch: &'a Expr,
        else_branch: &'a Expr,
        scope: usize,
        else_scope: usize,
        baseline_names: Vec<String>,
        then_value: Option<CheckedValue>,
        then_bindings: HashMap<String, Binding>,
    },
    ResumeBlockStatement {
        expression: &'a Expr,
        statements: &'a [Statement],
        tail: &'a Expr,
        parent_scope: usize,
        block_scope: usize,
        index: usize,
        outer_names: Vec<String>,
        recovery_checkpoint: crate::source_verify::recovery::Checkpoint,
    },
    ResumeWhileCondition {
        condition: &'a Expr,
        block_scope: usize,
        baseline_bindings: HashMap<String, Binding>,
    },
    ResumeWhileBody {
        expression: &'a Expr,
        statements: &'a [Statement],
        tail: &'a Expr,
        parent_scope: usize,
        block_scope: usize,
        index: usize,
        outer_names: Vec<String>,
        statement_span: Span,
        baseline_names: Vec<String>,
        baseline_bindings: HashMap<String, Binding>,
    },
    ResumeForSource {
        expression: &'a Expr,
        statements: &'a [Statement],
        tail: &'a Expr,
        parent_scope: usize,
        block_scope: usize,
        index: usize,
        outer_names: Vec<String>,
        item: &'a str,
        item_span: Span,
        values: &'a Expr,
        body: &'a Expr,
    },
    ResumeForBody {
        expression: &'a Expr,
        statements: &'a [Statement],
        tail: &'a Expr,
        parent_scope: usize,
        block_scope: usize,
        index: usize,
        outer_names: Vec<String>,
        item: &'a str,
        item_inserted: bool,
        source: Option<&'a str>,
        baseline: Option<HashMap<String, Binding>>,
    },
    ResumeBlockTail {
        parent_scope: usize,
        block_scope: usize,
        outer_names: Vec<String>,
    },
    ResumeCallArgument {
        expression: &'a Expr,
        name: &'a str,
        args: &'a [Expr],
        scope: usize,
        index: usize,
        target: VerifierCallTarget<'a>,
        borrowed_bytes_loans: Vec<(String, SourceLoanId)>,
    },
    ResumeMethodReceiver {
        expression: &'a Expr,
        receiver: &'a Expr,
        method: &'a str,
        args: &'a [Expr],
        scope: usize,
    },
    ResumeMethodArgument {
        expression: &'a Expr,
        method: &'a Function,
        args: &'a [Expr],
        scope: usize,
        index: usize,
    },
    ResumeTry {
        expression: &'a Expr,
        operand: &'a Expr,
        scope: usize,
    },
    /// Resumable Effects v1 (issue #204). See `iterative::calls::frame_resume_yield`.
    ResumeYield {
        expression: &'a Expr,
        request: &'a Expr,
    },
    ResumeProject {
        expression: &'a Expr,
        base: &'a Expr,
        field: &'a str,
    },
    ResumeRecordField {
        expression: &'a Expr,
        type_name: &'a str,
        type_arguments: &'a [Type],
        fields: &'a [crate::ast::FieldInitializer],
        declared_fields: Option<&'a [FieldDeclaration]>,
        scope: usize,
        index: usize,
        supplied: HashSet<&'a str>,
    },
    PrepareRecordField {
        expression: &'a Expr,
        type_name: &'a str,
        type_arguments: &'a [Type],
        fields: &'a [crate::ast::FieldInitializer],
        declared_fields: Option<&'a [FieldDeclaration]>,
        scope: usize,
        index: usize,
        supplied: HashSet<&'a str>,
    },
    ResumeVariantField {
        expression: &'a Expr,
        type_name: &'a str,
        type_arguments: &'a [Type],
        case_name: &'a str,
        fields: &'a [crate::ast::FieldInitializer],
        declaration: Option<&'a TypeDeclaration>,
        case: Option<&'a VariantCaseDeclaration>,
        scope: usize,
        index: usize,
        supplied: HashSet<&'a str>,
    },
    PrepareVariantField {
        expression: &'a Expr,
        type_name: &'a str,
        type_arguments: &'a [Type],
        case_name: &'a str,
        fields: &'a [crate::ast::FieldInitializer],
        declaration: Option<&'a TypeDeclaration>,
        case: Option<&'a VariantCaseDeclaration>,
        scope: usize,
        index: usize,
        supplied: HashSet<&'a str>,
    },
    ResumeUpdateBase {
        expression: &'a Expr,
        base: &'a Expr,
        fields: &'a [crate::ast::FieldInitializer],
        scope: usize,
    },
    ResumeUpdateField {
        expression: &'a Expr,
        base_type: Type,
        fields: &'a [crate::ast::FieldInitializer],
        declared_fields: &'a [FieldDeclaration],
        scope: usize,
        index: usize,
        supplied: HashSet<&'a str>,
    },
    PrepareUpdateField {
        expression: &'a Expr,
        base_type: Type,
        fields: &'a [crate::ast::FieldInitializer],
        declared_fields: &'a [FieldDeclaration],
        scope: usize,
        index: usize,
        supplied: HashSet<&'a str>,
    },
    ResumeMatchScrutinee {
        expression: &'a Expr,
        scrutinee: &'a Expr,
        arms: &'a [crate::ast::MatchArm],
        scope: usize,
    },
    ResumeRecordMatchArm {
        owns_record_input: bool,
        arm: &'a crate::ast::MatchArm,
        parent_scope: usize,
        arm_scope: usize,
        outer_names: Vec<String>,
    },
    PrepareVariantMatchArm(OriginalVariantMatchState<'a>),
    ResumeVariantMatchArm {
        state: OriginalVariantMatchState<'a>,
        arm_scope: usize,
    },
    /// Refutable Match v1: one decision-chain state machine per scalar
    /// match, mirroring `OriginalVariantMatchState` without case bookkeeping.
    PrepareScalarMatchArm(ScalarMatchState<'a>),
    ResumeScalarMatchGuard {
        state: ScalarMatchState<'a>,
        arm_scope: usize,
    },
    ResumeScalarMatchArm {
        state: ScalarMatchState<'a>,
        arm_scope: usize,
    },
}

pub(super) struct OriginalVariantMatchState<'a> {
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

#[test]
fn original_and_packed_carriers_keep_exact_existing_physical_models() {
    assert_eq!(std::mem::size_of::<OriginalVerifierFrame<'static>>(), 320);
    assert_eq!(
        std::mem::size_of::<OriginalVariantMatchState<'static>>(),
        312
    );
    assert_eq!(
        std::mem::size_of::<super::VerifierFrame<'static>>(),
        std::mem::size_of::<OriginalVerifierFrame<'static>>()
    );
    assert_eq!(
        std::mem::size_of::<super::VariantMatchState<'static>>(),
        std::mem::size_of::<OriginalVariantMatchState<'static>>()
    );
    assert_eq!(
        std::mem::size_of::<super::VariantMatchName>(),
        std::mem::size_of::<Option<String>>()
    );
    assert_eq!(std::mem::size_of::<super::VariantMatchName>(), 24);
}

#[test]
fn known_name_retains_exact_owned_storage_and_unknown_retains_complete_ordinal() {
    let mut name = String::with_capacity(128);
    name.push_str("Choice");
    let pointer = name.as_ptr();
    let capacity = name.capacity();
    let known = super::VariantMatchName::Known(name);
    assert_eq!(known.as_deref(), Some("Choice"));
    assert_eq!(known.as_ref().unwrap().as_ptr(), pointer);
    assert_eq!(known.as_ref().unwrap().capacity(), capacity);
    assert_eq!(known.refused_input(), None);
    for origin in [None, Some(0), Some(1), Some(usize::MAX)] {
        let unknown = super::VariantMatchName::Unknown(origin);
        assert_eq!(unknown.as_deref(), None);
        assert_eq!(unknown.as_ref(), None);
        assert_eq!(unknown.refused_input(), origin);
    }
    for name in [None, Some("Choice".to_owned())] {
        let expected = name.clone();
        let adapted = super::VariantMatchName::from(name);
        assert_eq!(adapted.as_ref(), expected.as_ref());
        assert_eq!(adapted.refused_input(), None);
    }
}

#[test]
fn complete_original_frame_and_state_declarations_remain_byte_exact() {
    let source = include_str!("layout_original.rs");
    let start = source
        .find("pub(super) enum OriginalVerifierFrame")
        .unwrap();
    let definitions = source[start..]
        .split_once("#[test]")
        .unwrap()
        .0
        .replace("OriginalVerifierFrame", "VerifierFrame")
        .replace("OriginalVariantMatchState", "VariantMatchState")
        .replace(
            "crate::source_verify::recovery::Checkpoint",
            "super::recovery::Checkpoint",
        );
    let original = include_str!("../../../docs/evidence/conditional-refusal-799/scope.rs.before");
    let frame = &original[original.find("pub(super) enum VerifierFrame").unwrap()
        ..original
            .find("#[allow(dead_code)]\npub(super) struct ScalarMatchState")
            .unwrap()];
    let state = &original[original
        .find("pub(super) struct VariantMatchState")
        .unwrap()
        ..original.find("pub(super) enum VerifierCallTarget").unwrap()];
    assert_eq!(definitions, format!("{frame}{state}"));
}
