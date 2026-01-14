import type { Id } from "../types/id.js";
import type { CreateRelation } from "../types/op.js";

/**
 * Builder for CreateRelation operations with full control.
 */
export class RelationBuilder {
  private _id?: Id;
  private relationType?: Id;
  private from?: Id;
  private to?: Id;
  private entity?: Id;
  private position?: string;
  private fromSpace?: Id;
  private fromVersion?: Id;
  private toSpace?: Id;
  private toVersion?: Id;

  /**
   * Sets the relation ID.
   */
  id(id: Id): this {
    this._id = id;
    return this;
  }

  /**
   * Sets the relation type.
   */
  type(id: Id): this {
    this.relationType = id;
    return this;
  }

  /**
   * Sets the source entity.
   */
  fromEntity(id: Id): this {
    this.from = id;
    return this;
  }

  /**
   * Sets the target entity.
   */
  toEntity(id: Id): this {
    this.to = id;
    return this;
  }

  /**
   * Sets an explicit reified entity ID.
   */
  reifiedEntity(id: Id): this {
    this.entity = id;
    return this;
  }

  /**
   * Sets the position string for ordering.
   */
  atPosition(pos: string): this {
    this.position = pos;
    return this;
  }

  /**
   * Sets the from_space pin.
   */
  pinFromSpace(id: Id): this {
    this.fromSpace = id;
    return this;
  }

  /**
   * Sets the from_version pin.
   */
  pinFromVersion(id: Id): this {
    this.fromVersion = id;
    return this;
  }

  /**
   * Sets the to_space pin.
   */
  pinToSpace(id: Id): this {
    this.toSpace = id;
    return this;
  }

  /**
   * Sets the to_version pin.
   */
  pinToVersion(id: Id): this {
    this.toVersion = id;
    return this;
  }

  /**
   * Builds the CreateRelation, returning undefined if required fields are missing.
   */
  build(): CreateRelation | undefined {
    if (!this._id || !this.relationType || !this.from || !this.to) {
      return undefined;
    }

    return {
      type: "createRelation",
      id: this._id,
      relationType: this.relationType,
      from: this.from,
      to: this.to,
      entity: this.entity,
      position: this.position,
      fromSpace: this.fromSpace,
      fromVersion: this.fromVersion,
      toSpace: this.toSpace,
      toVersion: this.toVersion,
    };
  }
}
