fn security_descriptor(file: &StdFile, sid: &[u8]) -> Result<SecurityDescriptor, Error> {
    let mut owner = std::ptr::null_mut();
    let mut group = std::ptr::null_mut();
    let mut dacl = std::ptr::null_mut();
    let mut descriptor: PSECURITY_DESCRIPTOR = std::ptr::null_mut();
    let status = unsafe {
        GetSecurityInfo(
            file.as_raw_handle().cast(),
            SE_FILE_OBJECT,
            OWNER_SECURITY_INFORMATION | GROUP_SECURITY_INFORMATION | DACL_SECURITY_INFORMATION,
            &mut owner,
            &mut group,
            &mut dacl,
            std::ptr::null_mut(),
            &mut descriptor,
        )
    };
    if status != 0 || descriptor.is_null() {
        return Err(Error::Changed);
    }
    let result = (|| {
        if owner.is_null()
            || dacl.is_null()
            || unsafe { IsValidSid(owner) } == 0
            || unsafe { EqualSid(owner, sid.as_ptr().cast_mut().cast()) } == 0
        {
            return Err(Error::Changed);
        }
        validate_dacl(descriptor, dacl, sid, FILE_ALL_ACCESS)?;
        let mut control = 0u16;
        let mut revision = 0u32;
        if unsafe { GetSecurityDescriptorControl(descriptor, &mut control, &mut revision) } == 0 {
            return Err(Error::Changed);
        }
        if control & SE_SELF_RELATIVE == 0 {
            return Err(Error::Changed);
        }
        let length = unsafe { GetSecurityDescriptorLength(descriptor) } as usize;
        if length == 0 || length > MAX_SECURITY_DESCRIPTOR_BYTES {
            return Err(Error::Changed);
        }
        let mut words = vec![0u64; length.div_ceil(std::mem::size_of::<u64>())];
        unsafe {
            std::ptr::copy_nonoverlapping(
                descriptor.cast::<u8>(),
                words.as_mut_ptr().cast::<u8>(),
                length,
            )
        };
        let owned = SecurityDescriptor {
            words,
            bytes: length,
        };
        validate_owned_descriptor(&owned, sid)?;
        Ok(owned)
    })();
    let released =
        unsafe { windows_sys::Win32::Foundation::LocalFree(descriptor.cast()) }.is_null();
    result.and_then(|owned| if released { Ok(owned) } else { Err(Error::Io) })
}

fn validate_dacl(
    descriptor: PSECURITY_DESCRIPTOR,
    dacl: *mut ACL,
    sid: &[u8],
    expected_access: u32,
) -> Result<(), Error> {
    let mut control = 0u16;
    let mut revision = 0u32;
    if unsafe { GetSecurityDescriptorControl(descriptor, &mut control, &mut revision) } == 0
        || control & SE_DACL_PROTECTED == 0
    {
        return Err(Error::Changed);
    }
    let mut present = 0i32;
    let mut defaulted = 0i32;
    let mut observed = std::ptr::null_mut();
    if unsafe { GetSecurityDescriptorDacl(descriptor, &mut present, &mut observed, &mut defaulted) }
        == 0
        || present == 0
        || observed.is_null()
        || observed != dacl
    {
        return Err(Error::Changed);
    }
    if unsafe { IsValidAcl(dacl) } == 0 {
        return Err(Error::Changed);
    }
    let acl = unsafe { &*dacl };
    if acl.AceCount != 2 {
        return Err(Error::Changed);
    }
    let system_sid: [u8; 12] = [1, 1, 0, 0, 0, 0, 0, 5, 18, 0, 0, 0];
    let mut user_seen = false;
    let mut system_seen = false;
    for index in 0..acl.AceCount {
        let mut ace = std::ptr::null_mut();
        if unsafe { GetAce(dacl, u32::from(index), &mut ace) } == 0 || ace.is_null() {
            return Err(Error::Changed);
        }
        let acl_start = dacl as usize;
        let acl_end = acl_start
            .checked_add(usize::from(acl.AclSize))
            .ok_or(Error::Changed)?;
        let ace_start = ace as usize;
        if ace_start < acl_start + std::mem::size_of::<ACL>()
            || ace_start
                .checked_add(std::mem::size_of::<ACE_HEADER>())
                .is_none_or(|end| end > acl_end)
        {
            return Err(Error::Changed);
        }
        let header = unsafe { &*(ace.cast::<ACE_HEADER>()) };
        let ace_size = usize::from(header.AceSize);
        let sid_offset = std::mem::offset_of!(ACCESS_ALLOWED_ACE, SidStart);
        if header.AceType != 0
            || ace_size < sid_offset + 8
            || ace_start
                .checked_add(ace_size)
                .is_none_or(|end| end > acl_end)
        {
            return Err(Error::Changed);
        }
        let sid_start = unsafe { ace.cast::<u8>().add(sid_offset) };
        let sid_length = 8usize
            .checked_add(usize::from(unsafe { *sid_start.add(1) }) * 4)
            .ok_or(Error::Changed)?;
        if sid_offset + sid_length != ace_size {
            return Err(Error::Changed);
        }
        let allowed = unsafe { &*(ace.cast::<ACCESS_ALLOWED_ACE>()) };
        if allowed.Header.AceType != 0
            || allowed.Header.AceFlags != 0
            || allowed.Mask != expected_access
        {
            return Err(Error::Changed);
        }
        if usize::from(allowed.Header.AceSize)
            < std::mem::offset_of!(ACCESS_ALLOWED_ACE, SidStart) + 8
        {
            return Err(Error::Changed);
        }
        let ace_sid = std::ptr::addr_of!(allowed.SidStart).cast_mut().cast();
        if unsafe { IsValidSid(ace_sid) } == 0 {
            return Err(Error::Changed);
        }
        if unsafe { EqualSid(ace_sid, sid.as_ptr().cast_mut().cast()) } != 0 {
            if user_seen {
                return Err(Error::Changed);
            }
            user_seen = true;
        } else if unsafe { EqualSid(ace_sid, system_sid.as_ptr().cast_mut().cast()) } != 0 {
            if system_seen {
                return Err(Error::Changed);
            }
            system_seen = true;
        } else {
            return Err(Error::Changed);
        }
    }
    if !user_seen || !system_seen {
        return Err(Error::Changed);
    }
    Ok(())
}

