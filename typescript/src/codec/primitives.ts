import { createId, type Id } from "../types/id.js";

/**
 * ZigZag encodes a signed integer to an unsigned integer.
 */
export function zigzagEncode(n: bigint): bigint {
  return (n << 1n) ^ (n >> 63n);
}

/**
 * ZigZag decodes an unsigned integer to a signed integer.
 */
export function zigzagDecode(n: bigint): bigint {
  return (n >> 1n) ^ -(n & 1n);
}

/**
 * Binary writer for encoding GRC-20 data.
 */
export class Writer {
  private buffer: Uint8Array;
  private pos: number = 0;

  constructor(initialCapacity: number = 1024) {
    this.buffer = new Uint8Array(initialCapacity);
  }

  private ensureCapacity(needed: number): void {
    const required = this.pos + needed;
    if (required > this.buffer.length) {
      let newCapacity = this.buffer.length * 2;
      while (newCapacity < required) {
        newCapacity *= 2;
      }
      const newBuffer = new Uint8Array(newCapacity);
      newBuffer.set(this.buffer);
      this.buffer = newBuffer;
    }
  }

  /**
   * Returns the written bytes.
   */
  finish(): Uint8Array {
    return this.buffer.subarray(0, this.pos);
  }

  /**
   * Returns the current position (bytes written).
   */
  position(): number {
    return this.pos;
  }

  /**
   * Writes a single byte.
   */
  writeByte(value: number): void {
    this.ensureCapacity(1);
    this.buffer[this.pos++] = value;
  }

  /**
   * Writes raw bytes.
   */
  writeBytes(bytes: Uint8Array): void {
    this.ensureCapacity(bytes.length);
    this.buffer.set(bytes, this.pos);
    this.pos += bytes.length;
  }

  /**
   * Writes a 16-byte UUID (raw, no length prefix).
   * @throws Error if the ID is not exactly 16 bytes.
   */
  writeId(id: Id): void {
    if (id.length !== 16) {
      throw new Error(`writeId expects 16-byte ID, got ${id.length} bytes`);
    }
    this.writeBytes(id);
  }

  /**
   * Writes an unsigned varint (LEB128).
   */
  writeVarint(value: bigint): void {
    if (value < 0n) {
      throw new Error("writeVarint requires non-negative value");
    }
    this.ensureCapacity(10);
    let v = value;
    do {
      let byte = Number(v & 0x7fn);
      v >>= 7n;
      if (v !== 0n) {
        byte |= 0x80;
      }
      this.buffer[this.pos++] = byte;
    } while (v !== 0n);
  }

  /**
   * Writes an unsigned varint from a number.
   */
  writeVarintNumber(value: number): void {
    this.writeVarint(BigInt(value));
  }

  /**
   * Writes a signed varint (ZigZag encoded).
   */
  writeSignedVarint(value: bigint): void {
    this.writeVarint(zigzagEncode(value));
  }

  /**
   * Writes a length-prefixed string (UTF-8).
   */
  writeString(s: string): void {
    const bytes = new TextEncoder().encode(s);
    this.writeVarintNumber(bytes.length);
    this.writeBytes(bytes);
  }

  /**
   * Writes a length-prefixed byte array.
   */
  writeLengthPrefixedBytes(bytes: Uint8Array): void {
    this.writeVarintNumber(bytes.length);
    this.writeBytes(bytes);
  }

  /**
   * Writes a 64-bit float (IEEE 754, little-endian).
   */
  writeFloat64(value: number): void {
    this.ensureCapacity(8);
    const view = new DataView(this.buffer.buffer, this.buffer.byteOffset + this.pos, 8);
    view.setFloat64(0, value, true);
    this.pos += 8;
  }

  /**
   * Writes a vector of IDs with length prefix.
   */
  writeIdVec(ids: Id[]): void {
    this.writeVarintNumber(ids.length);
    for (const id of ids) {
      this.writeId(id);
    }
  }

  /**
   * Writes a 32-bit signed integer (little-endian).
   */
  writeInt32LE(value: number): void {
    this.ensureCapacity(4);
    const view = new DataView(this.buffer.buffer, this.buffer.byteOffset + this.pos, 4);
    view.setInt32(0, value, true);
    this.pos += 4;
  }

  /**
   * Writes a 16-bit signed integer (little-endian).
   */
  writeInt16LE(value: number): void {
    this.ensureCapacity(2);
    const view = new DataView(this.buffer.buffer, this.buffer.byteOffset + this.pos, 2);
    view.setInt16(0, value, true);
    this.pos += 2;
  }

  /**
   * Writes a 64-bit signed integer (little-endian).
   */
  writeInt64LE(value: bigint): void {
    this.ensureCapacity(8);
    const view = new DataView(this.buffer.buffer, this.buffer.byteOffset + this.pos, 8);
    view.setBigInt64(0, value, true);
    this.pos += 8;
  }

  /**
   * Writes a 48-bit signed integer (little-endian) using 6 bytes.
   * Values outside the 48-bit signed range will be truncated.
   */
  writeInt48LE(value: bigint): void {
    this.ensureCapacity(6);
    // Write as 64-bit then take only the low 6 bytes
    const tempBuffer = new ArrayBuffer(8);
    const view = new DataView(tempBuffer);
    view.setBigInt64(0, value, true);
    const bytes = new Uint8Array(tempBuffer);
    this.buffer.set(bytes.subarray(0, 6), this.pos);
    this.pos += 6;
  }
}

/**
 * Decode error with context.
 */
