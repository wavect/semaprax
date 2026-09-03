//! Private complete source/cleanup/loan carriers for the sealed compiler cache.
//! Tags and field order belong to the enclosing versioned codec, never to a
//! public source or graph ABI. Decode alone is not semantic admission.

mod ast {
    use crate::ast::*;
    use crate::cache_codec::{codec_enum, codec_struct};

    codec_struct!(Span {
        start,
        end,
        line,
        column
    });
    codec_enum!(Type {
        0 => I64, 1 => I32, 2 => Char, 3 => U8, 4 => Usize,
        5 => ArrayU8(length), 6 => F32, 7 => F64, 8 => Bool,
        9 => String, 10 => Bytes, 11 => Str, 12 => SliceU8,
        13 => Named { name, arguments }
    });
    codec_enum!(ParamMode { 0 => Value, 1 => Own, 2 => Borrow, 3 => Shared });
    codec_enum!(MatchMode { 0 => Value, 1 => Own, 2 => Borrow });
    codec_struct!(Program {
        path,
        module,
        module_uses,
        permits,
        types,
        interfaces,
        protocols,
        implementations,
        functions
    });
    codec_enum!(ModuleUseKind { 0 => Function, 1 => Type });
    codec_struct!(ModuleUse {
        kind,
        persistent_id,
        target_module,
        alias,
        span
    });
    codec_struct!(TypeDeclaration {
        stable_id,
        explicit_id,
        name,
        name_span,
        type_parameters,
        kind,
        extends,
        span
    });
    codec_struct!(TypeParameterDeclaration { name, span });
    codec_enum!(TypeDeclarationKind {
        0 => Resource { lifecycles }, 1 => Record { fields },
        2 => Variant { cases }, 3 => Class { fields, methods }
    });
    codec_struct!(VariantCaseDeclaration {
        stable_id,
        explicit_id,
        name,
        name_span,
        fields,
        span
    });
    codec_struct!(ResourceLifecycleDeclaration {
        stable_id,
        kind,
        span
    });
    codec_enum!(ResourceLifecycleKind { 0 => Trivial, 1 => Imported { import_key } });
    codec_struct!(InterfaceDeclaration {
        stable_id,
        explicit_id,
        name,
        name_span,
        permits,
        imports,
        span
    });
    codec_struct!(ProtocolDeclaration {
        stable_id,
        explicit_id,
        name,
        name_span,
        methods,
        span
    });
    codec_struct!(ProtocolMethod {
        stable_id,
        explicit_id,
        name,
        name_span,
        params,
        return_type,
        span
    });
    codec_struct!(ProtocolImplementation {
        stable_id,
        explicit_id,
        protocol_id,
        receiver_id,
        members,
        span
    });
    codec_struct!(ProtocolImplementationMember {
        method_id,
        function_id,
        span
    });
    codec_struct!(ImportDeclaration {
        stable_id,
        explicit_id,
        name,
        name_span,
        native_rust,
        params,
        result,
        effects,
        failure,
        consumes,
        consumes_span,
        span
    });
    codec_enum!(ImportResult { 0 => Unit, 1 => I64, 2 => Bool });
    codec_enum!(ImportFailure { 0 => Infallible, 1 => Status { domain_id } });
    codec_struct!(FieldDeclaration {
        stable_id,
        explicit_id,
        name,
        name_span,
        ty,
        span
    });
    codec_struct!(Function {
        stable_id,
        explicit_id,
        name,
        name_span,
        type_parameters,
        params,
        return_type,
        effects,
        requires,
        ensures,
        body,
        span
    });
    codec_struct!(Param {
        name,
        mode,
        ty,
        span
    });
    codec_struct!(Expr { kind, span });
    codec_enum!(ExprKind {
        0 => Int(value), 1 => Int32(value), 2 => Char(value), 3 => Uint8(value),
        4 => Usize(value), 5 => ArrayU8(values), 6 => RepeatArrayU8 { value, count },
        7 => Float32(bits), 8 => Float64(bits), 9 => Bool(value),
        10 => String(value), 11 => Var(name),
        12 => Call { name, type_arguments, args },
        13 => MethodCall { receiver, method, method_span, type_arguments, args },
        14 => SuperMethod { method, method_span, args },
        15 => Unary { op, value }, 16 => Binary { op, left, right },
        17 => Block { statements, tail },
        18 => If { condition, then_branch, else_branch },
        19 => ConstructRecord { type_name, type_span, type_arguments, fields },
        20 => ConstructVariant { type_name, type_span, type_arguments, case_name, case_span, fields },
        21 => Match { mode, scrutinee, arms }, 22 => Try { operand },
        23 => UpdateRecord { base, fields }, 24 => Project { base, field, field_span }
    });
    codec_struct!(MatchArm {
        pattern,
        guard,
        value,
        span
    });
    codec_enum!(MatchPattern {
        0 => Variant { type_name, type_span, case_name, case_span, fields, span },
        1 => Record { type_name, type_span, fields, span }, 2 => Wildcard { span },
        3 => Literal { value, span }, 4 => Or { alternatives, span }, 5 => Binding { name, span }
    });
    codec_enum!(PatternLiteral { 0 => Int(value), 1 => Int32(value), 2 => Uint8(value), 3 => Usize(value), 4 => Char(value), 5 => Bool(value) });
    codec_struct!(RecordMatchPatternField {
        name,
        name_span,
        pattern,
        span
    });
    codec_enum!(RecordMatchFieldPattern {
        0 => Binding { name, span }, 1 => Wildcard { span },
        2 => Record { type_name, type_span, fields, span }
    });
    codec_struct!(MatchPatternField {
        name,
        name_span,
        binding,
        binding_span,
        span
    });
    codec_struct!(FieldInitializer {
        name,
        name_span,
        value,
        span
    });
    codec_struct!(FieldTarget { name, span });
    codec_enum!(Statement {
        0 => Let { name, name_span, mutable, declared, value, span },
        1 => Assign { name, name_span, field, value, span },
        2 => Unsafe { audit, audit_span, body, span },
        3 => While { condition, body, span }
    });
    codec_enum!(UnaryOp { 0 => Neg, 1 => Not });
    codec_enum!(BinaryOp {
        0 => Add, 1 => Sub, 2 => Mul, 3 => Div, 4 => Rem, 5 => Eq, 6 => Ne,
        7 => Lt, 8 => Le, 9 => Gt, 10 => Ge, 11 => And, 12 => Or
    });
}

