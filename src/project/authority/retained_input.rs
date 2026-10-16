//! Exact bytes from an existing held handle under its original recheck authority.
use super::*;

impl HeldFile {
    pub(in crate::project) fn retain_exact_utf8(
        &mut self,
        remaining: usize,
    ) -> Result<String, Vec<Diagnostic>> {
        if self.expected_len > remaining {
            return Err(capacity("codec_retained_input_bytes", remaining));
        }
        self.recheck()?;
        self.file
            .seek(SeekFrom::Start(0))
            .map_err(|error| authentication(format!("cannot seek retained input: {error}")))?;
        let mut bytes = Vec::new();
        self.file
            .by_ref()
            .take((self.limit as u64) + 1)
            .read_to_end(&mut bytes)
            .map_err(|error| authentication(format!("cannot retain held input: {error}")))?;
        let observed: [u8; 32] = Sha256::digest(&bytes).into();
        if bytes.len() != self.expected_len || observed != self.expected_sha256 {
            return Err(authentication(
                "held input changed during receipt retention",
            ));
        }
        self.recheck()?;
        String::from_utf8(bytes).map_err(|_| authentication("retained Project input is not UTF-8"))
    }
}
