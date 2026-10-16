//! Complete private checked-module wire carriers. Access stays behind the
//! authenticated cache owner; this module grants no source or backend authority.
use super::*;
use crate::cache_codec::{codec_enum, codec_struct, codec_tuple, Codec, Decoder, Encoder};

// Transparent compatibility carrier: no new nested node or depth level.
// Its actual backing is independently counted by decode and the live builder.
impl Codec for ResolvedResidualType {
    fn encode(&self, encoder: &mut Encoder) -> crate::cache_codec::Result<()> {
        (**self).encode(encoder)
    }
    fn decode(decoder: &mut Decoder<'_>) -> crate::cache_codec::Result<Self> {
        decoder.allocate(std::mem::size_of::<ResolvedType>())?;
        let ty = ResolvedType::decode(decoder)?;
        Self::try_new(ty).ok_or_else(|| {
            crate::cache_codec::capacity("cache codec residual type backing was refused")
        })
    }
}

// Same wire as the former owned-String tuple: one nested node around text.
// Every identity is valid, so encoding has no refusal case of its own.
impl Codec for DeclarationId {
    fn encode(&self, encoder: &mut Encoder) -> crate::cache_codec::Result<()> {
        encoder.nested(|encoder| encoder.text(self.as_str()))
    }

    /// Decodes borrowed wire text straight into the shared backing; no
    /// temporary `String` is made. The backing is charged to two independent
    /// ledgers: the decoder's own allocation cap, and the decode caller's
    /// active output budget, passed explicitly as the construction payer. A
    /// refusal by either returns an error and constructs nothing.
    fn decode(decoder: &mut Decoder<'_>) -> crate::cache_codec::Result<Self> {
        decoder.nested(|decoder| {
            let text = decoder.text()?;
            let refused = || {
                crate::cache_codec::capacity(
                    "cache codec declaration identity backing allocation was refused",
                )
            };
            let Some(backing) = DeclarationId::shared_backing_bytes(text.len()) else {
                return Err(refused());
            };
            decoder.allocate(backing)?;
            DeclarationId::paid_borrowed(text, |bytes| {
                if crate::bounded_output::reserve_active_required(bytes) {
                    Ok(())
                } else {
                    Err(refused())
                }
            })
        })
    }
}
codec_tuple!(FunctionInstanceId(0));
impl Codec for ValueId {
    fn encode(&self, encoder: &mut Encoder) -> crate::cache_codec::Result<()> {
        self.as_str().to_owned().encode(encoder)
    }

    fn decode(decoder: &mut Decoder<'_>) -> crate::cache_codec::Result<Self> {
        String::decode(decoder).map(Self::new)
    }
}
impl Codec for ExpressionId {
    fn encode(&self, encoder: &mut Encoder) -> crate::cache_codec::Result<()> {
        encoder.nested(|encoder| match self.cached_text() {
            Some(text) => text.encode(encoder),
            None => Err(crate::cache_codec::capacity(
                "cache codec expression identity backing allocation was refused",
            )),
        })
    }

