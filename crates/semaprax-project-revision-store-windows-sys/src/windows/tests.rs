use super::*;

#[test]
fn relative_name_grammar_rejects_windows_alias_and_stream_spellings() {
    for rejected in [
        "", ".", "..", "a/b", "a\\b", "a:b", "name.", "name ", "CON", "con.txt", "COM1",
        "lpt9.bin", "CLOCK$", "nul.txt",
    ] {
        assert_eq!(validate_name(rejected), Err(Error::Invalid), "{rejected:?}");
    }
    for admitted in ["entry.json", "sources", ".stage-deadbeef", "COM10"] {
        assert_eq!(validate_name(admitted), Ok(()), "{admitted:?}");
    }
}

#[test]
fn absolute_root_grammar_rejects_non_drive_authority_before_open() {
    for rejected in [
        Path::new("relative\\store"),
        Path::new("\\\\server\\share\\store"),
        Path::new("\\\\?\\C:\\store"),
        Path::new("C:\\store\\..\\other"),
        Path::new("C:\\store:stream"),
    ] {
        assert_eq!(
            open_absolute_components(rejected).unwrap_err(),
            Error::Invalid
        );
    }
}

#[test]
fn publication_primitive_has_one_closed_information_class_and_zero_flags() {
    let source = include_str!("filesystem.rs");
    assert!(source.contains("FileRenameInformationEx"));
    assert!(source.contains("(*information).flags = 0;"));
    assert!(!source.contains("FILE_RENAME_FLAG_POSIX_SEMANTICS"));
    assert!(!source.contains("FileRenameInformation,"));
}

fn descriptor(sddl: &str) -> SecurityDescriptor {
    use windows_sys::Win32::Security::Authorization::{
        ConvertStringSecurityDescriptorToSecurityDescriptorW, SDDL_REVISION_1,
    };
    let text = sddl.encode_utf16().chain(Some(0)).collect::<Vec<_>>();
    let mut raw = std::ptr::null_mut();
    let mut length = 0u32;
    assert_ne!(
        unsafe {
            ConvertStringSecurityDescriptorToSecurityDescriptorW(
                text.as_ptr(),
                SDDL_REVISION_1,
                &mut raw,
                &mut length,
            )
        },
        0
    );
    assert!(!raw.is_null());
    let mut words = vec![0u64; (length as usize).div_ceil(std::mem::size_of::<u64>())];
    unsafe {
        std::ptr::copy_nonoverlapping(
            raw.cast::<u8>(),
            words.as_mut_ptr().cast::<u8>(),
            length as usize,
        )
    };
    assert!(unsafe { windows_sys::Win32::Foundation::LocalFree(raw.cast()) }.is_null());
    SecurityDescriptor {
        words,
        bytes: length as usize,
    }
}

fn sid_text(sid: &[u8]) -> String {
    use windows_sys::Win32::Security::Authorization::ConvertSidToStringSidW;
    let mut text = std::ptr::null_mut();
    assert_ne!(
        unsafe { ConvertSidToStringSidW(sid.as_ptr().cast_mut().cast(), &mut text) },
        0
    );
    let mut length = 0usize;
    while length < 184 && unsafe { *text.add(length) } != 0 {
        length += 1;
    }
    assert!(length < 184);
    let value = String::from_utf16(unsafe { std::slice::from_raw_parts(text, length) }).unwrap();
    assert!(unsafe { windows_sys::Win32::Foundation::LocalFree(text.cast()) }.is_null());
    value
}

#[test]
fn protected_dacl_is_exactly_current_sid_and_system_without_inheritance() {
    let token = capture_effective_token().unwrap();
    let sid = sid_text(&token.sid);
    let exact = descriptor(&format!("O:{sid}D:P(A;;FA;;;SY)(A;;FA;;;{sid})"));
    assert_eq!(validate_owned_descriptor(&exact, &token.sid), Ok(()));
    for rejected in [
        format!("O:{sid}D:P(A;;FA;;;{sid})"),
        format!("O:{sid}D:(A;;FA;;;SY)(A;;FA;;;{sid})"),
        format!("O:{sid}D:P(A;OICI;FA;;;SY)(A;;FA;;;{sid})"),
        format!("O:{sid}D:P(A;;FA;;;WD)(A;;FA;;;{sid})"),
        format!("O:{sid}D:P(D;;FA;;;SY)(A;;FA;;;{sid})"),
        format!("O:{sid}D:P(A;;FR;;;SY)(A;;FA;;;{sid})"),
    ] {
        assert_eq!(
            validate_owned_descriptor(&descriptor(&rejected), &token.sid),
            Err(Error::Changed),
            "{rejected}"
        );
    }
    token.token.close().unwrap();
}