fn validate_owned_descriptor(descriptor: &SecurityDescriptor, sid: &[u8]) -> Result<(), Error> {
    let mut owner = std::ptr::null_mut();
    let mut defaulted = 0i32;
    if unsafe { GetSecurityDescriptorOwner(descriptor.as_ptr(), &mut owner, &mut defaulted) } == 0
        || owner.is_null()
        || unsafe { IsValidSid(owner) } == 0
        || unsafe { EqualSid(owner, sid.as_ptr().cast_mut().cast()) } == 0
    {
        return Err(Error::Changed);
    }
    let mut present = 0i32;
    let mut dacl = std::ptr::null_mut();
    if unsafe {
        GetSecurityDescriptorDacl(descriptor.as_ptr(), &mut present, &mut dacl, &mut defaulted)
    } == 0
        || present == 0
        || dacl.is_null()
    {
        return Err(Error::Changed);
    }
    validate_dacl(descriptor.as_ptr(), dacl, sid, FILE_ALL_ACCESS)
}

fn capture_effective_token() -> Result<TokenAuthority, Error> {
    let mut handle: HANDLE = std::ptr::null_mut();
    let thread = unsafe { OpenThreadToken(GetCurrentThread(), TOKEN_QUERY, 1, &mut handle) };
    if thread == 0
        && (unsafe { GetLastError() } != ERROR_NO_TOKEN
            || unsafe { OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &mut handle) } == 0)
    {
        return Err(Error::Changed);
    }
    let token = OwnedHandle(handle);
    let user = token_information(token.raw(), TokenUser)?;
    let token_user = unsafe { &*(user.as_ptr().cast::<TOKEN_USER>()) };
    if unsafe { IsValidSid(token_user.User.Sid) } == 0 {
        return Err(Error::Changed);
    }
    let sid_length =
        unsafe { windows_sys::Win32::Security::GetLengthSid(token_user.User.Sid) } as usize;
    if sid_length == 0 || sid_length > MAX_TOKEN_BYTES {
        return Err(Error::Changed);
    }
    let sid = unsafe { std::slice::from_raw_parts(token_user.User.Sid.cast::<u8>(), sid_length) }
        .to_vec();
    let mut statistics = std::mem::MaybeUninit::<TOKEN_STATISTICS>::uninit();
    let mut written = 0u32;
    if unsafe {
        GetTokenInformation(
            token.raw(),
            TokenStatistics,
            statistics.as_mut_ptr().cast(),
            std::mem::size_of::<TOKEN_STATISTICS>() as u32,
            &mut written,
        )
    } == 0
        || written as usize != std::mem::size_of::<TOKEN_STATISTICS>()
    {
        return Err(Error::Changed);
    }
    let statistics = unsafe { statistics.assume_init() };
    let identity = TokenIdentity {
        token_low: statistics.TokenId.LowPart,
        token_high: statistics.TokenId.HighPart,
        authentication_low: statistics.AuthenticationId.LowPart,
        authentication_high: statistics.AuthenticationId.HighPart,
        modified_low: statistics.ModifiedId.LowPart,
        modified_high: statistics.ModifiedId.HighPart,
    };
    Ok(TokenAuthority {
        token,
        sid,
        identity,
    })
}

