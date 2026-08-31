//! Actual owned-pipe flag normalization, not a complete launcher execution.
use super::inherit;
use std::fs::File;
use std::os::fd::{AsRawFd, FromRawFd};

#[test]
fn selected_pipe_clears_cloexec_without_changing_peer_or_open_file_flags() {
    let mut descriptors = [-1; 2];
    assert_eq!(
        unsafe { libc::pipe2(descriptors.as_mut_ptr(), libc::O_CLOEXEC) },
        0
    );
    let read = unsafe { File::from_raw_fd(descriptors[0]) };
    let write = unsafe { File::from_raw_fd(descriptors[1]) };
    let flags = unsafe { libc::fcntl(read.as_raw_fd(), libc::F_GETFL) };
    assert!(flags >= 0);
    assert_eq!(
        unsafe { libc::fcntl(read.as_raw_fd(), libc::F_GETFD) },
        libc::FD_CLOEXEC
    );
    assert_eq!(inherit(read.as_raw_fd()), Ok(()));
    assert_eq!(unsafe { libc::fcntl(read.as_raw_fd(), libc::F_GETFD) }, 0);
    assert_eq!(
        unsafe { libc::fcntl(write.as_raw_fd(), libc::F_GETFD) },
        libc::FD_CLOEXEC
    );
    assert_eq!(
        unsafe { libc::fcntl(read.as_raw_fd(), libc::F_GETFL) },
        flags
    );
    assert_eq!(inherit(read.as_raw_fd()), Ok(()));
    assert_eq!(inherit(-1), Err(()));
}
