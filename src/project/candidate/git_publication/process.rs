//! Explicit Unix host adapter; never instantiated by an agent request or capsule.
use super::*;
use std::path::Path;

pub struct CandidateGitProcessAuthority {
    #[cfg(unix)]
    inner: unix::Host,
    identity: String,
}
impl CandidateGitProcessAuthority {
    pub fn open(
        executable: &Path,
        repository: &Path,
        max_commands: usize,
        timeout_ms: u64,
    ) -> Result<Self> {
        #[cfg(unix)]
        {
            let inner = unix::Host::open(executable, repository, max_commands, timeout_ms)?;
            let identity = inner
                .repo
                .to_str()
                .ok_or_else(|| invalid("Git repository must have a UTF8 absolute path"))?
                .to_owned();
            Ok(Self { inner, identity })
        }
        #[cfg(not(unix))]
        {
            let _ = (executable, repository, max_commands, timeout_ms);
            Err(invalid("Git process publication is supported only on Unix"))
        }
    }
    pub fn object_format(&self) -> GitObjectFormat {
        #[cfg(unix)]
        {
            self.inner.format
        }
        #[cfg(not(unix))]
        {
            GitObjectFormat::Sha256
        }
    }
    pub fn repository_identity(&self) -> &str {
        &self.identity
    }
}
impl CandidateGitAuthority for CandidateGitProcessAuthority {
    fn repository(&self) -> io::Result<CandidateGitRepository> {
        #[cfg(unix)]
        self.inner.recheck()?;
        #[cfg(not(unix))]
        return Err(io::Error::other("unsupported Git publication host"));
        #[allow(unreachable_code)]
        Ok(CandidateGitRepository {
            identity: self.identity.clone(),
            bare: true,
            sha256: self.object_format() == GitObjectFormat::Sha256,
        })
    }
    fn read_ref(&mut self, reference: &str) -> io::Result<Option<String>> {
        #[cfg(unix)]
        {
            self.inner.read_ref(reference)
        }
        #[cfg(not(unix))]
        {
            let _ = reference;
            Err(io::Error::other("unsupported Git publication host"))
        }
    }
    fn read_object(&mut self, oid: &str, max_bytes: usize) -> io::Result<CandidateGitObject> {
        #[cfg(unix)]
        {
            checked_oid(oid, self.inner.format)?;
            let kind = self.inner.success(&["cat-file", "-t", oid], &[], 32)?;
            let kind = match kind.as_slice() {
                b"blob\n" => CandidateGitObjectKind::Blob,
                b"tree\n" => CandidateGitObjectKind::Tree,
                b"commit\n" => CandidateGitObjectKind::Commit,
                _ => return Err(io::Error::other("unsupported Git object type")),
            };
            let size = self.inner.success(&["cat-file", "-s", oid], &[], 32)?;
            let size = std::str::from_utf8(&size)
                .ok()
                .and_then(|s| s.strip_suffix('\n'))
                .and_then(|s| s.parse::<usize>().ok())
                .ok_or_else(|| io::Error::other("invalid Git object size"))?;
            if size > max_bytes || size > MAX_OBJECT {
                return Err(io::Error::other("Git object exceeds host bound"));
            }
            let bytes = self
                .inner
                .success(&["cat-file", kind.name(), oid], &[], size)?;
            if bytes.len() != size {
                return Err(io::Error::other("Git object size changed"));
            }
            Ok(CandidateGitObject { kind, bytes })
        }
        #[cfg(not(unix))]
        {
            let _ = (oid, max_bytes);
            Err(io::Error::other("unsupported Git publication host"))
        }
    }
    fn write_object(
        &mut self,
        kind: CandidateGitObjectKind,
        bytes: &[u8],
        expected_oid: &str,
    ) -> io::Result<()> {
        #[cfg(unix)]
        {
            checked_oid(expected_oid, self.inner.format)?;
            if bytes.len() > MAX_OBJECT {
                return Err(io::Error::other("Git object exceeds host byte bound"));
            }
            if object_oid(self.inner.format, kind, bytes) != expected_oid {
                return Err(io::Error::other("invalid expected Git object digest"));
            }
            let result = self.inner.success(
                &[
                    "hash-object",
                    "-w",
                    "--stdin",
                    "--no-filters",
                    "-t",
                    kind.name(),
                ],
                bytes,
                65,
            )?;
            if result != format!("{expected_oid}\n").as_bytes() {
                return Err(io::Error::other("written Git object digest disagrees"));
            }
            Ok(())
        }
        #[cfg(not(unix))]
        {
            let _ = (kind, bytes, expected_oid);
            Err(io::Error::other("unsupported Git publication host"))
        }
    }
    fn compare_and_swap_ref(
        &mut self,
        reference: &str,
        expected_old: &str,
        new_commit: &str,
    ) -> io::Result<CandidateGitRefUpdate> {
        #[cfg(unix)]
        {
            checked_oid(expected_old, self.inner.format)?;
            checked_oid(new_commit, self.inner.format)?;
            if self.inner.read_ref(reference)?.as_deref() != Some(expected_old) {
                return Ok(CandidateGitRefUpdate::NotMatched);
            }
            // Every failure after spawn is uncertain, even if Git exited nonzero.
            self.inner.success(
                &[
                    "update-ref",
                    "--no-deref",
                    reference,
                    new_commit,
                    expected_old,
                ],
                &[],
                0,
            )?;
            Ok(CandidateGitRefUpdate::Updated)
        }
        #[cfg(not(unix))]
        {
            let _ = (reference, expected_old, new_commit);
            Err(io::Error::other("unsupported Git publication host"))
        }
    }
}
#[cfg(unix)]
fn checked_oid(oid: &str, format: GitObjectFormat) -> io::Result<()> {
    if valid_oid(oid).map_err(|_| io::Error::other("invalid Git OID"))? != format {
        return Err(io::Error::other(
            "Git OID differs from configured repository format",
        ));
    }
    Ok(())
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
#[path = "process/platform.rs"]
mod platform;

#[cfg(all(unix, not(any(target_os = "linux", target_os = "macos"))))]
mod platform {
    use std::fs::{File, Metadata};
    use std::io;
    use std::path::Path;
    use std::time::Instant;

    pub(super) const SUPPORTED: bool = false;

    #[derive(Clone, Copy)]
    pub(super) struct Limits {
        pub(super) stdout: usize,
        pub(super) stderr: usize,
        pub(super) deadline: Instant,
    }

    pub(super) fn run(
        _executable_path: &Path,
        _executable: &File,
        _executable_metadata: &Metadata,
        _repository: &File,
        _command: &[&str],
        _input: &[u8],
        limits: Limits,
    ) -> io::Result<(i32, Vec<u8>)> {
        let _ = (limits.stdout, limits.stderr, limits.deadline);
        Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "held Git execution requires Linux or macOS",
        ))
    }
}

