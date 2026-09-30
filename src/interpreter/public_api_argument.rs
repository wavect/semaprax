/// One borrowed Project-v8/v9 public invocation argument. Borrowed host
/// carriers are snapshotted before evaluation and never become interpreter
/// ownership.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PublicApiArgument<'a> {
    I64(i64),
    U8(u8),
    Usize(u64),
    Bool(bool),
    BorrowStr(&'a str),
    BorrowSliceU8(&'a [u8]),
}
