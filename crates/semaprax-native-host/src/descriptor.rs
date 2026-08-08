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

    pub(crate) fn has_scalar_parameters(&self) -> bool {
        self.parameters
            .iter()
            .any(|parameter| matches!(parameter, Parameter::Scalar { .. }))
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
mod tests {
    use super::*;
    use semaprax::codegen::emit_native_adapter_admission;
    use semaprax::hir::{self, DeclarationId};
    use std::path::Path;

    const SOURCE: &str = r#"module test.host_descriptor;

@id("token.type")
resource Token { @id("token.drop") drop trivial; }

@id("token.scalar-mix")
fn scalar_mix(value: own Token, delta: i64, condition: bool) -> i64 {
    0
}

@id("token.select-second")
fn select_second(first: own Token, second: own Token) -> Token { second }

@id("test.main")
fn main() -> i64 { 0 }
"#;

    struct WireOffsets {
        physical_module: usize,
        function_template: usize,
        owned_tag: usize,
        owned_index: usize,
        owned_ordinal: usize,
        resource_length: usize,
        resource_bytes: usize,
        scalar_kind: usize,
        result_tag: usize,
        result_parameter: usize,
        result_value: usize,
        result_ordinal: usize,
    }

    fn push_u32(output: &mut Vec<u8>, value: u32) -> usize {
        let offset = output.len();
        output.extend_from_slice(&value.to_le_bytes());
        offset
    }

    fn push_text(output: &mut Vec<u8>, value: &str) -> (usize, usize) {
        let length = push_u32(output, value.len().try_into().unwrap());
        let bytes = output.len();
        output.extend_from_slice(value.as_bytes());
        (length, bytes)
    }

    fn canonical_wire() -> (Vec<u8>, WireOffsets) {
        let mut output = Vec::new();
        output.extend_from_slice(MAGIC);
        push_u32(&mut output, VERSION);
        push_u32(&mut output, HEADER_SIZE);
        let declared_length = push_u32(&mut output, 0);
        let target = current_target_tag();
        push_text(&mut output, &target);
        output.extend_from_slice(&schema_fingerprint());
        output.extend_from_slice(&fingerprint_target(target.as_bytes()));
        let physical_module = output.len();
        output.extend_from_slice(&[0x11; FINGERPRINT_BYTES]);
        let function_template = output.len();
        output.extend_from_slice(&[0x22; FINGERPRINT_BYTES]);
        push_text(&mut output, "test.module");
        push_text(&mut output, "test.function");
        push_u32(&mut output, 2);

        let owned_tag = push_u32(&mut output, PARAMETER_OWNED_RESOURCE);
        let owned_index = push_u32(&mut output, 0);
        push_text(&mut output, "token.value");
        let owned_ordinal = push_u32(&mut output, 0);
        let (resource_length, resource_bytes) = push_text(&mut output, "token.type");
        push_text(&mut output, "token.drop");

        push_u32(&mut output, PARAMETER_SCALAR);
        push_u32(&mut output, 1);
        push_text(&mut output, "delta.value");
        let scalar_kind = push_u32(&mut output, SCALAR_I64);

        let result_tag = push_u32(&mut output, RESULT_OWNED_INPUT);
        let result_parameter = push_u32(&mut output, 0);
        let (_, result_value) = push_text(&mut output, "token.value");
        let result_ordinal = push_u32(&mut output, 0);

        let length = u32::try_from(output.len()).unwrap();
        output[declared_length..declared_length + 4].copy_from_slice(&length.to_le_bytes());
        (
            output,
            WireOffsets {
                physical_module,
                function_template,
                owned_tag,
                owned_index,
                owned_ordinal,
                resource_length,
                resource_bytes,
                scalar_kind,
                result_tag,
                result_parameter,
                result_value,
                result_ordinal,
            },
        )
    }

    fn replace_u32(bytes: &mut [u8], offset: usize, value: u32) {
        bytes[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
    }

    fn artifact(function: &str) -> semaprax::codegen::NativeAdapterAdmissionArtifact {
        let parsed = semaprax::parse(SOURCE, Path::new("host-descriptor-unit.spx")).unwrap();
        let resolved = hir::resolve(&parsed).unwrap();
        emit_native_adapter_admission(&resolved, &DeclarationId::new(function), "descriptor.h")
            .unwrap()
    }

    #[test]
    fn compiler_artifacts_round_trip_scalar_and_multi_owner_shapes_exactly() {
        let scalar = artifact("token.scalar-mix");
        let scalar_descriptor = Descriptor::parse(scalar.descriptor()).unwrap();
        assert!(scalar_descriptor.has_scalar_parameters());
        assert_eq!(scalar_descriptor.parameters.len(), 3);
        assert_eq!(scalar_descriptor.owner_requirements().len(), 1);
        assert_eq!(scalar_descriptor.getter_symbol(), scalar.getter_symbol());

        let multi_owner = artifact("token.select-second");
        let multi_owner_descriptor = Descriptor::parse(multi_owner.descriptor()).unwrap();
        assert!(!multi_owner_descriptor.has_scalar_parameters());
        assert_eq!(multi_owner_descriptor.owner_requirements().len(), 2);
        assert_eq!(
            multi_owner_descriptor.result,
            ResultShape::OwnedInput {
                parameter_index: 1,
                owner_ordinal: 1,
            }
        );
        assert_eq!(
            multi_owner_descriptor.getter_symbol(),
            multi_owner.getter_symbol()
        );
    }

    #[test]
    fn every_body_discriminant_index_and_fingerprint_is_checked() {
        let (canonical, offsets) = canonical_wire();
        Descriptor::parse(&canonical).unwrap();

        let mut cases = Vec::new();
        let mut physical_zero = canonical.clone();
        physical_zero[offsets.physical_module..offsets.physical_module + FINGERPRINT_BYTES].fill(0);
        cases.push(physical_zero);
        let mut function_zero = canonical.clone();
        function_zero[offsets.function_template..offsets.function_template + FINGERPRINT_BYTES]
            .fill(0);
        cases.push(function_zero);
        for (offset, value) in [
            (offsets.owned_tag, 99),
            (offsets.owned_index, 1),
            (offsets.owned_ordinal, 1),
            (offsets.scalar_kind, 99),
            (offsets.result_tag, 99),
            (offsets.result_parameter, 1),
            (offsets.result_ordinal, 1),
        ] {
            let mut hostile = canonical.clone();
            replace_u32(&mut hostile, offset, value);
            cases.push(hostile);
        }
        let mut wrong_result_value = canonical.clone();
        wrong_result_value[offsets.result_value] ^= 1;
        cases.push(wrong_result_value);

        for hostile in cases {
            assert_eq!(
                Descriptor::parse(&hostile),
                Err(DescriptorError::NonCanonical)
            );
        }
    }

    #[test]
    fn identity_lengths_nuls_and_utf8_fail_closed() {
        let (canonical, offsets) = canonical_wire();

        let mut empty = canonical.clone();
        replace_u32(&mut empty, offsets.resource_length, 0);
        assert_eq!(
            Descriptor::parse(&empty),
            Err(DescriptorError::NonCanonical)
        );

        let mut nul = canonical.clone();
        nul[offsets.resource_bytes] = 0;
        assert_eq!(Descriptor::parse(&nul), Err(DescriptorError::NonCanonical));

        let mut invalid_utf8 = canonical;
        invalid_utf8[offsets.resource_bytes] = 0xff;
        assert_eq!(
            Descriptor::parse(&invalid_utf8),
            Err(DescriptorError::Malformed)
        );
    }
}