    fn decode(decoder: &mut Decoder<'_>) -> crate::cache_codec::Result<Self> {
        decoder.nested(|decoder| {
            let value = String::decode(decoder)?;
            decoder.allocate(Self::SHARED_ALLOCATION_CARRIER_BYTES)?;
            let identity = Self::from_owned(value);
            if identity.cached_text().is_none() {
                return Err(crate::cache_codec::capacity(
                    "cache codec expression identity backing allocation was refused",
                ));
            }
            Ok(identity)
        })
    }
}
codec_enum!(FunctionExecutionId {0=>Monomorphic(value),1=>Generic(value)});
codec_enum!(ControlTarget {0=>Function(value),1=>Loop{owner,site}});
codec_enum!(SourceForKind {0=>Copy,1=>BorrowRecord,2=>OwnedIterator,3=>BorrowBytes});
codec_enum!(LoopLatchKind {0=>Condition,1=>SourceForStep{kind,step},2=>OwnedIteratorStep{next,assignment,step,remainder}});
codec_enum!(DeclarationKind {0=>Resource,1=>ResourceDrop,2=>Record,3=>Field,4=>Class,5=>Variant,6=>VariantCase,7=>CaseField,8=>Interface,9=>Import,10=>Function});
codec_enum!(IdentityOrigin {0=>Explicit,1=>Automatic,2=>CompilerOwned});
codec_struct!(Declaration {
    id,
    name,
    kind,
    identity_origin,
    owner
});
codec_enum!(ByteSliceRootKind {0=>FunctionParameter,1=>OwnedBytes,2=>FixedArray,3=>BorrowedStr,4=>CommandArguments,5=>StdinStreamReader,6=>OwnedString,7=>OwnedVectorField,8=>SourceExprStatic,9=>OwnedVectorStringSlot});
codec_enum!(ByteSliceExtent {0=>Constant(value),1=>ParameterLength,2=>ValueLength});
codec_struct!(ByteSliceRangeStep {
    source,
    producer,
    start,
    end
});
codec_struct!(VectorFieldProvenance {
    element,
    field,
    index
});
codec_struct!(ByteSliceProvenance {
    vector_field,
    root,
    projections,
    projected_type,
    root_kind,
    root_length,
    offset,
    length,
    producer,
    ranges
});
codec_enum!(ResolvedType {0=>Unit,1=>I64,2=>I32,3=>Char,4=>U8,5=>Usize,6=>ArrayU8(length),7=>F32,8=>F64,9=>Bool,10=>String,11=>Bytes,12=>Str,13=>SliceU8,14=>TypeParameter{owner,index},15=>Nominal{declaration,arguments},16=>Function{parameters,result},17=>OnceFunction,18=>OnceFunctionI64,19=>OnceFunctionI64Pair,20=>MutFunctionI64,21=>StringMap,22=>Never});
codec_struct!(TypeFacts {
    copy,
    contains_resource,
    sized,
    needs_drop,
    layout_key
});
codec_enum!(OwnershipMode {0=>Value,1=>Own,2=>Borrow,3=>Shared});
codec_enum!(ResolvedMatchMode {0=>Value,1=>Own,2=>Borrow});
codec_enum!(ResolvedAgentTypeRoleKind {0=>Task,1=>State,2=>Observation,3=>Proposal,4=>Outcome,5=>Result});
codec_enum!(ResolvedAgentOperationRoleKind {0=>Initialize,1=>Observe,2=>Propose,3=>Authorize,4=>Execute,5=>Reduce});
codec_enum!(ResolvedAgentOperationKind {0=>Deterministic,1=>Model,2=>Effect});
codec_struct!(ResolvedAgentTypeRole { role, stable_id });
codec_struct!(ResolvedAgentOperationRole {
    role,
    kind,
    stable_id,
    embedded
});
codec_struct!(ResolvedAgentModelWaitBinding { helper_id });
codec_struct!(AgentExecutionSourceAssociation {
    module,
    operations,
    model_wait,
    helper_top_level
});
codec_struct!(ResolvedAgentDeclaration {
    stable_id,
    name,
    types,
    operations,
    runtime_v1_json,
    model_wait,
    source_association
});

// Every lookup map is part of the complete HIR value, including native import,
// inheritance, generic identity, and byte-slice provenance sidecars.
impl Codec for DeclarationIndex {
    fn encode(&self, encoder: &mut Encoder) -> crate::cache_codec::Result<()> {
        encoder.nested(|encoder| {
            self.declarations.encode(encoder)?;
            self.types_by_name.encode(encoder)?;
            self.functions_by_name.encode(encoder)?;
            self.fields_by_owner_name.encode(encoder)?;
            self.record_fields.encode(encoder)?;
            self.cases_by_owner_name.encode(encoder)?;
            self.variant_cases.encode(encoder)?;
            self.case_fields.encode(encoder)?;
            self.type_parameters.encode(encoder)?;
            self.imports_by_key.encode(encoder)?;
            self.native_rust_imports_by_name.encode(encoder)?;
            self.type_facts_by_id.encode(encoder)?;
            self.class_parents.encode(encoder)?;
            self.byte_slice_roots.encode(encoder)?;
            Ok(())
        })
    }
    fn decode(decoder: &mut Decoder<'_>) -> crate::cache_codec::Result<Self> {
        decoder.nested(|decoder| {
            Ok(Self {
                nominal_drop_workspace: Default::default(),
                declarations: Codec::decode(decoder)?,
                types_by_name: Codec::decode(decoder)?,
                functions_by_name: Codec::decode(decoder)?,
                fields_by_owner_name: Codec::decode(decoder)?,
                record_fields: Codec::decode(decoder)?,
                cases_by_owner_name: Codec::decode(decoder)?,
                variant_cases: Codec::decode(decoder)?,
                case_fields: Codec::decode(decoder)?,
                type_parameters: Codec::decode(decoder)?,
                imports_by_key: Codec::decode(decoder)?,
                native_rust_imports_by_name: Codec::decode(decoder)?,
                type_facts_by_id: Codec::decode(decoder)?,
                class_parents: Codec::decode(decoder)?,
                byte_slice_roots: Codec::decode(decoder)?,
            })
        })
    }
}

