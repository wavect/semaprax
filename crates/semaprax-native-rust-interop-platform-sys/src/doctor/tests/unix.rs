//! Signal-policy mutations stay in isolated subprocesses, never the test host.
use super::*;

#[test]
fn automatic_reaping_policies_are_rejected_before_launch() {
    for mode in ["ignore", "no-child-wait", "handler"] {
        let output = Command::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                "doctor::tests::unix::signal_policy_subprocess",
                "--nocapture",
            ])
            .env("SEMAPRAX_DOCTOR_SIGNAL_POLICY", mode)
            .env("SEMAPRAX_DOCTOR_SIGNAL_FIXTURE", fixture("normal"))
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{mode}: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert!(String::from_utf8_lossy(&output.stdout).contains("policy-rejected"));
    }
}

extern "C" fn custom_handler(_: libc::c_int) {}

#[test]
fn signal_policy_subprocess() {
    let Some(mode) = std::env::var_os("SEMAPRAX_DOCTOR_SIGNAL_POLICY") else {
        return;
    };
    let path = std::env::var_os("SEMAPRAX_DOCTOR_SIGNAL_FIXTURE").unwrap();
    let mut prepared = prepare(Path::new(&path)).unwrap();
    // This sentinel would return Spawn if launch admission were reached. It
    // avoids creating a child if a regression removes the policy rejection.
    prepared.fault = Some(Fault::Spawn);
    let mut action = unsafe { std::mem::zeroed::<libc::sigaction>() };
    match mode.to_str().unwrap() {
        "ignore" => action.sa_sigaction = libc::SIG_IGN,
        "no-child-wait" => {
            action.sa_sigaction = libc::SIG_DFL;
            action.sa_flags = libc::SA_NOCLDWAIT;
        }
        "handler" => action.sa_sigaction = custom_handler as *const () as usize,
        _ => panic!("closed signal-policy vocabulary"),
    }
    assert_eq!(unsafe { libc::sigemptyset(&mut action.sa_mask) }, 0);
    assert_eq!(
        unsafe { libc::sigaction(libc::SIGCHLD, &action, std::ptr::null_mut()) },
        0
    );
    assert_eq!(run(&prepared), Err(ProbeError::Invalid));
    println!("policy-rejected");
}