// Opt-in physical evidence, never success-shaped prerequisite skipping. The
// host supplies fixed local NTFS with 8.3 creation already disabled and no
// aliases in the normalized drive-absolute parent chain. Tests never change
// volume settings. Finite create-only fixtures remain inert for host inspection
// and cleanup after all handles settle; no recursive deletion occurs here.
const PHYSICAL_PARENT: &str = "SEMAPRAX_WINDOWS_REVISION_STORE_TEST_PARENT";
static NEXT_PHYSICAL: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

struct PhysicalFixture {
    path: std::path::PathBuf,
}

impl PhysicalFixture {
    fn new(label: &str) -> Self {
        use std::os::windows::fs::MetadataExt as _;
        let parent = std::path::PathBuf::from(std::env::var_os(PHYSICAL_PARENT).expect("explicit physical gate requires SEMAPRAX_WINDOWS_REVISION_STORE_TEST_PARENT on controlled fixed local NTFS with 8.3 creation disabled"));
        let metadata = std::fs::symlink_metadata(&parent).expect("controlled parent must exist");
        assert!(metadata.is_dir());
        assert_eq!(
            metadata.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT,
            0,
            "controlled parent must not be a reparse point"
        );
        let parent_handle = open_absolute_components(&parent)
            .expect("parent must be normalized drive-absolute with no reparse/short aliases");
        require_ntfs_local(&parent_handle).expect("parent must be fixed local NTFS");
        close_file(parent_handle).expect("checked parent preflight settlement");
        let counter = NEXT_PHYSICAL.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let path = parent.join(format!(
            "spx-{label}-{}-{counter}-{nanos}",
            std::process::id()
        ));
        create_exact_fixture_directory(&path);
        eprintln!(
            "retained controlled Windows revision-store fixture: {}",
            path.display()
        );
        hold_root(&path).unwrap_or_else(|e| panic!("physical fixture admission failed ({e:?}); require normalized fixed local NTFS, no aliases, exact effective-user owner/protected DACL: {}", path.display())).settle().expect("checked fixture root settlement");
        Self { path }
    }
    fn hold(&self) -> Root {
        hold_root(&self.path).expect("controlled exact root admission")
    }
}

fn create_exact_fixture_directory(path: &Path) {
    use windows_sys::Win32::Security::SECURITY_ATTRIBUTES;
    use windows_sys::Win32::Storage::FileSystem::CreateDirectoryW;
    let token = capture_effective_token().expect("capture fixture effective token");
    let sid = sid_text(&token.sid);
    assert_ne!(
        sid, "S-1-5-18",
        "two-principal fixture requires a non-LocalSystem effective user"
    );
    let descriptor = descriptor(&format!("O:{sid}D:P(A;;FA;;;SY)(A;;FA;;;{sid})"));
    validate_owned_descriptor(&descriptor, &token.sid).expect("exact fixture descriptor");
    let attributes = SECURITY_ATTRIBUTES {
        nLength: std::mem::size_of::<SECURITY_ATTRIBUTES>() as u32,
        lpSecurityDescriptor: descriptor.as_ptr(),
        bInheritHandle: 0,
    };
    let wide = path
        .as_os_str()
        .encode_wide()
        .chain(Some(0))
        .collect::<Vec<_>>();
    assert!(
        !wide[..wide.len() - 1].contains(&0),
        "fixture path contains NUL"
    );
    // SAFETY: NUL-terminated path, descriptor and attributes remain alive for
    // this create-new call; private fixture setup stays in unsafe quarantine.
    let created = unsafe { CreateDirectoryW(wide.as_ptr(), &attributes) };
    assert_ne!(
        created,
        0,
        "create-only exact fixture failed: {:?}",
        std::io::Error::last_os_error()
    );
    token
        .token
        .close()
        .expect("checked fixture token settlement");
}

