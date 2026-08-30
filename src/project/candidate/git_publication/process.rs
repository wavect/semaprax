//! Explicit Unix host adapter; never instantiated by an agent request or capsule.
use super::*;
use std::path::{Path, PathBuf};

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

#[cfg(unix)]
mod unix {
    use super::*;
    use std::fs::{File, Metadata};
    use std::io::{Read, Write};
    use std::os::unix::fs::{MetadataExt, PermissionsExt};
    use std::os::unix::process::CommandExt;
    use std::process::{Command, Stdio};
    use std::sync::mpsc;
    pub(super) struct Host {
        pub(super) repo: PathBuf,
        pub(super) format: GitObjectFormat,
        executable: PathBuf,
        repo_file: File,
        executable_file: File,
        executable_meta: Metadata,
        config_file: File,
        config: Vec<u8>,
        _lock: File,
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
            let lock = open_file(&repo.join(".semaprax-git-publication.lock"), false, true)
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
            let config_file = open_file(&repo.join("config"), false, false)
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
                config_file,
                config,
                _lock: lock,
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
            same_file(&self.repo, &self.repo_file, true)?;
            same_file(&self.executable, &self.executable_file, false)?;
            let executable_meta = self.executable_file.metadata()?;
            if executable_meta.len() != self.executable_meta.len()
                || executable_meta.mtime() != self.executable_meta.mtime()
                || executable_meta.mtime_nsec() != self.executable_meta.mtime_nsec()
                || executable_meta.ctime() != self.executable_meta.ctime()
                || executable_meta.ctime_nsec() != self.executable_meta.ctime_nsec()
            {
                return Err(io::Error::other("trusted Git executable changed"));
            }
            validate_storage(&self.repo, self.deadline)?;
            same_file(&self.repo.join("config"), &self.config_file, false)?;
            same_file(
                &self.repo.join(".semaprax-git-publication.lock"),
                &self._lock,
                false,
            )?;
            let config = open_file(&self.repo.join("config"), false, false)?;
            if read_bounded(&config, 65_536)? != self.config {
                return Err(io::Error::other("Git config changed"));
            }
            for path in [
                "commondir",
                "worktrees",
                "shallow",
                "info/grafts",
                "objects/info/alternates",
                "objects/info/http-alternates",
            ] {
                match std::fs::symlink_metadata(self.repo.join(path)) {
                    Err(e) if e.kind() == io::ErrorKind::NotFound => (),
                    _ => return Err(io::Error::other("Git repository indirection is forbidden")),
                }
            }
            // Fixed Git object/ref directories must not redirect the host boundary.
            for path in ["objects", "refs"] {
                let file = open_file(&self.repo.join(path), true, false)?;
                if !file.metadata()?.is_dir() {
                    return Err(io::Error::other("invalid Git storage directory"));
                }
            }
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
            self.recheck()?;
            if self.remaining == 0
                || Instant::now() >= self.deadline
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
            let mut command = Command::new(&self.executable);
            command
                .current_dir(&self.repo)
                .arg(format!("--git-dir={}", self.repo.display()))
                .arg("--no-replace-objects")
                .args([
                    "-c",
                    "core.hooksPath=/dev/null",
                    "-c",
                    "core.fsmonitor=false",
                    "-c",
                    "protocol.allow=never",
                    "-c",
                    "core.commitGraph=false",
                ])
                .args(args)
                .env_clear()
                .env("GIT_CONFIG_NOSYSTEM", "1")
                .env("GIT_CONFIG_SYSTEM", "/dev/null")
                .env("GIT_CONFIG_GLOBAL", "/dev/null")
                .env("GIT_NO_REPLACE_OBJECTS", "1")
                .env("GIT_NO_LAZY_FETCH", "1")
                .env("GIT_OPTIONAL_LOCKS", "0")
                .env("GIT_TERMINAL_PROMPT", "0")
                .env("GIT_ATTR_NOSYSTEM", "1")
                .env("LC_ALL", "C")
                .stdin(Stdio::piped())
                .stdout(Stdio::piped())
                .stderr(Stdio::piped())
                .process_group(0);
            let mut child = command.spawn()?;
            let pid = rustix::process::Pid::from_raw(child.id() as i32);
            let mut stdin = child
                .stdin
                .take()
                .ok_or_else(|| io::Error::other("missing Git stdin"))?;
            let stdout = child
                .stdout
                .take()
                .ok_or_else(|| io::Error::other("missing Git stdout"))?;
            let stderr = child
                .stderr
                .take()
                .ok_or_else(|| io::Error::other("missing Git stderr"))?;
            let (tx, rx) = mpsc::channel();
            let input = input.to_vec();
            let outtx = tx.clone();
            std::thread::spawn(move || {
                let _ = outtx.send((0, read_pipe(stdout, limit)));
            });
            let errtx = tx.clone();
            std::thread::spawn(move || {
                let _ = errtx.send((1, read_pipe(stderr, 65_536)));
            });
            std::thread::spawn(move || {
                let value = stdin.write_all(&input).map(|_| Vec::new());
                drop(stdin);
                let _ = tx.send((2, value));
            });
            let mut output = None;
            let mut received = 0;
            let mut status = None;
            let result = loop {
                if Instant::now() >= self.deadline {
                    break Err(io::Error::other("Git host deadline exceeded"));
                }
                match child.try_wait() {
                    Ok(Some(s)) => status = Some(s),
                    Ok(None) => (),
                    Err(e) => break Err(e),
                }
                match rx.recv_timeout(Duration::from_millis(2)) {
                    Ok((kind, Ok(bytes))) => {
                        received += 1;
                        if kind == 0 {
                            output = Some(bytes);
                        }
                    }
                    Ok((_, Err(e))) => break Err(e),
                    Err(mpsc::RecvTimeoutError::Timeout) => (),
                    Err(mpsc::RecvTimeoutError::Disconnected) if received < 3 => {
                        break Err(io::Error::other("Git pipe worker failed"))
                    }
                    Err(_) => (),
                }
                if received == 3 {
                    if let Some(status) = status {
                        break Ok((
                            status.code().unwrap_or(-1),
                            output.take().unwrap_or_default(),
                        ));
                    }
                }
            };
            if result.is_err() {
                if let Some(pid) = pid {
                    let _ = rustix::process::kill_process_group(pid, rustix::process::Signal::KILL);
                }
                let _ = child.kill();
                let _ = child.try_wait();
            }
            self.recheck()?;
            result
        }
    }
    // The repository is host-controlled; this also rejects preexisting nested
    // redirects. The permanent lease cannot stop a malicious same-UID mutator.
    fn validate_storage(root: &Path, deadline: Instant) -> io::Result<()> {
        let mut pending = vec![(root.to_path_buf(), 0usize)];
        let mut entries = 0usize;
        while let Some((directory, depth)) = pending.pop() {
            if depth > 64 || Instant::now() >= deadline {
                return Err(io::Error::other("Git storage traversal bound exceeded"));
            }
            for entry in std::fs::read_dir(directory)? {
                let entry = entry?;
                entries += 1;
                if entries > 65_536 {
                    return Err(io::Error::other("Git storage entry bound exceeded"));
                }
                let metadata = std::fs::symlink_metadata(entry.path())?;
                if metadata.is_dir() {
                    pending.push((entry.path(), depth + 1));
                } else if !metadata.is_file() || metadata.nlink() != 1 {
                    return Err(io::Error::other(
                        "Git storage redirects, special files, or hardlinks are forbidden",
                    ));
                }
            }
        }
        Ok(())
    }
    fn read_pipe(reader: impl Read, limit: usize) -> io::Result<Vec<u8>> {
        let mut result = Vec::new();
        reader.take(limit as u64 + 1).read_to_end(&mut result)?;
        if result.len() > limit {
            Err(io::Error::other("Git pipe exceeded byte bound"))
        } else {
            Ok(result)
        }
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
}
