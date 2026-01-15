//! Value encoding/decoding for GRC-20 binary format.
//!
//! Implements the wire format for property values (spec Section 6.5).

use std::borrow::Cow;

use crate::codec::primitives::{Reader, Writer};
use crate::error::{DecodeError, EncodeError};
use crate::limits::{MAX_BYTES_LEN, MAX_EMBEDDING_BYTES, MAX_EMBEDDING_DIMS, MAX_POSITION_LEN, MAX_STRING_LEN};
use crate::model::{
    DataType, DecimalMantissa, DictionaryBuilder, EmbeddingSubType, PropertyValue, Value,
    WireDictionaries,
};

// =============================================================================
// DECODING
// =============================================================================

/// Decodes a Value from the reader based on the data type (zero-copy).
pub fn decode_value<'a>(
    reader: &mut Reader<'a>,
    data_type: DataType,
    dicts: &WireDictionaries,
) -> Result<Value<'a>, DecodeError> {
    match data_type {
        DataType::Bool => decode_bool(reader),
        DataType::Int64 => decode_int64(reader, dicts),
        DataType::Float64 => decode_float64(reader, dicts),
        DataType::Decimal => decode_decimal(reader, dicts),
        DataType::Text => decode_text(reader, dicts),
        DataType::Bytes => decode_bytes(reader),
        DataType::Date => decode_date(reader),
        DataType::Time => decode_time(reader),
        DataType::Datetime => decode_datetime(reader),
        DataType::Schedule => decode_schedule(reader),
        DataType::Point => decode_point(reader),
        DataType::Embedding => decode_embedding(reader),
    }
}

fn decode_bool<'a>(reader: &mut Reader<'a>) -> Result<Value<'a>, DecodeError> {
    let byte = reader.read_byte("bool")?;
    match byte {
        0x00 => Ok(Value::Bool(false)),
        0x01 => Ok(Value::Bool(true)),
        _ => Err(DecodeError::InvalidBool { value: byte }),
    }
}

fn decode_int64<'a>(reader: &mut Reader<'a>, dicts: &WireDictionaries) -> Result<Value<'a>, DecodeError> {
    let value = reader.read_signed_varint("int64")?;
    let unit_index = reader.read_varint("int64.unit")? as usize;
    let unit = if unit_index == 0 {
        None
    } else {
        let idx = unit_index - 1;
        if idx >= dicts.units.len() {
            return Err(DecodeError::IndexOutOfBounds {
                dict: "units",
                index: unit_index,
                size: dicts.units.len() + 1,
            });
        }
        Some(dicts.units[idx])
    };
    Ok(Value::Int64 { value, unit })
}

fn decode_float64<'a>(reader: &mut Reader<'a>, dicts: &WireDictionaries) -> Result<Value<'a>, DecodeError> {
    let value = reader.read_f64("float64")?;
    let unit_index = reader.read_varint("float64.unit")? as usize;
    let unit = if unit_index == 0 {
        None
    } else {
        let idx = unit_index - 1;
        if idx >= dicts.units.len() {
            return Err(DecodeError::IndexOutOfBounds {
                dict: "units",
                index: unit_index,
                size: dicts.units.len() + 1,
            });
        }
        Some(dicts.units[idx])
    };
    Ok(Value::Float64 { value, unit })
}

fn decode_decimal<'a>(reader: &mut Reader<'a>, dicts: &WireDictionaries) -> Result<Value<'a>, DecodeError> {
    let exponent = reader.read_signed_varint("decimal.exponent")? as i32;
    let mantissa_type = reader.read_byte("decimal.mantissa_type")?;

    let mantissa = match mantissa_type {
        0x00 => {
            let v = reader.read_signed_varint("decimal.mantissa")?;
            DecimalMantissa::I64(v)
        }
        0x01 => {
            let len = reader.read_varint("decimal.mantissa_len")? as usize;
            let bytes = reader.read_bytes(len, "decimal.mantissa_bytes")?;

            // Validate minimal encoding
            if !bytes.is_empty() {
                let first = bytes[0];
                // Check for redundant sign extension
                if bytes.len() > 1 {
                    let second = bytes[1];
                    if (first == 0x00 && (second & 0x80) == 0)
                        || (first == 0xFF && (second & 0x80) != 0) {
                        return Err(DecodeError::DecimalMantissaNotMinimal);
                    }
                }
            }

            DecimalMantissa::Big(Cow::Borrowed(bytes))
        }
        _ => {
            return Err(DecodeError::MalformedEncoding {
                context: "invalid decimal mantissa type"
            });
        }
    };

    // Validate normalization
    match &mantissa {
        DecimalMantissa::I64(v) => {
            if *v == 0 {
                if exponent != 0 {
                    return Err(DecodeError::DecimalNotNormalized);
                }
            } else if *v % 10 == 0 {
                return Err(DecodeError::DecimalNotNormalized);
            }
        }
        DecimalMantissa::Big(bytes) => {
            if is_big_mantissa_zero(bytes) {
                if exponent != 0 {
                    return Err(DecodeError::DecimalNotNormalized);
                }
            } else if is_big_mantissa_divisible_by_10(bytes) {
                return Err(DecodeError::DecimalNotNormalized);
            }
        }
    }

    let unit_index = reader.read_varint("decimal.unit")? as usize;
    let unit = if unit_index == 0 {
        None
    } else {
        let idx = unit_index - 1;
        if idx >= dicts.units.len() {
            return Err(DecodeError::IndexOutOfBounds {
                dict: "units",
                index: unit_index,
                size: dicts.units.len() + 1,
            });
        }
        Some(dicts.units[idx])
    };

    Ok(Value::Decimal { exponent, mantissa, unit })
}

