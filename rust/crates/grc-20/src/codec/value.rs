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
        DataType::Rect => decode_rect(reader),
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
    // DATE: 6 bytes (int32 days + int16 offset_min), little-endian
    let bytes = reader.read_bytes(6, "date")?;
    let days = i32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]);
    let offset_min = i16::from_le_bytes([bytes[4], bytes[5]]);

    // Validate offset_min range
    if offset_min < -1440 || offset_min > 1440 {
        return Err(DecodeError::MalformedEncoding {
            context: "DATE offset_min outside range [-1440, +1440]",
        });
    }

    Ok(Value::Date { days, offset_min })
}

fn decode_time<'a>(reader: &mut Reader<'a>) -> Result<Value<'a>, DecodeError> {
    // TIME: 8 bytes (int48 time_us + int16 offset_min), little-endian
    let bytes = reader.read_bytes(8, "time")?;

    // Read int48 as 6 bytes, sign-extend to i64
    let time_us_unsigned = u64::from_le_bytes([
        bytes[0], bytes[1], bytes[2], bytes[3], bytes[4], bytes[5], 0, 0
    ]);
    // Sign-extend from 48 bits
    let time_us = if time_us_unsigned & 0x8000_0000_0000 != 0 {
        (time_us_unsigned | 0xFFFF_0000_0000_0000) as i64
    } else {
        time_us_unsigned as i64
    };

    let offset_min = i16::from_le_bytes([bytes[6], bytes[7]]);

    // Validate time_us range
    if time_us < 0 || time_us > 86_399_999_999 {
        return Err(DecodeError::MalformedEncoding {
            context: "TIME time_us outside range [0, 86399999999]",
        });
    }

    // Validate offset_min range
    if offset_min < -1440 || offset_min > 1440 {
        return Err(DecodeError::MalformedEncoding {
            context: "TIME offset_min outside range [-1440, +1440]",
        });
    }

    Ok(Value::Time { time_us, offset_min })
}

fn decode_datetime<'a>(reader: &mut Reader<'a>) -> Result<Value<'a>, DecodeError> {
    // DATETIME: 10 bytes (int64 epoch_us + int16 offset_min), little-endian
    let bytes = reader.read_bytes(10, "datetime")?;
    let epoch_us = i64::from_le_bytes([
        bytes[0], bytes[1], bytes[2], bytes[3],
        bytes[4], bytes[5], bytes[6], bytes[7]
    ]);
    let offset_min = i16::from_le_bytes([bytes[8], bytes[9]]);

    // Validate offset_min range
    if offset_min < -1440 || offset_min > 1440 {
        return Err(DecodeError::MalformedEncoding {
            context: "DATETIME offset_min outside range [-1440, +1440]",
        });
    }

    Ok(Value::Datetime { epoch_us, offset_min })
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

    // Read in wire order: latitude, longitude, altitude (optional)
    let lat = reader.read_f64("point.lat")?;
    let lon = reader.read_f64("point.lon")?;
    let alt = if ordinate_count == 3 {
        Some(reader.read_f64("point.alt")?)
    } else {
        None
    };

    // Validate bounds
    if !(-90.0..=90.0).contains(&lat) {
        return Err(DecodeError::LatitudeOutOfRange { lat });
    }
    if !(-180.0..=180.0).contains(&lon) {
        return Err(DecodeError::LongitudeOutOfRange { lon });
    }
    if lat.is_nan() || lon.is_nan() {
        return Err(DecodeError::FloatIsNan);
    }
    if let Some(a) = alt {
        if a.is_nan() {
            return Err(DecodeError::FloatIsNan);
        }
    }

    Ok(Value::Point { lat, lon, alt })
}

