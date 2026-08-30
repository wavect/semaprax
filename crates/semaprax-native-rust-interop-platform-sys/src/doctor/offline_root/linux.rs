//! Detached tmpfs construction only; no namespace bootstrap or executable entry.
use super::{Error, Plan};
use std::mem::MaybeUninit;
use std::os::fd::RawFd;

// Linux UAPI linux/mount.h. The supported native 64-bit ABIs share this layout.
const CLOEXEC: libc::c_uint = 1;
const SET_STRING: libc::c_uint = 1;
const CREATE: libc::c_uint = 6;
const RDONLY: u64 = 1;
const NOSUID: u64 = 2;
const NODEV: u64 = 4;
const WRITE_CHUNK: usize = 8192;

#[repr(C)]
struct MountAttr {
    attr_set: u64,
    attr_clr: u64,
    propagation: u64,
    userns_fd: u64,
}

/// Explicitly owned by the already-controlled child. There is deliberately no
/// Drop: unwinding/implicit descriptor cleanup is not a child setup protocol.
#[derive(Debug)]
#[must_use]
pub(super) struct Root {
    fd: RawFd,
}

impl Root {
    pub(super) fn as_raw_fd(&self) -> RawFd {
        self.fd
    }

    /// The caller must own the controlled child and exclude descriptor reuse.
    pub(super) unsafe fn close(self) {
        close_owned(self.fd);
    }
}

/// Materialize only inside an already controlled child, with private mapped
/// user/mount namespace authority. The caller must exclude concurrent access to
/// the new descriptors/tree, signals that unwind, and return into parent code.
/// This function neither closes inherited descriptors nor authenticates that
/// bootstrap precondition. It allocates nothing and never retries a syscall.
pub(super) unsafe fn materialize(plan: &Plan<'_>) -> Result<Root, Error> {
    materialize_inner(
        plan,
        #[cfg(test)]
        None,
    )
}

fn materialize_inner(
    plan: &Plan<'_>,
    #[cfg(test)] mut control: Option<&mut TestControl>,
) -> Result<Root, Error> {
    let context = operation!(FsOpen, control, unsafe {
        libc::syscall(libc::SYS_fsopen, c"tmpfs".as_ptr(), CLOEXEC)
    });
    if context < 0 {
        return Err(Error::Io);
    }
    // Kernel-created descriptors fit c_int, including on both admitted ABIs.
    let context = context as RawFd;
    let prepared = (|| {
        for (key, value) in [
            (c"size", plan.size_value()),
            (c"nr_inodes", plan.inode_value()),
            (c"mode", c"0700"),
        ] {
            if operation!(Configure, control, unsafe {
                libc::syscall(
                    libc::SYS_fsconfig,
                    context,
                    SET_STRING,
                    key.as_ptr(),
                    value.as_ptr(),
                    0 as libc::c_int,
                )
            }) != 0
            {
                return Err(Error::Io);
            }
        }
        if operation!(Create, control, unsafe {
            libc::syscall(
                libc::SYS_fsconfig,
                context,
                CREATE,
                std::ptr::null::<libc::c_char>(),
                std::ptr::null::<libc::c_char>(),
                0 as libc::c_int,
            )
        }) != 0
        {
            return Err(Error::Io);
        }
        let mounted = operation!(Mount, control, unsafe {
            libc::syscall(
                libc::SYS_fsmount,
                context,
                CLOEXEC,
                (NOSUID | NODEV) as libc::c_uint,
            )
        });
        if mounted < 0 {
            Err(Error::Io)
        } else {
            Ok(mounted as RawFd)
        }
    })();
    close_owned_checked(
        context,
        #[cfg(test)]
        &mut control,
    );
    let root = prepared?;
    let populated = populate(
        root,
        plan,
        #[cfg(test)]
        &mut control,
    );
    match populated {
        Ok(()) => Ok(Root { fd: root }),
        Err(error) => {
            close_owned_checked(
                root,
                #[cfg(test)]
                &mut control,
            );
            Err(error)
        }
    }
}