/// Checks if a big-endian two's complement mantissa represents zero.
fn is_big_mantissa_zero(bytes: &[u8]) -> bool {
    bytes.iter().all(|&b| b == 0)
}

/// Checks if a big-endian two's complement mantissa is divisible by 10.
///
/// A number is divisible by 10 if its remainder when divided by 10 is 0.
/// For big-endian bytes, we compute: sum(byte[i] * 256^(n-1-i)) mod 10.
/// Since 256 mod 10 = 6, we can compute iteratively: (carry * 6 + byte) mod 10.
///
/// For negative numbers (high bit set), we need to handle two's complement.
fn is_big_mantissa_divisible_by_10(bytes: &[u8]) -> bool {
    if bytes.is_empty() {
        return true; // Zero is divisible by 10
    }

    // Check if negative (high bit set)
    let is_negative = bytes[0] & 0x80 != 0;

    if is_negative {
        // For negative two's complement, compute the absolute value first
        // by inverting bits and adding 1, then check divisibility
        let abs_mod = twos_complement_abs_mod_10(bytes);
        abs_mod == 0
    } else {
        // Positive: just compute mod 10 directly
        // 256 mod 10 = 6, so we iterate: remainder = (remainder * 6 + byte) mod 10
        let mut remainder = 0u32;
        for &byte in bytes {
            // remainder * 256 + byte, mod 10
            // Since 256 = 25 * 10 + 6, we have: (r * 256) mod 10 = (r * 6) mod 10
            remainder = (remainder * 6 + byte as u32) % 10;
        }
        remainder == 0
    }
}

/// Computes |x| mod 10 for a negative two's complement number.
fn twos_complement_abs_mod_10(bytes: &[u8]) -> u32 {
    // Two's complement negation: invert all bits and add 1
    // To get |x| mod 10, we compute (-x) mod 10
    //
    // For a two's complement negative number x (represented in bytes),
    // -x = ~x + 1 (bit inversion plus one)
    //
    // We compute (inverted bytes) mod 10, then add 1 mod 10

    // First, compute (inverted bytes as big-endian unsigned) mod 10
    let mut remainder = 0u32;
    for &byte in bytes {
        let inverted = !byte;
        remainder = (remainder * 6 + inverted as u32) % 10;
    }

    // Add 1 (for two's complement)
    (remainder + 1) % 10
}

fn decode_text<'a>(reader: &mut Reader<'a>, dicts: &WireDictionaries) -> Result<Value<'a>, DecodeError> {
    let value = reader.read_str(MAX_STRING_LEN, "text")?;
    let lang_index = reader.read_varint("text.language")? as usize;

    let language = if lang_index == 0 {
        None
    } else {
        let idx = lang_index - 1;
        if idx >= dicts.languages.len() {
            return Err(DecodeError::IndexOutOfBounds {
                dict: "languages",
                index: lang_index,
                size: dicts.languages.len() + 1, // +1 for index 0
            });
        }
        Some(dicts.languages[idx])
    };

    Ok(Value::Text { value: Cow::Borrowed(value), language })
}

fn decode_bytes<'a>(reader: &mut Reader<'a>) -> Result<Value<'a>, DecodeError> {
    let len = reader.read_varint("bytes.len")? as usize;
    if len > MAX_BYTES_LEN {
        return Err(DecodeError::LengthExceedsLimit {
            field: "bytes",
            len,
            max: MAX_BYTES_LEN,
        });
    }
    let bytes = reader.read_bytes(len, "bytes")?;
    Ok(Value::Bytes(Cow::Borrowed(bytes)))
}

fn decode_date<'a>(reader: &mut Reader<'a>) -> Result<Value<'a>, DecodeError> {
    let value = reader.read_str(MAX_STRING_LEN, "date")?;
    // Basic validation: DATE should not contain 'T' (that's DATETIME)
    if value.contains('T') {
        return Err(DecodeError::MalformedEncoding {
            context: "DATE should not contain 'T' separator (use DATETIME instead)",
        });
    }
    Ok(Value::Date(Cow::Borrowed(value)))
}

fn decode_time<'a>(reader: &mut Reader<'a>) -> Result<Value<'a>, DecodeError> {
    let value = reader.read_str(MAX_STRING_LEN, "time")?;
    // Basic validation: TIME should have timezone (Z or +/- offset)
    if !value.contains('Z') && !value.contains('+') && !value.rfind('-').map(|p| p >= 8).unwrap_or(false) {
        return Err(DecodeError::MalformedEncoding {
            context: "TIME must include timezone (Z or offset)",
        });
    }
    Ok(Value::Time(Cow::Borrowed(value)))
}

