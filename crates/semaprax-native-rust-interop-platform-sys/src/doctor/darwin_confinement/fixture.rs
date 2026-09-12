// Hostile-input fixture for `doctor::darwin_confinement`'s tests. Compiled at
// test time (see `tests.rs`), never shipped. Each mode is a deliberate probe
// of one confinement boundary; the harness runs it both unconfined (to prove
// the action would otherwise succeed) and confined (to prove Seatbelt denies
// it with a specific errno, not merely "something failed").
fn main() {
    let mut args = std::env::args().skip(1);
    match args.next().as_deref() {
        Some("healthy") => {
            let scratch = args.next().expect("healthy mode needs a scratch path");
            std::fs::write(format!("{scratch}/ok.txt"), b"confined-ok")
                .expect("write inside the confined scratch root must succeed");
            println!("healthy-ok");
        }
        Some("fail") => std::process::exit(9),
        Some("sleep") => {
            std::thread::sleep(std::time::Duration::from_secs(5));
            println!("should-not-print");
        }
        Some("leak-descendant") => {
            // Spawn a detached grandchild and exit without waiting on it. The
            // grandchild inherits this process's group, so it stays a member
            // of the confined process group after the primary exits.
            let _ = std::process::Command::new("/bin/sleep").arg("2").spawn();
            println!("leaked");
        }
        Some("escape-write") => {
            let outside = args.next().expect("escape-write mode needs a target path");
            match std::fs::write(&outside, b"escaped") {
                Ok(()) => println!("write-permitted"),
                Err(err) => println!("write-denied:{}", err.raw_os_error().unwrap_or(-1)),
            }
        }
        Some("escape-network") => match std::net::TcpStream::connect("127.0.0.1:18237") {
            Ok(_) => println!("connect-permitted"),
            Err(err) => println!("connect-denied:{}", err.raw_os_error().unwrap_or(-1)),
        },
        _ => std::process::exit(2),
    }
}