#[test]
#[ignore = "requires controlled fixed-local-NTFS parent via SEMAPRAX_WINDOWS_REVISION_STORE_TEST_PARENT; explicit --ignored gate"]
fn physical_safe_roundtrip_bounds_inventory_and_checked_settlement() {
    let fixture = PhysicalFixture::new("roundtrip");
    let root = fixture.hold();
    assert!(root.inventory().unwrap().is_empty());
    let stage = root.create_directory("stage").unwrap();
    let child = stage.create_directory(&root, "sources").unwrap();
    let bytes = b"exact immutable fixture bytes";
    let file = child.create_file(&root, "unit.spx", bytes).unwrap();
    assert_eq!(file.fact().length(), bytes.len() as u64);
    assert_eq!(file.read_bounded(&root, bytes.len()).unwrap(), bytes);
    assert_eq!(file.read_bounded(&root, bytes.len() - 1), Err(Error::Limit));
    assert_eq!(file.read_bounded(&root, usize::MAX), Err(Error::Limit));
    file.settle().unwrap();
    let inventory = child.inventory(&root).unwrap();
    assert_eq!(inventory.len(), 1);
    assert_eq!(inventory[0].name(), "unit.spx");
    assert_eq!(inventory[0].kind(), Kind::File);
    assert_eq!(inventory[0].fact().length(), bytes.len() as u64);
    child.flush().unwrap();
    child.settle().unwrap();
    stage.flush().unwrap();
    let stage_fact = stage.fact();
    root.rename_no_replace(&stage, "entry").unwrap();
    stage.recheck_against(&root).unwrap();
    stage.settle().unwrap();
    let published = root.open_directory("entry").unwrap();
    assert_eq!(published.fact(), stage_fact);
    let reopened = published.open_directory(&root, "sources").unwrap();
    let reread = reopened.open_file(&root, "unit.spx").unwrap();
    assert_eq!(reread.read_bounded(&root, bytes.len()).unwrap(), bytes);
    reread.settle().unwrap();
    reopened.settle().unwrap();
    published.settle().unwrap();
    root.recheck_path(&fixture.path).unwrap();
    root.flush().unwrap();
    root.settle().unwrap();
}

#[test]
#[ignore = "requires controlled fixed-local-NTFS parent via SEMAPRAX_WINDOWS_REVISION_STORE_TEST_PARENT; explicit --ignored gate"]
fn physical_no_clobber_preserves_foreign_destination_and_stage_bytes() {
    let fixture = PhysicalFixture::new("collision");
    let root = fixture.hold();
    let stage = root.create_directory("stage").unwrap();
    stage
        .create_file(&root, "owned", b"owned")
        .unwrap()
        .settle()
        .unwrap();
    let foreign = root.create_directory("taken").unwrap();
    foreign
        .create_file(&root, "foreign", b"foreign")
        .unwrap()
        .settle()
        .unwrap();
    assert_eq!(root.rename_no_replace(&stage, "taken"), Err(Error::Exists));
    assert_eq!(
        stage
            .create_file(&root, "owned", b"replacement")
            .err()
            .unwrap(),
        Error::Exists
    );
    assert_eq!(root.create_directory("taken").err().unwrap(), Error::Exists);
    let owned = stage.open_file(&root, "owned").unwrap();
    assert_eq!(owned.read_bounded(&root, 5).unwrap(), b"owned");
    owned.settle().unwrap();
    let retained = foreign.open_file(&root, "foreign").unwrap();
    assert_eq!(retained.read_bounded(&root, 7).unwrap(), b"foreign");
    retained.settle().unwrap();
    foreign.settle().unwrap();
    stage.settle().unwrap();
    root.settle().unwrap();
}

#[test]
#[ignore = "requires controlled fixed-local-NTFS parent via SEMAPRAX_WINDOWS_REVISION_STORE_TEST_PARENT; explicit --ignored gate"]
fn physical_hard_links_and_alternate_streams_reject_without_erasing_bytes() {
    let fixture = PhysicalFixture::new("links-streams");
    let root = fixture.hold();
    let links = root.create_directory("links").unwrap();
    links
        .create_file(&root, "value", b"original")
        .unwrap()
        .settle()
        .unwrap();
    let path = fixture.path.join("links").join("value");
    std::fs::hard_link(&path, fixture.path.join("links").join("alias"))
        .expect("NTFS hardlink fixture must be supported");
    assert_eq!(
        links.open_file(&root, "value").err().unwrap(),
        Error::Changed
    );
    assert_eq!(links.inventory(&root).err().unwrap(), Error::Changed);
    assert_eq!(std::fs::read(&path).unwrap(), b"original");
    assert_eq!(
        std::fs::read(fixture.path.join("links").join("alias")).unwrap(),
        b"original"
    );
    links.settle().unwrap();
    let streams = root.create_directory("streams").unwrap();
    streams
        .create_file(&root, "value", b"primary")
        .unwrap()
        .settle()
        .unwrap();
    let alternate = fixture.path.join("streams").join("value:hidden");
    std::fs::write(&alternate, b"foreign-stream")
        .expect("NTFS alternate stream fixture must be supported");
    assert_eq!(
        streams.open_file(&root, "value").err().unwrap(),
        Error::Changed
    );
    assert_eq!(
        std::fs::read(fixture.path.join("streams").join("value")).unwrap(),
        b"primary"
    );
    assert_eq!(std::fs::read(&alternate).unwrap(), b"foreign-stream");
    streams.settle().unwrap();
    root.settle().unwrap();
}

