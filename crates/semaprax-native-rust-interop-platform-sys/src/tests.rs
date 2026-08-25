//! Source-locked hostile evidence for the OS quarantine.
const COMMON_SOURCE: &str = include_str!("lib.rs");
const UNIX_SOURCE: &str = include_str!("unix.rs");
const WINDOWS_SOURCE: &str = include_str!("windows.rs");

fn production_sources() -> String {
    [COMMON_SOURCE, UNIX_SOURCE, WINDOWS_SOURCE].concat()
}

use super::{enter_prepared_file_syscalls, Error, TEST_PREPARED_FILE_SYSCALL_ENTRIES};
#[cfg(any(target_os = "linux", target_os = "macos", target_os = "windows"))]
use super::{set_test_settlement_failures, TestSettlementFailure};
use std::sync::atomic::Ordering;

#[cfg(any(target_os = "linux", windows))]
fn archive_prepared_for_test(
    archiver: &super::platform::Executable,
    cwd: &super::platform::Directory,
    input: &super::platform::RegularFile,
    prepared: super::platform::PreparedArchiveInvocation,
    process: &mut super::platform::PreparedProcessArena,
) -> Result<super::platform::RegularFile, Error> {
    super::platform::archive_prepared(archiver, cwd, input, prepared, process)
}

#[cfg(target_os = "macos")]
fn archive_prepared_for_test(
    archiver: &super::platform::Executable,
    cwd: &super::platform::Directory,
    input: &super::platform::RegularFile,
    prepared: super::platform::PreparedArchiveInvocation,
    process: &mut super::platform::PreparedProcessArena,
) -> Result<super::platform::RegularFile, Error> {
    super::platform::archive_prepared_settled(archiver, cwd, input, prepared, process)
        .map_err(|failure| failure.error)
}

#[test]
fn archive_header_parser_rejects_noncanonical_sizes_and_unknown_members() {
    use super::{
        archive_extended_name, archive_member_kind, archive_member_size,
        exact_archive_member_metadata, ArchiveMemberKind,
    };

    assert_eq!(archive_member_size(b"123       "), Ok(123));
    for invalid in [
        b"          ".as_slice(),
        b"+1        ".as_slice(),
        b"01x       ".as_slice(),
    ] {
        assert_eq!(archive_member_size(invalid), Err(Error::Invalid));
    }
    assert_eq!(
        archive_member_kind(b"module.o/       ", b"module.o"),
        Ok(ArchiveMemberKind::Input),
    );
    assert_eq!(
        archive_member_kind(b"/               ", b"module.o"),
        Ok(ArchiveMemberKind::GnuLinkerIndex),
    );
    assert_eq!(
        archive_member_kind(b"__.SYMDEF SORTED", b"module.o"),
        Ok(ArchiveMemberKind::BsdSortedLinkerIndex),
    );
    assert_eq!(
        archive_member_kind(b"foreign.o/      ", b"module.o"),
        Err(Error::Invalid),
    );
    assert_eq!(
        archive_member_kind(b"__.SYMDEF_EVIL  ", b"module.o"),
        Err(Error::Invalid),
    );
    assert_eq!(
        archive_member_kind(b"#1/8            ", b"module.o"),
        Ok(ArchiveMemberKind::Extended(8)),
    );
    assert_eq!(
        archive_extended_name(b"module.o\0\0\0\0"),
        Ok(b"module.o".as_slice()),
    );
    for invalid in [
        b"module.o".as_slice(),
        b"module.o\0\0\0".as_slice(),
        b"module.o\0x\0\0".as_slice(),
        b"module.o\0\0\0\0\0".as_slice(),
    ] {
        assert_eq!(archive_extended_name(invalid), Err(Error::Invalid));
    }

    #[cfg(target_os = "linux")]
    let metadata = [
        (ArchiveMemberKind::GnuLinkerIndex, b"0       ".as_slice()),
        (ArchiveMemberKind::Input, b"644     ".as_slice()),
    ];
    #[cfg(target_os = "macos")]
    let metadata = [
        (ArchiveMemberKind::Extended(20), b"100644  ".as_slice()),
        (ArchiveMemberKind::Extended(12), b"644     ".as_slice()),
    ];
    #[cfg(windows)]
    let metadata = [
        (ArchiveMemberKind::GnuLinkerIndex, b"0       ".as_slice()),
        (ArchiveMemberKind::Input, b"100666  ".as_slice()),
    ];
    for (kind, mode) in metadata {
        let mut header = [b' '; 60];
        #[cfg(not(windows))]
        header[16..28].copy_from_slice(b"0           ");
        #[cfg(windows)]
        header[16..28].copy_from_slice(b"-1          ");
        #[cfg(any(target_os = "linux", target_os = "macos"))]
        {
            header[28..34].copy_from_slice(b"0     ");
            header[34..40].copy_from_slice(b"0     ");
        }
        header[40..48].copy_from_slice(mode);
        #[cfg(target_os = "linux")]
        let input_mode = 0o600;
        #[cfg(not(target_os = "linux"))]
        let input_mode = 0o644;
        assert_eq!(
            exact_archive_member_metadata(&header, kind, input_mode),
            Ok(())
        );
        for offset in [16, 28, 34, 40] {
            let mut hostile = header;
            hostile[offset] = b'9';
            assert_eq!(
                exact_archive_member_metadata(&hostile, kind, input_mode),
                Err(Error::Invalid),
            );
        }
    }
    #[cfg(target_os = "macos")]
    {
        let mut header = [b' '; 60];
        header[16..28].copy_from_slice(b"0           ");
        header[28..34].copy_from_slice(b"0     ");
        header[34..40].copy_from_slice(b"0     ");
        header[40..48].copy_from_slice(b"100644  ");
        assert_eq!(
            exact_archive_member_metadata(&header, ArchiveMemberKind::Extended(12), 0o100600),
            Ok(()),
        );
        for hostile_mode in [
            b"100600  ".as_slice(),
            b"644     ".as_slice(),
            b"100666  ".as_slice(),
        ] {
            let mut hostile = header;
            hostile[40..48].copy_from_slice(hostile_mode);
            assert_eq!(
                exact_archive_member_metadata(&hostile, ArchiveMemberKind::Extended(12), 0o100600,),
                Err(Error::Invalid),
            );
        }
    }
    #[cfg(windows)]
    {
        let mut old_synthetic_date = [b' '; 60];
        old_synthetic_date[16..28].copy_from_slice(b"0           ");
        old_synthetic_date[40..48].copy_from_slice(b"0       ");
        assert_eq!(
            exact_archive_member_metadata(
                &old_synthetic_date,
                ArchiveMemberKind::GnuLinkerIndex,
                0,
            ),
            Err(Error::Invalid),
        );
    }
    #[cfg(target_os = "linux")]
    {
        let mut nondeterministic = [b' '; 60];
        nondeterministic[16..28].copy_from_slice(b"0           ");
        nondeterministic[28..34].copy_from_slice(b"0     ");
        nondeterministic[34..40].copy_from_slice(b"0     ");
        nondeterministic[40..48].copy_from_slice(b"600     ");
        assert_eq!(
            exact_archive_member_metadata(&nondeterministic, ArchiveMemberKind::Input, 0o600),
            Err(Error::Invalid),
        );
    }
}

#[cfg(target_os = "macos")]
fn append_darwin_archive_member(
    archive: &mut Vec<u8>,
    extended_name: &[u8],
    mode: &[u8; 8],
    data: &[u8],
) {
    assert!(extended_name.len().is_multiple_of(4));
    let mut header = [b' '; 60];
    let encoded_name = format!("#1/{}", extended_name.len());
    header[..encoded_name.len()].copy_from_slice(encoded_name.as_bytes());
    header[16..28].copy_from_slice(b"0           ");
    header[28..34].copy_from_slice(b"0     ");
    header[34..40].copy_from_slice(b"0     ");
    header[40..48].copy_from_slice(mode);
    let size = extended_name.len() + data.len();
    let encoded_size = size.to_string();
    header[48..48 + encoded_size.len()].copy_from_slice(encoded_size.as_bytes());
    header[58..].copy_from_slice(b"`\n");
    archive.extend_from_slice(&header);
    archive.extend_from_slice(extended_name);
    archive.extend_from_slice(data);
    if size & 1 != 0 {
        archive.push(b'\n');
    }
}

#[cfg(target_os = "macos")]
fn synthetic_darwin_archive(input: &[u8], input_mode: &[u8; 8]) -> Vec<u8> {
    let mut archive = b"!<arch>\n".to_vec();
    append_darwin_archive_member(&mut archive, b"__.SYMDEF SORTED\0\0\0\0", b"100644  ", b"");
    append_darwin_archive_member(&mut archive, b"module.o\0\0\0\0", input_mode, input);
    archive
}

#[cfg(target_os = "macos")]
#[test]
fn darwin_archive_admission_accepts_only_the_two_deterministic_input_modes() {
    use std::ffi::OsStr;
    use std::os::unix::fs::PermissionsExt as _;

    let root = std::env::temp_dir().join(format!(
        "semaprax-darwin-archive-mode-fixture-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos(),
    ));
    std::fs::create_dir(&root).unwrap();
    let input_bytes = b"exact-mach-o-object";
    std::fs::write(root.join("module.o"), input_bytes).unwrap();
    std::fs::set_permissions(
        root.join("module.o"),
        std::fs::Permissions::from_mode(0o600),
    )
    .unwrap();
    let root = std::fs::canonicalize(root).unwrap();
    let directory = super::platform::hold_directory(&root).unwrap();
    let input = super::platform::hold_regular_file(&directory, OsStr::new("module.o")).unwrap();

    for (index, mode) in [b"600     ", b"100644  "].into_iter().enumerate() {
        let name = format!("accepted-{index}.a");
        std::fs::write(
            root.join(&name),
            synthetic_darwin_archive(input_bytes, mode),
        )
        .unwrap();
        let archive = super::platform::hold_regular_file(&directory, OsStr::new(&name)).unwrap();
        super::platform::test_exact_archive_member(&archive, &input).unwrap();
    }

    for (index, mode) in [b"100600  ", b"644     ", b"100666  "]
        .into_iter()
        .enumerate()
    {
        let name = format!("rejected-{index}.a");
        std::fs::write(
            root.join(&name),
            synthetic_darwin_archive(input_bytes, mode),
        )
        .unwrap();
        let archive = super::platform::hold_regular_file(&directory, OsStr::new(&name)).unwrap();
        assert_eq!(
            super::platform::test_exact_archive_member(&archive, &input),
            Err(Error::Invalid),
        );
    }

    drop((input, directory));
    std::fs::remove_dir_all(root).unwrap();
}

#[cfg(target_os = "macos")]
#[test]
fn darwin_real_d_archive_is_exact_and_reproducible_across_tool_versions() {
    use std::ffi::OsStr;

    if std::env::var_os("SEMAPRAX_REQUIRE_DARWIN_REAL_ARCHIVE").as_deref() != Some(OsStr::new("1"))
    {
        return;
    }

    let source_root = std::env::temp_dir().join(format!(
        "semaprax-darwin-real-archive-source-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos(),
    ));
    std::fs::create_dir(&source_root).unwrap();
    std::fs::write(
        source_root.join("module.c"),
        b"int semaprax_darwin_real_archive_probe(void){return 17;}\n",
    )
    .unwrap();
    let compile = std::process::Command::new(
        std::env::var_os("CLANG").unwrap_or_else(|| "/usr/bin/clang".into()),
    )
    .current_dir(&source_root)
    .args(["-std=c11", "-O2", "-Wall", "-Wextra", "-Werror", "-c"])
    .arg("module.c")
    .args(["-o", "module.o"])
    .output()
    .unwrap();
    assert!(
        compile.status.success(),
        "{}",
        String::from_utf8_lossy(&compile.stderr)
    );
    let object = std::fs::read(source_root.join("module.o")).unwrap();
    std::fs::remove_dir_all(source_root).unwrap();

    let mut archives = Vec::new();
    let mut admitted_modes = Vec::new();
    for index in 0..2 {
        let root = std::env::temp_dir().join(format!(
            "semaprax-darwin-real-archive-{}-{index}",
            std::process::id(),
        ));
        std::fs::create_dir(&root).unwrap();
        let root = std::fs::canonicalize(root).unwrap();
        let directory = super::platform::hold_directory(&root).unwrap();
        let input =
            super::platform::write_file_new(&directory, OsStr::new("module.o"), &object, 0o600)
                .unwrap();
        let archiver =
            super::platform::hold_external_executable(std::path::Path::new("/usr/bin/libtool"))
                .unwrap();
        let prepared = super::platform::prepare_archive_invocation(
            OsStr::new("module.o"),
            OsStr::new("libsemaprax_native_rust_sdk.a"),
        )
        .unwrap();
        let mut process = super::platform::prepare_process_arena(1).unwrap();
        let archive = super::platform::archive_prepared_settled(
            &archiver,
            &directory,
            &input,
            prepared,
            &mut process,
        )
        .unwrap();
        let bytes = super::platform::read_exact(
            &archive,
            usize::try_from(super::SDK_ARCHIVE_MAX_BYTES).unwrap(),
        )
        .unwrap();
        let first_size =
            usize::try_from(super::archive_member_size(&bytes[56..66]).unwrap()).unwrap();
        let second_header = 68 + first_size + (first_size & 1);
        let mode = &bytes[second_header + 40..second_header + 48];
        assert!(
            mode == b"600     " || mode == b"100644  ",
            "Darwin -D emitted an unauthenticated input mode: {mode:?}",
        );
        admitted_modes.push(mode.to_vec());
        archives.push(bytes);
        drop((archive, archiver, input, directory, process));
        std::fs::remove_dir_all(root).unwrap();
    }
    assert_eq!(archives[0], archives[1]);
    assert_eq!(admitted_modes[0], admitted_modes[1]);
}

#[test]
fn sparse_oversize_archive_is_rejected_before_digest_io() {
    use std::ffi::OsStr;

    let root = std::env::temp_dir().join(format!(
        "semaprax-archive-oversize-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos(),
    ));
    std::fs::create_dir(&root).unwrap();
    let root = std::fs::canonicalize(root).unwrap();
    let name = OsStr::new("oversize.a");
    let file = std::fs::File::create(root.join(name)).unwrap();
    file.set_len(super::SDK_ARCHIVE_MAX_BYTES + 1).unwrap();
    drop(file);
    let directory = super::platform::hold_directory(&root).unwrap();
    let start = std::time::Instant::now();
    assert!(matches!(
        super::platform::test_hold_regular_file_name_bounded(
            &directory,
            name,
            super::SDK_ARCHIVE_MAX_BYTES,
        ),
        Err(Error::OutputLimit),
    ));
    assert!(start.elapsed() < std::time::Duration::from_secs(1));
    drop(directory);
    std::fs::remove_dir_all(root).unwrap();
}

#[cfg(any(target_os = "linux", target_os = "macos", windows))]
#[test]
fn archive_output_insertion_is_rejected_before_process_consumption_and_preserved() {
    use std::ffi::OsStr;

    let root = std::env::temp_dir().join(format!(
        "semaprax-archive-insertion-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos(),
    ));
    std::fs::create_dir(&root).unwrap();
    let root = std::fs::canonicalize(root).unwrap();
    let cwd = super::platform::hold_directory(&root).unwrap();
    #[cfg(unix)]
    let (input_name, output_name) = (
        OsStr::new("module.o"),
        OsStr::new("libsemaprax_native_rust_sdk.a"),
    );
    #[cfg(windows)]
    let (input_name, output_name) = (
        OsStr::new("module.obj"),
        OsStr::new("semaprax_native_rust_sdk.lib"),
    );
    let input = super::platform::write_file_new(&cwd, input_name, b"owned-object", 0o600).unwrap();
    let foreign =
        super::platform::write_file_new(&cwd, output_name, b"foreign-must-survive", 0o600).unwrap();
    let executable_path = std::env::current_exe().unwrap();
    let executable = super::platform::hold_external_executable(&executable_path).unwrap();
    let prepared = super::platform::prepare_archive_invocation(input_name, output_name).unwrap();
    let mut process = super::platform::prepare_process_arena(1).unwrap();
    assert!(matches!(
        archive_prepared_for_test(&executable, &cwd, &input, prepared, &mut process,),
        Err(Error::Exists)
    ));
    assert_eq!(
        super::platform::prepared_process_arena_remaining(&process),
        1
    );
    assert_eq!(
        super::platform::read_exact(&foreign, 64).unwrap(),
        b"foreign-must-survive",
    );
    drop((foreign, input, executable, cwd, process));
    std::fs::remove_dir_all(&root).unwrap();
}

