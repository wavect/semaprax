//! Closed set of compiler-owned declaration spellings.
//!
//! Each variant names one fixed compiler constant. An identity built from it
//! borrows that `'static` text and allocates nothing. The set grants no
//! prelude, admission or provenance authority: equality, order, hash and the
//! cache wire are those of the bytes, so a compiler identity and an equal
//! shared one are interchangeable. Source-authored or formatted text cannot
//! name a variant, so a dynamic identity can never take this route.
use crate::hir::ResolvedHostCommandOperation;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum CompilerDeclaration {
    Vec,
    VecOperation(crate::vec_ops::VecOp),
    BoxOperation(crate::box_ops::BoxOp),
    StdinInspection(crate::stdin_stream_ops::PureOp),
    StrOperation(crate::str_ops::StrOp),
    ByteOperation(crate::byte_ops::ByteOp),
    HostIoOperation(crate::host_io_ops::HostIoOp),
    IterYield,
    IterDone,
    IterItem,
    IterRest,
    Box,
    Option,
    OptionNone,
    OptionSome,
    OptionSomeValue,
    Result,
    ResultOk,
    ResultOkValue,
    ResultErr,
    ResultErrError,
    List,
    ListNil,
    ListCons,
    ListHead,
    ListTail,
    ListStep,
    Iter,
    IterStep,
    Map,
    Set,
    StdinReader,
    ByteGet,
    FunctionInvoke,
    VecField,
    LiteralFormat,
    RowLoanUnbound,
    OnceConstruct,
    OnceInvoke,
    OnceMixedConstruct,
    OnceMixedInvoke,
    OncePairConstruct,
    OncePairInvoke,
    OnceDrop,
    OnceMixedDrop,
    OncePairDrop,
    BytesDrop,
    BoxDrop,
    IterDrop,
    StringTake,
    StringDrop,
    StringMapDrop,
    CollectionDrop,
    StdinReaderDrop,
    VecDrop,
    Command(ResolvedHostCommandOperation),
    MapOperation(crate::map_ops::MapOp),
    ListOperation(crate::list_ops::ListOp),
    IteratorOperation(crate::iterator_ops::IteratorOp),
}

impl CompilerDeclaration {
    pub(crate) const fn text(self) -> &'static str {
        use crate::hir::closure::once;
        match self {
            Self::StringTake => crate::string_ops::take::ID,
            Self::Vec => crate::prelude::VEC_ID,
            Self::VecOperation(op) => op.id(),
            Self::BoxOperation(op) => op.id(),
            Self::StdinInspection(op) => op.id(),
            Self::StrOperation(op) => op.id(),
            Self::ByteOperation(op) => op.id(),
            Self::HostIoOperation(op) => op.id(),
            Self::IterYield => crate::iterator_ops::YIELD_ID,
            Self::IterDone => crate::iterator_ops::DONE_ID,
            Self::IterItem => crate::iterator_ops::ITEM_ID,
            Self::IterRest => crate::iterator_ops::REST_ID,
            Self::Box => crate::prelude::BOX_ID,
            Self::Option => crate::prelude::OPTION_ID,
            Self::OptionNone => crate::prelude::OPTION_NONE_ID,
            Self::OptionSome => crate::prelude::OPTION_SOME_ID,
            Self::OptionSomeValue => crate::prelude::OPTION_SOME_VALUE_ID,
            Self::Result => crate::prelude::RESULT_ID,
            Self::ResultOk => crate::prelude::RESULT_OK_ID,
            Self::ResultOkValue => crate::prelude::RESULT_OK_VALUE_ID,
            Self::ResultErr => crate::prelude::RESULT_ERR_ID,
            Self::ResultErrError => crate::prelude::RESULT_ERR_ERROR_ID,
            Self::List => crate::list_ops::LIST_ID,
            Self::ListNil => crate::list_ops::NIL_CASE_ID,
            Self::ListCons => crate::list_ops::CONS_CASE_ID,
            Self::ListHead => crate::list_ops::HEAD_ID,
            Self::ListTail => crate::list_ops::TAIL_ID,
            Self::ListStep => crate::list_ops::STEP_ID,
            Self::Iter => crate::iterator_ops::ITER_ID,
            Self::IterStep => crate::iterator_ops::STEP_ID,
            Self::Map => crate::map_ops::MAP_ID,
            Self::Set => crate::map_ops::SET_ID,
            Self::StdinReader => crate::stdin_stream_ops::READER_ID,
            Self::ByteGet => crate::byte_ops::GET_ID,
            Self::FunctionInvoke => "core.function.invoke",
            Self::VecField => crate::vec_field::ID,
            Self::LiteralFormat => crate::literal_format::ID,
            Self::RowLoanUnbound => crate::hir::row_loan::UNBOUND_CALLEE,
            Self::OnceConstruct => once::CONSTRUCT_ID,
            Self::OnceInvoke => once::INVOKE_ID,
            Self::OnceMixedConstruct => once::MIXED_CONSTRUCT_ID,
            Self::OnceMixedInvoke => once::MIXED_INVOKE_ID,
            Self::OncePairConstruct => once::PAIR_CONSTRUCT_ID,
            Self::OncePairInvoke => once::PAIR_INVOKE_ID,
            Self::OnceDrop => once::DROP_ID,
            Self::OnceMixedDrop => once::MIXED_DROP_ID,
            Self::OncePairDrop => once::PAIR_DROP_ID,
            Self::BytesDrop => crate::cleanup::BYTES_DROP_LIFECYCLE_ID,
            Self::BoxDrop => crate::cleanup::BOX_DROP_LIFECYCLE_ID,
            Self::IterDrop => crate::cleanup::ITER_DROP_LIFECYCLE_ID,
            Self::StringDrop => crate::cleanup::STRING_DROP_LIFECYCLE_ID,
            Self::StringMapDrop => crate::string_ops::MAP_DROP_LIFECYCLE_ID,
            Self::CollectionDrop => crate::map_ops::DROP_ID,
            Self::StdinReaderDrop => crate::stdin_stream_ops::DROP_ID,
            Self::VecDrop => crate::cleanup::VEC_DROP_LIFECYCLE_ID,
            Self::Command(operation) => crate::command_io_ops::id(operation),
            Self::MapOperation(op) => op.id(),
            Self::ListOperation(op) => op.id(),
            Self::IteratorOperation(op) => op.id(),
        }
    }
}

