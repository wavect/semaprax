use super::*;

#[test]
fn capture_helper() {
    let Ok(mode) = std::env::var("SEMAPRAX_ARCHIVE_CAPTURE_HELPER") else {
        return;
    };
    match mode.as_str() {
        "ok" => {
            println!("capture-helper-ok");
            std::process::exit(0);
        }
        "fail" => {
            eprintln!("capture-helper-failed");
            std::process::exit(7);
        }
        "large" => {
            std::io::stdout().write_all(&[b'x'; 8192]).unwrap();
            std::process::exit(0);
        }
        "wait" => {
            std::thread::sleep(Duration::from_secs(30));
            std::process::exit(0);
        }
        _ => panic!("unknown helper mode"),
    }
}

#[test]
fn finite_capture_preserves_exit_status_and_rejects_overflow_and_deadline() {
    let fixture = crate::Fixture::new("capture");
    let child = |mode: &str| {
        let mut command = Command::new(std::env::current_exe().unwrap());
        command
            .args(["--exact", "command::tests::capture_helper", "--nocapture"])
            .env("SEMAPRAX_ARCHIVE_CAPTURE_HELPER", mode)
            .current_dir(&fixture.root);
        command
    };
    for (mode, code) in [("ok", 0), ("fail", 7)] {
        let output = run(
            &mut child(mode),
            b"",
            &fixture.root.join(mode),
            Duration::from_secs(10),
            4096,
            4096,
        );
        assert_eq!(output.status.code(), Some(code));
        let bytes = if mode == "ok" {
            output.stdout
        } else {
            output.stderr
        };
        assert!(String::from_utf8(bytes).unwrap().contains(if mode == "ok" {
            "capture-helper-ok"
        } else {
            "capture-helper-failed"
        }));
    }
    let overflow = attempt(
        &mut child("large"),
        b"",
        &fixture.root.join("large"),
        Duration::from_secs(10),
        512,
        4096,
    )
    .unwrap_err();
    assert!(overflow.contains("byte limit"), "{overflow}");
    let deadline = attempt(
        &mut child("wait"),
        b"",
        &fixture.root.join("wait"),
        Duration::from_millis(100),
        4096,
        4096,
    )
    .unwrap_err();
    assert!(deadline.contains("deadline"), "{deadline}");
    assert!(!deadline.contains("uncertain"), "{deadline}");
    let occupied = fixture.root.join("occupied");
    fs::create_dir(&occupied).unwrap();
    fs::write(occupied.join("sentinel"), b"retain").unwrap();
    assert!(attempt(
        &mut child("ok"),
        b"",
        &occupied,
        Duration::from_secs(1),
        4096,
        4096
    )
    .is_err());
    assert_eq!(fs::read(occupied.join("sentinel")).unwrap(), b"retain");
}