fn decode_datetime<'a>(reader: &mut Reader<'a>) -> Result<Value<'a>, DecodeError> {
    let value = reader.read_str(MAX_STRING_LEN, "datetime")?;
    // Basic validation: DATETIME should contain 'T'
    if !value.contains('T') {
        return Err(DecodeError::MalformedEncoding {
            context: "DATETIME must contain 'T' separator",
        });
    }
    // DATETIME must include timezone (Z or offset)
    // For datetime strings, timezone appears after 'T', so check for:
    // - 'Z' anywhere (UTC)
    // - '+' anywhere (positive offset like +05:30)
    // - '-' after the 'T' position (negative offset like -05:00)
    let t_pos = value.find('T').unwrap(); // Safe: we just checked for 'T'
    let has_timezone = value.contains('Z')
        || value.contains('+')
        || value[t_pos..].contains('-');
    if !has_timezone {
        return Err(DecodeError::MalformedEncoding {
            context: "DATETIME must include timezone (Z or offset)",
        });
    }
    Ok(Value::Datetime(Cow::Borrowed(value)))
}

fn decode_schedule<'a>(reader: &mut Reader<'a>) -> Result<Value<'a>, DecodeError> {
    let value = reader.read_str(MAX_STRING_LEN, "schedule")?;
    // RFC 5545 iCalendar format - basic validation
    // Full validation would require a complete iCalendar parser
    Ok(Value::Schedule(Cow::Borrowed(value)))
}

fn decode_point<'a>(reader: &mut Reader<'a>) -> Result<Value<'a>, DecodeError> {
    let ordinate_count = reader.read_byte("point.ordinate_count")?;

    if ordinate_count != 2 && ordinate_count != 3 {
        return Err(DecodeError::MalformedEncoding {
            context: "POINT ordinate_count must be 2 or 3",
        });
    }

    // Read in wire order: longitude, latitude, altitude (optional)
    let lon = reader.read_f64("point.lon")?;
    let lat = reader.read_f64("point.lat")?;
    let alt = if ordinate_count == 3 {
        Some(reader.read_f64("point.alt")?)
    } else {
        None
    };

    // Validate bounds
    if !(-180.0..=180.0).contains(&lon) {
        return Err(DecodeError::LongitudeOutOfRange { lon });
    }
    if !(-90.0..=90.0).contains(&lat) {
        return Err(DecodeError::LatitudeOutOfRange { lat });
    }
    if lon.is_nan() || lat.is_nan() {
        return Err(DecodeError::FloatIsNan);
    }
    if let Some(a) = alt {
        if a.is_nan() {
            return Err(DecodeError::FloatIsNan);
        }
    }

    Ok(Value::Point { lon, lat, alt })
}

fn decode_embedding<'a>(reader: &mut Reader<'a>) -> Result<Value<'a>, DecodeError> {
    let sub_type_byte = reader.read_byte("embedding.sub_type")?;
    let sub_type = EmbeddingSubType::from_u8(sub_type_byte)
        .ok_or(DecodeError::InvalidEmbeddingSubType { sub_type: sub_type_byte })?;

    let dims = reader.read_varint("embedding.dims")? as usize;
    if dims > MAX_EMBEDDING_DIMS {
        return Err(DecodeError::LengthExceedsLimit {
            field: "embedding.dims",
            len: dims,
            max: MAX_EMBEDDING_DIMS,
        });
    }

    let expected_bytes = sub_type.bytes_for_dims(dims);
    if expected_bytes > MAX_EMBEDDING_BYTES {
        return Err(DecodeError::LengthExceedsLimit {
            field: "embedding.data",
            len: expected_bytes,
            max: MAX_EMBEDDING_BYTES,
        });
    }

    let data = reader.read_bytes(expected_bytes, "embedding.data")?;

    // Validate no NaN in float32 embeddings
    if sub_type == EmbeddingSubType::Float32 {
        for chunk in data.chunks_exact(4) {
            let f = f32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]);
            if f.is_nan() {
                return Err(DecodeError::FloatIsNan);
            }
        }
    }

    // Validate binary embedding has zeros in unused bits
    if sub_type == EmbeddingSubType::Binary && dims % 8 != 0 {
        let last_byte = data[data.len() - 1];
        let unused_bits = 8 - (dims % 8);
        let mask = !((1u8 << (8 - unused_bits)) - 1);
        if last_byte & mask != 0 {
            return Err(DecodeError::MalformedEncoding {
                context: "binary embedding has non-zero unused bits",
            });
        }
    }

    Ok(Value::Embedding { sub_type, dims, data: Cow::Borrowed(data) })
}

/// Decodes a PropertyValue (property index + value + optional language).
pub fn decode_property_value<'a>(
    reader: &mut Reader<'a>,
    dicts: &WireDictionaries,
) -> Result<PropertyValue<'a>, DecodeError> {
    let prop_index = reader.read_varint("property")? as usize;
    if prop_index >= dicts.properties.len() {
        return Err(DecodeError::IndexOutOfBounds {
            dict: "properties",
            index: prop_index,
            size: dicts.properties.len(),
        });
    }

    let (property, data_type) = dicts.properties[prop_index];
    let value = decode_value(reader, data_type, dicts)?;

    Ok(PropertyValue { property, value })
}