fn token_information(
    token: HANDLE,
    class: windows_sys::Win32::Security::TOKEN_INFORMATION_CLASS,
) -> Result<Vec<u64>, Error> {
    let mut needed = 0u32;
    unsafe { GetTokenInformation(token, class, std::ptr::null_mut(), 0, &mut needed) };
    let length = needed as usize;
    if length == 0 || length > MAX_TOKEN_BYTES {
        return Err(Error::Changed);
    }
    let mut output = vec![0u64; length.div_ceil(std::mem::size_of::<u64>())];
    if unsafe {
        GetTokenInformation(
            token,
            class,
            output.as_mut_ptr().cast(),
            needed,
            &mut needed,
        )
    } == 0
        || needed as usize != length
    {
        return Err(Error::Changed);
    }
    Ok(output)
}

fn require_token_stable(authority: &TokenAuthority) -> Result<(), Error> {
    let observed = capture_effective_token()?;
    if observed.sid != authority.sid || observed.identity != authority.identity {
        return Err(Error::Changed);
    }
    observed.token.close()
}

fn acquire_mutex(
    fact: Fact,
    descriptor: &SecurityDescriptor,
    sid: &[u8],
) -> Result<MutexGuard, Error> {
    let descriptor = mutex_security_descriptor(descriptor, sid)?;
    let mut name = format!("Global\\SemapraxRevisionStore-{:016x}-", fact.volume)
        .encode_utf16()
        .collect::<Vec<_>>();
    for byte in fact.file_id {
        name.extend(format!("{byte:02x}").encode_utf16());
    }
    name.push(0);
    let security = windows_sys::Win32::Security::SECURITY_ATTRIBUTES {
        nLength: std::mem::size_of::<windows_sys::Win32::Security::SECURITY_ATTRIBUTES>() as u32,
        lpSecurityDescriptor: descriptor.as_ptr(),
        bInheritHandle: 0,
    };
    let handle = unsafe { CreateMutexW(&security, 1, name.as_ptr()) };
    if handle.is_null() {
        return Err(Error::Changed);
    }
    let existed = unsafe { GetLastError() } == ERROR_ALREADY_EXISTS;
    let owned = OwnedHandle(handle);
    validate_kernel_dacl(owned.raw(), sid)?;
    if existed {
        owned.close()?;
        return Err(Error::Busy);
    }
    Ok(MutexGuard(Some(owned)))
}

fn mutex_security_descriptor(
    source: &SecurityDescriptor,
    sid: &[u8],
) -> Result<SecurityDescriptor, Error> {
    validate_owned_descriptor(source, sid)?;
    let mut descriptor = SecurityDescriptor {
        words: source.words.clone(),
        bytes: source.bytes,
    };
    let mut present = 0i32;
    let mut defaulted = 0i32;
    let mut dacl = std::ptr::null_mut();
    if unsafe {
        GetSecurityDescriptorDacl(
            descriptor.as_mut_ptr(),
            &mut present,
            &mut dacl,
            &mut defaulted,
        )
    } == 0
        || present == 0
        || dacl.is_null()
    {
        return Err(Error::Changed);
    }
    for index in 0..2u32 {
        let mut ace = std::ptr::null_mut();
        if unsafe { GetAce(dacl, index, &mut ace) } == 0 || ace.is_null() {
            return Err(Error::Changed);
        }
        // The cloned, owned descriptor was fully validated as two standard
        // allow ACEs above; only the object-specific access mask is changed.
        unsafe {
            (*ace.cast::<ACCESS_ALLOWED_ACE>()).Mask = MUTEX_ALL_ACCESS;
        }
    }
    validate_dacl(descriptor.as_ptr(), dacl, sid, MUTEX_ALL_ACCESS)?;
    Ok(descriptor)
}

fn validate_kernel_dacl(handle: HANDLE, sid: &[u8]) -> Result<(), Error> {
    let mut owner = std::ptr::null_mut();
    let mut dacl = std::ptr::null_mut();
    let mut descriptor = std::ptr::null_mut();
    let status = unsafe {
        GetSecurityInfo(
            handle,
            SE_KERNEL_OBJECT,
            OWNER_SECURITY_INFORMATION | DACL_SECURITY_INFORMATION,
            &mut owner,
            std::ptr::null_mut(),
            &mut dacl,
            std::ptr::null_mut(),
            &mut descriptor,
        )
    };
    if status != 0 || descriptor.is_null() {
        return Err(Error::Changed);
    }
    let valid = !owner.is_null()
        && !dacl.is_null()
        && unsafe { IsValidSid(owner) } != 0
        && unsafe { EqualSid(owner, sid.as_ptr().cast_mut().cast()) } != 0
        && validate_dacl(descriptor, dacl, sid, MUTEX_ALL_ACCESS).is_ok();
    let released =
        unsafe { windows_sys::Win32::Foundation::LocalFree(descriptor.cast()) }.is_null();
    if !valid {
        return Err(Error::Changed);
    }
    if released {
        Ok(())
    } else {
        Err(Error::Io)
    }
}
