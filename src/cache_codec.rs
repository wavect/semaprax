//! Compiler-private bounded cache encoding. Decoding is not authentication,
//! source admission, HIR validation, or permission to use a cached value.
use crate::diagnostic::Diagnostic;
use std::collections::{BTreeMap, BTreeSet};

mod carriers;

pub(crate) const MAX_BYTES: usize = 128 * 1024 * 1024;
pub(crate) const MAX_ALLOCATION: usize = 128 * 1024 * 1024;
pub(crate) const MAX_NODES: usize = 1_000_000;
pub(crate) const MAX_DEPTH: usize = 256;
pub(crate) type Result<T> = std::result::Result<T, Vec<Diagnostic>>;

pub(crate) trait Codec: Sized {
    fn encode(&self, encoder: &mut Encoder) -> Result<()>;
    fn decode(decoder: &mut Decoder<'_>) -> Result<Self>;
}

pub(crate) fn encode<T: Codec>(value: &T) -> Result<Vec<u8>> {
    let mut encoder = Encoder {
        bytes: Vec::new(),
        nodes: 0,
        depth: 0,
    };
    value.encode(&mut encoder)?;
    Ok(encoder.bytes)
}

pub(crate) fn decode<T: Codec>(bytes: &[u8]) -> Result<T> {
    if bytes.len() > MAX_BYTES {
        return Err(capacity("cache codec wire exceeds its byte limit"));
    }
    let mut decoder = Decoder {
        bytes,
        offset: 0,
        nodes: 0,
        depth: 0,
        allocation: 0,
    };
    decoder.allocate(std::mem::size_of::<T>())?;
    let result = T::decode(&mut decoder)?;
    if decoder.offset != bytes.len() {
        return Err(grammar("cache codec has trailing bytes"));
    }
    Ok(result)
}

pub(crate) struct Encoder {
    bytes: Vec<u8>,
    nodes: usize,
    depth: usize,
}
impl Encoder {
    pub(crate) fn nested<T>(
        &mut self,
        operation: impl FnOnce(&mut Self) -> Result<T>,
    ) -> Result<T> {
        self.node()?;
        if self.depth >= MAX_DEPTH {
            return Err(capacity("cache codec nesting exceeds its depth limit"));
        }
        self.depth += 1;
        let result = operation(self);
        self.depth -= 1;
        result
    }
    fn node(&mut self) -> Result<()> {
        if self.nodes >= MAX_NODES {
            return Err(capacity("cache codec exceeds its node limit"));
        }
        self.nodes += 1;
        Ok(())
    }
    fn raw(&mut self, bytes: &[u8]) -> Result<()> {
        if bytes.len() > MAX_BYTES.saturating_sub(self.bytes.len()) {
            return Err(capacity("cache codec output exceeds its byte limit"));
        }
        let needed = self.bytes.len() + bytes.len();
        if needed > self.bytes.capacity() {
            let target = needed.max(
                self.bytes
                    .capacity()
                    .saturating_mul(2)
                    .clamp(256, MAX_BYTES),
            );
            self.bytes
                .try_reserve_exact(target - self.bytes.len())
                .map_err(|_| capacity("cache codec output allocation failed"))?;
        }
        self.bytes.extend_from_slice(bytes);
        Ok(())
    }
    fn length(&mut self, length: usize) -> Result<()> {
        u32::try_from(length)
            .map_err(|_| capacity("cache codec sequence length exceeds u32"))?
            .encode(self)
    }
    fn text(&mut self, text: &str) -> Result<()> {
        self.nested(|encoder| {
            encoder.length(text.len())?;
            encoder.raw(text.as_bytes())
        })
    }
}

pub(crate) struct Decoder<'a> {
    bytes: &'a [u8],
    offset: usize,
    nodes: usize,
    depth: usize,
    allocation: usize,
}
impl<'a> Decoder<'a> {
    pub(crate) fn nested<T>(
        &mut self,
        operation: impl FnOnce(&mut Self) -> Result<T>,
    ) -> Result<T> {
        self.node()?;
        if self.depth >= MAX_DEPTH {
            return Err(capacity("cache codec nesting exceeds its depth limit"));
        }
        self.depth += 1;
        let result = operation(self);
        self.depth -= 1;
        result
    }
    fn node(&mut self) -> Result<()> {
        if self.nodes >= MAX_NODES {
            return Err(capacity("cache codec exceeds its node limit"));
        }
        self.nodes += 1;
        Ok(())
    }
    fn take(&mut self, length: usize) -> Result<&'a [u8]> {
        let end = self
            .offset
            .checked_add(length)
            .filter(|end| *end <= self.bytes.len())
            .ok_or_else(|| grammar("cache codec input is truncated"))?;
        let bytes = &self.bytes[self.offset..end];
        self.offset = end;
        Ok(bytes)
    }
    pub(crate) fn allocate(&mut self, bytes: usize) -> Result<()> {
        self.allocation = self
            .allocation
            .checked_add(bytes)
            .filter(|used| *used <= MAX_ALLOCATION)
            .ok_or_else(|| capacity("cache codec exceeds its charged allocation limit"))?;
        Ok(())
    }
    fn sequence(&mut self, length: usize, element_bytes: usize) -> Result<()> {
        if length > MAX_NODES.saturating_sub(self.nodes) {
            return Err(capacity(
                "cache codec sequence exceeds its remaining node budget",
            ));
        }
        self.allocate(
            length
                .checked_mul(element_bytes)
                .ok_or_else(|| capacity("cache codec allocation accounting overflow"))?,
        )
    }
    fn length(&mut self) -> Result<usize> {
        Ok(u32::decode(self)? as usize)
    }
    fn text(&mut self) -> Result<&'a str> {
        self.nested(|decoder| {
            let length = decoder.length()?;
            std::str::from_utf8(decoder.take(length)?)
                .map_err(|_| grammar("cache codec string is not UTF-8"))
        })
    }
}

