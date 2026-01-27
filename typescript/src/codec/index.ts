export {
  encodeEdit,
  decodeEdit,
  type EncodeOptions,
} from "./edit.js";

export {
  Writer,
  Reader,
  DecodeError,
  EncodeError,
  zigzagEncode,
  zigzagDecode,
} from "./primitives.js";

export {
  // Preloading
  preloadCompression,
  isCompressionReady,
  // Auto encode/decode (recommended)
  encodeEditAuto,
  decodeEditAuto,
  type EncodeAutoOptions,
  // Explicit compressed encode/decode
  encodeEditCompressed,
  decodeEditCompressed,
  // Utilities
  isCompressed,
  compress,
  decompress,
} from "./compression.js";
