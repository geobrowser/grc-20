//! Value types for GRC-20 properties.
//!
//! Values are typed attribute instances on entities and relations.

use std::borrow::Cow;

use crate::model::Id;

/// Data types for property values (spec Section 2.4).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum DataType {
    Bool = 1,
    Int64 = 2,
    Float64 = 3,
    Decimal = 4,
    Text = 5,
    Bytes = 6,
    Date = 7,
    Time = 8,
    Datetime = 9,
    Schedule = 10,
    Point = 11,
    Rect = 12,
    Embedding = 13,
}

impl DataType {
    /// Creates a DataType from its wire representation.
    pub fn from_u8(v: u8) -> Option<DataType> {
        match v {
            1 => Some(DataType::Bool),
            2 => Some(DataType::Int64),
            3 => Some(DataType::Float64),
            4 => Some(DataType::Decimal),
            5 => Some(DataType::Text),
            6 => Some(DataType::Bytes),
            7 => Some(DataType::Date),
            8 => Some(DataType::Time),
            9 => Some(DataType::Datetime),
            10 => Some(DataType::Schedule),
            11 => Some(DataType::Point),
            12 => Some(DataType::Rect),
            13 => Some(DataType::Embedding),
            _ => None,
        }
    }
}

/// Embedding sub-types (spec Section 2.4).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum EmbeddingSubType {
    /// 32-bit IEEE 754 float, little-endian (4 bytes per dim)
    Float32 = 0,
    /// Signed 8-bit integer (1 byte per dim)
    Int8 = 1,
    /// Bit-packed binary, LSB-first (1/8 byte per dim)
    Binary = 2,
}

impl EmbeddingSubType {
    /// Creates an EmbeddingSubType from its wire representation.
    pub fn from_u8(v: u8) -> Option<EmbeddingSubType> {
        match v {
            0 => Some(EmbeddingSubType::Float32),
            1 => Some(EmbeddingSubType::Int8),
            2 => Some(EmbeddingSubType::Binary),
            _ => None,
        }
    }

    /// Returns the number of bytes needed for the given number of dimensions.
    pub fn bytes_for_dims(self, dims: usize) -> usize {
        match self {
            EmbeddingSubType::Float32 => dims * 4,
            EmbeddingSubType::Int8 => dims,
            EmbeddingSubType::Binary => dims.div_ceil(8),
        }
    }
}

/// Decimal mantissa representation.
///
/// Most decimals fit in i64; larger values use big-endian two's complement bytes.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum DecimalMantissa<'a> {
    /// Mantissa fits in signed 64-bit integer.
    I64(i64),
    /// Arbitrary precision: big-endian two's complement, minimal-length.
    Big(Cow<'a, [u8]>),
}

impl DecimalMantissa<'_> {
    /// Returns whether this mantissa has trailing zeros (not normalized).
    pub fn has_trailing_zeros(&self) -> bool {
        match self {
            DecimalMantissa::I64(v) => *v != 0 && *v % 10 == 0,
            DecimalMantissa::Big(bytes) => {
                // For big mantissas, we'd need to convert to check
                // This is a simplification - full check would convert to decimal
                !bytes.is_empty() && bytes[bytes.len() - 1] == 0
            }
        }
    }

    /// Returns true if this is the zero mantissa.
    pub fn is_zero(&self) -> bool {
        match self {
            DecimalMantissa::I64(v) => *v == 0,
            DecimalMantissa::Big(bytes) => bytes.iter().all(|b| *b == 0),
        }
    }
}

