#![cfg(all(unix, not(any(target_os = "linux", target_os = "macos"))))]

use semaprax::project::CandidateGitProcessAuthority;
use std::path::Path;

#[test]
fn held_git_process_authority_fails_closed_on_unsupported_unix() {
    let errors = match CandidateGitProcessAuthority::open(
        Path::new("/unsupported/git"),
        Path::new("/unsupported/repository.git"),
        1,
        1,
    ) {
        Err(errors) => errors,
        Ok(_) => panic!("unsupported Unix must reject before opening either pathname"),
    };
    assert_eq!(errors.len(), 1);
    assert_eq!(errors[0].code, "SPX-G266");
    assert_eq!(
        errors[0].message,
        "held Git execution requires Linux or macOS"
    );
}