// =============================================================================
// ENCODING
// =============================================================================

/// Encodes a Value to the writer.
pub fn encode_value(
    writer: &mut Writer,
    value: &Value<'_>,
    dict_builder: &mut DictionaryBuilder,
) -> Result<(), EncodeError> {
    match value {
        Value::Bool(v) => {
            writer.write_byte(if *v { 0x01 } else { 0x00 });
        }
        Value::Int64 { value, unit } => {
            writer.write_signed_varint(*value);
            let unit_index = dict_builder.add_unit(*unit);
            writer.write_varint(unit_index as u64);
        }
        Value::Float64 { value, unit } => {
            if value.is_nan() {
                return Err(EncodeError::FloatIsNan);
            }
            writer.write_f64(*value);
            let unit_index = dict_builder.add_unit(*unit);
            writer.write_varint(unit_index as u64);
        }
        Value::Decimal { exponent, mantissa, unit } => {
            encode_decimal(writer, *exponent, mantissa)?;
            let unit_index = dict_builder.add_unit(*unit);
            writer.write_varint(unit_index as u64);
        }
        Value::Text { value, language } => {
            writer.write_string(value);
            let lang_index = dict_builder.add_language(*language);
            writer.write_varint(lang_index as u64);
        }
        Value::Bytes(bytes) => {
            writer.write_bytes_prefixed(bytes);
        }
        Value::Date(value) => {
            // DATE should not contain 'T' (that's DATETIME)
            if value.contains('T') {
                return Err(EncodeError::InvalidDate {
                    reason: "DATE should not contain 'T' separator (use DATETIME instead)",
                });
            }
            writer.write_string(value);
        }
        Value::Time(value) => {
            // TIME must have timezone
            if !value.contains('Z') && !value.contains('+') && !value.rfind('-').map(|p| p >= 8).unwrap_or(false) {
                return Err(EncodeError::InvalidInput {
                    context: "TIME must include timezone (Z or offset)",
                });
            }
            writer.write_string(value);
        }
        Value::Datetime(value) => {
            // DATETIME must contain 'T'
            if !value.contains('T') {
                return Err(EncodeError::InvalidInput {
                    context: "DATETIME must contain 'T' separator",
                });
            }
            // DATETIME must include timezone (Z or offset)
            let t_pos = value.find('T').unwrap(); // Safe: we just checked for 'T'
            let has_timezone = value.contains('Z')
                || value.contains('+')
                || value[t_pos..].contains('-');
            if !has_timezone {
                return Err(EncodeError::InvalidInput {
                    context: "DATETIME must include timezone (Z or offset)",
                });
            }
            writer.write_string(value);
        }
        Value::Schedule(s) => {
            // RFC 5545 iCalendar format
            writer.write_string(s);
        }
        Value::Point { lon, lat, alt } => {
            if *lon < -180.0 || *lon > 180.0 {
                return Err(EncodeError::LongitudeOutOfRange { lon: *lon });
            }
            if *lat < -90.0 || *lat > 90.0 {
                return Err(EncodeError::LatitudeOutOfRange { lat: *lat });
            }
            if lat.is_nan() || lon.is_nan() {
                return Err(EncodeError::FloatIsNan);
            }
            if let Some(a) = alt {
                if a.is_nan() {
                    return Err(EncodeError::FloatIsNan);
                }
            }
            // Write ordinate_count: 2 for 2D, 3 for 3D
            let ordinate_count = if alt.is_some() { 3u8 } else { 2u8 };
            writer.write_byte(ordinate_count);
            // Write in wire order: longitude, latitude, altitude (optional)
            writer.write_f64(*lon);
            writer.write_f64(*lat);
            if let Some(a) = alt {
                writer.write_f64(*a);
            }
        }
        Value::Embedding { sub_type, dims, data } => {
            let expected = sub_type.bytes_for_dims(*dims);
            if data.len() != expected {
                return Err(EncodeError::EmbeddingDimensionMismatch {
                    sub_type: *sub_type as u8,
                    dims: *dims,
                    data_len: data.len(),
                });
            }
            // Check for NaN in float32
            if *sub_type == EmbeddingSubType::Float32 {
                for chunk in data.chunks_exact(4) {
                    let f = f32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]);
                    if f.is_nan() {
                        return Err(EncodeError::FloatIsNan);
                    }
                }
            }
            writer.write_byte(*sub_type as u8);
            writer.write_varint(*dims as u64);
            writer.write_bytes(data);
        }
    }
    Ok(())
}