#[cfg(unix)]
mod unix {
    use super::*;
    use std::fs::{File, Metadata};
    use std::os::unix::fs::{MetadataExt, PermissionsExt};
    use std::path::PathBuf;

    use super::platform;

    struct Lease(File);
    impl Drop for Lease {
        fn drop(&mut self) {
            // Closing alone can leave flock held by a concurrent fork's copy
            // until exec, despite CLOEXEC. End the lease on its owning scope,
            // including admission errors and unwinding, without deleting it.
            let _ = fs2::FileExt::unlock(&self.0);
        }
    }

    pub(super) struct Host {
        pub(super) repo: PathBuf,
        pub(super) format: GitObjectFormat,
        executable: PathBuf,
        repo_file: File,
        executable_file: File,
        executable_meta: Metadata,
        objects_file: File,
        refs_file: File,
        config_file: File,
        config: Vec<u8>,
        lock: Lease,
        deadline: Instant,
        remaining: usize,
        io_bytes: usize,
    }
    impl Host {
        pub(super) fn open(
            executable: &Path,
            repository: &Path,
            max_commands: usize,
            timeout_ms: u64,
        ) -> Result<Self> {
            if !platform::SUPPORTED {
                return Err(host("held Git execution requires Linux or macOS"));
            }
            if !executable.is_absolute()
                || !repository.is_absolute()
                || !(1..=4096).contains(&max_commands)
                || !(1..=60_000).contains(&timeout_ms)
            {
                return Err(invalid(
                    "Git host requires absolute paths and bounded command/deadline policy",
                ));
            }
            let executable = executable
                .canonicalize()
                .map_err(|_| host("cannot resolve trusted Git executable"))?;
            let repo = repository
                .canonicalize()
                .map_err(|_| host("cannot resolve bare Git repository"))?;
            if repo != repository {
                return Err(invalid("Git repository path must already be canonical"));
            }
            let executable_file = open_file(&executable, false, false)
                .map_err(|_| host("cannot hold trusted Git executable"))?;
            let meta = executable_file
                .metadata()
                .map_err(|_| host("cannot inspect Git executable"))?;
            if !meta.is_file() || meta.permissions().mode() & 0o111 == 0 {
                return Err(invalid(
                    "Git executable must resolve to an executable regular file",
                ));
            }
            let repo_file = open_file(&repo, true, false)
                .map_err(|_| host("cannot hold bare repository directory"))?;
            let objects_file = open_file_at(&repo_file, c"objects", true, false)
                .map_err(|_| host("cannot hold Git objects directory"))?;
            let refs_file = open_file_at(&repo_file, c"refs", true, false)
                .map_err(|_| host("cannot hold Git refs directory"))?;
            let lock = open_file_at(&repo_file, c".semaprax-git-publication.lock", false, true)
                .map_err(|_| host("cannot open Git publication lock"))?;
            if !lock
                .metadata()
                .map_err(|_| host("cannot inspect Git publication lock"))?
                .is_file()
            {
                return Err(invalid("Git publication lock must be regular"));
            }
            fs2::FileExt::try_lock_exclusive(&lock)
                .map_err(|_| host("Git publication host is already leased"))?;
            let lock = Lease(lock);
            let config_file = open_file_at(&repo_file, c"config", false, false)
                .map_err(|_| host("cannot hold Git config"))?;
            let config = read_bounded(&config_file, 65_536)
                .map_err(|_| host("cannot read bounded Git config"))?;
            let format = validate_config(&config)?;
            let value = Self {
                repo,
                format,
                executable,
                repo_file,
                executable_file,
                executable_meta: meta,
                objects_file,
                refs_file,
                config_file,
                config,
                lock,
                deadline: Instant::now() + Duration::from_millis(timeout_ms),
                remaining: max_commands,
                io_bytes: 0,
            };
            value.recheck().map_err(|_| {
                invalid("Git repository has forbidden indirection or held-input drift")
            })?;
            Ok(value)
        }
        pub(super) fn recheck(&self) -> io::Result<()> {
            let executable_meta = self.executable_file.metadata()?;
            if executable_meta.dev() != self.executable_meta.dev()
                || executable_meta.ino() != self.executable_meta.ino()
                || executable_meta.len() != self.executable_meta.len()
                || executable_meta.mode() != self.executable_meta.mode()
                || executable_meta.mtime() != self.executable_meta.mtime()
                || executable_meta.mtime_nsec() != self.executable_meta.mtime_nsec()
            {
                return Err(io::Error::other("trusted Git executable changed"));
            }
            validate_storage(&self.repo_file, self.deadline)?;
            let config = open_file_at(&self.repo_file, c"config", false, false)?;
            same_file_handles(&config, &self.config_file, false)?;
            if read_bounded(&config, 65_536)? != self.config {
                return Err(io::Error::other("Git config changed"));
            }
            let lock = open_file_at(
                &self.repo_file,
                c".semaprax-git-publication.lock",
                false,
                false,
            )?;
            same_file_handles(&lock, &self.lock.0, false)?;
            for name in [c"commondir", c"worktrees", c"shallow"] {
                require_absent_at(&self.repo_file, name)?;
            }
            let info = open_file_at(&self.repo_file, c"info", true, false).ok();
            if let Some(info) = info.as_ref() {
                require_absent_at(info, c"grafts")?;
            }
            require_absent_nested(&self.objects_file, c"info", c"alternates")?;
            require_absent_nested(&self.objects_file, c"info", c"http-alternates")?;
            let objects = open_file_at(&self.repo_file, c"objects", true, false)?;
            let refs = open_file_at(&self.repo_file, c"refs", true, false)?;
            same_file_handles(&objects, &self.objects_file, true)?;
            same_file_handles(&refs, &self.refs_file, true)?;
            Ok(())
        }
        pub(super) fn read_ref(&mut self, reference: &str) -> io::Result<Option<String>> {
            valid_ref(reference).map_err(|_| io::Error::other("invalid Git branch ref"))?;
            let (code, _) = self.run(&["symbolic-ref", "--quiet", reference], &[], 4096)?;
            if code != 1 {
                return Err(io::Error::other(
                    "Git publication branch must be a direct ref",
                ));
            }
            let (code, output) =
                self.run(&["show-ref", "--verify", "--hash", reference], &[], 65)?;
            if code != 0 {
                return if matches!(code, 1 | 128) && output.is_empty() {
                    Ok(None)
                } else {
                    Err(io::Error::other("cannot read Git branch ref"))
                };
            }
            let oid = std::str::from_utf8(&output)
                .ok()
                .and_then(|s| s.strip_suffix('\n'))
                .ok_or_else(|| io::Error::other("invalid Git ref output"))?;
            checked_oid(oid, self.format)?;
            Ok(Some(oid.to_owned()))
        }
        pub(super) fn success(
            &mut self,
            args: &[&str],
            input: &[u8],
            limit: usize,
        ) -> io::Result<Vec<u8>> {
            let (code, output) = self.run(args, input, limit)?;
            if code != 0 {
                return Err(io::Error::other("host Git command failed"));
            }
            Ok(output)
        }
        fn run(&mut self, args: &[&str], input: &[u8], limit: usize) -> io::Result<(i32, Vec<u8>)> {
            if Instant::now() >= self.deadline {
                return Err(io::Error::other("Git host deadline exceeded"));
            }
            self.recheck()?;
            if self.remaining == 0
                || input.len() > MAX_OBJECT
                || limit > MAX_OBJECT
                || self
                    .io_bytes
                    .saturating_add(input.len())
                    .saturating_add(limit)
                    .saturating_add(65_536)
                    > 512 * 1024 * 1024
            {
                return Err(io::Error::other("Git host execution budget exhausted"));
            }
            self.remaining -= 1;
            self.io_bytes += input.len() + limit + 65_536;
            let result = platform::run(
                &self.executable,
                &self.executable_file,
                &self.executable_meta,
                &self.repo_file,
                args,
                input,
                platform::Limits {
                    stdout: limit,
                    stderr: 65_536,
                    deadline: self.deadline,
                },
            );
            match result {
                Ok(value) => {
                    self.recheck()?;
                    Ok(value)
                }
                Err(primary) => {
                    let _ = self.recheck();
                    Err(primary)
                }
            }
        }
    }
    // The repository is host-controlled; this also rejects preexisting nested
    // redirects. The permanent lease cannot stop a malicious same-UID mutator.
    fn validate_storage(root: &File, deadline: Instant) -> io::Result<()> {
        let mut pending = vec![(root.try_clone()?, 0usize)];
        let mut entries = 0usize;
        while let Some((directory, depth)) = pending.pop() {
            if depth > 64 {
                return Err(io::Error::other("Git storage traversal bound exceeded"));
            }
            if Instant::now() >= deadline {
                return Err(io::Error::other("Git host deadline exceeded"));
            }
            for entry in rustix::fs::Dir::read_from(&directory).map_err(io::Error::from)? {
                let entry = entry.map_err(io::Error::from)?;
                let name = entry.file_name();
                if name.to_bytes() == b"." || name.to_bytes() == b".." {
                    continue;
                }
                entries += 1;
                if entries > 65_536 {
                    return Err(io::Error::other("Git storage entry bound exceeded"));
                }
                if let Ok(child) = open_file_at(&directory, name, true, false) {
                    pending.push((child, depth + 1));
                    continue;
                }
                let child = open_file_at(&directory, name, false, false)?;
                let metadata = child.metadata()?;
                if !metadata.is_file() || metadata.nlink() != 1 {
                    return Err(io::Error::other(
                        "Git storage redirects, special files, or hardlinks are forbidden",
                    ));
                }
            }
        }
        Ok(())
    }
    fn open_file(path: &Path, directory: bool, create: bool) -> io::Result<File> {
        use rustix::fs::{Mode, OFlags};
        let flags = if create {
            OFlags::RDWR | OFlags::CREATE
        } else {
            OFlags::RDONLY
        };
        let flags = flags
            | OFlags::NOFOLLOW
            | OFlags::CLOEXEC
            | OFlags::NONBLOCK
            | if directory {
                OFlags::DIRECTORY
            } else {
                OFlags::empty()
            };
        rustix::fs::open(path, flags, Mode::from_bits_truncate(0o600))
            .map(File::from)
            .map_err(io::Error::from)
    }
    fn open_file_at(
        parent: &File,
        name: &std::ffi::CStr,
        directory: bool,
        create: bool,
    ) -> io::Result<File> {
        use rustix::fs::{Mode, OFlags};
        let flags = if create {
            OFlags::RDWR | OFlags::CREATE
        } else {
            OFlags::RDONLY
        };
        let flags = flags
            | OFlags::NOFOLLOW
            | OFlags::CLOEXEC
            | OFlags::NONBLOCK
            | if directory {
                OFlags::DIRECTORY
            } else {
                OFlags::empty()
            };
        rustix::fs::openat(parent, name, flags, Mode::from_bits_truncate(0o600))
            .map(File::from)
            .map_err(io::Error::from)
    }
    fn same_file_handles(current: &File, held: &File, directory: bool) -> io::Result<()> {
        let current = current.metadata()?;
        let held = held.metadata()?;
        if current.dev() != held.dev()
            || current.ino() != held.ino()
            || (directory && !current.is_dir())
            || (!directory && !current.is_file())
        {
            Err(io::Error::other("held Git host input changed"))
        } else {
            Ok(())
        }
    }
    fn require_absent_at(parent: &File, name: &std::ffi::CStr) -> io::Result<()> {
        match open_file_at(parent, name, false, false) {
            Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
            _ => Err(io::Error::other("Git repository indirection is forbidden")),
        }
    }
    fn require_absent_nested(
        parent: &File,
        directory: &std::ffi::CStr,
        name: &std::ffi::CStr,
    ) -> io::Result<()> {
        match open_file_at(parent, directory, true, false) {
            Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
            Err(_) => Err(io::Error::other("Git repository indirection is forbidden")),
            Ok(directory) => require_absent_at(&directory, name),
        }
    }
    #[cfg(all(test, any(target_os = "linux", target_os = "macos")))]
    fn same_file(path: &Path, file: &File, directory: bool) -> io::Result<()> {
        let current = std::fs::symlink_metadata(path)?;
        let held = file.metadata()?;
        if current.file_type().is_symlink()
            || current.dev() != held.dev()
            || current.ino() != held.ino()
            || (directory && !current.is_dir())
            || (!directory && !current.is_file())
        {
            Err(io::Error::other("held Git host input changed"))
        } else {
            Ok(())
        }
    }
    fn read_bounded(file: &File, limit: usize) -> io::Result<Vec<u8>> {
        use std::os::unix::fs::FileExt;
        let mut bytes = vec![0; limit + 1];
        let mut size = 0;
        loop {
            let n = file.read_at(&mut bytes[size..], size as u64)?;
            if n == 0 {
                break;
            }
            size += n;
            if size > limit {
                return Err(io::Error::other("Git config exceeds byte bound"));
            }
        }
        bytes.truncate(size);
        Ok(bytes)
    }
    fn validate_config(bytes: &[u8]) -> Result<GitObjectFormat> {
        let text = std::str::from_utf8(bytes).map_err(|_| invalid("Git config must be UTF8"))?;
        let mut section = "";
        let mut values = BTreeMap::new();
        for raw in text.lines() {
            let line = raw.trim();
            if line.is_empty() || line.starts_with('#') || line.starts_with(';') {
                continue;
            }
            if line.starts_with('[') {
                section =
                    match line {
                        "[core]" => "core",
                        "[extensions]" => "extensions",
                        _ => return Err(invalid(
                            "Git publication admits only minimal core/extensions repository config",
                        )),
                    };
                continue;
            }
            let (key, value) = line.split_once('=').ok_or_else(|| {
                invalid("Git config requires explicit simple key/value assignments")
            })?;
            let key = key.trim().to_ascii_lowercase();
            let value = value.trim();
            let allowed = match (section, key.as_str()) {
                ("core", "repositoryformatversion") => matches!(value, "0" | "1"),
                ("core", "bare") => value == "true",
                ("core", "filemode" | "logallrefupdates") => matches!(value, "true" | "false"),
                ("extensions", "objectformat") => matches!(value, "sha1" | "sha256"),
                _ => false,
            };
            if !allowed || values.insert((section, key), value).is_some() {
                return Err(invalid(
                    "Git config contains unsupported, duplicate, or unsafe settings",
                ));
            }
        }
        let get = |section: &str, key: &str| values.get(&(section, key.to_owned())).copied();
        if get("core", "bare") != Some("true") {
            return Err(invalid("Git publication requires a bare repository"));
        }
        match (
            get("core", "repositoryformatversion"),
            get("extensions", "objectformat"),
        ) {
            (Some("0"), None) | (Some("1"), None | Some("sha1")) => Ok(GitObjectFormat::Sha1),
            (Some("1"), Some("sha256")) => Ok(GitObjectFormat::Sha256),
            _ => Err(invalid("Git repository version and object format disagree")),
        }
    }