mod inventory {
    use crate::cache_codec::{codec_enum, codec_struct, codec_tuple};
    use crate::cleanup::*;

    codec_tuple!(CleanupStorageId(0));
    codec_tuple!(LivenessFlagId(0));
    codec_struct!(CleanupInventory {
        schema,
        entry_state,
        slots,
        flags
    });
    codec_struct!(CleanupEntryState {
        live_owned_parameters,
        conditional_owned_parameters
    });
    codec_struct!(ConditionalVariantEntry {
        storage,
        variant,
        cases
    });
    codec_struct!(ConditionalVariantCase { case, live_flags });
    codec_struct!(CleanupStorageSlot {
        id,
        discovery_index,
        origin,
        ty,
        shape
    });
    codec_enum!(CleanupStorageOrigin {
        0 => Parameter { value, parameter_index }, 1 => Binding { value },
        2 => Temporary { expression }, 3 => ProvisionalResult { value }
    });
    codec_enum!(FieldLivenessShape {
        0 => NoDrop, 1 => Leaf { flag, lifecycle },
        2 => Record { declaration, fields }, 3 => Variant { declaration, cases }
    });
    codec_struct!(VariantCaseLiveness {
        case,
        case_index,
        fields
    });
    codec_struct!(FieldLiveness {
        field,
        field_index,
        shape
    });
    codec_struct!(CleanupFlag {
        id,
        place,
        lifecycle
    });
    codec_struct!(CleanupPlace {
        storage,
        projections
    });
}

