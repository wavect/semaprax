//! Strict decoder for the compiler-derived pointer-free native descriptor.

#![forbid(unsafe_code)]

use sha2::{Digest, Sha256};

const MAGIC: &[u8; 8] = b"SPXNABI1";
const VERSION: u32 = 1;
const HEADER_SIZE: u32 = 20;
const FINGERPRINT_BYTES: usize = 32;
const SCHEMA_FINGERPRINT_DOMAIN: &[u8] = b"semaprax.native-adapter-schema.v1\0";
const TARGET_FINGERPRINT_DOMAIN: &[u8] = b"semaprax.native-adapter-target.v1\0";
const GETTER_SYMBOL_DOMAIN: &[u8] = b"semaprax.native-adapter-getter.v1\0";
const PARAMETER_SCALAR: u32 = 1;
const PARAMETER_OWNED_RESOURCE: u32 = 2;
const SCALAR_I64: u32 = 1;
const SCALAR_BOOL: u32 = 2;
const RESULT_SCALAR_I64: u32 = 1;
const RESULT_OWNED_INPUT: u32 = 2;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ScalarKind {
    I64,
    Bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum Parameter {
    Scalar {
        index: usize,
        kind: ScalarKind,
    },
    Owned {
        index: usize,
        value: String,
        owner_ordinal: usize,
        resource: String,
        lifecycle: String,
    },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ResultShape {
    ScalarI64,
    OwnedInput {
        parameter_index: usize,
        owner_ordinal: usize,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct Descriptor {
    pub(crate) physical_module: [u8; FINGERPRINT_BYTES],
    pub(crate) function_template: [u8; FINGERPRINT_BYTES],
    pub(crate) module: String,
    pub(crate) function: String,
    pub(crate) parameters: Vec<Parameter>,
    pub(crate) result: ResultShape,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum DescriptorError {
    Malformed,
    UnsupportedSchema,
    WrongTarget,
    NonCanonical,
}

impl Descriptor {
    pub(crate) fn parse(bytes: &[u8]) -> Result<Self, DescriptorError> {
        let mut reader = Reader { bytes, offset: 0 };
        if reader.take(MAGIC.len())? != MAGIC
            || reader.u32()? != VERSION
            || reader.u32()? != HEADER_SIZE
        {
            return Err(DescriptorError::UnsupportedSchema);
        }
        let declared = reader.usize()?;
        if declared != bytes.len() {
            return Err(DescriptorError::Malformed);
        }
        let target = reader.text()?;
        if target != current_target_tag() {
            return Err(DescriptorError::WrongTarget);
        }
        let schema = reader.fingerprint()?;
        if schema != schema_fingerprint() {
            return Err(DescriptorError::UnsupportedSchema);
        }
        let target_fingerprint = reader.fingerprint()?;
        if target_fingerprint != fingerprint_target(target.as_bytes()) {
            return Err(DescriptorError::WrongTarget);
        }
        let physical_module = reader.fingerprint()?;
        let function_template = reader.fingerprint()?;
        if physical_module == [0; FINGERPRINT_BYTES] || function_template == [0; FINGERPRINT_BYTES]
        {
            return Err(DescriptorError::NonCanonical);
        }
        let module = reader.text()?;
        let function = reader.text()?;
        let parameter_count = reader.usize()?;
        let mut parameters = Vec::with_capacity(parameter_count.min(1024));
        let mut next_owner = 0_usize;
        for expected_index in 0..parameter_count {
            let tag = reader.u32()?;
            let index = reader.usize()?;
            if index != expected_index {
                return Err(DescriptorError::NonCanonical);
            }
            let value_identity = reader.text()?;
            match tag {
                PARAMETER_SCALAR => {
                    let kind = match reader.u32()? {
                        SCALAR_I64 => ScalarKind::I64,
                        SCALAR_BOOL => ScalarKind::Bool,
                        _ => return Err(DescriptorError::NonCanonical),
                    };
                    parameters.push(Parameter::Scalar { index, kind });
                }
                PARAMETER_OWNED_RESOURCE => {
                    let owner_ordinal = reader.usize()?;
                    if owner_ordinal != next_owner {
                        return Err(DescriptorError::NonCanonical);
                    }
                    next_owner = next_owner
                        .checked_add(1)
                        .ok_or(DescriptorError::Malformed)?;
                    parameters.push(Parameter::Owned {
                        index,
                        value: value_identity,
                        owner_ordinal,
                        resource: reader.text()?,
                        lifecycle: reader.text()?,
                    });
                }
                _ => return Err(DescriptorError::NonCanonical),
            }
        }
        let result = match reader.u32()? {
            RESULT_SCALAR_I64 => ResultShape::ScalarI64,
            RESULT_OWNED_INPUT => {
                let parameter_index = reader.usize()?;
                let value_identity = reader.text()?;
                let owner_ordinal = reader.usize()?;
                let Some(Parameter::Owned {
                    index,
                    value: expected_value,
                    owner_ordinal: expected_ordinal,
                    ..
                }) = parameters.get(parameter_index)
                else {
                    return Err(DescriptorError::NonCanonical);
                };
                if *index != parameter_index
                    || *expected_value != value_identity
                    || *expected_ordinal != owner_ordinal
                {
                    return Err(DescriptorError::NonCanonical);
                }
                ResultShape::OwnedInput {
                    parameter_index,
                    owner_ordinal,
                }
            }
            _ => return Err(DescriptorError::NonCanonical),
        };
        if reader.offset != bytes.len() {
            return Err(DescriptorError::Malformed);
        }
        Ok(Self {
            physical_module,
            function_template,
            module,
            function,
            parameters,
            result,
        })
    }

    pub(crate) fn owned_parameter(&self, owner_ordinal: usize) -> Option<(&str, &str, usize)> {
        self.parameters
            .iter()
            .find_map(|parameter| match parameter {
                Parameter::Owned {
                    index,
                    value: _,
                    owner_ordinal: candidate,
                    resource,
                    lifecycle,
                } if *candidate == owner_ordinal => {
                    Some((resource.as_str(), lifecycle.as_str(), *index))
                }
                Parameter::Scalar { .. } | Parameter::Owned { .. } => None,
            })
    }

    pub(crate) fn scalar_kinds(&self) -> Vec<ScalarKind> {
        self.parameters
            .iter()
            .filter_map(|parameter| match parameter {
                Parameter::Scalar { kind, .. } => Some(*kind),
                Parameter::Owned { .. } => None,
            })
            .collect()
    }

    pub(crate) fn owner_requirements(&self) -> Vec<(&str, &str)> {
        self.parameters
            .iter()
            .filter_map(|parameter| match parameter {
                Parameter::Owned {
                    resource,
                    lifecycle,
                    ..
                } => Some((resource.as_str(), lifecycle.as_str())),
                Parameter::Scalar { .. } => None,
            })
            .collect()
    }

    pub(crate) fn getter_symbol(&self) -> String {
        let mut hasher = Sha256::new();
        hasher.update(GETTER_SYMBOL_DOMAIN);
        hash_field(&mut hasher, &self.physical_module);
        hash_field(&mut hasher, &self.function_template);
        let digest = hasher.finalize();
        let mut symbol = String::with_capacity(4 + 48 + 22);
        symbol.push_str("spx_");
        for byte in &digest[..24] {
            use std::fmt::Write as _;
            write!(symbol, "{byte:02x}").expect("writing to a string cannot fail");
        }
        symbol.push_str("_adapter_descriptor_v1");
        symbol
    }
}

struct Reader<'a> {
    bytes: &'a [u8],
    offset: usize,
}

impl<'a> Reader<'a> {
    fn take(&mut self, count: usize) -> Result<&'a [u8], DescriptorError> {
        let end = self
            .offset
            .checked_add(count)
            .ok_or(DescriptorError::Malformed)?;
        let value = self
            .bytes
            .get(self.offset..end)
            .ok_or(DescriptorError::Malformed)?;
        self.offset = end;
        Ok(value)
    }

    fn u32(&mut self) -> Result<u32, DescriptorError> {
        let bytes: [u8; 4] = self
            .take(4)?
            .try_into()
            .map_err(|_| DescriptorError::Malformed)?;
        Ok(u32::from_le_bytes(bytes))
    }

    fn usize(&mut self) -> Result<usize, DescriptorError> {
        usize::try_from(self.u32()?).map_err(|_| DescriptorError::Malformed)
    }

    fn text(&mut self) -> Result<String, DescriptorError> {
        let length = self.usize()?;
        let bytes = self.take(length)?;
        let value = std::str::from_utf8(bytes).map_err(|_| DescriptorError::Malformed)?;
        if value.is_empty() || value.contains('\0') {
            return Err(DescriptorError::NonCanonical);
        }
        Ok(value.to_owned())
    }

    fn fingerprint(&mut self) -> Result<[u8; FINGERPRINT_BYTES], DescriptorError> {
        self.take(FINGERPRINT_BYTES)?
            .try_into()
            .map_err(|_| DescriptorError::Malformed)
    }
}

fn current_target_tag() -> String {
    let endian = if cfg!(target_endian = "little") {
        "little"
    } else {
        "big"
    };
    let environment = if cfg!(target_env = "msvc") {
        "msvc"
    } else if cfg!(target_env = "gnu") {
        "gnu"
    } else if cfg!(target_env = "musl") {
        "musl"
    } else if cfg!(target_os = "macos") {
        "apple"
    } else {
        "unknown"
    };
    let object_format = if cfg!(windows) {
        "coff"
    } else if cfg!(target_os = "macos") {
        "macho"
    } else {
        "elf"
    };
    let getter_abi = if cfg!(windows) {
        "descriptor-getter-cdecl"
    } else {
        "descriptor-getter-c"
    };
    format!(
        "{}-{}-{environment}-{object_format}-ptr{}-{endian}-{getter_abi}",
        std::env::consts::ARCH,
        std::env::consts::OS,
        usize::BITS
    )
}

fn schema_fingerprint() -> [u8; FINGERPRINT_BYTES] {
    let mut hasher = Sha256::new();
    hasher.update(SCHEMA_FINGERPRINT_DOMAIN);
    hash_field(&mut hasher, MAGIC);
    hasher.update(VERSION.to_le_bytes());
    hasher.update(HEADER_SIZE.to_le_bytes());
    for tag in [
        PARAMETER_SCALAR,
        PARAMETER_OWNED_RESOURCE,
        SCALAR_I64,
        SCALAR_BOOL,
        RESULT_SCALAR_I64,
        RESULT_OWNED_INPUT,
    ] {
        hasher.update(tag.to_le_bytes());
    }
    hasher.update(b"u32le-lengths;utf8-identities;ordered-signature;pointer-free");
    hasher.finalize().into()
}

fn fingerprint_target(target: &[u8]) -> [u8; FINGERPRINT_BYTES] {
    let mut hasher = Sha256::new();
    hasher.update(TARGET_FINGERPRINT_DOMAIN);
    hash_field(&mut hasher, target);
    hasher.finalize().into()
}

fn hash_field(hasher: &mut Sha256, bytes: &[u8]) {
    hasher.update((bytes.len() as u64).to_be_bytes());
    hasher.update(bytes);
}

#[cfg(test)]
#[path = "descriptor/tests.rs"]
mod tests;