    #[cfg(all(test, any(target_os = "linux", target_os = "macos")))]
    mod tests {
        use super::*;
        use std::sync::atomic::{AtomicU64, Ordering};

        static SERIAL: AtomicU64 = AtomicU64::new(0);

        struct Repository(PathBuf);
        impl Repository {
            fn new() -> Self {
                let path = std::env::temp_dir().join(format!(
                    "spx-git-lease-{}-{}",
                    std::process::id(),
                    SERIAL.fetch_add(1, Ordering::Relaxed)
                ));
                std::fs::create_dir(&path).unwrap();
                std::fs::create_dir(path.join("objects")).unwrap();
                std::fs::create_dir(path.join("refs")).unwrap();
                std::fs::write(
                    path.join("config"),
                    b"[core]\nrepositoryformatversion = 0\nbare = true\n",
                )
                .unwrap();
                Self(path.canonicalize().unwrap())
            }

            fn open(&self) -> Result<Host> {
                // Admission only: no command is run by these lease tests.
                let executable = std::env::current_exe().unwrap().canonicalize().unwrap();
                Host::open(&executable, &self.0, 1, 60_000)
            }

            fn assert_leased(&self) {
                let errors = self.open().err().expect("a live host must exclude rivals");
                assert_eq!(errors.len(), 1);
                assert_eq!(errors[0].code, "SPX-G266");
                assert_eq!(errors[0].message, "Git publication host is already leased");
            }
        }
        impl Drop for Repository {
            fn drop(&mut self) {
                // Best effort; see the note on ProcessFixture's destructor.
                let _ = std::fs::remove_dir_all(&self.0);
            }
        }