mod plan {
    use crate::cache_codec::{codec_enum, codec_struct, codec_tuple};
    use crate::cleanup_plan::*;

    codec_tuple!(BlockId(0));
    codec_tuple!(EdgeId(0));
    codec_tuple!(CleanupRegionId(0));
    codec_tuple!(ExitTargetId(0));
    codec_tuple!(CleanupSlotId(0));
    codec_enum!(StatusLane { 0 => OperationFailure, 1 => ContractFalse });
    codec_struct!(StatusSourceId { expression, lane });
    codec_enum!(StorageId {
        0 => Value(value), 1 => Temporary(expression),
        2 => CallArgument { call, parameter_index, value_expression }, 3 => ProvisionalResult
    });
    codec_struct!(CleanupPlace {
        storage,
        projections
    });
    codec_struct!(CleanupSlot {
        id,
        storage,
        ty,
        storage_index,
        field_liveness_shape
    });
    codec_struct!(CleanupEntryState {
        live_owned_parameters,
        conditional_owned_parameters
    });
    codec_struct!(ConditionalVariantEntry {
        storage,
        variant,
        cases
    });
    codec_struct!(ConditionalVariantCase { case, live_places });
    codec_struct!(CallArgumentTransfer {
        parameter_index,
        source
    });
    codec_enum!(CleanupTransition {
        0 => Initialize { at, destination }, 1 => InitializeVariant { at, destination, variant },
        2 => Transfer { at, source, destination }, 3 => TransferVariant { at, source, destination, variant },
        4 => AuthenticateVariantCase { at, source, variant, case },
        5 => CallCommit { call, arguments }, 6 => SelectFailure { source },
        7 => StageCopyResult { source }
    });
    codec_enum!(StagedCopyResultSource {
        0 => Body { expression, instance },
        1 => TryResidual { expression, operand, source_instance, target_instance, result, ok_case, ok_field, err_case, err_field },
        2 => TryOptionNone { expression, operand, source_instance, target_instance, option, some_case, some_field, none_case }
    });
    codec_enum!(CheckedOperation { 0 => Neg, 1 => Add, 2 => Sub, 3 => Mul, 4 => Div, 5 => Rem });
    codec_enum!(StatusCase {
        0 => AddOverflow, 1 => SubOverflow, 2 => MulOverflow, 3 => DivisionByZero,
        4 => DivisionOverflow, 5 => RemainderByZero, 6 => RemainderOverflow, 7 => NegationOverflow
    });
    codec_enum!(ContractPhase { 0 => Requires, 1 => Ensures });
    codec_enum!(StatusProducer {
        0 => PropagatedCall { callee }, 1 => CheckedArithmetic { operation, normalized_cases },
        2 => ContractFalse { phase, ordinal }
    });
    codec_struct!(StatusSource { id, producer });
    codec_enum!(CleanupTerminator { 0 => Goto(edge), 1 => Branch(edges), 2 => Exit(target) });
    codec_struct!(CleanupBlock {
        id,
        region,
        transitions,
        terminator
    });
    codec_enum!(EdgeCondition {
        0 => Always, 1 => BooleanResult(expression, value),
        2 => VariantCase { scrutinee, case, matches },
        3 => ArmSelected { scrutinee, arm, selected },
        4 => StatusZero(source), 5 => StatusNonzero(source)
    });
    codec_struct!(CleanupEdge {
        id,
        from,
        to,
        condition
    });
    codec_struct!(CleanupRegion {
        id,
        parent,
        slots,
        normal_scope_end
    });
    codec_struct!(FinalizeAction {
        source,
        lifecycle_id,
        guard_flag,
        active_case
    });
    codec_struct!(VariantCaseGuard {
        storage,
        variant,
        case
    });
    codec_enum!(ExitContinuation {
        0 => Continue(edge), 1 => CommitResult { source }, 2 => ReturnFailure { source }, 3 => ReturnUnit
    });
    codec_enum!(CleanupResultSource { 0 => Scalar { expression }, 1 => Owned { storage } });
    codec_struct!(ExitTarget {
        id,
        from,
        leaves_regions,
        finalize_in_order,
        continuation
    });
    codec_struct!(CleanupPlan {
        schema,
        entry,
        entry_state,
        slots,
        status_sources,
        blocks,
        edges,
        regions,
        exits
    });
}

