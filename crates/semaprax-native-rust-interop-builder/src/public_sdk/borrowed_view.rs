//! Safe Rust fragments for closed owner-tied, callback-scoped string views.
//! They carry no Semaprax loan proof and never manufacture a reference from a
//! raw carrier. Only the exact library method type establishes the relation.

pub(super) fn render(owner: &str, method: &str) -> String {
    format!(
        r#"
pub struct SpxBorrowedStrView<'owner> {{ value: &'owner str }}
impl SpxBorrowedStrView<'_> {{ pub fn as_str(&self)->&str {{ self.value }} }}
impl SpxBorrowedInputAdapter {{
 pub fn with_str_view<R>(&self, consumer: impl for<'view> FnOnce(SpxBorrowedStrView<'view>)->R)->Result<R,SpxBorrowedInputError> {{
  if self.active.replace(true){{self.rejected_reentries.set(self.rejected_reentries.get()+1);return Err(SpxBorrowedInputError::Reentered)}}
  // The guard belongs to the invocation, so forgetting the view cannot end it.
  let _active=SpxBorrowedInputGuard{{active:&self.active}};
  let target:for<'owner> fn(&'owner {owner})->&'owner str={method};
  let value=target(&self.owner);
  self.target_calls.set(self.target_calls.get()+1);
  Ok(consumer(SpxBorrowedStrView{{value}}))
 }}
 pub fn with_exclusive<R>(&mut self, consumer: impl FnOnce(&mut {owner})->R)->Result<R,SpxBorrowedInputError> {{
  if self.active.replace(true){{self.rejected_reentries.set(self.rejected_reentries.get()+1);return Err(SpxBorrowedInputError::Reentered)}}
  let _active=SpxBorrowedInputGuard{{active:&self.active}};
  Ok(consumer(&mut self.owner))
 }}
 pub fn replace_owner(&mut self, owner:{owner})->{owner} {{ core::mem::replace(&mut self.owner,owner) }}
 pub fn into_owner(self)->{owner} {{ self.owner }}
}}
"#
    )
}

/// Url inspection has no borrowed-input predicate. It shares only the scoped
/// view operations and the per-owner invocation guard with the Regex fragment.
pub(super) fn render_url() -> String {
    let mut source = String::from(
        "use core::cell::Cell;\n\
#[derive(Debug)]pub enum SpxBorrowedInputError{Reentered}\n\
struct SpxBorrowedInputGuard<'a>{active:&'a Cell<bool>}\n\
impl Drop for SpxBorrowedInputGuard<'_>{fn drop(&mut self){self.active.set(false)}}\n\
pub struct SpxBorrowedInputAdapter{owner:url_alias::Url,active:Cell<bool>,rejected_reentries:Cell<usize>,target_calls:Cell<usize>}\n\
impl SpxBorrowedInputAdapter{\n\
pub fn new(owner:url_alias::Url)->Self{Self{owner,active:Cell::new(false),rejected_reentries:Cell::new(0),target_calls:Cell::new(0)}}\n\
pub fn rejected_reentries(&self)->usize{self.rejected_reentries.get()}\n\
pub fn target_calls(&self)->usize{self.target_calls.get()}\n\
}\n",
    );
    source.push_str(&render("url_alias::Url", "url_alias::Url::as_str"));
    source
}
