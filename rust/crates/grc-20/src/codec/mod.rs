//! Binary encoding/decoding for GRC-20.
//!
//! This module implements the GRC-20 v2 binary format (spec Section 6).

pub mod edit;
pub mod op;
pub mod primitives;
pub mod value;

pub use edit::{
    decode_edit, decompress, encode_edit, encode_edit_compressed,
    encode_edit_compressed_with_options, encode_edit_profiled, encode_edit_with_options,
    EncodeOptions,
};
pub use primitives::{Reader, Writer, zigzag_decode, zigzag_encode};
pub use value::{decode_value, encode_value};
