/**
 * GRC-20 v2 TypeScript SDK
 *
 * Binary property graph format for decentralized knowledge networks.
 *
 * @packageDocumentation
 */

// Types
export * from "./types/index.js";

// Builders
export * from "./builder/index.js";

// Ops (functional API)
export * from "./ops/index.js";

// Utilities
export * from "./util/index.js";

// Genesis (well-known IDs)
export {
  genesisId,
  languageId,
  languages,
  properties,
  types,
  relationTypes,
} from "./genesis/index.js";

// Codec
export {
  encodeEdit,
  decodeEdit,
  type EncodeOptions,
  Writer,
  Reader,
  DecodeError,
  // Compression (lazy-loaded WASM)
  preloadCompression,
  isCompressionReady,
  encodeEditAuto,
  decodeEditAuto,
  type EncodeAutoOptions,
  encodeEditCompressed,
  decodeEditCompressed,
  isCompressed,
  compress,
  decompress,
} from "./codec/index.js";
