use std::fs;
use std::path::PathBuf;
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};

static SERIAL: AtomicU64 = AtomicU64::new(0);

pub(super) struct Fixture {
    pub root: PathBuf,
    pub source: PathBuf,
    permitted: Vec<PathBuf>,
}

impl Fixture {
    pub fn new(source: &str) -> Self {
        let root = std::env::temp_dir().join(format!(
            "semaprax-interpreter-strings-{}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos(),
            SERIAL.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&root).unwrap();
        let mut fixture = Self {
            source: root.join("source.spx"),
            root,
            permitted: Vec::new(),
        };
        fixture.write("source.spx", source);
        fixture
    }

    pub fn write(&mut self, name: &str, contents: impl AsRef<[u8]>) -> PathBuf {
        assert!(name
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || b".-_".contains(&byte)));
        assert!(!matches!(name, "" | "." | ".."));
        let path = self.root.join(name);
        if !self.permitted.contains(&path) {
            self.permitted.push(path.clone());
        }
        fs::write(&path, contents).unwrap();
        path
    }

    pub fn native(&mut self, source: &str, optimization: &str) -> String {
        let path = self.write("probe.c", source);
        let stem = format!("native{optimization}");
        let executable = self
            .root
            .join(format!("{stem}{}", std::env::consts::EXE_SUFFIX));
        self.permitted.push(executable.clone());
        if cfg!(windows) {
            for extension in ["lib", "exp", "pdb", "ilk"] {
                self.permitted
                    .push(self.root.join(format!("{stem}.{extension}")));
            }
        }
        let compiler = std::env::var_os("CLANG").unwrap_or_else(|| "clang".into());
        let output = Command::new(compiler)
            .current_dir(&self.root)
            .args([
                "-std=c11",
                optimization,
                "-Wall",
                "-Wextra",
                "-Werror",
                "-DSPX_NO_ENTRY_WRAPPER",
            ])
            .arg(path)
            .arg("-o")
            .arg(&executable)
            .output()
            .expect("Clang is required for the selected internal String parity gate");
        assert!(
            output.status.success(),
            "{}: {}",
            self.root.display(),
            String::from_utf8_lossy(&output.stderr)
        );
        let output = Command::new(executable)
            .current_dir(&self.root)
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}: {}",
            self.root.display(),
            String::from_utf8_lossy(&output.stderr)
        );
        assert!(output.stderr.is_empty());
        String::from_utf8(output.stdout).unwrap()
    }

    pub fn cleanup(self) {
        // Only successful fixtures are removed; validate the complete flat
        // inventory before deleting anything, including Windows sidecars.
        let entries = fs::read_dir(&self.root)
            .unwrap()
            .map(|entry| entry.unwrap())
            .collect::<Vec<_>>();
        assert!(entries.len() <= self.permitted.len());
        for entry in &entries {
            assert!(self.permitted.contains(&entry.path()));
            let metadata = fs::symlink_metadata(entry.path()).unwrap();
            assert!(metadata.is_file() && !metadata.file_type().is_symlink());
            #[cfg(windows)]
            {
                use std::os::windows::fs::MetadataExt;
                assert_eq!(metadata.file_attributes() & 0x400, 0);
            }
        }
        for entry in entries {
            fs::remove_file(entry.path()).unwrap();
        }
        fs::remove_dir(self.root).unwrap();
    }
}
