//! Compiler-owned owned-string operations, including Text Toolkit v1.
use super::*;

impl Evaluator<'_> {
    pub(super) fn evaluate_string_op(
        &mut self,
        op: crate::string_ops::StringOp,
        args: &[ResolvedExpr],
        environment: &mut Environment,
        depth: usize,
    ) -> Result<Value, Flow> {
        self.charge()?;
        let mut values = Vec::with_capacity(args.len());
        for (index, argument) in args.iter().enumerate() {
            // A borrowed map operand aliases its owner instead of moving it.
            if argument.ty == ResolvedType::StringMap
                && op.param_ownership(index) == hir::OwnershipMode::Borrow
            {
                let ResolvedExprKind::Place(place) = &argument.kind else {
                    return Err(Flow::Guard("borrowed map operand is not a named place"));
                };
                values.push(
                    self.lookup_place(environment, place)?
                        .ok_or(Flow::Guard("borrowed map owner is unavailable"))?,
                );
                continue;
            }
            values.push(self.evaluate(argument, environment, depth)?);
        }
        if op.is_text_toolkit() {
            return self.evaluate_text_toolkit(op, &values);
        }
        if op.is_collection() {
            return self.evaluate_collection(op, values);
        }
        if op.is_conversion() {
            return self.evaluate_conversion(op, &values);
        }
        match op {
            crate::string_ops::StringOp::Len => match values.first() {
                Some(Value::String(value)) => Ok(Value::Int(value.len() as i64)),
                _ => Err(Flow::Guard("ill-typed string operation operand")),
            },
            crate::string_ops::StringOp::IsEmpty => match values.first() {
                Some(Value::String(value)) => Ok(Value::Bool(value.is_empty())),
                _ => Err(Flow::Guard("ill-typed string operation operand")),
            },
            crate::string_ops::StringOp::Concat => match (values.first(), values.get(1)) {
                (Some(Value::String(left)), Some(Value::String(right))) => {
                    let length = left.len().checked_add(right.len()).ok_or(
                        Flow::Utf8MaterializationLimitExceeded {
                            attempted_materializations: u64::MAX,
                            attempted_bytes: u64::MAX,
                        },
                    )?;
                    self.charge_utf8_materialization(length)?;
                    let mut result = String::with_capacity(length);
                    result.push_str(left);
                    result.push_str(right);
                    Ok(Value::String(result))
                }
                _ => Err(Flow::Guard("ill-typed string operation operand")),
            },
            crate::string_ops::StringOp::StartsWith => match (values.first(), values.get(1)) {
                (Some(Value::String(value)), Some(Value::String(prefix))) => {
                    Ok(Value::Bool(value.starts_with(prefix.as_str())))
                }
                _ => Err(Flow::Guard("ill-typed string operation operand")),
            },
            crate::string_ops::StringOp::Contains => match (values.first(), values.get(1)) {
                (Some(Value::String(value)), Some(Value::String(needle))) => {
                    Ok(Value::Bool(value.contains(needle.as_str())))
                }
                _ => Err(Flow::Guard("ill-typed string operation operand")),
            },
            crate::string_ops::StringOp::LenChars => match values.first() {
                Some(Value::String(value)) => Ok(Value::Int(value.chars().count() as i64)),
                _ => Err(Flow::Guard("ill-typed string operation operand")),
            },
            crate::string_ops::StringOp::FromChar => match values.first() {
                Some(Value::Char(scalar)) => match char::from_u32(*scalar) {
                    Some(value) => {
                        let mut bytes = [0u8; 4];
                        let value = value.encode_utf8(&mut bytes);
                        Ok(Value::String(self.materialize_utf8_copy(value)?))
                    }
                    None => Err(Flow::Guard("ill-typed string operation operand")),
                },
                _ => Err(Flow::Guard("ill-typed string operation operand")),
            },
            crate::string_ops::StringOp::FromI64 => match values.first() {
                Some(Value::Int(value)) => Ok(Value::String(
                    self.materialize_utf8_copy(&value.to_string())?,
                )),
                _ => Err(Flow::Guard("ill-typed string operation operand")),
            },
            crate::string_ops::StringOp::FromUsize => match values.first() {
                Some(Value::Usize(value)) => Ok(Value::String(
                    self.materialize_utf8_copy(&value.to_string())?,
                )),
                _ => Err(Flow::Guard("ill-typed string operation operand")),
            },
            _ => Err(Flow::Guard("ill-typed string operation operand")),
        }
    }

    /// Text Toolkit v1. Offsets are byte offsets; every failure is the
    /// checked `semaprax.text.v1` status the native backend selects too.
    fn evaluate_text_toolkit(
        &mut self,
        op: crate::string_ops::StringOp,
        values: &[Value],
    ) -> Result<Value, Flow> {
        use crate::string_ops::StringOp;
        match (op, values) {
            (StringOp::Slice, [Value::String(text), Value::Int(start), Value::Int(end)]) => {
                let (start, end) = text_range(text, *start, *end)?;
                if !text.is_char_boundary(start) || !text.is_char_boundary(end) {
                    return Err(text_failure(crate::string_ops::TEXT_NOT_CHAR_BOUNDARY_CODE));
                }
                Ok(Value::String(
                    self.materialize_utf8_copy(&text[start..end])?,
                ))
            }
            (StringOp::Find, [Value::String(text), Value::String(needle), Value::Int(from)]) => {
                let (from, _) = text_range(text, *from, text.len() as i64)?;
                Ok(Value::Int(
                    crate::string_ops::find_bytes(text.as_bytes(), needle.as_bytes(), from)
                        .map_or(-1, |offset| offset as i64),
                ))
            }
            (StringOp::ToI64, [Value::String(text)]) => Ok(Value::OptionI64(
                crate::string_ops::parse_i64(text.as_bytes()),
            )),
            (StringOp::Trim, [Value::String(text)]) => {
                let trimmed = crate::string_ops::trim_range(text.as_bytes());
                Ok(Value::String(self.materialize_utf8_copy(&text[trimmed])?))
            }
            (StringOp::ByteAt, [Value::String(text), Value::Int(index)]) => {
                let (index, _) = text_range(text, *index, text.len() as i64)?;
                text.as_bytes()
                    .get(index)
                    .map(|byte| Value::Int(i64::from(*byte)))
                    .ok_or_else(|| text_failure(crate::string_ops::TEXT_OUT_OF_RANGE_CODE))
            }
            (StringOp::FileReadText, [Value::BorrowedStr(path)]) => {
                let bytes = self.read_file_text(path.bytes.as_ref())?;
                let text = String::from_utf8(bytes)
                    .map_err(|_| text_failure(crate::string_ops::TEXT_INVALID_UTF8_CODE))?;
                self.charge_utf8_materialization(text.len())?;
                Ok(Value::String(text))
            }
            _ => Err(Flow::Guard("ill-typed Text Toolkit operand")),
        }
    }
}

