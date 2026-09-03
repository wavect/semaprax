//! Allocation-free child setup before the held launcher image.
use super::{close, Prepared};

pub(super) fn enter(prepared: &Prepared) -> ! {
    if unsafe { libc::prctl(libc::PR_SET_PDEATHSIG, libc::SIGKILL, 0, 0, 0) } != 0
        || unsafe { libc::getppid() } != prepared.parent
    {
        stop();
    }
    let mut ready = 0u8;
    if unsafe { libc::read(prepared.ready[0], (&mut ready as *mut u8).cast(), 1) } != 1
        || ready != b'1'
    {
        stop();
    }
    if unsafe {
        libc::mount(
            std::ptr::null(),
            c"/".as_ptr(),
            std::ptr::null(),
            libc::MS_REC | libc::MS_PRIVATE,
            std::ptr::null(),
        )
    } != 0
        || !enter_empty_root()
        || unsafe { libc::sethostname(c"semaprax-doctor".as_ptr().cast(), 15) } != 0
        || unsafe { libc::prctl(libc::PR_SET_NO_NEW_PRIVS, 1, 0, 0, 0) } != 0
        || !signals()
    {
        stop();
    }
    // Every destination is known-live on entry. Close it explicitly so dup3
    // never hides a destination-close failure.
    for fd in 0..=10 {
        if close(fd).is_err() {
            stop();
        }
    }
    for (source, destination) in [
        (prepared.stdin[0], 0),
        (prepared.report[1], 1),
        (prepared.error[1], 2),
        (prepared.request, 3),
        (prepared.bundle, 4),
        (prepared.worker, 5),
        (prepared.collector, 6),
    ] {
        if unsafe { libc::dup3(source, destination, 0) } != destination {
            stop();
        }
    }
    for fd in prepared.inventory() {
        if fd != prepared.launcher && close(fd).is_err() {
            stop();
        }
    }
    if unsafe { libc::getppid() } != prepared.parent {
        stop();
    }
    let argv = [c"semaprax-doctor-launcher".as_ptr(), std::ptr::null()];
    let environment: [*const libc::c_char; 1] = [std::ptr::null()];
    unsafe {
        libc::syscall(
            libc::SYS_execveat,
            prepared.launcher,
            c"".as_ptr(),
            argv.as_ptr(),
            environment.as_ptr(),
            libc::AT_EMPTY_PATH,
        );
    }
    stop()
}

fn enter_empty_root() -> bool {
    // Mount directly over `/`: creating a staging pathname in the inherited
    // tree would itself exercise ambient host-filesystem authority. After the
    // overmount, `pivot_root(".", ".")` stacks the old root at the new root so
    // one detach removes it without retaining an old-root path or descriptor.
    if unsafe {
        libc::mount(
            c"tmpfs".as_ptr(),
            c"/".as_ptr(),
            c"tmpfs".as_ptr(),
            libc::MS_NOSUID | libc::MS_NODEV | libc::MS_NOEXEC,
            c"size=65536,nr_inodes=1,mode=0500".as_ptr().cast(),
        )
    } != 0
        || unsafe { libc::chdir(c"/".as_ptr()) } != 0
        || unsafe { libc::syscall(libc::SYS_pivot_root, c".".as_ptr(), c".".as_ptr()) } != 0
        || unsafe { libc::umount2(c".".as_ptr(), libc::MNT_DETACH) } != 0
        || unsafe { libc::chdir(c"/".as_ptr()) } != 0
        || unsafe { libc::chroot(c".".as_ptr()) } != 0
        || unsafe {
            libc::mount(
                std::ptr::null(),
                c"/".as_ptr(),
                std::ptr::null(),
                libc::MS_REMOUNT
                    | libc::MS_RDONLY
                    | libc::MS_NOSUID
                    | libc::MS_NODEV
                    | libc::MS_NOEXEC,
                std::ptr::null(),
            )
        } != 0
    {
        return false;
    }
    authenticate_empty_root()
}

