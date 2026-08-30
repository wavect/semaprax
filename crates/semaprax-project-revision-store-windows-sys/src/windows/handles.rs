#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Error {
    Invalid,
    Limit,
    Busy,
    Exists,
    Changed,
    Io,
    Uncertain,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Kind {
    Directory,
    File,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Fact {
    volume: u64,
    file_id: [u8; 16],
    attributes: u32,
    links: u32,
    length: u64,
}

impl Fact {
    pub fn length(&self) -> u64 {
        self.length
    }
}

pub struct InventoryEntry {
    name: String,
    kind: Kind,
    fact: Fact,
}

impl InventoryEntry {
    pub fn name(&self) -> &str {
        &self.name
    }
    pub fn kind(&self) -> Kind {
        self.kind
    }
    pub fn fact(&self) -> Fact {
        self.fact
    }
}

struct TokenAuthority {
    token: OwnedHandle,
    sid: Vec<u8>,
    identity: TokenIdentity,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct TokenIdentity {
    token_low: u32,
    token_high: i32,
    authentication_low: u32,
    authentication_high: i32,
    modified_low: u32,
    modified_high: i32,
}

struct SecurityDescriptor {
    words: Vec<u64>,
    bytes: usize,
}

impl SecurityDescriptor {
    fn as_ptr(&self) -> *mut core::ffi::c_void {
        debug_assert!(self.bytes <= self.words.len() * std::mem::size_of::<u64>());
        self.words.as_ptr().cast_mut().cast()
    }

    fn as_mut_ptr(&mut self) -> *mut core::ffi::c_void {
        self.words.as_mut_ptr().cast()
    }
}

struct OwnedHandle(HANDLE);

impl OwnedHandle {
    fn raw(&self) -> HANDLE {
        self.0
    }

    fn close(mut self) -> Result<(), Error> {
        let handle = self.0;
        self.0 = std::ptr::null_mut();
        if unsafe { CloseHandle(handle) } == 0 {
            Err(Error::Io)
        } else {
            Ok(())
        }
    }
}

impl Drop for OwnedHandle {
    fn drop(&mut self) {
        if self.0.is_null() {
            return;
        }
        if unsafe { CloseHandle(self.0) } == 0 {
            std::process::abort();
        }
    }
}

struct Held {
    file: CheckedFile,
    fact: Fact,
}

struct CheckedFile(Option<StdFile>);

impl CheckedFile {
    fn new(file: StdFile) -> Self {
        Self(Some(file))
    }

    fn settle(mut self) -> Result<(), Error> {
        let file = self.0.take().expect("held file remains unsettled");
        close_file(file)
    }
}

impl Deref for CheckedFile {
    type Target = StdFile;
    fn deref(&self) -> &Self::Target {
        self.0.as_ref().expect("held file remains unsettled")
    }
}

pub struct Root {
    held: Held,
    token: TokenAuthority,
    security_descriptor: SecurityDescriptor,
    mutex: MutexGuard,
}

pub struct Directory {
    held: Held,
    authority: Fact,
}

pub struct RegularFile {
    held: Held,
    authority: Fact,
}

struct MutexGuard(Option<OwnedHandle>);

impl MutexGuard {
    fn settle(mut self) -> Result<(), Error> {
        let handle = self.0.take().expect("owned mutex remains unsettled");
        if unsafe { ReleaseMutex(handle.raw()) } == 0 {
            return Err(Error::Io);
        }
        handle.close()
    }
}

impl Drop for MutexGuard {
    fn drop(&mut self) {
        if let Some(handle) = self.0.as_ref() {
            // Failure-path cleanup cannot replace the already selected status.
            // Every successful route uses the checked settle operation above.
            let _ = unsafe { ReleaseMutex(handle.raw()) };
        }
    }
}

pub fn hold_root(path: &Path) -> Result<Root, Error> {
    let token = capture_effective_token()?;
    let file = open_absolute_components(path)?;
    require_ntfs_local(&file)?;
    let fact = authenticate_handle(&file, Kind::Directory, &token.sid)?;
    let security_descriptor = security_descriptor(&file, &token.sid)?;
    let mutex = acquire_mutex(fact, &security_descriptor, &token.sid)?;
    let root = Root {
        held: Held {
            file: CheckedFile::new(file),
            fact,
        },
        token,
        security_descriptor,
        mutex,
    };
    root.recheck_path(path)?;
    Ok(root)
}

impl Root {
    pub fn fact(&self) -> Fact {
        self.held.fact
    }

    pub fn recheck(&self) -> Result<(), Error> {
        require_token_stable(&self.token)?;
        require_fact(
            &self.held.file,
            self.held.fact,
            Kind::Directory,
            &self.token.sid,
        )
    }

    pub fn recheck_path(&self, path: &Path) -> Result<(), Error> {
        self.recheck()?;
        let rebound = open_absolute_components(path)?;
        let fact = authenticate_handle(&rebound, Kind::Directory, &self.token.sid)?;
        if fact != self.held.fact {
            return Err(Error::Changed);
        }
        close_file(rebound)
    }

    pub fn inventory(&self) -> Result<Vec<InventoryEntry>, Error> {
        self.recheck()?;
        enumerate(&self.held.file, &self.token.sid)
    }

    pub fn open_directory(&self, name: &str) -> Result<Directory, Error> {
        self.recheck()?;
        open_child_directory(&self.held.file, name, &self.token.sid, self.held.fact)
    }

    pub fn create_directory(&self, name: &str) -> Result<Directory, Error> {
        self.recheck()?;
        create_child_directory(
            &self.held.file,
            name,
            &self.security_descriptor,
            &self.token.sid,
            self.held.fact,
        )
    }

    pub fn rename_no_replace(&self, stage: &Directory, destination: &str) -> Result<(), Error> {
        self.recheck()?;
        if stage.authority != self.held.fact {
            return Err(Error::Changed);
        }
        stage.recheck(&self.token.sid)?;
        validate_name(destination)?;
        rename_no_replace(&stage.held.file, &self.held.file, destination)
    }

    pub fn flush(&self) -> Result<(), Error> {
        flush(&self.held.file)
    }

    pub fn settle(self) -> Result<(), Error> {
        let Root {
            held,
            token,
            security_descriptor: _,
            mutex,
        } = self;
        held.file.settle()?;
        token.token.close()?;
        mutex.settle()
    }
}

impl Directory {
    pub fn fact(&self) -> Fact {
        self.held.fact
    }

    pub fn recheck_against(&self, root: &Root) -> Result<(), Error> {
        root.recheck()?;
        if self.authority != root.held.fact {
            return Err(Error::Changed);
        }
        self.recheck(&root.token.sid)
    }

    pub fn inventory(&self, root: &Root) -> Result<Vec<InventoryEntry>, Error> {
        self.recheck_against(root)?;
        enumerate(&self.held.file, &root.token.sid)
    }

    pub fn open_directory(&self, root: &Root, name: &str) -> Result<Directory, Error> {
        self.recheck_against(root)?;
        open_child_directory(&self.held.file, name, &root.token.sid, self.authority)
    }

    pub fn create_directory(&self, root: &Root, name: &str) -> Result<Directory, Error> {
        self.recheck_against(root)?;
        create_child_directory(
            &self.held.file,
            name,
            &root.security_descriptor,
            &root.token.sid,
            self.authority,
        )
    }

    pub fn create_file(&self, root: &Root, name: &str, bytes: &[u8]) -> Result<RegularFile, Error> {
        self.recheck_against(root)?;
        validate_name(name)?;
        let mut file = relative_file(
            &self.held.file,
            name,
            FILE_ACCESS,
            FILE_CREATE,
            FILE_NON_DIRECTORY_FILE,
            Some(&root.security_descriptor),
        )?;
        file.write_all(bytes).map_err(|_| Error::Io)?;
        file.flush().map_err(|_| Error::Io)?;
        flush(&file)?;
        let fact = authenticate_handle(&file, Kind::File, &root.token.sid)?;
        require_directory_name_without_short_alias(&self.held.file, name)?;
        if fact.length != bytes.len() as u64 {
            return Err(Error::Changed);
        }
        Ok(RegularFile {
            held: Held {
                file: CheckedFile::new(file),
                fact,
            },
            authority: self.authority,
        })
    }

    pub fn open_file(&self, root: &Root, name: &str) -> Result<RegularFile, Error> {
        self.recheck_against(root)?;
        validate_name(name)?;
        let file = relative_file(
            &self.held.file,
            name,
            FILE_GENERIC_READ | SYNCHRONIZE,
            FILE_OPEN,
            FILE_NON_DIRECTORY_FILE,
            None,
        )?;
        let fact = authenticate_handle(&file, Kind::File, &root.token.sid)?;
        require_directory_name_without_short_alias(&self.held.file, name)?;
        Ok(RegularFile {
            held: Held {
                file: CheckedFile::new(file),
                fact,
            },
            authority: self.authority,
        })
    }

    pub fn flush(&self) -> Result<(), Error> {
        flush(&self.held.file)
    }

    pub fn settle(self) -> Result<(), Error> {
        self.held.file.settle()
    }

    fn recheck(&self, sid: &[u8]) -> Result<(), Error> {
        require_fact(&self.held.file, self.held.fact, Kind::Directory, sid)
    }
}

impl RegularFile {
    pub fn fact(&self) -> Fact {
        self.held.fact
    }

    pub fn read_bounded(&self, root: &Root, limit: usize) -> Result<Vec<u8>, Error> {
        let maximum = limit.checked_add(1).ok_or(Error::Limit)?;
        root.recheck()?;
        if self.authority != root.held.fact {
            return Err(Error::Changed);
        }
        require_fact(&self.held.file, self.held.fact, Kind::File, &root.token.sid)?;
        let mut duplicate = self.held.file.try_clone().map_err(|_| Error::Io)?;
        let mut output = Vec::with_capacity(limit.min(8192));
        let read = (|| {
            duplicate.seek(SeekFrom::Start(0)).map_err(|_| Error::Io)?;
            (&mut duplicate)
                .take(maximum as u64)
                .read_to_end(&mut output)
                .map_err(|_| Error::Io)
        })();
        let close = close_file(duplicate);
        read?;
        close?;
        if output.len() > limit {
            return Err(Error::Limit);
        }
        require_fact(&self.held.file, self.held.fact, Kind::File, &root.token.sid)?;
        Ok(output)
    }

    pub fn settle(self) -> Result<(), Error> {
        self.held.file.settle()
    }
}