/// String Collections v1 map carrier: entries in ascending bytewise key
/// order, at most `capacity` of them.
#[derive(Debug, PartialEq)]
pub(super) struct StringMapValue {
    capacity: usize,
    entries: Vec<(String, i64)>,
}

impl StringMapValue {
    fn find(&self, key: &str) -> Result<usize, usize> {
        self.entries
            .binary_search_by(|(entry, _)| entry.as_bytes().cmp(key.as_bytes()))
    }
}

impl Evaluator<'_> {
    /// String Collections v1. Every failure is the checked `semaprax.map.v1`
    /// status the native backend selects too.
    fn evaluate_collection(
        &mut self,
        op: crate::string_ops::StringOp,
        values: Vec<Value>,
    ) -> Result<Value, Flow> {
        use crate::string_ops::StringOp;
        match (op, values.as_slice()) {
            (StringOp::Compare, [Value::String(left), Value::String(right)]) => Ok(Value::Int(
                crate::string_ops::compare_bytes(left.as_bytes(), right.as_bytes()),
            )),
            (StringOp::MapNew, [Value::Usize(capacity)]) => {
                if *capacity > crate::string_ops::MAX_MAP_CAPACITY {
                    return Err(map_failure(crate::string_ops::MAP_CAPACITY_CODE));
                }
                Ok(Value::Map(Arc::new(StringMapValue {
                    capacity: *capacity as usize,
                    entries: Vec::new(),
                })))
            }
            (
                StringOp::MapAdd | StringOp::MapSet,
                [Value::Map(_), Value::String(_), Value::Int(_)],
            ) => {
                let mut values = values.into_iter();
                let (Some(Value::Map(map)), Some(Value::String(key)), Some(Value::Int(value))) =
                    (values.next(), values.next(), values.next())
                else {
                    return Err(Flow::Guard("ill-typed map operand"));
                };
                let mut map =
                    Arc::try_unwrap(map).map_err(|_| Flow::Guard("aliased owned map carrier"))?;
                match map.find(&key) {
                    Ok(index) => {
                        let entry = &mut map.entries[index].1;
                        *entry = if op == StringOp::MapSet {
                            value
                        } else {
                            entry.checked_add(value).ok_or_else(|| {
                                map_failure(crate::string_ops::MAP_VALUE_OVERFLOW_CODE)
                            })?
                        };
                    }
                    Err(index) => {
                        if map.entries.len() >= map.capacity {
                            return Err(map_failure(crate::string_ops::MAP_FULL_CODE));
                        }
                        let key = self.materialize_utf8_copy(&key)?;
                        map.entries.insert(index, (key, value));
                    }
                }
                Ok(Value::Map(Arc::new(map)))
            }
            (StringOp::MapGetOr, [Value::Map(map), Value::String(key), Value::Int(fallback)]) => {
                Ok(Value::Int(
                    map.find(key)
                        .map_or(*fallback, |index| map.entries[index].1),
                ))
            }
            (StringOp::MapHas, [Value::Map(map), Value::String(key)]) => {
                Ok(Value::Bool(map.find(key).is_ok()))
            }
            (StringOp::MapLen, [Value::Map(map)]) => Ok(Value::Usize(map.entries.len() as u64)),
            (StringOp::MapKeyAt, [Value::Map(map), Value::Usize(index)]) => {
                let key = usize::try_from(*index)
                    .ok()
                    .and_then(|index| map.entries.get(index))
                    .map(|(key, _)| key.clone())
                    .ok_or_else(|| map_failure(crate::string_ops::MAP_INDEX_OUT_OF_RANGE_CODE))?;
                Ok(Value::String(self.materialize_utf8_copy(&key)?))
            }
            (StringOp::MapValueAt, [Value::Map(map), Value::Usize(index)]) => {
                usize::try_from(*index)
                    .ok()
                    .and_then(|index| map.entries.get(index))
                    .map(|(_, value)| Value::Int(*value))
                    .ok_or_else(|| map_failure(crate::string_ops::MAP_INDEX_OUT_OF_RANGE_CODE))
            }
            _ => Err(Flow::Guard("ill-typed String Collections operand")),
        }
    }
}

