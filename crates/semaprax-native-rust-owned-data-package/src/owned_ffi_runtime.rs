//! Shared generated private FFI lifetime protocol for owned-data SDK profiles.
//!
//! A copied host value stays inside `invoke` until the existing provider context
//! close proves that no owner remains. This adds no provider operation or ABI.

pub(super) const CONTEXT: &str = r#"
pub(super)struct Context{
    storage:Vec<u64>,raw:NonNull<RawContext>,size:u64,known_open:bool,
    _thread:PhantomData<Rc<()>>
}
impl Context{
pub fn new()->Result<Self,Failure>{
    let size=unsafe{spx_owned_data_context_size_v1()};
    let align=unsafe{spx_owned_data_context_align_v1()};
    if size==0||align==0||align>core::mem::align_of::<u64>()as u64{return Err(Failure::Adapter)}
    let words=usize::try_from(size.checked_add(7).ok_or(Failure::Adapter)?/8).map_err(|_|Failure::Adapter)?;
    let mut storage=vec![0u64;words];
    let raw:NonNull<RawContext>=NonNull::new(storage.as_mut_ptr().cast()).ok_or(Failure::Host)?;
    if unsafe{spx_owned_data_context_init_v1(raw.as_ptr().cast(),size)}!=0{return Err(Failure::Adapter)}
    Ok(Self{storage,raw,size,known_open:true,_thread:PhantomData})
}
pub fn invoke<T>(&mut self,call:impl FnOnce(&mut Self)->T)->Result<T,Failure>{
    if !self.known_open{
        if unsafe{spx_owned_data_context_init_v1(self.raw.as_ptr().cast(),self.size)}!=0{return Err(Failure::Adapter)}
        self.known_open=true;
    }
    let invocation=Invocation{context:self};
    let result=call(invocation.context);
    // `result` is still private. Drop performs the sole checked close before
    // either a language value or an ordinary SDK error can reach its caller.
    drop(invocation);
    Ok(result)
}
fn close(&mut self){
    if self.known_open{
        // Clear first: an uncertain close is fail-stop, never a retry.
        self.known_open=false;
        if unsafe{spx_owned_data_context_drop_v1(self.raw.as_ptr())}!=0{std::process::abort()}
    }
}
"#;

pub(super) fn append_owner_operations(
    output: &mut String,
    exact_capacity: bool,
    needs_discard: bool,
) {
    output.push_str(r#"
pub fn copy_and_settle(&mut self,handle:Handle)->Result<Vec<u8>,Failure>{
    let mut guard=Guard{context:self,handle,armed:true};
    let mut length=0u64;
    if unsafe{spx_owned_bytes_len_v1(guard.context.raw.as_ptr(),handle,&mut length)}!=0{return Err(Failure::Adapter)}
    if length>65536{return Err(Failure::Adapter)}
    let length=usize::try_from(length).map_err(|_|Failure::Adapter)?;
    let mut bytes=vec![0u8;length];
"#);
    if exact_capacity {
        output.push_str("if bytes.capacity()!=length{return Err(Failure::Host)}\n");
    }
    output.push_str(r#"
    let pointer=if length==0{core::ptr::null_mut()}else{bytes.as_mut_ptr()};
    if unsafe{spx_owned_bytes_copy_v1(guard.context.raw.as_ptr(),handle,pointer,length as u64)}!=0{return Err(Failure::Adapter)}
    if unsafe{spx_owned_bytes_drop_v1(guard.context.raw.as_ptr(),handle)}!=0{std::process::abort()}
    guard.armed=false;
    Ok(bytes)
}
"#);
    if needs_discard {
        output.push_str(
            r#"pub fn discard(&mut self,handle:Handle)->Result<(),Failure>{
    if unsafe{spx_owned_bytes_drop_v1(self.raw.as_ptr(),handle)}!=0{std::process::abort()}
    Ok(())
}
"#,
        );
    }
    output.push_str(r#"}
struct Guard<'a>{context:&'a mut Context,handle:Handle,armed:bool}
impl Drop for Guard<'_>{
    fn drop(&mut self){
        if self.armed&&unsafe{spx_owned_bytes_drop_v1(self.context.raw.as_ptr(),self.handle)}!=0{std::process::abort()}
    }
}
"#);
}

pub(super) fn append_multi_owner_operations(output: &mut String) {
    output.push_str(r#"
pub fn copy_many_and_settle(&mut self,handles:&[Handle])->Result<Vec<Vec<u8>>,Failure>{
    if handles.is_empty()||handles.len()>256{return Err(Failure::Adapter)}
    for(index,handle)in handles.iter().enumerate(){if *handle==0||handles[..index].contains(handle){std::process::abort()}}
    let mut guard=ManyGuard{context:self,handles:handles.to_vec(),armed:true};
    let mut lengths=Vec::with_capacity(handles.len());let mut total=0u64;
    for handle in handles{let mut length=0u64;if unsafe{spx_owned_bytes_len_v1(guard.context.raw.as_ptr(),*handle,&mut length)}!=0{std::process::abort()}total=total.checked_add(length).filter(|total|*total<=65536).ok_or(Failure::Adapter)?;lengths.push(usize::try_from(length).map_err(|_|Failure::Adapter)?)}
    let mut values=Vec::with_capacity(handles.len());for length in lengths{values.push(vec![0u8;length])}
    for(index,handle)in handles.iter().enumerate(){let bytes=&mut values[index];let pointer=if bytes.is_empty(){core::ptr::null_mut()}else{bytes.as_mut_ptr()};if unsafe{spx_owned_bytes_copy_v1(guard.context.raw.as_ptr(),*handle,pointer,bytes.len()as u64)}!=0{return Err(Failure::Adapter)}}
    for handle in handles{if unsafe{spx_owned_bytes_drop_v1(guard.context.raw.as_ptr(),*handle)}!=0{std::process::abort()}}
    guard.armed=false;Ok(values)
}
}
struct ManyGuard<'a>{context:&'a mut Context,handles:Vec<Handle>,armed:bool}
impl Drop for ManyGuard<'_>{fn drop(&mut self){if self.armed{for handle in &self.handles{if unsafe{spx_owned_bytes_drop_v1(self.context.raw.as_ptr(),*handle)}!=0{std::process::abort()}}}}}
"#);
}

// Always emitted, even when all selected results are scalars: successful
// copying is not the proof that the complete provider context has settled.
pub(super) const INVOCATION: &str = r#"// Owner guards are nested inside the invocation callback. Rust unwinds those
// first, then reaches this guard; provider closure therefore follows ownership
// settlement on ordinary errors and on host unwinding alike.
struct Invocation<'a>{context:&'a mut Context}
impl Drop for Invocation<'_>{fn drop(&mut self){self.context.close()}}
impl Drop for Context{fn drop(&mut self){let _=self.storage.len();self.close()}}
"#;