codec_struct!(ResolvedProgram {
    module,
    permits,
    agents,
    entrypoint,
    declarations,
    types,
    interfaces,
    function_templates,
    functions,
    function_instances
});
codec_struct!(ResolvedNativeRustImportCall {
    expression,
    import,
    args,
    result
});
codec_enum!(ResolvedHostCommandOperation {0=>ArgsLen,1=>ArgUtf8,2=>StdinRead,3=>StderrWrite,4=>StdoutAppend,5=>StderrAppend,6=>NetConnect,7=>NetSend,8=>NetRecv,9=>NetStreamStdout,10=>NetWait,11=>NetClose,12=>NetTlsConnect,13=>NetListen,14=>NetAccept,15=>NetCloseListener,16=>NetTlsAccept,17=>HttpsGet,18=>FileRead,19=>FileWriteNew,20=>FileStat,21=>FileList,22=>FileCreateDir,23=>FileRemove,24=>FileWriteAtomic,25=>EnvLen,26=>EnvNameUtf8,27=>EnvValueUtf8,28=>ProcessRun,29=>FileWriteAtomicChecked,30=>HttpsPost,31=>StdinStreamOpen,32=>StdinStreamNext});
codec_struct!(ResolvedHostCommandCall {
    expression,
    operation,
    args
});
codec_struct!(ResolvedTypeDeclaration {
    id,
    name,
    type_parameters,
    kind,
    span
});
codec_struct!(ResolvedTypeParameterDeclaration { name, index, span });
codec_enum!(ResolvedTypeDeclarationKind {0=>Resource{drop},1=>Record{fields},2=>Class{fields,methods},3=>Variant{cases}});
codec_struct!(ResolvedVariantCaseDeclaration {
    id,
    name,
    index,
    fields,
    span
});
codec_struct!(ResolvedResourceDrop { id, kind });
codec_enum!(ResolvedResourceDropKind {0=>Trivial,1=>Imported{import,import_key}});
codec_struct!(ResolvedInterface {
    id,
    name,
    permits,
    imports,
    span
});
codec_struct!(ResolvedImport {
    id,
    name,
    interface,
    import_key,
    native_rust,
    index_selected,
    selected_index_digest,
    selected_receiver,
    rust_path,
    parameters,
    result,
    effects,
    required_authority,
    failure,
    span
});
codec_struct!(ResolvedImportParameter {
    name,
    ty,
    ownership,
    consumes_on_failure
});
codec_struct!(ResolvedImportResult {
    kind,
    ownership,
    producer,
    out_slot_initialization,
    ownership_transfer
});
codec_enum!(ResolvedImportResultKind {0=>Unit,1=>I64,2=>Bool,3=>ResultI64I64,4=>OwnedResource{resource},5=>OwnedString,6=>OwnedOptionString,7=>OwnedResultStringI64,8=>OwnedResultStringOptionI64,9=>OwnedResultResourceI64{resource},10=>BorrowedStr{resource}});
codec_enum!(ResolvedImportFailure {0=>Infallible,1=>Status{domain_id,normalization}});
codec_struct!(ResolvedFieldDeclaration {
    id,
    name,
    index,
    ty,
    span
});
codec_struct!(ResolvedYieldsClause {
    request_type,
    response_type,
    span
});
codec_struct!(ResolvedFunction {
    id,
    name,
    params,
    result_id,
    return_type,
    effects,
    yields,
    requires,
    ensures,
    body,
    cleanup,
    cleanup_plan,
    loan_plan,
    span
});
codec_struct!(ResolvedFunctionInstance {
    id,
    template,
    type_arguments,
    function
});
codec_struct!(ResolvedFunctionTemplate {
    id,
    name,
    type_parameters,
    params,
    result_id,
    return_type,
    effects,
    requires,
    ensures,
    body,
    span
});
codec_struct!(ResolvedParam {
    id,
    name,
    ownership,
    ty,
    span
});
codec_struct!(ResolvedBinding {
    id,
    name,
    ownership,
    ty,
    span
});
codec_struct!(ResolvedClosureCapture { binding, value });
codec_struct!(ResolvedExpr {
    id,
    ty,
    ownership,
    kind,
    span
});
codec_enum!(ResolvedExprKind {
    0=>Int(value),1=>Int32(value),2=>Char(value),3=>Uint8(value),4=>Usize(value),5=>ArrayU8(values),
    6=>RepeatArrayU8{value,count},7=>Float32(bits),8=>Float64(bits),9=>Bool(value),10=>String(value),
    11=>Place(place),12=>BorrowPlace{operation,place},13=>ByteRange{operation,source,start,end},
    14=>Call{callee,type_arguments,instance,args},15=>NativeRustImportCall(call),16=>HostCommandCall(call),
    17=>Unary{op,value},18=>Binary{op,left,right},19=>Block{statements,tail},20=>If{condition,then_branch,else_branch},
    21=>ConstructRecord{record,fields},22=>ConstructVariant{variant,case,fields},23=>Match{mode,scrutinee,arms},
    24=>Try{operand,result,ok_case,ok_field,err_case,err_field,residual_type},
    25=>TryOption{operand,option,some_case,some_field,none_case,residual_type},
    26=>UpdateRecord{base,record,fields},27=>Project{base,field},28=>Upcast{source},29=>FunctionReference{target},30=>Invoke{callable,args},31=>Closure{parameters,captures,body},
    32=>Yield{request},33=>LiteralFormat{template,args},34=>VecFieldRead{origin,element,field,bytes,args},35=>LexicalExit{kind,target,value},36=>BorrowedUtf8Literal(value),37=>VecConstructor{element,count,statements,tail},
    38=>RowLoan{loan,args}
});
codec_enum!(VecFieldReadOrigin { 0=>Explicit,1=>Traversal });
// Row Loan v1: decoded descriptors carry no authority until complete HIR
// validation replays their call slot, traversal region and callee proof.
codec_enum!(RowLoanVersion { 0=>V1 });
codec_struct!(RowLoan {
    version,
    element,
    traversal,
    bound,
    callee,
    parameter
});
codec_struct!(ResolvedMatchArm {
    pattern,
    guard,
    value,
    span
});
codec_enum!(ResolvedMatchPattern {0=>Variant{variant,case,fields},1=>Record{record,instance,fields},2=>Wildcard,3=>Literal(value),4=>Or(alternatives),5=>Binding(binding)});
codec_enum!(PatternValue {0=>Int(value),1=>Int32(value),2=>Uint8(value),3=>Usize(value),4=>Char(value),5=>Bool(value)});
codec_struct!(ResolvedMatchPatternField { field, binding });
codec_struct!(ResolvedRecordMatchPatternField { field, pattern });
codec_enum!(ResolvedRecordMatchFieldPattern {0=>Binding(binding),1=>Wildcard,2=>Record{record,instance,fields}});
codec_struct!(ResolvedFieldInitializer { field, value });
codec_enum!(ResolvedStatement {0=>Let{binding,mutable,value,span},1=>Assign{binding,field,value,span},2=>Unsafe{audit,body,span},3=>While{condition,body,span},4=>LexicalWhile{condition,body,latch,span},5=>CopyDiscard{value,span}});
codec_struct!(Place { root, projections });
codec_enum!(PlaceProjection {0=>Field(field),1=>VariantField{case,field}});

#[cfg(test)]
mod tests;

#[cfg(test)]
mod record_traversal_tests;

#[cfg(test)]
mod byte_traversal_tests;

impl Codec for super::StaticUtf8 {
    fn encode(&self, encoder: &mut Encoder) -> crate::cache_codec::Result<()> {
        encoder.nested(|encoder| match self.cached_text() {
            Some(text) => text.encode(encoder),
            None => Err(crate::cache_codec::capacity(
                "static text backing allocation was refused",
            )),
        })
    }
    fn decode(decoder: &mut Decoder<'_>) -> crate::cache_codec::Result<Self> {
        decoder.nested(|decoder| {
            let text = String::decode(decoder)?;
            decoder.allocate(Self::OWNED_ALLOCATION_CARRIER_BYTES)?;
            let value = Self::from_owned(text);
            if !value.is_backed() {
                return Err(crate::cache_codec::capacity(
                    "static text backing allocation was refused",
                ));
            }
            Ok(value)
        })
    }
}
