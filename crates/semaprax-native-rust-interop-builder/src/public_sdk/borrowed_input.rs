//! RI-06 generated-adapter fragment for an invocation-scoped borrowed input.
//!
//! This is intentionally only a renderer. `owner_sdk` remains the authority
//! that selects a validated owner, and may attach this fragment only after the
//! HIR loan plan has authenticated its lifetime. The fragment itself holds no
//! carrier, does no conversion, and has no authority to extend a borrow.

use super::sdk_error;
use semaprax::diagnostic::Diagnostic;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum BorrowedInputProfile {
    RegexUtf8,
    RegexBytes,
}

impl BorrowedInputProfile {
    const fn owner_type(self) -> &'static str {
        match self {
            Self::RegexUtf8 => "regex_alias::Regex",
            Self::RegexBytes => "regex_alias::bytes::Regex",
        }
    }

    const fn method_path(self) -> &'static str {
        match self {
            Self::RegexUtf8 => "regex_alias::Regex::is_match",
            Self::RegexBytes => "regex_alias::bytes::Regex::is_match",
        }
    }

    const fn input_type(self) -> &'static str {
        match self {
            Self::RegexUtf8 => "&str",
            Self::RegexBytes => "&[u8]",
        }
    }
}

/// Renders the direct-reference adapter used by the future owned-receiver
/// invocation seam. The caller must supply source for `regex_alias`; this
/// renderer deliberately does not load packages, invoke a compiler, or create
/// a Rust value from an ABI carrier.
pub(super) fn render_borrowed_input_adapter(
    profile: BorrowedInputProfile,
) -> Result<String, Diagnostic> {
    let owner = profile.owner_type();
    let method = profile.method_path();
    let input = profile.input_type();
    if owner.is_empty() || method.is_empty() || input.is_empty() {
        return Err(sdk_error("borrowed input profile is incomplete"));
    }
    Ok(format!(
        "use core::cell::Cell;\
#[derive(Debug)]pub enum SpxBorrowedInputError{{Reentered}}\
struct SpxBorrowedInputGuard<'a>{{active:&'a Cell<bool>}}\
impl Drop for SpxBorrowedInputGuard<'_>{{fn drop(&mut self){{self.active.set(false)}}}}\
pub struct SpxBorrowedInputAdapter{{owner:{owner},active:Cell<bool>,adapter_copies:Cell<usize>,rejected_reentries:Cell<usize>,target_calls:Cell<usize>,last_input_pointer:Cell<usize>,last_input_length:Cell<usize>}}\
impl SpxBorrowedInputAdapter{{\
pub fn new(owner:{owner})->Self{{Self{{owner,active:Cell::new(false),adapter_copies:Cell::new(0),rejected_reentries:Cell::new(0),target_calls:Cell::new(0),last_input_pointer:Cell::new(0),last_input_length:Cell::new(0)}}}}\
pub fn adapter_copies(&self)->usize{{self.adapter_copies.get()}}\
pub fn rejected_reentries(&self)->usize{{self.rejected_reentries.get()}}\
pub fn target_calls(&self)->usize{{self.target_calls.get()}}\
pub fn last_input_pointer(&self)->usize{{self.last_input_pointer.get()}}\
pub fn last_input_length(&self)->usize{{self.last_input_length.get()}}\
pub fn invoke(&self,input:{input})->Result<bool,SpxBorrowedInputError>{{self.invoke_with_pre_call(input,|_|{{}})}}\
pub fn invoke_with_pre_call(&self,input:{input},pre_call:impl FnOnce(&Self))->Result<bool,SpxBorrowedInputError>{{\
if self.active.replace(true){{self.rejected_reentries.set(self.rejected_reentries.get()+1);return Err(SpxBorrowedInputError::Reentered)}}\
let _active=SpxBorrowedInputGuard{{active:&self.active}};\
pre_call(self);\
let input_pointer=input.as_ptr();let input_length=input.len();\
self.last_input_pointer.set(input_pointer as usize);self.last_input_length.set(input_length);\
let target:fn(&{owner},{input})->bool={method};\
let matched=target(&self.owner,input);\
debug_assert_eq!(input_pointer,input.as_ptr());debug_assert_eq!(input_length,input.len());\
debug_assert_eq!(self.adapter_copies.get(),0);\
self.target_calls.set(self.target_calls.get()+1);Ok(matched)\
}}\
}}\n"
    ))
}
