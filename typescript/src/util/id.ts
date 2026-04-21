import { createId, type Id } from "../types/id.js";

// Re-export Id type for convenience
export type { Id } from "../types/id.js";

/**
 * Returns true if the bytes are the nil UUID or an RFC 4122/9562 UUID
 * with the standard variant and a known version nibble.
 */
export function isValidUuidBytes(bytes: Uint8Array): boolean {
  if (bytes.length !== 16) {
    return false;
  }

  let isNil = true;
  for (let i = 0; i < 16; i++) {
    if (bytes[i] !== 0) {
      isNil = false;
      break;
    }
  }
  if (isNil) {
    return true;
  }

  const version = (bytes[6] & 0xf0) >> 4;
  return (bytes[8] & 0xc0) === 0x80 && version >= 1 && version <= 8;
}

/**
 * Formats a UUID as non-hyphenated lowercase hex (recommended display format).
 */
export function formatId(id: Id): string {
  let s = "";
  for (let i = 0; i < 16; i++) {
    s += id[i].toString(16).padStart(2, "0");
  }
  return s;
}

/**
 * Parses a UUID from hex string (with or without hyphens).
 * Returns undefined if the string is invalid.
 */
export function parseId(s: string): Id | undefined {
  // Remove hyphens if present
  const hex = s.replace(/-/g, "").toLowerCase();
  if (hex.length !== 32) {
    return undefined;
  }

  const bytes = new Uint8Array(16);
  for (let i = 0; i < 16; i++) {
    const byte = parseInt(hex.slice(i * 2, i * 2 + 2), 16);
    if (Number.isNaN(byte)) {
      return undefined;
    }
    bytes[i] = byte;
  }
  return createId(bytes);
}

/**
 * Generates a random UUIDv4.
 */
export function randomId(): Id {
  const bytes = new Uint8Array(16);
  crypto.getRandomValues(bytes);

  // Set version 4 (0100 in bits 4-7 of byte 6)
  bytes[6] = (bytes[6] & 0x0f) | 0x40;
  // Set RFC 4122 variant (10 in bits 6-7 of byte 8)
  bytes[8] = (bytes[8] & 0x3f) | 0x80;

  return createId(bytes);
}

/**
 * SHA-256 hash function using Web Crypto API.
 * Works in both Node.js and browsers.
 */
async function sha256(data: Uint8Array): Promise<Uint8Array> {
  const hashBuffer = await crypto.subtle.digest("SHA-256", data);
  return new Uint8Array(hashBuffer);
}

/**
 * Synchronous SHA-256 implementation for environments where async is not ideal.
 * Uses a pure JavaScript implementation.
 */