/// A typed value that can be stored on an entity or relation.
#[derive(Debug, Clone, PartialEq)]
pub enum Value<'a> {
    /// Boolean value.
    Bool(bool),

    /// 64-bit signed integer with optional unit.
    Int64 {
        value: i64,
        /// Unit entity ID, or None for no unit.
        unit: Option<Id>,
    },

    /// 64-bit IEEE 754 float (NaN not allowed) with optional unit.
    Float64 {
        value: f64,
        /// Unit entity ID, or None for no unit.
        unit: Option<Id>,
    },

    /// Arbitrary-precision decimal: value = mantissa * 10^exponent, with optional unit.
    Decimal {
        exponent: i32,
        mantissa: DecimalMantissa<'a>,
        /// Unit entity ID, or None for no unit.
        unit: Option<Id>,
    },

    /// UTF-8 text with optional language.
    Text {
        value: Cow<'a, str>,
        /// Language entity ID, or None for default language.
        language: Option<Id>,
    },

    /// Opaque byte array.
    Bytes(Cow<'a, [u8]>),

    /// Calendar date (6 bytes: int32 days + int16 offset_min).
    Date {
        /// Signed days since Unix epoch (1970-01-01).
        days: i32,
        /// Signed UTC offset in minutes (e.g., +330 for +05:30).
        offset_min: i16,
    },

    /// Time of day (8 bytes: int48 time_us + int16 offset_min).
    Time {
        /// Microseconds since midnight (0 to 86,399,999,999).
        time_us: i64,
        /// Signed UTC offset in minutes (e.g., +330 for +05:30).
        offset_min: i16,
    },

    /// Combined date and time (10 bytes: int64 epoch_us + int16 offset_min).
    Datetime {
        /// Microseconds since Unix epoch (1970-01-01T00:00:00Z).
        epoch_us: i64,
        /// Signed UTC offset in minutes (e.g., +330 for +05:30).
        offset_min: i16,
    },

    /// RFC 5545 iCalendar schedule string.
    Schedule(Cow<'a, str>),

    /// WGS84 geographic coordinate with optional altitude.
    Point {
        /// Latitude in degrees (-90 to +90).
        lat: f64,
        /// Longitude in degrees (-180 to +180).
        lon: f64,
        /// Altitude in meters above WGS84 ellipsoid (optional).
        alt: Option<f64>,
    },

    /// Axis-aligned bounding box in WGS84 coordinates.
    Rect {
        /// Southern edge latitude (-90 to +90).
        min_lat: f64,
        /// Western edge longitude (-180 to +180).
        min_lon: f64,
        /// Northern edge latitude (-90 to +90).
        max_lat: f64,
        /// Eastern edge longitude (-180 to +180).
        max_lon: f64,
    },

    /// Dense vector for semantic similarity search.
    Embedding {
        sub_type: EmbeddingSubType,
        dims: usize,
        /// Raw bytes in the format specified by sub_type.
        data: Cow<'a, [u8]>,
    },
}

impl Value<'_> {
    /// Returns the data type of this value.
    pub fn data_type(&self) -> DataType {
        match self {
            Value::Bool(_) => DataType::Bool,
            Value::Int64 { .. } => DataType::Int64,
            Value::Float64 { .. } => DataType::Float64,
            Value::Decimal { .. } => DataType::Decimal,
            Value::Text { .. } => DataType::Text,
            Value::Bytes(_) => DataType::Bytes,
            Value::Date { .. } => DataType::Date,
            Value::Time { .. } => DataType::Time,
            Value::Datetime { .. } => DataType::Datetime,
            Value::Schedule(_) => DataType::Schedule,
            Value::Point { .. } => DataType::Point,
            Value::Rect { .. } => DataType::Rect,
            Value::Embedding { .. } => DataType::Embedding,
        }
    }

    /// Validates this value according to spec rules.
    ///
    /// Returns an error description if invalid, None if valid.
    pub fn validate(&self) -> Option<&'static str> {
        match self {
            Value::Float64 { value, .. } => {
                if value.is_nan() {
                    return Some("NaN is not allowed in Float64");
                }
            }
            Value::Decimal { exponent, mantissa, .. } => {
                // Zero must be {0, 0}
                if mantissa.is_zero() && *exponent != 0 {
                    return Some("zero DECIMAL must have exponent 0");
                }
                // Non-zero must not have trailing zeros
                if !mantissa.is_zero() && mantissa.has_trailing_zeros() {
                    return Some("DECIMAL mantissa has trailing zeros (not normalized)");
                }
            }
            Value::Point { lat, lon, alt } => {
                if *lat < -90.0 || *lat > 90.0 {
                    return Some("latitude out of range [-90, +90]");
                }
                if *lon < -180.0 || *lon > 180.0 {
                    return Some("longitude out of range [-180, +180]");
                }
                if lat.is_nan() || lon.is_nan() {
                    return Some("NaN is not allowed in Point coordinates");
                }
                if let Some(a) = alt {
                    if a.is_nan() {
                        return Some("NaN is not allowed in Point altitude");
                    }
                }
            }
            Value::Rect { min_lat, min_lon, max_lat, max_lon } => {
                if *min_lat < -90.0 || *min_lat > 90.0 || *max_lat < -90.0 || *max_lat > 90.0 {
                    return Some("latitude out of range [-90, +90]");
                }
                if *min_lon < -180.0 || *min_lon > 180.0 || *max_lon < -180.0 || *max_lon > 180.0 {
                    return Some("longitude out of range [-180, +180]");
                }
                if min_lat.is_nan() || min_lon.is_nan() || max_lat.is_nan() || max_lon.is_nan() {
                    return Some("NaN is not allowed in Rect coordinates");
                }
            }
            Value::Date { offset_min, .. } => {
                if *offset_min < -1440 || *offset_min > 1440 {
                    return Some("DATE offset_min outside range [-1440, +1440]");
                }
            }
            Value::Time { time_us, offset_min } => {
                if *time_us < 0 || *time_us > 86_399_999_999 {
                    return Some("TIME time_us outside range [0, 86399999999]");
                }
                if *offset_min < -1440 || *offset_min > 1440 {
                    return Some("TIME offset_min outside range [-1440, +1440]");
                }
            }
            Value::Datetime { offset_min, .. } => {
                if *offset_min < -1440 || *offset_min > 1440 {
                    return Some("DATETIME offset_min outside range [-1440, +1440]");
                }
            }
            Value::Embedding {
                sub_type,
                dims,
                data,
            } => {
                let expected = sub_type.bytes_for_dims(*dims);
                if data.len() != expected {
                    return Some("embedding data length doesn't match dims");
                }
                // Check for NaN in float32 embeddings
                if *sub_type == EmbeddingSubType::Float32 {
                    for chunk in data.chunks_exact(4) {
                        let f = f32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]);
                        if f.is_nan() {
                            return Some("NaN is not allowed in float32 embedding");
                        }
                    }
                }
            }
            _ => {}
        }
        None
    }
}

