//! Complete private checked-module wire carriers. Access stays behind the
//! authenticated cache owner; this module grants no source or backend authority.
use super::*;
use crate::cache_codec::{codec_enum, codec_struct, codec_tuple};

codec_tuple!(DeclarationId(0));
codec_tuple!(FunctionInstanceId(0));
codec_tuple!(ValueId(0));
codec_tuple!(ExpressionId(0));
codec_enum!(FunctionExecutionId {0=>Monomorphic(value),1=>Generic(value)});
codec_enum!(DeclarationKind {0=>Resource,1=>ResourceDrop,2=>Record,3=>Field,4=>Class,5=>Variant,6=>VariantCase,7=>CaseField,8=>Interface,9=>Import,10=>Function});
codec_enum!(IdentityOrigin {0=>Explicit,1=>Automatic,2=>CompilerOwned});
codec_struct!(Declaration {
    id,
    name,
    kind,
    identity_origin,
    owner
});
codec_enum!(ByteSliceRootKind {0=>FunctionParameter,1=>OwnedBytes,2=>FixedArray,3=>BorrowedStr,4=>CommandArguments});
codec_enum!(ByteSliceExtent {0=>Constant(value),1=>ParameterLength,2=>ValueLength});
codec_struct!(ByteSliceRangeStep {
    source,
    producer,
    start,
    end
});
codec_struct!(ByteSliceProvenance {
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
codec_enum!(ResolvedType {0=>Unit,1=>I64,2=>I32,3=>Char,4=>U8,5=>Usize,6=>ArrayU8(length),7=>F32,8=>F64,9=>Bool,10=>String,11=>Bytes,12=>Str,13=>SliceU8,14=>TypeParameter{owner,index},15=>Nominal{declaration,arguments}});
codec_struct!(TypeFacts {
    copy,
    contains_resource,
    sized,
    needs_drop,
    layout_key
});
codec_enum!(OwnershipMode {0=>Value,1=>Own,2=>Borrow,3=>Shared});
codec_enum!(ResolvedMatchMode {0=>Value,1=>Own,2=>Borrow});

// Every lookup map is part of the complete HIR value, including native import,
// inheritance, generic identity, and byte-slice provenance sidecars.
codec_struct!(DeclarationIndex {
    declarations,
    types_by_name,
    functions_by_name,
    fields_by_owner_name,
    record_fields,
    cases_by_owner_name,
    variant_cases,
    case_fields,
    type_parameters,
    imports_by_key,
    native_rust_imports_by_name,
    type_facts_by_id,
    class_parents,
    byte_slice_roots
});
codec_struct!(ResolvedProgram {
    module,
    permits,
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
codec_enum!(ResolvedHostCommandOperation {0=>ArgsLen,1=>ArgUtf8,2=>StdinRead,3=>StderrWrite,4=>StdoutAppend,5=>StderrAppend});
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
codec_enum!(ResolvedImportResultKind {0=>Unit,1=>I64,2=>Bool});
codec_enum!(ResolvedImportFailure {0=>Infallible,1=>Status{domain_id,normalization}});
codec_struct!(ResolvedFieldDeclaration {
    id,
    name,
    index,
    ty,
    span
});
codec_struct!(ResolvedFunction {
    id,
    name,
    params,
    result_id,
    return_type,
    effects,
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
    26=>UpdateRecord{base,record,fields},27=>Project{base,field},28=>Upcast{source}
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
codec_enum!(ResolvedStatement {0=>Let{binding,mutable,value,span},1=>Assign{binding,field,value,span},2=>Unsafe{audit,body,span},3=>While{condition,body,span}});
codec_struct!(Place { root, projections });
codec_enum!(PlaceProjection {0=>Field(field),1=>VariantField{case,field}});
