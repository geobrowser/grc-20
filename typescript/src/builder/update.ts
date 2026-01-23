import type { Id } from "../types/id.js";
import type { UnsetLanguage, UnsetValue } from "../types/op.js";
import type { DecimalMantissa, PropertyValue, Value } from "../types/value.js";
import { EmbeddingSubType } from "../types/value.js";

/**
 * Builder for UpdateEntity operations.
 */
export class UpdateEntityBuilder {
  private readonly _id: Id;
  private _set: PropertyValue[] = [];
  private _unset: UnsetValue[] = [];

  constructor(id: Id) {
    this._id = id;
  }

  /**
   * Returns the entity ID.
   */
  get id(): Id {
    return this._id;
  }

  /**
   * Sets a property value.
   */
  set(property: Id, value: Value): this {
    this._set.push({ property, value });
    return this;
  }

  /**
   * Sets a TEXT value.
   */
  setText(property: Id, value: string, language?: Id): this {
    this._set.push({
      property,
      value: { type: "text", value, language },
    });
    return this;
  }

  /**
   * Sets an INT64 value.
   */
  setInt64(property: Id, value: bigint, unit?: Id): this {
    this._set.push({
      property,
      value: { type: "int64", value, unit },
    });
    return this;
  }

  /**
   * Sets a FLOAT64 value.
   */
  setFloat64(property: Id, value: number, unit?: Id): this {
    this._set.push({
      property,
      value: { type: "float64", value, unit },
    });
    return this;
  }

  /**
   * Sets a BOOL value.
   */
  setBool(property: Id, value: boolean): this {
    this._set.push({
      property,
      value: { type: "bool", value },
    });
    return this;
  }

  /**
   * Sets a BYTES value.
   */
  setBytes(property: Id, value: Uint8Array): this {
    this._set.push({
      property,
      value: { type: "bytes", value },
    });
    return this;
  }

  /**
   * Sets a POINT value (latitude, longitude, optional altitude).
   */
  setPoint(property: Id, lat: number, lon: number, alt?: number): this {
    this._set.push({
      property,
      value: { type: "point", lat, lon, alt },
    });
    return this;
  }

  /**
   * Sets a DATE value.
   * @param value - RFC 3339 date string (e.g., "2024-01-15" or "2024-01-15+05:30")
   */
  setDate(property: Id, value: string): this {
    this._set.push({
      property,
      value: { type: "date", value },
    });
    return this;
  }

  /**
   * Sets a TIME value.
   * @param value - RFC 3339 time string (e.g., "14:30:45.123456Z" or "14:30:45+05:30")
   */
  setTime(property: Id, value: string): this {
    this._set.push({
      property,
      value: { type: "time", value },
    });
    return this;
  }

  /**
   * Sets a DATETIME value.
   * @param value - RFC 3339 datetime string (e.g., "2024-01-15T14:30:45.123456Z")
   */
  setDatetime(property: Id, value: string): this {
    this._set.push({
      property,
      value: { type: "datetime", value },
    });
    return this;
  }

  /**
   * Sets a SCHEDULE value (RFC 5545 iCalendar format).
   */
  setSchedule(property: Id, value: string): this {
    this._set.push({
      property,
      value: { type: "schedule", value },
    });
    return this;
  }

  /**
   * Sets a DECIMAL value.
   */
  setDecimal(property: Id, exponent: number, mantissa: DecimalMantissa, unit?: Id): this {
    this._set.push({
      property,
      value: { type: "decimal", exponent, mantissa, unit },
    });
    return this;
  }

  /**
   * Sets a DECIMAL value from a bigint mantissa.
   */
  setDecimalI64(property: Id, exponent: number, mantissa: bigint, unit?: Id): this {
    return this.setDecimal(property, exponent, { type: "i64", value: mantissa }, unit);
  }

  /**
   * Sets an EMBEDDING value.
   */
  setEmbedding(
    property: Id,
    subType: EmbeddingSubType,
    dims: number,
    data: Uint8Array
  ): this {
    this._set.push({
      property,
      value: { type: "embedding", subType, dims, data },
    });
    return this;
  }

  /**
   * Unsets a specific property+language combination.
   */
  unset(property: Id, language: UnsetLanguage): this {
    this._unset.push({ property, language });
    return this;
  }

  /**
   * Unsets all values for a property (all languages).
   */
  unsetAll(property: Id): this {
    this._unset.push({ property, language: { type: "all" } });
    return this;
  }

  /**
   * Unsets the English value for a property.
   */
  unsetEnglish(property: Id): this {
    this._unset.push({ property, language: { type: "english" } });
    return this;
  }

  /**
   * Unsets a specific language for a property.
   */
  unsetLanguage(property: Id, language: Id): this {
    this._unset.push({
      property,
      language: { type: "specific", language },
    });
    return this;
  }

  /**
   * Returns the built set values array.
   */
  getSet(): PropertyValue[] {
    return this._set;
  }

  /**
   * Returns the built unset array.
   */
  getUnset(): UnsetValue[] {
    return this._unset;
  }
}