#[test]
#[ignore = "requires controlled fixed-local-NTFS parent via SEMAPRAX_WINDOWS_REVISION_STORE_TEST_PARENT and symlink creation privilege/Developer Mode; explicit --ignored gate"]
fn physical_reparse_child_rejects_without_traversing_or_modifying_target() {
    let fixture = PhysicalFixture::new("reparse");
    let root = fixture.hold();
    let target = root.create_directory("target").unwrap();
    target
        .create_file(&root, "sentinel", b"foreign-target")
        .unwrap()
        .settle()
        .unwrap();
    target.settle().unwrap();
    std::os::windows::fs::symlink_dir(fixture.path.join("target"), fixture.path.join("link"))
        .expect(
            "explicit reparse gate requires Windows symlink creation privilege or Developer Mode",
        );
    assert_eq!(root.open_directory("link").err().unwrap(), Error::Changed);
    assert_eq!(root.inventory().err().unwrap(), Error::Changed);
    assert_eq!(
        std::fs::read(fixture.path.join("target").join("sentinel")).unwrap(),
        b"foreign-target"
    );
    root.settle().unwrap();
}

#[test]
#[ignore = "requires controlled fixed-local-NTFS parent via SEMAPRAX_WINDOWS_REVISION_STORE_TEST_PARENT; explicit --ignored gate"]
fn physical_concurrent_root_mutex_is_busy_then_reacquirable_after_settlement() {
    let fixture = PhysicalFixture::new("mutex");
    let root = fixture.hold();
    let path = fixture.path.clone();
    let error = std::thread::spawn(move || match hold_root(&path) {
        Err(error) => error,
        Ok(unexpected) => {
            unexpected.settle().unwrap();
            panic!("concurrent root admission bypassed owned mutex")
        }
    })
    .join()
    .unwrap();
    assert_eq!(error, Error::Busy);
    root.recheck().unwrap();
    root.settle().unwrap();
    fixture.hold().settle().unwrap();
}

#[test]
#[ignore = "requires controlled fixed-local-NTFS parent via SEMAPRAX_WINDOWS_REVISION_STORE_TEST_PARENT; explicit --ignored gate"]
fn physical_same_principal_path_substitution_rejects_without_adopting_replacement() {
    let fixture = PhysicalFixture::new("substitution");
    let root = fixture.hold();
    let original = root.create_directory("original").unwrap();
    original
        .create_file(&root, "sentinel", b"original")
        .unwrap()
        .settle()
        .unwrap();
    original.settle().unwrap();
    let moved = fixture.path.with_file_name(format!(
        "{}-held",
        fixture.path.file_name().unwrap().to_str().unwrap()
    ));
    std::fs::rename(&fixture.path, &moved)
        .expect("same-principal fixture substitution must be possible");
    eprintln!("retained moved authority fixture: {}", moved.display());
    create_exact_fixture_directory(&fixture.path);
    std::fs::write(fixture.path.join("foreign"), b"foreign").unwrap();
    assert_eq!(root.recheck_path(&fixture.path), Err(Error::Changed));
    let names = root
        .inventory()
        .unwrap()
        .into_iter()
        .map(|e| e.name().to_owned())
        .collect::<Vec<_>>();
    assert_eq!(names, ["original"]);
    assert_eq!(
        std::fs::read(moved.join("original").join("sentinel")).unwrap(),
        b"original"
    );
    assert_eq!(
        std::fs::read(fixture.path.join("foreign")).unwrap(),
        b"foreign"
    );
    root.settle().unwrap();
}