        #[test]
        fn dropping_git_host_releases_lease_with_an_inherited_descriptor_alive() {
            let repository = Repository::new();
            let host = repository.open().unwrap();
            // dup and fork share the same open-file description. Holding a dup
            // makes the pre-exec child lifetime deterministic without a race.
            let inherited = host.lock.0.try_clone().unwrap();
            repository.assert_leased();
            drop(host);

            let next = repository.open().expect("drop must release the host lease");
            same_file(
                &repository.0.join(".semaprax-git-publication.lock"),
                &inherited,
                false,
            )
            .unwrap();
            repository.assert_leased();
            drop(inherited);
            repository.assert_leased();
            drop(next);
            let final_host = repository.open().unwrap();
            final_host.recheck().unwrap();
        }

        #[test]
        fn unwinding_git_host_releases_lease_with_an_inherited_descriptor_alive() {
            let repository = Repository::new();
            let mut inherited = None;
            let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                let host = repository.open().unwrap();
                inherited = Some(host.lock.0.try_clone().unwrap());
                repository.assert_leased();
                panic!("host scope unwinds");
            }));
            assert_eq!(
                result.unwrap_err().downcast_ref::<&str>(),
                Some(&"host scope unwinds")
            );
            let next = repository.open().unwrap();
            repository.assert_leased();
            drop(inherited.unwrap());
            repository.assert_leased();
            next.recheck().unwrap();
        }

        #[test]
        fn expired_host_deadline_is_the_sticky_primary() {
            let repository = Repository::new();
            let executable = std::env::current_exe().unwrap().canonicalize().unwrap();
            let mut host = Host::open(&executable, &repository.0, 1, 20).unwrap();
            let started = Instant::now();
            while started.elapsed() < Duration::from_millis(25) {
                std::hint::spin_loop();
            }
            let error = host
                .read_ref("refs/heads/review")
                .expect_err("expired authority must not start a process");
            assert_eq!(error.to_string(), "Git host deadline exceeded");
        }
    }

    #[cfg(all(test, any(target_os = "linux", target_os = "macos")))]
    mod process_tests {
        include!("process/tests.rs");
    }
}