fn populate(
    root: RawFd,
    plan: &Plan<'_>,
    #[cfg(test)] control: &mut Option<&mut TestControl>,
) -> Result<(), Error> {
    let mut fs = MaybeUninit::<libc::statfs64>::uninit();
    // On the admitted native 64-bit ABIs statfs64 is the kernel statfs layout.
    // Unlike glibc's statfs declaration it exposes f_flags on x86-64 as well.
    if operation!(Inspect, control, unsafe {
        libc::syscall(libc::SYS_fstatfs, root, fs.as_mut_ptr())
    }) != 0
    {
        return Err(Error::Io);
    }
    // SAFETY: the successful syscall initialized the entire result structure.
    let fs = unsafe { fs.assume_init() };
    #[cfg(test)]
    let fs = corrupt_metadata(fs, control.as_deref(), false);
    if fs.f_type != libc::TMPFS_MAGIC
        || fs.f_bsize != plan.page_size() as _
        || fs.f_blocks != plan.block_count() as u64
        || fs.f_files != plan.inode_count() as u64
    {
        return Err(Error::Io);
    }
    for path in plan.directories() {
        if operation!(Directory, control, unsafe {
            libc::syscall(
                libc::SYS_mkdirat,
                root,
                path.as_ptr(),
                0o700 as libc::mode_t,
            )
        }) != 0
            || operation!(DirectoryMode, control, unsafe {
                libc::syscall(
                    libc::SYS_fchmodat,
                    root,
                    path.as_ptr(),
                    0o700 as libc::mode_t,
                )
            }) != 0
        {
            return Err(Error::Io);
        }
    }
    for file in plan.files() {
        let fd = operation!(Open, control, unsafe {
            libc::syscall(
                libc::SYS_openat,
                root,
                file.path.as_ptr(),
                libc::O_WRONLY | libc::O_CREAT | libc::O_EXCL | libc::O_NOFOLLOW | libc::O_CLOEXEC,
                0o600 as libc::mode_t,
            )
        });
        if fd < 0 {
            return Err(Error::Io);
        }
        let fd = fd as RawFd;
        let written = (|| {
            for chunk in file.bytes.chunks(WRITE_CHUNK) {
                let count = operation!(Write, control, unsafe {
                    libc::syscall(libc::SYS_write, fd, chunk.as_ptr(), chunk.len())
                });
                if count != chunk.len() as libc::c_long {
                    return Err(Error::Io);
                }
            }
            let mode: libc::mode_t = if file.executable { 0o500 } else { 0o400 };
            if operation!(FileMode, control, unsafe {
                libc::syscall(libc::SYS_fchmod, fd, mode)
            }) != 0
            {
                return Err(Error::Io);
            }
            Ok(())
        })();
        // No writable description survives into the read-only transition.
        close_owned_checked(
            fd,
            #[cfg(test)]
            control,
        );
        written?;
    }
    let attributes = MountAttr {
        attr_set: RDONLY | NOSUID | NODEV,
        attr_clr: 0,
        propagation: 0,
        userns_fd: 0,
    };
    if operation!(ReadOnly, control, unsafe {
        libc::syscall(
            libc::SYS_mount_setattr,
            root,
            c"".as_ptr(),
            libc::AT_EMPTY_PATH as libc::c_uint,
            &attributes as *const MountAttr,
            std::mem::size_of::<MountAttr>(),
        )
    }) != 0
    {
        return Err(Error::Io);
    }
    let mut fs = MaybeUninit::<libc::statfs64>::uninit();
    if operation!(Verify, control, unsafe {
        libc::syscall(libc::SYS_fstatfs, root, fs.as_mut_ptr())
    }) != 0
    {
        return Err(Error::Io);
    }
    // SAFETY: successful syscall initialized the result.
    let fs = unsafe { fs.assume_init() };
    #[cfg(test)]
    let fs = corrupt_metadata(fs, control.as_deref(), true);
    if fs.f_type != libc::TMPFS_MAGIC
        || fs.f_bsize != plan.page_size() as _
        || fs.f_blocks != plan.block_count() as u64
        || fs.f_files != plan.inode_count() as u64
        // ST_RDONLY | ST_NOSUID | ST_NODEV; infer the libc field's signedness.
        || fs.f_flags & 7 != 7
    {
        return Err(Error::Io);
    }
    Ok(())
}

