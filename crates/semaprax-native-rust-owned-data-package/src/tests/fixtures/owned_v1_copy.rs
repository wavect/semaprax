#![allow(dead_code)]
#[path="sdk.rs"]mod sdk;
use std::sync::atomic::{AtomicBool,AtomicUsize,Ordering::SeqCst};
const KIND:usize=@KIND@;
static LENGTH:AtomicUsize=AtomicUsize::new(0);
static INACTIVE:AtomicBool=AtomicBool::new(false);
static COPIES:AtomicUsize=AtomicUsize::new(0);
static DROPS:AtomicUsize=AtomicUsize::new(0);
static CLOSES:AtomicUsize=AtomicUsize::new(0);
#[repr(C)]struct State{payload:*mut Vec<u8>}
#[no_mangle]extern "C" fn spx_owned_data_context_size_v1()->u64{std::mem::size_of::<State>() as u64}
#[no_mangle]extern "C" fn spx_owned_data_context_align_v1()->u64{std::mem::align_of::<State>() as u64}
#[no_mangle]unsafe extern "C" fn spx_owned_data_context_init_v1(state:*mut State,len:u64)->u32{
 assert_eq!(len,std::mem::size_of::<State>() as u64);
 // SDK supplies fresh storage of the requested size/alignment.
 unsafe{state.write(State{payload:std::ptr::null_mut()})};0
}
#[no_mangle]unsafe extern "C" fn spx_owned_data_context_drop_v1(state:*mut State)->u32{
 assert!(unsafe{(*state).payload.is_null()});CLOSES.fetch_add(1,SeqCst);0
}
#[no_mangle]unsafe extern "C" fn spx_owned_data_call_spx_fixture_dot_value_v1(state:*mut State,tag:*mut u32,handle:*mut u64,error:*mut i64)->u32{
 assert!(unsafe{(*state).payload.is_null()});
 let inactive=INACTIVE.load(SeqCst);
 // All output pointers are initialized SDK locals. Each active owner remains
 // in provider allocation until exactly one matching drop.
 unsafe{*tag=if KIND==1{u32::from(!inactive)}else{u32::from(inactive)};
 *error=if inactive&&KIND==2{i64::MIN}else{0};*handle=if inactive{0}else{1};
 if !inactive{(*state).payload=Box::into_raw(Box::new(vec![0xff;LENGTH.load(SeqCst)]));}}
 0
}
#[no_mangle]unsafe extern "C" fn spx_owned_bytes_len_v1(state:*mut State,handle:u64,len:*mut u64)->u32{
 assert_eq!(handle,1);assert!(!unsafe{(*state).payload.is_null()});
 unsafe{*len=(*(*state).payload).len() as u64;}0
}
#[no_mangle]unsafe extern "C" fn spx_owned_bytes_copy_v1(state:*mut State,handle:u64,destination:*mut u8,len:u64)->u32{
 assert_eq!(handle,1);assert!(!unsafe{(*state).payload.is_null()});
 // Provider owns source, SDK owns destination; valid nonempty ranges are
 // distinct allocations. Empty copy must use the frozen null convention.
 let source=unsafe{&*(*state).payload};assert_eq!(source.len(),len as usize);
 if len==0{assert!(destination.is_null());}else{
 assert!(!destination.is_null());assert_ne!(source.as_ptr(),destination.cast_const());
 unsafe{std::ptr::copy_nonoverlapping(source.as_ptr(),destination,len as usize);}}
 COPIES.fetch_add(1,SeqCst);0
}
#[no_mangle]unsafe extern "C" fn spx_owned_bytes_drop_v1(state:*mut State,handle:u64)->u32{
 assert_eq!(handle,1);assert!(!unsafe{(*state).payload.is_null()});
 // Take once, overwrite provider bytes, and actually deallocate before public
 // success. Returned host bytes must survive both overwrite and deallocation.
 let pointer=unsafe{std::mem::replace(&mut (*state).payload,std::ptr::null_mut())};
 let mut payload=unsafe{Box::from_raw(pointer)};payload.fill(0x55);drop(payload);
 DROPS.fetch_add(1,SeqCst);0
}
fn main(){
 for (len,inactive) in [(0,false),(3,false),(65536,false),(0,true)]{
 if inactive&&KIND==0{continue;}
 LENGTH.store(len,SeqCst);INACTIVE.store(inactive,SeqCst);COPIES.store(0,SeqCst);DROPS.store(0,SeqCst);CLOSES.store(0,SeqCst);
 let mut sdk=sdk::NativeRustOwnedDataSdk::new().unwrap();let raw=sdk.spx_fixture_dot_value();let value=@NORMALIZE@;
 assert_eq!(COPIES.load(SeqCst),usize::from(!inactive),"copy-call-count");
 assert_eq!(DROPS.load(SeqCst),usize::from(!inactive));assert_eq!(CLOSES.load(SeqCst),1);
 if inactive{assert!(value.is_none());}else{let value=value.unwrap();assert_eq!(value.len(),len);assert!(value.iter().all(|b|*b==0xff));}
 drop(sdk);assert_eq!(CLOSES.load(SeqCst),1);
 }
 println!("owned-v1-copy-ok");
}