fn authenticate_empty_root() -> bool {
    let mut root = std::mem::MaybeUninit::<libc::stat>::zeroed();
    let mut cwd = std::mem::MaybeUninit::<libc::stat>::zeroed();
    // On the admitted native 64-bit ABIs statfs64 is the kernel statfs layout
    // and exposes f_flags consistently on x86-64 and AArch64.
    let mut filesystem = std::mem::MaybeUninit::<libc::statfs64>::zeroed();
    if unsafe { libc::stat(c"/".as_ptr(), root.as_mut_ptr()) } != 0
        || unsafe { libc::stat(c".".as_ptr(), cwd.as_mut_ptr()) } != 0
        || unsafe { libc::syscall(libc::SYS_statfs, c"/".as_ptr(), filesystem.as_mut_ptr()) } != 0
    {
        return false;
    }
    let root = unsafe { root.assume_init() };
    let cwd = unsafe { cwd.assume_init() };
    let filesystem = unsafe { filesystem.assume_init() };
    let capacity = u64::try_from(filesystem.f_bsize)
        .ok()
        .and_then(|block| block.checked_mul(filesystem.f_blocks));
    if root.st_dev != cwd.st_dev
        || root.st_ino != cwd.st_ino
        || filesystem.f_type != libc::TMPFS_MAGIC
        || !matches!(capacity, Some(1..=65_536))
        // Linux ST_RDONLY | ST_NOSUID | ST_NODEV | ST_NOEXEC.
        || filesystem.f_flags & 15 != 15
    {
        return false;
    }
    empty_directory(c"/")
}

fn empty_directory(path: &std::ffi::CStr) -> bool {
    let directory = unsafe {
        libc::open(
            path.as_ptr(),
            libc::O_RDONLY | libc::O_DIRECTORY | libc::O_CLOEXEC | libc::O_NOFOLLOW,
        )
    };
    if directory < 0 {
        return false;
    }
    let mut buffer = [0u8; 512];
    loop {
        let count = unsafe {
            libc::syscall(
                libc::SYS_getdents64,
                directory,
                buffer.as_mut_ptr(),
                buffer.len(),
            )
        };
        if count < 0 {
            let _ = close(directory);
            return false;
        }
        if count == 0 {
            return close(directory).is_ok();
        }
        let Ok(count) = usize::try_from(count) else {
            let _ = close(directory);
            return false;
        };
        let mut cursor = 0usize;
        while cursor < count {
            if count - cursor < 20 {
                let _ = close(directory);
                return false;
            }
            let record = &buffer[cursor..count];
            let length = usize::from(u16::from_ne_bytes([record[16], record[17]]));
            let Some(end) = cursor.checked_add(length).filter(|end| *end <= count) else {
                let _ = close(directory);
                return false;
            };
            if length < 20 {
                let _ = close(directory);
                return false;
            }
            let name = &record[19..length];
            let Some(nul) = name.iter().position(|byte| *byte == 0) else {
                let _ = close(directory);
                return false;
            };
            if &name[..nul] != b"." && &name[..nul] != b".." {
                let _ = close(directory);
                return false;
            }
            cursor = end;
        }
    }
}

fn signals() -> bool {
    let mut action = unsafe { std::mem::zeroed::<libc::sigaction>() };
    action.sa_sigaction = libc::SIG_DFL;
    if unsafe { libc::sigemptyset(&mut action.sa_mask) } != 0 {
        return false;
    }
    for signal in 1..=64 {
        if signal == libc::SIGKILL || signal == libc::SIGSTOP || signal == 32 || signal == 33 {
            continue;
        }
        if unsafe { libc::sigaction(signal, &action, std::ptr::null_mut()) } != 0 {
            return false;
        }
    }
    unsafe { libc::sigprocmask(libc::SIG_SETMASK, &action.sa_mask, std::ptr::null_mut()) == 0 }
}

fn stop() -> ! {
    unsafe { libc::_exit(126) }
}
