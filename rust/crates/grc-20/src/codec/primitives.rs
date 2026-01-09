//! Primitive encoding/decoding for GRC-20 binary format.
//!
//! Implements varint, signed varint (zigzag), and basic types.

use crate::error::DecodeError;
use crate::limits::MAX_VARINT_BYTES;
use crate::model::Id;

// =============================================================================
// DECODING
// =============================================================================

/// Reader for decoding binary data.
///
/// Wraps a byte slice and provides methods for reading primitives
/// with bounds checking and error handling.
#[derive(Debug, Clone)]
pub struct Reader<'a> {
    data: &'a [u8],
    pos: usize,
}

impl<'a> Reader<'a> {
    /// Creates a new reader from a byte slice.
    pub fn new(data: &'a [u8]) -> Self {
        Self { data, pos: 0 }
    }

    /// Returns the current position in the data.
    pub fn position(&self) -> usize {
        self.pos
    }

    /// Returns the remaining bytes.
    pub fn remaining(&self) -> &'a [u8] {
        &self.data[self.pos..]
    }

    /// Returns the number of remaining bytes.
    pub fn remaining_len(&self) -> usize {
        self.data.len() - self.pos
    }

    /// Returns true if all data has been consumed.
    pub fn is_empty(&self) -> bool {
        self.pos >= self.data.len()
    }

    /// Reads a single byte.
    #[inline]
    pub fn read_byte(&mut self, context: &'static str) -> Result<u8, DecodeError> {
        if self.pos >= self.data.len() {
            return Err(DecodeError::UnexpectedEof { context });
        }
        let byte = self.data[self.pos];
        self.pos += 1;
        Ok(byte)
    }

    /// Reads exactly n bytes.
    #[inline]
    pub fn read_bytes(&mut self, n: usize, context: &'static str) -> Result<&'a [u8], DecodeError> {
        if self.pos + n > self.data.len() {
            return Err(DecodeError::UnexpectedEof { context });
        }
        let bytes = &self.data[self.pos..self.pos + n];
        self.pos += n;
        Ok(bytes)
    }

    /// Reads a 16-byte UUID.
    #[inline]
    pub fn read_id(&mut self, context: &'static str) -> Result<Id, DecodeError> {
        let bytes = self.read_bytes(16, context)?;
        // SAFETY: read_bytes guarantees exactly 16 bytes, try_into always succeeds
        Ok(bytes.try_into().unwrap())
    }

    /// Reads an unsigned varint (LEB128).
    #[inline]
    pub fn read_varint(&mut self, context: &'static str) -> Result<u64, DecodeError> {
        let mut result: u64 = 0;
        let mut shift = 0;

        for i in 0..MAX_VARINT_BYTES {
            let byte = self.read_byte(context)?;
            let value = (byte & 0x7F) as u64;

            // Check for overflow
            if shift >= 64 || (shift == 63 && value > 1) {
                return Err(DecodeError::VarintOverflow);
            }

            result |= value << shift;

            if byte & 0x80 == 0 {
                return Ok(result);
            }
            shift += 7;

            if i == MAX_VARINT_BYTES - 1 {
                return Err(DecodeError::VarintTooLong);
            }
        }

        Err(DecodeError::VarintTooLong)
    }

    /// Reads a signed varint (zigzag encoded).
    pub fn read_signed_varint(&mut self, context: &'static str) -> Result<i64, DecodeError> {
        let unsigned = self.read_varint(context)?;
        Ok(zigzag_decode(unsigned))
    }

    /// Reads a length-prefixed UTF-8 string.
    #[inline]
    pub fn read_string(
        &mut self,
        max_len: usize,
        field: &'static str,
    ) -> Result<String, DecodeError> {
        let len = self.read_varint(field)? as usize;
        if len > max_len {
            return Err(DecodeError::LengthExceedsLimit {
                field,
                len,
                max: max_len,
            });
        }
        let bytes = self.read_bytes(len, field)?;
        // Validate UTF-8 on borrowed slice, then allocate once (avoids intermediate Vec)
        std::str::from_utf8(bytes)
            .map(|s| s.to_string())
            .map_err(|_| DecodeError::InvalidUtf8 { field })
    }

    /// Reads a length-prefixed byte array.
    pub fn read_bytes_prefixed(
        &mut self,
        max_len: usize,
        field: &'static str,
    ) -> Result<Vec<u8>, DecodeError> {
        let len = self.read_varint(field)? as usize;
        if len > max_len {
            return Err(DecodeError::LengthExceedsLimit {
                field,
                len,
                max: max_len,
            });
        }
        let bytes = self.read_bytes(len, field)?;
        Ok(bytes.to_vec())
    }

    /// Reads a little-endian f64.
    #[inline]
    pub fn read_f64(&mut self, context: &'static str) -> Result<f64, DecodeError> {
        let bytes = self.read_bytes(8, context)?;
        // SAFETY: read_bytes guarantees exactly 8 bytes, try_into always succeeds
        let value = f64::from_le_bytes(bytes.try_into().unwrap());
        if value.is_nan() {
            return Err(DecodeError::FloatIsNan);
        }
        Ok(value)
    }

    /// Reads a little-endian f64 without NaN check.
    #[inline]
    pub fn read_f64_unchecked(&mut self, context: &'static str) -> Result<f64, DecodeError> {
        let bytes = self.read_bytes(8, context)?;
        // SAFETY: read_bytes guarantees exactly 8 bytes, try_into always succeeds
        Ok(f64::from_le_bytes(bytes.try_into().unwrap()))
    }

    /// Reads a vector of IDs with length prefix.
    pub fn read_id_vec(
        &mut self,
        max_len: usize,
        field: &'static str,
    ) -> Result<Vec<Id>, DecodeError> {
        let count = self.read_varint(field)? as usize;
        if count > max_len {
            return Err(DecodeError::LengthExceedsLimit {
                field,
                len: count,
                max: max_len,
            });
        }
        let mut ids = Vec::with_capacity(count);
        for _ in 0..count {
            ids.push(self.read_id(field)?);
        }
        Ok(ids)
    }
}