fn encode_decimal(
    writer: &mut Writer,
    exponent: i32,
    mantissa: &DecimalMantissa<'_>,
) -> Result<(), EncodeError> {
    // Validate normalization
    match mantissa {
        DecimalMantissa::I64(v) => {
            if *v == 0 {
                if exponent != 0 {
                    return Err(EncodeError::DecimalNotNormalized);
                }
            } else if *v % 10 == 0 {
                return Err(EncodeError::DecimalNotNormalized);
            }
        }
        DecimalMantissa::Big(bytes) => {
            if is_big_mantissa_zero(bytes) {
                if exponent != 0 {
                    return Err(EncodeError::DecimalNotNormalized);
                }
            } else if is_big_mantissa_divisible_by_10(bytes) {
                return Err(EncodeError::DecimalNotNormalized);
            }
        }
    }

    writer.write_signed_varint(exponent as i64);

    match mantissa {
        DecimalMantissa::I64(v) => {
            writer.write_byte(0x00);
            writer.write_signed_varint(*v);
        }
        DecimalMantissa::Big(bytes) => {
            writer.write_byte(0x01);
            writer.write_varint(bytes.len() as u64);
            writer.write_bytes(bytes);
        }
    }

    Ok(())
}

/// Encodes a PropertyValue (property index + value + optional language).
pub fn encode_property_value(
    writer: &mut Writer,
    pv: &PropertyValue<'_>,
    dict_builder: &mut DictionaryBuilder,
    data_type: DataType,
) -> Result<(), EncodeError> {
    let prop_index = dict_builder.add_property(pv.property, data_type);
    writer.write_varint(prop_index as u64);
    encode_value(writer, &pv.value, dict_builder)?;
    Ok(())
}

/// Validates a position string according to spec rules.
pub fn validate_position(pos: &str) -> Result<(), EncodeError> {
    if pos.len() > MAX_POSITION_LEN {
        return Err(EncodeError::PositionTooLong);
    }
    for c in pos.chars() {
        if !c.is_ascii_alphanumeric() {
            return Err(EncodeError::InvalidPositionChar);
        }
    }
    Ok(())
}