function sha256Sync(data: Uint8Array): Uint8Array {
  // SHA-256 constants
  const K = new Uint32Array([
    0x428a2f98, 0x71374491, 0xb5c0fbcf, 0xe9b5dba5, 0x3956c25b, 0x59f111f1,
    0x923f82a4, 0xab1c5ed5, 0xd807aa98, 0x12835b01, 0x243185be, 0x550c7dc3,
    0x72be5d74, 0x80deb1fe, 0x9bdc06a7, 0xc19bf174, 0xe49b69c1, 0xefbe4786,
    0x0fc19dc6, 0x240ca1cc, 0x2de92c6f, 0x4a7484aa, 0x5cb0a9dc, 0x76f988da,
    0x983e5152, 0xa831c66d, 0xb00327c8, 0xbf597fc7, 0xc6e00bf3, 0xd5a79147,
    0x06ca6351, 0x14292967, 0x27b70a85, 0x2e1b2138, 0x4d2c6dfc, 0x53380d13,
    0x650a7354, 0x766a0abb, 0x81c2c92e, 0x92722c85, 0xa2bfe8a1, 0xa81a664b,
    0xc24b8b70, 0xc76c51a3, 0xd192e819, 0xd6990624, 0xf40e3585, 0x106aa070,
    0x19a4c116, 0x1e376c08, 0x2748774c, 0x34b0bcb5, 0x391c0cb3, 0x4ed8aa4a,
    0x5b9cca4f, 0x682e6ff3, 0x748f82ee, 0x78a5636f, 0x84c87814, 0x8cc70208,
    0x90befffa, 0xa4506ceb, 0xbef9a3f7, 0xc67178f2,
  ]);

  // Initial hash values
  let h0 = 0x6a09e667;
  let h1 = 0xbb67ae85;
  let h2 = 0x3c6ef372;
  let h3 = 0xa54ff53a;
  let h4 = 0x510e527f;
  let h5 = 0x9b05688c;
  let h6 = 0x1f83d9ab;
  let h7 = 0x5be0cd19;

  // Pre-processing: adding padding bits
  const bitLen = data.length * 8;
  const paddedLen = Math.ceil((data.length + 9) / 64) * 64;
  const padded = new Uint8Array(paddedLen);
  padded.set(data);
  padded[data.length] = 0x80;

  // Append original length in bits as 64-bit big-endian
  const view = new DataView(padded.buffer);
  view.setUint32(paddedLen - 4, bitLen, false);

  // Process each 64-byte chunk
  const W = new Uint32Array(64);
  for (let offset = 0; offset < paddedLen; offset += 64) {
    // Copy chunk into first 16 words
    for (let i = 0; i < 16; i++) {
      W[i] = view.getUint32(offset + i * 4, false);
    }

    // Extend the first 16 words into the remaining 48 words
    for (let i = 16; i < 64; i++) {
      const s0 =
        ((W[i - 15] >>> 7) | (W[i - 15] << 25)) ^
        ((W[i - 15] >>> 18) | (W[i - 15] << 14)) ^
        (W[i - 15] >>> 3);
      const s1 =
        ((W[i - 2] >>> 17) | (W[i - 2] << 15)) ^
        ((W[i - 2] >>> 19) | (W[i - 2] << 13)) ^
        (W[i - 2] >>> 10);
      W[i] = (W[i - 16] + s0 + W[i - 7] + s1) >>> 0;
    }

    // Initialize working variables
    let a = h0,
      b = h1,
      c = h2,
      d = h3,
      e = h4,
      f = h5,
      g = h6,
      h = h7;

    // Compression function main loop
    for (let i = 0; i < 64; i++) {
      const S1 =
        ((e >>> 6) | (e << 26)) ^ ((e >>> 11) | (e << 21)) ^ ((e >>> 25) | (e << 7));
      const ch = (e & f) ^ (~e & g);
      const temp1 = (h + S1 + ch + K[i] + W[i]) >>> 0;
      const S0 =
        ((a >>> 2) | (a << 30)) ^ ((a >>> 13) | (a << 19)) ^ ((a >>> 22) | (a << 10));
      const maj = (a & b) ^ (a & c) ^ (b & c);
      const temp2 = (S0 + maj) >>> 0;

      h = g;
      g = f;
      f = e;
      e = (d + temp1) >>> 0;
      d = c;
      c = b;
      b = a;
      a = (temp1 + temp2) >>> 0;
    }

    // Add the compressed chunk to the current hash value
    h0 = (h0 + a) >>> 0;
    h1 = (h1 + b) >>> 0;
    h2 = (h2 + c) >>> 0;
    h3 = (h3 + d) >>> 0;
    h4 = (h4 + e) >>> 0;
    h5 = (h5 + f) >>> 0;
    h6 = (h6 + g) >>> 0;
    h7 = (h7 + h) >>> 0;
  }

  // Produce the final hash value (big-endian)
  const result = new Uint8Array(32);
  const resultView = new DataView(result.buffer);
  resultView.setUint32(0, h0, false);
  resultView.setUint32(4, h1, false);
  resultView.setUint32(8, h2, false);
  resultView.setUint32(12, h3, false);
  resultView.setUint32(16, h4, false);
  resultView.setUint32(20, h5, false);
  resultView.setUint32(24, h6, false);
  resultView.setUint32(28, h7, false);

  return result;
}

/**
 * Derives a UUIDv8 from input bytes using SHA-256.
 *
 * This implements the `derived_uuid` function from spec Section 2.1:
 * ```
 * hash = SHA-256(input_bytes)[0:16]
 * hash[6] = (hash[6] & 0x0F) | 0x80  // version 8
 * hash[8] = (hash[8] & 0x3F) | 0x80  // RFC 4122 variant
 * ```
 */
export function derivedUuid(input: Uint8Array): Id {
  const hash = sha256Sync(input);
  const id = new Uint8Array(16);
  id.set(hash.subarray(0, 16));

  // Set version 8 (1000 in bits 4-7 of byte 6)
  id[6] = (id[6] & 0x0f) | 0x80;
  // Set RFC 4122 variant (10 in bits 6-7 of byte 8)
  id[8] = (id[8] & 0x3f) | 0x80;

  return createId(id);
}

/**
 * Async version of derivedUuid using Web Crypto API.
 */
export async function derivedUuidAsync(input: Uint8Array): Promise<Id> {
  const hash = await sha256(input);
  const id = new Uint8Array(16);
  id.set(hash.subarray(0, 16));

  // Set version 8 (1000 in bits 4-7 of byte 6)
  id[6] = (id[6] & 0x0f) | 0x80;
  // Set RFC 4122 variant (10 in bits 6-7 of byte 8)
  id[8] = (id[8] & 0x3f) | 0x80;

  return createId(id);
}

/**
 * Derives a UUIDv8 from a string using SHA-256.
 */
export function derivedUuidFromString(input: string): Id {
  return derivedUuid(new TextEncoder().encode(input));
}

/**
 * Derives a unique-mode relation ID.
 *
 * ```
 * id = derived_uuid(from_id || to_id || type_id)
 * ```
 */
export function uniqueRelationId(fromId: Id, toId: Id, typeId: Id): Id {
  const input = new Uint8Array(48);
  input.set(fromId, 0);
  input.set(toId, 16);
  input.set(typeId, 32);
  return derivedUuid(input);
}

const RELATION_ENTITY_PREFIX = new TextEncoder().encode("grc20:relation-entity:");

/**
 * Derives the reified entity ID from a relation ID.
 *
 * ```
 * entity_id = derived_uuid("grc20:relation-entity:" || relation_id)
 * ```
 */
export function relationEntityId(relationId: Id): Id {
  const input = new Uint8Array(RELATION_ENTITY_PREFIX.length + 16);
  input.set(RELATION_ENTITY_PREFIX, 0);
  input.set(relationId, RELATION_ENTITY_PREFIX.length);
  return derivedUuid(input);
}
