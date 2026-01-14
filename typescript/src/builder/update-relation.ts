import type { Id } from "../types/id.js";
import type { UpdateRelation, UnsetRelationField } from "../types/op.js";

/**
 * Builder for UpdateRelation operations.
 */
export class UpdateRelationBuilder {
  private readonly _id: Id;
  private fromSpace?: Id;
  private fromVersion?: Id;
  private toSpace?: Id;
  private toVersion?: Id;
  private position?: string;
  private unsetFields: UnsetRelationField[] = [];

  constructor(id: Id) {
    this._id = id;
  }

  /**
   * Returns the relation ID.
   */
  get id(): Id {
    return this._id;
  }

  /**
   * Sets the from_space pin.
   */
  setFromSpace(id: Id): this {
    this.fromSpace = id;
    return this;
  }

  /**
   * Sets the from_version pin.
   */
  setFromVersion(id: Id): this {
    this.fromVersion = id;
    return this;
  }

  /**
   * Sets the to_space pin.
   */
  setToSpace(id: Id): this {
    this.toSpace = id;
    return this;
  }

  /**
   * Sets the to_version pin.
   */
  setToVersion(id: Id): this {
    this.toVersion = id;
    return this;
  }

  /**
   * Sets the position string.
   */
  setPosition(pos: string): this {
    this.position = pos;
    return this;
  }

  /**
   * Unsets the from_space pin.
   */
  unsetFromSpace(): this {
    this.unsetFields.push("fromSpace");
    return this;
  }

  /**
   * Unsets the from_version pin.
   */
  unsetFromVersion(): this {
    this.unsetFields.push("fromVersion");
    return this;
  }

  /**
   * Unsets the to_space pin.
   */
  unsetToSpace(): this {
    this.unsetFields.push("toSpace");
    return this;
  }

  /**
   * Unsets the to_version pin.
   */
  unsetToVersion(): this {
    this.unsetFields.push("toVersion");
    return this;
  }

  /**
   * Unsets the position.
   */
  unsetPosition(): this {
    this.unsetFields.push("position");
    return this;
  }

  /**
   * Builds the UpdateRelation operation.
   */
  build(): UpdateRelation {
    return {
      type: "updateRelation",
      id: this._id,
      fromSpace: this.fromSpace,
      fromVersion: this.fromVersion,
      toSpace: this.toSpace,
      toVersion: this.toVersion,
      position: this.position,
      unset: this.unsetFields,
    };
  }
}