/// Decodes a position string with validation (zero-copy).
pub fn decode_position<'a>(reader: &mut Reader<'a>) -> Result<Cow<'a, str>, DecodeError> {
    let pos = reader.read_str(MAX_POSITION_LEN, "position")?;
    for c in pos.chars() {
        if !c.is_ascii_alphanumeric() {
            return Err(DecodeError::InvalidPositionChar { char: c });
        }
    }
    Ok(Cow::Borrowed(pos))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_bool_roundtrip() {
        for v in [true, false] {
            let value = Value::Bool(v);
            let dicts = WireDictionaries::default();
            let mut dict_builder = DictionaryBuilder::new();

            let mut writer = Writer::new();
            encode_value(&mut writer, &value, &mut dict_builder).unwrap();

            let mut reader = Reader::new(writer.as_bytes());
            let decoded = decode_value(&mut reader, DataType::Bool, &dicts).unwrap();

            assert_eq!(value, decoded);
        }
    }

    #[test]
    fn test_int64_roundtrip() {
        for v in [0i64, 1, -1, i64::MAX, i64::MIN, 12345678] {
            let value = Value::Int64 { value: v, unit: None };
            let mut dict_builder = DictionaryBuilder::new();

            let mut writer = Writer::new();
            encode_value(&mut writer, &value, &mut dict_builder).unwrap();

            let dicts = dict_builder.build();
            let mut reader = Reader::new(writer.as_bytes());
            let decoded = decode_value(&mut reader, DataType::Int64, &dicts).unwrap();

            assert_eq!(value, decoded);
        }
    }

    #[test]
    fn test_float64_roundtrip() {
        for v in [0.0, 1.0, -1.0, f64::INFINITY, f64::NEG_INFINITY, 3.14159] {
            let value = Value::Float64 { value: v, unit: None };
            let mut dict_builder = DictionaryBuilder::new();

            let mut writer = Writer::new();
            encode_value(&mut writer, &value, &mut dict_builder).unwrap();

            let dicts = dict_builder.build();
            let mut reader = Reader::new(writer.as_bytes());
            let decoded = decode_value(&mut reader, DataType::Float64, &dicts).unwrap();

            assert_eq!(value, decoded);
        }
    }

    #[test]
    fn test_text_roundtrip() {
        let value = Value::Text {
            value: Cow::Owned("hello world".to_string()),
            language: None,
        };
        let mut dict_builder = DictionaryBuilder::new();

        let mut writer = Writer::new();
        encode_value(&mut writer, &value, &mut dict_builder).unwrap();

        // Build dicts for decoding
        let decode_dicts = dict_builder.build();

        let mut reader = Reader::new(writer.as_bytes());
        let decoded = decode_value(&mut reader, DataType::Text, &decode_dicts).unwrap();

        // Compare inner values since one is Owned and one is Borrowed
        match (&value, &decoded) {
            (Value::Text { value: v1, language: l1 }, Value::Text { value: v2, language: l2 }) => {
                assert_eq!(v1.as_ref(), v2.as_ref());
                assert_eq!(l1, l2);
            }
            _ => panic!("expected Text values"),
        }
    }

    #[test]
    fn test_point_roundtrip() {
        // 2D point (no altitude)
        let value = Value::Point { lon: -122.4194, lat: 37.7749, alt: None };
        let dicts = WireDictionaries::default();
        let mut dict_builder = DictionaryBuilder::new();

        let mut writer = Writer::new();
        encode_value(&mut writer, &value, &mut dict_builder).unwrap();

        let mut reader = Reader::new(writer.as_bytes());
        let decoded = decode_value(&mut reader, DataType::Point, &dicts).unwrap();

        assert_eq!(value, decoded);

        // 3D point (with altitude)
        let value_3d = Value::Point { lon: -122.4194, lat: 37.7749, alt: Some(100.0) };
        let mut dict_builder = DictionaryBuilder::new();

        let mut writer = Writer::new();
        encode_value(&mut writer, &value_3d, &mut dict_builder).unwrap();

        let mut reader = Reader::new(writer.as_bytes());
        let decoded_3d = decode_value(&mut reader, DataType::Point, &dicts).unwrap();

        assert_eq!(value_3d, decoded_3d);
    }

    #[test]
    fn test_point_validation() {
        // Latitude out of range
        let value = Value::Point { lon: 0.0, lat: 91.0, alt: None };
        let mut dict_builder = DictionaryBuilder::new();
        let mut writer = Writer::new();
        let result = encode_value(&mut writer, &value, &mut dict_builder);
        assert!(result.is_err());

        // Longitude out of range
        let value = Value::Point { lon: 181.0, lat: 0.0, alt: None };
        let mut dict_builder = DictionaryBuilder::new();
        let mut writer = Writer::new();
        let result = encode_value(&mut writer, &value, &mut dict_builder);
        assert!(result.is_err());

        // NaN in altitude
        let value = Value::Point { lon: 0.0, lat: 0.0, alt: Some(f64::NAN) };
        let mut dict_builder = DictionaryBuilder::new();
        let mut writer = Writer::new();
        let result = encode_value(&mut writer, &value, &mut dict_builder);
        assert!(result.is_err());
    }

    #[test]
    fn test_schedule_roundtrip() {
        let dicts = WireDictionaries::default();
        let mut dict_builder = DictionaryBuilder::new();

        // Simple iCalendar event (single occurrence)
        let value = Value::Schedule(Cow::Owned("BEGIN:VEVENT\r\nDTSTART:20240315T090000Z\r\nDTEND:20240315T100000Z\r\nEND:VEVENT".to_string()));

        let mut writer = Writer::new();
        encode_value(&mut writer, &value, &mut dict_builder).unwrap();

        let mut reader = Reader::new(writer.as_bytes());
        let decoded = decode_value(&mut reader, DataType::Schedule, &dicts).unwrap();

        match (&value, &decoded) {
            (Value::Schedule(s1), Value::Schedule(s2)) => {
                assert_eq!(s1.as_ref(), s2.as_ref());
            }
            _ => panic!("expected Schedule values"),
        }
    }

    #[test]
    fn test_embedding_roundtrip() {
        let value = Value::Embedding {
            sub_type: EmbeddingSubType::Float32,
            dims: 4,
            data: Cow::Owned(vec![0u8; 16]), // 4 dims * 4 bytes
        };
        let dicts = WireDictionaries::default();
        let mut dict_builder = DictionaryBuilder::new();

        let mut writer = Writer::new();
        encode_value(&mut writer, &value, &mut dict_builder).unwrap();

        let mut reader = Reader::new(writer.as_bytes());
        let decoded = decode_value(&mut reader, DataType::Embedding, &dicts).unwrap();

        // Compare inner values since one is Owned and one is Borrowed
        match (&value, &decoded) {
            (
                Value::Embedding { sub_type: s1, dims: d1, data: data1 },
                Value::Embedding { sub_type: s2, dims: d2, data: data2 },
            ) => {
                assert_eq!(s1, s2);
                assert_eq!(d1, d2);
                assert_eq!(data1.as_ref(), data2.as_ref());
            }
            _ => panic!("expected Embedding values"),
        }
    }

    #[test]
    fn test_decimal_normalized() {
        // Valid: 12.34 = 1234 * 10^-2
        let valid = Value::Decimal {
            exponent: -2,
            mantissa: DecimalMantissa::I64(1234),
            unit: None,
        };
        let mut dict_builder = DictionaryBuilder::new();
        let mut writer = Writer::new();
        assert!(encode_value(&mut writer, &valid, &mut dict_builder).is_ok());

        // Invalid: has trailing zeros
        let invalid = Value::Decimal {
            exponent: -2,
            mantissa: DecimalMantissa::I64(1230),
            unit: None,
        };
        let mut dict_builder = DictionaryBuilder::new();
        let mut writer = Writer::new();
        assert!(encode_value(&mut writer, &invalid, &mut dict_builder).is_err());
    }

    #[test]
    fn test_date_roundtrip() {
        let dicts = WireDictionaries::default();
        let mut dict_builder = DictionaryBuilder::new();

        // Test various date formats
        let test_cases = [
            "2024",
            "2024-03",
            "2024-03-15",
            "-0100",
            "-0100-03-15",
        ];

        for date_str in test_cases {
            let value = Value::Date(Cow::Owned(date_str.to_string()));

            let mut writer = Writer::new();
            encode_value(&mut writer, &value, &mut dict_builder).unwrap();

            let mut reader = Reader::new(writer.as_bytes());
            let decoded = decode_value(&mut reader, DataType::Date, &dicts).unwrap();

            match (&value, &decoded) {
                (Value::Date(v1), Value::Date(v2)) => {
                    assert_eq!(v1.as_ref(), v2.as_ref());
                }
                _ => panic!("expected Date values"),
            }
        }
    }

    #[test]
    fn test_time_roundtrip() {
        let dicts = WireDictionaries::default();
        let mut dict_builder = DictionaryBuilder::new();

        // Test various time formats (all with timezone)
        let test_cases = [
            "14:30:00Z",
            "14:30:00.000Z",
            "14:30:00+05:30",
            "00:00:00Z",
        ];

        for time_str in test_cases {
            let value = Value::Time(Cow::Owned(time_str.to_string()));

            let mut writer = Writer::new();
            encode_value(&mut writer, &value, &mut dict_builder).unwrap();

            let mut reader = Reader::new(writer.as_bytes());
            let decoded = decode_value(&mut reader, DataType::Time, &dicts).unwrap();

            match (&value, &decoded) {
                (Value::Time(v1), Value::Time(v2)) => {
                    assert_eq!(v1.as_ref(), v2.as_ref());
                }
                _ => panic!("expected Time values"),
            }
        }
    }

    #[test]
    fn test_datetime_roundtrip() {
        let dicts = WireDictionaries::default();
        let mut dict_builder = DictionaryBuilder::new();

        // Test various datetime formats (all with timezone)
        let test_cases = [
            "2024-03-15T14:30:00Z",
            "2024-03-15T14:30:00.000Z",
            "2024-03-15T14:30:00+05:30",
            "2024-03-15T14:30:00-05:00",
            "2025-11-18T05:00:00.000Z",
        ];

        for dt_str in test_cases {
            let value = Value::Datetime(Cow::Owned(dt_str.to_string()));

            let mut writer = Writer::new();
            encode_value(&mut writer, &value, &mut dict_builder).unwrap();

            let mut reader = Reader::new(writer.as_bytes());
            let decoded = decode_value(&mut reader, DataType::Datetime, &dicts).unwrap();

            match (&value, &decoded) {
                (Value::Datetime(v1), Value::Datetime(v2)) => {
                    assert_eq!(v1.as_ref(), v2.as_ref());
                }
                _ => panic!("expected Datetime values"),
            }
        }
    }

    #[test]
    fn test_date_validation() {
        let mut dict_builder = DictionaryBuilder::new();

        // DATE should reject strings with 'T'
        let invalid = Value::Date(Cow::Borrowed("2024-03-15T14:30:00Z"));
        let mut writer = Writer::new();
        assert!(encode_value(&mut writer, &invalid, &mut dict_builder).is_err());
    }

    #[test]
    fn test_time_validation() {
        let mut dict_builder = DictionaryBuilder::new();

        // TIME should reject strings without timezone
        let invalid = Value::Time(Cow::Borrowed("14:30:00"));
        let mut writer = Writer::new();
        assert!(encode_value(&mut writer, &invalid, &mut dict_builder).is_err());
    }

    #[test]
    fn test_datetime_validation() {
        let mut dict_builder = DictionaryBuilder::new();

        // DATETIME should reject strings without 'T'
        let invalid = Value::Datetime(Cow::Borrowed("2024-03-15"));
        let mut writer = Writer::new();
        assert!(encode_value(&mut writer, &invalid, &mut dict_builder).is_err());

        // DATETIME should reject strings without timezone
        let invalid_no_tz = Value::Datetime(Cow::Borrowed("2024-03-15T14:30:00"));
        let mut writer = Writer::new();
        assert!(encode_value(&mut writer, &invalid_no_tz, &mut dict_builder).is_err());
    }

    #[test]
    fn test_big_decimal_normalization_helpers() {
        // Test is_big_mantissa_zero
        assert!(is_big_mantissa_zero(&[]));
        assert!(is_big_mantissa_zero(&[0]));
        assert!(is_big_mantissa_zero(&[0, 0, 0]));
        assert!(!is_big_mantissa_zero(&[1]));
        assert!(!is_big_mantissa_zero(&[0, 1]));

        // Test is_big_mantissa_divisible_by_10 for positive numbers
        // 10 in big-endian = [0x0A]
        assert!(is_big_mantissa_divisible_by_10(&[0x0A])); // 10
        assert!(is_big_mantissa_divisible_by_10(&[0x14])); // 20
        assert!(is_big_mantissa_divisible_by_10(&[0x64])); // 100
        assert!(is_big_mantissa_divisible_by_10(&[0x01, 0xF4])); // 500

        assert!(!is_big_mantissa_divisible_by_10(&[0x01])); // 1
        assert!(!is_big_mantissa_divisible_by_10(&[0x07])); // 7
        assert!(!is_big_mantissa_divisible_by_10(&[0x0B])); // 11
        assert!(!is_big_mantissa_divisible_by_10(&[0x15])); // 21

        // Test negative numbers (two's complement)
        // -10 in two's complement (1 byte): 0xF6
        assert!(is_big_mantissa_divisible_by_10(&[0xF6])); // -10
        // -20 in two's complement (1 byte): 0xEC
        assert!(is_big_mantissa_divisible_by_10(&[0xEC])); // -20
        // -1 in two's complement (1 byte): 0xFF
        assert!(!is_big_mantissa_divisible_by_10(&[0xFF])); // -1
        // -7 in two's complement (1 byte): 0xF9
        assert!(!is_big_mantissa_divisible_by_10(&[0xF9])); // -7
    }

    #[test]
    fn test_big_decimal_normalization_encode() {
        // Valid: mantissa not divisible by 10
        let valid = Value::Decimal {
            exponent: 0,
            mantissa: DecimalMantissa::Big(Cow::Owned(vec![0x07])), // 7
            unit: None,
        };
        let mut dict_builder = DictionaryBuilder::new();
        let mut writer = Writer::new();
        assert!(encode_value(&mut writer, &valid, &mut dict_builder).is_ok());

        // Invalid: mantissa is 10 (divisible by 10)
        let invalid = Value::Decimal {
            exponent: 0,
            mantissa: DecimalMantissa::Big(Cow::Owned(vec![0x0A])), // 10
            unit: None,
        };
        let mut dict_builder = DictionaryBuilder::new();
        let mut writer = Writer::new();
        assert!(encode_value(&mut writer, &invalid, &mut dict_builder).is_err());

        // Invalid: zero mantissa with non-zero exponent
        let invalid_zero = Value::Decimal {
            exponent: 1,
            mantissa: DecimalMantissa::Big(Cow::Owned(vec![0x00])),
            unit: None,
        };
        let mut dict_builder = DictionaryBuilder::new();
        let mut writer = Writer::new();
        assert!(encode_value(&mut writer, &invalid_zero, &mut dict_builder).is_err());

        // Valid: zero mantissa with zero exponent
        let valid_zero = Value::Decimal {
            exponent: 0,
            mantissa: DecimalMantissa::Big(Cow::Owned(vec![0x00])),
            unit: None,
        };
        let mut dict_builder = DictionaryBuilder::new();
        let mut writer = Writer::new();
        assert!(encode_value(&mut writer, &valid_zero, &mut dict_builder).is_ok());
    }

    #[test]
    fn test_date_helpers() {
        // Test extracting date part from datetime
        assert_eq!(extract_date_part("2024-03-15T14:30:00Z"), "2024-03-15");
        assert_eq!(extract_date_part("2024-03-15T00:00:00.000Z"), "2024-03-15");
        assert_eq!(extract_date_part("2025-11-18T05:00:00.000Z"), "2025-11-18");
        assert_eq!(extract_date_part("2024-03-15"), "2024-03-15");
        assert_eq!(extract_date_part("2024-03"), "2024-03");
        assert_eq!(extract_date_part("2024"), "2024");

        // Test converting date to datetime
        assert_eq!(date_to_datetime("2024-03-15"), "2024-03-15T00:00:00.000Z");
        assert_eq!(date_to_datetime("2024-03"), "2024-03-01T00:00:00.000Z");
        assert_eq!(date_to_datetime("2024"), "2024-01-01T00:00:00.000Z");
        // Datetime should pass through unchanged
        assert_eq!(date_to_datetime("2024-03-15T14:30:00Z"), "2024-03-15T14:30:00Z");
    }
}