macro_rules! integer {
    ($($ty:ty),* $(,)?) => {$ (
        impl Codec for $ty {
            fn encode(&self, encoder:&mut Encoder)->Result<()> { encoder.node()?; encoder.raw(&self.to_le_bytes()) }
            fn decode(decoder:&mut Decoder<'_>)->Result<Self> {
                decoder.node()?;
                let bytes=decoder.take(std::mem::size_of::<Self>())?;
                Ok(Self::from_le_bytes(bytes.try_into().map_err(|_|grammar("cache codec integer width mismatch"))?))
            }
        }
    )*};
}
integer!(u8, u16, u32, u64, i8, i16, i32, i64);
impl Codec for usize {
    fn encode(&self, encoder: &mut Encoder) -> Result<()> {
        u64::try_from(*self)
            .map_err(|_| capacity("cache codec usize exceeds u64"))?
            .encode(encoder)
    }
    fn decode(decoder: &mut Decoder<'_>) -> Result<Self> {
        usize::try_from(u64::decode(decoder)?)
            .map_err(|_| capacity("cache codec integer exceeds this host width"))
    }
}
impl Codec for bool {
    fn encode(&self, encoder: &mut Encoder) -> Result<()> {
        u8::from(*self).encode(encoder)
    }
    fn decode(decoder: &mut Decoder<'_>) -> Result<Self> {
        match u8::decode(decoder)? {
            0 => Ok(false),
            1 => Ok(true),
            _ => Err(grammar("cache codec boolean tag is invalid")),
        }
    }
}
impl Codec for String {
    fn encode(&self, encoder: &mut Encoder) -> Result<()> {
        encoder.text(self)
    }
    fn decode(decoder: &mut Decoder<'_>) -> Result<Self> {
        let text = decoder.text()?;
        decoder.allocate(text.len())?;
        let mut result = String::new();
        result
            .try_reserve_exact(text.len())
            .map_err(|_| capacity("cache codec string allocation failed"))?;
        result.push_str(text);
        Ok(result)
    }
}
impl Codec for &'static str {
    fn encode(&self, encoder: &mut Encoder) -> Result<()> {
        static_token(self)?;
        encoder.text(self)
    }
    fn decode(decoder: &mut Decoder<'_>) -> Result<Self> {
        static_token(decoder.text()?)
    }
}
fn static_token(value: &str) -> Result<&'static str> {
    match value {
        "semaprax.cleanup-inventory.v1" => Ok("semaprax.cleanup-inventory.v1"),
        "semaprax.cleanup-inventory.v2" => Ok("semaprax.cleanup-inventory.v2"),
        "semaprax.cleanup-plan.v2" => Ok("semaprax.cleanup-plan.v2"),
        "semaprax.cleanup-plan.v3" => Ok("semaprax.cleanup-plan.v3"),
        "semaprax.cleanup-plan.v4" => Ok("semaprax.cleanup-plan.v4"),
        "semaprax.cleanup-plan.v5" => Ok("semaprax.cleanup-plan.v5"),
        "semaprax.cleanup-plan.v6" => Ok("semaprax.cleanup-plan.v6"),
        "semaprax.cleanup-plan.v7" => Ok("semaprax.cleanup-plan.v7"),
        "semaprax.loan-plan.v1" => Ok("semaprax.loan-plan.v1"),
        "callee" => Ok("callee"),
        "success_only" => Ok("success_only"),
        "final_zero_status_commit" => Ok("final_zero_status_commit"),
        "semaprax.status.v1" => Ok("semaprax.status.v1"),
        _ => Err(grammar("cache codec static compiler token is unknown")),
    }
}
impl<T: Codec> Codec for Vec<T> {
    fn encode(&self, encoder: &mut Encoder) -> Result<()> {
        encoder.nested(|encoder| {
            encoder.length(self.len())?;
            for item in self {
                item.encode(encoder)?;
            }
            Ok(())
        })
    }
    fn decode(decoder: &mut Decoder<'_>) -> Result<Self> {
        decoder.nested(|decoder| {
            let length = decoder.length()?;
            decoder.sequence(length, std::mem::size_of::<T>())?;
            let mut result = Vec::new();
            result
                .try_reserve_exact(length)
                .map_err(|_| capacity("cache codec vector allocation failed"))?;
            for _ in 0..length {
                result.push(T::decode(decoder)?);
            }
            Ok(result)
        })
    }
}
impl<T: Codec> Codec for Option<T> {
    fn encode(&self, encoder: &mut Encoder) -> Result<()> {
        encoder.nested(|encoder| match self {
            None => 0u8.encode(encoder),
            Some(value) => {
                1u8.encode(encoder)?;
                value.encode(encoder)
            }
        })
    }
    fn decode(decoder: &mut Decoder<'_>) -> Result<Self> {
        decoder.nested(|decoder| match u8::decode(decoder)? {
            0 => Ok(None),
            1 => Ok(Some(T::decode(decoder)?)),
            _ => Err(grammar("cache codec option tag is invalid")),
        })
    }
}
impl<T: Codec> Codec for Box<T> {
    fn encode(&self, encoder: &mut Encoder) -> Result<()> {
        encoder.nested(|encoder| self.as_ref().encode(encoder))
    }
    fn decode(decoder: &mut Decoder<'_>) -> Result<Self> {
        decoder.nested(|decoder| {
            decoder.allocate(std::mem::size_of::<T>())?;
            Ok(Box::new(T::decode(decoder)?))
        })
    }
}
impl<A: Codec, B: Codec> Codec for (A, B) {
    fn encode(&self, encoder: &mut Encoder) -> Result<()> {
        encoder.nested(|encoder| {
            self.0.encode(encoder)?;
            self.1.encode(encoder)
        })
    }
    fn decode(decoder: &mut Decoder<'_>) -> Result<Self> {
        decoder.nested(|decoder| Ok((A::decode(decoder)?, B::decode(decoder)?)))
    }
}
impl<K: Codec + Ord, V: Codec> Codec for BTreeMap<K, V> {
    fn encode(&self, encoder: &mut Encoder) -> Result<()> {
        encoder.nested(|encoder| {
            encoder.length(self.len())?;
            for (key, value) in self {
                key.encode(encoder)?;
                value.encode(encoder)?;
            }
            Ok(())
        })
    }
    fn decode(decoder: &mut Decoder<'_>) -> Result<Self> {
        decoder.nested(|decoder| {
            let length = decoder.length()?;
            // Logical conservative node charge; stable std maps offer no fallible
            // allocator API. These charges are not a process RSS/OOM guarantee.
            let per_entry = std::mem::size_of::<(K, V)>()
                .checked_add(8 * std::mem::size_of::<usize>())
                .and_then(|size| size.checked_mul(2))
                .ok_or_else(|| capacity("cache codec map accounting overflow"))?;
            decoder.sequence(length, per_entry)?;
            let mut result = BTreeMap::new();
            for _ in 0..length {
                let key = K::decode(decoder)?;
                if result
                    .last_key_value()
                    .is_some_and(|(last, _)| last >= &key)
                {
                    return Err(grammar("cache codec map keys are not strictly increasing"));
                }
                let value = V::decode(decoder)?;
                result.insert(key, value);
            }
            Ok(result)
        })
    }
}
impl<T: Codec + Ord> Codec for BTreeSet<T> {
    fn encode(&self, encoder: &mut Encoder) -> Result<()> {
        encoder.nested(|encoder| {
            encoder.length(self.len())?;
            for value in self {
                value.encode(encoder)?;
            }
            Ok(())
        })
    }
    fn decode(decoder: &mut Decoder<'_>) -> Result<Self> {
        decoder.nested(|decoder| {
            let length = decoder.length()?;
            let per_entry = std::mem::size_of::<T>()
                .checked_add(8 * std::mem::size_of::<usize>())
                .and_then(|size| size.checked_mul(2))
                .ok_or_else(|| capacity("cache codec set accounting overflow"))?;
            decoder.sequence(length, per_entry)?;
            let mut result = BTreeSet::new();
            for _ in 0..length {
                let value = T::decode(decoder)?;
                if result.last().is_some_and(|last| last >= &value) {
                    return Err(grammar(
                        "cache codec set entries are not strictly increasing",
                    ));
                }
                result.insert(value);
            }
            Ok(result)
        })
    }
}