mod loan {
    use crate::cache_codec::{codec_enum, codec_struct, codec_tuple};
    use crate::loan_plan::*;

    codec_tuple!(LoanId(0));
    codec_enum!(LoanPointPhase { 0 => Before, 1 => After });
    codec_struct!(LoanProgramPoint { expression, phase });
    codec_enum!(LoanCause { 0 => SliceView, 1 => BorrowedCall { argument }, 2 => MatchBorrow { arm }, 3 => StrView });
    codec_struct!(Loan {
        id,
        site,
        origin,
        parent,
        start,
        ends,
        end_edges,
        cause
    });
    codec_struct!(LoanEndpoint {
        point,
        live_before,
        starts,
        kills,
        live_after
    });
    codec_struct!(LoanEdge { from, to, live });
    codec_struct!(LoanPlan {
        schema,
        loans,
        endpoints,
        edges
    });
}

#[cfg(test)]
mod tests {
    use crate::cache_codec::{decode, encode, Codec};
    use std::fmt::Debug;

    fn roundtrip<T: Codec + Eq + Debug>(value: &T) {
        let bytes = encode(value).unwrap();
        let restored: T = decode(&bytes).unwrap();
        assert_eq!(&restored, value);
        assert_eq!(encode(&restored).unwrap(), bytes);
        assert!(decode::<T>(&bytes[..bytes.len() - 1]).is_err());
    }

    #[test]
    fn source_program_roundtrip_keeps_spans_contracts_types_and_protocol_bindings() {
        let mut program = crate::parse(
            include_str!("../../examples/frame-payload-project/src/frame.spx"),
            "src/frame.spx",
        )
        .unwrap();
        let span = crate::ast::Span {
            start: 1,
            end: 9,
            line: 2,
            column: 3,
        };
        // Codec coverage is structural, not a source-conformance certificate.
        program.protocols.push(crate::ast::ProtocolDeclaration {
            stable_id: "codec.protocol".into(),
            explicit_id: true,
            name: "Read".into(),
            name_span: span,
            span,
            methods: vec![crate::ast::ProtocolMethod {
                stable_id: "codec.protocol.read".into(),
                explicit_id: true,
                name: "read".into(),
                name_span: span,
                span,
                params: vec![crate::ast::Param {
                    name: "self".into(),
                    mode: crate::ast::ParamMode::Borrow,
                    ty: crate::ast::Type::Named {
                        name: "Self".into(),
                        arguments: vec![],
                    },
                    span,
                }],
                return_type: crate::ast::Type::I64,
            }],
        });
        program
            .implementations
            .push(crate::ast::ProtocolImplementation {
                stable_id: "codec.implementation".into(),
                explicit_id: true,
                protocol_id: "codec.protocol".into(),
                receiver_id: "codec.receiver".into(),
                span,
                members: vec![crate::ast::ProtocolImplementationMember {
                    method_id: "codec.protocol.read".into(),
                    function_id: "codec.read".into(),
                    span,
                }],
            });
        roundtrip(&program);
        // Floating literals are stored as exact bits; decoding must not convert
        // NaNs or negative zero through host floating-point normalization.
        roundtrip(&crate::ast::ExprKind::Float32(0xffc0_0042));
        roundtrip(&crate::ast::ExprKind::Float64(0x8000_0000_0000_0000));
    }

