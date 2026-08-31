//! Native operations on one fresh, exclusively owned memory-file descriptor.
use super::{DoctorOfflineInput, Error, Storage};
use std::fs::File;
use std::os::fd::{AsRawFd, FromRawFd, IntoRawFd};

const IMMUTABLE: i32 =
    libc::F_SEAL_WRITE | libc::F_SEAL_GROW | libc::F_SEAL_SHRINK | libc::F_SEAL_SEAL;
const REQUIRED: i32 = IMMUTABLE | libc::F_SEAL_EXEC;

pub(super) fn create(
    bytes: &[u8],
    max_bytes: usize,
    storage: Storage,
    #[cfg(test)] mut control: Option<&mut TestControl>,
) -> Result<(File, DoctorOfflineInput), Error> {
    // Every pwrite offset is representable before creating any descriptor.
    libc::off_t::try_from(bytes.len()).map_err(|_| Error::Limit)?;
    #[cfg(test)]
    {
        record(&mut control, TestStage::Create);
        if fault(&control, TestFault::Create) {
            return Err(Error::Io);
        }
        if fault(&control, TestFault::CreateUnsupported) {
            return Err(Error::Unsupported);
        }
    }
    // Closed internal policy, not caller-chosen flags. Neither explicit mode
    // has an older-kernel, host-default, or executable-permission fallback.
    let (name, execution_flag) = match storage {
        Storage::NonExecutable => (c"semaprax-doctor-input", libc::MFD_NOEXEC_SEAL),
        Storage::Executable => (c"semaprax-doctor-executable", libc::MFD_EXEC),
    };
    let fd = unsafe {
        libc::memfd_create(
            name.as_ptr(),
            libc::MFD_CLOEXEC | libc::MFD_ALLOW_SEALING | execution_flag,
        )
    };
    if fd < 0 {
        return Err(match std::io::Error::last_os_error().raw_os_error() {
            Some(libc::EINVAL | libc::ENOSYS) => Error::Unsupported,
            _ => Error::Io,
        });
    }
    // Arm checked ownership immediately, including unexpected Rust unwinding.
    let file = NewFile(Some(unsafe { File::from_raw_fd(fd) }));
    let result = populate(
        file.borrow(),
        bytes,
        max_bytes,
        storage,
        #[cfg(test)]
        &mut control,
    );
    match result {
        Ok(snapshot) => Ok((file.transfer(), snapshot)),
        Err(primary) => {
            #[cfg(test)]
            record(&mut control, TestStage::Close);
            #[cfg(test)]
            let forced = control.as_ref().is_some_and(|state| state.close_fault);
            #[cfg(not(test))]
            let forced = false;
            file.reject(forced);
            Err(primary)
        }
    }
}

struct NewFile(Option<File>);
impl NewFile {
    fn borrow(&self) -> &File {
        self.0.as_ref().expect("new input remains owned")
    }
    fn transfer(mut self) -> File {
        self.0.take().expect("new input transfers once")
    }
    fn reject(mut self, forced: bool) {
        if let Some(file) = self.0.take() {
            checked_close(file, forced);
        }
    }
}
impl Drop for NewFile {
    fn drop(&mut self) {
        if let Some(file) = self.0.take() {
            checked_close(file, false);
        }
    }
}
fn checked_close(file: File, forced: bool) {
    // Disarm File before the sole syscall; even EINTR must never be retried.
    // Tests force an uncertain result only AFTER actually closing the owned fd.
    let result = unsafe { libc::close(file.into_raw_fd()) };
    if result < 0 || forced {
        unsafe { libc::_exit(126) };
    }
}

fn populate(
    file: &File,
    bytes: &[u8],
    max_bytes: usize,
    storage: Storage,
    #[cfg(test)] control: &mut Option<&mut TestControl>,
) -> Result<DoctorOfflineInput, Error> {
    let fd = file.as_raw_fd();
    if storage == Storage::Executable {
        #[cfg(test)]
        {
            record(control, TestStage::Mode);
            if fault(control, TestFault::Mode) {
                return Err(Error::Io);
            }
        }
        // Set owner-only read/execute immediately on the owned unpublished
        // file. Its original O_RDWR description still permits bounded pwrite.
        if unsafe { libc::fchmod(fd, 0o500) } != 0 {
            return Err(Error::Io);
        }
    }
    let mut offset = 0;
    #[cfg(test)]
    let mut write_call = 0;
    while offset < bytes.len() {
        let end = bytes.len().min(offset + 8192);
        let expected = end - offset;
        let position = libc::off_t::try_from(offset).map_err(|_| Error::Limit)?;
        #[cfg(test)]
        let forced = {
            write_call += 1;
            record(
                control,
                TestStage::Write {
                    offset,
                    length: expected,
                },
            );
            match control.as_ref().and_then(|state| state.fault) {
                Some(TestFault::Write { call, outcome }) if call == write_call => {
                    Some(match outcome {
                        TestWriteFault::Short => (expected - 1) as isize,
                        TestWriteFault::Zero => 0,
                        TestWriteFault::Interrupted | TestWriteFault::Io => -1,
                    })
                }
                _ => None,
            }
        };
        let write =
            || unsafe { libc::pwrite(fd, bytes[offset..end].as_ptr().cast(), expected, position) };
        #[cfg(test)]
        let count = forced.unwrap_or_else(write);
        #[cfg(not(test))]
        let count = write();
        // Short, zero, interrupted and failed writes select one sticky failure.
        // Never retry or seal/read/publish a partial carrier.
        if count < 0 || count as usize != expected {
            return Err(Error::Io);
        }
        offset = end;
    }
    #[cfg(test)]
    {
        record(control, TestStage::Seal);
        if fault(control, TestFault::Seal) {
            return Err(Error::Io);
        }
    }
    let seals = match storage {
        Storage::NonExecutable => IMMUTABLE,
        Storage::Executable => REQUIRED,
    };
    if unsafe { libc::fcntl(fd, libc::F_ADD_SEALS, seals) } != 0 {
        return Err(Error::Io);
    }
    verify_properties(
        file,
        bytes.len(),
        storage,
        #[cfg(test)]
        control,
    )?;
    #[cfg(test)]
    {
        record(control, TestStage::Snapshot);
        if fault(control, TestFault::Snapshot) {
            return Err(Error::Io);
        }
    }
    // No raw byte constructor: all existing seals/storage/size/read checks run.
    let snapshot = DoctorOfflineInput::acquire(file, max_bytes)?;
    #[cfg(test)]
    {
        record(control, TestStage::Compare);
        if fault(control, TestFault::Mismatch) {
            return Err(Error::Invalid);
        }
    }
    if snapshot.bytes() != bytes {
        return Err(Error::Invalid);
    }
    Ok(snapshot)
}