// =============================================================================
// ENCODING
// =============================================================================

/// Writer for encoding binary data.
#[derive(Debug, Clone, Default)]
pub struct Writer {
    buf: Vec<u8>,
}

impl Writer {
    /// Creates a new writer.
    pub fn new() -> Self {
        Self { buf: Vec::new() }
    }

    /// Creates a new writer with capacity.
    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            buf: Vec::with_capacity(capacity),
        }
    }

    /// Returns the written bytes.
    pub fn into_bytes(self) -> Vec<u8> {
        self.buf
    }

    /// Returns a reference to the written bytes.
    pub fn as_bytes(&self) -> &[u8] {
        &self.buf
    }

    /// Returns the number of bytes written.
    pub fn len(&self) -> usize {
        self.buf.len()
    }

    /// Returns true if no bytes have been written.
    pub fn is_empty(&self) -> bool {
        self.buf.is_empty()
    }

    /// Writes a single byte.
    #[inline]
    pub fn write_byte(&mut self, byte: u8) {
        self.buf.push(byte);
    }

    /// Writes raw bytes.
    #[inline]
    pub fn write_bytes(&mut self, bytes: &[u8]) {
        self.buf.extend_from_slice(bytes);
    }

    /// Writes a 16-byte UUID.
    #[inline]
    pub fn write_id(&mut self, id: &Id) {
        self.buf.extend_from_slice(id);
    }

    /// Writes an unsigned varint (LEB128).
    #[inline]
    pub fn write_varint(&mut self, mut value: u64) {
        // Use stack buffer to batch writes (faster than multiple push calls)
        let mut buf = [0u8; 10]; // Max 10 bytes for 64-bit varint
        let mut len = 0;
        loop {
            let mut byte = (value & 0x7F) as u8;
            value >>= 7;
            if value != 0 {
                byte |= 0x80;
            }
            buf[len] = byte;
            len += 1;
            if value == 0 {
                break;
            }
        }
        self.buf.extend_from_slice(&buf[..len]);
    }

    /// Writes a signed varint (zigzag encoded).
    pub fn write_signed_varint(&mut self, value: i64) {
        self.write_varint(zigzag_encode(value));
    }

    /// Writes a length-prefixed UTF-8 string.
    pub fn write_string(&mut self, s: &str) {
        self.write_varint(s.len() as u64);
        self.buf.extend_from_slice(s.as_bytes());
    }

    /// Writes a length-prefixed byte array.
    pub fn write_bytes_prefixed(&mut self, bytes: &[u8]) {
        self.write_varint(bytes.len() as u64);
        self.buf.extend_from_slice(bytes);
    }

    /// Writes a little-endian f64.
    pub fn write_f64(&mut self, value: f64) {
        self.buf.extend_from_slice(&value.to_le_bytes());
    }

    /// Writes a vector of IDs with length prefix.
    pub fn write_id_vec(&mut self, ids: &[Id]) {
        self.write_varint(ids.len() as u64);
        for id in ids {
            self.write_id(id);
        }
    }
}