    #[test]
    fn nonempty_ownership_cleanup_and_loan_carriers_roundtrip_in_canonical_order() {
        let source = r#"
module codec.loans;
@id("loan.consume") fn consume(value: own Bytes) -> i64 { 7 }
@id("loan.main") fn main() -> i64 {
    let source = [7u8, 8u8, 9u8];
    let owned = bytes_copy(array_as_slice(source));
    let parent = bytes_as_slice(owned);
    let child = byte_range(parent, 1usize, byte_len(parent));
    let sibling = bytes_as_slice(owned);
    let observed = if byte_len(child) + byte_len(sibling) > 0usize { 1 } else { 0 };
    consume(owned) + observed
}
"#;
        let program = crate::parse(source, "codec-loans.spx").unwrap();
        let resolved = crate::hir::resolve(&program).unwrap();
        let main = resolved
            .functions
            .iter()
            .find(|function| function.id.as_str() == "loan.main")
            .unwrap();
        assert!(!main.cleanup.slots.is_empty());
        assert!(!main.cleanup_plan.blocks.is_empty());
        assert!(main.loan_plan.loans.len() >= 4);
        roundtrip(&main.cleanup);
        roundtrip(&main.cleanup_plan);
        roundtrip(&main.loan_plan);
        roundtrip(&resolved);
    }

    #[test]
    fn unknown_carrier_tags_and_unresolved_static_tokens_reject() {
        assert!(decode::<crate::ast::ExprKind>(&u16::MAX.to_le_bytes()).is_err());
        assert!(decode::<crate::cleanup_plan::CleanupTransition>(&u16::MAX.to_le_bytes()).is_err());
        assert!(decode::<crate::loan_plan::LoanCause>(&u16::MAX.to_le_bytes()).is_err());
        let unresolved = crate::loan_plan::LoanPlan::unresolved();
        assert!(encode(&unresolved).is_err());
        // An owned string can carry these bytes, but the static-token decoder
        // must not intern or leak them into a trusted schema lifetime.
        let bytes = encode(&"unresolved".to_owned()).unwrap();
        assert!(decode::<crate::loan_plan::LoanPlan>(&bytes).is_err());
    }

    #[test]
    fn hostile_framing_collection_order_and_depth_are_bounded_before_adoption() {
        use std::collections::{BTreeMap, BTreeSet};
        assert!(decode::<bool>(&[2]).is_err());
        let mut trailing = encode(&crate::loan_plan::LoanCause::SliceView).unwrap();
        trailing.push(0);
        assert!(decode::<crate::loan_plan::LoanCause>(&trailing).is_err());
        for keys in [[2u8, 1u8], [1u8, 1u8]] {
            let mut map = 2u32.to_le_bytes().to_vec();
            map.extend_from_slice(&[keys[0], 0, keys[1], 0]);
            assert!(decode::<BTreeMap<u8, u8>>(&map).is_err());
            let mut set = 2u32.to_le_bytes().to_vec();
            set.extend_from_slice(&keys);
            assert!(decode::<BTreeSet<u8>>(&set).is_err());
        }
        let oversized = ((crate::cache_codec::MAX_NODES + 1) as u32).to_le_bytes();
        assert!(decode::<Vec<u8>>(&oversized).is_err());
        // Below the node limit, but above the allocation accounting limit;
        // rejection must precede reading or allocating any declared entries.
        let allocation = ((crate::cache_codec::MAX_NODES - 2) as u32).to_le_bytes();
        type WideKey = ((u64, u64), (u64, u64));
        let errors = decode::<BTreeMap<WideKey, WideKey>>(&allocation).unwrap_err();
        assert!(errors.iter().any(|error| error.code == "SPX-G305"));
        assert!(errors[0].message.contains("charged allocation limit"));
        assert!(decode::<String>(&u32::MAX.to_le_bytes()).is_err());
        let unknown = encode(&"not-a-compiler-token".to_owned()).unwrap();
        assert!(decode::<&'static str>(&unknown).is_err());
        let mut nested = Vec::new();
        for _ in 0..=crate::cache_codec::MAX_DEPTH {
            nested.extend_from_slice(&13u16.to_le_bytes()); // Type::Named
            nested.extend_from_slice(&0u32.to_le_bytes()); // empty name
            nested.extend_from_slice(&1u32.to_le_bytes()); // one type argument
        }
        nested.extend_from_slice(&0u16.to_le_bytes()); // Type::I64
        assert!(decode::<crate::ast::Type>(&nested).is_err());
    }
}
