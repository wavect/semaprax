//! Authenticate the kernel sealing object before any filesystem metadata/read.
use super::DoctorOfflineInputError as Error;
use std::fs::File;
use std::mem::MaybeUninit;
use std::os::fd::AsRawFd as _;

const REQUIRED_SEALS: libc::c_int =
    libc::F_SEAL_WRITE | libc::F_SEAL_GROW | libc::F_SEAL_SHRINK | libc::F_SEAL_SEAL;
const CHUNK_BYTES: usize = 8192;

#[cfg(test)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum TestStage {
    Seals,
    FileSystem,
    Metadata,
    Allocate,
    Read { offset: usize, length: usize },
}

#[cfg(test)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum TestReadFault {
    Short,
    Zero,
    Interrupted,
    Io,
}

#[cfg(test)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum TestFault {
    Seals,
    FileSystem,
    Metadata,
    Allocation,
    WrongFileSystem,
    NonRegular,
    NegativeSize,
    Read { call: usize, outcome: TestReadFault },
}

#[cfg(test)]
#[derive(Default)]
pub(super) struct TestControl {
    pub(super) trace: Vec<TestStage>,
    pub(super) fault: Option<TestFault>,
}

#[cfg(test)]
fn enter(control: &mut Option<&mut TestControl>, stage: TestStage) -> Option<TestFault> {
    control.as_deref_mut().and_then(|control| {
        control.trace.push(stage);
        control.fault
    })
}

pub(super) fn snapshot(
    file: &File,
    max_bytes: usize,
    #[cfg(test)] mut control: Option<&mut TestControl>,
) -> Result<Vec<u8>, Error> {
    let fd = file.as_raw_fd();
    #[cfg(test)]
    let seal_fault = enter(&mut control, TestStage::Seals);
    // SAFETY: the borrowed File keeps this descriptor alive throughout the
    // call. F_GET_SEALS is a kernel memfd/shmem/hugetlb type dispatch; unlike
    // fstat/fstatfs or closing a duplicate, it does not call arbitrary file I/O.
    let seals = unsafe { libc::fcntl(fd, libc::F_GET_SEALS, 0 as libc::c_long) };
    #[cfg(test)]
    let seals = if seal_fault == Some(TestFault::Seals) {
        -1
    } else {
        seals
    };
    if seals < 0 {
        let error = std::io::Error::last_os_error().raw_os_error();
        #[cfg(test)]
        let error = if seal_fault == Some(TestFault::Seals) {
            Some(libc::EIO)
        } else {
            error
        };
        return Err(if error == Some(libc::EINVAL) {
            Error::Invalid
        } else {
            Error::Io
        });
    }
    if seals & REQUIRED_SEALS != REQUIRED_SEALS {
        return Err(Error::Invalid);
    }

    #[cfg(test)]
    let fs_fault = enter(&mut control, TestStage::FileSystem);
    let mut filesystem = MaybeUninit::<libc::statfs>::uninit();
    // SAFETY: writable statfs storage; the borrowed descriptor has passed the
    // immutable sealing gate, restricting this query to kernel memory files.
    let status = unsafe { libc::fstatfs(fd, filesystem.as_mut_ptr()) };
    #[cfg(test)]
    let status = if fs_fault == Some(TestFault::FileSystem) {
        -1
    } else {
        status
    };
    if status != 0 {
        return Err(Error::Io);
    }
    // SAFETY: successful fstatfs initialized the structure.
    let filesystem = unsafe { filesystem.assume_init() };
    #[cfg(test)]
    let filesystem = if fs_fault == Some(TestFault::WrongFileSystem) {
        let mut changed = filesystem;
        changed.f_type = 0;
        changed
    } else {
        filesystem
    };
    if filesystem.f_type != libc::TMPFS_MAGIC {
        return Err(Error::Invalid);
    }

    #[cfg(test)]
    let stat_fault = enter(&mut control, TestStage::Metadata);
    let mut metadata = MaybeUninit::<libc::stat>::uninit();
    // SAFETY: writable stat storage and the same borrowed immutable tmpfs file.
    let status = unsafe { libc::fstat(fd, metadata.as_mut_ptr()) };
    #[cfg(test)]
    let status = if stat_fault == Some(TestFault::Metadata) {
        -1
    } else {
        status
    };
    if status != 0 {
        return Err(Error::Io);
    }
    // SAFETY: successful fstat initialized the structure.
    let metadata = unsafe { metadata.assume_init() };
    #[cfg(test)]
    let metadata = match stat_fault {
        Some(TestFault::NonRegular) => {
            let mut changed = metadata;
            changed.st_mode = libc::S_IFDIR;
            changed
        }
        Some(TestFault::NegativeSize) => {
            let mut changed = metadata;
            changed.st_size = -1;
            changed
        }
        _ => metadata,
    };
    if metadata.st_mode & libc::S_IFMT != libc::S_IFREG || metadata.st_size <= 0 {
        return Err(Error::Invalid);
    }
    let length = usize::try_from(metadata.st_size).map_err(|_| Error::Io)?;
    if length > max_bytes {
        return Err(Error::Limit);
    }

    #[cfg(test)]
    let allocation_fault = enter(&mut control, TestStage::Allocate);
    let mut bytes = Vec::new();
    let reserve_length = length;
    // A capacity-overflow request deterministically exercises the same fallible
    // reservation branch without requesting physical memory from the allocator.
    #[cfg(test)]
    let reserve_length = if allocation_fault == Some(TestFault::Allocation) {
        usize::MAX
    } else {
        reserve_length
    };
    bytes
        .try_reserve_exact(reserve_length)
        .map_err(|_| Error::Io)?;
    bytes.resize(length, 0);
    let mut offset = 0usize;
    #[cfg(test)]
    let mut read_call = 0usize;
    while offset < length {
        let count = CHUNK_BYTES.min(length - offset);
        let position = libc::off_t::try_from(offset).map_err(|_| Error::Io)?;
        #[cfg(test)]
        let read_fault = {
            read_call += 1;
            match enter(
                &mut control,
                TestStage::Read {
                    offset,
                    length: count,
                },
            ) {
                Some(TestFault::Read { call, outcome }) if call == read_call => Some(outcome),
                _ => None,
            }
        };
        #[cfg(test)]
        let result = match read_fault {
            Some(TestReadFault::Short) => isize::try_from(count - 1).map_err(|_| Error::Io)?,
            Some(TestReadFault::Zero) => 0,
            // Simulated syscall outcomes, not physical signal/error evidence.
            Some(TestReadFault::Interrupted | TestReadFault::Io) => -1,
            None => read_chunk(fd, &mut bytes[offset..offset + count], position),
        };
        #[cfg(not(test))]
        let result = read_chunk(fd, &mut bytes[offset..offset + count], position);
        if usize::try_from(result).map_err(|_| Error::Io)? != count {
            return Err(Error::Io);
        }
        offset += count;
    }
    Ok(bytes)
}

fn read_chunk(fd: libc::c_int, bytes: &mut [u8], position: libc::off_t) -> isize {
    // SAFETY: writable slice and a live borrowed descriptor. pread does not
    // change the shared file offset. No retry occurs for a nonexact outcome.
    unsafe { libc::pread(fd, bytes.as_mut_ptr().cast(), bytes.len(), position) }
}