fn close_owned(fd: RawFd) {
    close_owned_checked(
        fd,
        #[cfg(test)]
        &mut None,
    );
}

fn close_owned_checked(fd: RawFd, #[cfg(test)] control: &mut Option<&mut TestControl>) {
    // SAFETY: only freshly created, exclusively owned tmpfs/context descriptors
    // reach here. Each is closed exactly once; EINTR is not retried.
    let result = unsafe { libc::close(fd) };
    // Simulated uncertain return after the real one-shot close, not a claim of
    // kernel failure. It feeds the production fail-stop decision below.
    #[cfg(test)]
    let result = if control
        .as_deref()
        .is_some_and(|control| control.close_failure)
    {
        -1
    } else {
        result
    };
    if result != 0 {
        // SAFETY: uncertain ownership is fail-stop in the controlled child.
        unsafe { libc::_exit(126) }
    }
}

#[cfg(test)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Step {
    FsOpen,
    Configure,
    Create,
    Mount,
    Inspect,
    Directory,
    DirectoryMode,
    Open,
    Write,
    FileMode,
    ReadOnly,
    Verify,
}

#[cfg(test)]
#[derive(Default)]
struct TestControl {
    calls: [usize; 12],
    fault: Option<(Step, usize)>,
    write_result: Option<libc::c_long>,
    opened: [Option<RawFd>; 3],
    corrupt: Option<(bool, Corrupt)>,
    close_failure: bool,
}

#[cfg(test)]
#[derive(Clone, Copy)]
enum Corrupt {
    Type,
    PageSize,
    Blocks,
    Inodes,
    Flags,
}

#[cfg(test)]
fn corrupt_metadata(
    mut fs: libc::statfs64,
    control: Option<&TestControl>,
    final_check: bool,
) -> libc::statfs64 {
    if let Some((at_final, fault)) = control.and_then(|control| control.corrupt) {
        if at_final == final_check {
            match fault {
                Corrupt::Type => fs.f_type = 0,
                Corrupt::PageSize => fs.f_bsize = 0,
                Corrupt::Blocks => fs.f_blocks = 0,
                Corrupt::Inodes => fs.f_files = 0,
                Corrupt::Flags => fs.f_flags = 0,
            }
        }
    }
    fs
}

// The test-only path has fixed stack storage: no TLS, Vec or post-fork logging.
macro_rules! operation {
    ($step:ident, $control:ident, $body:expr) => {{
        #[cfg(test)]
        let inject = if let Some(control) = $control.as_deref_mut() {
            let step = Step::$step;
            control.calls[step as usize] += 1;
            if control.fault == Some((step, control.calls[step as usize])) {
                Some(if step == Step::Write {
                    control.write_result.unwrap_or(-1)
                } else {
                    -1
                })
            } else {
                None
            }
        } else {
            None
        };
        #[cfg(test)]
        let result: libc::c_long = match inject {
            Some(result) => result,
            None => $body,
        };
        #[cfg(not(test))]
        let result: libc::c_long = $body;
        #[cfg(test)]
        if result >= 0 {
            if let Some(control) = $control.as_deref_mut() {
                let slot = match Step::$step {
                    Step::FsOpen => Some(0),
                    Step::Mount => Some(1),
                    Step::Open => Some(2),
                    _ => None,
                };
                if let Some(slot) = slot {
                    control.opened[slot] = Some(result as RawFd);
                }
            }
        }
        result
    }};
}
use operation;

#[cfg(test)]
#[path = "linux/tests.rs"]
mod tests;