export class DecodeError extends Error {
  constructor(
    public code: string,
    message: string
  ) {
    super(`[${code}] ${message}`);
    this.name = "DecodeError";
  }
}

/**
 * Binary reader for decoding GRC-20 data.
 */
export class Reader {
  private buffer: Uint8Array;
  private pos: number = 0;

  constructor(buffer: Uint8Array) {
    this.buffer = buffer;
  }

  /**
   * Returns true if there are more bytes to read.
   */
  hasMore(): boolean {
    return this.pos < this.buffer.length;
  }

  /**
   * Returns the current position.
   */
  position(): number {
    return this.pos;
  }

  /**
   * Returns remaining bytes.
   */
  remaining(): number {
    return this.buffer.length - this.pos;
  }

  /**
   * Reads a single byte.
   */
  readByte(): number {
    if (this.pos >= this.buffer.length) {
      throw new DecodeError("E005", "unexpected end of input");
    }
    return this.buffer[this.pos++];
  }

  /**
   * Peeks at the next byte without consuming it.
   */
  peekByte(): number {
    if (this.pos >= this.buffer.length) {
      throw new DecodeError("E005", "unexpected end of input");
    }
    return this.buffer[this.pos];
  }

  /**
   * Reads raw bytes.
   */
  readBytes(n: number): Uint8Array {
    if (this.pos + n > this.buffer.length) {
      throw new DecodeError("E005", `unexpected end of input: need ${n} bytes, have ${this.buffer.length - this.pos}`);
    }
    const result = this.buffer.subarray(this.pos, this.pos + n);
    this.pos += n;
    return result;
  }

  /**
   * Reads a 16-byte UUID.
   */
  readId(): Id {
    return createId(new Uint8Array(this.readBytes(16)));
  }

  /**
   * Reads an unsigned varint (LEB128).
   */
  readVarint(): bigint {
    let result = 0n;
    let shift = 0n;
    let byteCount = 0;

    while (true) {
      if (this.pos >= this.buffer.length) {
        throw new DecodeError("E005", "unexpected end of input while reading varint");
      }

      const byte = this.buffer[this.pos++];
      byteCount++;

      if (byteCount > 10) {
        throw new DecodeError("E005", "varint exceeds maximum length (10 bytes)");
      }

      result |= BigInt(byte & 0x7f) << shift;
      shift += 7n;

      if ((byte & 0x80) === 0) {
        break;
      }
    }

    return result;
  }

  /**
   * Reads an unsigned varint as a number (throws if > MAX_SAFE_INTEGER).
   */
  readVarintNumber(): number {
    const value = this.readVarint();
    if (value > BigInt(Number.MAX_SAFE_INTEGER)) {
      throw new DecodeError("E005", "varint value exceeds safe integer range");
    }
    return Number(value);
  }

  /**
   * Reads a signed varint (ZigZag encoded).
   */
  readSignedVarint(): bigint {
    return zigzagDecode(this.readVarint());
  }

  /**
   * Reads a length-prefixed string (UTF-8).
   */
  readString(): string {
    const len = this.readVarintNumber();
    const bytes = this.readBytes(len);
    try {
      return new TextDecoder("utf-8", { fatal: true }).decode(bytes);
    } catch {
      throw new DecodeError("E004", "invalid UTF-8 in string");
    }
  }

  /**
   * Reads a length-prefixed byte array.
   */
  readLengthPrefixedBytes(): Uint8Array {
    const len = this.readVarintNumber();
    return new Uint8Array(this.readBytes(len));
  }

  /**
   * Reads a 64-bit float (IEEE 754, little-endian).
   */
  readFloat64(): number {
    const bytes = this.readBytes(8);
    const view = new DataView(bytes.buffer, bytes.byteOffset, 8);
    return view.getFloat64(0, true);
  }

  /**
   * Reads a vector of IDs with length prefix.
   */
  readIdVec(): Id[] {
    const count = this.readVarintNumber();
    const ids: Id[] = [];
    for (let i = 0; i < count; i++) {
      ids.push(this.readId());
    }
    return ids;
  }

  /**
   * Reads a 32-bit signed integer (little-endian).
   */
  readInt32LE(): number {
    const bytes = this.readBytes(4);
    const view = new DataView(bytes.buffer, bytes.byteOffset, 4);
    return view.getInt32(0, true);
  }

  /**
   * Reads a 16-bit signed integer (little-endian).
   */
  readInt16LE(): number {
    const bytes = this.readBytes(2);
    const view = new DataView(bytes.buffer, bytes.byteOffset, 2);
    return view.getInt16(0, true);
  }

  /**
   * Reads a 64-bit signed integer (little-endian).
   */
  readInt64LE(): bigint {
    const bytes = this.readBytes(8);
    const view = new DataView(bytes.buffer, bytes.byteOffset, 8);
    return view.getBigInt64(0, true);
  }

  /**
   * Reads a 48-bit signed integer (little-endian) from 6 bytes.
   */
  readInt48LE(): bigint {
    const bytes = this.readBytes(6);
    // Extend to 8 bytes for reading as int64, then sign-extend from 48 bits
    const tempBuffer = new ArrayBuffer(8);
    const tempBytes = new Uint8Array(tempBuffer);
    tempBytes.set(bytes, 0);
    // Sign-extend: if bit 47 is set, fill high bytes with 0xFF
    if (bytes[5] & 0x80) {
      tempBytes[6] = 0xff;
      tempBytes[7] = 0xff;
    }
    const view = new DataView(tempBuffer);
    return view.getBigInt64(0, true);
  }
}