macro_rules! codec_struct {
    ($ty:path {$($field:ident),* $(,)?}) => {
        impl $crate::cache_codec::Codec for $ty {
            fn encode(&self, out:&mut $crate::cache_codec::Encoder)->$crate::cache_codec::Result<()> {out.nested(|out|{$($crate::cache_codec::Codec::encode(&self.$field,out)?;)*Ok(())})}
            fn decode(input:&mut $crate::cache_codec::Decoder<'_>)->$crate::cache_codec::Result<Self> {input.nested(|input|Ok(Self {$($field:$crate::cache_codec::Codec::decode(input)?),*}))}
        }
    };
}
macro_rules! codec_tuple {
    ($ty:ident ($($index:tt),* $(,)?)) => {
        impl $crate::cache_codec::Codec for $ty {
            fn encode(&self,out:&mut $crate::cache_codec::Encoder)->$crate::cache_codec::Result<()> {out.nested(|out|{$($crate::cache_codec::Codec::encode(&self.$index,out)?;)*Ok(())})}
            fn decode(input:&mut $crate::cache_codec::Decoder<'_>)->$crate::cache_codec::Result<Self> {input.nested(|input|Ok(Self($({let _=stringify!($index);$crate::cache_codec::Codec::decode(input)?}),*)))}
        }
    };
}
macro_rules! codec_enum {
    ($ty:path {$($tag:literal => $variant:ident $(($($tuple:ident),* $(,)?))? $({$($field:ident),* $(,)?})?),* $(,)?}) => {
        impl $crate::cache_codec::Codec for $ty {
            fn encode(&self,out:&mut $crate::cache_codec::Encoder)->$crate::cache_codec::Result<()> {out.nested(|out|match self {
                $(Self::$variant $(($($tuple),*))? $({$($field),*})? => { $crate::cache_codec::Codec::encode(&($tag as u16),out)?; $($($crate::cache_codec::Codec::encode($tuple,out)?;)*)? $($($crate::cache_codec::Codec::encode($field,out)?;)*)? Ok(()) }),*
            })}
            fn decode(input:&mut $crate::cache_codec::Decoder<'_>)->$crate::cache_codec::Result<Self> {input.nested(|input|match <u16 as $crate::cache_codec::Codec>::decode(input)? {
                $($tag => Ok(Self::$variant $(($({let $tuple=$crate::cache_codec::Codec::decode(input)?;$tuple}),*))? $({$($field:$crate::cache_codec::Codec::decode(input)?),*})?)),*,
                _=>Err($crate::cache_codec::grammar("cache codec enum tag is unknown")),
            })}
        }
    };
}
pub(crate) use {codec_enum, codec_struct, codec_tuple};
pub(crate) fn grammar(message: &'static str) -> Vec<Diagnostic> {
    vec![Diagnostic::io("SPX-G304", message)]
}
pub(crate) fn capacity(message: &'static str) -> Vec<Diagnostic> {
    vec![Diagnostic::io("SPX-G305", message)]
}

#[cfg(test)]
#[path = "cache_codec/nested_owned_records_tests.rs"]
mod nested_owned_records_tests;