// =============================================================================
// ZIGZAG ENCODING
// =============================================================================

/// Encodes a signed integer using zigzag encoding.
///
/// Maps negative numbers to odd positive numbers:
/// 0 -> 0, -1 -> 1, 1 -> 2, -2 -> 3, 2 -> 4, ...
#[inline]
pub fn zigzag_encode(n: i64) -> u64 {
    ((n << 1) ^ (n >> 63)) as u64
}

/// Decodes a zigzag-encoded unsigned integer back to signed.
#[inline]
pub fn zigzag_decode(n: u64) -> i64 {
    ((n >> 1) as i64) ^ (-((n & 1) as i64))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_zigzag_roundtrip() {
        for v in [0i64, 1, -1, 127, -128, i64::MAX, i64::MIN] {
            assert_eq!(zigzag_decode(zigzag_encode(v)), v);
        }
    }

    #[test]
    fn test_zigzag_values() {
        assert_eq!(zigzag_encode(0), 0);
        assert_eq!(zigzag_encode(-1), 1);
        assert_eq!(zigzag_encode(1), 2);
        assert_eq!(zigzag_encode(-2), 3);
        assert_eq!(zigzag_encode(2), 4);
    }

    #[test]
    fn test_varint_roundtrip() {
        let test_values = [0u64, 1, 127, 128, 255, 256, 16383, 16384, u64::MAX];

        for v in test_values {
            let mut writer = Writer::new();
            writer.write_varint(v);

            let mut reader = Reader::new(writer.as_bytes());
            let decoded = reader.read_varint("test").unwrap();
            assert_eq!(v, decoded, "failed for {}", v);
        }
    }

    #[test]
    fn test_signed_varint_roundtrip() {
        let test_values = [0i64, 1, -1, 127, -128, i64::MAX, i64::MIN];

        for v in test_values {
            let mut writer = Writer::new();
            writer.write_signed_varint(v);

            let mut reader = Reader::new(writer.as_bytes());
            let decoded = reader.read_signed_varint("test").unwrap();
            assert_eq!(v, decoded, "failed for {}", v);
        }
    }

    #[test]
    fn test_string_roundtrip() {
        let test_strings = ["", "hello", "hello world", "unicode: \u{1F600}"];

        for s in test_strings {
            let mut writer = Writer::new();
            writer.write_string(s);

            let mut reader = Reader::new(writer.as_bytes());
            let decoded = reader.read_string(1000, "test").unwrap();
            assert_eq!(s, decoded);
        }
    }

    #[test]
    fn test_id_roundtrip() {
        let id = [1u8, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16];

        let mut writer = Writer::new();
        writer.write_id(&id);

        let mut reader = Reader::new(writer.as_bytes());
        let decoded = reader.read_id("test").unwrap();
        assert_eq!(id, decoded);
    }

    #[test]
    fn test_f64_roundtrip() {
        let test_values = [0.0, 1.0, -1.0, f64::INFINITY, f64::NEG_INFINITY, 3.14159];

        for v in test_values {
            let mut writer = Writer::new();
            writer.write_f64(v);

            let mut reader = Reader::new(writer.as_bytes());
            let decoded = reader.read_f64("test").unwrap();
            assert_eq!(v, decoded, "failed for {}", v);
        }
    }

    #[test]
    fn test_f64_nan_rejected() {
        let mut writer = Writer::new();
        writer.write_f64(f64::NAN);

        let mut reader = Reader::new(writer.as_bytes());
        let result = reader.read_f64("test");
        assert!(matches!(result, Err(DecodeError::FloatIsNan)));
    }

    #[test]
    fn test_varint_too_long() {
        // 11 continuation bytes should fail
        let data = [0x80u8; 11];
        let mut reader = Reader::new(&data);
        let result = reader.read_varint("test");
        assert!(matches!(result, Err(DecodeError::VarintTooLong)));
    }

    #[test]
    fn test_string_too_long() {
        let mut writer = Writer::new();
        writer.write_varint(1000); // length
        writer.write_bytes(&[0u8; 1000]);

        let mut reader = Reader::new(writer.as_bytes());
        let result = reader.read_string(100, "test"); // max 100
        assert!(matches!(
            result,
            Err(DecodeError::LengthExceedsLimit { max: 100, .. })
        ));
    }

    #[test]
    fn test_unexpected_eof() {
        let data = [0u8; 5];
        let mut reader = Reader::new(&data);
        let result = reader.read_bytes(10, "test");
        assert!(matches!(result, Err(DecodeError::UnexpectedEof { .. })));
    }
}
