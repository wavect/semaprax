use super::fixture::{all_bundle, native_bundle, request, request_target, Ending};
use super::{launch, observe, report};
use std::io::{self, Read};
use std::os::fd::{AsRawFd, IntoRawFd};
use std::time::{Duration, Instant};

#[test]
#[ignore = "requires actual provisioned worker/collector context; synthetic tools are not real distributions"]
fn all_three_roles_settle_and_tool_failure_is_an_ordinary_exit_one_report() {
    // Healthy controls bracket the failure. Rust runs after the failed Node
    // observation; a per-tool failure must not erase its successful row.
    for fail_node in [false, true, false] {
        let bundle = all_bundle(Ending::Exit(if fail_node { 7 } else { 0 }));
        let tools = [
            ("clang", "ok", "/bin/clang (clang version 1.0.0)"),
            (
                "node",
                if fail_node { "failed" } else { "ok" },
                if fail_node {
                    "offline tool terminated unsuccessfully"
                } else {
                    "v22.0.0"
                },
            ),
            ("rust", "ok", "rustc 1.88.0"),
        ];
        report::require(
            observe::run(&request_target(&bundle, 3), &bundle, None),
            "all",
            &tools,
            i32::from(fail_node),
        );
    }
}

#[test]
#[ignore = "requires actual provisioned worker/collector, pipe resizing and exclusive report reader"]
fn closed_report_sink_fails_after_collection_without_successful_delivery() {
    // 65,534 printable quotes + LF stay below the worker's exact output limit
    // and the literal ARM MOVZ length bound. JSON escaping exceeds 128KiB.
    let mut version = vec![b'"'; 65_534];
    version.push(b'\n');
    let bundle = native_bundle(&version);
    let request = request(&bundle);
    let detail = format!("/bin/clang ({})", "\"".repeat(65_534));
    let tools = [("clang", "ok", detail.as_str())];
    // Identical input and writer prefix must physically produce the entire
    // ordinary report before the destructive-reader variant is meaningful.
    report::require(observe::run(&request, &bundle, None), "native", &tools, 0);
    let expected = report::expected("native", &tools, "debug");
    let mut owned = observe::OwnedCollector(launch::spawn_with_capacity(
        &request,
        &bundle,
        None,
        Some(4096),
    ));
    let mut reader = owned.0.stdout.take().unwrap();
    let fd = reader.as_raw_fd();
    let capacity = unsafe { libc::fcntl(fd, libc::F_GETPIPE_SZ) };
    assert!(capacity > 0);
    assert!(
        (capacity as usize) + 64 < expected.len(),
        "report must exceed capacity plus consumed prefix"
    );
    let flags = unsafe { libc::fcntl(fd, libc::F_GETFL) };
    assert!(flags >= 0);
    assert_eq!(
        unsafe { libc::fcntl(fd, libc::F_SETFL, flags | libc::O_NONBLOCK) },
        0
    );
    let prefix = read_prefix(&mut reader).expect("observe bounded post-collection report prefix");
    assert_eq!(prefix.as_slice(), &expected[..64]);
    // Only this reader exists. The producer cannot have completed: total bytes
    // written <= capacity + the exact 64 consumed bytes < complete report length.
    // This prefix can only come from finish after successful live collection.
    assert!(
        owned.0.try_wait().unwrap().is_none(),
        "collector exited before sink closure"
    );
    // Scheduling can still race the delivery deadline after this observation.
    // This proves post-collection closed-sink failed delivery, not an isolated
    // errno EPIPE observation; exact error-branch evidence belongs to scripts.
    assert_eq!(
        unsafe { libc::close(reader.into_raw_fd()) },
        0,
        "close sole report reader exactly once"
    );
    let observation = observe::collect(&mut owned.0).expect("bounded failed report delivery");
    // Partial report bytes were intentionally observed above. This is failed
    // delivery after collection, not a claim of atomic/no-byte publication.
    assert_eq!(observation.status.code(), Some(126));
    assert!(observation.stdout.is_empty()); // reader deliberately consumed.
    assert!(observation.stderr.is_empty(), "{:?}", observation.stderr);
    report::require(observe::run(&request, &bundle, None), "native", &tools, 0);
}

fn read_prefix(reader: &mut impl Read) -> io::Result<[u8; 64]> {
    let mut bytes = [0; 64];
    let mut offset = 0;
    let deadline = Instant::now() + Duration::from_secs(65);
    while offset != bytes.len() {
        match reader.read(&mut bytes[offset..]) {
            Ok(0) => {
                return Err(io::Error::new(
                    io::ErrorKind::UnexpectedEof,
                    "report prefix",
                ))
            }
            Ok(count) => offset += count,
            Err(error) if error.kind() == io::ErrorKind::WouldBlock => {}
            Err(error) => return Err(error),
        }
        if Instant::now() >= deadline {
            return Err(io::Error::new(
                io::ErrorKind::TimedOut,
                "report prefix deadline",
            ));
        }
        std::thread::sleep(Duration::from_millis(1));
    }
    Ok(bytes)
}
