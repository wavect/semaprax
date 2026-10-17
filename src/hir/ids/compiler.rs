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
    Box,
    Option,
    OptionNone,
    OptionSome,
    OptionSomeValue,
    List,
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
            Self::Box => crate::prelude::BOX_ID,
            Self::Option => crate::prelude::OPTION_ID,
            Self::OptionNone => crate::prelude::OPTION_NONE_ID,
            Self::OptionSome => crate::prelude::OPTION_SOME_ID,
            Self::OptionSomeValue => crate::prelude::OPTION_SOME_VALUE_ID,
            Self::List => crate::list_ops::LIST_ID,
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
