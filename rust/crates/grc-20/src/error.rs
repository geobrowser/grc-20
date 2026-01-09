//! Error types for GRC-20 encoding/decoding and validation.

use thiserror::Error;

use crate::model::{DataType, Id};

/// Error codes as defined in spec Section 8.3.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ErrorCode {
    /// E001: Invalid magic/version
    InvalidMagicOrVersion,
    /// E002: Index out of bounds
    IndexOutOfBounds,
    /// E003: Invalid signature
    InvalidSignature,
    /// E004: Invalid UTF-8 encoding
    InvalidUtf8,
    /// E005: Malformed varint/length/reserved bits/encoding
    MalformedEncoding,
}

impl ErrorCode {
    /// Returns the error code string (e.g., "E001").
    pub fn code(&self) -> &'static str {
        match self {
            ErrorCode::InvalidMagicOrVersion => "E001",
            ErrorCode::IndexOutOfBounds => "E002",
            ErrorCode::InvalidSignature => "E003",
            ErrorCode::InvalidUtf8 => "E004",
            ErrorCode::MalformedEncoding => "E005",
        }
    }
}

/// Error during binary decoding.
#[derive(Debug, Clone, PartialEq, Error)]
pub enum DecodeError {
    // === E001: Invalid magic/version ===
    #[error("[E001] invalid magic bytes: expected GRC2 or GRC2Z, found {found:?}")]
    InvalidMagic { found: [u8; 4] },

    #[error("[E001] unsupported version: {version}")]
    UnsupportedVersion { version: u8 },

    // === E002: Index out of bounds ===
    #[error("[E002] {dict} index {index} out of bounds (size: {size})")]
    IndexOutOfBounds {
        dict: &'static str,
        index: usize,
        size: usize,
    },

    // === E004: Invalid UTF-8 ===
    #[error("[E004] invalid UTF-8 in {field}")]
    InvalidUtf8 { field: &'static str },

    // === E005: Malformed encoding ===
    #[error("[E005] unexpected end of input while reading {context}")]
    UnexpectedEof { context: &'static str },

    #[error("[E005] varint exceeds maximum length (10 bytes)")]
    VarintTooLong,

    #[error("[E005] varint overflow (value exceeds u64)")]
    VarintOverflow,

    #[error("[E005] {field} length {len} exceeds maximum {max}")]
    LengthExceedsLimit {
        field: &'static str,
        len: usize,
        max: usize,
    },

    #[error("[E005] invalid op type: {op_type}")]
    InvalidOpType { op_type: u8 },

    #[error("[E005] invalid data type: {data_type}")]
    InvalidDataType { data_type: u8 },

    #[error("[E005] invalid embedding sub-type: {sub_type}")]
    InvalidEmbeddingSubType { sub_type: u8 },

    #[error("[E005] invalid bool value: {value} (expected 0x00 or 0x01)")]
    InvalidBool { value: u8 },

    #[error("[E005] reserved bits are non-zero in {context}")]
    ReservedBitsSet { context: &'static str },

    #[error("[E005] POINT latitude {lat} out of range [-90, +90]")]
    LatitudeOutOfRange { lat: f64 },

    #[error("[E005] POINT longitude {lon} out of range [-180, +180]")]
    LongitudeOutOfRange { lon: f64 },

    #[error("[E005] position string contains invalid character: {char:?}")]
    InvalidPositionChar { char: char },

    #[error("[E005] position string length {len} exceeds maximum 64")]
    PositionTooLong { len: usize },

    #[error("[E005] embedding data length {actual} doesn't match expected {expected} for {dims} dims")]
    EmbeddingDataMismatch {
        dims: usize,
        expected: usize,
        actual: usize,
    },

    #[error("[E005] DECIMAL has trailing zeros in mantissa (not normalized)")]
    DecimalNotNormalized,

    #[error("[E005] DECIMAL mantissa bytes are not minimal")]
    DecimalMantissaNotMinimal,

    #[error("[E005] float value is NaN")]
    FloatIsNan,

    #[error("[E005] malformed encoding: {context}")]
    MalformedEncoding { context: &'static str },

    // === Compression errors ===
    #[error("[E005] zstd decompression failed: {0}")]
    DecompressionFailed(String),

    #[error("[E005] decompressed size {actual} doesn't match declared {declared}")]
    UncompressedSizeMismatch { declared: usize, actual: usize },
}

impl DecodeError {
    /// Returns the error code for this error.
    pub fn code(&self) -> ErrorCode {
        match self {
            DecodeError::InvalidMagic { .. } | DecodeError::UnsupportedVersion { .. } => {
                ErrorCode::InvalidMagicOrVersion
            }
            DecodeError::IndexOutOfBounds { .. } => ErrorCode::IndexOutOfBounds,
            DecodeError::InvalidUtf8 { .. } => ErrorCode::InvalidUtf8,
            _ => ErrorCode::MalformedEncoding,
        }
    }
}

/// Error during binary encoding.
#[derive(Debug, Clone, PartialEq, Error)]
pub enum EncodeError {
    #[error("{field} length {len} exceeds maximum {max}")]
    LengthExceedsLimit {
        field: &'static str,
        len: usize,
        max: usize,
    },

    #[error("embedding data length {data_len} doesn't match {dims} dims for sub-type {sub_type:?}")]
    EmbeddingDimensionMismatch {
        sub_type: u8,
        dims: usize,
        data_len: usize,
    },

    #[error("zstd compression failed: {0}")]
    CompressionFailed(String),

    #[error("DECIMAL value is not normalized (has trailing zeros)")]
    DecimalNotNormalized,

    #[error("float value is NaN")]
    FloatIsNan,

    #[error("POINT latitude {lat} out of range [-90, +90]")]
    LatitudeOutOfRange { lat: f64 },

    #[error("POINT longitude {lon} out of range [-180, +180]")]
    LongitudeOutOfRange { lon: f64 },

    #[error("position string contains invalid character")]
    InvalidPositionChar,

    #[error("position string length exceeds maximum 64")]
    PositionTooLong,

    #[error("batch entity has {actual} values but schema requires {expected}")]
    BatchEntityValueCountMismatch { expected: usize, actual: usize },
}

/// Error during semantic validation.
#[derive(Debug, Clone, PartialEq, Error)]
pub enum ValidationError {
    #[error("value type mismatch for property {property:?}: expected {expected:?}")]
    TypeMismatch { property: Id, expected: DataType },

    #[error("entity {entity:?} is dead (tombstoned)")]
    EntityIsDead { entity: Id },

    #[error("relation {relation:?} is dead (tombstoned)")]
    RelationIsDead { relation: Id },

    #[error("property {property:?} not found in schema")]
    PropertyNotFound { property: Id },

    #[error("data type mismatch for property {property:?}: schema says {schema:?}, edit declares {declared:?}")]
    DataTypeInconsistent {
        property: Id,
        schema: DataType,
        declared: DataType,
    },
}