impl Evaluator<'_> {
    /// Conversions v1. Every failure is the checked `semaprax.convert.v1`
    /// status the native backend selects too.
    fn evaluate_conversion(
        &mut self,
        op: crate::string_ops::StringOp,
        values: &[Value],
    ) -> Result<Value, Flow> {
        use crate::string_ops::{StringOp, CONVERT_NAN_CODE, CONVERT_OUT_OF_RANGE_CODE};
        match (op, values) {
            (StringOp::FromStr, [Value::BorrowedStr(text)]) => {
                let text = std::str::from_utf8(text.bytes.as_ref())
                    .map_err(|_| Flow::Guard("ill-typed borrowed string operand"))?;
                Ok(Value::String(self.materialize_utf8_copy(text)?))
            }
            // Rust's `as` rounds to nearest, ties to even, like C and Wasm.
            (StringOp::F64FromI64, [Value::Int(value)]) => Ok(Value::Float64(*value as f64)),
            (StringOp::I64FromF64, [Value::Float64(value)]) => {
                if value.is_nan() {
                    Err(convert_failure(CONVERT_NAN_CODE))
                } else if *value >= -9_223_372_036_854_775_808.0
                    && *value < 9_223_372_036_854_775_808.0
                {
                    Ok(Value::Int(value.trunc() as i64))
                } else {
                    Err(convert_failure(CONVERT_OUT_OF_RANGE_CODE))
                }
            }
            (StringOp::UsizeFromI64, [Value::Int(value)]) => u64::try_from(*value)
                .map(Value::Usize)
                .map_err(|_| convert_failure(CONVERT_OUT_OF_RANGE_CODE)),
            (StringOp::I64FromUsize, [Value::Usize(value)]) => i64::try_from(*value)
                .map(Value::Int)
                .map_err(|_| convert_failure(CONVERT_OUT_OF_RANGE_CODE)),
            _ => Err(Flow::Guard("ill-typed conversion operand")),
        }
    }
}

fn convert_failure(code: u32) -> Flow {
    Flow::Failure(
        NormalizedStatus::try_new(
            crate::string_ops::CONVERT_STATUS_DOMAIN,
            code,
            StatusClass::Adapter,
            Retryability::Known(false),
        )
        .expect("compiler-owned conversion status table is valid"),
    )
}

fn map_failure(code: u32) -> Flow {
    Flow::Failure(
        NormalizedStatus::try_new(
            crate::string_ops::MAP_STATUS_DOMAIN,
            code,
            StatusClass::Adapter,
            Retryability::Known(false),
        )
        .expect("compiler-owned map status table is valid"),
    )
}

/// Check `0 <= start <= end <= len` and convert to native offsets.
fn text_range(text: &str, start: i64, end: i64) -> Result<(usize, usize), Flow> {
    let length = text.len() as i64;
    if start < 0 || start > end || end > length {
        return Err(text_failure(crate::string_ops::TEXT_OUT_OF_RANGE_CODE));
    }
    Ok((start as usize, end as usize))
}

fn text_failure(code: u32) -> Flow {
    Flow::Failure(
        NormalizedStatus::try_new(
            crate::string_ops::TEXT_STATUS_DOMAIN,
            code,
            StatusClass::Adapter,
            Retryability::Known(false),
        )
        .expect("compiler-owned text status table is valid"),
    )
}
