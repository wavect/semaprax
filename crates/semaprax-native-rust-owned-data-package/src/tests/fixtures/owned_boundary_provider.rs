// Test-only ABI protocol double, compiled in a separate subprocess crate.
// Never included as Rust code by the unsafe-free package or generated SDK.
use std::sync::atomic::{AtomicU32, Ordering};
static MODE: AtomicU32 = AtomicU32::new(0);
static INITS: AtomicU32 = AtomicU32::new(0);
#[repr(C)]
struct State {
    live: u64,
}
fn mode() -> u32 {
    MODE.load(Ordering::Relaxed)
}
fn event(value: &str) {
    println!("event:{value}");
}
#[no_mangle]
extern "C" fn spx_owned_data_context_size_v1() -> u64 {
    8
}
#[no_mangle]
extern "C" fn spx_owned_data_context_align_v1() -> u64 {
    8
}
#[no_mangle]
unsafe extern "C" fn spx_owned_data_context_init_v1(context: *mut State, length: u64) -> u32 {
    event("init");
    let attempt = INITS.fetch_add(1, Ordering::Relaxed);
    if mode() == 18 || (mode() == 19 && attempt != 0) {
        return 2;
    }
    assert_eq!(length, 8);
    unsafe {
        (*context).live = 0;
    }
    0
}
#[no_mangle]
unsafe extern "C" fn spx_owned_data_context_drop_v1(context: *mut State) -> u32 {
    event("close");
    if mode() == 8 || unsafe { (*context).live != 0 } {
        return 5;
    }
    0
}
#[no_mangle]
unsafe extern "C" fn spx_owned_bytes_len_v1(
    context: *mut State,
    handle: u64,
    length: *mut u64,
) -> u32 {
    event("len");
    assert_eq!(handle, 1);
    assert!(unsafe { (*context).live > 0 });
    if mode() == 5 {
        return 2;
    }
    unsafe {
        *length = match mode() {
            6 => 65537,
            15 => 0,
            16 => 65536,
            _ => 3,
        };
    }
    0
}
#[no_mangle]
unsafe extern "C" fn spx_owned_bytes_copy_v1(
    context: *mut State,
    handle: u64,
    destination: *mut u8,
    length: u64,
) -> u32 {
    event("copy");
    assert_eq!(handle, 1);
    assert!(unsafe { (*context).live > 0 });
    if length == 0 {
        assert!(destination.is_null());
        return 0;
    }
    assert!(!destination.is_null());
    if mode() == 4 || mode() == 27 {
        unsafe {
            *destination = b'x';
        }
        return 4;
    }
    for index in 0..length as usize {
        unsafe {
            *destination.add(index) = if mode() == 17 { 0xff } else { b'a' };
        }
    }
    0
}
#[no_mangle]
unsafe extern "C" fn spx_owned_bytes_drop_v1(context: *mut State, handle: u64) -> u32 {
    event("drop");
    assert_eq!(handle, 1);
    assert!(unsafe { (*context).live > 0 });
    if mode() == 7 || mode() == 27 {
        return 5;
    }
    unsafe {
        (*context).live -= 1;
    }
    0
}
unsafe fn returned_status(context: *mut State) -> Option<u32> {
    match mode() {
        12 => Some(77),
        13 => Some(1),
        14 => Some(2),
        22 => {
            unsafe {
                (*context).live = 1;
            }
            Some(2)
        }
        _ => None,
    }
}
unsafe fn owned_call(context: *mut State, tag: *mut u32, handle: *mut u64, error: *mut i64) -> u32 {
    event("call");
    if mode() == 26 {
        unsafe {
            *handle = 1;
            (*context).live = 1;
        }
        return 2;
    }
    if let Some(status) = unsafe { returned_status(context) } {
        return status;
    }
    unsafe {
        *tag = if KIND == 1 { 1 } else { 0 };
        *handle = 1;
        *error = 0;
        (*context).live = if mode() == 1 { 2 } else { 1 };
        match mode() {
            2 => {
                *tag = 0;
                *handle = 0;
            }
            3 => {
                *tag = 1;
                *handle = 0;
                *error = 42;
            }
            9 => {
                *tag = 7;
            }
            10 => {
                *tag = if KIND == 1 { 0 } else { 1 };
            }
            11 => {
                *error = 9;
            }
            23 => {
                *tag = 0;
                *handle = 0;
                (*context).live = 0;
            }
            24 => {
                *tag = 1;
                *handle = 0;
                *error = 42;
                (*context).live = 0;
            }
            _ => {}
        }
    }
    0
}
unsafe fn flat_call(context: *mut State, record: *mut u64) -> u32 {
    event("call");
    if mode() == 26 {
        unsafe {
            *record.add(1) = 1;
            (*context).live = 1;
        }
        return 2;
    }
    if let Some(status) = unsafe { returned_status(context) } {
        return status;
    }
    unsafe {
        *record = 42;
        *record.add(1) = 1;
        *record.add(2) = if mode() == 20 { 2 } else { 1 };
        (*context).live = if mode() == 1 { 2 } else { 1 };
    }
    0
}