// =============================================================================
// DATE HELPERS
// =============================================================================

/// Extracts the date part from an ISO 8601 date or datetime string.
///
/// Examples:
/// - "2024-03-15T14:30:00Z" -> "2024-03-15"
/// - "2024-03-15" -> "2024-03-15"
/// - "2024-03" -> "2024-03"
/// - "2024" -> "2024"
pub fn extract_date_part(s: &str) -> &str {
    if let Some(t_pos) = s.find('T') {
        &s[..t_pos]
    } else {
        s
    }
}

/// Converts an ISO 8601 date string to a full datetime string at midnight UTC.
///
/// If the input already contains a time component, returns it unchanged.
///
/// Examples:
/// - "2024-03-15" -> "2024-03-15T00:00:00.000Z"
/// - "2024-03" -> "2024-03-01T00:00:00.000Z"
/// - "2024" -> "2024-01-01T00:00:00.000Z"
/// - "2024-03-15T14:30:00Z" -> "2024-03-15T14:30:00Z" (unchanged)
pub fn date_to_datetime(s: &str) -> String {
    // If already has time component, return as-is
    if s.contains('T') {
        return s.to_string();
    }

    // Handle BCE dates (with leading -)
    let (prefix, date_part) = if let Some(rest) = s.strip_prefix('-') {
        ("-", rest)
    } else {
        ("", s)
    };

    let parts: Vec<&str> = date_part.split('-').collect();

    match parts.len() {
        1 => {
            // Year only -> YYYY-01-01T00:00:00.000Z
            format!("{}{}-01-01T00:00:00.000Z", prefix, parts[0])
        }
        2 => {
            // Year-month -> YYYY-MM-01T00:00:00.000Z
            format!("{}{}-{}-01T00:00:00.000Z", prefix, parts[0], parts[1])
        }
        3 => {
            // Full date -> YYYY-MM-DDT00:00:00.000Z
            format!("{}{}-{}-{}T00:00:00.000Z", prefix, parts[0], parts[1], parts[2])
        }
        _ => s.to_string(), // Invalid, return as-is
    }
}
