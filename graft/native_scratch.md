---
covers: []
---
# native_scratch.rs

- MAX_ATTEMPTS · constant · L11-L11 — const MAX_ATTEMPTS: usize = 64;
- SERIAL · constant · L12-L12 — static SERIAL: AtomicU64 = AtomicU64::new(0);
- Scratch · struct · L14-L22 — pub(super) struct Scratch
- create · function · L25-L33 — pub(super) fn create(leaf: &str, contents: Option<&[u8]>) -> io::Result<Self>
- create_in · function · L35-L104 — fn create_in(
- path · function · L106-L108 — pub(super) fn path(&self) -> &Path
- seal · function · L112-L122 — pub(super) fn seal(&mut self) -> io::Result<()>
- cleanup · function · L126-L140 — pub(super) fn cleanup(mut self) -> io::Result<()>
- bind_directory · function · L142-L145 — fn bind_directory(&self) -> io::Result<()>
- verify_file · function · L147-L154 — fn verify_file(&self) -> io::Result<()>
- inventory · function · L156-L171 — fn inventory(&self, has_file: bool) -> io::Result<()>
- one_component · function · L174-L180 — fn one_component(path: &Path) -> bool
- plain · function · L182-L200 — fn plain(path: &Path, directory: bool) -> io::Result<()>
- bind · function · L202-L208 — fn bind(path: &Path, expected: &Handle, directory: bool) -> io::Result<()>
- single_link · function · L211-L223 — fn single_link(file: &File) -> io::Result<()>
- single_link · function · L226-L228 — fn single_link(_file: &File) -> io::Result<()>
- changed · function · L230-L232 — fn changed(message: &str) -> io::Error
- tests · module · L236-L236 — mod tests;