// These existing owned-call sites formerly received an &str into new(). The
// closed enum replaces that receiver; it introduces no larger call carrier.
const _: () = assert!(std::mem::size_of::<CompilerDeclaration>() <= std::mem::size_of::<&str>());
const _: () = assert!(std::mem::align_of::<CompilerDeclaration>() <= std::mem::align_of::<&str>());

// Complete closed text dispatch for the additional warm preparation calls.
// Payload and helper types remain owned here or by their original operation
// module. The numeric packet cannot construct an identity or admit an operation.
#[cfg(test)]
impl CompilerDeclaration {
    pub(crate) const TEXT_CALL_CONTROLS: usize = {
        use std::mem::size_of;
        let branches = [
            size_of::<crate::vec_ops::VecOp>() + crate::vec_ops::VecOp::ID_CALL_CONTROLS,
            size_of::<crate::box_ops::BoxOp>() + crate::box_ops::BoxOp::ID_CALL_CONTROLS,
            size_of::<crate::stdin_stream_ops::PureOp>() + crate::stdin_stream_ops::PureOp::ID_CALL_CONTROLS,
            size_of::<crate::str_ops::StrOp>() + crate::str_ops::StrOp::ID_CALL_CONTROLS,
            size_of::<crate::byte_ops::ByteOp>() + crate::byte_ops::ByteOp::ID_CALL_CONTROLS,
            size_of::<crate::host_io_ops::HostIoOp>() + crate::host_io_ops::HostIoOp::ID_CALL_CONTROLS,
            size_of::<crate::map_ops::MapOp>() + crate::map_ops::MapOp::ID_CALL_CONTROLS,
            size_of::<crate::list_ops::ListOp>() + crate::list_ops::ListOp::ID_CALL_CONTROLS,
            size_of::<crate::iterator_ops::IteratorOp>() + crate::iterator_ops::IteratorOp::ID_CALL_CONTROLS,
            size_of::<ResolvedHostCommandOperation>() + crate::command_io_ops::ID_CALL_CONTROLS,
        ];
        let mut maximum = 0;
        let mut index = 0;
        while index < branches.len() {
            if branches[index] > maximum { maximum = branches[index]; }
            index += 1;
        }
        4 * size_of::<Self>() // copied text(self) actual/formal and match sum
            + 2 * size_of::<&'static str>() // text return/as_str arm receiver
            + maximum // mutually exclusive payload binding and project helper
    };
}