#[cfg(unix)]
#[test]
fn archive_nonregular_output_insertions_are_exactly_present_and_never_followed() {
    use std::ffi::OsStr;
    use std::os::unix::fs::symlink;

    let root = std::env::temp_dir().join(format!(
        "semaprax-archive-nonregular-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos(),
    ));
    std::fs::create_dir(&root).unwrap();
    let root = std::fs::canonicalize(root).unwrap();
    let cwd = super::platform::hold_directory(&root).unwrap();
    let input =
        super::platform::write_file_new(&cwd, OsStr::new("module.o"), b"owned-object", 0o600)
            .unwrap();
    let executable =
        super::platform::hold_external_executable(&std::env::current_exe().unwrap()).unwrap();
    let output = root.join("libsemaprax_native_rust_sdk.a");
    let foreign = root.join("foreign-target");
    std::fs::write(&foreign, b"foreign-must-survive").unwrap();
    symlink(&foreign, &output).unwrap();
    let prepared = super::platform::prepare_archive_invocation(
        OsStr::new("module.o"),
        OsStr::new("libsemaprax_native_rust_sdk.a"),
    )
    .unwrap();
    let mut process = super::platform::prepare_process_arena(1).unwrap();
    assert_eq!(
        archive_prepared_for_test(&executable, &cwd, &input, prepared, &mut process,).err(),
        Some(Error::Exists),
    );
    assert_eq!(
        super::platform::prepared_process_arena_remaining(&process),
        1
    );
    assert_eq!(std::fs::read(&foreign).unwrap(), b"foreign-must-survive");
    std::fs::remove_file(&output).unwrap();
    std::fs::create_dir(&output).unwrap();
    let prepared = super::platform::prepare_archive_invocation(
        OsStr::new("module.o"),
        OsStr::new("libsemaprax_native_rust_sdk.a"),
    )
    .unwrap();
    assert_eq!(
        archive_prepared_for_test(&executable, &cwd, &input, prepared, &mut process,).err(),
        Some(Error::Exists),
    );
    assert_eq!(
        super::platform::prepared_process_arena_remaining(&process),
        1
    );
    drop((input, executable, cwd, process));
    std::fs::remove_dir_all(&root).unwrap();
}

#[cfg(target_os = "macos")]
#[test]
fn darwin_archive_scratch_open_failure_reports_created_namespace_as_uncertain() {
    use std::ffi::OsStr;

    let root = std::env::temp_dir().join(format!(
        "semaprax-darwin-archive-scratch-open-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos(),
    ));
    std::fs::create_dir(&root).unwrap();
    let root = std::fs::canonicalize(root).unwrap();
    let cwd = super::platform::hold_directory(&root).unwrap();
    let input =
        super::platform::write_file_new(&cwd, OsStr::new("module.o"), b"owned-object", 0o600)
            .unwrap();
    let executable =
        super::platform::hold_external_executable(&std::env::current_exe().unwrap()).unwrap();
    let prepared = super::platform::prepare_archive_invocation(
        OsStr::new("module.o"),
        OsStr::new("libsemaprax_native_rust_sdk.a"),
    )
    .unwrap();
    let mut process = super::platform::prepare_process_arena(1).unwrap();
    super::platform::test_reset_archive_later_actions();
    super::platform::test_inject_archive_scratch_open_failure(true);
    let failure = super::platform::archive_prepared_settled(
        &executable,
        &cwd,
        &input,
        prepared,
        &mut process,
    )
    .err()
    .expect("injected post-mkdir open failure must fail");
    super::platform::test_inject_archive_scratch_open_failure(false);
    assert_eq!(
        failure.phase,
        super::DarwinArchiveFailurePhase::ScratchCreation
    );
    assert_eq!(
        failure.settlement,
        super::DarwinArchiveSettlement::Uncertain
    );
    assert_eq!(super::platform::test_archive_later_actions(), 0);
    assert_eq!(
        super::platform::prepared_process_arena_remaining(&process),
        1
    );
    assert!(root.join("archive-tmp").is_dir());
    assert!(!root.join("libsemaprax_native_rust_sdk.a").exists());

    drop((input, executable, cwd, process));
    std::fs::remove_dir_all(root).unwrap();
}

#[cfg(target_os = "macos")]
#[test]
fn darwin_every_post_process_failure_is_absorbing_and_preserves_namespace_bytes() {
    use super::platform::TestDarwinArchiveFailurePoint as Point;
    use super::DarwinArchiveFailurePhase as Phase;
    use std::ffi::OsStr;
    use std::os::unix::fs::PermissionsExt;

    let cases = [
        (Point::ProcessOutput, Phase::ProcessOutput, 0, true),
        (Point::ScratchCleanup, Phase::ScratchCleanup, 0, true),
        (
            Point::ArchiverRecheckBeforeHold,
            Phase::ArchiverRecheck,
            1,
            false,
        ),
        (
            Point::WorkingDirectoryRecheckBeforeHold,
            Phase::WorkingDirectoryRecheck,
            2,
            false,
        ),
        (Point::InputRecheckBeforeHold, Phase::InputRecheck, 3, false),
        (Point::OutputHold, Phase::OutputHold, 4, false),
        (Point::ExactArchive, Phase::ExactArchive, 5, false),
        (
            Point::ArchiverRecheckAfterAuthentication,
            Phase::ArchiverRecheck,
            6,
            false,
        ),
        (Point::LaunchPathRecheck, Phase::LaunchPathRecheck, 7, false),
        (
            Point::WorkingDirectoryRecheckAfterAuthentication,
            Phase::WorkingDirectoryRecheck,
            8,
            false,
        ),
        (
            Point::InputRecheckAfterAuthentication,
            Phase::InputRecheck,
            9,
            false,
        ),
        (Point::OutputRecheck, Phase::OutputRecheck, 10, false),
    ];

    for (point, phase, expected_actions, scratch_remains) in cases {
        let root = std::env::temp_dir().join(format!(
            "semaprax-darwin-archive-boundary-{}-{}-{point:?}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos(),
        ));
        std::fs::create_dir(&root).unwrap();
        std::fs::write(
            root.join("module.c"),
            b"int semaprax_archive_boundary_probe(void){return 11;}\n",
        )
        .unwrap();
        let clang = std::env::var_os("CLANG").unwrap_or_else(|| "/usr/bin/clang".into());
        let compile = std::process::Command::new(clang)
            .current_dir(&root)
            .args(["-c", "module.c", "-o", "module.o"])
            .output()
            .unwrap();
        assert!(
            compile.status.success(),
            "{point:?}: {}",
            String::from_utf8_lossy(&compile.stderr)
        );
        std::fs::set_permissions(
            root.join("module.o"),
            std::fs::Permissions::from_mode(0o600),
        )
        .unwrap();

        let root = std::fs::canonicalize(root).unwrap();
        let cwd = super::platform::hold_directory(&root).unwrap();
        let input = super::platform::test_hold_regular_file_name_bounded(
            &cwd,
            OsStr::new("module.o"),
            super::SDK_ARCHIVE_MAX_BYTES,
        )
        .unwrap();
        let executable =
            super::platform::hold_external_executable(std::path::Path::new("/usr/bin/libtool"))
                .unwrap();
        let prepared = super::platform::prepare_archive_invocation(
            OsStr::new("module.o"),
            OsStr::new("libsemaprax_native_rust_sdk.a"),
        )
        .unwrap();
        let mut process = super::platform::prepare_process_arena(1).unwrap();
        super::platform::test_reset_archive_later_actions();
        super::platform::test_inject_darwin_archive_failure(Some(point));
        let failure = super::platform::archive_prepared_settled(
            &executable,
            &cwd,
            &input,
            prepared,
            &mut process,
        )
        .err()
        .expect("injected post-process boundary must fail");
        super::platform::test_inject_darwin_archive_failure(None);
        assert_eq!(failure.phase, phase, "{point:?}");
        assert_eq!(
            failure.settlement,
            super::DarwinArchiveSettlement::Uncertain,
            "{point:?}"
        );
        assert_eq!(
            super::platform::test_archive_later_actions(),
            expected_actions,
            "{point:?} performed an action after the selected boundary"
        );
        assert_eq!(root.join("archive-tmp").is_dir(), scratch_remains);
        let archive = root.join("libsemaprax_native_rust_sdk.a");
        assert!(archive.is_file(), "{point:?} lost the archiver output");
        let preserved = root.join("authenticated-or-unsettled-archive");
        std::fs::rename(&archive, &preserved).unwrap();
        std::fs::write(&archive, b"foreign-must-survive").unwrap();
        assert!(preserved.is_file());
        assert_eq!(std::fs::read(&archive).unwrap(), b"foreign-must-survive");

        drop((input, executable, cwd, process));
        std::fs::remove_dir_all(root).unwrap();
    }
}

#[cfg(target_os = "macos")]
#[test]
fn darwin_archive_process_failure_is_absorbing_after_the_effect_boundary() {
    use std::ffi::OsStr;

    let root = std::env::temp_dir().join(format!(
        "semaprax-darwin-archive-settlement-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos(),
    ));
    std::fs::create_dir(&root).unwrap();
    let root = std::fs::canonicalize(root).unwrap();
    let cwd = super::platform::hold_directory(&root).unwrap();
    let input =
        super::platform::write_file_new(&cwd, OsStr::new("module.o"), b"owned-object", 0o600)
            .unwrap();
    // The test image is held exactly but is not an archiver. Once it has been
    // invoked, its namespace effects cannot be inferred from its exit status.
    let executable =
        super::platform::hold_external_executable(&std::env::current_exe().unwrap()).unwrap();
    let prepared = super::platform::prepare_archive_invocation(
        OsStr::new("module.o"),
        OsStr::new("libsemaprax_native_rust_sdk.a"),
    )
    .unwrap();
    let mut process = super::platform::prepare_process_arena(1).unwrap();
    super::platform::test_reset_archive_later_actions();
    let failure = super::platform::archive_prepared_settled(
        &executable,
        &cwd,
        &input,
        prepared,
        &mut process,
    )
    .err()
    .expect("non-archiver test image must fail");
    assert_eq!(failure.phase, super::DarwinArchiveFailurePhase::Process);
    assert_eq!(
        failure.settlement,
        super::DarwinArchiveSettlement::Uncertain
    );
    assert_eq!(super::platform::test_archive_later_actions(), 0);
    assert!(root.join("archive-tmp").is_dir());

    drop((input, executable, cwd, process));
    std::fs::remove_dir_all(root).unwrap();
}

