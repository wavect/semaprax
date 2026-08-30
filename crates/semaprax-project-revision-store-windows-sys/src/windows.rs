use std::ffi::{OsStr, OsString};
use std::fs::File as StdFile;
use std::io::{Read, Seek, SeekFrom, Write};
use std::ops::Deref;
use std::os::windows::ffi::{OsStrExt, OsStringExt};
use std::os::windows::io::{AsRawHandle, FromRawHandle, IntoRawHandle};
use std::path::{Component, Path};

use windows_sys::Wdk::Foundation::OBJECT_ATTRIBUTES;
use windows_sys::Wdk::Storage::FileSystem::{
    FileRenameInformationEx, NtCreateFile, NtSetInformationFile, FILE_CREATE, FILE_DIRECTORY_FILE,
    FILE_NON_DIRECTORY_FILE, FILE_OPEN, FILE_OPEN_REPARSE_POINT, FILE_SYNCHRONOUS_IO_NONALERT,
};
use windows_sys::Win32::Foundation::{
    CloseHandle, GetLastError, ERROR_ALREADY_EXISTS, ERROR_HANDLE_EOF, ERROR_MORE_DATA,
    ERROR_NO_MORE_FILES, ERROR_NO_TOKEN, HANDLE, INVALID_HANDLE_VALUE,
    STATUS_OBJECT_NAME_COLLISION, UNICODE_STRING,
};
use windows_sys::Win32::Security::Authorization::{
    GetSecurityInfo, SE_FILE_OBJECT, SE_KERNEL_OBJECT,
};
use windows_sys::Win32::Security::{
    EqualSid, GetAce, GetSecurityDescriptorControl, GetSecurityDescriptorDacl,
    GetSecurityDescriptorLength, GetSecurityDescriptorOwner, GetTokenInformation, IsValidAcl,
    IsValidSid, TokenStatistics, TokenUser, ACCESS_ALLOWED_ACE, ACE_HEADER, ACL,
    DACL_SECURITY_INFORMATION, GROUP_SECURITY_INFORMATION, OWNER_SECURITY_INFORMATION,
    PSECURITY_DESCRIPTOR, SE_DACL_PROTECTED, SE_SELF_RELATIVE, TOKEN_QUERY, TOKEN_STATISTICS,
    TOKEN_USER,
};
use windows_sys::Win32::Storage::FileSystem::{
    CreateFileW, FileIdBothDirectoryInfo, FileIdBothDirectoryRestartInfo, FileIdInfo,
    FileStandardInfo, FileStreamInfo, FlushFileBuffers, GetDriveTypeW, GetFileInformationByHandle,
    GetFileInformationByHandleEx, GetVolumeInformationByHandleW, BY_HANDLE_FILE_INFORMATION,
    DELETE, FILE_ADD_FILE, FILE_ADD_SUBDIRECTORY, FILE_ALL_ACCESS, FILE_ATTRIBUTE_DIRECTORY,
    FILE_ATTRIBUTE_NORMAL, FILE_ATTRIBUTE_REPARSE_POINT, FILE_DELETE_CHILD, FILE_GENERIC_READ,
    FILE_GENERIC_WRITE, FILE_ID_BOTH_DIR_INFO, FILE_ID_INFO, FILE_LIST_DIRECTORY,
    FILE_READ_ATTRIBUTES, FILE_SHARE_DELETE, FILE_SHARE_READ, FILE_SHARE_WRITE, FILE_STANDARD_INFO,
    FILE_STREAM_INFO, FILE_TRAVERSE, FILE_WRITE_ATTRIBUTES, OPEN_EXISTING, READ_CONTROL,
    SYNCHRONIZE,
};
use windows_sys::Win32::System::SystemServices::{
    FILE_CASE_PRESERVED_NAMES, FILE_CASE_SENSITIVE_SEARCH, FILE_NAMED_STREAMS,
    FILE_PERSISTENT_ACLS, FILE_UNICODE_ON_DISK,
};
use windows_sys::Win32::System::Threading::{
    CreateMutexW, GetCurrentProcess, GetCurrentThread, OpenProcessToken, OpenThreadToken,
    ReleaseMutex, MUTEX_ALL_ACCESS,
};
use windows_sys::Win32::System::WindowsProgramming::DRIVE_FIXED;
use windows_sys::Win32::System::IO::IO_STATUS_BLOCK;

const HELD_SHARE: u32 = FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE;
const DIRECTORY_FLAGS: u32 = 0x0200_0000 | 0x0020_0000; // BACKUP_SEMANTICS | OPEN_REPARSE_POINT
const DIRECTORY_READ_ACCESS: u32 =
    FILE_LIST_DIRECTORY | FILE_TRAVERSE | FILE_READ_ATTRIBUTES | READ_CONTROL | SYNCHRONIZE;
const DIRECTORY_ACCESS: u32 = FILE_LIST_DIRECTORY
    | FILE_TRAVERSE
    | FILE_READ_ATTRIBUTES
    | FILE_ADD_FILE
    | FILE_ADD_SUBDIRECTORY
    | FILE_DELETE_CHILD
    | FILE_WRITE_ATTRIBUTES
    | DELETE
    | READ_CONTROL
    | FILE_GENERIC_WRITE
    | SYNCHRONIZE;
const FILE_ACCESS: u32 = FILE_GENERIC_READ | FILE_GENERIC_WRITE | DELETE | SYNCHRONIZE;
const OBJ_CASE_INSENSITIVE: u32 = 0x40;
const MAX_SECURITY_DESCRIPTOR_BYTES: usize = 65_536;
const MAX_TOKEN_BYTES: usize = 8_192;
const MAX_DIRECTORY_QUERY_BYTES: usize = 65_536;
const MAX_STREAM_QUERY_BYTES: usize = 4_096;

// The quarantine is split by responsibility while retaining one private namespace.
include!("windows/handles.rs");
include!("windows/filesystem.rs");
include!("windows/security.rs");
include!("windows/inventory.rs");

#[cfg(test)]
mod tests;