/// A property-value pair that can be attached to an object.
#[derive(Debug, Clone, PartialEq)]
pub struct PropertyValue<'a> {
    /// The property ID this value is for.
    pub property: Id,
    /// The value.
    pub value: Value<'a>,
}

/// A property definition in the schema.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Property {
    /// The property's unique identifier.
    pub id: Id,
    /// The data type for values of this property.
    pub data_type: DataType,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_embedding_bytes_for_dims() {
        assert_eq!(EmbeddingSubType::Float32.bytes_for_dims(10), 40);
        assert_eq!(EmbeddingSubType::Int8.bytes_for_dims(10), 10);
        assert_eq!(EmbeddingSubType::Binary.bytes_for_dims(10), 2);
        assert_eq!(EmbeddingSubType::Binary.bytes_for_dims(8), 1);
        assert_eq!(EmbeddingSubType::Binary.bytes_for_dims(9), 2);
    }

    #[test]
    fn test_value_validation_nan() {
        assert!(Value::Float64 { value: f64::NAN, unit: None }.validate().is_some());
        assert!(Value::Float64 { value: f64::INFINITY, unit: None }.validate().is_none());
        assert!(Value::Float64 { value: -f64::INFINITY, unit: None }.validate().is_none());
        assert!(Value::Float64 { value: 42.0, unit: None }.validate().is_none());
    }

    #[test]
    fn test_value_validation_point() {
        assert!(Value::Point { lat: 91.0, lon: 0.0, alt: None }.validate().is_some());
        assert!(Value::Point { lat: -91.0, lon: 0.0, alt: None }.validate().is_some());
        assert!(Value::Point { lat: 0.0, lon: 181.0, alt: None }.validate().is_some());
        assert!(Value::Point { lat: 0.0, lon: -181.0, alt: None }.validate().is_some());
        assert!(Value::Point { lat: 90.0, lon: 180.0, alt: None }.validate().is_none());
        assert!(Value::Point { lat: -90.0, lon: -180.0, alt: None }.validate().is_none());
        // With altitude
        assert!(Value::Point { lat: 0.0, lon: 0.0, alt: Some(1000.0) }.validate().is_none());
        assert!(Value::Point { lat: 0.0, lon: 0.0, alt: Some(f64::NAN) }.validate().is_some());
    }

    #[test]
    fn test_value_validation_rect() {
        // Invalid latitudes
        assert!(Value::Rect { min_lat: -91.0, min_lon: 0.0, max_lat: 0.0, max_lon: 0.0 }.validate().is_some());
        assert!(Value::Rect { min_lat: 0.0, min_lon: 0.0, max_lat: 91.0, max_lon: 0.0 }.validate().is_some());
        // Invalid longitudes
        assert!(Value::Rect { min_lat: 0.0, min_lon: -181.0, max_lat: 0.0, max_lon: 0.0 }.validate().is_some());
        assert!(Value::Rect { min_lat: 0.0, min_lon: 0.0, max_lat: 0.0, max_lon: 181.0 }.validate().is_some());
        // Valid rect
        assert!(Value::Rect { min_lat: 24.5, min_lon: -125.0, max_lat: 49.4, max_lon: -66.9 }.validate().is_none());
        // NaN not allowed
        assert!(Value::Rect { min_lat: f64::NAN, min_lon: 0.0, max_lat: 0.0, max_lon: 0.0 }.validate().is_some());
    }

    #[test]
    fn test_decimal_normalization() {
        // Zero must have exponent 0
        let zero_bad = Value::Decimal {
            exponent: 1,
            mantissa: DecimalMantissa::I64(0),
            unit: None,
        };
        assert!(zero_bad.validate().is_some());

        // Non-zero with trailing zeros is invalid
        let trailing = Value::Decimal {
            exponent: 0,
            mantissa: DecimalMantissa::I64(1230),
            unit: None,
        };
        assert!(trailing.validate().is_some());

        // Valid decimal
        let valid = Value::Decimal {
            exponent: -2,
            mantissa: DecimalMantissa::I64(1234),
            unit: None,
        };
        assert!(valid.validate().is_none());
    }
}
