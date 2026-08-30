//! CLOEXEC_DEFAULT is independent of the current soft descriptor limit and
//! includes descriptors opened concurrently by other parent threads.
use super::{above_stdio, Fd, Launch, ProbeError};

unsafe extern "C" {
    fn posix_spawn_file_actions_addfchdir_np(
        actions: *mut libc::posix_spawn_file_actions_t,
        fd: libc::c_int,
    ) -> libc::c_int;
}

// The bool carries post-spawn setup-close uncertainty to the caller. It must
// establish its Group guard before settling and fail-stopping on that flag.
pub(super) fn spawn(
    launch: &Launch,
    stdout: &Fd,
    stderr: &Fd,
    null: &Fd,
) -> Result<(libc::pid_t, bool), ProbeError> {
    let argv = [
        launch.path.as_ptr().cast_mut(),
        c"--version".as_ptr().cast_mut(),
        std::ptr::null_mut(),
    ];
    let mut environment = launch
        .environment
        .iter()
        .map(|value| value.as_ptr().cast_mut())
        .collect::<Vec<_>>();
    environment.push(std::ptr::null_mut());
    let raw_cwd = unsafe {
        libc::open(
            launch.cwd.as_ptr(),
            libc::O_RDONLY | libc::O_DIRECTORY | libc::O_CLOEXEC,
        )
    };
    if raw_cwd < 0 {
        return Err(ProbeError::Spawn);
    }
    let cwd = above_stdio(Fd(Some(raw_cwd)))?;
    let mut actions = std::ptr::null_mut();
    let mut attributes = std::ptr::null_mut();
    let mut pid = 0;
    let flags = (libc::POSIX_SPAWN_CLOEXEC_DEFAULT | libc::POSIX_SPAWN_SETPGROUP) as libc::c_short;
    // No Rust allocation/unwind occurs after successful spawn and before the
    // pid is handed to the owning Group. Cleanup failures are data until then.
    let (result, cleanup_failed) = unsafe {
        let actions_initialized = libc::posix_spawn_file_actions_init(&mut actions) == 0;
        let attributes_initialized = libc::posix_spawnattr_init(&mut attributes) == 0;
        let configured = actions_initialized
            && attributes_initialized
            && libc::posix_spawnattr_setflags(&mut attributes, flags) == 0
            && libc::posix_spawnattr_setpgroup(&mut attributes, 0) == 0
            && posix_spawn_file_actions_addfchdir_np(&mut actions, cwd.raw()) == 0
            && libc::posix_spawn_file_actions_adddup2(&mut actions, null.raw(), 0) == 0
            && libc::posix_spawn_file_actions_adddup2(&mut actions, stdout.raw(), 1) == 0
            && libc::posix_spawn_file_actions_adddup2(&mut actions, stderr.raw(), 2) == 0
            && libc::posix_spawn_file_actions_addclose(&mut actions, cwd.raw()) == 0
            && libc::posix_spawn_file_actions_addclose(&mut actions, null.raw()) == 0
            && libc::posix_spawn_file_actions_addclose(&mut actions, stdout.raw()) == 0
            && libc::posix_spawn_file_actions_addclose(&mut actions, stderr.raw()) == 0;
        let result = if configured {
            libc::posix_spawn(
                &mut pid,
                launch.path.as_ptr(),
                &actions,
                &attributes,
                argv.as_ptr(),
                environment.as_ptr(),
            )
        } else {
            libc::EINVAL
        };
        let actions_failed =
            actions_initialized && libc::posix_spawn_file_actions_destroy(&mut actions) != 0;
        let attributes_failed =
            attributes_initialized && libc::posix_spawnattr_destroy(&mut attributes) != 0;
        (result, actions_failed || attributes_failed)
    };
    let cleanup_failed = cwd.close().is_err() || cleanup_failed;
    if result != 0 {
        if cleanup_failed {
            std::process::abort();
        }
        return Err(ProbeError::Spawn);
    }
    Ok((pid, cleanup_failed))
}