fn verify_properties(
    file: &File,
    expected_length: usize,
    storage: Storage,
    #[cfg(test)] control: &mut Option<&mut TestControl>,
) -> Result<(), Error> {
    let fd = file.as_raw_fd();
    #[cfg(test)]
    {
        record(control, TestStage::GetSeals);
        if fault(control, TestFault::GetSeals) {
            return Err(Error::Io);
        }
    }
    let seals = unsafe { libc::fcntl(fd, libc::F_GET_SEALS) };
    if seals < 0 {
        return Err(Error::Io);
    }
    #[cfg(test)]
    let seals = if fault(control, TestFault::MissingSeals) {
        seals & !libc::F_SEAL_EXEC
    } else {
        seals
    };
    if seals & REQUIRED != REQUIRED {
        return Err(Error::Invalid);
    }
    #[cfg(test)]
    {
        record(control, TestStage::Metadata);
        if fault(control, TestFault::Metadata) {
            return Err(Error::Io);
        }
    }
    let mut metadata = std::mem::MaybeUninit::<libc::stat>::zeroed();
    if unsafe { libc::fstat(fd, metadata.as_mut_ptr()) } != 0 {
        return Err(Error::Io);
    }
    let metadata = unsafe { metadata.assume_init() };
    let mode = metadata.st_mode;
    #[cfg(test)]
    let mode = if fault(control, TestFault::ExecutableMode) {
        mode | 0o100
    } else {
        mode
    };
    #[cfg(test)]
    let mode = if fault(control, TestFault::WrongMode) {
        mode & !0o100
    } else if fault(control, TestFault::ExcessMode) {
        mode | 0o020
    } else {
        mode
    };
    let length = metadata.st_size;
    #[cfg(test)]
    let length = if fault(control, TestFault::SizeMismatch) {
        length.saturating_add(1)
    } else {
        length
    };
    let valid_mode = match storage {
        Storage::NonExecutable => mode & 0o111 == 0,
        Storage::Executable => mode & 0o7777 == 0o500,
    };
    if mode & libc::S_IFMT != libc::S_IFREG
        || !valid_mode
        || usize::try_from(length).map_err(|_| Error::Invalid)? != expected_length
    {
        return Err(Error::Invalid);
    }
    #[cfg(test)]
    {
        record(control, TestStage::GetFlags);
        if fault(control, TestFault::GetFlags) {
            return Err(Error::Io);
        }
    }
    let flags = unsafe { libc::fcntl(fd, libc::F_GETFD) };
    if flags < 0 {
        return Err(Error::Io);
    }
    #[cfg(test)]
    let flags = if fault(control, TestFault::MissingCloexec) {
        flags & !libc::FD_CLOEXEC
    } else {
        flags
    };
    if flags & libc::FD_CLOEXEC == 0 {
        return Err(Error::Invalid);
    }
    Ok(())
}

#[cfg(test)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum TestStage {
    Create,
    Mode,
    Write { offset: usize, length: usize },
    Seal,
    GetSeals,
    Metadata,
    GetFlags,
    Snapshot,
    Compare,
    Close,
}

#[cfg(test)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum TestWriteFault {
    Short,
    Zero,
    Interrupted,
    Io,
}

#[cfg(test)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum TestFault {
    Create,
    CreateUnsupported,
    Mode,
    Write {
        call: usize,
        outcome: TestWriteFault,
    },
    Seal,
    GetSeals,
    MissingSeals,
    Metadata,
    ExecutableMode,
    WrongMode,
    ExcessMode,
    SizeMismatch,
    GetFlags,
    MissingCloexec,
    Snapshot,
    Mismatch,
}

#[cfg(test)]
#[derive(Default)]
pub(super) struct TestControl {
    pub fault: Option<TestFault>,
    pub close_fault: bool,
    pub events: Vec<TestStage>,
}

#[cfg(test)]
fn record(control: &mut Option<&mut TestControl>, stage: TestStage) {
    if let Some(control) = control.as_deref_mut() {
        control.events.push(stage);
    }
}

#[cfg(test)]
fn fault(control: &Option<&mut TestControl>, expected: TestFault) -> bool {
    control
        .as_ref()
        .is_some_and(|control| control.fault == Some(expected))
}