#[cfg(target_os = "macos")]
#[test]
fn darwin_authenticated_archive_is_preserved_on_later_rejection() {
    use std::ffi::OsStr;
    use std::os::unix::fs::PermissionsExt;

    let root = std::env::temp_dir().join(format!(
        "semaprax-darwin-archive-owned-rejection-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos(),
    ));
    std::fs::create_dir(&root).unwrap();
    std::fs::write(
        root.join("module.c"),
        b"int semaprax_archive_settlement_probe(void){return 7;}\n",
    )
    .unwrap();
    let clang = std::env::var_os("CLANG").unwrap_or_else(|| "/usr/bin/clang".into());
    let compile = std::process::Command::new(clang)
        .current_dir(&root)
        .args(["-c", "module.c", "-o", "module.o"])
        .output()
        .unwrap();
    assert!(
        compile.status.success(),
        "{}",
        String::from_utf8_lossy(&compile.stderr)
    );
    std::fs::set_permissions(
        root.join("module.o"),
        std::fs::Permissions::from_mode(0o600),
    )
    .unwrap();

    let root = std::fs::canonicalize(root).unwrap();
    let cwd = super::platform::hold_directory(&root).unwrap();
    let input = super::platform::test_hold_regular_file_name_bounded(
        &cwd,
        OsStr::new("module.o"),
        super::SDK_ARCHIVE_MAX_BYTES,
    )
    .unwrap();
    let executable =
        super::platform::hold_external_executable(std::path::Path::new("/usr/bin/libtool"))
            .unwrap();
    let prepared = super::platform::prepare_archive_invocation(
        OsStr::new("module.o"),
        OsStr::new("libsemaprax_native_rust_sdk.a"),
    )
    .unwrap();
    let mut process = super::platform::prepare_process_arena(1).unwrap();
    super::platform::test_inject_archive_post_authentication_failure(true);
    let failure = super::platform::archive_prepared_settled(
        &executable,
        &cwd,
        &input,
        prepared,
        &mut process,
    )
    .err()
    .expect("injected post-authentication rejection must fail");
    super::platform::test_inject_archive_post_authentication_failure(false);
    assert_eq!(
        failure.phase,
        super::DarwinArchiveFailurePhase::ArchiverRecheck
    );
    assert_eq!(
        failure.settlement,
        super::DarwinArchiveSettlement::Uncertain
    );
    let archive = root.join("libsemaprax_native_rust_sdk.a");
    assert!(archive.is_file());
    assert!(!root.join("archive-tmp").exists());
    let preserved = root.join("authenticated-archive-preserved");
    std::fs::rename(&archive, &preserved).unwrap();
    std::fs::write(&archive, b"foreign-must-survive").unwrap();
    assert!(preserved.is_file());
    assert_eq!(std::fs::read(&archive).unwrap(), b"foreign-must-survive");

    drop((input, executable, cwd, process));
    std::fs::remove_dir_all(root).unwrap();
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
#[test]
fn unix_archive_preparation_is_fixed_to_one_sdk_object() {
    use std::ffi::OsStr;

    for (input, output) in [
        ("../module.o", "libsemaprax_native_rust_sdk.a"),
        ("module.o", "../libsemaprax_native_rust_sdk.a"),
        ("foreign.o", "libsemaprax_native_rust_sdk.a"),
        ("module.o", "foreign.a"),
    ] {
        assert!(matches!(
            super::platform::prepare_archive_invocation(OsStr::new(input), OsStr::new(output),),
            Err(Error::Invalid)
        ));
    }
    let prepared = super::platform::prepare_archive_invocation(
        OsStr::new("module.o"),
        OsStr::new("libsemaprax_native_rust_sdk.a"),
    )
    .unwrap();
    assert!(super::platform::prepared_archive_owned_capacity(&prepared) > 0);
    let arguments = super::platform::test_prepared_archive_arguments(&prepared);
    #[cfg(target_os = "linux")]
    assert_eq!(
        arguments,
        [
            b"rcsD".as_slice(),
            b"libsemaprax_native_rust_sdk.a",
            b"module.o"
        ]
    );
    #[cfg(target_os = "macos")]
    assert_eq!(
        arguments,
        [
            b"-static".as_slice(),
            b"-D",
            b"-o",
            b"libsemaprax_native_rust_sdk.a",
            b"module.o",
        ],
    );
}

#[cfg(windows)]
#[test]
fn windows_archive_preparation_is_fixed_to_one_sdk_object() {
    use std::ffi::OsStr;

    for (input, output) in [
        (r"..\module.obj", "semaprax_native_rust_sdk.lib"),
        ("module.obj", r"..\semaprax_native_rust_sdk.lib"),
        ("foreign.obj", "semaprax_native_rust_sdk.lib"),
        ("module.obj", "foreign.lib"),
    ] {
        assert!(matches!(
            super::platform::prepare_archive_invocation(OsStr::new(input), OsStr::new(output),),
            Err(Error::Invalid)
        ));
    }
    let prepared = super::platform::prepare_archive_invocation(
        OsStr::new("module.obj"),
        OsStr::new("semaprax_native_rust_sdk.lib"),
    )
    .unwrap();
    assert!(super::platform::prepared_archive_owned_capacity(&prepared) > 0);
    assert_eq!(
        super::platform::test_prepared_archive_arguments(&prepared),
        [
            "/NOLOGO",
            "/BREPRO",
            "/OUT:semaprax_native_rust_sdk.lib",
            "module.obj",
        ],
    );
}

#[cfg(windows)]
fn append_windows_archive_member(archive: &mut Vec<u8>, name: &[u8], mode: &[u8; 8], data: &[u8]) {
    assert!(name.len() <= 16);
    let mut header = [b' '; 60];
    header[..name.len()].copy_from_slice(name);
    header[16..28].copy_from_slice(b"-1          ");
    header[40..48].copy_from_slice(mode);
    let encoded_size = data.len().to_string();
    header[48..48 + encoded_size.len()].copy_from_slice(encoded_size.as_bytes());
    header[58..].copy_from_slice(b"`\n");
    archive.extend_from_slice(&header);
    archive.extend_from_slice(data);
    if data.len() & 1 != 0 {
        archive.push(b'\n');
    }
}

#[cfg(windows)]
fn synthetic_windows_archive(input: &[u8], empty_longnames: bool) -> Vec<u8> {
    let mut archive = b"!<arch>\n".to_vec();
    append_windows_archive_member(&mut archive, b"/", b"0       ", b"");
    append_windows_archive_member(&mut archive, b"/", b"0       ", b"");
    if empty_longnames {
        append_windows_archive_member(&mut archive, b"//", b"0       ", b"");
    }
    append_windows_archive_member(&mut archive, b"module.obj/", b"100666  ", input);
    archive
}

#[cfg(windows)]
#[test]
fn windows_archive_admission_is_closed_over_the_two_brepro_layouts() {
    use std::ffi::OsStr;

    let root = std::env::temp_dir().join(format!(
        "semaprax-windows-archive-fixture-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos(),
    ));
    std::fs::create_dir(&root).unwrap();
    let root = std::fs::canonicalize(root).unwrap();
    let input_bytes = b"exact-coff-object";
    std::fs::write(root.join("module.obj"), input_bytes).unwrap();
    let directory = super::platform::hold_directory(&root).unwrap();
    let input = super::platform::hold_regular_file(&directory, OsStr::new("module.obj")).unwrap();

    for (index, bytes) in [
        synthetic_windows_archive(input_bytes, false),
        synthetic_windows_archive(input_bytes, true),
    ]
    .into_iter()
    .enumerate()
    {
        let name = format!("accepted-{index}.lib");
        std::fs::write(root.join(&name), bytes).unwrap();
        let archive = super::platform::hold_regular_file(&directory, OsStr::new(&name)).unwrap();
        super::platform::test_exact_archive_member(&archive, &input).unwrap();
    }

    let mut hostile = Vec::new();

    let mut nonempty_longnames = b"!<arch>\n".to_vec();
    append_windows_archive_member(&mut nonempty_longnames, b"/", b"0       ", b"");
    append_windows_archive_member(&mut nonempty_longnames, b"/", b"0       ", b"");
    append_windows_archive_member(&mut nonempty_longnames, b"//", b"0       ", b"module.obj\0");
    append_windows_archive_member(
        &mut nonempty_longnames,
        b"module.obj/",
        b"100666  ",
        input_bytes,
    );
    hostile.push(nonempty_longnames);

    let mut indirect_name = b"!<arch>\n".to_vec();
    append_windows_archive_member(&mut indirect_name, b"/", b"0       ", b"");
    append_windows_archive_member(&mut indirect_name, b"/", b"0       ", b"");
    append_windows_archive_member(&mut indirect_name, b"//", b"0       ", b"");
    append_windows_archive_member(&mut indirect_name, b"/0", b"100666  ", input_bytes);
    hostile.push(indirect_name);

    let mut hybrid = b"!<arch>\n".to_vec();
    append_windows_archive_member(&mut hybrid, b"/", b"0       ", b"");
    append_windows_archive_member(&mut hybrid, b"/", b"0       ", b"");
    append_windows_archive_member(&mut hybrid, b"/<HYBRIDMAP>/", b"0       ", b"");
    append_windows_archive_member(&mut hybrid, b"module.obj/", b"100666  ", input_bytes);
    hostile.push(hybrid);

    let mut duplicate = synthetic_windows_archive(input_bytes, false);
    append_windows_archive_member(&mut duplicate, b"module.obj/", b"100666  ", input_bytes);
    hostile.push(duplicate);

    let mut foreign = synthetic_windows_archive(input_bytes, false);
    append_windows_archive_member(&mut foreign, b"foreign.obj/", b"100666  ", input_bytes);
    hostile.push(foreign);

    let mut reordered = b"!<arch>\n".to_vec();
    append_windows_archive_member(&mut reordered, b"module.obj/", b"100666  ", input_bytes);
    append_windows_archive_member(&mut reordered, b"/", b"0       ", b"");
    append_windows_archive_member(&mut reordered, b"/", b"0       ", b"");
    hostile.push(reordered);

    for (index, bytes) in hostile.into_iter().enumerate() {
        let name = format!("rejected-{index}.lib");
        std::fs::write(root.join(&name), bytes).unwrap();
        let archive = super::platform::hold_regular_file(&directory, OsStr::new(&name)).unwrap();
        assert_eq!(
            super::platform::test_exact_archive_member(&archive, &input),
            Err(Error::Invalid),
        );
    }

    drop((input, directory));
    std::fs::remove_dir_all(root).unwrap();
}

#[cfg(windows)]
#[test]
fn windows_c_compile_plan_disables_incremental_linker_compatible_timestamps() {
    use std::ffi::OsStr;

    let prepared = super::platform::prepare_c_compile_invocation(
        "x86_64-pc-windows-msvc",
        OsStr::new("module.c"),
        2,
        false,
        33_554_432,
    )
    .unwrap();
    let expected = [
        "-std=c11",
        "-target",
        "x86_64-pc-windows-msvc",
        "-mno-incremental-linker-compatible",
        "-Wall",
        "-Wextra",
        "-Werror",
        "-O2",
        "-c",
        "module.c",
        "-o",
        "-",
    ];
    let (arguments, capacity) = super::platform::test_prepared_c_compile_arguments(&prepared);
    assert!(arguments.iter().map(String::as_str).eq(expected));
    assert_eq!(capacity, expected.len());
}

#[cfg(windows)]
#[test]
fn windows_real_brepro_archive_round_trips_through_exact_admission() {
    use sha2::{Digest as _, Sha256};
    use std::ffi::OsStr;
    use std::path::Path;

    let required = std::env::var_os("SEMAPRAX_REQUIRE_WINDOWS_REAL_ARCHIVE").as_deref()
        == Some(OsStr::new("1"));
    let archiver = std::env::var_os("SEMAPRAX_ARCHIVER");
    let vctools = std::env::var_os("SEMAPRAX_VCTOOLS");
    let clang = std::env::var_os("CLANG");
    if required {
        assert!(
            archiver.is_some(),
            "SEMAPRAX_REQUIRE_WINDOWS_REAL_ARCHIVE=1 requires SEMAPRAX_ARCHIVER"
        );
        assert!(
            vctools.is_some(),
            "SEMAPRAX_REQUIRE_WINDOWS_REAL_ARCHIVE=1 requires SEMAPRAX_VCTOOLS"
        );
        assert!(
            clang.is_some(),
            "SEMAPRAX_REQUIRE_WINDOWS_REAL_ARCHIVE=1 requires CLANG"
        );
    }
    let (Some(archiver), Some(vctools), Some(clang)) = (archiver, vctools, clang) else {
        return;
    };
    let archiver = std::path::PathBuf::from(archiver);
    let vctools = std::path::PathBuf::from(vctools);
    assert_eq!(
        archiver.strip_prefix(&vctools).unwrap(),
        Path::new(r"bin\Hostx64\x64\lib.exe"),
    );

    let root = std::env::temp_dir().join(format!(
        "semaprax-windows-real-archive-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos(),
    ));
    std::fs::create_dir(&root).unwrap();
    let root = std::fs::canonicalize(root).unwrap();
    std::fs::write(
        root.join("module.c"),
        b"int semaprax_archive_probe(void){return 7;}\n",
    )
    .unwrap();
    let directory = super::platform::hold_directory(&root).unwrap();
    let clang = super::platform::hold_external_executable(std::path::Path::new(&clang)).unwrap();
    let mut compile_process = super::platform::prepare_process_arena(2).unwrap();
    let compile = || {
        super::platform::prepare_c_compile_invocation(
            "x86_64-pc-windows-msvc",
            OsStr::new("module.c"),
            2,
            false,
            usize::try_from(super::SDK_ARCHIVE_MAX_BYTES).unwrap(),
        )
        .unwrap()
    };
    let first =
        super::platform::compile_c_prepared(&clang, &directory, compile(), &mut compile_process)
            .unwrap();
    let second =
        super::platform::compile_c_prepared(&clang, &directory, compile(), &mut compile_process)
            .unwrap();
    let digest = |bytes: &[u8]| {
        use std::fmt::Write as _;

        let mut rendered = String::with_capacity(64);
        for byte in Sha256::digest(bytes) {
            write!(&mut rendered, "{byte:02x}").unwrap();
        }
        rendered
    };
    assert_eq!(
        first,
        second,
        "production Windows C objects differ: first_sha256={} second_sha256={}",
        digest(&first),
        digest(&second),
    );
    assert!(first.len() >= 8, "COFF object is shorter than its header");
    assert_eq!(
        &first[4..8],
        &[0, 0, 0, 0],
        "reproducible COFF TimeDateStamp must be zero"
    );
    let owned_input =
        super::platform::write_file_new(&directory, OsStr::new("module.obj"), &first, 0o600)
            .unwrap();
    let input_names = super::platform::prepare_discard_names([OsStr::new("module.obj")]).unwrap();
    let input = super::platform::transition_regular_file_to_external_read_prepared(
        &directory,
        &input_names,
        0,
        &owned_input,
    )
    .unwrap();
    drop(owned_input);
    let archiver_image = archiver;
    let archiver = super::platform::hold_external_executable(&archiver_image).unwrap();
    let prepared = super::platform::prepare_archive_invocation(
        OsStr::new("module.obj"),
        OsStr::new("semaprax_native_rust_sdk.lib"),
    )
    .unwrap();
    let mut process = super::platform::prepare_process_arena(1).unwrap();
    let start = std::time::Instant::now();
    let archive =
        super::platform::archive_prepared(&archiver, &directory, &input, prepared, &mut process);
    let elapsed = start.elapsed();
    let archive = archive.unwrap_or_else(|error| {
        let captured =
            String::from_utf8_lossy(&super::platform::test_last_captured_stdout()).into_owned();
        let verbatim = root.as_os_str().to_string_lossy().into_owned();
        let plain_root = verbatim
            .strip_prefix(r"\\?\")
            .map(std::borrow::ToOwned::to_owned)
            .unwrap_or_else(|| verbatim.clone());
        let clean_root = std::path::PathBuf::from(&plain_root);
        let absolute_input = format!("{plain_root}\\module.obj");
        std::fs::copy(root.join("module.obj"), root.join("m2.obj")).ok();
        let rendered = |name: &str, attempt: &std::io::Result<std::process::Output>| {
            attempt
                .as_ref()
                .map(|output| {
                    format!(
                        "{name}:exit={} stdout={:?} stderr={:?}",
                        output.status.code().unwrap_or(-1),
                        String::from_utf8_lossy(&output.stdout),
                        String::from_utf8_lossy(&output.stderr),
                    )
                })
                .unwrap_or_else(|error| format!("{name}:spawn-failed:{error:?}"))
        };
        let spawn = |cwd: &std::path::Path, arguments: &[&str]| {
            std::process::Command::new(&archiver_image)
                .current_dir(cwd)
                .args(arguments)
                .output()
        };
        let repo_cwd = std::env::current_dir().unwrap_or_else(|_| clean_root.clone());
        let probes = [
            rendered(
                "E_relative_brepro",
                &spawn(
                    &clean_root,
                    &[
                        "/NOLOGO",
                        "/BREPRO",
                        "/OUT:semaprax_probe_e.lib",
                        "module.obj",
                    ],
                ),
            ),
            rendered(
                "F_relative_nobrepro",
                &spawn(
                    &clean_root,
                    &["/NOLOGO", "/OUT:semaprax_probe_f.lib", "module.obj"],
                ),
            ),
            rendered(
                "G_copy_m2",
                &spawn(
                    &clean_root,
                    &["/NOLOGO", "/BREPRO", "/OUT:semaprax_probe_g.lib", "m2.obj"],
                ),
            ),
            rendered(
                "H_abs_from_repo_cwd",
                &spawn(
                    &repo_cwd,
                    &[
                        "/NOLOGO",
                        "/BREPRO",
                        &format!("/OUT:{plain_root}\\semaprax_probe_h.lib"),
                        absolute_input.as_str(),
                    ],
                ),
            ),
            rendered("I_cmd_type_read", &{
                let mut command = std::process::Command::new("cmd.exe");
                command
                    .current_dir(&clean_root)
                    .args(["/c", "cd", "&", "type", "module.obj"]);
                command.output()
            }),
        ];
        let plain_probe = probes.join(" | ");
        panic!(
            "{} strict_captured={captured:?}",
            windows_real_archive_failure_evidence(
                &directory,
                &input,
                OsStr::new("semaprax_native_rust_sdk.lib"),
                &root.join("semaprax_native_rust_sdk.lib"),
                error,
                elapsed,
                &plain_probe,
            )
        )
    });
    assert!(start.elapsed() < std::time::Duration::from_secs(5));
    super::platform::test_exact_archive_member(&archive, &input).unwrap();

    drop((
        archive,
        archiver,
        clang,
        input,
        directory,
        process,
        compile_process,
    ));
    std::fs::remove_dir_all(root).unwrap();
}

#[cfg(windows)]
fn windows_real_archive_failure_evidence(
    directory: &super::platform::Directory,
    input: &super::platform::RegularFile,
    output_name: &std::ffi::OsStr,
    archive: &std::path::Path,
    error: Error,
    elapsed: std::time::Duration,
    plain_probe: &str,
) -> String {
    use sha2::{Digest as _, Sha256};
    use std::fmt::Write as _;
    use std::io::{Read as _, Seek as _, SeekFrom};

    const MEMBER_CAP: usize = 8;
    const PREVIEW_CAP: usize = 64;
    const HASH_BYTE_CAP: u64 = super::SDK_ARCHIVE_MAX_BYTES;
    const DIAGNOSTIC_BYTE_CAP: usize = 16_384;

    fn hex(bytes: &[u8]) -> String {
        use std::fmt::Write as _;

        let mut rendered = String::with_capacity(bytes.len() * 2);
        for byte in bytes {
            write!(rendered, "{byte:02x}").unwrap();
        }
        rendered
    }

    fn escaped(bytes: &[u8]) -> String {
        use std::fmt::Write as _;

        let mut rendered = String::with_capacity(bytes.len() * 4);
        for byte in bytes {
            match byte {
                b' '..=b'~' if *byte != b'\\' => rendered.push(char::from(*byte)),
                b'\\' => rendered.push_str("\\\\"),
                _ => write!(rendered, "\\x{byte:02x}").unwrap(),
            }
        }
        rendered
    }

    let metadata = std::fs::metadata(archive).ok();
    let exists = metadata.is_some();
    let length = metadata.as_ref().map_or(0, std::fs::Metadata::len);
    let exact_replay = if !exists {
        "absent".to_owned()
    } else {
        match super::platform::hold_regular_file(directory, output_name) {
            Ok(output) => match super::platform::test_exact_archive_member(&output, input) {
                Ok(()) => "replay_ok".to_owned(),
                Err(replay) => format!("output_replay_err:{replay:?}"),
            },
            Err(replay) => format!("output_replay_err:hold:{replay:?}"),
        }
    };
    let mut evidence = format!(
        "Windows real archive admission failed: error={error:?} elapsed_ms={} output_exists={exists} output_length={length} exact_replay={exact_replay}",
        elapsed.as_millis(),
    );
    evidence.push_str(" | ");
    evidence.push_str(plain_probe);
    let Ok(mut file) = std::fs::File::open(archive) else {
        return evidence;
    };

    let hashed_bytes = length.min(HASH_BYTE_CAP);
    let mut hasher = Sha256::new();
    let mut remaining = hashed_bytes;
    let mut buffer = [0_u8; 4096];
    while remaining != 0 {
        let requested = usize::try_from(remaining.min(buffer.len() as u64)).unwrap();
        let read = file.read(&mut buffer[..requested]).unwrap_or(0);
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
        remaining -= read as u64;
    }
    let digest = hasher.finalize();
    write!(
        evidence,
        " hash_bytes={} sha256={}",
        hashed_bytes - remaining,
        hex(&digest)
    )
    .unwrap();

    if file.seek(SeekFrom::Start(0)).is_ok() {
        let mut magic = [0_u8; 8];
        if file.read_exact(&mut magic).is_ok() {
            write!(
                evidence,
                " magic_hex={} magic_escaped={}",
                hex(&magic),
                escaped(&magic)
            )
            .unwrap();
            let mut offset = 8_u64;
            for index in 0..MEMBER_CAP {
                let mut header = [0_u8; 60];
                if file.seek(SeekFrom::Start(offset)).is_err()
                    || file.read_exact(&mut header).is_err()
                {
                    break;
                }
                let parsed_size = super::archive_member_size(&header[48..58]);
                let payload_size = parsed_size.as_ref().copied().unwrap_or(0);
                let preview_length = usize::try_from(payload_size.min(PREVIEW_CAP as u64)).unwrap();
                let mut preview = [0_u8; PREVIEW_CAP];
                let preview_read = file.read(&mut preview[..preview_length]).unwrap_or(0);
                write!(
                    evidence,
                    "\nmember[{index}] offset={offset} header_hex={} name_hex={} name_escaped={} timestamp_hex={} timestamp_escaped={} owner_hex={} owner_escaped={} group_hex={} group_escaped={} mode_hex={} mode_escaped={} size_hex={} size_escaped={} end_hex={} end_escaped={} parsed_size={parsed_size:?} preview_len={preview_read} preview_hex={} preview_escaped={}",
                    hex(&header),
                    hex(&header[0..16]),
                    escaped(&header[0..16]),
                    hex(&header[16..28]),
                    escaped(&header[16..28]),
                    hex(&header[28..34]),
                    escaped(&header[28..34]),
                    hex(&header[34..40]),
                    escaped(&header[34..40]),
                    hex(&header[40..48]),
                    escaped(&header[40..48]),
                    hex(&header[48..58]),
                    escaped(&header[48..58]),
                    hex(&header[58..60]),
                    escaped(&header[58..60]),
                    hex(&preview[..preview_read]),
                    escaped(&preview[..preview_read]),
                )
                .unwrap();
                if parsed_size.is_err() {
                    break;
                }
                let Some(next) = offset
                    .checked_add(60)
                    .and_then(|value| value.checked_add(payload_size))
                    .and_then(|value| value.checked_add(payload_size & 1))
                else {
                    break;
                };
                if next <= offset || next > length {
                    break;
                }
                offset = next;
            }
        }
    }
    evidence.truncate(DIAGNOSTIC_BYTE_CAP);
    evidence
}

#[cfg(windows)]
#[test]
#[ignore = "legacy negative-control probe intentionally retains descendant file and directory authorities; the safe platform test windows_settled_nested_inventory_publishes_after_descendant_authorities_close is the blocking settled-publication regression"]
fn windows_mixed_root_inventory_replays_before_and_after_exact_directory_rename() {
    use std::ffi::OsStr;

    let root = std::env::temp_dir().join(format!(
        "semaprax-windows-mixed-inventory-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos(),
    ));
    std::fs::create_dir(&root).unwrap();
    let root = std::fs::canonicalize(root).unwrap();
    let parent = super::platform::test_hold_directory_owned(&root).unwrap();
    let stage = super::platform::create_directory_new(&parent, OsStr::new("stage"), 0o700).unwrap();
    let source =
        super::platform::create_directory_new(&stage, OsStr::new("source"), 0o700).unwrap();
    let native =
        super::platform::create_directory_new(&stage, OsStr::new("native"), 0o700).unwrap();

    let root_files = [
        super::platform::write_file_new(&stage, OsStr::new("Cargo.toml"), b"cargo", 0o600).unwrap(),
        super::platform::write_file_new(&stage, OsStr::new("build.rs"), b"build", 0o600).unwrap(),
        super::platform::write_file_new(&stage, OsStr::new("sdk.json"), b"sdk", 0o600).unwrap(),
    ];
    let source_files = [
        super::platform::write_file_new(&source, OsStr::new("lib.rs"), b"lib", 0o600).unwrap(),
        super::platform::write_file_new(&source, OsStr::new("ffi.rs"), b"ffi", 0o600).unwrap(),
        super::platform::write_file_new(&source, OsStr::new("api.rs"), b"api", 0o600).unwrap(),
    ];
    let native_files = [
        super::platform::write_file_new(&native, OsStr::new("sdk.lib"), b"archive", 0o600).unwrap(),
        super::platform::write_file_new(
            &native,
            OsStr::new("descriptor.json"),
            b"descriptor",
            0o600,
        )
        .unwrap(),
        super::platform::write_file_new(&native, OsStr::new("manifest.json"), b"manifest", 0o600)
            .unwrap(),
    ];

    let authenticate = || {
        let mut root_inventory = super::platform::prepare_inventory_entries_exact(
            [
                OsStr::new("Cargo.toml"),
                OsStr::new("build.rs"),
                OsStr::new("sdk.json"),
                OsStr::new("source"),
                OsStr::new("native"),
            ],
            3,
        )
        .unwrap();
        super::platform::inventory_entries_exact_prepared(
            &mut root_inventory,
            &stage,
            [&root_files[0], &root_files[1], &root_files[2]],
            [&source, &native],
        )
        .unwrap();
        let mut source_inventory = super::platform::prepare_inventory_entries_exact(
            [
                OsStr::new("lib.rs"),
                OsStr::new("ffi.rs"),
                OsStr::new("api.rs"),
            ],
            3,
        )
        .unwrap();
        super::platform::inventory_entries_exact_prepared(
            &mut source_inventory,
            &source,
            [&source_files[0], &source_files[1], &source_files[2]],
            [],
        )
        .unwrap();
        let mut native_inventory = super::platform::prepare_inventory_entries_exact(
            [
                OsStr::new("sdk.lib"),
                OsStr::new("descriptor.json"),
                OsStr::new("manifest.json"),
            ],
            3,
        )
        .unwrap();
        super::platform::inventory_entries_exact_prepared(
            &mut native_inventory,
            &native,
            [&native_files[0], &native_files[1], &native_files[2]],
            [],
        )
        .unwrap();
    };
    authenticate();

    let mut stage_name = super::platform::prepare_relative_name_arena(9).unwrap();
    super::platform::set_relative_name_arena(&mut stage_name, OsStr::new("stage")).unwrap();
    let identity_probe =
        super::platform::test_publish_stage_identity_probe(&parent, &stage, &stage_name);
    let mut publish = super::platform::prepare_publish_directory(OsStr::new("published")).unwrap();
    super::platform::publish_directory_new_prepared(
        &mut publish,
        &parent,
        &stage,
        &stage_name,
        OsStr::new("published"),
    )
    .unwrap_or_else(|error| {
        let statuses = super::platform::test_last_publish_statuses();
        let std_probe = std::fs::rename(root.join("stage"), root.join("published_std_probe"));
        std::fs::create_dir(root.join("probe_empty")).ok();
        let empty_sibling =
            std::fs::rename(root.join("probe_empty"), root.join("probe_empty_moved"));
        std::fs::create_dir(root.join("stage2")).ok();
        std::fs::write(root.join("stage2").join("leaf.txt"), b"leaf").ok();
        let unheld_child =
            std::fs::rename(root.join("stage2"), root.join("stage2_moved"));
        let held_file_dir = root.join("probe_held_file");
        std::fs::create_dir(&held_file_dir).ok();
        let held_file_directory = super::platform::hold_directory(&held_file_dir).unwrap();
        let held_leaf = super::platform::write_file_new(
            &held_file_directory,
            OsStr::new("leaf.bin"),
            b"leaf",
            0o600,
        )
        .unwrap();
        let held_file_result =
            std::fs::rename(&held_file_dir, root.join("probe_held_file_moved"));
        drop(held_leaf);
        let mut std_reopen_result: Result<(), std::io::Error> = Ok(());
        let mut std_reopened_leaf = None;
        if held_file_result.is_err() {
            std_reopened_leaf = std::fs::File::open(held_file_dir.join("leaf.bin")).ok();
            std_reopen_result =
                std::fs::rename(&held_file_dir, root.join("probe_held_std_moved"));
        }
        drop(std_reopened_leaf);
        drop(held_file_directory);
        let held_dir_parent = root.join("probe_held_dir");
        std::fs::create_dir(&held_dir_parent).ok();
        std::fs::create_dir(held_dir_parent.join("child")).ok();
        let held_child = super::platform::hold_directory(&held_dir_parent.join("child"))
            .expect("hold child directory");
        let held_dir_result = std::fs::rename(&held_dir_parent, root.join("probe_held_dir_moved"));
        drop(held_child);
        let retry_probe_start = std::time::Instant::now();
        let mut retry_probe = Err(std::io::Error::other("not attempted"));
        let mut retry_attempts = 0_usize;
        while retry_probe_start.elapsed() < std::time::Duration::from_secs(2) {
            retry_attempts += 1;
            retry_probe =
                std::fs::rename(root.join("stage"), root.join("published_retry_probe"));
            if retry_probe.is_ok() {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(25));
        }
        panic!(
            "directory publication failed: {error:?} ({identity_probe}) statuses={statuses:?} std_rename={std_probe:?} empty_sibling={empty_sibling:?} unheld_child={unheld_child:?} held_file={held_file_result:?} std_reopen={std_reopen_result:?} held_child_dir={held_dir_result:?} retry_after={}ms attempts={retry_attempts} last={retry_probe:?}",
            retry_probe_start.elapsed().as_millis()
        )
    });
    assert!(!root.join("stage").exists());
    assert!(super::platform::same_directory_path(&stage, &root.join("published")).unwrap());
    super::platform::recheck_directory(&parent).unwrap();
    super::platform::recheck_directory(&stage).unwrap();
    super::platform::recheck_directory(&source).unwrap();
    super::platform::recheck_directory(&native).unwrap();
    for file in root_files.iter().chain(&source_files).chain(&native_files) {
        super::platform::recheck_regular(file).unwrap();
    }
    authenticate();

    let source_names = super::platform::prepare_discard_names([
        OsStr::new("lib.rs"),
        OsStr::new("ffi.rs"),
        OsStr::new("api.rs"),
    ])
    .unwrap();
    let mut source_name = super::platform::prepare_relative_name_arena(6).unwrap();
    super::platform::set_relative_name_arena(&mut source_name, OsStr::new("source")).unwrap();
    super::platform::discard_owned_stage_prepared(
        &stage,
        &source,
        &source_name,
        &source_names,
        &[
            Some(&source_files[0]),
            Some(&source_files[1]),
            Some(&source_files[2]),
        ],
        &[None, None, None],
        #[cfg(debug_assertions)]
        None,
    )
    .unwrap();
    let native_names = super::platform::prepare_discard_names([
        OsStr::new("sdk.lib"),
        OsStr::new("descriptor.json"),
        OsStr::new("manifest.json"),
    ])
    .unwrap();
    let mut native_name = super::platform::prepare_relative_name_arena(6).unwrap();
    super::platform::set_relative_name_arena(&mut native_name, OsStr::new("native")).unwrap();
    super::platform::discard_owned_stage_prepared(
        &stage,
        &native,
        &native_name,
        &native_names,
        &[
            Some(&native_files[0]),
            Some(&native_files[1]),
            Some(&native_files[2]),
        ],
        &[None, None, None],
        #[cfg(debug_assertions)]
        None,
    )
    .unwrap();

    let root_names = super::platform::prepare_discard_names([
        OsStr::new("Cargo.toml"),
        OsStr::new("build.rs"),
        OsStr::new("sdk.json"),
    ])
    .unwrap();
    super::platform::set_relative_name_arena(&mut stage_name, OsStr::new("published")).unwrap();
    super::platform::discard_owned_stage_prepared(
        &parent,
        &stage,
        &stage_name,
        &root_names,
        &[
            Some(&root_files[0]),
            Some(&root_files[1]),
            Some(&root_files[2]),
        ],
        &[None, None, None],
        #[cfg(debug_assertions)]
        None,
    )
    .unwrap();
    assert!(!root.join("published").exists());

    drop((
        root_names,
        native_name,
        native_names,
        source_name,
        source_names,
        publish,
        stage_name,
        native_files,
        source_files,
        root_files,
        native,
        source,
        stage,
        parent,
    ));
    std::fs::remove_dir(&root).unwrap();
}

#[cfg(target_os = "linux")]
fn linux_runner_failure_helper(
    points: &[TestSettlementFailure],
    expected: Option<Error>,
    sentinel: &str,
) {
    let Some(root) = std::env::var_os("SEMAPRAX_SYS_TEST_HELPER_ROOT") else {
        return;
    };
    set_test_settlement_failures(points);
    let root = std::path::PathBuf::from(root);
    let directory = super::platform::hold_directory(&root).unwrap();
    let executable =
        super::platform::hold_executable(&directory, std::ffi::OsStr::new("noisy")).unwrap();
    let result =
        super::platform::execute_harness_with_output_limit(&executable, &directory, 65_536);
    if let Some(expected) = expected {
        assert_eq!(result, Err(expected));
    }
    std::fs::write(root.join(sentinel), b"returned").unwrap();
}

#[cfg(target_os = "linux")]
macro_rules! linux_runner_helper {
        ($name:ident, [$($point:ident),+], $expected:expr, $sentinel:literal) => {
            #[test]
            fn $name() {
                linux_runner_failure_helper(
                    &[$(TestSettlementFailure::$point),+],
                    $expected,
                    $sentinel,
                );
            }
        };
    }

#[cfg(target_os = "linux")]
linux_runner_helper!(
    helper_linux_pipe_read_fcntl,
    [UnixPipeReadFcntl],
    Some(Error::Spawn),
    "settled"
);
#[cfg(target_os = "linux")]
linux_runner_helper!(
    helper_linux_pipe_write_fcntl,
    [UnixPipeWriteFcntl],
    Some(Error::Spawn),
    "settled"
);
#[cfg(target_os = "linux")]
linux_runner_helper!(
    helper_linux_drain_fcntl,
    [UnixDrainFcntl],
    Some(Error::Spawn),
    "settled"
);
#[cfg(target_os = "linux")]
linux_runner_helper!(helper_linux_poll, [UnixPoll], Some(Error::Spawn), "settled");
#[cfg(target_os = "linux")]
linux_runner_helper!(helper_linux_read, [UnixRead], Some(Error::Spawn), "settled");
#[cfg(target_os = "linux")]
linux_runner_helper!(
    helper_linux_read_conversion,
    [UnixReadConversion],
    Some(Error::OutputLimit),
    "settled"
);
#[cfg(target_os = "linux")]
linux_runner_helper!(
    helper_linux_waitpid,
    [UnixWaitpid],
    Some(Error::Spawn),
    "settled"
);
#[cfg(target_os = "linux")]
linux_runner_helper!(
    helper_linux_deadline,
    [UnixDeadline],
    Some(Error::Spawn),
    "settled"
);
#[cfg(target_os = "linux")]
linux_runner_helper!(
    helper_linux_parent_write_close,
    [UnixParentWriteClose],
    None,
    "post-fail-stop"
);
#[cfg(target_os = "linux")]
linux_runner_helper!(
    helper_linux_parent_null_close,
    [UnixParentNullClose],
    None,
    "post-fail-stop"
);
#[cfg(target_os = "linux")]
linux_runner_helper!(
    helper_linux_settle_close,
    [UnixDrainFcntl, UnixSettleClose],
    None,
    "post-fail-stop"
);
#[cfg(target_os = "linux")]
linux_runner_helper!(
    helper_linux_success_read_close,
    [UnixSuccessReadClose],
    None,
    "post-fail-stop"
);
#[cfg(target_os = "linux")]
linux_runner_helper!(
    helper_linux_wait_settlement,
    [UnixDrainFcntl, UnixWait],
    None,
    "post-fail-stop"
);
#[cfg(target_os = "linux")]
linux_runner_helper!(
    helper_linux_group_settlement,
    [UnixDrainFcntl, UnixGroup],
    None,
    "post-fail-stop"
);

#[cfg(target_os = "linux")]
#[test]
fn linux_runner_boundaries_settle_or_fail_stop_without_later_action() {
    use std::os::unix::process::ExitStatusExt as _;
    use std::process::Command;

    let parent = std::fs::canonicalize(std::env::temp_dir()).unwrap();
    let root = parent.join(format!(
        "semaprax-sys-runner-boundaries-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir(&root).unwrap();
    let source = root.join("noisy.c");
    std::fs::write(
            &source,
            "#include <stdio.h>\n#include <unistd.h>\nint main(void){FILE *f=fopen(\"leader.pid\",\"w\");if(!f)return 2;fprintf(f,\"%ld\",(long)getpid());fclose(f);if(write(1,\"x\",1)!=1)return 2;sleep(1);return 0;}\n",
        )
        .unwrap();
    let compiler = std::env::var_os("CC").unwrap_or_else(|| "cc".into());
    let built = Command::new(compiler)
        .env("TMPDIR", &root)
        .args(["-std=c11", "-Wall", "-Wextra", "-Werror", "-O2"])
        .arg(&source)
        .arg("-o")
        .arg(root.join("noisy"))
        .output()
        .unwrap();
    assert!(
        built.status.success(),
        "{}",
        String::from_utf8_lossy(&built.stderr)
    );
    let current = std::env::current_exe().unwrap();
    for helper in [
        "tests::helper_linux_pipe_read_fcntl",
        "tests::helper_linux_pipe_write_fcntl",
        "tests::helper_linux_drain_fcntl",
        "tests::helper_linux_poll",
        "tests::helper_linux_read",
        "tests::helper_linux_read_conversion",
        "tests::helper_linux_waitpid",
        "tests::helper_linux_deadline",
    ] {
        let sentinel = root.join("settled");
        let _ = std::fs::remove_file(&sentinel);
        let status = Command::new(&current)
            .env("SEMAPRAX_SYS_TEST_HELPER_ROOT", &root)
            .args(["--exact", helper, "--nocapture"])
            .status()
            .unwrap();
        assert!(status.success(), "settled boundary failed: {helper}");
        assert!(
            sentinel.exists(),
            "settled boundary did not return: {helper}"
        );
    }
    for helper in [
        "tests::helper_linux_parent_write_close",
        "tests::helper_linux_parent_null_close",
        "tests::helper_linux_settle_close",
        "tests::helper_linux_success_read_close",
        "tests::helper_linux_wait_settlement",
        "tests::helper_linux_group_settlement",
    ] {
        let sentinel = root.join("post-fail-stop");
        let _ = std::fs::remove_file(&sentinel);
        let status = Command::new(&current)
            .env("SEMAPRAX_SYS_TEST_HELPER_ROOT", &root)
            .args(["--exact", helper, "--nocapture"])
            .status()
            .unwrap();
        assert!(!status.success(), "fail-stop boundary returned: {helper}");
        assert!(
            status.signal().is_some(),
            "fail-stop did not abort: {helper}"
        );
        assert!(
            !sentinel.exists(),
            "later action ran after fail-stop: {helper}"
        );
    }
    std::fs::remove_dir_all(&root).unwrap();
}

#[cfg(target_os = "linux")]
#[test]
fn linux_archive_runner_does_not_wait_for_foreign_pipe_holders_and_settles_owned_output() {
    use std::ffi::OsStr;
    use std::process::Command;

    let root = std::env::temp_dir().join(format!(
        "semaprax-sys-archive-pipe-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir(&root).unwrap();
    let source = root.join("archiver.c");
    std::fs::write(
        &source,
        "#define _GNU_SOURCE\n#include <stdio.h>\n#include <stdlib.h>\n#include <unistd.h>\nint main(int n,char **v){if(n!=4)return 2;FILE *f=fopen(v[2],\"wb\");if(!f)return 3;if(fwrite(\"!<arch>\\n\",1,8,f)!=8||fclose(f))return 4;int ready[2];if(pipe(ready))return 5;pid_t p=fork();if(p<0)return 6;if(!p){if(close(ready[0])||setsid()<0)_exit(7);f=fopen(\"failure-holder.pid\",\"w\");if(!f)_exit(8);if(fprintf(f,\"%ld\",(long)getpid())<=0||fclose(f))_exit(9);if(write(ready[1],\"x\",1)!=1||close(ready[1]))_exit(10);for(;;)pause();}if(close(ready[1]))return 11;char byte=0;if(read(ready[0],&byte,1)!=1||byte!='x'||close(ready[0]))return 12;return 7;}\n",
    )
    .unwrap();
    let built = Command::new(std::env::var_os("CC").unwrap_or_else(|| "cc".into()))
        .args(["-std=c11", "-Wall", "-Wextra", "-Werror", "-O2"])
        .arg(&source)
        .arg("-o")
        .arg(root.join("archiver"))
        .output()
        .unwrap();
    assert!(
        built.status.success(),
        "{}",
        String::from_utf8_lossy(&built.stderr)
    );

    let root = std::fs::canonicalize(root).unwrap();
    let directory = super::platform::hold_directory(&root).unwrap();
    let input = super::platform::write_file_new(
        &directory,
        OsStr::new("module.o"),
        b"not-consumed-after-exit",
        0o600,
    )
    .unwrap();
    let archiver = super::platform::hold_executable(&directory, OsStr::new("archiver")).unwrap();
    let prepared = super::platform::prepare_archive_invocation(
        OsStr::new("module.o"),
        OsStr::new("libsemaprax_native_rust_sdk.a"),
    )
    .unwrap();
    let mut process = super::platform::prepare_process_arena(1).unwrap();
    let start = std::time::Instant::now();
    let result =
        super::platform::archive_prepared(&archiver, &directory, &input, prepared, &mut process);
    let holder = std::fs::read_to_string(root.join("failure-holder.pid")).unwrap();
    let holder = holder.parse::<libc::pid_t>().unwrap();
    let mut holder = LinuxForeignHolderGuard::new(holder);
    holder.assert_alive();
    assert!(matches!(result, Err(Error::Exit)));
    assert!(start.elapsed() < std::time::Duration::from_secs(5));
    assert!(!root.join("libsemaprax_native_rust_sdk.a").exists());
    holder.settle();
    drop((archiver, input, directory, process));
    std::fs::remove_dir_all(root).unwrap();
}

#[cfg(target_os = "linux")]
fn compile_linux_archive_test_tool(
    root: &std::path::Path,
    name: &str,
    source: &str,
) -> std::path::PathBuf {
    use std::process::Command;

    let source_path = root.join(format!("{name}.c"));
    let output_path = root.join(name);
    std::fs::write(&source_path, source).unwrap();
    let built = Command::new(std::env::var_os("CC").unwrap_or_else(|| "cc".into()))
        .args(["-std=c11", "-Wall", "-Wextra", "-Werror", "-O2"])
        .arg(&source_path)
        .arg("-o")
        .arg(&output_path)
        .output()
        .unwrap();
    assert!(
        built.status.success(),
        "{}",
        String::from_utf8_lossy(&built.stderr)
    );
    output_path
}

#[cfg(target_os = "linux")]
fn linux_archive_test_root(label: &str) -> std::path::PathBuf {
    let root = std::env::temp_dir().join(format!(
        "semaprax-sys-{label}-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir(&root).unwrap();
    root
}

#[cfg(target_os = "linux")]
struct LinuxForeignHolderGuard {
    pid: libc::pid_t,
}

#[cfg(target_os = "linux")]
impl LinuxForeignHolderGuard {
    fn new(pid: libc::pid_t) -> Self {
        assert!(pid > 1, "unsafe foreign stdout holder pid");
        assert_eq!(
            unsafe { libc::kill(pid, 0) },
            0,
            "foreign holder is not live"
        );
        Self { pid }
    }

    fn assert_alive(&self) {
        assert!(self.pid > 1, "foreign stdout holder already settled");
        assert_eq!(unsafe { libc::kill(self.pid, 0) }, 0);
    }

    fn settle(&mut self) {
        assert!(self.pid > 1, "unsafe foreign stdout holder pid");
        let _ = unsafe { libc::kill(self.pid, libc::SIGKILL) };
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
        loop {
            let alive = unsafe { libc::kill(self.pid, 0) } == 0;
            if !alive && std::io::Error::last_os_error().raw_os_error() == Some(libc::ESRCH) {
                self.pid = 0;
                return;
            }
            assert!(
                std::time::Instant::now() < deadline,
                "foreign stdout holder did not settle"
            );
            std::thread::sleep(std::time::Duration::from_millis(1));
        }
    }
}

#[cfg(target_os = "linux")]
impl Drop for LinuxForeignHolderGuard {
    fn drop(&mut self) {
        if self.pid > 1 {
            let _ = unsafe { libc::kill(self.pid, libc::SIGKILL) };
        }
    }
}

#[cfg(target_os = "linux")]
#[test]
fn linux_archive_seed_is_exactly_initialized_held_and_removed() {
    use std::ffi::OsStr;

    let root = linux_archive_test_root("archive-seed");
    let root = std::fs::canonicalize(root).unwrap();
    let directory = super::platform::hold_directory(&root).unwrap();
    super::platform::test_archive_seed_round_trip(&directory, OsStr::new("owned.a")).unwrap();
    assert!(!root.join("owned.a").exists());
    drop(directory);
    std::fs::remove_dir_all(root).unwrap();
}

#[cfg(target_os = "linux")]
#[test]
fn linux_real_archive_succeeds_without_waiting_for_foreign_pipe_holder() {
    use std::ffi::OsStr;
    use std::os::unix::fs::PermissionsExt as _;
    use std::process::Command;

    let root = linux_archive_test_root("archive-success-pipe");
    let object_source = root.join("module.c");
    std::fs::write(
        &object_source,
        "int semaprax_archive_probe(void){return 7;}\n",
    )
    .unwrap();
    let object = Command::new(std::env::var_os("CC").unwrap_or_else(|| "cc".into()))
        .args(["-std=c11", "-Wall", "-Wextra", "-Werror", "-O2", "-c"])
        .arg(&object_source)
        .arg("-o")
        .arg(root.join("module.o"))
        .output()
        .unwrap();
    assert!(
        object.status.success(),
        "{}",
        String::from_utf8_lossy(&object.stderr)
    );
    std::fs::set_permissions(
        root.join("module.o"),
        std::fs::Permissions::from_mode(0o600),
    )
    .unwrap();
    let real_archiver = std::env::var("SEMAPRAX_ARCHIVER")
        .unwrap_or_else(|_| "/usr/bin/x86_64-linux-gnu-ar".to_owned());
    assert!(std::path::Path::new(&real_archiver).is_absolute());
    assert!(std::path::Path::new(&real_archiver).is_file());
    let c_archiver = real_archiver.replace('\\', "\\\\").replace('"', "\\\"");
    compile_linux_archive_test_tool(
        &root,
        "archive-wrapper",
        &format!(
            "#define _GNU_SOURCE\n#include <stdio.h>\n#include <stdlib.h>\n#include <sys/stat.h>\n#include <unistd.h>\nint main(int n,char **v){{if(n!=4)return 2;struct stat s;if(stat(v[2],&s))return 3;FILE *seed=fopen(\"seed.ino\",\"w\");if(!seed)return 4;if(fprintf(seed,\"%llu\",(unsigned long long)s.st_ino)<=0||fclose(seed))return 5;int ready[2];if(pipe(ready))return 6;pid_t p=fork();if(p<0)return 7;if(!p){{if(close(ready[0])||setsid()<0)_exit(8);FILE *f=fopen(\"holder.pid\",\"w\");if(!f)_exit(9);if(fprintf(f,\"%ld\",(long)getpid())<=0||fclose(f))_exit(10);if(write(ready[1],\"x\",1)!=1||close(ready[1]))_exit(11);for(;;)pause();}}if(close(ready[1]))return 12;char byte=0;if(read(ready[0],&byte,1)!=1||byte!='x'||close(ready[0]))return 13;v[0]=\"{c_archiver}\";execv(\"{c_archiver}\",v);return 14;}}\n"
        ),
    );

    let root = std::fs::canonicalize(root).unwrap();
    let directory = super::platform::hold_directory(&root).unwrap();
    let input = super::platform::hold_regular_file(&directory, OsStr::new("module.o")).unwrap();
    let (input_mode, _, _) = super::platform::test_regular_file_facts(&input);
    assert_eq!(input_mode & 0o777, 0o600);
    let archiver =
        super::platform::hold_executable(&directory, OsStr::new("archive-wrapper")).unwrap();
    let prepared = super::platform::prepare_archive_invocation(
        OsStr::new("module.o"),
        OsStr::new("libsemaprax_native_rust_sdk.a"),
    )
    .unwrap();
    let mut process = super::platform::prepare_process_arena(1).unwrap();
    let start = std::time::Instant::now();
    let archive =
        super::platform::archive_prepared(&archiver, &directory, &input, prepared, &mut process);
    let holder = std::fs::read_to_string(root.join("holder.pid")).unwrap();
    let holder = holder.parse::<libc::pid_t>().unwrap();
    let mut holder = LinuxForeignHolderGuard::new(holder);
    holder.assert_alive();
    let archive = archive.unwrap();
    let seed_ino = std::fs::read_to_string(root.join("seed.ino"))
        .unwrap()
        .parse::<u64>()
        .unwrap();
    let (_, _, archive_ino) = super::platform::test_regular_file_facts(&archive);
    assert_eq!(archive_ino, seed_ino);
    super::platform::test_exact_archive_member(&archive, &input).unwrap();
    assert!(start.elapsed() < std::time::Duration::from_secs(5));
    assert!(root.join("libsemaprax_native_rust_sdk.a").is_file());
    holder.settle();
    drop((archive, archiver, input, directory, process));
    std::fs::remove_dir_all(root).unwrap();
}

#[cfg(target_os = "linux")]
#[test]
fn linux_archive_cleanup_preserves_replaced_foreign_inode_and_fails_closed() {
    use std::ffi::OsStr;

    let root = linux_archive_test_root("archive-foreign-inode");
    compile_linux_archive_test_tool(
        &root,
        "replace-archive",
        "#define _GNU_SOURCE\n#include <stdio.h>\n#include <unistd.h>\nint main(int n,char **v){if(n!=4)return 2;if(unlink(v[2]))return 3;FILE *f=fopen(v[2],\"wb\");if(!f)return 4;if(fwrite(\"foreign-must-survive\",1,20,f)!=20||fclose(f))return 5;return 7;}\n",
    );
    std::fs::write(root.join("module.o"), b"input").unwrap();
    let root = std::fs::canonicalize(root).unwrap();
    let directory = super::platform::hold_directory(&root).unwrap();
    let input = super::platform::hold_regular_file(&directory, OsStr::new("module.o")).unwrap();
    let archiver =
        super::platform::hold_executable(&directory, OsStr::new("replace-archive")).unwrap();
    let prepared = super::platform::prepare_archive_invocation(
        OsStr::new("module.o"),
        OsStr::new("libsemaprax_native_rust_sdk.a"),
    )
    .unwrap();
    let mut process = super::platform::prepare_process_arena(1).unwrap();
    assert!(matches!(
        super::platform::archive_prepared(&archiver, &directory, &input, prepared, &mut process,),
        Err(Error::Changed)
    ));
    assert_eq!(
        std::fs::read(root.join("libsemaprax_native_rust_sdk.a")).unwrap(),
        b"foreign-must-survive"
    );
    drop((archiver, input, directory, process));
    std::fs::remove_dir_all(root).unwrap();
}

#[cfg(target_os = "macos")]
fn darwin_failure_helper(points: &[TestSettlementFailure]) {
    let Some(root) = std::env::var_os("SEMAPRAX_SYS_TEST_HELPER_ROOT") else {
        return;
    };
    set_test_settlement_failures(points);
    let root = std::path::PathBuf::from(root);
    let directory = super::platform::hold_directory(&root).unwrap();
    let executable =
        super::platform::hold_executable(&directory, std::ffi::OsStr::new("quiet")).unwrap();
    let _ = super::platform::execute_harness(&executable, &directory);
    std::fs::write(root.join("post-fail-stop"), b"returned").unwrap();
}

#[cfg(target_os = "macos")]
fn darwin_returning_failure_helper(point: TestSettlementFailure, expected: Error) {
    let Some(root) = std::env::var_os("SEMAPRAX_SYS_TEST_HELPER_ROOT") else {
        return;
    };
    set_test_settlement_failures(&[point]);
    let root = std::path::PathBuf::from(root);
    let directory = super::platform::hold_directory(&root).unwrap();
    let executable =
        super::platform::hold_executable(&directory, std::ffi::OsStr::new("quiet")).unwrap();
    assert_eq!(
        super::platform::execute_harness(&executable, &directory),
        Err(expected)
    );
    std::fs::write(root.join("post-return"), b"returned").unwrap();
}

#[cfg(target_os = "macos")]
#[test]
fn helper_darwin_actions_destroy() {
    darwin_failure_helper(&[TestSettlementFailure::DarwinActionsDestroy]);
}

#[cfg(target_os = "macos")]
#[test]
fn helper_darwin_attributes_destroy() {
    darwin_failure_helper(&[TestSettlementFailure::DarwinAttributesDestroy]);
}

#[cfg(target_os = "macos")]
#[test]
fn helper_darwin_attest_settlement_fail_stop() {
    darwin_failure_helper(&[
        TestSettlementFailure::DarwinAttest,
        TestSettlementFailure::UnixWait,
    ]);
}

#[cfg(target_os = "macos")]
#[test]
fn helper_darwin_sigcont_settlement_fail_stop() {
    darwin_failure_helper(&[
        TestSettlementFailure::DarwinSigcont,
        TestSettlementFailure::UnixGroup,
    ]);
}

#[cfg(target_os = "macos")]
#[test]
fn helper_darwin_attest_returns_changed_after_settlement() {
    darwin_returning_failure_helper(TestSettlementFailure::DarwinAttest, Error::Changed);
}

#[cfg(target_os = "macos")]
#[test]
fn helper_darwin_sigcont_returns_spawn_after_settlement() {
    darwin_returning_failure_helper(TestSettlementFailure::DarwinSigcont, Error::Spawn);
}

#[cfg(target_os = "macos")]
#[test]
fn darwin_spawn_resource_destroy_uncertainty_fail_stops_without_later_action() {
    use std::os::unix::process::ExitStatusExt as _;
    use std::process::Command;

    let parent = std::fs::canonicalize(std::env::temp_dir()).unwrap();
    let root = parent.join(format!(
        "semaprax-sys-darwin-destroy-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir(&root).unwrap();
    let source = root.join("quiet.c");
    std::fs::write(&source, "int main(void){return 0;}\n").unwrap();
    let compiler = std::env::var_os("CC").unwrap_or_else(|| "/usr/bin/cc".into());
    let built = Command::new(compiler)
        .env_clear()
        .env("TMPDIR", &root)
        .env("PATH", "/usr/bin:/bin")
        .args(["-std=c11", "-Wall", "-Wextra", "-Werror", "-O2"])
        .arg(&source)
        .arg("-o")
        .arg(root.join("quiet"))
        .output()
        .unwrap();
    assert!(
        built.status.success(),
        "{}",
        String::from_utf8_lossy(&built.stderr)
    );
    let current = std::env::current_exe().unwrap();
    for helper in [
        "tests::helper_darwin_attest_returns_changed_after_settlement",
        "tests::helper_darwin_sigcont_returns_spawn_after_settlement",
    ] {
        let sentinel = root.join("post-return");
        let _ = std::fs::remove_file(&sentinel);
        let status = Command::new(&current)
            .env("SEMAPRAX_SYS_TEST_HELPER_ROOT", &root)
            .args(["--exact", helper, "--nocapture"])
            .status()
            .unwrap();
        assert!(
            status.success(),
            "settled operation did not return: {helper}"
        );
        assert!(
            sentinel.exists(),
            "post-return sentinel missing after settled operation: {helper}"
        );
    }
    for helper in [
        "tests::helper_darwin_actions_destroy",
        "tests::helper_darwin_attributes_destroy",
        "tests::helper_darwin_attest_settlement_fail_stop",
        "tests::helper_darwin_sigcont_settlement_fail_stop",
    ] {
        let sentinel = root.join("post-fail-stop");
        let _ = std::fs::remove_file(&sentinel);
        let status = Command::new(&current)
            .env("SEMAPRAX_SYS_TEST_HELPER_ROOT", &root)
            .args(["--exact", helper, "--nocapture"])
            .status()
            .unwrap();
        assert!(!status.success(), "destroy uncertainty returned: {helper}");
        assert!(
            status.signal().is_some(),
            "destroy uncertainty did not abort: {helper}"
        );
        assert!(
            !sentinel.exists(),
            "later action ran after destroy uncertainty"
        );
    }
    std::fs::remove_dir_all(&root).unwrap();
}

#[cfg(target_os = "windows")]
fn windows_runner_failure_helper(
    points: &[TestSettlementFailure],
    executable_name: &str,
    expected: Option<Error>,
    bounded_output: bool,
    sentinel: &str,
) {
    let Some(root) = std::env::var_os("SEMAPRAX_SYS_TEST_HELPER_ROOT") else {
        return;
    };
    set_test_settlement_failures(points);
    let root = std::path::PathBuf::from(root);
    let directory = super::platform::hold_directory(&root).unwrap();
    let executable =
        super::platform::hold_executable(&directory, std::ffi::OsStr::new(executable_name))
            .unwrap();
    let result = if bounded_output {
        super::platform::clang_version(&executable, &directory, 64).map(|_| ())
    } else {
        super::platform::execute_harness(&executable, &directory)
    };
    if let Some(expected) = expected {
        assert_eq!(result, Err(expected));
    }
    std::fs::write(root.join(sentinel), b"returned").unwrap();
}

#[cfg(target_os = "windows")]
macro_rules! windows_runner_helper {
        ($name:ident, [$($point:ident),+], $exe:literal, $expected:expr, $bounded:expr, $sentinel:literal) => {
            #[test]
            fn $name() {
                windows_runner_failure_helper(
                    &[$(TestSettlementFailure::$point),+],
                    $exe,
                    $expected,
                    $bounded,
                    $sentinel,
                );
            }
        };
    }

#[cfg(target_os = "windows")]
windows_runner_helper!(
    helper_windows_image,
    [WindowsImage],
    "quiet.exe",
    Some(Error::Changed),
    false,
    "settled"
);
#[cfg(target_os = "windows")]
windows_runner_helper!(
    helper_windows_assign,
    [WindowsAssign],
    "quiet.exe",
    Some(Error::Changed),
    false,
    "settled"
);
#[cfg(target_os = "windows")]
windows_runner_helper!(
    helper_windows_resume,
    [WindowsResume],
    "quiet.exe",
    Some(Error::Spawn),
    false,
    "settled"
);
#[cfg(target_os = "windows")]
windows_runner_helper!(
    helper_windows_peek,
    [WindowsPeek],
    "quiet.exe",
    Some(Error::Spawn),
    false,
    "settled"
);
#[cfg(target_os = "windows")]
windows_runner_helper!(
    helper_windows_read,
    [WindowsRead],
    "output.exe",
    Some(Error::Spawn),
    true,
    "settled"
);
#[cfg(target_os = "windows")]
windows_runner_helper!(
    helper_windows_unassigned_fail_stop,
    [WindowsImage, WindowsTerminateProcess],
    "quiet.exe",
    None,
    false,
    "post-fail-stop"
);
#[cfg(target_os = "windows")]
windows_runner_helper!(
    helper_windows_wait_unassigned_fail_stop,
    [WindowsImage, WindowsWaitUnassigned],
    "quiet.exe",
    None,
    false,
    "post-fail-stop"
);
#[cfg(target_os = "windows")]
windows_runner_helper!(
    helper_windows_terminate_job_fail_stop,
    [WindowsPeek, WindowsTerminateJob],
    "quiet.exe",
    None,
    false,
    "post-fail-stop"
);
#[cfg(target_os = "windows")]
windows_runner_helper!(
    helper_windows_query_job_fail_stop,
    [WindowsPeek, WindowsQueryJob],
    "quiet.exe",
    None,
    false,
    "post-fail-stop"
);

#[cfg(target_os = "windows")]
#[test]
fn windows_runner_failures_use_only_explicit_test_state() {
    use std::process::Command;

    let parent = std::fs::canonicalize(std::env::temp_dir()).unwrap();
    let root = parent.join(format!(
        "semaprax-sys-runner-boundaries-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir(&root).unwrap();
    for (name, source) in [
            ("quiet", "int main(void){return 0;}\n"),
            (
                "output",
                "#include <windows.h>\n#include <stdio.h>\nint main(void){fputs(\"x\",stdout);fflush(stdout);Sleep(30000);return 0;}\n",
            ),
            (
                "handle_probe",
                "#include <windows.h>\n#include <stdint.h>\n#include <stdlib.h>\n#include <string.h>\nstatic int nibble(char c){if(c>='0'&&c<='9')return c-'0';if(c>='a'&&c<='f')return c-'a'+10;return -1;}\nint main(int argc,char **argv){if(argc!=4)return 7;char *end=0;uintptr_t handle=(uintptr_t)_strtoui64(argv[1],&end,10);if(!end||*end)return 6;end=0;uint64_t volume=(uint64_t)_strtoui64(argv[2],&end,10);if(!end||*end||strlen(argv[3])!=32)return 6;if(getenv(\"PATH\")!=0)return 8;FILE_ID_INFO info;if(!GetFileInformationByHandleEx((HANDLE)handle,FileIdInfo,&info,sizeof(info)))return 0;if(info.VolumeSerialNumber!=volume)return 0;for(size_t i=0;i<16;i++){int high=nibble(argv[3][i*2]);int low=nibble(argv[3][i*2+1]);if(high<0||low<0)return 6;if(info.FileId.Identifier[i]!=(unsigned char)((high<<4)|low))return 0;}return 9;}\n",
            ),
        ] {
            let source_path = root.join(format!("{name}.c"));
            std::fs::write(&source_path, source).unwrap();
            let compiler = std::env::var_os("CLANG").unwrap_or_else(|| "clang".into());
            let built = Command::new(compiler)
                .env("TMP", &root)
                .env("TEMP", &root)
                .args([
                    "-std=c11",
                    "-Wall",
                    "-Wextra",
                    "-Werror",
                    "-D_CRT_SECURE_NO_WARNINGS",
                    "-O2",
                ])
                .arg(&source_path)
                .arg("-o")
                .arg(root.join(format!("{name}.exe")))
                .output()
                .unwrap();
            assert!(
                built.status.success(),
                "{}",
                String::from_utf8_lossy(&built.stderr)
            );
        }
    use std::fmt::Write as _;
    use std::os::windows::io::AsRawHandle as _;
    let sentinel = root.join("unlisted-handle");
    std::fs::write(&sentinel, b"must not be inherited").unwrap();
    let inherited = std::fs::File::open(&sentinel).unwrap();
    let raw = inherited.as_raw_handle();
    assert_ne!(
        unsafe {
            windows_sys::Win32::Foundation::SetHandleInformation(
                raw.cast(),
                windows_sys::Win32::Foundation::HANDLE_FLAG_INHERIT,
                windows_sys::Win32::Foundation::HANDLE_FLAG_INHERIT,
            )
        },
        0
    );
    let mut identity = windows_sys::Win32::Storage::FileSystem::FILE_ID_INFO::default();
    assert_ne!(
        unsafe {
            windows_sys::Win32::Storage::FileSystem::GetFileInformationByHandleEx(
                raw.cast(),
                windows_sys::Win32::Storage::FileSystem::FileIdInfo,
                (&mut identity as *mut windows_sys::Win32::Storage::FileSystem::FILE_ID_INFO)
                    .cast(),
                u32::try_from(std::mem::size_of::<
                    windows_sys::Win32::Storage::FileSystem::FILE_ID_INFO,
                >())
                .unwrap(),
            )
        },
        0
    );
    let mut file_id = String::with_capacity(32);
    for byte in identity.FileId.Identifier {
        write!(&mut file_id, "{byte:02x}").unwrap();
    }
    let arguments = [
        (raw as usize).to_string(),
        identity.VolumeSerialNumber.to_string(),
        file_id,
    ];
    let directory = super::platform::hold_directory(&root).unwrap();
    let executable =
        super::platform::hold_executable(&directory, std::ffi::OsStr::new("handle_probe.exe"))
            .unwrap();
    super::platform::execute_harness_with_arguments(&executable, &directory, &arguments).unwrap();
    drop(executable);
    drop(directory);
    drop(inherited);
    let current = std::env::current_exe().unwrap();
    for helper in [
        "tests::helper_windows_image",
        "tests::helper_windows_assign",
        "tests::helper_windows_resume",
        "tests::helper_windows_peek",
        "tests::helper_windows_read",
    ] {
        let sentinel = root.join("settled");
        let _ = std::fs::remove_file(&sentinel);
        let status = Command::new(&current)
            .env("SEMAPRAX_SYS_TEST_HELPER_ROOT", &root)
            .args(["--exact", helper, "--nocapture"])
            .status()
            .unwrap();
        assert!(status.success(), "settled boundary failed: {helper}");
        assert!(
            sentinel.exists(),
            "settled boundary did not return: {helper}"
        );
    }
    for helper in [
        "tests::helper_windows_unassigned_fail_stop",
        "tests::helper_windows_wait_unassigned_fail_stop",
        "tests::helper_windows_terminate_job_fail_stop",
        "tests::helper_windows_query_job_fail_stop",
    ] {
        let sentinel = root.join("post-fail-stop");
        let _ = std::fs::remove_file(&sentinel);
        let status = Command::new(&current)
            .env("SEMAPRAX_SYS_TEST_HELPER_ROOT", &root)
            .args(["--exact", helper, "--nocapture"])
            .status()
            .unwrap();
        assert!(!status.success(), "fail-stop boundary returned: {helper}");
        assert!(
            !sentinel.exists(),
            "later action ran after fail-stop: {helper}"
        );
    }
    std::fs::remove_dir_all(&root).unwrap();
}

#[cfg(target_os = "linux")]
fn inventory_record(name: &[u8], inode: u64) -> Vec<u8> {
    let length = (19 + name.len() + 1 + 7) & !7;
    let mut bytes = vec![0_u8; length];
    bytes[..8].copy_from_slice(&inode.to_ne_bytes());
    bytes[16..18].copy_from_slice(&u16::try_from(length).unwrap().to_ne_bytes());
    bytes[18] = 8;
    bytes[19..19 + name.len()].copy_from_slice(name);
    bytes
}

#[cfg(unix)]
fn with_inventory_fixture(
    root: &std::path::Path,
    action: impl FnOnce(
        &super::platform::Directory,
        &super::platform::PreparedDiscardNames<1>,
        &super::platform::RegularFile,
        &mut super::platform::PreparedInventoryExact<1>,
    ),
) {
    use std::ffi::OsStr;

    let _ = std::fs::remove_dir_all(root);
    std::fs::create_dir_all(root).unwrap();
    let directory = super::platform::hold_directory(root).unwrap();
    let names = super::platform::prepare_discard_names([OsStr::new("a")]).unwrap();
    let file = super::platform::write_file_new_prepared(&directory, &names, 0, b"inventory", 0o600)
        .unwrap();
    let mut prepared = super::platform::prepare_inventory_exact(&names).unwrap();
    action(&directory, &names, &file, &mut prepared);
    drop((prepared, file, names, directory));
    std::fs::remove_dir_all(root).unwrap();
}

#[cfg(target_os = "macos")]
fn inventory_record(name: &[u8], inode: u64) -> Vec<u8> {
    let length = (21 + name.len() + 1 + 3) & !3;
    let mut bytes = vec![0_u8; length];
    bytes[..8].copy_from_slice(&inode.to_ne_bytes());
    bytes[16..18].copy_from_slice(&u16::try_from(length).unwrap().to_ne_bytes());
    bytes[18..20].copy_from_slice(&u16::try_from(name.len()).unwrap().to_ne_bytes());
    bytes[20] = 8;
    bytes[21..21 + name.len()].copy_from_slice(name);
    bytes
}

#[test]
fn prepared_file_syscall_gate_resolves_name_before_entry() {
    TEST_PREPARED_FILE_SYSCALL_ENTRIES.store(0, Ordering::Relaxed);
    assert_eq!(
        enter_prepared_file_syscalls::<()>(Err(Error::Invalid)),
        Err(Error::Invalid)
    );
    assert_eq!(
        TEST_PREPARED_FILE_SYSCALL_ENTRIES.load(Ordering::Relaxed),
        0
    );

    let resolved = ();
    assert!(enter_prepared_file_syscalls(Ok(&resolved)).is_ok());
    assert_eq!(
        TEST_PREPARED_FILE_SYSCALL_ENTRIES.load(Ordering::Relaxed),
        1
    );
}

#[test]
fn production_source_exposes_no_prepared_file_syscall_observer() {
    let source = production_sources();
    assert!(!source.contains(concat!("pub fn reset_prepared_file_", "syscall_entries")));
    assert!(!source.contains(concat!("pub fn prepared_file_", "syscall_entries")));
    assert!(!source.contains(concat!("static PREPARED_FILE_", "SYSCALL_ENTRIES")));
}

#[test]
fn windows_external_reader_handoff_and_discard_rebound_are_exact() {
    let source = WINDOWS_SOURCE;
    let transition_start = source
        .find("pub fn transition_regular_file_to_external_read_prepared")
        .expect("Windows external-reader transition");
    let transition_end = source[transition_start..]
        .find("pub fn recheck_regular")
        .map(|offset| transition_start + offset)
        .expect("end Windows external-reader transition");
    let transition = &source[transition_start..transition_end];
    for required in [
        "recheck_held_regular(tracked)?",
        "hold_regular_file_name_external_read_prepared(directory, name)?",
        "rebound.identity != tracked.identity",
        "rebound.digest != tracked.digest",
    ] {
        assert!(
            transition.contains(required),
            "missing Windows external-reader contract: {required}"
        );
    }
    let reader_start = source
        .find("fn hold_regular_file_name_external_read_prepared")
        .expect("Windows read-compatible holder");
    let reader = &source[reader_start..transition_start];
    assert!(reader.contains("REGULAR_READ_ACCESS"));
    let production = source;
    assert_eq!(
            production
                .matches("hold_regular_file_name_external_read_prepared")
                .count(),
            5,
            "Windows read-compatible holder must serve inventory handoff, executable images, and exact archive admission"
        );

    let discard_start = source
        .find("pub fn discard_owned_stage_prepared")
        .expect("Windows prepared discard");
    let discard_end = source[discard_start..]
        .find("fn disposition_delete")
        .map(|offset| discard_start + offset)
        .expect("end Windows prepared discard");
    let discard = &source[discard_start..discard_end];
    for required in [
        "let mut deletion_handles: [Option<RegularFile>; N]",
        "let rebound = hold_regular_file_name_prepared(stage, name)",
        "let file = settled[index].expect(\"attached settled prefix\")",
        "rebound.identity != identity || rebound.digest != digest",
        "deletion_handles[index] = Some(rebound)",
        "for (deleted, file) in deletion_handles[..attached]",
        "must_close_deletion_handles(&mut deletion_handles[..attached])",
        "DIRECTORY_OWNED_ACCESS",
        "directory_information(&stage_deletion)",
        "disposition_delete_and_close(stage_deletion)",
    ] {
        assert!(
            discard.contains(required),
            "missing Windows discard rebound contract: {required}"
        );
    }
    assert!(
        discard
            .find("must_close_deletion_handles(&mut deletion_handles[..attached])")
            .unwrap()
            < discard
                .find("disposition_delete_and_close(stage_deletion)")
                .unwrap()
    );
    let executable_start = source
        .find("pub fn hold_executable")
        .expect("Windows held executable");
    let executable_end = source[executable_start..]
        .find("pub fn executable_regular_file")
        .map(|offset| executable_start + offset)
        .expect("end Windows held executable");
    let executable = &source[executable_start..executable_end];
    assert!(executable.contains("hold_regular_file_name_external_read_prepared"));
    assert!(!executable.contains("hold_regular_file(directory, name)"));
    let close_start = source
        .find("fn must_close_deletion_handles")
        .expect("Windows deletion-handle settlement");
    let close_end = source[close_start..]
        .find("fn run_argv")
        .map(|offset| close_start + offset)
        .expect("end Windows deletion-handle settlement");
    let close = &source[close_start..close_end];
    for required in [
        "file.into_raw_handle()",
        "close_failed |= unsafe { CloseHandle(handle.cast()) } == 0",
        "std::process::abort()",
    ] {
        assert!(
            close.contains(required),
            "missing Windows deletion-handle settlement: {required}"
        );
    }
}

#[cfg(unix)]
#[test]
fn prepared_inventory_record_parser_rejects_malformed_and_stale_bytes() {
    use super::platform::test_parse_inventory_records;

    let valid = inventory_record(b"a", 7);
    assert_eq!(
        test_parse_inventory_records(&valid, &[(b"a".as_slice(), 7)]),
        Ok(())
    );

    let header = if cfg!(target_os = "macos") { 21 } else { 19 };
    assert!(test_parse_inventory_records(&vec![0_u8; header - 1], &[]).is_err());
    for record_length in [0_u16, 8, 21, u16::try_from(valid.len() + 8).unwrap()] {
        let mut malformed = valid.clone();
        malformed[16..18].copy_from_slice(&record_length.to_ne_bytes());
        assert!(test_parse_inventory_records(&malformed, &[(b"a".as_slice(), 7)]).is_err());
    }

    let mut missing_nul = valid.clone();
    let terminator = header + 1;
    missing_nul[terminator..].fill(0xff);
    assert!(test_parse_inventory_records(&missing_nul, &[(b"a".as_slice(), 7)]).is_err());

    let early_nul = inventory_record(b"a\0late", 7);
    #[cfg(target_os = "linux")]
    assert_eq!(
        test_parse_inventory_records(&early_nul, &[(b"a".as_slice(), 7)]),
        Ok(())
    );
    #[cfg(target_os = "macos")]
    assert!(test_parse_inventory_records(&early_nul, &[(b"a".as_slice(), 7)]).is_err());

    let mut nonzero_padding = valid.clone();
    nonzero_padding[terminator + 1..].fill(0xa5);
    assert_eq!(
        test_parse_inventory_records(&nonzero_padding, &[(b"a".as_slice(), 7)]),
        Ok(())
    );

    let mut poisoned_tail = valid.clone();
    poisoned_tail.extend_from_slice(&[0xff; 3]);
    assert!(test_parse_inventory_records(&poisoned_tail, &[(b"a".as_slice(), 7)]).is_err());

    let mut duplicate = valid.clone();
    duplicate.extend_from_slice(&valid);
    assert!(test_parse_inventory_records(&duplicate, &[(b"a".as_slice(), 7)]).is_err());
    assert!(test_parse_inventory_records(
        &inventory_record(b"unknown", 7),
        &[(b"a".as_slice(), 7)]
    )
    .is_err());
    #[cfg(target_os = "linux")]
    assert!(
        test_parse_inventory_records(&inventory_record(b"a", 0), &[(b"a".as_slice(), 0)]).is_err()
    );

    #[cfg(target_os = "macos")]
    {
        let mut with_tombstone = inventory_record(b"a", 7);
        with_tombstone.extend_from_slice(&inventory_record(b"", 0));
        with_tombstone.extend_from_slice(&inventory_record(b"b", 8));
        assert_eq!(
            test_parse_inventory_records(
                &with_tombstone,
                &[(b"a".as_slice(), 7), (b"b".as_slice(), 8)]
            ),
            Ok(())
        );
        let overlong = inventory_record(&vec![b'a'; 1024], 7);
        assert!(test_parse_inventory_records(&overlong, &[]).is_err());
    }
}

#[cfg(unix)]
#[test]
fn prepared_inventory_seek_reset_and_authentication_failures_are_bounded() {
    let base = std::fs::canonicalize(std::env::temp_dir())
        .unwrap()
        .join(format!("semaprax-inventory-failure-{}", std::process::id()));
    for (suffix, failures, expected_scans) in [
        ("initial", (true, false, false, false), 0),
        ("reset", (false, true, false, false), 1),
        ("authentication", (false, false, true, false), 1),
    ] {
        let root = base.join(suffix);
        with_inventory_fixture(&root, |directory, names, file, prepared| {
            super::platform::test_inventory_exact_failures(
                prepared, failures.0, failures.1, failures.2, failures.3,
            );
            assert!(super::platform::inventory_exact_prepared(
                prepared,
                directory,
                names,
                [Some(file)]
            )
            .is_err());
            assert_eq!(
                super::platform::test_inventory_exact_scan_entries(prepared),
                expected_scans
            );
            assert_eq!(
                super::platform::prepared_inventory_exact_remaining(prepared),
                1
            );
        });
    }
    let _ = std::fs::remove_dir_all(base);
}

#[cfg(unix)]
#[test]
fn prepared_inventory_rebound_close_failure_child() {
    let Ok(root) = std::env::var("SEMAPRAX_INVENTORY_CLOSE_FAILURE_ROOT") else {
        return;
    };
    let root = std::path::Path::new(&root);
    with_inventory_fixture(root, |directory, names, file, prepared| {
        super::platform::test_inventory_exact_failures(prepared, false, false, true, true);
        let _ = super::platform::inventory_exact_prepared(prepared, directory, names, [Some(file)]);
        std::fs::write(root.join("later-action"), b"must not exist").unwrap();
    });
}

#[cfg(unix)]
#[test]
fn prepared_inventory_rebound_close_failure_is_fail_stop() {
    let root = std::fs::canonicalize(std::env::temp_dir())
        .unwrap()
        .join(format!(
            "semaprax-inventory-close-failure-{}",
            std::process::id()
        ));
    let _ = std::fs::remove_dir_all(&root);
    let status = std::process::Command::new(std::env::current_exe().unwrap())
        .arg("--exact")
        .arg("tests::prepared_inventory_rebound_close_failure_child")
        .arg("--nocapture")
        .env("SEMAPRAX_INVENTORY_CLOSE_FAILURE_ROOT", &root)
        .status()
        .unwrap();
    assert!(!status.success());
    assert!(!root.join("later-action").exists());
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn linux_prepared_transfer_has_injection_and_allocation_free_copy_fallback_contract() {
    let source = UNIX_SOURCE;
    let link = source
        .find("libc::AT_EMPTY_PATH")
        .expect("Linux prepared transfer uses the held source descriptor");
    let fallback = source[link..]
        .find("fn copy_regular_file_new_prepared")
        .map(|offset| link + offset)
        .expect("Linux fallback is independently authored");
    let linux = &source[link..fallback];
    let linked = linux.find("if result == 0").expect("link success branch");
    let injected = linux
        .find("if fail_before_authentication")
        .expect("debug failure precedes reopen authentication");
    let reopened = linux
        .find("hold_regular_file_name_prepared")
        .expect("prepared destination reopen");
    assert!(linked < injected && injected < reopened);
    for errno in ["libc::EPERM", "libc::EACCES", "libc::EOPNOTSUPP"] {
        assert!(linux.contains(errno));
    }
    assert!(linux.contains("copy_regular_file_new_prepared("));

    let copy = &source[fallback..];
    for required in [
        "libc::O_EXCL",
        "file.write_all(source_bytes)",
        "file.sync_data()",
        "authenticate_regular_file(file)",
    ] {
        assert!(copy.contains(required));
    }
    assert!(!source.contains(concat!("CAP_DAC_", "READ_SEARCH")));
}

#[test]
fn prepared_inventory_exact_source_contract_is_raw_bounded_and_allocation_free() {
    let unix_source = UNIX_SOURCE;
    let linux_start = unix_source
        .find("#[cfg(target_os = \"linux\")]\nfn parse_linux_inventory_records")
        .expect("Linux raw inventory scanner");
    let darwin_start = unix_source[linux_start..]
        .find("#[cfg(target_os = \"macos\")]\nfn parse_darwin_inventory_records")
        .map(|offset| linux_start + offset)
        .expect("Darwin raw inventory scanner");
    let inventory_start = unix_source[darwin_start..]
        .find("pub fn inventory_exact_prepared")
        .map(|offset| darwin_start + offset)
        .expect("Unix prepared inventory entry point");
    let linux = &unix_source[linux_start..darwin_start];
    let darwin = &unix_source[darwin_start..inventory_start];
    for required in [
        "libc::SYS_getdents64",
        "let bytes_limit = libc::c_uint::try_from(capacity)",
        "prepared.storage.fill(u64::MAX)",
        "record < 20",
        "record % std::mem::align_of::<u64>() != 0",
        "next > bytes.len()",
        "maximum_records",
        "maximum_queries",
    ] {
        assert!(
            linux.contains(required),
            "missing Linux contract: {required}"
        );
    }
    for required in [
        "SYS_GETDIRENTRIES64",
        "let bytes_limit: libc::size_t",
        "let mut base: libc::off_t",
        "prepared.storage.fill(u64::MAX)",
        "!record.is_multiple_of(4)",
        "name_length > 1023",
        "name_end >= next",
        "next > bytes.len()",
        "maximum_records",
        "maximum_queries",
    ] {
        assert!(
            darwin.contains(required),
            "missing Darwin contract: {required}"
        );
    }
    for forbidden in ["fdopendir", "readdir", "BTreeSet", "to_vec("] {
        assert!(!linux.contains(forbidden));
        assert!(!darwin.contains(forbidden));
    }

    let windows_source = WINDOWS_SOURCE;
    let windows_start = windows_source
        .find("pub fn inventory_exact_prepared")
        .expect("Windows prepared inventory entry point");
    let windows_end = windows_source[windows_start..]
        .find("pub fn publish_directory_new")
        .map(|offset| windows_start + offset)
        .expect("end of Windows inventory scanner");
    let windows = &windows_source[windows_start..windows_end];
    for required in [
        "FileIdExtdDirectoryRestartInfo",
        "FILE_ID_EXTD_DIR_INFO",
        "prepared.storage.fill(u64::MAX)",
        "entry.FileId.Identifier != tracked.identity.file_id",
        "std::mem::size_of::<FILE_ID_EXTD_DIR_INFO>()",
        "record_header_end > byte_length",
        "next < minimum",
        "next_end > byte_length",
        "maximum_records",
        "maximum_queries",
    ] {
        assert!(
            windows.contains(required),
            "missing Windows contract: {required}"
        );
    }
    let full_header_bound = windows
        .find("if record_header_end > byte_length")
        .expect("complete Windows record must fit");
    let entry_reference = windows
        .find("let entry = unsafe")
        .expect("Windows entry reference");
    assert!(full_header_bound < entry_reference);
    assert!(!windows.contains("FILE_ID_BOTH_DIR_INFO"));
    assert!(!windows.contains("String::from_utf16"));
}

#[test]
fn prepared_publish_source_contract_has_no_late_name_or_handle_allocation() {
    let unix_source = UNIX_SOURCE;
    let unix_start = unix_source
        .find("fn observe_publish_rebound")
        .expect("Unix prepared publish");
    let unix_end = unix_source[unix_start..]
        .find("pub fn discard_owned_stage_prepared")
        .map(|offset| unix_start + offset)
        .expect("end Unix prepared publish");
    let unix = &unix_source[unix_start..unix_end];
    for required in [
        "prepared.remaining != 1",
        "prepared.exact_capacity",
        "relative_name_arena_cstr",
        "observe_publish_rebound",
        "prepared_directory_identity(stage)",
        "libc::SYS_renameat2",
        "renameatx_np",
    ] {
        assert!(
            unix.contains(required),
            "missing Unix publish contract: {required}"
        );
    }
    for forbidden in ["c_name(", "try_clone", "CString::new", "Vec::"] {
        assert!(
            !unix.contains(forbidden),
            "late Unix publish operation: {forbidden}"
        );
    }

    let windows_source = WINDOWS_SOURCE;
    let windows_start = windows_source
        .find("fn observe_publish_rebound")
        .expect("Windows prepared publish");
    let windows_end = windows_source[windows_start..]
        .find("pub fn discard_owned_stage_prepared")
        .map(|offset| windows_start + offset)
        .expect("end Windows prepared publish");
    let windows = &windows_source[windows_start..windows_end];
    for required in [
        "prepared.remaining != 1",
        "prepared.exact_capacity",
        "relative_file_arena",
        "observe_publish_rebound",
        "NtSetInformationFile",
        "FileRenameInformation",
    ] {
        assert!(
            windows.contains(required),
            "missing Windows publish contract: {required}"
        );
    }
    for forbidden in [
        "prepare_relative_name(",
        "named_information(",
        "try_clone",
        "collect::<Vec",
        "SetFileInformationByHandle(",
        "FileRenameInfoEx",
    ] {
        assert!(
            !windows.contains(forbidden),
            "late Windows publish operation: {forbidden}"
        );
    }
}

#[test]
fn settlement_failure_injection_is_test_local_and_has_no_ambient_control() {
    let source = production_sources();
    let ambient_name = ["SEMAPRAX_NATIVE_RUST", "_INTEROP_TEST_SETTLEMENT_FAILURE"].concat();
    assert!(!source.contains(&ambient_name));
    assert!(source.contains("#[cfg(test)]\nstatic TEST_SETTLEMENT_FAILURES"));
    assert!(source.contains("#[cfg(not(test))]\n        {\n            false\n        }"));
    let obsolete_function = ["fn injected_settlement_", "failure(point: &str)"].concat();
    assert!(!source.contains(&obsolete_function));
}

#[test]
fn prepared_process_arena_is_exact_and_consumes_twelve_without_growth() {
    let plan = super::platform::prepare_process_arena_plan(12).unwrap();
    let required = super::platform::prepared_process_arena_plan_capacity(&plan);
    let mut arena = super::platform::materialize_process_arena(plan).unwrap();
    let capacity = super::platform::prepared_process_arena_owned_capacity(&arena);
    assert_eq!(capacity, required);
    #[cfg(windows)]
    assert!((131_080 + 8..=1_245_188).contains(&capacity));
    for remaining in (0..12).rev() {
        super::platform::consume_process_arena(&mut arena).unwrap();
        assert_eq!(
            super::platform::prepared_process_arena_remaining(&arena),
            remaining
        );
        assert_eq!(
            super::platform::prepared_process_arena_owned_capacity(&arena),
            capacity
        );
    }
    assert_eq!(
        super::platform::consume_process_arena(&mut arena),
        Err(Error::OutputLimit)
    );
}

#[cfg(windows)]
#[test]
fn windows_process_arena_attribute_plan_is_exact_aligned_and_bounded() {
    const MAX_ATTRIBUTE_BYTES: usize = 1_048_576;
    for attribute_bytes in [1, 8, 9, 65_537, MAX_ATTRIBUTE_BYTES] {
        let plan = super::platform::process_arena_plan(12, attribute_bytes, 2).unwrap();
        let aligned =
            attribute_bytes.div_ceil(std::mem::size_of::<u64>()) * std::mem::size_of::<u64>();
        assert_eq!(
            super::platform::prepared_process_arena_plan_capacity(&plan),
            131_080 + aligned
        );
        let arena = super::platform::materialize_process_arena(plan).unwrap();
        assert_eq!(
            super::platform::prepared_process_arena_owned_capacity(&arena),
            131_080 + aligned
        );
    }
    assert!(matches!(
        super::platform::process_arena_plan(12, 0, 2),
        Err(Error::Unsupported)
    ));
    assert!(matches!(
        super::platform::process_arena_plan(12, MAX_ATTRIBUTE_BYTES + 1, 2),
        Err(Error::OutputLimit)
    ));

    let include = std::ffi::OsStr::new(r"C:\sdk\include;C:\msvc\include");
    let libraries = std::ffi::OsStr::new(r"C:\sdk\lib;C:\msvc\lib");
    let plan = super::platform::prepare_process_arena_plan_with_environment(
        12,
        Some(include),
        Some(libraries),
    )
    .unwrap();
    let required = super::platform::prepared_process_arena_plan_capacity(&plan);
    let arena = super::platform::materialize_process_arena_with_environment(
        plan,
        Some(include),
        Some(libraries),
    )
    .unwrap();
    assert_eq!(
        super::platform::prepared_process_arena_owned_capacity(&arena),
        required
    );
}

#[cfg(unix)]
#[test]
fn sysroot_output_is_one_nonempty_absolute_utf8_line() {
    assert_eq!(
        super::platform::one_sysroot_line(b"/toolchain\n"),
        Ok(&b"/toolchain"[..])
    );
    assert_eq!(
        super::platform::one_sysroot_line(b"/toolchain\r\n"),
        Ok(&b"/toolchain"[..])
    );
    for invalid in [
        &b""[..],
        &b"/toolchain"[..],
        &b"\n"[..],
        &b"/one\n/two\n"[..],
        &b"/one\0two\n"[..],
        &[0xff, b'\n'],
    ] {
        assert_eq!(
            super::platform::one_sysroot_line(invalid),
            Err(Error::Invalid)
        );
    }
    let resolver = super::platform::prepare_tool_resolver("rustc", 32_768).unwrap();
    assert!(matches!(
        super::platform::hold_rustc_discovery_prepared(
            resolver,
            std::ffi::OsStr::new("relative-rustc")
        ),
        Err(Error::Invalid)
    ));
}

#[test]
fn direct_rustc_and_windows_process_source_contract_is_closed() {
    let source = [UNIX_SOURCE, WINDOWS_SOURCE].concat();
    let discovery_symbol = ["pub fn hold_rustc_", "discovery_prepared"].concat();
    let direct_compile_symbol = ["pub fn compile_direct_", "rustc_prepared"].concat();
    let generic_worker = ["fn compile_rust_", "prepared_inner"].concat();
    assert_eq!(source.matches(&discovery_symbol).count(), 2);
    assert_eq!(source.matches(&direct_compile_symbol).count(), 2);
    assert_eq!(source.matches(&generic_worker).count(), 2);
    let generic_public = ["pub fn compile_rust_", "prepared("].concat();
    let legacy_public = ["pub fn compile_rust_", "staticlib("].concat();
    let misplaced = ["misplaced_windows_", "direct_rustc"].concat();
    assert!(!source.contains(&generic_public));
    assert!(!source.contains(&legacy_public));
    assert!(!source.contains(&misplaced));

    let windows_source = WINDOWS_SOURCE;
    let windows_start = windows_source
        .find("fn run_argv(\n    executable: &Executable,\n    cwd: &Directory,\n    arguments: &[String]")
        .unwrap();
    let windows_end = windows_source[windows_start..]
        .find("fn terminate_unassigned")
        .map(|offset| windows_start + offset)
        .unwrap();
    let windows = &windows_source[windows_start..windows_end];
    for required in [
        "final_path_prepared(&executable.file.file, &mut process_arena.application)",
        "final_path_prepared(&cwd.file, &mut process_arena.cwd)",
        "process_arena.application.resize(PROCESS_PATH_UNITS, 0)",
        "process_arena.environment.as_ptr().cast()",
        "let mut attribute_bytes = process_arena.attribute_bytes",
        "process_arena.attributes.resize(attribute_words, 0)",
        "let null_name = [u16::from(b'N'), u16::from(b'U'), u16::from(b'L'), 0]",
        "must_terminate_unassigned(process_handle.raw())",
        "failed |= thread_handle.close().is_err()",
        "failed |= job.close().is_err()",
    ] {
        assert!(
            windows.contains(required),
            "missing Windows process contract: {required}"
        );
    }
    for forbidden in [
        "final_path(&executable.file.file)",
        "OpenOptions",
        "vec![0_u8; attribute_bytes]",
        "String::from_utf16",
        "PathBuf::from",
        "InitializeProcThreadAttributeList(std::ptr::null_mut()",
        "let empty_environment",
    ] {
        assert!(
            !windows.contains(forbidden),
            "late Windows process allocation: {forbidden}"
        );
    }
    let obsolete_attribute_words = ["PROCESS_ATTRIBUTE_", "WORDS"].concat();
    assert!(!source.contains(&obsolete_attribute_words));
    assert!(source.contains("pub fn prepare_process_arena_plan(uses: usize)"));
    assert!(source.contains(
        "InitializeProcThreadAttributeList(std::ptr::null_mut(), 1, 0, &mut attribute_bytes)"
    ));

    let compile_start = windows_source
        .find("pub fn prepare_c_compile_invocation(")
        .expect("Windows prepared C compile plan");
    let compile_end = windows_source[compile_start..]
        .find("pub fn prepared_c_compile_owned_capacity")
        .map(|offset| compile_start + offset)
        .expect("end Windows prepared C compile plan");
    let compile = &windows_source[compile_start..compile_end];
    assert_eq!(
        compile
            .matches("-mno-incremental-linker-compatible")
            .count(),
        1
    );
    assert!(
        compile.find("target,").unwrap()
            < compile
                .find("\"-mno-incremental-linker-compatible\"")
                .unwrap()
    );
}

#[test]
fn windows_directory_identity_source_excludes_mutable_length_and_binds_all_rechecks() {
    let source = WINDOWS_SOURCE;
    let start = source.find("struct DirectoryIdentity").unwrap();
    let end = source[start..]
        .find("pub struct Directory")
        .map(|offset| start + offset)
        .unwrap();
    let identity = &source[start..end];
    for required in ["volume: u64", "file_id: [u8; 16]", "stable_attributes: u32"] {
        assert!(identity.contains(required));
    }
    assert!(!identity.contains("length:"));

    let windows = source;
    for required in [
        "identity: DirectoryIdentity",
        "directory_identity: Option<DirectoryIdentity>",
        "FILE_ATTRIBUTE_DIRECTORY | FILE_ATTRIBUTE_REPARSE_POINT",
        "stable_attributes != FILE_ATTRIBUTE_DIRECTORY",
        "directory_information(&directory.file)? != directory.identity",
        "directory_information(&rebound)? == directory.identity",
        "directory_information(&rebound)? != stage.identity",
        "Result<DirectoryIdentity, Error>",
    ] {
        assert!(
            windows.contains(required),
            "missing stable Windows directory identity contract: {required}",
        );
    }
}

#[test]
fn linux_rust_staticlib_link_tail_is_frozen_for_prepared_and_legacy_paths() {
    let unix = UNIX_SOURCE;
    let native_start = unix
        .find("const LINUX_RUST_STATICLIB_NATIVE_LIBS: [&str; 7]")
        .unwrap();
    let native_end = unix[native_start..]
        .find("\n    ];")
        .map(|offset| native_start + offset + "\n    ];".len())
        .unwrap();
    let native = &unix[native_start..native_end];
    let mut previous = 0usize;
    for required in [
        "-lgcc_s",
        "-lutil",
        "-lrt",
        "-lpthread",
        "-lm",
        "-ldl",
        "-lc",
    ] {
        let offset = native.find(required).unwrap();
        assert!(
            offset >= previous,
            "Linux native-static library order changed"
        );
        previous = offset;
    }
    assert_eq!(
            unix.matches("LINUX_RUST_STATICLIB_NATIVE_LIBS")
                .count(),
            3,
            "the frozen Linux native-static library tail must have one definition and exactly two link consumers",
        );
    assert_eq!(
            unix.matches("LINUX_LINKER_ARGUMENT").count(),
            3,
            "the absolute Linux linker argument must have one definition and exactly two link consumers",
        );

    let prepared_start = unix.find("pub fn prepare_link_invocation(").unwrap();
    let prepared_end = unix[prepared_start..]
        .find("pub fn prepared_link_owned_capacity(")
        .map(|offset| prepared_start + offset)
        .unwrap();
    let prepared = &unix[prepared_start..prepared_end];
    let prepared_linker = prepared
        .find("values[count] = LINUX_LINKER_ARGUMENT")
        .unwrap();
    let prepared_archive = prepared.find("rust_archive.to_str()").unwrap();
    let prepared_output = prepared.find("output.to_str()").unwrap();
    let prepared_tail = prepared
        .find("for value in LINUX_RUST_STATICLIB_NATIVE_LIBS")
        .unwrap();
    assert!(
        prepared_linker < prepared_archive
            && prepared_archive < prepared_output
            && prepared_output < prepared_tail
    );

    let legacy_start = unix.find("pub fn link_harness(").unwrap();
    let legacy_end = unix[legacy_start..]
        .find("let mut process_arena = prepare_process_arena(1)?")
        .map(|offset| legacy_start + offset)
        .unwrap();
    let legacy = &unix[legacy_start..legacy_end];
    assert!(legacy.contains("arguments.insert(2, argument(LINUX_LINKER_ARGUMENT)?)"));
    let legacy_archive = legacy.find("rust_archive.to_str()").unwrap();
    let legacy_output = legacy.find("output.to_str()").unwrap();
    let legacy_tail = legacy.find("LINUX_RUST_STATICLIB_NATIVE_LIBS").unwrap();
    assert!(legacy_archive < legacy_output && legacy_output < legacy_tail);
}

#[test]
fn windows_rust_staticlib_link_tail_is_frozen_after_the_archive() {
    let windows = WINDOWS_SOURCE;
    let crt_start = windows
        .find("const WINDOWS_DYNAMIC_CRT_LINK_ARGS: [&str; 2]")
        .unwrap();
    let crt_end = windows[crt_start..]
        .find("];")
        .map(|offset| crt_start + offset + 2)
        .unwrap();
    let crt = &windows[crt_start..crt_end];
    assert!(crt.contains("\"-Xlinker\", \"/NODEFAULTLIB:libcmt\""));
    assert_eq!(windows.matches("WINDOWS_DYNAMIC_CRT_LINK_ARGS").count(), 4);
    let native_start = windows
        .find("const WINDOWS_RUST_STATICLIB_NATIVE_LIBS: [&str; 7]")
        .unwrap();
    let native_end = windows[native_start..]
        .find("\n    ];")
        .map(|offset| native_start + offset + "\n    ];".len())
        .unwrap();
    let native = &windows[native_start..native_end];
    let mut previous = 0usize;
    for required in [
        "-lkernel32",
        "-ladvapi32",
        "-ldbghelp",
        "-lntdll",
        "-luserenv",
        "-lws2_32",
        "-lmsvcrt",
    ] {
        let offset = native.find(required).unwrap();
        assert!(
            offset >= previous,
            "Windows native-static library order changed"
        );
        previous = offset;
    }
    assert_eq!(
            windows
                .matches("WINDOWS_RUST_STATICLIB_NATIVE_LIBS")
                .count(),
            9,
            "the frozen Windows native-static library tail must have one definition, seven indexed prepared entries, and one legacy consumer",
        );

    let prepared_start = windows.find("pub fn prepare_link_invocation(").unwrap();
    let prepared_end = windows[prepared_start..]
        .find("pub fn prepared_link_owned_capacity(")
        .map(|offset| prepared_start + offset)
        .unwrap();
    let prepared = &windows[prepared_start..prepared_end];
    let arguments_start = prepared.find("let argument_parts:").unwrap();
    let arguments_end = prepared[arguments_start..]
        .find("preflight_windows_command_line(&argument_parts)?")
        .map(|offset| arguments_start + offset)
        .unwrap();
    let arguments = &prepared[arguments_start..arguments_end];
    let prepared_linker = arguments.find("&[\"-fuse-ld=link\"]").unwrap();
    let prepared_vctools = arguments
        .find("&[\"-Xmicrosoft-visualc-tools-root\"]")
        .unwrap();
    let prepared_crt = arguments.find("WINDOWS_DYNAMIC_CRT_LINK_ARGS").unwrap();
    let prepared_archive = arguments.find("&[rust_archive]").unwrap();
    let prepared_tail = arguments
        .find("WINDOWS_RUST_STATICLIB_NATIVE_LIBS")
        .unwrap();
    let prepared_output = arguments.find("&[\"-o\"]").unwrap();
    assert!(
        prepared_vctools < prepared_linker
            && prepared_linker < prepared_crt
            && prepared_crt < prepared_archive
            && prepared_archive < prepared_tail
            && prepared_tail < prepared_output
    );
    assert!(
        prepared.find("linker_units > MAX_TOOL_PATH_UNITS").unwrap()
            < prepared
                .find("Vec::with_capacity(argument_parts.len())")
                .unwrap()
    );

    let legacy_start = windows.find("pub fn link_harness(").unwrap();
    let legacy_end = windows[legacy_start..]
        .find("let command_line = windows_command_line(&arguments)?")
        .map(|offset| legacy_start + offset)
        .unwrap();
    let legacy = &windows[legacy_start..legacy_end];
    let legacy_linker = legacy.find("\"-fuse-ld=link\".to_owned()").unwrap();
    let legacy_vctools = legacy
        .find("\"-Xmicrosoft-visualc-tools-root\".to_owned()")
        .unwrap();
    let legacy_crt = legacy.find("WINDOWS_DYNAMIC_CRT_LINK_ARGS").unwrap();
    let legacy_archive = legacy.find("rust_archive.to_string_lossy()").unwrap();
    let legacy_tail = legacy.find("WINDOWS_RUST_STATICLIB_NATIVE_LIBS").unwrap();
    let legacy_output = legacy.find("arguments.extend([\"-o\"").unwrap();
    assert!(
        legacy_vctools < legacy_linker
            && legacy_linker < legacy_crt
            && legacy_crt < legacy_archive
            && legacy_archive < legacy_tail
            && legacy_tail < legacy_output
    );
}

#[cfg(windows)]
#[test]
fn windows_prepared_link_owns_the_exact_native_static_tail() {
    for linker in [None, Some(std::ffi::OsStr::new("relative\\link.exe"))] {
        assert!(matches!(
            super::platform::prepare_link_invocation(
                "x86_64-pc-windows-msvc",
                linker,
                Some(std::ffi::OsStr::new(
                    r"C:\Program Files\Microsoft Visual Studio\Lïnk",
                )),
                std::ffi::OsStr::new("main.obj"),
                std::ffi::OsStr::new("module.obj"),
                std::ffi::OsStr::new("bridge.lib"),
                std::ffi::OsStr::new("output.exe"),
                false,
            ),
            Err(Error::Invalid)
        ));
    }
    let vctools = r"C:\Program Files\Microsoft Visual Studio\Lïnk";
    let linker = r"C:\Program Files\Microsoft Visual Studio\Lïnk\bin\Hostx64\x64\link.exe";
    let prepared = super::platform::prepare_link_invocation(
        "x86_64-pc-windows-msvc",
        Some(std::ffi::OsStr::new(linker)),
        Some(std::ffi::OsStr::new(vctools)),
        std::ffi::OsStr::new("main.obj"),
        std::ffi::OsStr::new("module.obj"),
        std::ffi::OsStr::new("bridge.lib"),
        std::ffi::OsStr::new("output.exe"),
        false,
    )
    .unwrap();
    let owned = super::platform::prepared_link_owned_capacity(&prepared);
    let expected = [
        "-target",
        "x86_64-pc-windows-msvc",
        "-Xmicrosoft-visualc-tools-root",
        r"C:\Program Files\Microsoft Visual Studio\Lïnk",
        "-fuse-ld=link",
        "-Xlinker",
        "/NODEFAULTLIB:libcmt",
        "main.obj",
        "module.obj",
        "bridge.lib",
        "-lkernel32",
        "-ladvapi32",
        "-ldbghelp",
        "-lntdll",
        "-luserenv",
        "-lws2_32",
        "-lmsvcrt",
        "-o",
        "output.exe",
    ];
    let (arguments, capacity) = super::platform::test_prepared_link_arguments(&prepared);
    assert!(arguments.iter().map(String::as_str).eq(expected));
    assert_eq!(capacity, expected.len());
    assert_eq!(
        super::platform::prepared_link_owned_capacity(&prepared),
        owned,
    );
}

#[test]
fn linux_runner_uses_the_held_executable_path_as_argv0_before_fexecve() {
    let source = UNIX_SOURCE;
    let start = source
        .find("#[cfg(target_os = \"linux\")]\nfn run_argv_mode(")
        .unwrap();
    let end = source[start..]
        .find("#[cfg(target_os = \"macos\")]\nfn run_argv(")
        .map(|offset| start + offset)
        .unwrap();
    let runner = &source[start..end];
    assert!(!runner.contains("semaprax-native-rust-interop-tool"));
    for required in [
        "let executable_fd_format = b\"/proc/self/fd/%d\\0\"",
        "libc::snprintf(",
        "libc::readlink(",
        "argv[0] = argv0.as_ptr().cast()",
        "fexecve(executable_fd, argv.as_ptr(), env.as_ptr())",
    ] {
        assert!(
            runner.contains(required),
            "missing Linux argv0 contract: {required}"
        );
    }
    let duplicated = runner.find("libc::F_DUPFD").unwrap();
    let readlink = runner.find("libc::readlink(").unwrap();
    let argv0 = runner.find("argv[0] = argv0.as_ptr().cast()").unwrap();
    let execute = runner
        .find("fexecve(executable_fd, argv.as_ptr(), env.as_ptr())")
        .unwrap();
    assert!(duplicated < readlink && readlink < argv0 && argv0 < execute);
}

#[test]
fn linux_archive_runner_closes_after_leader_and_settles_only_its_held_output() {
    let source = UNIX_SOURCE;
    for required in [
        "fn run_archive_argv(",
        "if stdout_limit != 0",
        "close_pipe_after_leader: bool",
        "if close_pipe_after_leader",
        "if quiesce_group(pid).is_err()",
        "Some(libc::EAGAIN) => break Ok((output, child_status))",
        "fn create_owned_archive_seed(",
        "libc::O_CREAT",
        "libc::O_EXCL",
        "Err(_) => std::process::abort()",
        "fn discard_created_archive_identity(",
        "if discard_created_archive_identity(directory, name, created_identity).is_err()",
        "let metadata = rebound.metadata().map_err(|_| Error::Changed)?",
        "if !metadata.is_file() || identity(&metadata) != created_identity",
        "libc::unlinkat(directory.file.as_raw_fd(), name.0.as_ptr(), 0)",
        "let owned_output = create_owned_archive_seed(cwd, &prepared.output_name)?",
        "discard_created_archive_identity(",
    ] {
        assert!(
            source.contains(required),
            "missing exact Linux archive settlement contract: {required}"
        );
    }
    let general = source
        .find("fn run_argv(\n")
        .expect("general Linux runner wrapper");
    let archive = source
        .find("fn run_archive_argv(\n")
        .expect("Linux archive runner wrapper");
    assert!(source[general..archive].contains("false,"));
    assert!(source[archive..].contains("true,"));
    let create = source
        .split("fn create_owned_archive_seed(")
        .nth(1)
        .and_then(|tail| tail.split("fn discard_created_archive_identity(").next())
        .expect("bounded Linux archive seed creation");
    assert!(create.contains("let metadata = match file.metadata()"));
    assert!(create.contains("Err(_) => std::process::abort()"));
    assert!(!create.contains("file.metadata().map_err"));
}

#[cfg(windows)]
#[test]
fn windows_directory_identity_survives_full_inventory_and_rejects_foreign_or_substituted_path() {
    use std::ffi::OsStr;

    let parent = std::fs::canonicalize(std::env::temp_dir()).unwrap();
    let root = parent.join(format!(
        "semaprax-windows-directory-identity-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos(),
    ));
    let stage_path = root.join("stage");
    let displaced_path = root.join("displaced");
    let foreign_path = root.join("foreign");
    std::fs::create_dir(&root).unwrap();
    std::fs::create_dir(&stage_path).unwrap();
    std::fs::create_dir(&foreign_path).unwrap();
    let stage = super::platform::hold_directory(&stage_path).unwrap();
    let names = super::platform::prepare_discard_names([
        OsStr::new("a"),
        OsStr::new("b"),
        OsStr::new("c"),
        OsStr::new("d"),
        OsStr::new("e"),
        OsStr::new("f"),
        OsStr::new("g"),
    ])
    .unwrap();
    let files = (0..7)
        .map(|index| {
            super::platform::write_file_new_prepared(
                &stage,
                &names,
                index,
                &[u8::try_from(index).unwrap()],
                0,
            )
            .unwrap()
        })
        .collect::<Vec<_>>();
    super::platform::recheck_directory(&stage).unwrap();
    let mut inventory = super::platform::prepare_inventory_exact(&names).unwrap();
    let attached = std::array::from_fn(|index| Some(&files[index]));
    super::platform::inventory_exact_prepared(&mut inventory, &stage, &names, attached).unwrap();
    assert!(!super::platform::same_directory_path(&stage, &foreign_path).unwrap());

    drop((inventory, files, names));
    std::fs::rename(&stage_path, &displaced_path).unwrap();
    std::fs::create_dir(&stage_path).unwrap();
    super::platform::recheck_directory(&stage).unwrap();
    assert!(!super::platform::same_directory_path(&stage, &stage_path).unwrap());
    drop(stage);
    std::fs::remove_dir_all(&root).unwrap();
}