fn decode_rect<'a>(reader: &mut Reader<'a>) -> Result<Value<'a>, DecodeError> {
    // RECT: 32 bytes (4 x float64), little-endian
    // Wire order: min_lat, min_lon, max_lat, max_lon
    let min_lat = reader.read_f64("rect.min_lat")?;
    let min_lon = reader.read_f64("rect.min_lon")?;
    let max_lat = reader.read_f64("rect.max_lat")?;
    let max_lon = reader.read_f64("rect.max_lon")?;

    // Validate bounds
    if !(-90.0..=90.0).contains(&min_lat) || !(-90.0..=90.0).contains(&max_lat) {
        return Err(DecodeError::LatitudeOutOfRange { lat: if !(-90.0..=90.0).contains(&min_lat) { min_lat } else { max_lat } });
    }
    if !(-180.0..=180.0).contains(&min_lon) || !(-180.0..=180.0).contains(&max_lon) {
        return Err(DecodeError::LongitudeOutOfRange { lon: if !(-180.0..=180.0).contains(&min_lon) { min_lon } else { max_lon } });
    }
    if min_lat.is_nan() || min_lon.is_nan() || max_lat.is_nan() || max_lon.is_nan() {
        return Err(DecodeError::FloatIsNan);
    }

    Ok(Value::Rect { min_lat, min_lon, max_lat, max_lon })
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
        Value::Date { days, offset_min } => {
            // Validate offset_min range
            if *offset_min < -1440 || *offset_min > 1440 {
                return Err(EncodeError::InvalidInput {
                    context: "DATE offset_min outside range [-1440, +1440]",
                });
            }
            // DATE: 6 bytes (int32 days + int16 offset_min), little-endian
            writer.write_bytes(&days.to_le_bytes());
            writer.write_bytes(&offset_min.to_le_bytes());
        }
        Value::Time { time_us, offset_min } => {
            // Validate time_us range
            if *time_us < 0 || *time_us > 86_399_999_999 {
                return Err(EncodeError::InvalidInput {
                    context: "TIME time_us outside range [0, 86399999999]",
                });
            }
            // Validate offset_min range
            if *offset_min < -1440 || *offset_min > 1440 {
                return Err(EncodeError::InvalidInput {
                    context: "TIME offset_min outside range [-1440, +1440]",
                });
            }
            // TIME: 8 bytes (int48 time_us + int16 offset_min), little-endian
            // Write int48 as 6 bytes
            let time_bytes = time_us.to_le_bytes();
            writer.write_bytes(&time_bytes[0..6]);
            writer.write_bytes(&offset_min.to_le_bytes());
        }
        Value::Datetime { epoch_us, offset_min } => {
            // Validate offset_min range
            if *offset_min < -1440 || *offset_min > 1440 {
                return Err(EncodeError::InvalidInput {
                    context: "DATETIME offset_min outside range [-1440, +1440]",
                });
            }
            // DATETIME: 10 bytes (int64 epoch_us + int16 offset_min), little-endian
            writer.write_bytes(&epoch_us.to_le_bytes());
            writer.write_bytes(&offset_min.to_le_bytes());
        }
        Value::Schedule(s) => {
            // RFC 5545 iCalendar format
            writer.write_string(s);
        }
        Value::Point { lat, lon, alt } => {
            if *lat < -90.0 || *lat > 90.0 {
                return Err(EncodeError::LatitudeOutOfRange { lat: *lat });
            }
            if *lon < -180.0 || *lon > 180.0 {
                return Err(EncodeError::LongitudeOutOfRange { lon: *lon });
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
            // Write in wire order: latitude, longitude, altitude (optional)
            writer.write_f64(*lat);
            writer.write_f64(*lon);
            if let Some(a) = alt {
                writer.write_f64(*a);
            }
        }
        Value::Rect { min_lat, min_lon, max_lat, max_lon } => {
            if *min_lat < -90.0 || *min_lat > 90.0 || *max_lat < -90.0 || *max_lat > 90.0 {
                return Err(EncodeError::LatitudeOutOfRange { lat: if *min_lat < -90.0 || *min_lat > 90.0 { *min_lat } else { *max_lat } });
            }
            if *min_lon < -180.0 || *min_lon > 180.0 || *max_lon < -180.0 || *max_lon > 180.0 {
                return Err(EncodeError::LongitudeOutOfRange { lon: if *min_lon < -180.0 || *min_lon > 180.0 { *min_lon } else { *max_lon } });
            }
            if min_lat.is_nan() || min_lon.is_nan() || max_lat.is_nan() || max_lon.is_nan() {
                return Err(EncodeError::FloatIsNan);
            }
            // RECT: 32 bytes (4 x float64), little-endian
            // Wire order: min_lat, min_lon, max_lat, max_lon
            writer.write_f64(*min_lat);
            writer.write_f64(*min_lon);
            writer.write_f64(*max_lat);
            writer.write_f64(*max_lon);
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
        let value = Value::Point { lat: 37.7749, lon: -122.4194, alt: None };
        let dicts = WireDictionaries::default();
        let mut dict_builder = DictionaryBuilder::new();

        let mut writer = Writer::new();
        encode_value(&mut writer, &value, &mut dict_builder).unwrap();

        let mut reader = Reader::new(writer.as_bytes());
        let decoded = decode_value(&mut reader, DataType::Point, &dicts).unwrap();

        assert_eq!(value, decoded);

        // 3D point (with altitude)
        let value_3d = Value::Point { lat: 37.7749, lon: -122.4194, alt: Some(100.0) };
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
        let value = Value::Point { lat: 91.0, lon: 0.0, alt: None };
        let mut dict_builder = DictionaryBuilder::new();
        let mut writer = Writer::new();
        let result = encode_value(&mut writer, &value, &mut dict_builder);
        assert!(result.is_err());

        // Longitude out of range
        let value = Value::Point { lat: 0.0, lon: 181.0, alt: None };
        let mut dict_builder = DictionaryBuilder::new();
        let mut writer = Writer::new();
        let result = encode_value(&mut writer, &value, &mut dict_builder);
        assert!(result.is_err());

        // NaN in altitude
        let value = Value::Point { lat: 0.0, lon: 0.0, alt: Some(f64::NAN) };
        let mut dict_builder = DictionaryBuilder::new();
        let mut writer = Writer::new();
        let result = encode_value(&mut writer, &value, &mut dict_builder);
        assert!(result.is_err());
    }

    #[test]
    fn test_rect_roundtrip() {
        let value = Value::Rect {
            min_lat: 24.5,
            min_lon: -125.0,
            max_lat: 49.4,
            max_lon: -66.9,
        };
        let dicts = WireDictionaries::default();
        let mut dict_builder = DictionaryBuilder::new();

        let mut writer = Writer::new();
        encode_value(&mut writer, &value, &mut dict_builder).unwrap();

        let mut reader = Reader::new(writer.as_bytes());
        let decoded = decode_value(&mut reader, DataType::Rect, &dicts).unwrap();

        assert_eq!(value, decoded);
    }

    #[test]
    fn test_rect_validation() {
        // Latitude out of range
        let value = Value::Rect { min_lat: -91.0, min_lon: 0.0, max_lat: 0.0, max_lon: 0.0 };
        let mut dict_builder = DictionaryBuilder::new();
        let mut writer = Writer::new();
        let result = encode_value(&mut writer, &value, &mut dict_builder);
        assert!(result.is_err());

        let value = Value::Rect { min_lat: 0.0, min_lon: 0.0, max_lat: 91.0, max_lon: 0.0 };
        let mut dict_builder = DictionaryBuilder::new();
        let mut writer = Writer::new();
        let result = encode_value(&mut writer, &value, &mut dict_builder);
        assert!(result.is_err());

        // Longitude out of range
        let value = Value::Rect { min_lat: 0.0, min_lon: -181.0, max_lat: 0.0, max_lon: 0.0 };
        let mut dict_builder = DictionaryBuilder::new();
        let mut writer = Writer::new();
        let result = encode_value(&mut writer, &value, &mut dict_builder);
        assert!(result.is_err());

        let value = Value::Rect { min_lat: 0.0, min_lon: 0.0, max_lat: 0.0, max_lon: 181.0 };
        let mut dict_builder = DictionaryBuilder::new();
        let mut writer = Writer::new();
        let result = encode_value(&mut writer, &value, &mut dict_builder);
        assert!(result.is_err());

        // NaN not allowed
        let value = Value::Rect { min_lat: f64::NAN, min_lon: 0.0, max_lat: 0.0, max_lon: 0.0 };
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

        // Test various date values
        let test_cases = [
            (0, 0),           // Unix epoch, UTC
            (19797, 0),       // March 15, 2024 UTC
            (19797, 330),     // March 15, 2024 +05:30
            (-36524, 0),      // 100 BCE
            (i32::MAX, 0),    // Far future
            (i32::MIN, 0),    // Far past
        ];

        for (days, offset_min) in test_cases {
            let value = Value::Date { days, offset_min };

            let mut writer = Writer::new();
            encode_value(&mut writer, &value, &mut dict_builder).unwrap();

            let mut reader = Reader::new(writer.as_bytes());
            let decoded = decode_value(&mut reader, DataType::Date, &dicts).unwrap();

            assert_eq!(value, decoded);
        }
    }

    #[test]
    fn test_time_roundtrip() {
        let dicts = WireDictionaries::default();
        let mut dict_builder = DictionaryBuilder::new();

        // Test various time values
        let test_cases = [
            (0, 0),                      // Midnight UTC
            (52_200_000_000, 0),         // 14:30:00 UTC
            (52_200_500_000, 330),       // 14:30:00.500 +05:30
            (86_399_999_999, 0),         // 23:59:59.999999 UTC
            (0, -300),                   // Midnight -05:00
        ];

        for (time_us, offset_min) in test_cases {
            let value = Value::Time { time_us, offset_min };

            let mut writer = Writer::new();
            encode_value(&mut writer, &value, &mut dict_builder).unwrap();

            let mut reader = Reader::new(writer.as_bytes());
            let decoded = decode_value(&mut reader, DataType::Time, &dicts).unwrap();

            assert_eq!(value, decoded);
        }
    }

    #[test]
    fn test_datetime_roundtrip() {
        let dicts = WireDictionaries::default();
        let mut dict_builder = DictionaryBuilder::new();

        // Test various datetime values
        let test_cases = [
            (0, 0),                          // Unix epoch UTC
            (1_710_513_000_000_000, 0),      // 2024-03-15T14:30:00Z
            (1_710_493_200_000_000, 330),    // 2024-03-15T14:30:00+05:30
            (-1_000_000_000_000, 0),         // Before epoch
            (i64::MAX / 2, 0),               // Far future (within safe range)
        ];

        for (epoch_us, offset_min) in test_cases {
            let value = Value::Datetime { epoch_us, offset_min };

            let mut writer = Writer::new();
            encode_value(&mut writer, &value, &mut dict_builder).unwrap();

            let mut reader = Reader::new(writer.as_bytes());
            let decoded = decode_value(&mut reader, DataType::Datetime, &dicts).unwrap();

            assert_eq!(value, decoded);
        }
    }

    #[test]
    fn test_date_validation() {
        let mut dict_builder = DictionaryBuilder::new();

        // DATE should reject offset_min outside range
        let invalid = Value::Date { days: 0, offset_min: 1500 };
        let mut writer = Writer::new();
        assert!(encode_value(&mut writer, &invalid, &mut dict_builder).is_err());

        let invalid_neg = Value::Date { days: 0, offset_min: -1500 };
        let mut writer = Writer::new();
        assert!(encode_value(&mut writer, &invalid_neg, &mut dict_builder).is_err());
    }

    #[test]
    fn test_time_validation() {
        let mut dict_builder = DictionaryBuilder::new();

        // TIME should reject time_us outside range
        let invalid_high = Value::Time { time_us: 86_400_000_000, offset_min: 0 };
        let mut writer = Writer::new();
        assert!(encode_value(&mut writer, &invalid_high, &mut dict_builder).is_err());

        let invalid_neg = Value::Time { time_us: -1, offset_min: 0 };
        let mut writer = Writer::new();
        assert!(encode_value(&mut writer, &invalid_neg, &mut dict_builder).is_err());

        // TIME should reject offset_min outside range
        let invalid_offset = Value::Time { time_us: 0, offset_min: 1500 };
        let mut writer = Writer::new();
        assert!(encode_value(&mut writer, &invalid_offset, &mut dict_builder).is_err());
    }

    #[test]
    fn test_datetime_validation() {
        let mut dict_builder = DictionaryBuilder::new();

        // DATETIME should reject offset_min outside range
        let invalid = Value::Datetime { epoch_us: 0, offset_min: 1500 };
        let mut writer = Writer::new();
        assert!(encode_value(&mut writer, &invalid, &mut dict_builder).is_err());

        let invalid_neg = Value::Datetime { epoch_us: 0, offset_min: -1500 };
        let mut writer = Writer::new();
        assert!(encode_value(&mut writer, &invalid_neg, &mut dict_builder).is_err());
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

}
