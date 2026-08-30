// Standalone trusted test executable; never linked into the library or CLI.
use std::io::{Read, Write};
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::time::Duration;

fn main() {
    let arguments = std::env::args_os().collect::<Vec<_>>();
    if arguments
        .get(1)
        .is_some_and(|value| value == "--fixture-child")
    {
        std::thread::sleep(Duration::from_secs(3));
        return;
    }
    assert_eq!(arguments.len(), 2);
    assert_eq!(arguments[1], "--version");
    let invoked = PathBuf::from(&arguments[0]);
    let scenario = invoked.file_stem().unwrap().to_str().unwrap();
    match scenario {
        "exact" => std::io::stdout().write_all(&vec![b'x'; 65_536]).unwrap(),
        "overflow" => {
            let _ = std::io::stdout().write_all(&vec![b'x'; 65_537]);
        }
        "stderr-overflow" => {
            let _ = std::io::stderr().write_all(&vec![b'x'; 65_537]);
        }
        "combined-overflow" => {
            let _ = std::io::stdout().write_all(&vec![b'x'; 32_768]);
            let _ = std::io::stderr().write_all(&vec![b'y'; 32_769]);
        }
        "sleep" => std::thread::sleep(Duration::from_secs(3)),
        "closed-sleep" => {
            close_output();
            std::thread::sleep(Duration::from_secs(3));
        }
        "nonzero" => std::process::exit(7),
        "invalid-utf8" => std::io::stdout().write_all(&[0xff]).unwrap(),
        "stdin-eof" => {
            let mut byte = [0_u8; 1];
            assert_eq!(std::io::stdin().read(&mut byte).unwrap(), 0);
            println!("rustc 1.88.0 (fixture)");
        }
        "descendant"
        | "closed-descendant"
        | "fault-kill-descendant"
        | "fault-settle-descendant"
        | "fault-close-descendant" => {
            let mut command = Command::new(&invoked);
            command.arg("--fixture-child").stdin(Stdio::null());
            if scenario == "closed-descendant" {
                command.stdout(Stdio::null()).stderr(Stdio::null());
            }
            let child = command.spawn().unwrap();
            std::fs::write(invoked.with_extension("pid"), child.id().to_string()).unwrap();
            // Deliberately do not wait: the doctor owns descendant settlement.
            drop(child);
            println!("rustc 1.88.0 (fixture)");
        }
        "normal" => println!("rustc 1.88.0 (fixture)"),
        #[cfg(target_os = "macos")]
        "closed-high-fd" => {
            unsafe extern "C" {
                fn fcntl(fd: i32, command: i32, ...) -> i32;
                fn __error() -> *mut i32;
            }
            unsafe {
                assert_eq!(fcntl(512, 1), -1); // F_GETFD
                assert_eq!(*__error(), 9); // EBADF
            }
            println!("rustc 1.88.0 (fixture)");
        }
        _ => panic!("unknown fixture scenario"),
    }
}

#[cfg(unix)]
fn close_output() {
    unsafe extern "C" {
        fn close(fd: i32) -> i32;
    }
    unsafe {
        assert_eq!(close(1), 0);
        assert_eq!(close(2), 0);
    }
}

#[cfg(windows)]
fn close_output() {
    #[link(name = "kernel32")]
    unsafe extern "system" {
        fn GetStdHandle(which: u32) -> *mut std::ffi::c_void;
        fn CloseHandle(handle: *mut std::ffi::c_void) -> i32;
    }
    unsafe {
        assert_ne!(CloseHandle(GetStdHandle(-11_i32 as u32)), 0);
        assert_ne!(CloseHandle(GetStdHandle(-12_i32 as u32)), 0);
    }
}
